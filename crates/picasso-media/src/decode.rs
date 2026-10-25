//! Frame-accurate video decoding through an `ffmpeg` child process that
//! streams raw RGBA frames at a fixed size.
//!
//! Sequential reads (playback, export) reuse the running process; jumps
//! backwards or far ahead restart it with an input seek.

use crate::frame::RgbaImage;
use crate::probe::input_args;
use crate::tools::{Tool, command};
use crate::{MediaError, Result};
use picasso_core::time::seconds_to_frames;
use picasso_core::{MediaItem, Rational};
use std::ffi::OsString;
use std::io::{BufReader, Read};
use std::process::{Child, ChildStdout, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// How far ahead (in seconds) we decode-and-discard instead of re-seeking.
const MAX_SKIP_SECONDS: f64 = 2.0;
/// Codecs that benefit from hardware decoding when available.
const HWACCEL_CODECS: &[&str] = &["h264", "hevc", "av1", "vp9", "mpeg2video", "vc1"];

/// Process-wide hardware-decoding switch, set from the app's Preferences.
///
/// Decoding hardware belongs to the machine, not to a project, so this is a
/// global setting (as in DaVinci Resolve's Preferences ▸ System ▸ Decode Options)
/// rather than a project setting. `PICASSO_NO_HWACCEL` still wins for one run.
static HARDWARE_DECODING: AtomicBool = AtomicBool::new(true);

pub fn set_hardware_decoding(enabled: bool) {
    HARDWARE_DECODING.store(enabled, Ordering::Relaxed);
}

pub fn hardware_decoding() -> bool {
    HARDWARE_DECODING.load(Ordering::Relaxed) && std::env::var_os("PICASSO_NO_HWACCEL").is_none()
}

/// Whether `codec` should be decoded by the GPU, given the current switch.
fn uses_hwaccel(codec: &str, hardware: bool) -> bool {
    hardware && HWACCEL_CODECS.contains(&codec)
}

pub struct VideoDecoder {
    input: Vec<OsString>,
    fps: Rational,
    width: u32,
    height: u32,
    still: bool,
    hwaccel: bool,
    proc: Option<Running>,
    /// Source frame index the next read will return.
    next_index: i64,
    last: Option<(i64, Arc<RgbaImage>)>,
}

struct Running {
    child: Child,
    stdout: BufReader<ChildStdout>,
    stderr: Arc<Mutex<String>>,
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl VideoDecoder {
    /// Decoder producing `width`×`height` RGBA frames (scaled by FFmpeg).
    pub fn new(media: &MediaItem, width: u32, height: u32) -> Result<Self> {
        let video = media.info.video.as_ref().ok_or_else(|| MediaError::NoStreams(media.name.clone()))?;
        let hwaccel = uses_hwaccel(&video.codec, hardware_decoding());
        Ok(Self {
            input: input_args(media),
            fps: video.fps,
            width: width.max(2),
            height: height.max(2),
            still: video.is_still && media.sequence.is_none(),
            hwaccel,
            proc: None,
            next_index: 0,
            last: None,
        })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The frame displayed at `seconds` into the media. Past the end of the
    /// stream the last decoded frame is held.
    pub fn frame_at(&mut self, seconds: f64) -> Result<Option<Arc<RgbaImage>>> {
        let index = if self.still { 0 } else { seconds_to_frames(seconds.max(0.0), self.fps) };
        if let Some((i, img)) = &self.last
            && *i == index {
                return Ok(Some(img.clone()));
            }
        if self.still && self.last.is_some() {
            return Ok(self.last.as_ref().map(|(_, i)| i.clone()));
        }
        let max_skip = (MAX_SKIP_SECONDS * self.fps.as_f64()) as i64;
        if self.proc.is_none() || index < self.next_index || index - self.next_index > max_skip {
            self.start(index)?;
        }
        while self.next_index <= index {
            match self.read_frame()? {
                Some(img) => {
                    let img = Arc::new(img);
                    self.last = Some((self.next_index, img.clone()));
                    self.next_index += 1;
                }
                None => {
                    // End of stream: hold the last frame.
                    self.proc = None;
                    return Ok(self.last.as_ref().map(|(_, i)| i.clone()));
                }
            }
        }
        Ok(self.last.as_ref().map(|(_, i)| i.clone()))
    }

    fn start(&mut self, index: i64) -> Result<()> {
        self.proc = None;
        let mut cmd = command(Tool::Ffmpeg);
        cmd.args(["-hide_banner", "-loglevel", "error", "-nostdin"]);
        if self.hwaccel {
            // Falls back to software decoding when no device is available.
            cmd.args(["-hwaccel", "auto"]);
        }
        if index > 0 {
            let secs = index as f64 * self.fps.den as f64 / self.fps.num as f64;
            cmd.args(["-ss", &format!("{secs:.6}")]);
        }
        cmd.args(&self.input);
        let fps = format!("fps={}/{},", self.fps.num, self.fps.den);
        let filter = format!(
            "{}scale={}:{}:flags=bicubic,format=rgba",
            if self.still { "" } else { fps.as_str() },
            self.width,
            self.height
        );
        cmd.args(["-an", "-sn", "-dn", "-vf", &filter]);
        if self.still {
            cmd.args(["-frames:v", "1"]);
        }
        cmd.args(["-f", "rawvideo", "-pix_fmt", "rgba", "-"]);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|_| MediaError::ToolMissing("ffmpeg"))?;
        let stdout = BufReader::with_capacity(RgbaImage::byte_len(self.width, self.height), child.stdout.take().unwrap());
        let stderr = Arc::new(Mutex::new(String::new()));
        let mut err_pipe = child.stderr.take().unwrap();
        let sink = stderr.clone();
        std::thread::spawn(move || {
            let mut buf = String::new();
            let _ = err_pipe.read_to_string(&mut buf);
            *sink.lock().unwrap() = buf;
        });
        self.proc = Some(Running { child, stdout, stderr });
        self.next_index = index;
        Ok(())
    }

    fn read_frame(&mut self) -> Result<Option<RgbaImage>> {
        let len = RgbaImage::byte_len(self.width, self.height);
        let proc = self.proc.as_mut().expect("started");
        let mut data = vec![0u8; len];
        let mut filled = 0;
        while filled < len {
            match proc.stdout.read(&mut data[filled..])? {
                0 => break,
                n => filled += n,
            }
        }
        if filled == len {
            return Ok(Some(RgbaImage { width: self.width, height: self.height, data }));
        }
        // Stream ended. Report an error only if nothing was ever decoded.
        let _ = proc.child.wait();
        if self.last.is_none() {
            std::thread::sleep(std::time::Duration::from_millis(20));
            let msg = proc.stderr.lock().unwrap().trim().to_owned();
            if !msg.is_empty() {
                return Err(MediaError::Ffmpeg(msg));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_supported_codecs_use_the_gpu() {
        assert!(uses_hwaccel("h264", true));
        assert!(uses_hwaccel("hevc", true));
        assert!(uses_hwaccel("av1", true));
        assert!(!uses_hwaccel("prores", true));
        assert!(!uses_hwaccel("h264", false));
    }
}
