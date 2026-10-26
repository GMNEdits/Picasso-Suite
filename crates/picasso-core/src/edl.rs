//! CMX 3600 edit decision lists: the timeline written as events (one per clip, cuts and
//! dissolves), and read back as clips placed by the names of the media they use.
//!
//! An event line is `NNN  REEL     TRACK  TRANS  SRC_IN SRC_OUT REC_IN REC_OUT`; the clip's
//! name follows in a `* FROM CLIP NAME:` comment, which is how the clips are matched to the
//! media pool when a list is read. Record times start at 01:00:00:00, like the timeline.

use crate::{Fps, Project, TrackKind};

/// The timeline starts at this time of day (01:00:00:00).
const START_SECONDS: u64 = 3600;

/// The most events read from one list.
pub const MAX_EVENTS: usize = 10_000;

/// Writes the timeline as an EDL: the video of the lowest video track, then the sound of the
/// first audio track, each clip one event.
pub fn to_edl(project: &Project, title: &str) -> String {
    let fps = project.fps;
    let start = START_SECONDS * fps.nominal();
    let mut out = format!(
        "TITLE: {}\nFCM: NON-DROP FRAME\n\n",
        title.lines().next().unwrap_or("Timeline")
    );
    let mut n = 0;
    for (kind, channel) in [(TrackKind::Video, "V"), (TrackKind::Audio, "A")] {
        let Some(track) = project.tracks.iter().find(|t| t.kind == kind) else {
            continue;
        };
        for (i, clip) in track.clips.iter().enumerate() {
            let name = project
                .media(clip.media)
                .map_or_else(|| clip.name.clone(), |m| m.name.clone());
            let source_out = clip.source_in + clip.source_span();
            let record = (start + clip.start, start + clip.end());
            // A dissolve into this clip is written as the event that takes it.
            let dissolve = clip.transition.filter(|_| i > 0).map(|t| t.frames);
            n += 1;
            let trans = match dissolve {
                Some(frames) => format!("D    {frames:03}"),
                None => "C       ".to_string(),
            };
            out.push_str(&format!(
                "{n:03}  AX       {channel:<5} {trans} {} {} {} {}\n",
                fps.timecode(clip.source_in),
                fps.timecode(source_out),
                fps.timecode(record.0),
                fps.timecode(record.1),
            ));
            if (clip.speed - 1.0).abs() > 1e-4 {
                let source_fps = f64::from(fps.nominal() as u32) * f64::from(clip.speed);
                out.push_str(&format!(
                    "M2   AX       {source_fps:05.1}                {}\n",
                    fps.timecode(clip.source_in)
                ));
            }
            out.push_str(&format!("* FROM CLIP NAME: {name}\n\n"));
        }
    }
    out
}

/// One event read from an EDL: the clip of media `name` from `source_in` for `duration`
/// frames, at `record` frames from the start of the timeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EdlEvent {
    pub name: String,
    pub audio: bool,
    pub source_in: u64,
    pub record: u64,
    pub duration: u64,
}

/// Frames of a `HH:MM:SS:FF` (or `;` drop-frame) timecode.
fn frames(fps: Fps, tc: &str) -> Option<u64> {
    let parts: Vec<u64> = tc
        .split([':', ';', '.'])
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    let [h, m, s, f] = parts[..] else {
        return None;
    };
    let n = fps.nominal();
    (m < 60 && s < 60 && f < n.max(1)).then(|| ((h * 60 + m) * 60 + s) * n + f)
}

/// Reads the events of an EDL. Record times are made relative to the first event's, so a list
/// that starts at 01:00:00:00 (or anywhere) lands at the start of the timeline. Lines that are
/// not events are skipped; a list without events is an error.
pub fn parse_edl(text: &str, fps: Fps) -> Result<Vec<EdlEvent>, String> {
    let mut events: Vec<EdlEvent> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(name) = line
            .strip_prefix("* FROM CLIP NAME:")
            .or_else(|| line.strip_prefix("*FROM CLIP NAME:"))
        {
            if let Some(last) = events.last_mut() {
                last.name = name.trim().to_string();
            }
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.len() < 8 || !words[0].chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        // The four times are the last four words (a dissolve adds its length before them).
        let times: Vec<u64> = words[words.len() - 4..]
            .iter()
            .map(|t| frames(fps, t))
            .collect::<Option<_>>()
            .ok_or_else(|| format!("a bad timecode in: {line}"))?;
        let (src_in, src_out, rec_in, rec_out) = (times[0], times[1], times[2], times[3]);
        if rec_out <= rec_in || src_out < src_in {
            continue;
        }
        let channel = words[2].to_ascii_uppercase();
        if channel == "NONE" {
            continue;
        }
        if events.len() >= MAX_EVENTS {
            return Err("the list has too many events".into());
        }
        events.push(EdlEvent {
            name: words[1].to_string(),
            audio: channel.starts_with('A') && !channel.contains('V'),
            source_in: src_in,
            record: rec_in,
            duration: rec_out - rec_in,
        });
    }
    let first = events
        .iter()
        .map(|e| e.record)
        .min()
        .ok_or("there are no events in the list")?;
    for e in &mut events {
        e.record -= first;
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Command;

    #[test]
    fn a_timeline_round_trips_through_an_edl() {
        let mut p = Project::demo();
        let a = p.tracks[0].clips[1].id;
        Command::SetClipSpeed {
            clip: a,
            speed: 2.0,
        }
        .apply(&mut p)
        .unwrap();
        let text = to_edl(&p, "Timeline 1");
        assert!(text.starts_with("TITLE: Timeline 1\nFCM: NON-DROP FRAME"));
        assert!(
            text.contains("001  AX       V     C        00:00:00:00"),
            "{text}"
        );
        assert!(text.contains("M2   AX"), "{text}");
        let events = parse_edl(&text, p.fps).unwrap();
        let clips = &p.tracks[0].clips;
        assert_eq!(events.len(), clips.len());
        for (e, c) in events.iter().zip(clips) {
            assert_eq!(
                (e.record, e.duration, e.source_in),
                (c.start, c.duration, c.source_in)
            );
            assert_eq!(Some(&e.name), p.media(c.media).map(|m| &m.name));
            assert!(!e.audio);
        }
    }

    #[test]
    fn lists_from_elsewhere_are_read() {
        let text = "TITLE: X\n\n\
            001  REEL1    V     C        00:00:10:00 00:00:12:00 10:00:00:00 10:00:02:00\n\
            * FROM CLIP NAME: shot.mov\n\
            002  REEL2    A     C        00:00:00:00 00:00:01:00 10:00:02:00 10:00:03:00\n\
            003  BL       V     D    024 00:00:00:00 00:00:01:00 10:00:02:00 10:00:03:00\n\
            garbage line\n";
        let fps = Fps { num: 24, den: 1 };
        let events = parse_edl(text, fps).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(
            events[0],
            EdlEvent {
                name: "shot.mov".into(),
                audio: false,
                source_in: 240,
                record: 0,
                duration: 48,
            }
        );
        assert!(events[1].audio);
        assert_eq!(events[1].name, "REEL2");
        assert_eq!(events[2].record, 48);
        assert!(parse_edl("nothing here", fps).is_err());
        assert!(parse_edl("001 A V C 1 2 3 4", fps).is_err());
    }
}
