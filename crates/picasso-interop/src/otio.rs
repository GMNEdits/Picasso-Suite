//! OpenTimelineIO export — an open interchange format that DaVinci Resolve,
//! Premiere, Avid and others import on every OS.

use picasso_core::{Project, TrackKind};
use serde_json::{Value, json};
use std::path::Path;

fn rt(value: i64, rate: f64) -> Value {
    json!({ "OTIO_SCHEMA": "RationalTime.1", "rate": rate, "value": value as f64 })
}

fn range(start: i64, duration: i64, rate: f64) -> Value {
    json!({ "OTIO_SCHEMA": "TimeRange.1", "start_time": rt(start, rate), "duration": rt(duration, rate) })
}

/// `file://` URL for a local path on any OS (`C:\a b\c.mov` → `file:///C:/a%20b/c.mov`).
pub fn file_url(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    let mut out = String::from("file://");
    if !s.starts_with('/') {
        out.push('/');
    }
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' | b':' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn to_json(project: &Project) -> Value {
    let fps = project.settings.fps.as_f64();
    let mut tracks = Vec::new();
    // OTIO stacks composite bottom-up like ours; audio tracks follow video.
    for kind in [TrackKind::Video, TrackKind::Audio] {
        for (_, track) in project.timeline.tracks_of(kind) {
            let mut children = Vec::new();
            let mut cursor = 0;
            for clip in &track.clips {
                if clip.start > cursor {
                    children.push(json!({ "OTIO_SCHEMA": "Gap.1", "name": "", "source_range": range(0, clip.start - cursor, fps) }));
                }
                let media = project.media(clip.media_id);
                let reference = match media {
                    Some(m) if m.sequence.is_none() => json!({
                        "OTIO_SCHEMA": "ExternalReference.1",
                        "target_url": file_url(&m.path),
                        "available_range": null,
                        "metadata": {},
                    }),
                    Some(m) => {
                        let seq = m.sequence.as_ref().unwrap();
                        let pattern = m.path.to_string_lossy().into_owned();
                        let (prefix, rest) = pattern.split_once('%').unwrap_or((&pattern, ""));
                        let digits: i64 = rest.trim_start_matches('0').split('d').next().and_then(|d| d.parse().ok()).unwrap_or(4);
                        let suffix = rest.split_once('d').map_or("", |(_, s)| s);
                        let (dir, name_prefix) = prefix.rsplit_once(['/', '\\']).unwrap_or(("", prefix));
                        json!({
                            "OTIO_SCHEMA": "ImageSequenceReference.1",
                            "target_url_base": format!("{}/", file_url(Path::new(dir))),
                            "name_prefix": name_prefix,
                            "name_suffix": suffix,
                            "start_frame": seq.start_number,
                            "frame_step": 1,
                            "rate": seq.fps.as_f64(),
                            "frame_zero_padding": digits,
                            "missing_frame_policy": "error",
                            "available_range": null,
                            "metadata": {},
                        })
                    }
                    None => json!({ "OTIO_SCHEMA": "MissingReference.1", "metadata": {} }),
                };
                let mut effects = Vec::new();
                if !clip.enabled {
                    effects.push(json!({ "OTIO_SCHEMA": "Effect.1", "name": "disabled", "effect_name": "Disabled", "metadata": {} }));
                }
                children.push(json!({
                    "OTIO_SCHEMA": "Clip.1",
                    "name": clip.name,
                    "source_range": range(clip.source_in, clip.duration, fps),
                    "media_reference": reference,
                    "effects": effects,
                    "markers": [],
                    "metadata": { "picasso": {
                        "clip_id": clip.id.to_string(),
                        "link": clip.link.map(|l| l.to_string()),
                        "volume": clip.volume,
                        "transform": { "zoom": clip.transform.zoom, "pan_x": clip.transform.pan_x,
                                       "pan_y": clip.transform.pan_y, "opacity": clip.transform.opacity },
                    }},
                }));
                cursor = clip.end();
            }
            tracks.push(json!({
                "OTIO_SCHEMA": "Track.1",
                "name": track.name,
                "kind": if kind == TrackKind::Video { "Video" } else { "Audio" },
                "children": children,
                "source_range": null,
                "effects": [],
                "markers": [],
                "metadata": { "picasso": { "enabled": track.enabled } },
            }));
        }
    }
    json!({
        "OTIO_SCHEMA": "Timeline.1",
        "name": project.timeline.name,
        "global_start_time": null,
        "metadata": { "picasso": {
            "width": project.settings.width,
            "height": project.settings.height,
            "fps": [project.settings.fps.num, project.settings.fps.den],
        }},
        "tracks": {
            "OTIO_SCHEMA": "Stack.1",
            "name": "tracks",
            "children": tracks,
            "source_range": null,
            "effects": [],
            "markers": [],
            "metadata": {},
        },
    })
}

pub fn write(project: &Project, path: &Path) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(&to_json(project)).map_err(std::io::Error::other)?;
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use picasso_core::*;

    #[test]
    fn urls_for_each_os() {
        assert_eq!(file_url(Path::new(r"C:\My Media\a.mov")), "file:///C:/My%20Media/a.mov");
        assert_eq!(file_url(Path::new("/Volumes/Media/a.mov")), "file:///Volumes/Media/a.mov");
    }

    #[test]
    fn gaps_and_clips() {
        let mut p = Project::default();
        let m = MediaItem {
            id: new_id(),
            name: "a.mov".into(),
            path: "/x/a.mov".into(),
            sequence: None,
            info: MediaInfo {
                format_name: "mov".into(),
                duration_secs: 10.0,
                video: Some(VideoInfo {
                    codec: "h264".into(), width: 1920, height: 1080, fps: Rational::new(25, 1),
                    pix_fmt: "yuv420p".into(), has_alpha: false, is_still: false,
                }),
                audio: None,
            },
        };
        let id = m.id;
        p.media.push(m);
        p.place_media(id, 50, Some(0), None).unwrap();
        let v = to_json(&p);
        let children = &v["tracks"]["children"][0]["children"];
        assert_eq!(children[0]["OTIO_SCHEMA"], "Gap.1");
        assert_eq!(children[0]["source_range"]["duration"]["value"], 50.0);
        assert_eq!(children[1]["media_reference"]["target_url"], "file:///x/a.mov");
        assert_eq!(v["tracks"]["children"][1]["kind"], "Audio");
    }
}
