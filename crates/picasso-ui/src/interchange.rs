//! Timelines to and from other editors: OpenTimelineIO, Final Cut Pro 7 / Premiere XML and AAF
//! import (File > Import > Timeline, or a dropped `.otio`, `.xml` or `.aaf` file) and the XML
//! and AAF exports (File > Export, and the Deliver page's presets of the same names).
//!
//! An imported timeline goes on a new timeline of the project, as one edit. Its clips are matched
//! to the media pool by the path of their file, then by name; files that are not in the pool are
//! imported first (videos and sounds are read on a thread of their own, as dropped files are),
//! and files that are not on this computer are added as offline media, so the clips keep their
//! place and can be relinked.

use picasso_core::otio::{OtioMedia, OtioTimeline};
use picasso_core::{
    Command, ImportedClip, ImportedTrack, MediaId, MediaSource, Project, TrackKind,
};

use crate::app::StudioApp;
#[cfg(not(target_arch = "wasm32"))]
use crate::platform::Incoming;
use crate::platform::{self, ImportedFile};

/// A media file the timeline uses that was read in the background: its path, whether it is
/// sound only, and what its file says.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) type Probed = (String, bool, Result<picasso_media::MediaMetadata, String>);
/// The browser reads no files in the background.
#[cfg(target_arch = "wasm32")]
pub(crate) type Probed = (String, bool, Result<(), String>);

/// The most files of one timeline imported into the pool (the rest are listed offline).
const MAX_IMPORTED: usize = 200;

/// Timeline files larger than this are read on a thread of their own (a feature-length edit's
/// XML or AAF is several megabytes); smaller ones are read at once.
#[cfg(not(target_arch = "wasm32"))]
const READ_IN_BACKGROUND: usize = 1024 * 1024;

/// The kinds of other editors' timeline files that are imported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TimelineFile {
    Otio,
    Xml,
    Aaf,
}

/// A path as it is compared: forward slashes, any case.
fn same_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

/// A file name without its folder or extension, in any case.
fn stem(name: &str) -> String {
    let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
    file.rsplit_once('.')
        .map_or(file, |(s, _)| s)
        .to_lowercase()
}

/// The path of media with a file.
fn path_of(source: &MediaSource) -> Option<&str> {
    match source {
        MediaSource::Video { path } | MediaSource::Audio { path } | MediaSource::Image { path } => {
            Some(path)
        }
        _ => None,
    }
}

/// The media of the pool that `path` (or, failing that, the name) points to.
fn find_media(project: &Project, path: Option<&str>, names: &[&str]) -> Option<MediaId> {
    if let Some(path) = path {
        let wanted = same_path(path);
        if let Some(m) = project
            .media
            .iter()
            .find(|m| path_of(&m.source).is_some_and(|p| same_path(p) == wanted))
        {
            return Some(m.id);
        }
    }
    names
        .iter()
        .filter(|n| !n.trim().is_empty())
        .find_map(|n| {
            let wanted = stem(n);
            project.media.iter().find(|m| stem(&m.name) == wanted)
        })
        .map(|m| m.id)
}

/// The files of a timeline that are not in the pool yet and can be read here, without repeats.
fn files_to_import(project: &Project, timeline: &OtioTimeline) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for clip in timeline.tracks.iter().flat_map(|t| &t.clips) {
        let OtioMedia::File { path } = &clip.media else {
            continue;
        };
        let known = find_media(project, Some(path), &[]).is_some()
            || out.iter().any(|p| same_path(p) == same_path(path));
        if !known && out.len() < MAX_IMPORTED && readable(path) {
            out.push(path.clone());
        }
    }
    out
}

/// Whether a file can be imported from here: on this computer, and not a network share or an
/// address (see `picasso_decode::refuse_path`).
#[cfg(not(target_arch = "wasm32"))]
fn readable(path: &str) -> bool {
    picasso_decode::refuse_path(path).is_none() && std::path::Path::new(path).is_file()
}

/// The browser reads no paths.
#[cfg(target_arch = "wasm32")]
fn readable(_path: &str) -> bool {
    false
}

impl StudioApp {
    /// The timeline's name as a file name: letters and digits, the rest `_`.
    fn timeline_file_stem(&self) -> String {
        self.project()
            .timeline_name
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect()
    }

    /// File > Export > Timeline: XML, and the Final Cut Pro 7 and Premiere XML render presets
    /// (the same format): asks where, then writes the timeline as an `xmeml` sequence.
    pub(crate) fn export_xml(&mut self, ctx: &egui::Context, premiere: bool) {
        let written = picasso_core::xmeml::to_xmeml(self.project());
        let kind = if premiere {
            "Premiere XML"
        } else {
            "Final Cut Pro 7 XML"
        };
        platform::save_as(
            self.inbox.clone(),
            ctx.clone(),
            format!("{}.xml", self.timeline_file_stem()),
            (kind, vec!["xml"]),
            written.text.into_bytes(),
            false,
        );
        if written.skipped > 0 {
            self.info(format!(
                "{kind}: {} generated or compound clip(s) have no file and are left out",
                written.skipped
            ));
        }
    }

    /// File > Export > Timeline: AAF: asks where, then writes the timeline as an AAF file for
    /// Avid Media Composer, Pro Tools and the like.
    pub(crate) fn export_aaf(&mut self, ctx: &egui::Context) {
        let written = match picasso_aaf::to_aaf(self.project()) {
            Ok(w) => w,
            Err(e) => {
                self.fail(format!("AAF: {e}"));
                return;
            }
        };
        platform::save_as(
            self.inbox.clone(),
            ctx.clone(),
            format!("{}.aaf", self.timeline_file_stem()),
            ("AAF", vec!["aaf"]),
            written.bytes,
            false,
        );
        if written.skipped > 0 {
            self.info(format!(
                "AAF: {} generated or compound clip(s) have no file and are left out",
                written.skipped
            ));
        }
    }

    /// Reads another editor's timeline file onto a new timeline: an OpenTimelineIO file, a Final
    /// Cut Pro 7 / Premiere / Resolve XML sequence, or an AAF composition (Avid Media Composer,
    /// Pro Tools, Premiere, Resolve). A large file is read on a thread of its own, so the window
    /// does not wait for it; `import_read_timeline` goes on with what was read.
    pub(crate) fn import_timeline_file(&mut self, name: &str, format: TimelineFile, bytes: Vec<u8>) {
        let fps = self.project().fps;
        let size = bytes.len();
        let read = move || match format {
            TimelineFile::Otio => {
                picasso_core::otio::read_otio(&String::from_utf8_lossy(&bytes), fps)
            }
            TimelineFile::Xml => {
                picasso_core::xmeml::read_xmeml(&String::from_utf8_lossy(&bytes), fps)
            }
            TimelineFile::Aaf => picasso_aaf::read_aaf(&bytes, fps),
        };
        #[cfg(not(target_arch = "wasm32"))]
        if size > READ_IN_BACKGROUND {
            self.info(format!("Reading {name}..."));
            let (name, inbox, ctx, epoch) = (
                name.to_string(),
                self.inbox.clone(),
                self.ctx.clone(),
                self.epoch,
            );
            std::thread::spawn(move || {
                let read = read();
                inbox.push(&ctx, Incoming::TimelineRead { name, read, epoch });
            });
            return;
        }
        let _ = size;
        self.import_read_timeline(name, read());
    }

    /// Puts a timeline read from another editor's file on a new timeline. The videos and sounds
    /// it uses that are not in the pool are read in the background first; `finish_otio` goes on
    /// with them.
    pub(crate) fn import_read_timeline(&mut self, name: &str, read: Result<OtioTimeline, String>) {
        let timeline = match read {
            Ok(t) if t.tracks.iter().all(|t| t.clips.is_empty()) => {
                self.fail(format!("{name}: the timeline has no clips to import"));
                return;
            }
            Ok(t) => t,
            Err(e) => {
                self.fail(format!("{name}: {e}"));
                return;
            }
        };
        let files: Vec<(String, bool)> = files_to_import(self.project(), &timeline)
            .into_iter()
            .filter(|p| platform::is_path_media(p))
            .map(|p| {
                let sound = platform::is_audio_name(&p);
                (p, sound)
            })
            .collect();
        if files.is_empty() {
            self.finish_otio(name, timeline, Vec::new(), self.epoch);
            return;
        }
        self.info(format!("Reading the media of {name}..."));
        let (name, inbox, ctx, epoch) = (
            name.to_string(),
            self.inbox.clone(),
            self.ctx.clone(),
            self.epoch,
        );
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let probed = files
                .into_iter()
                .map(|(path, sound)| {
                    let read = crate::app::probe_file(&path, sound);
                    (path, sound, read)
                })
                .collect();
            inbox.push(
                &ctx,
                Incoming::OtioRead {
                    name,
                    timeline,
                    probed,
                    epoch,
                },
            );
        });
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (inbox, ctx, files);
            self.finish_otio(&name, timeline, Vec::new(), epoch);
        }
    }

    /// The media of an OTIO timeline was read: adds what is missing to the pool, then puts the
    /// timeline on a new timeline of the project.
    pub(crate) fn finish_otio(
        &mut self,
        name: &str,
        timeline: OtioTimeline,
        probed: Vec<Probed>,
        epoch: u64,
    ) {
        if epoch != self.epoch {
            self.info(format!(
                "{name} was not imported: another project was opened while it was read"
            ));
            return;
        }
        // Videos and sounds read in the background, then images (quick to read here).
        #[cfg(not(target_arch = "wasm32"))]
        for (path, sound, read) in probed {
            let file = path.rsplit(['/', '\\']).next().unwrap_or(&path).to_string();
            self.add_probed(
                ImportedFile {
                    name: file,
                    path: Some(path),
                    bytes: Vec::new(),
                },
                sound,
                read,
            );
        }
        #[cfg(target_arch = "wasm32")]
        let _ = probed;
        for path in files_to_import(self.project(), &timeline) {
            if platform::is_path_media(&path) {
                continue;
            }
            if let Some(bytes) = platform::read_path(&path) {
                let file = path.rsplit(['/', '\\']).next().unwrap_or(&path).to_string();
                self.import_file(ImportedFile {
                    name: file,
                    path: Some(path),
                    bytes,
                });
            }
        }

        let fps = self.project().fps.nominal();
        let (mut offline, mut generated) = (0, 0);
        let mut tracks = Vec::new();
        for track in &timeline.tracks {
            let mut clips = Vec::new();
            for c in &track.clips {
                let path = match &c.media {
                    OtioMedia::File { path } => Some(path.as_str()),
                    _ => None,
                };
                let found = match &c.media {
                    OtioMedia::Generated(_) => None,
                    _ => find_media(self.project(), path, &[&c.media_name, &c.name]),
                };
                let media = match found {
                    Some(id) => Some(id),
                    None => {
                        // Kept as offline (or generated) media, so the clip keeps its place.
                        let source = match &c.media {
                            OtioMedia::Generated(source) => {
                                generated += 1;
                                source.clone()
                            }
                            other => {
                                offline += 1;
                                let path = match other {
                                    OtioMedia::File { path } => path.clone(),
                                    _ => c.media_name.clone(),
                                };
                                offline_source(path, track.kind)
                            }
                        };
                        let shown = [&c.media_name, &c.name]
                            .into_iter()
                            .find(|n| !n.trim().is_empty())
                            .map_or_else(|| "Offline".to_string(), |n| stem_keep_case(n));
                        let length = match source {
                            MediaSource::Video { .. } | MediaSource::Audio { .. } => c
                                .available
                                .filter(|l| *l > 0)
                                .or(Some(c.source_in + c.duration * 4)),
                            _ => None,
                        };
                        match self.run(Command::AddMedia {
                            name: shown,
                            source,
                            length,
                            default_duration: c.duration.max(1).min(fps * 3600),
                        }) {
                            Some(picasso_core::Outcome::Media(id)) => Some(id),
                            _ => None,
                        }
                    }
                };
                let Some(media) = media else { continue };
                clips.push(ImportedClip {
                    media,
                    name: if c.name.trim().is_empty() {
                        stem_keep_case(&c.media_name)
                    } else {
                        c.name.clone()
                    },
                    start: c.record,
                    source_in: c.source_in,
                    duration: c.duration,
                    speed: c.speed,
                    reverse: c.reverse,
                    freeze: c.freeze,
                    enabled: c.enabled,
                    transition: c.transition,
                });
            }
            tracks.push(ImportedTrack {
                kind: track.kind,
                muted: !track.enabled,
                clips,
            });
        }
        let wanted: usize = tracks.iter().map(|t| t.clips.len()).sum();
        let title = if timeline.name.is_empty() {
            name.rsplit_once('.').map_or(name, |(s, _)| s).to_string()
        } else {
            timeline.name.clone()
        };
        if self
            .run(Command::ImportTimeline {
                name: title,
                tracks,
                markers: timeline.markers,
            })
            .is_none()
        {
            return;
        }
        self.media_generation += 1;
        self.playhead = 0;
        let placed: usize = self.project().tracks.iter().map(|t| t.clips.len()).sum();
        let mut notes = Vec::new();
        if placed < wanted {
            notes.push(format!("{} could not be placed", wanted - placed));
        }
        if timeline.skipped > 0 {
            notes.push(format!(
                "{} nested or unknown item(s) left out",
                timeline.skipped
            ));
        }
        if offline > 0 {
            notes.push(format!("{offline} media file(s) not found (shown offline)"));
        }
        if generated > 0 {
            notes.push(format!("{generated} generated clip(s) made here"));
        }
        self.info(if notes.is_empty() {
            format!("Imported {placed} clips from {name}")
        } else {
            format!("Imported {placed} clips from {name}; {}", notes.join(", "))
        });
    }
}

/// Offline media for a file that is not here: a sound by its extension, or by the track it is
/// on when the name says nothing; a picture otherwise.
fn offline_source(path: String, track: TrackKind) -> MediaSource {
    if platform::is_audio_name(&path)
        || (track == TrackKind::Audio && !platform::is_video_name(&path))
    {
        MediaSource::Audio { path }
    } else if platform::is_path_media(&path) || !is_image_name(&path) {
        MediaSource::Video { path }
    } else {
        MediaSource::Image { path }
    }
}

fn is_image_name(name: &str) -> bool {
    const STILLS: [&str; 10] = [
        "png", "jpg", "jpeg", "tif", "tiff", "bmp", "exr", "dpx", "webp", "gif",
    ];
    let lower = name.to_ascii_lowercase();
    STILLS.iter().any(|e| lower.ends_with(&format!(".{e}")))
}

/// A file name without its folder or extension, as it is written.
fn stem_keep_case(name: &str) -> String {
    let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
    file.rsplit_once('.').map_or(file, |(s, _)| s).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_is_matched_by_path_then_name() {
        let mut p = Project::new("t");
        for (name, path) in [("Shot A", r"C:\Media\Shot A.mov"), ("b", "/x/b.wav")] {
            Command::AddMedia {
                name: name.into(),
                source: MediaSource::Video { path: path.into() },
                length: Some(100),
                default_duration: 10,
            }
            .apply(&mut p)
            .unwrap();
        }
        let (a, b) = (p.media[0].id, p.media[1].id);
        assert_eq!(find_media(&p, Some("c:/media/shot a.MOV"), &[]), Some(a));
        assert_eq!(
            find_media(&p, Some("/elsewhere/B.wav"), &["B.wav"]),
            Some(b)
        );
        assert_eq!(find_media(&p, Some("/elsewhere/z.wav"), &["", "z"]), None);
        assert_eq!(stem_keep_case("/a/Shot 1.mov"), "Shot 1");
    }
}
