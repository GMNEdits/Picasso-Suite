//! Audio from media files, as 48 kHz stereo `f32` (Agents.md: a 32-bit float pipeline at 48 kHz).

use std::path::Path;

use ff::format::context::Input;
use ff::format::sample::{Sample, Type};
use ff::media::Type as MediaType;
use ff::software::resampling::Context as Resampler;
use ff::ChannelLayout;
use ffmpeg_next as ff;

use crate::{init, DecodeError, MICROS};

/// The sample rate of everything the program mixes.
pub const SAMPLE_RATE: u32 = 48_000;
/// Samples (per channel) that a waveform bucket covers: 1/50 s.
pub const WAVEFORM_BUCKET: u64 = SAMPLE_RATE as u64 / 50;
/// Decoding stops after this many samples per channel (one hour) so a hostile file cannot fill
/// memory with an endless stream.
const MAX_SAMPLES: u64 = SAMPLE_RATE as u64 * 3600;

/// Reads the audio of a file in pieces, from any position.
pub struct AudioDecoder {
    input: Input,
    stream: usize,
    decoder: ff::decoder::Audio,
    resampler: Resampler,
    time_base: ff::Rational,
    /// The channel layout the resampler was made for; decoded frames that do not say get it.
    layout: ChannelLayout,
    /// Decoded samples (interleaved stereo) that start at sample `queue_start`.
    queue: Vec<f32>,
    queue_start: Option<i64>,
    eof: bool,
}

impl AudioDecoder {
    /// Opens the audio of `path`; `None` when the file has no audio.
    pub fn open(path: &Path) -> Result<Option<Self>, DecodeError> {
        crate::guard(path)?;
        init();
        let input = ff::format::input(path).map_err(|e| DecodeError::Open(e.to_string()))?;
        let Some(stream) = input.streams().best(MediaType::Audio) else {
            return Ok(None);
        };
        let (index, params, time_base) = (stream.index(), stream.parameters(), stream.time_base());
        let decoder = ff::codec::context::Context::from_parameters(params)?
            .decoder()
            .audio()?;
        let layout = if decoder.channel_layout().is_empty() {
            ChannelLayout::default(i32::from(decoder.channels().max(1)))
        } else {
            decoder.channel_layout()
        };
        let resampler = Resampler::get(
            decoder.format(),
            layout,
            decoder.rate(),
            Sample::F32(Type::Packed),
            ChannelLayout::STEREO,
            SAMPLE_RATE,
        )?;
        Ok(Some(Self {
            input,
            stream: index,
            decoder,
            resampler,
            time_base,
            layout,
            queue: Vec::new(),
            queue_start: None,
            eof: false,
        }))
    }

    fn queue_end(&self) -> Option<i64> {
        Some(self.queue_start? + (self.queue.len() / 2) as i64)
    }

    fn to_sample(&self, pts: i64) -> i64 {
        let tb = self.time_base;
        let us = pts
            .saturating_mul(i64::from(tb.numerator()))
            .saturating_mul(MICROS)
            / i64::from(tb.denominator()).max(1);
        us.saturating_mul(i64::from(SAMPLE_RATE)) / MICROS
    }

    fn push(&mut self, out: &ff::frame::Audio, first_pts: Option<i64>) {
        let n = out.samples();
        let Some(bytes) = out.data(0).get(..n * 8) else {
            return;
        };
        if self.queue_start.is_none() {
            self.queue_start = Some(first_pts.map_or(0, |p| self.to_sample(p)));
        }
        self.queue.extend(
            bytes
                .chunks_exact(4)
                .map(|b| f32::from_ne_bytes([b[0], b[1], b[2], b[3]])),
        );
    }

    /// Decodes a little more into the queue. False at the end of the file.
    fn fill(&mut self) -> Result<bool, DecodeError> {
        if self.eof {
            return Ok(false);
        }
        let mut packet = ff::Packet::empty();
        match packet.read(&mut self.input) {
            Ok(()) => {
                if packet.stream() == self.stream {
                    self.decoder.send_packet(&packet)?;
                }
            }
            Err(ff::Error::Eof) => {
                self.decoder.send_eof()?;
                self.eof = true;
            }
            Err(e) => return Err(e.into()),
        }
        let mut frame = ff::frame::Audio::empty();
        while self.decoder.receive_frame(&mut frame).is_ok() {
            let pts = frame.pts();
            if frame.channel_layout().is_empty() {
                frame.set_channel_layout(self.layout);
            }
            // Room for the converted samples: more of them than came in when the rate goes up.
            let room =
                frame.samples() * SAMPLE_RATE as usize / self.decoder.rate().max(1) as usize + 256;
            let mut out =
                ff::frame::Audio::new(Sample::F32(Type::Packed), room, ChannelLayout::STEREO);
            self.resampler.run(&frame, &mut out)?;
            self.push(&out, pts);
        }
        if self.eof {
            let mut out =
                ff::frame::Audio::new(Sample::F32(Type::Packed), 8192, ChannelLayout::STEREO);
            if self.resampler.flush(&mut out).is_ok() {
                self.push(&out, None);
            }
        }
        Ok(!self.eof)
    }

    fn seek(&mut self, sample: i64) -> Result<(), DecodeError> {
        let us = sample.max(0).saturating_mul(MICROS) / i64::from(SAMPLE_RATE);
        self.input.seek(us, ..us.saturating_add(1))?;
        self.decoder.flush();
        self.queue.clear();
        self.queue_start = None;
        self.eof = false;
        Ok(())
    }

    /// `count` stereo samples (interleaved, so `2 * count` numbers) from sample `start` on.
    /// Where the file has no sound (before its start, after its end) the samples are zero.
    pub fn read_at(&mut self, start: u64, count: usize) -> Result<Vec<f32>, DecodeError> {
        let start = i64::try_from(start.min(MAX_SAMPLES)).unwrap_or(0);
        let want_end = start + i64::try_from(count).unwrap_or(0);
        // Continue where the last read stopped, otherwise seek.
        let continuing = match (self.queue_start, self.queue_end()) {
            (Some(from), Some(to)) => start >= from && start <= to + i64::from(SAMPLE_RATE),
            _ => false,
        };
        if !continuing {
            self.seek(start)?;
        }
        while self.queue_end().is_none_or(|e| e < want_end) {
            if !self.fill()? {
                break;
            }
        }
        let mut out = vec![0.0; count * 2];
        if let Some(from) = self.queue_start {
            for (i, frame) in out.chunks_exact_mut(2).enumerate() {
                let at = start + i as i64 - from;
                if let Ok(at) = usize::try_from(at) {
                    if let Some(s) = self.queue.get(at * 2..at * 2 + 2) {
                        frame.copy_from_slice(s);
                    }
                }
            }
            // Forget what is behind the read position.
            let drop_to = usize::try_from(start - from)
                .unwrap_or(0)
                .min(self.queue.len() / 2);
            if drop_to > 0 {
                self.queue.drain(..drop_to * 2);
                self.queue_start = Some(from + drop_to as i64);
            }
        }
        Ok(out)
    }

    /// True once the whole file has been decoded and read.
    fn finished(&self) -> bool {
        self.eof && self.queue.is_empty()
    }
}

/// How long the sound of a file lasts, in microseconds (`None` when the container does not say).
/// An error when the file has no sound.
pub fn probe_audio(path: &Path) -> Result<Option<i64>, DecodeError> {
    crate::guard(path)?;
    init();
    let input = ff::format::input(path).map_err(|e| DecodeError::Open(e.to_string()))?;
    let stream = input
        .streams()
        .best(MediaType::Audio)
        .ok_or(DecodeError::NoAudio)?;
    let duration = input.duration();
    let from_stream = {
        let tb = stream.time_base();
        let d = stream.duration();
        (d > 0).then(|| {
            d.saturating_mul(i64::from(tb.numerator()))
                .saturating_mul(MICROS)
                / i64::from(tb.denominator()).max(1)
        })
    };
    Ok((duration > 0).then_some(duration).or(from_stream))
}

/// The loudness of a file over time, for drawing a waveform: one byte (0..=255, the peak) per
/// [`WAVEFORM_BUCKET`] samples. `None` when the file has no audio.
pub fn waveform(path: &Path) -> Result<Option<Vec<u8>>, DecodeError> {
    let Some(mut decoder) = AudioDecoder::open(path)? else {
        return Ok(None);
    };
    let mut peaks = Vec::new();
    let mut at = 0u64;
    let chunk = WAVEFORM_BUCKET as usize * 50;
    while at < MAX_SAMPLES {
        let block = decoder.read_at(at, chunk)?;
        for bucket in block.chunks(WAVEFORM_BUCKET as usize * 2) {
            let peak = bucket.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            peaks.push((peak.min(1.0) * 255.0).round() as u8);
        }
        at += chunk as u64;
        if decoder.finished() {
            break;
        }
    }
    // Drop the silence the last block was padded with.
    while peaks.last() == Some(&0) {
        peaks.pop();
    }
    Ok(Some(peaks))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-second 440 Hz tone as a WAV file (44.1 kHz mono, so it is resampled); `None` when
    /// the `ffmpeg` program is not installed.
    fn tone(name: &str) -> Option<std::path::PathBuf> {
        let path = std::env::temp_dir().join(name);
        std::process::Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-f", "lavfi", "-i"])
            .arg("sine=frequency=440:duration=2:sample_rate=44100")
            .arg(&path)
            .status()
            .is_ok_and(|s| s.success())
            .then_some(path)
    }

    fn peak(samples: &[f32]) -> f32 {
        samples.iter().fold(0.0, |m, s| s.abs().max(m))
    }

    #[test]
    fn decodes_resamples_and_seeks() -> Result<(), String> {
        let Some(path) = tone("picasso_tone_test.wav") else {
            return Ok(());
        };
        let mut d = AudioDecoder::open(&path)
            .map_err(|e| e.to_string())?
            .ok_or("no audio")?;
        let first = d.read_at(0, 48_000).map_err(|e| e.to_string())?;
        assert_eq!(first.len(), 96_000, "one second of stereo");
        assert!(peak(&first) > 0.05, "the tone is there");
        // The same sound in both channels (the tone was mono).
        assert!(first.chunks_exact(2).all(|f| (f[0] - f[1]).abs() < 1e-6));
        // Continuing, then jumping back and forward.
        let second = d.read_at(48_000, 48_000).map_err(|e| e.to_string())?;
        assert!(peak(&second) > 0.05);
        let again = d.read_at(24_000, 1000).map_err(|e| e.to_string())?;
        assert!(peak(&again) > 0.05);
        // The tone runs to the end of the two seconds, 96000 samples.
        let late = d.read_at(90_000, 1000).map_err(|e| e.to_string())?;
        assert!(
            peak(&late) > 0.05,
            "all of the file is there, not only part"
        );
        // Past the end of the two seconds there is silence.
        let after = d.read_at(96_000 + 5000, 1000).map_err(|e| e.to_string())?;
        assert_eq!(peak(&after), 0.0);
        let _ = std::fs::remove_file(path);
        Ok(())
    }

    #[test]
    fn the_waveform_has_a_bucket_per_fiftieth_of_a_second() -> Result<(), String> {
        let Some(path) = tone("picasso_wave_test.wav") else {
            return Ok(());
        };
        let peaks = waveform(&path)
            .map_err(|e| e.to_string())?
            .ok_or("no audio")?;
        assert!((95..=101).contains(&peaks.len()), "{}", peaks.len());
        assert!(peaks.iter().all(|p| *p > 10), "a steady tone");
        let _ = std::fs::remove_file(path);
        Ok(())
    }

    #[test]
    fn a_file_without_sound_has_no_audio() -> Result<(), String> {
        let path = std::env::temp_dir().join("picasso_silent_test.mp4");
        let made = std::process::Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-f", "lavfi", "-i"])
            .arg("testsrc=duration=1:size=64x64:rate=24")
            .args(["-pix_fmt", "yuv420p"])
            .arg(&path)
            .status()
            .is_ok_and(|s| s.success());
        if !made {
            return Ok(());
        }
        assert!(AudioDecoder::open(&path)
            .map_err(|e| e.to_string())?
            .is_none());
        let _ = std::fs::remove_file(path);
        Ok(())
    }
}
