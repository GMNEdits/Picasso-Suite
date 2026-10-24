//! Numbered image sequence detection (`shot_0001.exr`, `frame.000123.dpx`, ...).

use std::path::{Path, PathBuf};

const SEQUENCE_EXTENSIONS: &[&str] = &[
    "exr", "dpx", "cin", "tif", "tiff", "png", "jpg", "jpeg", "bmp", "tga", "webp", "jxl", "j2k", "jp2",
    "sgi", "pgm", "ppm", "hdr",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectedSequence {
    /// FFmpeg printf pattern, e.g. `/shots/a_%04d.exr`.
    pub pattern: PathBuf,
    pub start_number: u64,
    pub frame_count: u64,
}

/// If `path` is one frame of a numbered sequence with at least two
/// consecutive frames on disk, returns the contiguous run containing it.
pub fn detect(path: &Path) -> Option<DetectedSequence> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if !SEQUENCE_EXTENSIONS.contains(&ext.as_str()) {
        return None;
    }
    let file = path.file_name()?.to_str()?;
    let stem_len = file.len() - ext.len() - 1;
    let stem = &file[..stem_len];
    let digits_start = stem.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    let digits = &stem[digits_start..];
    if digits.is_empty() {
        return None;
    }
    let prefix = &stem[..digits_start];
    let suffix = &file[stem_len..];
    let width = digits.len();
    let number: u64 = digits.parse().ok()?;
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));

    let frame_path = |n: u64| dir.join(format!("{prefix}{n:0width$}{suffix}"));
    let mut first = number;
    while first > 0 && frame_path(first - 1).is_file() {
        first -= 1;
    }
    let mut last = number;
    while frame_path(last + 1).is_file() {
        last += 1;
    }
    if last == first {
        return None;
    }
    // FFmpeg's printf patterns treat a literal '%' specially.
    let escaped_prefix = prefix.replace('%', "%%");
    let escaped_suffix = suffix.replace('%', "%%");
    Some(DetectedSequence {
        pattern: dir.join(format!("{escaped_prefix}%0{width}d{escaped_suffix}")),
        start_number: first,
        frame_count: last - first + 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_contiguous_run() {
        let dir = tempfile::tempdir().unwrap();
        for n in [9, 10, 11, 12, 20] {
            std::fs::write(dir.path().join(format!("shot_{n:04}.png")), b"").unwrap();
        }
        let seq = detect(&dir.path().join("shot_0010.png")).unwrap();
        assert_eq!(seq.start_number, 9);
        assert_eq!(seq.frame_count, 4);
        assert_eq!(seq.pattern, dir.path().join("shot_%04d.png"));
        assert_eq!(detect(&dir.path().join("shot_0020.png")), None);
    }

    #[test]
    fn ignores_video_files() {
        assert_eq!(detect(Path::new("take_0001.mov")), None);
    }
}
