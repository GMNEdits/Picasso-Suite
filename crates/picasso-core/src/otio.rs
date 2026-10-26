//! OpenTimelineIO export: the timeline as an `.otio` file (JSON), which DaVinci Resolve, Premiere
//! Pro, Avid, Nuke, Blender and others import. Written from the OTIO schema (Timeline.1, Stack.1,
//! Track.1, Clip.1 ...: the oldest versions, which every reader takes), with OTIO's own rules: the tracks of a stack go bottom to top, a gap is
//! an item of its own, and a clip's range is in the frames of its media.
//!
//! What OTIO has no field for (opacity, the transform, the volume) is kept under
//! `metadata.picasso`, which other programs leave alone.

use serde_json::{json, Value};

use crate::{Clip, MediaSource, Project, TrackKind, TransitionKind};

/// The timeline starts at this time of day (01:00:00:00), as on the timeline.
const START_SECONDS: u64 = 3600;

fn time(value: u64, rate: f64) -> Value {
    json!({ "OTIO_SCHEMA": "RationalTime.1", "rate": rate, "value": value as f64 })
}

fn range(start: u64, duration: u64, rate: f64) -> Value {
    json!({
        "OTIO_SCHEMA": "TimeRange.1",
        "start_time": time(start, rate),
        "duration": time(duration, rate),
    })
}

/// A `file://` address for a path on any system: `C:\a b\c.mov` is `file:///C:/a%20b/c.mov`.
pub fn file_url(path: &str) -> String {
    let path = path.replace('\\', "/");
    let mut out = String::from("file://");
    if !path.starts_with('/') {
        out.push('/');
    }
    for b in path.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' | b':' => {
                out.push(char::from(b));
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// OTIO's names for the eight marker colours (the last, fawn, is nearest to orange).
const MARKER_COLORS: [&str; 8] = [
    "BLUE", "CYAN", "GREEN", "YELLOW", "RED", "PINK", "PURPLE", "ORANGE",
];

/// What a clip shows: a file, a generated picture, or nothing OTIO can point to.
fn media_reference(project: &Project, clip: &Clip, rate: f64) -> Value {
    let Some(media) = project.media(clip.media) else {
        return json!({ "OTIO_SCHEMA": "MissingReference.1", "name": clip.name, "metadata": {} });
    };
    let available = media.length.map_or(Value::Null, |n| range(0, n, rate));
    let generator = |kind: &str, parameters: Value| {
        json!({
            "OTIO_SCHEMA": "GeneratorReference.1",
            "name": media.name,
            "generator_kind": kind,
            "parameters": parameters,
            "available_range": available,
            "metadata": {},
        })
    };
    match &media.source {
        MediaSource::Video { path } | MediaSource::Audio { path } | MediaSource::Image { path } => {
            json!({
                "OTIO_SCHEMA": "ExternalReference.1",
                "name": media.name,
                "target_url": file_url(path),
                "available_range": available,
                "metadata": {},
            })
        }
        MediaSource::Solid { rgb } => generator("SolidColor", json!({ "color": rgb })),
        MediaSource::Gradient { from, to } => {
            generator("Gradient", json!({ "from": from, "to": to }))
        }
        MediaSource::ColorBars => generator("SMPTEBars", json!({})),
        MediaSource::Title(title) => generator("Text", json!({ "text": title.text })),
        MediaSource::Compound { timeline } => json!({
            "OTIO_SCHEMA": "MissingReference.1",
            "name": media.name,
            "metadata": { "picasso": { "compound_timeline": timeline } },
        }),
    }
}

fn clip_json(project: &Project, clip: &Clip, rate: f64) -> Value {
    let mut effects = Vec::new();
    if clip.freeze {
        effects.push(json!({
            "OTIO_SCHEMA": "FreezeFrame.1",
            "name": "Freeze Frame",
            "effect_name": "FreezeFrame",
            "time_scalar": 0.0,
            "metadata": {},
        }));
    } else if (clip.speed - 1.0).abs() > 1e-4 || clip.reverse {
        let scalar = f64::from(clip.speed) * if clip.reverse { -1.0 } else { 1.0 };
        effects.push(json!({
            "OTIO_SCHEMA": "LinearTimeWarp.1",
            "name": "Speed",
            "effect_name": "LinearTimeWarp",
            "time_scalar": scalar,
            "metadata": {},
        }));
    }
    let t = &clip.transform;
    json!({
        "OTIO_SCHEMA": "Clip.1",
        "name": clip.name,
        "source_range": range(clip.source_in, clip.duration, rate),
        "media_reference": media_reference(project, clip, rate),
        "effects": effects,
        "markers": [],
        "enabled": clip.enabled,
        "metadata": { "picasso": {
            "opacity": clip.opacity,
            "gain_db": clip.audio.gain_db,
            "transform": {
                "x": t.x, "y": t.y, "scale_x": t.scale_x, "scale_y": t.scale_y,
                "rotation": t.rotation,
            },
        }},
    })
}

/// The timeline of `project` as an OTIO file.
pub fn to_otio(project: &Project) -> String {
    let rate = project.fps.as_f64();
    let mut tracks = Vec::new();
    // Picture tracks bottom (V1) first, then the sound tracks, as OTIO stacks them.
    for kind in [TrackKind::Video, TrackKind::Audio] {
        for track in project.tracks.iter().filter(|t| t.kind == kind) {
            let mut clips: Vec<&Clip> = track.clips.iter().collect();
            clips.sort_by_key(|c| c.start);
            let mut children = Vec::new();
            let mut cursor = 0;
            for (i, clip) in clips.iter().enumerate() {
                // A transition into this clip sits between it and the one before, half over each.
                let left = i.checked_sub(1).and_then(|j| clips.get(j));
                let frames = left.and_then(|l| crate::project::transition_frames(l, clip));
                if clip.start > cursor {
                    children.push(json!({
                        "OTIO_SCHEMA": "Gap.1",
                        "name": "",
                        "source_range": range(0, clip.start - cursor, rate),
                        "effects": [], "markers": [], "enabled": true, "metadata": {},
                    }));
                }
                if let (Some(frames), Some(t)) = (frames, clip.transition) {
                    children.push(json!({
                        "OTIO_SCHEMA": "Transition.1",
                        "name": match t.kind {
                            TransitionKind::CrossDissolve => "Cross Dissolve",
                            TransitionKind::DipToBlack => "Dip to Black",
                        },
                        "transition_type": "SMPTE_Dissolve",
                        "in_offset": time(frames / 2, rate),
                        "out_offset": time(frames - frames / 2, rate),
                        "metadata": { "picasso": { "kind": format!("{:?}", t.kind) } },
                    }));
                }
                children.push(clip_json(project, clip, rate));
                cursor = cursor.max(clip.end());
            }
            tracks.push(json!({
                "OTIO_SCHEMA": "Track.1",
                "name": track.name,
                "kind": if kind == TrackKind::Video { "Video" } else { "Audio" },
                "children": children,
                "source_range": null,
                "effects": [],
                "markers": [],
                "enabled": !track.muted,
                "metadata": { "picasso": { "locked": track.locked, "gain_db": track.gain_db } },
            }));
        }
    }
    let markers: Vec<Value> = project
        .markers
        .iter()
        .map(|m| {
            json!({
                "OTIO_SCHEMA": "Marker.2",
                "name": m.name,
                "color": MARKER_COLORS[usize::from(m.color) % MARKER_COLORS.len()],
                "marked_range": range(m.frame, 0, rate),
                "comment": "",
                "metadata": {},
            })
        })
        .collect();
    let timeline = json!({
        "OTIO_SCHEMA": "Timeline.1",
        "name": project.timeline_name,
        "global_start_time": time(START_SECONDS * project.fps.nominal(), rate),
        "metadata": { "picasso": {
            "project": project.name,
            "width": project.width,
            "height": project.height,
            "fps": [project.fps.num, project.fps.den],
        }},
        "tracks": {
            "OTIO_SCHEMA": "Stack.1",
            "name": "tracks",
            "children": tracks,
            "source_range": null,
            "effects": [],
            "markers": markers,
            "enabled": true,
            "metadata": {},
        },
    });
    serde_json::to_string_pretty(&timeline).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Outcome, Transition};

    fn project() -> Project {
        let mut p = Project::new("otio test");
        let v1 = p.tracks[0].id;
        let add = |p: &mut Project, name: &str, path: &str, start: u64| {
            let Ok(Outcome::Media(m)) = (Command::AddMedia {
                name: name.into(),
                source: MediaSource::Video { path: path.into() },
                length: Some(240),
                default_duration: 48,
            })
            .apply(p) else {
                panic!("media");
            };
            Command::AddClip {
                track: v1,
                media: m,
                start,
                duration: Some(48),
                with_sound: false,
            }
            .apply(p)
            .unwrap_or_else(|e| panic!("{e}"));
        };
        add(&mut p, "a.mov", r"C:\Media\a b.mov", 0);
        add(&mut p, "b.mov", "/home/me/b.mov", 72);
        p
    }

    #[test]
    fn the_timeline_is_tracks_of_clips_and_gaps() {
        let mut p = project();
        p.markers.push(crate::Marker {
            frame: 10,
            name: "Here".into(),
            color: 4,
        });
        let text = to_otio(&p);
        let v: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(v["OTIO_SCHEMA"], "Timeline.1");
        let stack = &v["tracks"];
        assert_eq!(stack["markers"][0]["color"], "RED");
        let v1 = &stack["children"][0];
        assert_eq!(v1["kind"], "Video");
        let kinds: Vec<&str> = v1["children"]
            .as_array()
            .map(|a| a.iter().filter_map(|c| c["OTIO_SCHEMA"].as_str()).collect())
            .unwrap_or_default();
        assert_eq!(kinds, ["Clip.1", "Gap.1", "Clip.1"]);
        let gap = &v1["children"][1]["source_range"]["duration"]["value"];
        assert_eq!(gap.as_f64(), Some(24.0), "48 to 72");
        let clip = &v1["children"][0];
        assert_eq!(
            clip["media_reference"]["target_url"],
            "file:///C:/Media/a%20b.mov"
        );
        assert_eq!(
            clip["media_reference"]["available_range"]["duration"]["value"].as_f64(),
            Some(240.0)
        );
        // The timeline starts at 01:00:00:00, as Resolve's do.
        assert_eq!(
            v["global_start_time"]["value"].as_f64(),
            Some(3600.0 * p.fps.as_f64().round())
        );
    }

    #[test]
    fn transitions_and_speed_changes_are_written() {
        let mut p = project();
        let track = &mut p.tracks[0];
        let second = &mut track.clips[1];
        second.start = 48;
        second.transition = Some(Transition {
            kind: TransitionKind::CrossDissolve,
            frames: 12,
        });
        second.speed = 2.0;
        let v: Value = serde_json::from_str(&to_otio(&p)).unwrap_or_else(|e| panic!("{e}"));
        let children = &v["tracks"]["children"][0]["children"];
        assert_eq!(children[1]["OTIO_SCHEMA"], "Transition.1");
        assert_eq!(children[1]["in_offset"]["value"].as_f64(), Some(6.0));
        assert_eq!(children[2]["effects"][0]["time_scalar"].as_f64(), Some(2.0));
    }

    #[test]
    fn file_addresses_escape_what_a_url_cannot_hold() {
        assert_eq!(file_url("/a/b c#1.mov"), "file:///a/b%20c%231.mov");
        assert_eq!(file_url(r"D:\x\é.mp4"), "file:///D:/x/%C3%A9.mp4");
    }
}
