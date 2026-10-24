//! Export presets. Only presets whose encoders exist in the installed FFmpeg
//! build are offered. H.264 and H.265 come first, including hardware encoders
//! (NVENC, Intel QSV, AMD AMF, VAAPI on Linux, VideoToolbox on macOS).

use crate::tools::{Tool, command};
use crate::{MediaError, Result};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    Best,
    High,
    Medium,
    Small,
}

impl Quality {
    pub const ALL: [Quality; 4] = [Quality::Best, Quality::High, Quality::Medium, Quality::Small];

    pub fn label(self) -> &'static str {
        match self {
            Quality::Best => "Best",
            Quality::High => "High",
            Quality::Medium => "Medium",
            Quality::Small => "Small file",
        }
    }

    fn step(self) -> i32 {
        match self {
            Quality::Best => 0,
            Quality::High => 1,
            Quality::Medium => 2,
            Quality::Small => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hardware {
    None,
    /// Linux VA-API: frames are uploaded to a render node before encoding.
    Vaapi,
}

#[derive(Clone, Debug)]
pub struct VideoCodec {
    pub encoder: &'static str,
    pub pix_fmt: &'static str,
    pub hardware: Hardware,
    /// Encoder options that don't depend on quality.
    pub args: &'static [&'static str],
    /// Rate-control option and its value at Best quality; each quality step adds `rc_step`.
    pub rc: Option<(&'static str, i32, i32)>,
}

#[derive(Clone, Debug)]
pub struct AudioCodec {
    pub encoder: &'static str,
    pub args: &'static [&'static str],
}

#[derive(Clone, Debug)]
pub struct ExportPreset {
    pub id: &'static str,
    pub label: &'static str,
    /// Allowed containers; the first is the default.
    pub extensions: &'static [&'static str],
    pub video: Option<VideoCodec>,
    pub audio: Option<AudioCodec>,
    /// Writes numbered image files instead of a movie.
    pub image_sequence: bool,
}

impl ExportPreset {
    pub fn default_extension(&self) -> &'static str {
        self.extensions[0]
    }

    /// Encoders this preset needs.
    pub fn encoders(&self) -> impl Iterator<Item = &'static str> {
        self.video.iter().map(|v| v.encoder).chain(self.audio.iter().map(|a| a.encoder))
    }

    /// Output arguments for the video stream.
    pub fn video_args(&self, quality: Quality, extension: &str) -> Vec<String> {
        let Some(v) = &self.video else { return vec!["-vn".into()] };
        let mut args = vec!["-c:v".into(), v.encoder.into()];
        args.extend(v.args.iter().map(|s| s.to_string()));
        if let Some((opt, best, step)) = v.rc {
            args.extend([opt.to_string(), (best + step * quality.step()).to_string()]);
        }
        if v.hardware == Hardware::None {
            args.extend(["-pix_fmt".into(), v.pix_fmt.into()]);
        }
        if matches!(extension, "mp4" | "mov" | "m4v") && !self.image_sequence {
            args.extend(["-movflags".into(), "+faststart".into()]);
        }
        // Tag H.265 so Apple players accept it.
        if matches!(v.encoder, "libx265" | "hevc_nvenc" | "hevc_qsv" | "hevc_amf" | "hevc_vaapi" | "hevc_videotoolbox")
            && matches!(extension, "mp4" | "mov" | "m4v")
        {
            args.extend(["-tag:v".into(), "hvc1".into()]);
        }
        args
    }

    pub fn audio_args(&self) -> Vec<String> {
        let Some(a) = &self.audio else { return vec!["-an".into()] };
        let mut args = vec!["-c:a".into(), a.encoder.into()];
        args.extend(a.args.iter().map(|s| s.to_string()));
        args
    }
}

const AAC: AudioCodec = AudioCodec { encoder: "aac", args: &["-b:a", "320k"] };
const PCM24: AudioCodec = AudioCodec { encoder: "pcm_s24le", args: &[] };
const OPUS: AudioCodec = AudioCodec { encoder: "libopus", args: &["-b:a", "256k"] };
const H26X: &[&str] = &["mp4", "mov", "mkv"];

const fn sw(encoder: &'static str, pix_fmt: &'static str, args: &'static [&'static str], rc: Option<(&'static str, i32, i32)>) -> VideoCodec {
    VideoCodec { encoder, pix_fmt, hardware: Hardware::None, args, rc }
}

pub static PRESETS: &[ExportPreset] = &[
    // ---- H.264 ----
    ExportPreset { id: "h264", label: "H.264 (x264)", extensions: H26X, image_sequence: false, audio: Some(AAC),
        video: Some(sw("libx264", "yuv420p", &["-preset", "medium", "-profile:v", "high"], Some(("-crf", 16, 3)))) },
    ExportPreset { id: "h264_10bit", label: "H.264 10-bit (x264 High 10)", extensions: H26X, image_sequence: false, audio: Some(AAC),
        video: Some(sw("libx264", "yuv420p10le", &["-preset", "medium", "-profile:v", "high10"], Some(("-crf", 16, 3)))) },
    ExportPreset { id: "h264_nvenc", label: "H.264 NVIDIA NVENC", extensions: H26X, image_sequence: false, audio: Some(AAC),
        video: Some(sw("h264_nvenc", "yuv420p", &["-preset", "p5", "-tune", "hq", "-rc", "vbr", "-b:v", "0"], Some(("-cq", 17, 4)))) },
    ExportPreset { id: "h264_qsv", label: "H.264 Intel Quick Sync", extensions: H26X, image_sequence: false, audio: Some(AAC),
        video: Some(sw("h264_qsv", "nv12", &["-preset", "medium"], Some(("-global_quality", 18, 4)))) },
    ExportPreset { id: "h264_amf", label: "H.264 AMD AMF", extensions: H26X, image_sequence: false, audio: Some(AAC),
        video: Some(sw("h264_amf", "nv12", &["-quality", "quality", "-rc", "cqp"], Some(("-qp_i", 18, 4)))) },
    ExportPreset { id: "h264_vaapi", label: "H.264 VA-API (Linux)", extensions: H26X, image_sequence: false, audio: Some(AAC),
        video: Some(VideoCodec { encoder: "h264_vaapi", pix_fmt: "nv12", hardware: Hardware::Vaapi, args: &[], rc: Some(("-qp", 18, 4)) }) },
    ExportPreset { id: "h264_videotoolbox", label: "H.264 VideoToolbox (macOS)", extensions: H26X, image_sequence: false, audio: Some(AAC),
        video: Some(sw("h264_videotoolbox", "yuv420p", &[], Some(("-q:v", 80, -10)))) },
    // ---- H.265 / HEVC ----
    ExportPreset { id: "h265", label: "H.265 (x265)", extensions: H26X, image_sequence: false, audio: Some(AAC),
        video: Some(sw("libx265", "yuv420p", &["-preset", "medium"], Some(("-crf", 18, 3)))) },
    ExportPreset { id: "h265_10bit", label: "H.265 10-bit Main10 (x265)", extensions: H26X, image_sequence: false, audio: Some(AAC),
        video: Some(sw("libx265", "yuv420p10le", &["-preset", "medium", "-profile:v", "main10"], Some(("-crf", 18, 3)))) },
    ExportPreset { id: "h265_nvenc", label: "H.265 NVIDIA NVENC", extensions: H26X, image_sequence: false, audio: Some(AAC),
        video: Some(sw("hevc_nvenc", "yuv420p", &["-preset", "p5", "-tune", "hq", "-rc", "vbr", "-b:v", "0"], Some(("-cq", 19, 4)))) },
    ExportPreset { id: "h265_nvenc_10bit", label: "H.265 10-bit NVIDIA NVENC", extensions: H26X, image_sequence: false, audio: Some(AAC),
        video: Some(sw("hevc_nvenc", "p010le", &["-preset", "p5", "-tune", "hq", "-rc", "vbr", "-b:v", "0", "-profile:v", "main10"], Some(("-cq", 19, 4)))) },
    ExportPreset { id: "h265_qsv", label: "H.265 Intel Quick Sync", extensions: H26X, image_sequence: false, audio: Some(AAC),
        video: Some(sw("hevc_qsv", "nv12", &["-preset", "medium"], Some(("-global_quality", 20, 4)))) },
    ExportPreset { id: "h265_amf", label: "H.265 AMD AMF", extensions: H26X, image_sequence: false, audio: Some(AAC),
        video: Some(sw("hevc_amf", "nv12", &["-quality", "quality", "-rc", "cqp"], Some(("-qp_i", 20, 4)))) },
    ExportPreset { id: "h265_vaapi", label: "H.265 VA-API (Linux)", extensions: H26X, image_sequence: false, audio: Some(AAC),
        video: Some(VideoCodec { encoder: "hevc_vaapi", pix_fmt: "nv12", hardware: Hardware::Vaapi, args: &[], rc: Some(("-qp", 20, 4)) }) },
    ExportPreset { id: "h265_videotoolbox", label: "H.265 VideoToolbox (macOS)", extensions: H26X, image_sequence: false, audio: Some(AAC),
        video: Some(sw("hevc_videotoolbox", "yuv420p", &[], Some(("-q:v", 80, -10)))) },
    // ---- Mastering / other ----
    ExportPreset { id: "prores_hq", label: "Apple ProRes 422 HQ", extensions: &["mov"], image_sequence: false, audio: Some(PCM24),
        video: Some(sw("prores_ks", "yuv422p10le", &["-profile:v", "3", "-vendor", "apl0"], None)) },
    ExportPreset { id: "prores_4444", label: "Apple ProRes 4444 (alpha)", extensions: &["mov"], image_sequence: false, audio: Some(PCM24),
        video: Some(sw("prores_ks", "yuva444p10le", &["-profile:v", "4", "-vendor", "apl0"], None)) },
    ExportPreset { id: "dnxhr_hq", label: "Avid DNxHR HQ", extensions: &["mov", "mxf"], image_sequence: false, audio: Some(PCM24),
        video: Some(sw("dnxhd", "yuv422p", &["-profile:v", "dnxhr_hq"], None)) },
    ExportPreset { id: "av1", label: "AV1 (SVT-AV1)", extensions: &["mkv", "mp4", "webm"], image_sequence: false, audio: Some(OPUS),
        video: Some(sw("libsvtav1", "yuv420p10le", &["-preset", "6"], Some(("-crf", 24, 6)))) },
    ExportPreset { id: "vp9", label: "VP9 WebM", extensions: &["webm", "mkv"], image_sequence: false, audio: Some(OPUS),
        video: Some(sw("libvpx-vp9", "yuv420p", &["-b:v", "0", "-row-mt", "1"], Some(("-crf", 24, 6)))) },
    ExportPreset { id: "ffv1", label: "FFV1 lossless", extensions: &["mkv"], image_sequence: false, audio: Some(AudioCodec { encoder: "flac", args: &[] }),
        video: Some(sw("ffv1", "yuv444p10le", &["-level", "3", "-g", "1"], None)) },
    ExportPreset { id: "png_seq", label: "PNG image sequence", extensions: &["png"], image_sequence: true, audio: None,
        video: Some(sw("png", "rgba", &[], None)) },
    ExportPreset { id: "wav", label: "Audio only — WAV 24-bit", extensions: &["wav"], image_sequence: false, audio: Some(PCM24), video: None },
    ExportPreset { id: "mp3", label: "Audio only — MP3", extensions: &["mp3"], image_sequence: false,
        audio: Some(AudioCodec { encoder: "libmp3lame", args: &["-b:a", "320k"] }), video: None },
];

pub fn preset(id: &str) -> Option<&'static ExportPreset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// Encoder names compiled into the installed FFmpeg.
pub fn available_encoders() -> Result<HashSet<String>> {
    let out = command(Tool::Ffmpeg)
        .args(["-hide_banner", "-encoders"])
        .output()
        .map_err(|_| MediaError::ToolMissing("ffmpeg"))?;
    Ok(parse_encoders(&String::from_utf8_lossy(&out.stdout)))
}

fn parse_encoders(text: &str) -> HashSet<String> {
    text.lines()
        .skip_while(|l| !l.trim_start().starts_with("------"))
        .skip(1)
        .filter_map(|l| l.split_whitespace().nth(1).map(str::to_owned))
        .collect()
}

/// Presets usable with the installed FFmpeg. Hardware presets are listed when
/// the encoder is compiled in; whether a device is present is only known when
/// an export runs.
pub fn available_presets(encoders: &HashSet<String>) -> Vec<&'static ExportPreset> {
    PRESETS.iter().filter(|p| p.encoders().all(|e| encoders.contains(e))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_encoder_list() {
        let text = "Encoders:\n V..... = Video\n ------\n V....D libx264   libx264 H.264\n A....D aac   AAC\n";
        let set = parse_encoders(text);
        assert!(set.contains("libx264") && set.contains("aac"));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn h264_and_h265_lead_and_build_args() {
        assert_eq!(PRESETS[0].id, "h264");
        let x265 = preset("h265_10bit").unwrap();
        let args = x265.video_args(Quality::High, "mp4");
        let joined = args.join(" ");
        assert!(joined.contains("-c:v libx265"));
        assert!(joined.contains("-crf 21"));
        assert!(joined.contains("-pix_fmt yuv420p10le"));
        assert!(joined.contains("-tag:v hvc1"));
        assert!(!preset("h265").unwrap().video_args(Quality::Best, "mkv").contains(&"-movflags".to_string()));
    }

    #[test]
    fn filters_by_available_encoders() {
        let enc: HashSet<String> = ["libx264", "aac"].into_iter().map(String::from).collect();
        let ids: Vec<_> = available_presets(&enc).iter().map(|p| p.id).collect();
        assert_eq!(ids, vec!["h264", "h264_10bit"]);
    }
}
