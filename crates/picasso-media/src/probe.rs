//! Reading media metadata with ffprobe.

use crate::sequence;
use crate::tools::{Tool, command};
use crate::{MediaError, Result};
use picasso_core::{AudioInfo, ImageSequence, MediaInfo, MediaItem, Rational, VideoInfo, new_id};
use serde::Deserialize;
use std::ffi::OsString;
use std::path::Path;

/// FFmpeg input arguments (`... -i <input>`) for a media item.
pub fn input_args(media: &MediaItem) -> Vec<OsString> {
    let mut args: Vec<OsString> = Vec::new();
    if let Some(seq) = &media.sequence {
        args.extend(["-framerate".into(), format!("{}/{}", seq.fps.num, seq.fps.den).into()]);
        args.extend(["-start_number".into(), seq.start_number.to_string().into()]);
    }
    args.push("-i".into());
    args.push(media.path.clone().into_os_string());
    args
}

/// Probes a file. Numbered image files are imported as a sequence at
/// `sequence_fps` when sibling frames exist.
pub fn probe(path: &Path, sequence_fps: Rational) -> Result<MediaItem> {
    // Projects must not depend on the working directory they were built in.
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let path = absolute.as_path();
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if let Some(seq) = sequence::detect(path) {
        let mut item = MediaItem {
            id: new_id(),
            name: sequence_name(&name),
            path: seq.pattern,
            sequence: Some(ImageSequence {
                start_number: seq.start_number,
                frame_count: seq.frame_count,
                fps: sequence_fps,
            }),
            info: MediaInfo::default(),
        };
        let mut info = probe_input(&input_args(&item))?;
        if let Some(v) = info.video.as_mut() {
            v.is_still = false;
            v.fps = sequence_fps;
        }
        info.duration_secs = seq.frame_count as f64 / sequence_fps.as_f64();
        item.info = info;
        return Ok(item);
    }
    let mut item = MediaItem { id: new_id(), name, path: path.to_path_buf(), sequence: None, info: MediaInfo::default() };
    item.info = probe_input(&input_args(&item))?;
    Ok(item)
}

fn sequence_name(file: &str) -> String {
    let (stem, ext) = file.rsplit_once('.').unwrap_or((file, ""));
    let base = stem.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end_matches(['_', '.', '-']);
    format!("{base} [seq].{ext}")
}

#[derive(Deserialize)]
struct ProbeOutput {
    #[serde(default)]
    streams: Vec<ProbeStream>,
    format: Option<ProbeFormat>,
}

#[derive(Deserialize)]
struct ProbeFormat {
    format_name: Option<String>,
    duration: Option<String>,
}

#[derive(Deserialize, Default)]
struct ProbeStream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    pix_fmt: Option<String>,
    r_frame_rate: Option<String>,
    avg_frame_rate: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u32>,
    duration: Option<String>,
    disposition: Option<Disposition>,
}

#[derive(Deserialize, Default)]
struct Disposition {
    #[serde(default)]
    attached_pic: u8,
}

fn probe_input(input: &[OsString]) -> Result<MediaInfo> {
    let out = command(Tool::Ffprobe)
        .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"])
        .args(input.iter().filter(|a| *a != "-i"))
        .output()
        .map_err(|_| MediaError::ToolMissing("ffprobe"))?;
    let label = input.last().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        return Err(MediaError::Probe(if err.is_empty() { label } else { err }));
    }
    let parsed: ProbeOutput =
        serde_json::from_slice(&out.stdout).map_err(|e| MediaError::Probe(format!("{label}: {e}")))?;
    parse_probe(parsed).ok_or(MediaError::NoStreams(label))
}

fn parse_probe(p: ProbeOutput) -> Option<MediaInfo> {
    let format_name = p.format.as_ref().and_then(|f| f.format_name.clone()).unwrap_or_default();
    let still_format = format_name == "image2" || format_name.ends_with("_pipe");
    let video = p
        .streams
        .iter()
        .filter(|s| s.codec_type.as_deref() == Some("video"))
        .find(|s| s.disposition.as_ref().is_none_or(|d| d.attached_pic == 0))
        .map(|s| {
            let pix_fmt = s.pix_fmt.clone().unwrap_or_default();
            let fps = [&s.avg_frame_rate, &s.r_frame_rate]
                .into_iter()
                .flatten()
                .find_map(|r| Rational::parse(r))
                .unwrap_or(Rational::new(25, 1));
            VideoInfo {
                codec: s.codec_name.clone().unwrap_or_default(),
                width: s.width.unwrap_or(0),
                height: s.height.unwrap_or(0),
                fps,
                has_alpha: pix_fmt_has_alpha(&pix_fmt),
                pix_fmt,
                is_still: still_format,
            }
        })
        .filter(|v| v.width > 0 && v.height > 0);
    let audio = p.streams.iter().find(|s| s.codec_type.as_deref() == Some("audio")).map(|s| AudioInfo {
        codec: s.codec_name.clone().unwrap_or_default(),
        sample_rate: s.sample_rate.as_deref().and_then(|r| r.parse().ok()).unwrap_or(48_000),
        channels: s.channels.unwrap_or(2),
    });
    if video.is_none() && audio.is_none() {
        return None;
    }
    let duration_secs = p
        .format
        .as_ref()
        .and_then(|f| f.duration.as_deref())
        .or_else(|| p.streams.iter().find_map(|s| s.duration.as_deref()))
        .and_then(|d| d.parse::<f64>().ok())
        .filter(|d| d.is_finite() && *d > 0.0)
        .unwrap_or(0.0);
    Some(MediaInfo { format_name, duration_secs, video, audio })
}

pub fn pix_fmt_has_alpha(pix_fmt: &str) -> bool {
    const ALPHA_PREFIXES: &[&str] = &["yuva", "rgba", "bgra", "argb", "abgr", "gbrap", "ya", "rgba64", "bgra64"];
    ALPHA_PREFIXES.iter().any(|p| pix_fmt.starts_with(p)) || pix_fmt == "pal8"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Option<MediaInfo> {
        parse_probe(serde_json::from_str(json).unwrap())
    }

    #[test]
    fn parses_h265_with_audio_and_skips_cover_art() {
        let info = parse(
            r#"{"streams":[
                {"codec_type":"video","codec_name":"mjpeg","width":600,"height":600,"disposition":{"attached_pic":1}},
                {"codec_type":"video","codec_name":"hevc","width":3840,"height":2160,"pix_fmt":"yuv420p10le",
                 "avg_frame_rate":"30000/1001","r_frame_rate":"30000/1001"},
                {"codec_type":"audio","codec_name":"aac","sample_rate":"48000","channels":2}],
              "format":{"format_name":"mov,mp4,m4a,3gp,3g2,mj2","duration":"12.512"}}"#,
        )
        .unwrap();
        let v = info.video.unwrap();
        assert_eq!((v.codec.as_str(), v.width, v.fps), ("hevc", 3840, Rational::new(30000, 1001)));
        assert!(!v.is_still && !v.has_alpha);
        assert_eq!(info.audio.unwrap().channels, 2);
        assert!((info.duration_secs - 12.512).abs() < 1e-9);
    }

    #[test]
    fn png_is_still_with_alpha() {
        let info = parse(
            r#"{"streams":[{"codec_type":"video","codec_name":"png","width":64,"height":32,"pix_fmt":"rgba",
                 "avg_frame_rate":"0/0","r_frame_rate":"25/1"}],"format":{"format_name":"png_pipe"}}"#,
        )
        .unwrap();
        assert!(info.is_still());
        assert!(info.video.unwrap().has_alpha);
    }

    #[test]
    fn audio_only() {
        let info = parse(
            r#"{"streams":[{"codec_type":"audio","codec_name":"flac","sample_rate":"96000","channels":6}],
              "format":{"format_name":"flac","duration":"3.0"}}"#,
        )
        .unwrap();
        assert!(info.video.is_none());
        assert_eq!(info.audio.unwrap().sample_rate, 96000);
    }

    #[test]
    fn sequence_names() {
        assert_eq!(sequence_name("shot_0012.exr"), "shot [seq].exr");
    }
}
