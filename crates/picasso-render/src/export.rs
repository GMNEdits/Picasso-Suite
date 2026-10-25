//! Rendering a timeline to a file: frames from the compositor are piped into
//! an `ffmpeg` encoder as raw RGBA, and audio clips are mixed by an FFmpeg
//! filter graph built from the timeline.

use crate::compositor::render_frame;
use crate::source::DecoderPool;
use picasso_core::time::frames_to_seconds;
use picasso_core::{MediaItem, Project, TrackKind};
use picasso_media::encoders::{ExportPreset, Hardware, Quality};
use picasso_media::probe::input_args;
use picasso_media::tools::{Tool, command};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error(transparent)]
    Media(#[from] picasso_media::MediaError),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("encoding failed: {0}")]
    Ffmpeg(String),
    #[error("export cancelled")]
    Cancelled,
    #[error("the timeline is empty")]
    Empty,
}

pub struct ExportSettings {
    pub preset: &'static ExportPreset,
    pub quality: Quality,
    pub output: PathBuf,
    /// Timeline frame range `[start, end)`; `None` exports the whole timeline.
    pub range: Option<(i64, i64)>,
}

#[derive(Default)]
pub struct ExportProgress {
    pub done: AtomicU64,
    pub total: AtomicU64,
    pub cancel: AtomicBool,
}

impl ExportProgress {
    pub fn fraction(&self) -> f32 {
        let total = self.total.load(Ordering::Relaxed);
        if total == 0 { 0.0 } else { self.done.load(Ordering::Relaxed) as f32 / total as f32 }
    }
}

/// Output path FFmpeg writes to; image sequences get a frame-number pattern.
pub fn output_target(settings: &ExportSettings) -> PathBuf {
    if !settings.preset.image_sequence {
        return settings.output.clone();
    }
    let stem = settings.output.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or("frame".into());
    let ext = settings.preset.default_extension();
    settings.output.with_file_name(format!("{}_%06d.{ext}", stem.replace('%', "%%")))
}

struct AudioPart<'a> {
    media: &'a MediaItem,
    source_start: f64,
    duration: f64,
    delay: f64,
    volume: f32,
}

fn audio_parts(project: &Project, start: i64, end: i64) -> Vec<AudioPart<'_>> {
    let fps = project.settings.fps;
    let mut parts = Vec::new();
    for track in project.timeline.tracks.iter().filter(|t| t.kind == TrackKind::Audio && t.enabled) {
        for clip in track.clips.iter().filter(|c| c.enabled && c.end() > start && c.start < end) {
            let Some(media) = project.media(clip.media_id).filter(|m| m.info.audio.is_some()) else { continue };
            let from = clip.start.max(start);
            let to = clip.end().min(end);
            parts.push(AudioPart {
                media,
                source_start: project.source_seconds(clip, from),
                duration: frames_to_seconds(to - from, fps),
                delay: frames_to_seconds(from - start, fps),
                volume: clip.volume,
            });
        }
    }
    parts
}

/// Builds the audio inputs and the `filter_complex` graph producing `[aout]`.
fn audio_graph(project: &Project, start: i64, end: i64, first_input: usize) -> (Vec<OsString>, String) {
    let total = frames_to_seconds(end - start, project.settings.fps);
    let rate = project.settings.sample_rate;
    let parts = audio_parts(project, start, end);
    let mut inputs = Vec::new();
    let mut graph = String::new();
    for (i, p) in parts.iter().enumerate() {
        inputs.extend(["-ss".into(), format!("{:.6}", p.source_start).into()]);
        inputs.extend(["-t".into(), format!("{:.6}", p.duration + 0.5).into()]);
        inputs.extend(input_args(p.media));
        graph.push_str(&format!(
            "[{}:a:0]atrim=duration={:.6},asetpts=PTS-STARTPTS,aresample={rate},\
             aformat=sample_fmts=fltp:channel_layouts=stereo,volume={:.4},adelay=delays={}:all=1[a{i}];",
            first_input + i,
            p.duration,
            p.volume,
            (p.delay * 1000.0).round() as i64,
        ));
    }
    if parts.is_empty() {
        graph.push_str(&format!("anullsrc=r={rate}:cl=stereo,atrim=end={total:.6}[aout]"));
    } else {
        for i in 0..parts.len() {
            graph.push_str(&format!("[a{i}]"));
        }
        graph.push_str(&format!(
            "amix=inputs={}:duration=longest:normalize=0,apad,atrim=end={total:.6}[aout]",
            parts.len()
        ));
    }
    (inputs, graph)
}

/// ffmpeg arguments that mix the timeline's audio from `start` to `end` and write it to stdout as
/// interleaved stereo 32-bit float samples at `rate` Hz, for playback.
pub fn preview_audio_args(project: &Project, start: i64, end: i64, rate: u32) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["-hide_banner", "-loglevel", "error", "-nostdin"].map(OsString::from).to_vec();
    let (inputs, graph) = audio_graph(project, start, end, 0);
    args.extend(inputs);
    args.extend(["-filter_complex".into(), graph.into()]);
    args.extend(["-map", "[aout]", "-f", "f32le", "-ac", "2"].map(OsString::from));
    args.extend(["-ar".into(), rate.to_string().into(), "pipe:1".into()]);
    args
}

/// The full ffmpeg argument list for an export.
pub fn ffmpeg_args(project: &Project, settings: &ExportSettings, start: i64, end: i64) -> Vec<OsString> {
    let preset = settings.preset;
    let s = &project.settings;
    let mut args: Vec<OsString> = ["-hide_banner", "-loglevel", "error", "-y"].map(OsString::from).to_vec();
    let vaapi = preset.video.as_ref().is_some_and(|v| v.hardware == Hardware::Vaapi);
    if vaapi {
        let device = std::env::var("PICASSO_VAAPI_DEVICE").unwrap_or_else(|_| "/dev/dri/renderD128".into());
        args.extend(["-vaapi_device".into(), device.into()]);
    }
    let mut next_input = 0;
    if preset.video.is_some() {
        args.extend(["-f", "rawvideo", "-pix_fmt", "rgba"].map(OsString::from));
        args.extend(["-s".into(), format!("{}x{}", s.width, s.height).into()]);
        args.extend(["-framerate".into(), format!("{}/{}", s.fps.num, s.fps.den).into()]);
        args.extend(["-i".into(), "-".into()]);
        next_input = 1;
    }
    let with_audio = preset.audio.is_some();
    if with_audio {
        let (inputs, graph) = audio_graph(project, start, end, next_input);
        args.extend(inputs);
        args.extend(["-filter_complex".into(), graph.into()]);
    }
    let ext = output_target(settings).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if let Some(v) = &preset.video {
        args.extend(["-map".into(), "0:v".into()]);
        // RGB → YUV with the HD matrix and tag the stream accordingly.
        let rgb_out = v.pix_fmt.starts_with("rgb") || v.pix_fmt.starts_with("gbr");
        let mut vf = String::new();
        if !rgb_out {
            vf.push_str("scale=out_color_matrix=bt709:out_range=limited");
        }
        if vaapi {
            vf.push_str(&format!(",format={},hwupload", v.pix_fmt));
        }
        if !vf.is_empty() {
            args.extend(["-vf".into(), vf.into()]);
        }
        if !rgb_out {
            args.extend(["-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709", "-color_range", "tv"].map(OsString::from));
        }
        args.extend(preset.video_args(settings.quality, &ext).into_iter().map(OsString::from));
    }
    if with_audio {
        args.extend(["-map".into(), "[aout]".into()]);
        args.extend(preset.audio_args().into_iter().map(OsString::from));
        args.extend(["-ar".into(), s.sample_rate.to_string().into()]);
    }
    args.extend(["-t".into(), format!("{:.6}", frames_to_seconds(end - start, s.fps)).into()]);
    args.push(output_target(settings).into_os_string());
    args
}

/// Renders the timeline (or `settings.range`) to `settings.output`.
pub fn export(project: &Project, settings: &ExportSettings, progress: &ExportProgress) -> Result<(), RenderError> {
    let (start, end) = settings.range.unwrap_or((0, project.timeline.duration()));
    if end <= start {
        return Err(RenderError::Empty);
    }
    if let Some(dir) = settings.output.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    progress.total.store((end - start) as u64, Ordering::Relaxed);
    progress.done.store(0, Ordering::Relaxed);

    let mut cmd = command(Tool::Ffmpeg);
    cmd.args(ffmpeg_args(project, settings, start, end));
    let has_video = settings.preset.video.is_some();
    cmd.stdin(if has_video { Stdio::piped() } else { Stdio::null() });
    cmd.stdout(Stdio::null()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|_| picasso_media::MediaError::ToolMissing("ffmpeg"))?;
    let stderr = Arc::new(Mutex::new(String::new()));
    let mut pipe = child.stderr.take().unwrap();
    let sink = stderr.clone();
    let reader = std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = pipe.read_to_string(&mut buf);
        *sink.lock().unwrap() = buf;
    });
    let fail = |child: &mut std::process::Child, reader: std::thread::JoinHandle<()>, fallback: String| {
        let _ = child.wait();
        let _ = reader.join();
        let msg = stderr.lock().unwrap().trim().to_owned();
        RenderError::Ffmpeg(if msg.is_empty() { fallback } else { msg })
    };

    if has_video {
        let mut stdin = child.stdin.take().unwrap();
        let mut pool = DecoderPool::new(12);
        let (w, h) = (project.settings.width, project.settings.height);
        for frame in start..end {
            if progress.cancel.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RenderError::Cancelled);
            }
            let img = render_frame(project, frame, w, h, &mut pool);
            if let Err(e) = stdin.write_all(&img.data) {
                drop(stdin);
                return Err(fail(&mut child, reader, e.to_string()));
            }
            progress.done.fetch_add(1, Ordering::Relaxed);
        }
        drop(stdin);
        if let Some((_, err)) = pool.failures().iter().next() {
            log::warn!("some media could not be decoded: {err}");
        }
    } else {
        while child.try_wait()?.is_none() {
            if progress.cancel.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RenderError::Cancelled);
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    let status = child.wait()?;
    let _ = reader.join();
    if !status.success() {
        let msg = stderr.lock().unwrap().trim().to_owned();
        return Err(RenderError::Ffmpeg(if msg.is_empty() { format!("ffmpeg exited with {status}") } else { msg }));
    }
    progress.done.store((end - start) as u64, Ordering::Relaxed);
    Ok(())
}

/// Clips referencing files that no longer exist (sequences are checked by pattern directory).
pub fn missing_media(project: &Project) -> Vec<&MediaItem> {
    project
        .media
        .iter()
        .filter(|m| {
            let p: &Path = &m.path;
            if m.sequence.is_some() { !p.parent().is_some_and(Path::is_dir) } else { !p.is_file() }
        })
        .collect()
}


#[cfg(test)]
mod tests {
    use super::*;
    use picasso_core::*;
    use picasso_media::encoders::preset;

    fn project() -> Project {
        let mut p = Project::new("t", ProjectSettings { fps: Rational::new(25, 1), ..Default::default() });
        let m = MediaItem {
            id: new_id(),
            name: "a".into(),
            path: "a.mov".into(),
            sequence: None,
            info: MediaInfo {
                format_name: "mov".into(),
                duration_secs: 10.0,
                video: None,
                audio: Some(AudioInfo { codec: "pcm".into(), sample_rate: 48000, channels: 2 }),
            },
        };
        let id = m.id;
        p.media.push(m);
        p.append_media(id).unwrap();
        p.append_media(id).unwrap();
        p
    }

    #[test]
    fn audio_graph_offsets_and_mixes() {
        let p = project();
        let (inputs, graph) = audio_graph(&p, 50, 300, 1);
        assert_eq!(inputs.iter().filter(|a| *a == "-i").count(), 2);
        // First clip starts 2 s into its media (range starts at frame 50), second is delayed 8 s.
        assert!(inputs.contains(&OsString::from("2.000000")));
        assert!(graph.contains("[1:a:0]atrim=duration=8.000000"));
        assert!(graph.contains("adelay=delays=8000:all=1[a1]"));
        assert!(graph.contains("amix=inputs=2"));
        assert!(graph.ends_with("atrim=end=10.000000[aout]"));
    }

    #[test]
    fn preview_audio_streams_raw_stereo_floats() {
        let p = project();
        let args: Vec<String> = preview_audio_args(&p, 50, 300, 44100).iter().map(|a| a.to_string_lossy().into_owned()).collect();
        let tail = &args[args.len() - 9..];
        assert_eq!(tail, ["-map", "[aout]", "-f", "f32le", "-ac", "2", "-ar", "44100", "pipe:1"]);
        assert!(args.iter().any(|a| a.contains("amix=inputs=2")));
    }

    #[test]
    fn silent_timeline_uses_null_source() {
        let mut p = project();
        p.timeline.tracks[1].clips.clear();
        let (inputs, graph) = audio_graph(&p, 0, 25, 1);
        assert!(inputs.is_empty());
        assert!(graph.starts_with("anullsrc"));
    }

    #[test]
    fn h265_args_are_tagged_and_mapped() {
        let p = project();
        let settings = ExportSettings {
            preset: preset("h265").unwrap(),
            quality: Quality::High,
            output: "out/final.mp4".into(),
            range: None,
        };
        let args: Vec<String> = ffmpeg_args(&p, &settings, 0, 500).iter().map(|a| a.to_string_lossy().into_owned()).collect();
        let s = args.join(" ");
        assert!(s.contains("-f rawvideo -pix_fmt rgba -s 1920x1080 -framerate 25/1 -i -"));
        assert!(s.contains("-map 0:v"));
        assert!(s.contains("-c:v libx265"));
        assert!(s.contains("-map [aout] -c:a aac"));
        assert!(s.contains("out_color_matrix=bt709"));
        assert!(s.ends_with("-t 20.000000 out/final.mp4"));
    }

    #[test]
    fn image_sequence_target() {
        let settings = ExportSettings {
            preset: preset("png_seq").unwrap(),
            quality: Quality::Best,
            output: "renders/shot.png".into(),
            range: None,
        };
        assert_eq!(output_target(&settings), PathBuf::from("renders/shot_%06d.png"));
    }
}
