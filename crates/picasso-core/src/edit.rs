//! Editing operations. Every operation keeps each track sorted and overlap-free;
//! placing a clip over existing material overwrites it (NLE "overwrite" edit).

use crate::model::{Clip, Id, Project, Track, TrackKind, new_id};
use crate::time::seconds_to_frames;
use std::collections::{HashMap, HashSet};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EditError {
    #[error("clip not found")]
    ClipNotFound,
    #[error("media not found")]
    MediaNotFound,
    #[error("track {0} is locked")]
    TrackLocked(String),
    #[error("a {0:?} clip cannot go on that track")]
    TrackKindMismatch(TrackKind),
    #[error("media in use on the timeline")]
    MediaInUse,
    #[error("nothing to do")]
    NoOp,
}

pub type EditResult<T> = Result<T, EditError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Start,
    End,
}

/// Default length of a still image when it is placed on the timeline.
pub const STILL_SECONDS: f64 = 5.0;

impl Track {
    fn sort(&mut self) {
        self.clips.sort_by_key(|c| c.start);
    }

    /// Removes all material in `[start, end)`, splitting clips that straddle it.
    pub fn clear_range(&mut self, start: i64, end: i64) {
        if end <= start {
            return;
        }
        let mut out = Vec::with_capacity(self.clips.len() + 1);
        for c in self.clips.drain(..) {
            if c.end() <= start || c.start >= end {
                out.push(c);
                continue;
            }
            if c.start < start {
                let mut left = c.clone();
                left.duration = start - c.start;
                out.push(left);
            }
            if c.end() > end {
                let mut right = c.clone();
                if c.start < start {
                    // Both halves survive: the right one is a new clip, unlinked.
                    right.id = new_id();
                    right.link = None;
                }
                right.source_in += end - c.start;
                right.start = end;
                right.duration = c.end() - end;
                out.push(right);
            }
        }
        self.clips = out;
        self.sort();
    }

    /// Overwrites `clip` onto the track.
    pub fn place(&mut self, clip: Clip) {
        self.clear_range(clip.start, clip.end());
        self.clips.push(clip);
        self.sort();
    }

    fn remove(&mut self, id: Id) -> Option<Clip> {
        let i = self.clips.iter().position(|c| c.id == id)?;
        Some(self.clips.remove(i))
    }
}

impl Project {
    fn check_track(&self, track: usize, kind: TrackKind) -> EditResult<()> {
        let t = self.timeline.tracks.get(track).ok_or(EditError::TrackKindMismatch(kind))?;
        if t.kind != kind {
            return Err(EditError::TrackKindMismatch(kind));
        }
        if t.locked {
            return Err(EditError::TrackLocked(t.name.clone()));
        }
        Ok(())
    }

    /// First video and audio track, creating them if the timeline has none.
    pub fn default_tracks(&mut self) -> (usize, usize) {
        let mut first = |kind| {
            let existing = self.timeline.tracks_of(kind).next().map(|(i, _)| i);
            existing.unwrap_or_else(|| self.timeline.add_track(kind))
        };
        (first(TrackKind::Video), first(TrackKind::Audio))
    }

    /// Overwrites a media item onto the timeline at `at`. Picture goes to
    /// `video_track`, sound to `audio_track`; the two clips are linked.
    pub fn place_media(
        &mut self,
        media_id: Id,
        at: i64,
        video_track: Option<usize>,
        audio_track: Option<usize>,
    ) -> EditResult<Vec<Id>> {
        let media = self.media(media_id).ok_or(EditError::MediaNotFound)?.clone();
        let duration = self
            .media_frames(media_id)
            .unwrap_or_else(|| seconds_to_frames(STILL_SECONDS, self.settings.fps));
        let at = at.max(0);
        let want_video = media.info.video.is_some() && video_track.is_some();
        let want_audio = media.info.audio.is_some() && audio_track.is_some();
        if want_video {
            self.check_track(video_track.unwrap(), TrackKind::Video)?;
        }
        if want_audio {
            self.check_track(audio_track.unwrap(), TrackKind::Audio)?;
        }
        if !want_video && !want_audio {
            return Err(EditError::NoOp);
        }
        let link = (want_video && want_audio).then(new_id);
        let mut ids = Vec::new();
        for (wanted, track) in [(want_video, video_track), (want_audio, audio_track)] {
            if !wanted {
                continue;
            }
            let mut clip = Clip::new(media_id, &media.name, at, duration, 0);
            clip.link = link;
            ids.push(clip.id);
            self.timeline.tracks[track.unwrap()].place(clip);
        }
        Ok(ids)
    }

    /// Appends media after the last clip on the default tracks.
    pub fn append_media(&mut self, media_id: Id) -> EditResult<Vec<Id>> {
        let (v, a) = self.default_tracks();
        let end = |t: usize| self.timeline.tracks[t].clips.last().map_or(0, Clip::end);
        let at = end(v).max(end(a));
        self.place_media(media_id, at, Some(v), Some(a))
    }

    /// The clip plus every clip linked to it.
    pub fn linked_group(&self, id: Id) -> Vec<Id> {
        let Some(clip) = self.timeline.clip(id) else { return Vec::new() };
        match clip.link {
            None => vec![id],
            Some(link) => self
                .timeline
                .tracks
                .iter()
                .flat_map(|t| &t.clips)
                .filter(|c| c.link == Some(link))
                .map(|c| c.id)
                .collect(),
        }
    }

    fn expand_links(&self, ids: impl IntoIterator<Item = Id>) -> HashSet<Id> {
        ids.into_iter().flat_map(|id| self.linked_group(id)).collect()
    }

    /// Splits every clip under `frame` on unlocked tracks. When `only` is given,
    /// just those clips (and their linked partners) are cut. Returns the ids of
    /// the new right-hand clips.
    pub fn blade(&mut self, frame: i64, only: Option<&HashSet<Id>>) -> Vec<Id> {
        let filter = only.map(|s| self.expand_links(s.iter().copied()));
        let mut new_links: HashMap<Id, Id> = HashMap::new();
        let mut created = Vec::new();
        for track in self.timeline.tracks.iter_mut().filter(|t| !t.locked) {
            let mut right_parts = Vec::new();
            for c in track.clips.iter_mut() {
                if !(frame > c.start && frame < c.end()) {
                    continue;
                }
                if filter.as_ref().is_some_and(|f| !f.contains(&c.id)) {
                    continue;
                }
                let mut right = c.clone();
                right.id = new_id();
                right.link = c.link.map(|l| *new_links.entry(l).or_insert_with(new_id));
                right.source_in += frame - c.start;
                right.start = frame;
                right.duration = c.end() - frame;
                c.duration = frame - c.start;
                created.push(right.id);
                right_parts.push(right);
            }
            track.clips.extend(right_parts);
            track.sort();
        }
        created
    }

    /// Removes clips (and linked partners). With `ripple`, later clips on the
    /// same track shift left to close the gap.
    pub fn delete_clips(&mut self, ids: &HashSet<Id>, ripple: bool) -> EditResult<usize> {
        let targets = self.expand_links(ids.iter().copied());
        let mut removed = 0;
        for track in self.timeline.tracks.iter_mut() {
            let ranges: Vec<(i64, i64)> = track
                .clips
                .iter()
                .filter(|c| targets.contains(&c.id))
                .map(|c| (c.start, c.end()))
                .collect();
            if ranges.is_empty() {
                continue;
            }
            if track.locked {
                return Err(EditError::TrackLocked(track.name.clone()));
            }
            track.clips.retain(|c| !targets.contains(&c.id));
            removed += ranges.len();
            if ripple {
                for &(start, end) in ranges.iter().rev() {
                    for c in track.clips.iter_mut().filter(|c| c.start >= end) {
                        c.start -= end - start;
                    }
                }
            }
        }
        if removed == 0 { Err(EditError::ClipNotFound) } else { Ok(removed) }
    }

    /// Moves a clip (and its linked partners) by `delta` frames. The dragged
    /// clip may also change to `target_track`; partners keep their tracks.
    /// Material at the destination is overwritten.
    pub fn move_clip(&mut self, id: Id, delta: i64, target_track: Option<usize>) -> EditResult<()> {
        let (src_track, _) = self.timeline.find_clip(id).ok_or(EditError::ClipNotFound)?;
        let group = self.linked_group(id);
        let kind = self.timeline.tracks[src_track].kind;
        let dest = target_track.unwrap_or(src_track);
        self.check_track(dest, kind)?;
        // Never move anything before frame 0.
        let earliest = group.iter().filter_map(|g| self.timeline.clip(*g)).map(|c| c.start).min().unwrap_or(0);
        let delta = delta.max(-earliest);
        if delta == 0 && dest == src_track {
            return Err(EditError::NoOp);
        }
        let mut moved = Vec::new();
        for gid in &group {
            let (ti, _) = self.timeline.find_clip(*gid).ok_or(EditError::ClipNotFound)?;
            let track = &self.timeline.tracks[ti];
            if track.locked {
                return Err(EditError::TrackLocked(track.name.clone()));
            }
            moved.push((ti, *gid));
        }
        let mut lifted = Vec::new();
        for (ti, gid) in moved {
            let mut clip = self.timeline.tracks[ti].remove(gid).expect("found above");
            clip.start += delta;
            lifted.push((if gid == id { dest } else { ti }, clip));
        }
        for (ti, clip) in lifted {
            self.timeline.tracks[ti].place(clip);
        }
        Ok(())
    }

    /// Trims one edge of a clip and its linked partners by `delta` frames,
    /// limited by available media, neighbouring clips and a 1-frame minimum.
    /// Returns the delta actually applied.
    pub fn trim_clip(&mut self, id: Id, edge: Edge, delta: i64) -> EditResult<i64> {
        let group = self.linked_group(id);
        if group.is_empty() {
            return Err(EditError::ClipNotFound);
        }
        let (mut lo, mut hi) = (i64::MIN, i64::MAX);
        for gid in &group {
            let (ti, ci) = self.timeline.find_clip(*gid).ok_or(EditError::ClipNotFound)?;
            let track = &self.timeline.tracks[ti];
            if track.locked {
                return Err(EditError::TrackLocked(track.name.clone()));
            }
            let c = &track.clips[ci];
            let media_len = self.media_frames(c.media_id);
            match edge {
                Edge::Start => {
                    let prev_end = if ci > 0 { track.clips[ci - 1].end() } else { 0 };
                    lo = lo.max(prev_end - c.start);
                    if media_len.is_some() {
                        lo = lo.max(-c.source_in);
                    }
                    hi = hi.min(c.duration - 1);
                }
                Edge::End => {
                    let next_start = track.clips.get(ci + 1).map_or(i64::MAX, |n| n.start);
                    hi = hi.min(next_start.saturating_sub(c.end()));
                    if let Some(len) = media_len {
                        hi = hi.min(len - (c.source_in + c.duration));
                    }
                    lo = lo.max(1 - c.duration);
                }
            }
        }
        let applied = delta.clamp(lo.min(0), hi.max(0));
        if applied == 0 {
            return Ok(0);
        }
        for gid in &group {
            let c = self.timeline.clip_mut(*gid).expect("found above");
            match edge {
                Edge::Start => {
                    c.start += applied;
                    c.source_in += applied;
                    c.duration -= applied;
                }
                Edge::End => c.duration += applied,
            }
        }
        Ok(applied)
    }

    pub fn remove_media(&mut self, id: Id) -> EditResult<()> {
        if self.media_in_use(id) {
            return Err(EditError::MediaInUse);
        }
        let before = self.media.len();
        self.media.retain(|m| m.id != id);
        if self.media.len() == before { Err(EditError::MediaNotFound) } else { Ok(()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use crate::time::Rational;

    fn project_with_media(secs: f64) -> (Project, Id) {
        let mut p = Project::new("t", ProjectSettings { fps: Rational::new(10, 1), ..Default::default() });
        let m = MediaItem {
            id: new_id(),
            name: "clip".into(),
            path: "clip.mov".into(),
            sequence: None,
            info: MediaInfo {
                format_name: "mov".into(),
                duration_secs: secs,
                video: Some(VideoInfo {
                    codec: "prores".into(),
                    width: 1920,
                    height: 1080,
                    fps: Rational::new(10, 1),
                    pix_fmt: "yuv422p10le".into(),
                    has_alpha: false,
                    is_still: false,
                }),
                audio: Some(AudioInfo { codec: "pcm".into(), sample_rate: 48000, channels: 2 }),
            },
        };
        let id = m.id;
        p.media.push(m);
        (p, id)
    }

    fn spans(t: &Track) -> Vec<(i64, i64, i64)> {
        t.clips.iter().map(|c| (c.start, c.duration, c.source_in)).collect()
    }

    #[test]
    fn append_links_video_and_audio() {
        let (mut p, m) = project_with_media(10.0);
        let ids = p.append_media(m).unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(p.linked_group(ids[0]).len(), 2);
        p.append_media(m).unwrap();
        assert_eq!(spans(&p.timeline.tracks[0]), vec![(0, 100, 0), (100, 100, 0)]);
        assert_eq!(p.timeline.duration(), 200);
    }

    #[test]
    fn overwrite_splits_underlying_clip() {
        let (mut p, m) = project_with_media(10.0);
        p.append_media(m).unwrap();
        let mut short = Clip::new(m, "x", 30, 20, 0);
        short.link = None;
        p.timeline.tracks[0].place(short);
        assert_eq!(spans(&p.timeline.tracks[0]), vec![(0, 30, 0), (30, 20, 0), (50, 50, 50)]);
    }

    #[test]
    fn blade_cuts_linked_pair_and_relinks() {
        let (mut p, m) = project_with_media(10.0);
        p.append_media(m).unwrap();
        let created = p.blade(40, None);
        assert_eq!(created.len(), 2);
        assert_eq!(spans(&p.timeline.tracks[0]), vec![(0, 40, 0), (40, 60, 40)]);
        assert_eq!(p.linked_group(created[0]).len(), 2);
        let left = p.timeline.tracks[0].clips[0].id;
        assert!(!p.linked_group(left).contains(&created[0]));
    }

    #[test]
    fn ripple_delete_closes_gap() {
        let (mut p, m) = project_with_media(10.0);
        let first = p.append_media(m).unwrap();
        p.append_media(m).unwrap();
        p.delete_clips(&HashSet::from([first[0]]), true).unwrap();
        assert_eq!(spans(&p.timeline.tracks[0]), vec![(0, 100, 0)]);
        assert_eq!(spans(&p.timeline.tracks[1]), vec![(0, 100, 0)]);
    }

    #[test]
    fn move_clamps_at_zero_and_overwrites() {
        let (mut p, m) = project_with_media(10.0);
        let a = p.append_media(m).unwrap();
        let b = p.append_media(m).unwrap();
        p.move_clip(b[0], -50, None).unwrap();
        // b moved to 50 and overwrote the tail of a on both tracks.
        assert_eq!(spans(&p.timeline.tracks[0]), vec![(0, 50, 0), (50, 100, 0)]);
        assert_eq!(spans(&p.timeline.tracks[1]), vec![(0, 50, 0), (50, 100, 0)]);
        assert_eq!(p.timeline.clip(a[0]).unwrap().duration, 50);
        p.move_clip(a[0], -999, None).unwrap_err();
    }

    #[test]
    fn trim_respects_media_and_neighbours() {
        let (mut p, m) = project_with_media(10.0);
        let a = p.append_media(m).unwrap();
        // Can't extend past the end of media.
        assert_eq!(p.trim_clip(a[0], Edge::End, 50).unwrap(), 0);
        assert_eq!(p.trim_clip(a[0], Edge::End, -30).unwrap(), -30);
        assert_eq!(p.trim_clip(a[0], Edge::Start, 20).unwrap(), 20);
        assert_eq!(spans(&p.timeline.tracks[1]), vec![(20, 50, 20)]);
        // Can't pull the head before the media start.
        assert_eq!(p.trim_clip(a[0], Edge::Start, -100).unwrap(), -20);
        // Minimum one frame.
        assert_eq!(p.trim_clip(a[0], Edge::End, -1000).unwrap(), -69);
        assert_eq!(p.timeline.clip(a[0]).unwrap().duration, 1);
    }

    #[test]
    fn locked_track_rejects_edits() {
        let (mut p, m) = project_with_media(1.0);
        let a = p.append_media(m).unwrap();
        p.timeline.tracks[1].locked = true;
        assert!(matches!(p.move_clip(a[0], 5, None), Err(EditError::TrackLocked(_))));
        assert!(p.delete_clips(&HashSet::from([a[0]]), false).is_err());
        assert_eq!(p.remove_media(m), Err(EditError::MediaInUse));
    }
}
