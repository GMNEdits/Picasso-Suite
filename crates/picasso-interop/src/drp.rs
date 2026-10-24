//! Import of DaVinci Resolve project exports (`.drp`).
//!
//! A `.drp` is a zip of XML documents: `project.xml`, the media pool under
//! `MediaPool/`, and one `SeqContainer/<id>.xml` per timeline holding tracks and
//! clips. This reader uses only the plain-XML fields (names, frame positions,
//! media file paths, frame rates, resolution); the opaque binary blobs Resolve
//! also stores (grades, Fusion comps, effects) are not interpreted. Media paths
//! are resolved for the current OS with [`Relinker`], so a project exported on
//! macOS, Windows or Linux opens on any of them.

use crate::relink::{Relinker, file_name};
use picasso_core::time::seconds_to_frames;
use picasso_core::{AudioInfo, Clip, Id, MediaInfo, MediaItem, Project, ProjectSettings, Rational, Track, TrackKind, VideoInfo, new_id};
use roxmltree::{Document, Node};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum DrpError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a valid .drp archive: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("malformed XML in {0}: {1}")]
    Xml(String, String),
    #[error("the project contains no timelines")]
    NoTimeline,
}

/// A timeline as described in the archive.
#[derive(Clone, Debug, PartialEq)]
pub struct DrpTimeline {
    pub id: String,
    pub name: String,
    pub sequence_id: String,
    pub fps: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub start_seconds: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DrpClip {
    pub element: String,
    pub name: String,
    pub start: i64,
    pub duration: i64,
    pub source_in: Option<i64>,
    pub media_path: Option<String>,
    pub media_fps: Option<f64>,
    pub media_start_seconds: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DrpTrack {
    pub kind: TrackKind,
    pub sequence_id: String,
    pub name: Option<String>,
    pub clips: Vec<DrpClip>,
}

/// Everything read from a `.drp`, before conversion.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DrpArchive {
    pub app_version: Option<String>,
    pub project_name: String,
    pub current_timeline: Option<String>,
    pub timelines: Vec<DrpTimeline>,
    pub tracks: Vec<DrpTrack>,
}

pub struct ImportReport {
    pub project: Project,
    pub timeline_name: String,
    pub other_timelines: Vec<String>,
    pub clips: usize,
    pub offline: Vec<String>,
    pub skipped: usize,
}

impl ImportReport {
    pub fn summary(&self) -> String {
        let mut s = format!("Imported \"{}\": {} clip(s)", self.timeline_name, self.clips);
        if !self.offline.is_empty() {
            s.push_str(&format!(", {} offline file(s) — use File ▸ Relink", self.offline.len()));
        }
        if self.skipped > 0 {
            s.push_str(&format!(", {} title/generator/effect item(s) skipped", self.skipped));
        }
        if !self.other_timelines.is_empty() {
            s.push_str(&format!(", {} other timeline(s) not imported", self.other_timelines.len()));
        }
        s
    }
}

// ------------------------------------------------------------------ reading

pub fn read_archive(path: &Path) -> Result<DrpArchive, DrpError> {
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file)?;
    let mut docs = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name()?.replace('\\', "/");
        if !name.to_ascii_lowercase().ends_with(".xml") {
            continue;
        }
        let mut text = String::new();
        entry.read_to_string(&mut text)?;
        docs.push((name, text));
    }
    parse_documents(&docs)
}

/// Parses the XML documents of an archive (`(entry name, contents)`).
pub fn parse_documents(docs: &[(String, String)]) -> Result<DrpArchive, DrpError> {
    let mut out = DrpArchive::default();
    let mut timeline_handles = Vec::new();
    let mut current_index = 0usize;
    for (name, text) in docs {
        let text = &sanitize_tag_names(text);
        let doc = Document::parse(text).map_err(|e| DrpError::Xml(name.clone(), e.to_string()))?;
        if out.app_version.is_none() {
            out.app_version = header_version(text);
        }
        let root = doc.root_element();
        if root.tag_name().name() == "SM_Project" {
            out.project_name = child_text(root, "ProjectName").unwrap_or_default();
            if let Some(handles) = child(root, "TimelineHandleVec") {
                timeline_handles = handles.children().filter(|n| n.has_tag_name("Element")).filter_map(|n| n.text()).map(|t| t.trim().to_owned()).collect();
            }
            current_index = child_text(root, "CurrentTimelineIndex").and_then(|v| v.parse().ok()).unwrap_or(0);
        }
        for tl in root.descendants().filter(|n| n.has_tag_name("Sm2Timeline")) {
            let Some(seq) = tl.descendants().find(|n| n.has_tag_name("Sm2Sequence")) else { continue };
            let resolution = child_text(seq, "Resolution").and_then(|h| hex_be_u64_pair(&h));
            out.timelines.push(DrpTimeline {
                id: tl.attribute("DbId").unwrap_or_default().to_owned(),
                name: child_text(tl, "Name").unwrap_or_else(|| "Timeline".into()),
                sequence_id: seq.attribute("DbId").unwrap_or_default().to_owned(),
                fps: child_text(seq, "FrameRate").and_then(|h| hex_le_f64(&h)).filter(|f| *f > 0.0 && *f < 1000.0),
                width: resolution.map(|(w, _)| w as u32).filter(|w| *w > 0 && *w <= 32768),
                height: resolution.map(|(_, h)| h as u32).filter(|h| *h > 0 && *h <= 32768),
                start_seconds: child_text(seq, "MediaExtents").and_then(|h| hex_le_f64(&h)).filter(|s| s.is_finite() && *s >= 0.0).unwrap_or(0.0),
            });
        }
        for (vec_tag, kind) in [("VideoTrackVec", TrackKind::Video), ("AudioTrackVec", TrackKind::Audio)] {
            if root.tag_name().name() != "Sm2SequenceContainer" {
                continue;
            }
            let Some(vec) = child(root, vec_tag) else { continue };
            for track in vec.descendants().filter(|n| n.has_tag_name("Sm2TiTrack")) {
                let clips = child(track, "Items")
                    .map(|items| {
                        items
                            .children()
                            .filter(|n| n.has_tag_name("Element"))
                            .filter_map(|e| e.children().find(|n| n.is_element()))
                            .filter_map(parse_clip)
                            .collect()
                    })
                    .unwrap_or_default();
                out.tracks.push(DrpTrack {
                    kind,
                    sequence_id: child_text(track, "Sequence").unwrap_or_default(),
                    name: child_text(track, "UserDefinedName").filter(|n| !n.is_empty()),
                    clips,
                });
            }
        }
    }
    out.current_timeline = timeline_handles.get(current_index).cloned().or_else(|| timeline_handles.first().cloned());
    Ok(out)
}

/// Resolve writes C++-style tag names (`<ListMgt::LmVersion>`), which are not
/// well-formed XML. Rewrites `::` inside tag names to `__`.
pub fn sanitize_tag_names(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_name = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '<' => {
                in_name = true;
                out.push(c);
                if chars.peek() == Some(&'/') {
                    out.push(chars.next().unwrap());
                }
            }
            ':' if in_name && chars.peek() == Some(&':') => {
                chars.next();
                out.push_str("__");
            }
            c if in_name && (c.is_whitespace() || c == '>' || c == '/') => {
                in_name = false;
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out
}

fn parse_clip(node: Node) -> Option<DrpClip> {
    let start = child_text(node, "Start")?.parse().ok()?;
    let duration: i64 = child_text(node, "Duration")?.parse().ok()?;
    if duration <= 0 {
        return None;
    }
    Some(DrpClip {
        element: node.tag_name().name().to_owned(),
        name: child_text(node, "Name").unwrap_or_default(),
        start,
        duration,
        source_in: child_text(node, "In").and_then(|v| v.parse().ok()),
        media_path: child_text(node, "MediaFilePath").filter(|p| !p.trim().is_empty()),
        media_fps: child_text(node, "MediaFrameRate").and_then(|h| hex_le_f64(&h)).filter(|f| *f > 0.0 && *f < 1000.0),
        media_start_seconds: child_text(node, "MediaStartTime").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0),
    })
}

fn child<'a, 'i>(node: Node<'a, 'i>, tag: &str) -> Option<Node<'a, 'i>> {
    node.children().find(|n| n.has_tag_name(tag))
}

fn child_text(node: Node, tag: &str) -> Option<String> {
    child(node, tag).map(|n| n.text().unwrap_or_default().trim().to_owned())
}

/// `<!--DbAppVer="19.1.1.0004" ...-->`
fn header_version(text: &str) -> Option<String> {
    let i = text.find("DbAppVer=\"")? + 10;
    Some(text[i..].split('"').next()?.to_owned())
}

fn hex_bytes(hex: &str) -> Option<Vec<u8>> {
    let hex = hex.trim();
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok()).collect()
}

/// First 8 bytes as a little-endian IEEE double.
pub fn hex_le_f64(hex: &str) -> Option<f64> {
    let b = hex_bytes(hex)?;
    Some(f64::from_le_bytes(b.get(..8)?.try_into().ok()?))
}

/// Two big-endian 64-bit integers.
pub fn hex_be_u64_pair(hex: &str) -> Option<(u64, u64)> {
    let b = hex_bytes(hex)?;
    Some((u64::from_be_bytes(b.get(..8)?.try_into().ok()?), u64::from_be_bytes(b.get(8..16)?.try_into().ok()?)))
}

// ------------------------------------------------------------------ conversion

/// Imports the archive's current timeline as a Picasso Suite project.
pub fn import(path: &Path, relinker: &Relinker) -> Result<ImportReport, DrpError> {
    let archive = read_archive(path)?;
    convert(&archive, relinker, |p, fps| picasso_media::probe(p, fps).ok())
}

/// Converts a parsed archive; `probe` reads local media (injectable for tests).
pub fn convert(
    archive: &DrpArchive,
    relinker: &Relinker,
    probe: impl Fn(&Path, Rational) -> Option<MediaItem>,
) -> Result<ImportReport, DrpError> {
    let timeline = archive
        .current_timeline
        .as_ref()
        .and_then(|id| archive.timelines.iter().find(|t| &t.id == id))
        .or_else(|| archive.timelines.iter().find(|t| archive.tracks.iter().any(|tr| tr.sequence_id == t.sequence_id)))
        .or(archive.timelines.first())
        .ok_or(DrpError::NoTimeline)?;
    let tracks: Vec<&DrpTrack> = archive.tracks.iter().filter(|t| t.sequence_id == timeline.sequence_id).collect();

    let fps = timeline.fps.and_then(Rational::from_f64).unwrap_or(Rational::new(24, 1));
    let settings = ProjectSettings {
        width: timeline.width.unwrap_or(1920) & !1,
        height: timeline.height.unwrap_or(1080) & !1,
        fps,
        ..ProjectSettings::default()
    };
    let name = if archive.project_name.is_empty() { "Imported" } else { archive.project_name.as_str() };
    let mut project = Project::new(name, settings.clone());
    project.timeline.name = timeline.name.clone();
    project.timeline.tracks.clear();
    let start_frame = (timeline.start_seconds * fps.as_f64()).round() as i64;
    project.timeline.start_timecode = start_frame;

    let mut media_by_path: HashMap<String, Id> = HashMap::new();
    let mut offline = Vec::new();
    let mut skipped = 0;
    let mut clips = 0;
    let mut counters = HashMap::new();
    for track in &tracks {
        let n = counters.entry(track.kind).or_insert(0);
        *n += 1;
        let prefix = if track.kind == TrackKind::Video { "V" } else { "A" };
        let mut out = Track::new(track.kind, &track.name.clone().unwrap_or(format!("{prefix}{n}")));
        for c in &track.clips {
            let Some(original) = &c.media_path else {
                skipped += 1;
                continue;
            };
            let media_id = *media_by_path.entry(original.clone()).or_insert_with(|| {
                let item = match relinker.resolve(original).and_then(|p| probe(&p, fps)) {
                    Some(item) => item,
                    None => {
                        offline.push(original.clone());
                        offline_media(original, c, track.kind, &settings)
                    }
                };
                let id = item.id;
                project.media.push(item);
                id
            });
            ensure_stream(&mut project, media_id, track.kind, &settings, c);
            let media_fps = c.media_fps.unwrap_or(fps.as_f64());
            let source_in_frames = c.source_in.map_or(0, |i| {
                // `In` counts source frames; some exports include the media's start timecode.
                let tc_offset = (c.media_start_seconds * media_fps).round() as i64;
                if tc_offset > 0 && i >= tc_offset { i - tc_offset } else { i }
            });
            let source_in = seconds_to_frames(source_in_frames as f64 / media_fps, fps);
            let mut clip = Clip::new(media_id, &c.name, c.start - start_frame, c.duration, source_in.max(0));
            if clip.name.is_empty() {
                clip.name = file_name(original).unwrap_or_default();
            }
            clip.start = clip.start.max(0);
            out.place(clip);
            clips += 1;
        }
        project.timeline.tracks.push(out);
    }
    if project.timeline.tracks.is_empty() {
        project.timeline = picasso_core::Timeline::with_default_tracks();
        project.timeline.name = timeline.name.clone();
    }
    link_pairs(&mut project);
    let other_timelines = archive.timelines.iter().filter(|t| t.id != timeline.id).map(|t| t.name.clone()).collect();
    Ok(ImportReport { project, timeline_name: timeline.name.clone(), other_timelines, clips, offline, skipped })
}

/// Placeholder for a file that couldn't be found; relinking replaces it.
fn offline_media(original: &str, clip: &DrpClip, kind: TrackKind, settings: &ProjectSettings) -> MediaItem {
    let mut item = MediaItem {
        id: new_id(),
        name: file_name(original).unwrap_or_else(|| original.to_owned()),
        path: PathBuf::from(original),
        sequence: None,
        info: MediaInfo { format_name: "offline".into(), duration_secs: 0.0, video: None, audio: None },
    };
    let media_fps = clip.media_fps.unwrap_or(settings.fps.as_f64());
    let end = clip.source_in.unwrap_or(0) + clip.duration;
    item.info.duration_secs = (end as f64 / media_fps).max(1.0);
    let _ = kind;
    item
}

/// Offline media learns which streams it has from the tracks that use it.
fn ensure_stream(project: &mut Project, media: Id, kind: TrackKind, settings: &ProjectSettings, clip: &DrpClip) {
    let Some(m) = project.media.iter_mut().find(|m| m.id == media) else { return };
    if m.info.format_name != "offline" {
        return;
    }
    let media_fps = clip.media_fps.and_then(Rational::from_f64).unwrap_or(settings.fps);
    let end = (clip.source_in.unwrap_or(0) + clip.duration) as f64 / media_fps.as_f64();
    m.info.duration_secs = m.info.duration_secs.max(end);
    match kind {
        TrackKind::Video if m.info.video.is_none() => {
            m.info.video = Some(VideoInfo {
                codec: "offline".into(),
                width: settings.width,
                height: settings.height,
                fps: media_fps,
                pix_fmt: String::new(),
                has_alpha: false,
                is_still: false,
            })
        }
        TrackKind::Audio if m.info.audio.is_none() => {
            m.info.audio = Some(AudioInfo { codec: "offline".into(), sample_rate: settings.sample_rate, channels: 2 })
        }
        _ => {}
    }
}

/// Links a video clip and an audio clip of the same media at the same position.
fn link_pairs(project: &mut Project) {
    let mut by_key: HashMap<(Id, i64, i64), Vec<(usize, usize)>> = HashMap::new();
    for (ti, t) in project.timeline.tracks.iter().enumerate() {
        for (ci, c) in t.clips.iter().enumerate() {
            by_key.entry((c.media_id, c.start, c.duration)).or_default().push((ti, ci));
        }
    }
    for group in by_key.values() {
        let has_v = group.iter().any(|(t, _)| project.timeline.tracks[*t].kind == TrackKind::Video);
        let has_a = group.iter().any(|(t, _)| project.timeline.tracks[*t].kind == TrackKind::Audio);
        if has_v && has_a {
            let link = new_id();
            for (t, c) in group {
                project.timeline.tracks[*t].clips[*c].link = Some(link);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal archive in the same shape as Resolve's exports.
    fn docs() -> Vec<(String, String)> {
        let project = r#"<?xml version="1.0" encoding="UTF-8"?>
<!--DbAppVer="20.0.0.0001" DbPrjVer="14"-->
<SM_Project DbId="p1"><ProjectName>Trailer</ProjectName>
 <TimelineHandleVec><Element>tl-b</Element><Element>tl-a</Element></TimelineHandleVec>
 <CurrentTimelineIndex>1</CurrentTimelineIndex></SM_Project>"#;
        // 25 fps (LE double), 1280x720 (BE pairs), start 3600 s.
        let pool = r#"<?xml version="1.0" encoding="UTF-8"?>
<Sm2MpFolder DbId="f"><Name>Master</Name><MediaVec>
 <Element><Sm2MpTimelineClip DbId="c1"><Name>Main</Name><TimelineSharedHandle>
  <Sm2Timeline DbId="tl-a"><Name>Main</Name><Sequence><Sm2Sequence DbId="seq-a">
   <MediaExtents>000000000020ac400000000000001440</MediaExtents>
   <FrameRate>00000000000039400000000000000000</FrameRate>
   <Resolution>000000000000050000000000000002d0</Resolution>
  </Sm2Sequence></Sequence></Sm2Timeline></TimelineSharedHandle></Sm2MpTimelineClip></Element>
 <Element><Sm2MpTimelineClip DbId="c2"><Name>Alt</Name><TimelineSharedHandle>
  <Sm2Timeline DbId="tl-b"><Name>Alt</Name><Sequence><Sm2Sequence DbId="seq-b">
   <FrameRate>00000000000038400000000000000000</FrameRate>
  </Sm2Sequence></Sequence></Sm2Timeline></TimelineSharedHandle></Sm2MpTimelineClip></Element>
</MediaVec></Sm2MpFolder>"#;
        let seq = r#"<?xml version="1.0" encoding="UTF-8"?>
<Sm2SequenceContainer DbId="s">
 <VideoTrackVec><Element><Sm2TiTrack DbId="v1"><Type>0</Type><Sequence>seq-a</Sequence><Items>
  <Element><Sm2TiVideoClip DbId="x1"><Name>A001.mov</Name><Start>90000</Start><Duration>100</Duration><In>25</In>
   <MediaStartTime>0</MediaStartTime><MediaFilePath>/Volumes/Shoot/Day1/A001.mov</MediaFilePath>
   <MediaFrameRate>00000000000039400000000000000000</MediaFrameRate></Sm2TiVideoClip></Element>
  <Element><Sm2TiVideoClip DbId="x2"><pLmVerTable><ListMgt::LmVersionTable DbId="v"/></pLmVerTable><Name>Fusion Title</Name><Start>90100</Start><Duration>50</Duration><In/>
   <MediaFilePath/></Sm2TiVideoClip></Element>
 </Items></Sm2TiTrack></Element></VideoTrackVec>
 <AudioTrackVec><Element><Sm2TiTrack DbId="a1"><Type>1</Type><Sequence>seq-a</Sequence><Items>
  <Element><Sm2TiAudioClip DbId="y1"><Name>A001.mov</Name><Start>90000</Start><Duration>100</Duration><In>25</In>
   <MediaFilePath>/Volumes/Shoot/Day1/A001.mov</MediaFilePath>
   <MediaFrameRate>00000000000039400000000000000000</MediaFrameRate></Sm2TiAudioClip></Element>
 </Items></Sm2TiTrack></Element></AudioTrackVec>
</Sm2SequenceContainer>"#;
        vec![
            ("project.xml".into(), project.into()),
            ("MediaPool/Master/MpFolder.xml".into(), pool.into()),
            ("SeqContainer/s.xml".into(), seq.into()),
        ]
    }

    #[test]
    fn sanitizes_cpp_tag_names_only() {
        assert_eq!(sanitize_tag_names("<A::B x=\"1::2\">t::u</A::B>"), "<A__B x=\"1::2\">t::u</A__B>");
    }

    #[test]
    fn decodes_binary_fields() {
        assert_eq!(hex_le_f64("00000000000038400000000000000000"), Some(24.0));
        assert_eq!(hex_le_f64("000000000020ac400000000000001440"), Some(3600.0));
        assert_eq!(hex_be_u64_pair("00000000000007800000000000000438"), Some((1920, 1080)));
    }

    #[test]
    fn parses_structure() {
        let a = parse_documents(&docs()).unwrap();
        assert_eq!(a.project_name, "Trailer");
        assert_eq!(a.app_version.as_deref(), Some("20.0.0.0001"));
        assert_eq!(a.current_timeline.as_deref(), Some("tl-a"));
        let main = a.timelines.iter().find(|t| t.id == "tl-a").unwrap();
        assert_eq!((main.fps, main.width, main.height, main.start_seconds), (Some(25.0), Some(1280), Some(720), 3600.0));
        assert_eq!(a.tracks.len(), 2);
        assert_eq!(a.tracks[0].clips.len(), 2);
    }

    #[test]
    fn converts_with_cross_os_relink() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("Day1")).unwrap();
        std::fs::write(dir.path().join("Day1").join("A001.mov"), b"x").unwrap();
        let relinker = Relinker::with_roots(vec![dir.path().to_path_buf()]);
        let probe = |p: &Path, fps: Rational| {
            Some(MediaItem {
                id: new_id(),
                name: "A001.mov".into(),
                path: p.to_path_buf(),
                sequence: None,
                info: MediaInfo {
                    format_name: "mov".into(),
                    duration_secs: 20.0,
                    video: Some(VideoInfo { codec: "h264".into(), width: 1280, height: 720, fps, pix_fmt: "yuv420p".into(), has_alpha: false, is_still: false }),
                    audio: Some(AudioInfo { codec: "aac".into(), sample_rate: 48000, channels: 2 }),
                },
            })
        };
        let r = convert(&parse_documents(&docs()).unwrap(), &relinker, probe).unwrap();
        let p = &r.project;
        assert_eq!(r.timeline_name, "Main");
        assert_eq!((p.settings.width, p.settings.height, p.settings.fps), (1280, 720, Rational::new(25, 1)));
        assert_eq!(r.clips, 2);
        assert_eq!(r.skipped, 1);
        assert!(r.offline.is_empty());
        assert_eq!(r.other_timelines, vec!["Alt".to_string()]);
        let v = &p.timeline.tracks[0].clips[0];
        assert_eq!((v.start, v.duration, v.source_in), (0, 100, 25));
        assert!(p.media[0].path.ends_with(Path::new("Day1").join("A001.mov")));
        // Picture and sound of the same take are linked.
        assert_eq!(p.linked_group(v.id).len(), 2);
    }

    #[test]
    fn missing_media_becomes_offline() {
        let r = convert(&parse_documents(&docs()).unwrap(), &Relinker::default(), |_, _| None).unwrap();
        assert_eq!(r.offline, vec!["/Volumes/Shoot/Day1/A001.mov".to_string()]);
        let m = &r.project.media[0];
        assert!(m.info.video.is_some() && m.info.audio.is_some());
        assert!(r.summary().contains("offline"));
    }

    /// Reads the sample project that ships with DaVinci Resolve, if installed.
    #[test]
    fn reads_resolve_sample_if_present() {
        let candidates = [
            r"C:\ProgramData\Blackmagic Design\DaVinci Resolve\Support\Developer\Fusion Fuse\Fuse.drp",
            "/Library/Application Support/Blackmagic Design/DaVinci Resolve/Developer/Fusion Fuse/Fuse.drp",
            "/opt/resolve/Developer/Fusion Fuse/Fuse.drp",
        ];
        let Some(path) = candidates.iter().map(Path::new).find(|p| p.is_file()) else { return };
        let a = read_archive(path).unwrap();
        assert_eq!(a.project_name, "Fuse");
        let tl = &a.timelines[0];
        assert_eq!((tl.fps, tl.width, tl.height), (Some(24.0), Some(1920), Some(1080)));
        let r = convert(&a, &Relinker::default(), |_, _| None).unwrap();
        assert_eq!(r.project.settings.fps, Rational::new(24, 1));
    }
}
