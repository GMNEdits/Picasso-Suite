//! Small previews for the interface: poster-frame thumbnails and audio
//! waveform envelopes.

use crate::decode::VideoDecoder;
use crate::frame::RgbaImage;
use crate::probe::input_args;
use crate::tools::{Tool, command};
use crate::{MediaError, Result};
use picasso_core::MediaItem;
use std::process::Stdio;

/// A thumbnail that fits in `max_w`×`max_h`, taken at `seconds` into the media.
pub fn thumbnail(media: &MediaItem, seconds: f64, max_w: u32, max_h: u32) -> Result<Option<RgbaImage>> {
    let Some(v) = &media.info.video else { return Ok(None) };
    let scale = (max_w as f64 / v.width.max(1) as f64).min(max_h as f64 / v.height.max(1) as f64);
    let w = ((v.width as f64 * scale).round() as u32).max(2) & !1;
    let h = ((v.height as f64 * scale).round() as u32).max(2) & !1;
    let mut dec = VideoDecoder::new(media, w, h)?;
    Ok(dec.frame_at(seconds)?.map(|img| (*img).clone()))
}

/// Peak amplitude (0..1) of the media's audio, `per_second` values per second, mixed to mono.
pub fn waveform(media: &MediaItem, per_second: u32) -> Result<Vec<f32>> {
    if media.info.audio.is_none() {
        return Ok(Vec::new());
    }
    // Decoding at a low rate is enough for a drawn envelope and keeps this fast.
    let rate = 4000u32;
    let out = command(Tool::Ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-nostdin"])
        .args(input_args(media))
        .args(["-vn", "-sn", "-ac", "1", "-ar", &rate.to_string(), "-f", "f32le", "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|_| MediaError::ToolMissing("ffmpeg"))?;
    if !out.status.success() {
        return Err(MediaError::Ffmpeg(String::from_utf8_lossy(&out.stderr).trim().to_owned()));
    }
    Ok(peaks(&out.stdout, (rate / per_second.max(1)) as usize))
}

fn peaks(bytes: &[u8], bucket: usize) -> Vec<f32> {
    let samples: Vec<f32> = bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    samples
        .chunks(bucket.max(1))
        .map(|c| c.iter().fold(0f32, |m, s| m.max(s.abs())).min(1.0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peaks_per_bucket() {
        let samples = [0.1f32, -0.8, 0.2, 0.3, -0.05, 0.0];
        let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        assert_eq!(peaks(&bytes, 2), vec![0.8, 0.3, 0.05]);
    }
}
