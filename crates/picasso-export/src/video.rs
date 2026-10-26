//! Video (and its sound) through FFmpeg's encoders.

use ff::codec::{self, encoder};
use ff::format::sample::{Sample, Type};
use ff::format::{self, Pixel};
use ff::{color, frame, ChannelLayout, Dictionary, Packet, Rational};
use ffmpeg_next as ff;
use picasso_audio::{frames_to_samples, plan, AudioPlan, AudioPool, Mixer, Offline, SAMPLE_RATE};
use picasso_core::Project;
use picasso_render::{render_frame, MediaCache};

use crate::burn::burn_in;
use crate::yuv::{rgba_to_yuv420p, rgba_to_yuv422p10};
use crate::{AudioCodec, ExportError, Options, Preset, Rate, Settings};
use crate::{BurnIn, Levels, PixelAspect};

fn enc_err(e: ff::Error) -> ExportError {
    ExportError::Encode(e.to_string())
}

/// What a preset asks of the video encoder.
struct VideoSpec {
    encoder: &'static str,
    pixel: Pixel,
    /// Options for the encoder, from the preset and what the Deliver page set.
    options: Vec<(&'static str, String)>,
    /// A rate in bits a second, when the page asked for one instead of a quality.
    bit_rate: Option<usize>,
}

fn video_spec(preset: Preset, o: &Options) -> Option<VideoSpec> {
    let rate = |h265: bool| match o.rate {
        Rate::Quality(q) => (vec![("crf", q.crf(h265).to_string())], None),
        Rate::Bitrate(kbps) => (Vec::new(), Some(kbps.max(1) as usize * 1000)),
    };
    match preset {
        Preset::H264Web => {
            let (mut options, bit_rate) = rate(false);
            options.push(("preset", o.speed.encoder_preset().to_string()));
            if let Some(profile) = o.profile.name() {
                options.push(("profile", profile.to_string()));
            }
            Some(VideoSpec {
                encoder: "libx264",
                pixel: Pixel::YUV420P,
                options,
                bit_rate,
            })
        }
        Preset::H265 => {
            let (mut options, bit_rate) = rate(true);
            options.push(("preset", o.speed.encoder_preset().to_string()));
            options.push(("x265-params", "log-level=error".to_string()));
            Some(VideoSpec {
                encoder: "libx265",
                pixel: Pixel::YUV420P,
                options,
                bit_rate,
            })
        }
        Preset::ProRes422 => Some(VideoSpec {
            encoder: "prores_ks",
            pixel: Pixel::YUV422P10LE,
            // 0 is Proxy, 1 LT, 2 ProRes 422 (standard), 3 HQ.
            options: vec![("profile", o.prores.min(3).to_string())],
            bit_rate: None,
        }),
        Preset::Wav | Preset::PngSequence => None,
    }
}

/// The sound encoder of a video file: AAC, or 16-bit PCM in a QuickTime file when asked.
fn audio_encoder_name(preset: Preset, o: &Options) -> &'static str {
    let pcm = match o.audio_codec {
        AudioCodec::Auto => preset == Preset::ProRes422,
        AudioCodec::Pcm => preset == Preset::ProRes422,
        AudioCodec::Aac => false,
    };
    if pcm {
        "pcm_s16le"
    } else {
        "aac"
    }
}

/// An open sound encoder and the samples waiting for it, as it wants them in blocks.
struct AudioOut {
    enc: encoder::Audio,
    stream: usize,
    stream_time_base: Rational,
    planar: bool,
    block: usize,
    /// Interleaved stereo samples not yet encoded.
    pending: Vec<f32>,
    /// Samples (per channel) encoded so far, the time of the next frame.
    encoded: i64,
}

impl AudioOut {
    fn send(
        &mut self,
        octx: &mut format::context::Output,
        count: usize,
    ) -> Result<(), ExportError> {
        let layout = ChannelLayout::STEREO;
        let sample = if self.planar {
            Sample::F32(Type::Planar)
        } else {
            Sample::I16(Type::Packed)
        };
        let mut frame = frame::Audio::new(sample, count, layout);
        frame.set_rate(SAMPLE_RATE);
        frame.set_pts(Some(self.encoded));
        let samples: Vec<f32> = self.pending.drain(..count * 2).collect();
        if self.planar {
            for (ch, plane) in [0usize, 1].into_iter().enumerate() {
                let dst = frame.plane_mut::<f32>(plane);
                for (d, s) in dst.iter_mut().zip(samples.iter().skip(ch).step_by(2)) {
                    *d = *s;
                }
            }
        } else {
            let dst = frame.plane_mut::<(i16, i16)>(0);
            for (d, s) in dst.iter_mut().zip(samples.chunks_exact(2)) {
                let q = |v: f32| (v.clamp(-1.0, 1.0) * 32767.0).round() as i16;
                *d = (q(s[0]), q(s[1]));
            }
        }
        self.encoded += count as i64;
        self.enc.send_frame(&frame).map_err(enc_err)?;
        self.drain(octx)
    }

    fn drain(&mut self, octx: &mut format::context::Output) -> Result<(), ExportError> {
        let mut packet = Packet::empty();
        while self.enc.receive_packet(&mut packet).is_ok() {
            packet.set_stream(self.stream);
            packet.rescale_ts(Rational(1, SAMPLE_RATE as i32), self.stream_time_base);
            packet.write_interleaved(octx).map_err(enc_err)?;
        }
        Ok(())
    }

    /// Encodes whole blocks of what is pending; with `all`, the rest as a last short block.
    fn flush_pending(
        &mut self,
        octx: &mut format::context::Output,
        all: bool,
    ) -> Result<(), ExportError> {
        while self.pending.len() / 2 >= self.block {
            self.send(octx, self.block)?;
        }
        let rest = self.pending.len() / 2;
        if all && rest > 0 {
            self.send(octx, rest)?;
        }
        Ok(())
    }

    fn finish(&mut self, octx: &mut format::context::Output) -> Result<(), ExportError> {
        self.flush_pending(octx, true)?;
        self.enc.send_eof().map_err(enc_err)?;
        self.drain(octx)
    }
}

pub(crate) fn encode(
    project: &Project,
    cache: &MediaCache,
    settings: &Settings,
    progress: &mut dyn FnMut(u64, u64) -> bool,
) -> Result<(), ExportError> {
    let preset = settings.preset;
    let o = &settings.options;
    let spec = video_spec(preset, o).ok_or(ExportError::Empty)?;
    let sound_plan: AudioPlan = plan(project);
    let want_video = o.video;
    let want_audio = o.audio && !sound_plan.clips.is_empty();
    if !want_video && !want_audio {
        return Err(ExportError::Empty);
    }
    let (width, height) = o.picture_size(project);
    let fps = o.frame_rate(project);
    // ProRes is always at video levels; the others follow the page.
    let full_range = o.levels == Levels::Full && preset != Preset::ProRes422;
    let time_base = Rational(fps.den as i32, fps.num as i32);

    let mut octx = format::output(&settings.path)
        .map_err(|e| ExportError::Write(format!("{}: {e}", settings.path.display())))?;
    let global_header = octx.format().flags().contains(format::Flags::GLOBAL_HEADER);

    // ---- video
    let mut video_index = 0;
    let mut venc: Option<encoder::Video> = None;
    if want_video {
        let codec =
            encoder::find_by_name(spec.encoder).ok_or(ExportError::NoEncoder(spec.encoder))?;
        let mut stream = octx.add_stream(codec).map_err(enc_err)?;
        video_index = stream.index();
        let mut enc = codec::context::Context::new_with_codec(codec)
            .encoder()
            .video()
            .map_err(enc_err)?;
        enc.set_width(width);
        enc.set_height(height);
        enc.set_format(spec.pixel);
        enc.set_time_base(time_base);
        enc.set_frame_rate(Some(Rational(fps.num as i32, fps.den as i32)));
        enc.set_gop(o.keyframes.unwrap_or(fps.nominal() as u32 * 2).max(1));
        if !o.frame_reordering || o.profile == crate::Profile::Baseline {
            enc.set_max_b_frames(0);
        }
        if let Some(rate) = spec.bit_rate {
            enc.set_bit_rate(rate);
        }
        enc.set_colorspace(color::Space::BT709);
        enc.set_color_range(if full_range {
            color::Range::JPEG
        } else {
            color::Range::MPEG
        });
        if o.pixel_aspect == PixelAspect::Cinemascope {
            enc.set_aspect_ratio(Rational(2, 1));
        }
        if global_header {
            enc.set_flags(codec::Flags::GLOBAL_HEADER);
        }
        let mut options = Dictionary::new();
        for (k, v) in &spec.options {
            options.set(k, v);
        }
        // What the file says about its colours; the pictures themselves are not changed.
        options.set("color_primaries", o.color_primaries.unwrap_or("bt709"));
        options.set("color_trc", o.color_trc.unwrap_or("bt709"));
        let opened = enc.open_with(options).map_err(enc_err)?;
        stream.set_parameters(&opened);
        venc = Some(opened);
    }

    // ---- sound, when anything makes a sound
    let mut audio: Option<AudioOut> = None;
    if want_audio {
        let name = audio_encoder_name(preset, o);
        let acodec = encoder::find_by_name(name).ok_or(ExportError::NoEncoder(name))?;
        let mut astream = octx.add_stream(acodec).map_err(enc_err)?;
        let stream_index = astream.index();
        let mut a = codec::context::Context::new_with_codec(acodec)
            .encoder()
            .audio()
            .map_err(enc_err)?;
        let planar = name == "aac";
        a.set_rate(SAMPLE_RATE as i32);
        a.set_channel_layout(ChannelLayout::STEREO);
        a.set_format(if planar {
            Sample::F32(Type::Planar)
        } else {
            Sample::I16(Type::Packed)
        });
        if planar {
            a.set_bit_rate(o.audio_kbps.clamp(32, 640) as usize * 1000);
        }
        a.set_time_base(Rational(1, SAMPLE_RATE as i32));
        if global_header {
            a.set_flags(codec::Flags::GLOBAL_HEADER);
        }
        let aenc = a.open_as(acodec).map_err(enc_err)?;
        astream.set_parameters(&aenc);
        let block = match aenc.frame_size() {
            0 => 1024,
            n => n as usize,
        };
        audio = Some(AudioOut {
            enc: aenc,
            stream: stream_index,
            stream_time_base: Rational(1, SAMPLE_RATE as i32),
            planar,
            block,
            pending: Vec::new(),
            encoded: 0,
        });
    }

    let timeline_frames = settings.end - settings.start;
    let total = o.output_frames(project, timeline_frames);
    if o.chapters {
        let marks: Vec<_> = project
            .markers
            .iter()
            .filter(|m| m.frame >= settings.start && m.frame < settings.end)
            .collect();
        for (i, mark) in marks.iter().enumerate() {
            let from = o
                .output_index(project, settings.start, mark.frame)
                .min(total);
            let to = marks
                .get(i + 1)
                .map_or(total, |next| {
                    o.output_index(project, settings.start, next.frame)
                })
                .clamp(from + 1, total.max(from + 1));
            octx.add_chapter(
                i as i64,
                time_base,
                from as i64,
                to as i64,
                mark.name.as_str(),
            )
            .map_err(enc_err)?;
        }
    }
    let mut header_options = Dictionary::new();
    if o.faststart {
        // Put the index at the front so the file can start playing while it downloads.
        header_options.set("movflags", "+faststart");
    }
    octx.write_header_with(header_options).map_err(enc_err)?;
    // The muxer may have changed the time bases when it wrote the header.
    let video_stream_tb = octx
        .stream(video_index)
        .map_or(time_base, |s| s.time_base());
    if let Some(a) = audio.as_mut() {
        if let Some(s) = octx.stream(a.stream) {
            a.stream_time_base = s.time_base();
        }
    }

    // ---- frames
    let cache = cache.blocking();
    let pool = AudioPool::new(|| {});
    let source = Offline(&pool);
    let mut mixer = Mixer::new();
    let audio_origin = frames_to_samples(project, settings.start);
    let audio_length = frames_to_samples(project, settings.end) - audio_origin;
    let mut audio_done = 0u64; // samples mixed so far
    let mut vframe = frame::Video::new(spec.pixel, width, height);
    vframe.set_color_space(color::Space::BT709);
    vframe.set_color_range(if full_range {
        color::Range::JPEG
    } else {
        color::Range::MPEG
    });
    let mut packet = Packet::empty();
    for n in 0..total {
        let timeline_frame = o.timeline_frame(project, settings.start, settings.end, n);
        if let Some(venc) = venc.as_mut() {
            let mut picture = render_frame(project, &cache, timeline_frame, width, height)
                .map_err(|e| ExportError::Render(e.to_string()))?;
            if o.burn_in != BurnIn::Off {
                let clock = project.fps.timecode(o.clock_start + timeline_frame);
                let name = project
                    .tracks
                    .iter()
                    .rev()
                    .flat_map(|t| t.clips.iter())
                    .find(|c| c.enabled && c.contains(timeline_frame))
                    .map(|c| c.name.as_str());
                let (code, name) = match o.burn_in {
                    BurnIn::Timecode => (Some(clock.as_str()), None),
                    BurnIn::ClipName => (None, name),
                    _ => (Some(clock.as_str()), name),
                };
                burn_in(
                    &mut picture.rgba,
                    (width as usize, height as usize),
                    code,
                    name,
                );
            }
            let strides = [vframe.stride(0), vframe.stride(1), vframe.stride(2)];
            {
                // The planes are separate buffers; take them one after the other.
                let mut y = vec![0u8; strides[0] * height as usize];
                let rows_c = if spec.pixel == Pixel::YUV420P {
                    height as usize / 2
                } else {
                    height as usize
                };
                let mut u = vec![0u8; strides[1] * rows_c];
                let mut v = vec![0u8; strides[2] * rows_c];
                let size = (width as usize, height as usize);
                if spec.pixel == Pixel::YUV420P {
                    rgba_to_yuv420p(
                        &picture.rgba,
                        size,
                        [&mut y, &mut u, &mut v],
                        strides,
                        full_range,
                    );
                } else {
                    rgba_to_yuv422p10(
                        &picture.rgba,
                        size,
                        [&mut y, &mut u, &mut v],
                        strides,
                        false,
                    );
                }
                vframe.data_mut(0).copy_from_slice(&y);
                vframe.data_mut(1).copy_from_slice(&u);
                vframe.data_mut(2).copy_from_slice(&v);
            }
            vframe.set_pts(Some(n as i64));
            venc.send_frame(&vframe).map_err(enc_err)?;
            while venc.receive_packet(&mut packet).is_ok() {
                packet.set_stream(video_index);
                packet.rescale_ts(time_base, video_stream_tb);
                packet.write_interleaved(&mut octx).map_err(enc_err)?;
            }
        }
        // The sound up to the end of this picture.
        if let Some(a) = audio.as_mut() {
            let target = if n + 1 == total {
                audio_length
            } else {
                (u128::from(n + 1) * u128::from(fps.den) * u128::from(SAMPLE_RATE)
                    / u128::from(fps.num.max(1)))
                .min(u128::from(audio_length)) as u64
            };
            let count = target.saturating_sub(audio_done) as usize;
            if count > 0 {
                let mut block = vec![0.0f32; count * 2];
                mixer.mix(&sound_plan, &source, audio_origin + audio_done, &mut block);
                a.pending.extend_from_slice(&block);
                audio_done = target;
                a.flush_pending(&mut octx, false)?;
            }
        }
        if !progress(n + 1, total) {
            return Err(ExportError::Cancelled);
        }
    }

    // ---- finish
    if let Some(venc) = venc.as_mut() {
        venc.send_eof().map_err(enc_err)?;
        while venc.receive_packet(&mut packet).is_ok() {
            packet.set_stream(video_index);
            packet.rescale_ts(time_base, video_stream_tb);
            packet.write_interleaved(&mut octx).map_err(enc_err)?;
        }
    }
    if let Some(a) = audio.as_mut() {
        a.finish(&mut octx)?;
    }
    octx.write_trailer().map_err(enc_err)?;
    Ok(())
}
