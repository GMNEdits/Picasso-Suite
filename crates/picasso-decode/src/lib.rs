//! Reads video files through FFmpeg (Agents.md: "Decode: ffmpeg-next"). Native only; the browser
//! build has no FFmpeg. Times are whole microseconds (`i64`), never floating-point seconds.

pub mod audio;
pub mod hw;

use std::path::{Path, PathBuf};
use std::sync::Once;

use ff::format::{context::Input, Pixel};
use ff::media::Type;
use ff::software::scaling::{context::Context as Scaler, flag::Flags};
use ffmpeg_next as ff;
use thiserror::Error;

/// Microseconds in a second, FFmpeg's own `AV_TIME_BASE`.
pub(crate) const MICROS: i64 = 1_000_000;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("could not read the file: {0}")]
    Open(String),
    #[error("{0}")]
    Refused(String),
    #[error("the file has no video")]
    NoVideo,
    #[error("the file has no sound")]
    NoAudio,
    #[error("could not decode: {0}")]
    Decode(String),
    #[error("there is no frame at that time")]
    NoFrame,
}

impl From<ff::Error> for DecodeError {
    fn from(e: ff::Error) -> Self {
        Self::Decode(e.to_string())
    }
}

/// What a file contains.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaInfo {
    pub width: u32,
    pub height: u32,
    /// Frame rate as a fraction (frames, seconds).
    pub fps: (u32, u32),
    /// Length in microseconds; `None` when the container does not say.
    pub duration_us: Option<i64>,
    pub video_codec: String,
    pub audio_codec: Option<String>,
    /// The video codec's profile ("High", "Main 10"), when the codec has profiles.
    pub profile: Option<String>,
    /// Its level ("4.0", "5.1"), read from the codec's configuration in the file.
    pub level: Option<String>,
}

impl MediaInfo {
    /// The codec as editors show it: "H.264 High L4.0", "H.265 Main L5.1", "VP9".
    pub fn codec_label(&self) -> String {
        let name = match self.video_codec.as_str() {
            "h264" => "H.264".to_string(),
            "hevc" => "H.265".to_string(),
            "prores" => "Apple ProRes".to_string(),
            "av1" => "AV1".to_string(),
            "vp9" => "VP9".to_string(),
            "vp8" => "VP8".to_string(),
            "mpeg4" => "MPEG-4".to_string(),
            "mjpeg" => "Motion JPEG".to_string(),
            other => other.to_uppercase(),
        };
        let mut label = name;
        if let Some(p) = &self.profile {
            label.push(' ');
            label.push_str(p);
        }
        if let Some(l) = &self.level {
            label.push_str(" L");
            label.push_str(l);
        }
        label
    }
}

/// A readable name for a codec profile (`H264(High10)` is "High 10").
fn profile_name(profile: ff::codec::Profile) -> Option<String> {
    use ff::codec::Profile;
    let text = match profile {
        Profile::Unknown | Profile::Reserved => return None,
        other => format!("{other:?}"),
    };
    // "H264(ConstrainedBaseline)" -> "Constrained Baseline".
    let inner = text
        .split_once('(')
        .map_or(text.as_str(), |(_, rest)| rest.trim_end_matches(')'));
    let mut out = String::new();
    for (i, c) in inner.chars().enumerate() {
        let after_letter = out.chars().last().is_some_and(|p: char| p.is_alphabetic());
        if i > 0 && ((c.is_uppercase() && after_letter) || (c.is_ascii_digit() && after_letter)) {
            out.push(' ');
        }
        out.push(c);
    }
    (!out.is_empty()).then_some(out)
}

/// The level of an H.264 or H.265 stream, from its configuration record in the file (`avcC` or
/// `hvcC`), which sits near the start or the end of an MP4 or QuickTime file.
fn level_of(path: &Path, codec: &str) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    const PART: u64 = 4 << 20;
    let (tag, at): (&[u8], usize) = match codec {
        "h264" => (b"avcC", 3),
        "hevc" => (b"hvcC", 12),
        _ => return None,
    };
    let mut file = std::fs::File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    let mut found = None;
    for start in [0, size.saturating_sub(PART)] {
        if file.seek(SeekFrom::Start(start)).is_err() {
            continue;
        }
        let mut buf = Vec::new();
        if (&mut file).take(PART).read_to_end(&mut buf).is_err() {
            continue;
        }
        if let Some(i) = buf.windows(4).position(|w| w == tag) {
            // The record starts after the four letters.
            found = buf.get(i + 4 + at).copied();
            if found.is_some() {
                break;
            }
        }
    }
    let idc = found.filter(|v| *v > 0)?;
    Some(if codec == "hevc" {
        // H.265 levels are 30 times the level: 153 is 5.1.
        let tenths = u32::from(idc) / 3;
        format!("{}.{}", tenths / 10, tenths % 10)
    } else {
        format!("{}.{}", idc / 10, idc % 10)
    })
}

/// One decoded picture, 8-bit RGBA.
#[derive(Clone, Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// Presentation time in microseconds.
    pub pts_us: i64,
    pub rgba: Vec<u8>,
}

/// Anything that can give the picture at a time (Agents.md: `FrameProvider`).
pub trait FrameProvider {
    fn frame_at(&mut self, pts_us: i64) -> Result<Frame, DecodeError>;
}

pub(crate) fn init() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        if let Err(e) = ff::init() {
            tracing::error!("ffmpeg did not start: {e}");
        }
        ff::util::log::set_level(ff::util::log::Level::Error);
    });
}

/// Why a media path must not be opened on its own, or `None` when it is an ordinary file path.
///
/// Project files and Resolve files carry media paths, and a file from someone else must not be
/// able to make the program reach out:
/// - FFmpeg reads `http://...`, `ftp:`, `concat:`, `pipe:`, `subfile,...` and the like as protocols
///   and not as file names, so those are refused;
/// - on Windows, touching `\server\share` sends the user's login hash to that server, so network
///   paths are refused unless `PICASSO_ALLOW_NETWORK_MEDIA` is set. Mapped drive letters are fine.
pub fn refuse_path(path: &str) -> Option<String> {
    let p = path.trim_start();
    let scheme_len = p
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
        .count();
    // One letter before the colon is a drive (`C:`); more is a protocol.
    if p.as_bytes().get(scheme_len) == Some(&b':') && scheme_len > 1 {
        return Some(format!(
            "{}: is a network or special address, not a file on this computer",
            &p[..scheme_len]
        ));
    }
    if p.starts_with("subfile,") {
        return Some("special FFmpeg addresses are not opened".to_string());
    }
    if (p.starts_with("\\\\") || p.starts_with("//"))
        && std::env::var_os("PICASSO_ALLOW_NETWORK_MEDIA").is_none()
    {
        return Some(
            "network paths are not opened automatically (set PICASSO_ALLOW_NETWORK_MEDIA=1 to allow them)".to_string(),
        );
    }
    None
}

fn guard(path: &Path) -> Result<(), DecodeError> {
    match path.to_str().and_then(refuse_path) {
        Some(why) => Err(DecodeError::Refused(why)),
        None => Ok(()),
    }
}

fn open(path: &Path) -> Result<Input, DecodeError> {
    guard(path)?;
    init();
    ff::format::input(path).map_err(|e| DecodeError::Open(e.to_string()))
}

fn rational(r: ff::Rational) -> (u32, u32) {
    (
        u32::try_from(r.numerator()).unwrap_or(0),
        u32::try_from(r.denominator()).unwrap_or(1).max(1),
    )
}

/// Reads the metadata of a file without decoding it.
pub fn probe(path: &Path) -> Result<MediaInfo, DecodeError> {
    let input = open(path)?;
    let video = input
        .streams()
        .best(Type::Video)
        .ok_or(DecodeError::NoVideo)?;
    let ctx = ff::codec::context::Context::from_parameters(video.parameters())?;
    let codec_name = ctx.id().name().to_string();
    let decoder = ctx.decoder().video()?;
    let profile = profile_name(decoder.profile());
    let fps = {
        let r = rational(video.avg_frame_rate());
        if r.0 == 0 {
            rational(video.rate())
        } else {
            r
        }
    };
    let duration = input.duration();
    Ok(MediaInfo {
        width: decoder.width(),
        height: decoder.height(),
        fps,
        duration_us: (duration > 0).then_some(duration),
        level: level_of(path, &codec_name),
        profile,
        video_codec: codec_name,
        audio_codec: input.streams().best(Type::Audio).and_then(|a| {
            ff::codec::context::Context::from_parameters(a.parameters())
                .ok()
                .map(|c| c.id().name().to_string())
        }),
    })
}

/// Decodes the pictures of one file, on request.
pub struct VideoDecoder {
    path: PathBuf,
    /// The pictures come from the graphics card ([`hw`]).
    on_card: bool,
    /// Whether hardware decoding was asked for when the file was opened.
    asked: bool,
    input: Input,
    stream: usize,
    decoder: ff::decoder::Video,
    time_base: ff::Rational,
    /// The last picture at or before the time asked for, kept so playing forward needs no seek.
    best: Option<(ff::frame::Video, i64)>,
    /// The first picture after it.
    ahead: Option<(ff::frame::Video, i64)>,
    /// The end of the file was read and the decoder drained: nothing more comes until a seek.
    at_end: bool,
    /// The converter to RGBA, kept while the pictures keep their format and size.
    scaler: Option<Scaler>,
}

impl VideoDecoder {
    /// Opens the video of a file, decoding it on the graphics card when System Preferences says
    /// so and the card can.
    pub fn open(path: &Path) -> Result<Self, DecodeError> {
        Self::open_with(path, hw::hardware_decoding())
    }

    fn open_with(path: &Path, hardware: bool) -> Result<Self, DecodeError> {
        let input = open(path)?;
        let (stream, params, time_base) = {
            let s = input
                .streams()
                .best(Type::Video)
                .ok_or(DecodeError::NoVideo)?;
            (s.index(), s.parameters(), s.time_base())
        };
        let mut context = ff::codec::context::Context::from_parameters(params)?;
        let on_card = hardware && hw::attach(&mut context);
        // Decode on every core: pictures of a 1080p or 4K file do not arrive in time on one. The
        // card needs no threads, and each would hold more of its memory.
        context.set_threading(ff::threading::Config {
            kind: ff::threading::Type::Frame,
            count: if on_card { 1 } else { 0 },
        });
        let decoder = context.decoder().video()?;
        Ok(Self {
            path: path.to_path_buf(),
            on_card,
            asked: hardware,
            input,
            stream,
            decoder,
            time_base,
            best: None,
            ahead: None,
            at_end: false,
            scaler: None,
        })
    }

    fn to_us(&self, pts: i64) -> i64 {
        let tb = self.time_base;
        pts.saturating_mul(i64::from(tb.numerator()))
            .saturating_mul(MICROS)
            / i64::from(tb.denominator()).max(1)
    }

    fn convert(
        &mut self,
        decoded: &ff::frame::Video,
        pts_us: i64,
        reduce: u32,
    ) -> Result<Frame, DecodeError> {
        // A proxy: the picture is made smaller while it is converted, which costs less than
        // converting it whole and scaling it afterwards.
        let reduce = reduce.max(1);
        let out_w = (decoded.width() / reduce).max(2);
        let out_h = (decoded.height() / reduce).max(2);
        let same = self.scaler.as_ref().is_some_and(|s| {
            let (input, output) = (s.input(), s.output());
            input.format == decoded.format()
                && input.width == decoded.width()
                && input.height == decoded.height()
                && output.width == out_w
                && output.height == out_h
        });
        if !same {
            self.scaler = Some(Scaler::get(
                decoded.format(),
                decoded.width(),
                decoded.height(),
                Pixel::RGBA,
                out_w,
                out_h,
                Flags::FAST_BILINEAR,
            )?);
        }
        let scaler = self.scaler.as_mut().ok_or(DecodeError::NoFrame)?;
        let mut rgba = ff::frame::Video::empty();
        scaler.run(decoded, &mut rgba)?;
        let (w, h) = (rgba.width() as usize, rgba.height() as usize);
        let stride = rgba.stride(0);
        let data = rgba.data(0);
        let mut out = Vec::with_capacity(w * h * 4);
        for row in 0..h {
            let start = row * stride;
            out.extend_from_slice(data.get(start..start + w * 4).ok_or(DecodeError::NoFrame)?);
        }
        Ok(Frame {
            width: rgba.width(),
            height: rgba.height(),
            pts_us,
            rgba: out,
        })
    }
}

impl FrameProvider for VideoDecoder {
    fn frame_at(&mut self, pts_us: i64) -> Result<Frame, DecodeError> {
        self.frame_at_reduced(pts_us, 1)
    }
}

impl VideoDecoder {
    /// The picture shown at `pts_us`: the last one that starts at or before it, made `reduce`
    /// times smaller on each side (1 is the whole picture). Asking for times that move forward a
    /// little each time decodes on from where it was; anything else seeks to the key frame before
    /// the time.
    ///
    /// If the graphics card fails, the file is opened again and decoded on the processor.
    pub fn frame_at_reduced(&mut self, pts_us: i64, reduce: u32) -> Result<Frame, DecodeError> {
        match self.read(pts_us, reduce) {
            Err(e) if self.on_card => {
                tracing::warn!(
                    "hardware decoding of {} failed ({e}); decoding on the processor",
                    self.path.display()
                );
                let asked = self.asked;
                *self = Self::open_with(&self.path.clone(), false)?;
                // Still counts as opened with the setting on, so it is not opened again for it.
                self.asked = asked;
                self.read(pts_us, reduce)
            }
            other => other,
        }
    }

    /// Whether System Preferences turned hardware decoding on or off since the file was opened,
    /// so it should be opened again.
    pub fn is_stale(&self) -> bool {
        self.asked != hw::hardware_decoding()
    }

    fn read(&mut self, pts_us: i64, reduce: u32) -> Result<Frame, DecodeError> {
        let target = pts_us.max(0);
        let continuing = self
            .best
            .as_ref()
            .is_some_and(|(_, t)| target >= *t && target - *t <= 2 * MICROS);
        if !continuing {
            // Jump to the key frame at or before the target, then decode forward.
            self.input.seek(target, ..target.saturating_add(1))?;
            self.decoder.flush();
            self.best = None;
            self.ahead = None;
            self.at_end = false;
        }
        // The picture that was already waiting.
        if let Some((frame, t)) = self.ahead.take() {
            if t <= target {
                self.best = Some((frame, t));
            } else {
                self.ahead = Some((frame, t));
            }
        }
        let mut decoded = ff::frame::Video::empty();
        let mut done = self.ahead.is_some();
        while !done {
            if !self.at_end {
                let mut packet = ff::Packet::empty();
                match packet.read(&mut self.input) {
                    Ok(()) => {
                        if packet.stream() != self.stream {
                            continue;
                        }
                        self.decoder.send_packet(&packet)?;
                    }
                    Err(ff::Error::Eof) => {
                        // Once: the decoder then gives what it holds, and past the last
                        // picture the last one stays.
                        self.decoder.send_eof()?;
                        self.at_end = true;
                    }
                    Err(e) => return Err(e.into()),
                }
            }
            let mut got = false;
            while self.decoder.receive_frame(&mut decoded).is_ok() {
                got = true;
                // The best-effort time also covers streams whose pictures carry no time of their own.
                let t = self.to_us(decoded.timestamp().unwrap_or(0));
                // Pictures on the card are copied out at once: the card has few of them.
                let picture = if hw::on_card(&decoded) {
                    hw::download(&decoded)?
                } else {
                    decoded.clone()
                };
                if t <= target {
                    self.best = Some((picture, t));
                } else {
                    self.ahead = Some((picture, t));
                    done = true;
                    break;
                }
            }
            if self.at_end && !got {
                done = true;
            }
        }
        // Before the first picture of the file, show the first one.
        let (frame, t) = self
            .best
            .clone()
            .or_else(|| self.ahead.clone())
            .ok_or(DecodeError::NoFrame)?;
        self.convert(&frame, t, reduce)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_is_an_error_not_a_crash() {
        assert!(matches!(
            probe(Path::new("this-file-does-not-exist.mp4")),
            Err(DecodeError::Open(_))
        ));
    }

    /// Makes a two-second test clip with the `ffmpeg` program; `None` when it is not installed.
    fn test_clip(name: &str) -> Option<std::path::PathBuf> {
        let path = std::env::temp_dir().join(name);
        let ok = std::process::Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-f", "lavfi", "-i"])
            .arg("testsrc=duration=2:size=320x180:rate=24")
            .args(["-pix_fmt", "yuv420p", "-g", "12"])
            .arg(&path)
            .status()
            .is_ok_and(|s| s.success());
        ok.then_some(path)
    }

    #[test]
    fn profile_names_read_like_words() {
        use ff::codec::profile::{Profile, H264, HEVC};
        assert_eq!(
            profile_name(Profile::H264(H264::ConstrainedBaseline)).as_deref(),
            Some("Constrained Baseline")
        );
        assert_eq!(
            profile_name(Profile::H264(H264::High10)).as_deref(),
            Some("High 10")
        );
        assert_eq!(
            profile_name(Profile::HEVC(HEVC::Main10)).as_deref(),
            Some("Main 10")
        );
        assert_eq!(profile_name(Profile::Unknown), None);
    }

    #[test]
    fn probes_and_decodes_a_clip() -> Result<(), String> {
        let Some(path) = test_clip("picasso_decode_test.mp4") else {
            return Ok(());
        };
        let info = probe(&path).map_err(|e| e.to_string())?;
        assert_eq!((info.width, info.height, info.fps), (320, 180, (24, 1)));
        let label = info.codec_label();
        assert!(label.starts_with("H.264 High L"), "{label}");
        let mut dec = VideoDecoder::open(&path).map_err(|e| e.to_string())?;
        let frame = dec.frame_at(1_000_000).map_err(|e| e.to_string())?;
        assert_eq!((frame.width, frame.rgba.len()), (320, 320 * 180 * 4));
        // 24 fps: one second is where frame 24 starts.
        assert_eq!(frame.pts_us / 1000, 1000);
        // A proxy at a quarter of the size, for the same moment.
        let small = dec
            .frame_at_reduced(1_000_000, 4)
            .map_err(|e| e.to_string())?;
        assert_eq!((small.width, small.height), (80, 45));
        assert_eq!(small.rgba.len(), 80 * 45 * 4);
        assert_eq!(small.pts_us, frame.pts_us);
        let _ = std::fs::remove_file(path);
        Ok(())
    }

    #[test]
    fn the_card_and_the_processor_give_the_same_pictures() -> Result<(), String> {
        let Some(path) = test_clip("picasso_decode_hw.mp4") else {
            return Ok(());
        };
        let mut card = VideoDecoder::open_with(&path, true).map_err(|e| e.to_string())?;
        let mut cpu = VideoDecoder::open_with(&path, false).map_err(|e| e.to_string())?;
        assert!(!cpu.on_card);
        for t in [0, 500_000, 1_000_000, 1_958_333] {
            let a = card.frame_at(t).map_err(|e| e.to_string())?;
            let b = cpu.frame_at(t).map_err(|e| e.to_string())?;
            assert_eq!((a.width, a.height, a.pts_us), (b.width, b.height, b.pts_us));
            // The card hands over NV12 rather than planar YUV, so colour edges round a little
            // differently (about one level in 255 on average); a wrong picture is far off.
            let total: u64 = a
                .rgba
                .iter()
                .zip(&b.rgba)
                .map(|(x, y)| u64::from(x.abs_diff(*y)))
                .sum();
            let mean = total as f64 / a.rgba.len() as f64;
            assert!(
                mean < 3.0,
                "{mean} levels apart at {t} (card: {})",
                card.on_card
            );
        }
        let _ = std::fs::remove_file(path);
        Ok(())
    }

    #[test]
    fn playing_past_the_end_and_looping_back_keeps_decoding() -> Result<(), String> {
        let Some(path) = test_clip("picasso_decode_loop.mp4") else {
            return Ok(());
        };
        let mut dec = VideoDecoder::open(&path).map_err(|e| e.to_string())?;
        // Play the last second forward, then on past the end, as reading ahead does.
        let mut last = 0;
        for k in 0..40 {
            let t = 1_000_000 + k * 41_667;
            let frame = dec.frame_at(t).map_err(|e| format!("at {t}: {e}"))?;
            last = frame.pts_us;
        }
        assert!(last > 1_900_000, "the last picture stays: {last}");
        // A loop goes back to the start.
        let first = dec.frame_at(0).map_err(|e| e.to_string())?;
        assert_eq!(first.pts_us, 0);
        let next = dec.frame_at(41_667).map_err(|e| e.to_string())?;
        assert!(next.pts_us > 0);
        let _ = std::fs::remove_file(path);
        Ok(())
    }
}

#[cfg(test)]
mod refuse_tests {
    use super::*;

    #[test]
    fn addresses_and_network_paths_are_refused_but_files_are_not() {
        for bad in [
            "http://example.com/a.mp4",
            "https://example.com/a.mp4",
            "ftp://h/a.mp4",
            "concat:/etc/passwd|/etc/hosts",
            "pipe:0",
            "data:video/mp4;base64,AAAA",
            "file:///etc/passwd",
            "subfile,,start,0,end,10,,:/etc/passwd",
            r"\\server\share\clip.mov",
            "//server/share/clip.mov",
            "  http://padded.example/a.mp4",
        ] {
            assert!(refuse_path(bad).is_some(), "{bad} should be refused");
        }
        for fine in [
            r"C:\Videos\clip.mp4",
            "C:/Videos/clip.mp4",
            "/home/me/clip.mp4",
            "clip.mp4",
            "relative/dir/clip.mov",
            "/Volumes/Media/shot 1.mov",
        ] {
            assert!(refuse_path(fine).is_none(), "{fine} should be allowed");
        }
    }

    #[test]
    fn opening_a_refused_path_is_an_error_not_a_network_request() {
        let err = probe(Path::new("http://127.0.0.1:9/a.mp4")).unwrap_err();
        assert!(matches!(err, DecodeError::Refused(_)), "{err}");
    }
}
