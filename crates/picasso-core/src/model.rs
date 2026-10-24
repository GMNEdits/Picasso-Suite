//! Project data model: project → media pool → timeline → tracks → clips.

use crate::time::{Rational, frames_to_seconds, seconds_to_frames};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

pub type Id = Uuid;

pub fn new_id() -> Id {
    Uuid::new_v4()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub width: u32,
    pub height: u32,
    pub fps: Rational,
    pub sample_rate: u32,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self { width: 1920, height: 1080, fps: Rational::new(25, 1), sample_rate: 48_000 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: Id,
    pub name: String,
    pub settings: ProjectSettings,
    pub media: Vec<MediaItem>,
    pub timeline: Timeline,
}

impl Default for Project {
    fn default() -> Self {
        Self::new("Untitled", ProjectSettings::default())
    }
}

impl Project {
    pub fn new(name: &str, settings: ProjectSettings) -> Self {
        Self {
            id: new_id(),
            name: name.to_owned(),
            settings,
            media: Vec::new(),
            timeline: Timeline::with_default_tracks(),
        }
        .with_hour_start()
    }

    /// Starts the timeline's timecode at 01:00:00:00.
    fn with_hour_start(mut self) -> Self {
        self.timeline.start_timecode = self.settings.fps.nominal() * 3600;
        self
    }

    pub fn media(&self, id: Id) -> Option<&MediaItem> {
        self.media.iter().find(|m| m.id == id)
    }

    /// Length of a media item in timeline frames; `None` for stills (unbounded).
    pub fn media_frames(&self, id: Id) -> Option<i64> {
        let m = self.media(id)?;
        if m.info.is_still() {
            return None;
        }
        Some(seconds_to_frames(m.info.duration_secs, self.settings.fps).max(1))
    }

    /// Source time in seconds that a clip shows at timeline `frame`.
    pub fn source_seconds(&self, clip: &Clip, frame: i64) -> f64 {
        frames_to_seconds(clip.source_in + (frame - clip.start), self.settings.fps)
    }

    pub fn media_in_use(&self, id: Id) -> bool {
        self.timeline.tracks.iter().flat_map(|t| &t.clips).any(|c| c.media_id == id)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MediaItem {
    pub id: Id,
    pub name: String,
    /// File path, or a printf-style pattern (`shot_%04d.exr`) for image sequences.
    pub path: PathBuf,
    pub sequence: Option<ImageSequence>,
    pub info: MediaInfo,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageSequence {
    pub start_number: u64,
    pub frame_count: u64,
    pub fps: Rational,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MediaInfo {
    pub format_name: String,
    pub duration_secs: f64,
    pub video: Option<VideoInfo>,
    pub audio: Option<AudioInfo>,
}

impl MediaInfo {
    pub fn is_still(&self) -> bool {
        self.video.as_ref().is_some_and(|v| v.is_still) && self.audio.is_none()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VideoInfo {
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub fps: Rational,
    pub pix_fmt: String,
    pub has_alpha: bool,
    pub is_still: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioInfo {
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrackKind {
    Video,
    Audio,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Timeline {
    pub id: Id,
    pub name: String,
    /// Timecode shown at frame 0, in frames (01:00:00:00 by default, as is customary).
    #[serde(default)]
    pub start_timecode: i64,
    /// Video tracks composite bottom (index 0) to top.
    pub tracks: Vec<Track>,
}

impl Timeline {
    pub fn with_default_tracks() -> Self {
        Self {
            id: new_id(),
            name: "Timeline 1".into(),
            start_timecode: 0,
            tracks: vec![Track::new(TrackKind::Video, "V1"), Track::new(TrackKind::Audio, "A1")],
        }
    }

    pub fn duration(&self) -> i64 {
        self.tracks.iter().flat_map(|t| &t.clips).map(Clip::end).max().unwrap_or(0)
    }

    pub fn tracks_of(&self, kind: TrackKind) -> impl Iterator<Item = (usize, &Track)> {
        self.tracks.iter().enumerate().filter(move |(_, t)| t.kind == kind)
    }

    pub fn add_track(&mut self, kind: TrackKind) -> usize {
        let n = self.tracks_of(kind).count() + 1;
        let prefix = match kind {
            TrackKind::Video => "V",
            TrackKind::Audio => "A",
        };
        self.tracks.push(Track::new(kind, &format!("{prefix}{n}")));
        self.tracks.len() - 1
    }

    pub fn find_clip(&self, id: Id) -> Option<(usize, usize)> {
        self.tracks.iter().enumerate().find_map(|(ti, t)| {
            t.clips.iter().position(|c| c.id == id).map(|ci| (ti, ci))
        })
    }

    pub fn clip(&self, id: Id) -> Option<&Clip> {
        self.find_clip(id).map(|(t, c)| &self.tracks[t].clips[c])
    }

    pub fn clip_mut(&mut self, id: Id) -> Option<&mut Clip> {
        self.find_clip(id).map(|(t, c)| &mut self.tracks[t].clips[c])
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: Id,
    pub kind: TrackKind,
    pub name: String,
    pub enabled: bool,
    pub locked: bool,
    /// Sorted by `start`, never overlapping.
    pub clips: Vec<Clip>,
}

impl Track {
    pub fn new(kind: TrackKind, name: &str) -> Self {
        Self { id: new_id(), kind, name: name.into(), enabled: true, locked: false, clips: Vec::new() }
    }

    pub fn clip_at(&self, frame: i64) -> Option<&Clip> {
        self.clips.iter().find(|c| c.contains(frame))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub id: Id,
    pub media_id: Id,
    pub name: String,
    /// First timeline frame.
    pub start: i64,
    /// Length in timeline frames (> 0).
    pub duration: i64,
    /// Offset into the media, in timeline frames.
    pub source_in: i64,
    /// Clips sharing a link id (e.g. picture + sound) move, trim and delete together.
    pub link: Option<Id>,
    pub enabled: bool,
    pub transform: Transform,
    /// Linear gain for audio clips.
    pub volume: f32,
}

impl Clip {
    pub fn new(media_id: Id, name: &str, start: i64, duration: i64, source_in: i64) -> Self {
        Self {
            id: new_id(),
            media_id,
            name: name.into(),
            start,
            duration,
            source_in,
            link: None,
            enabled: true,
            transform: Transform::default(),
            volume: 1.0,
        }
    }

    pub fn end(&self) -> i64 {
        self.start + self.duration
    }

    pub fn contains(&self, frame: i64) -> bool {
        frame >= self.start && frame < self.end()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    /// 1.0 = fit inside the frame preserving aspect ratio.
    pub zoom: f32,
    /// Offset in timeline pixels.
    pub pan_x: f32,
    pub pan_y: f32,
    pub opacity: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Self { zoom: 1.0, pan_x: 0.0, pan_y: 0.0, opacity: 1.0 }
    }
}
