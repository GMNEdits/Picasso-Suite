//! Final Cut Pro 7 XML (`xmeml`, version 5): the timeline as a sequence that Final Cut Pro 7,
//! Premiere Pro, DaVinci Resolve, Avid Media Composer (through its importers) and others read.
//! Premiere's "Final Cut Pro XML" import and export use the same format, so one writer serves
//! both presets. Written from Apple's published description of the interchange format (the
//! `xmeml` document type: `sequence`, `media`, `track`, `clipitem`, `file`, `transitionitem`).
//!
//! The sequence keeps the timeline's tracks, clips (by the files they come from), gaps, speed
//! changes, reverse, freeze frames, dissolves and markers. A clip of generated media (a title,
//! colour bars, a solid or a gradient) or a compound clip has no file to point to, so it is left
//! out; [`Written::skipped`] counts them. Grades, effects and keyframes stay in Picasso Suite.

use crate::{Clip, MediaSource, Project, TrackKind, TransitionKind};

/// The timeline starts at this time of day (01:00:00:00), as on the timeline.
const START_SECONDS: u64 = 3600;

/// The document, and how many clips it could not carry.
pub struct Written {
    pub text: String,
    pub skipped: usize,
}

/// Escapes the five characters XML gives a meaning to.
pub(crate) fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // Control characters other than tab and new line are not allowed in XML 1.0.
            c if (c as u32) < 0x20 && c != '\t' && c != '\n' => {}
            c => out.push(c),
        }
    }
    out
}

/// The `<rate>` of a frame rate: the whole frames of a second, and whether it is an NTSC rate
/// (29.97 is 30 with `ntsc` TRUE).
fn rate(project: &Project) -> String {
    let ntsc = project.fps.den == 1001;
    format!(
        "<rate><timebase>{}</timebase><ntsc>{}</ntsc></rate>",
        project.fps.nominal(),
        if ntsc { "TRUE" } else { "FALSE" }
    )
}

/// The `file://localhost/` address Final Cut Pro and Premiere both read.
fn path_url(path: &str) -> String {
    let url = crate::otio::file_url(path);
    match url.strip_prefix("file://") {
        Some(rest) => format!("file://localhost{rest}"),
        None => url,
    }
}

/// The path of a clip's media, when it has a file.
fn file_path<'a>(project: &'a Project, clip: &Clip) -> Option<&'a str> {
    match &project.media(clip.media)?.source {
        MediaSource::Video { path } | MediaSource::Audio { path } | MediaSource::Image { path } => {
            Some(path)
        }
        _ => None,
    }
}

/// Writes the files a sequence uses: the first clip of a file describes it in full, later ones
/// only name it, as the format asks.
struct Files {
    seen: Vec<u64>,
}

impl Files {
    fn element(&mut self, project: &Project, clip: &Clip, path: &str) -> String {
        let id = clip.media.0;
        if self.seen.contains(&id) {
            return format!("<file id=\"file-{id}\"/>");
        }
        self.seen.push(id);
        let media = project.media(clip.media);
        let name = media.map_or(clip.name.as_str(), |m| m.name.as_str());
        let file_name = path.rsplit(['/', '\\']).next().unwrap_or(name);
        let length = media
            .and_then(|m| m.length)
            .unwrap_or(clip.source_in + clip.source_span());
        let (video, audio) = match media.map(|m| &m.source) {
            Some(MediaSource::Audio { .. }) => (false, true),
            Some(MediaSource::Image { .. }) => (true, false),
            _ => (true, true),
        };
        let mut kinds = String::new();
        if video {
            kinds.push_str(&format!(
                "<video><samplecharacteristics>{}<width>{}</width><height>{}</height></samplecharacteristics></video>",
                rate(project),
                project.width,
                project.height
            ));
        }
        if audio {
            kinds.push_str(
                "<audio><samplecharacteristics><depth>16</depth><samplerate>48000</samplerate></samplecharacteristics><channelcount>2</channelcount></audio>",
            );
        }
        format!(
            "<file id=\"file-{id}\"><name>{}</name><pathurl>{}</pathurl>{}<duration>{length}</duration><timecode>{}<string>00:00:00:00</string><frame>0</frame><displayformat>NDF</displayformat></timecode><media>{kinds}</media></file>",
            escape(file_name),
            escape(&path_url(path)),
            rate(project),
            rate(project),
        )
    }
}

/// The Time Remap filter that carries a clip's speed, direction or freeze.
fn speed_filter(clip: &Clip) -> Option<String> {
    let speed = if clip.freeze {
        0.0
    } else {
        f64::from(clip.speed) * 100.0
    };
    if !clip.freeze && !clip.reverse && (speed - 100.0).abs() < 0.01 {
        return None;
    }
    Some(format!(
        "<filter><effect><name>Time Remap</name><effectid>timeremap</effectid><effectcategory>motion</effectcategory><effecttype>motion</effecttype><mediatype>video</mediatype>\
<parameter><parameterid>variablespeed</parameterid><name>variablespeed</name><value>0</value></parameter>\
<parameter><parameterid>speed</parameterid><name>speed</name><value>{speed:.4}</value></parameter>\
<parameter><parameterid>reverse</parameterid><name>reverse</name><value>{}</value></parameter>\
<parameter><parameterid>frameblending</parameterid><name>frameblending</name><value>FALSE</value></parameter>\
</effect></filter>",
        if clip.reverse { "TRUE" } else { "FALSE" }
    ))
}

/// A dissolve centred on the cut at `cut`, `frames` long.
fn transition_item(
    project: &Project,
    kind: TransitionKind,
    cut: u64,
    frames: u64,
    video: bool,
) -> String {
    let start = cut - frames / 2;
    let end = cut + (frames - frames / 2);
    let (name, id, category, media) = match (video, kind) {
        (false, _) => ("Cross Fade (+3dB)", "KGAudioTransCrossFade3dB", "", "audio"),
        (true, TransitionKind::CrossDissolve) => {
            ("Cross Dissolve", "Cross Dissolve", "Dissolve", "video")
        }
        (true, TransitionKind::DipToBlack) => (
            "Dip to Color Dissolve",
            "Dip to Color Dissolve",
            "Dissolve",
            "video",
        ),
    };
    format!(
        "<transitionitem>{}<start>{start}</start><end>{end}</end><alignment>center</alignment><effect><name>{name}</name><effectid>{id}</effectid><effectcategory>{category}</effectcategory><effecttype>transition</effecttype><mediatype>{media}</mediatype></effect></transitionitem>",
        rate(project)
    )
}

/// One track of the sequence.
fn track(project: &Project, t: &crate::Track, files: &mut Files, skipped: &mut usize) -> String {
    let video = t.kind == TrackKind::Video;
    let mut clips: Vec<&Clip> = t.clips.iter().collect();
    clips.sort_by_key(|c| c.start);
    let mut out = String::from("<track>");
    for (i, clip) in clips.iter().enumerate() {
        let Some(path) = file_path(project, clip) else {
            *skipped += 1;
            continue;
        };
        let left = i.checked_sub(1).and_then(|j| clips.get(j));
        let frames = left.and_then(|l| crate::project::transition_frames(l, clip));
        if let (Some(frames), Some(t)) = (frames, clip.transition) {
            out.push_str(&transition_item(project, t.kind, clip.start, frames, video));
        }
        let length = project
            .media(clip.media)
            .and_then(|m| m.length)
            .unwrap_or(clip.source_in + clip.source_span());
        let source_out = clip.source_in + clip.source_span();
        out.push_str(&format!(
            "<clipitem id=\"clipitem-{}\"><name>{}</name><enabled>{}</enabled><duration>{length}</duration>{}<start>{}</start><end>{}</end><in>{}</in><out>{source_out}</out>",
            clip.id.0,
            escape(&clip.name),
            if clip.enabled { "TRUE" } else { "FALSE" },
            rate(project),
            clip.start,
            clip.end(),
            clip.source_in,
        ));
        out.push_str(&files.element(project, clip, path));
        if !video {
            out.push_str(
                "<sourcetrack><mediatype>audio</mediatype><trackindex>1</trackindex></sourcetrack>",
            );
        }
        if video {
            if let Some(filter) = speed_filter(clip) {
                out.push_str(&filter);
            }
        }
        out.push_str("</clipitem>");
    }
    out.push_str(&format!(
        "<enabled>{}</enabled><locked>{}</locked></track>",
        if t.muted || (video && !t.picture) {
            "FALSE"
        } else {
            "TRUE"
        },
        if t.locked { "TRUE" } else { "FALSE" }
    ));
    out
}

/// The timeline of `project` as a Final Cut Pro 7 / Premiere XML document.
pub fn to_xmeml(project: &Project) -> Written {
    let mut files = Files { seen: Vec::new() };
    let mut skipped = 0;
    let start = START_SECONDS * project.fps.nominal();
    let mut video = String::new();
    let mut audio = String::new();
    for t in &project.tracks {
        match t.kind {
            TrackKind::Video => video.push_str(&track(project, t, &mut files, &mut skipped)),
            TrackKind::Audio => audio.push_str(&track(project, t, &mut files, &mut skipped)),
        }
    }
    let markers: String = project
        .markers
        .iter()
        .map(|m| {
            format!(
                "<marker><name>{}</name><comment></comment><in>{}</in><out>-1</out></marker>",
                escape(&m.name),
                m.frame
            )
        })
        .collect();
    let text = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE xmeml>\n<xmeml version=\"5\"><sequence id=\"sequence-1\"><name>{}</name><duration>{}</duration>{}<timecode>{}<string>{}</string><frame>{start}</frame><displayformat>NDF</displayformat></timecode><media><video><format><samplecharacteristics>{}<width>{}</width><height>{}</height><pixelaspectratio>square</pixelaspectratio></samplecharacteristics></format>{video}</video><audio><numOutputChannels>2</numOutputChannels><format><samplecharacteristics><depth>16</depth><samplerate>48000</samplerate></samplecharacteristics></format>{audio}</audio></media>{markers}</sequence></xmeml>\n",
        escape(&project.timeline_name),
        project.duration(),
        rate(project),
        rate(project),
        project.fps.timecode(start),
        rate(project),
        project.width,
        project.height,
    );
    Written { text, skipped }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Outcome, Transition};

    fn project() -> Project {
        let mut p = Project::new("xml test");
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
        add(&mut p, "a & b", r"C:\Media\a b.mov", 0);
        add(&mut p, "c", "/home/me/c.mov", 48);
        p
    }

    #[test]
    fn clips_files_and_dissolves_are_written() {
        let mut p = project();
        p.tracks[0].clips[1].transition = Some(Transition {
            kind: TransitionKind::CrossDissolve,
            frames: 12,
        });
        p.tracks[0].clips[1].speed = 2.0;
        p.markers.push(crate::Marker {
            frame: 5,
            name: "Here".into(),
            color: 0,
        });
        let w = to_xmeml(&p);
        assert_eq!(w.skipped, 0);
        let t = &w.text;
        assert!(t.starts_with("<?xml version=\"1.0\""), "{t}");
        assert!(t.contains("<xmeml version=\"5\">"));
        assert!(t.contains("<name>a &amp; b</name>"));
        assert!(
            t.contains("<pathurl>file://localhost/C:/Media/a%20b.mov</pathurl>"),
            "{t}"
        );
        assert!(t.contains("<pathurl>file://localhost/home/me/c.mov</pathurl>"));
        assert!(
            t.contains("<start>42</start><end>54</end><alignment>center</alignment>"),
            "{t}"
        );
        assert!(t.contains("<value>200.0000</value>"));
        assert!(t.contains("<marker><name>Here</name><comment></comment><in>5</in>"));
        assert!(t.contains("<timebase>24</timebase><ntsc>FALSE</ntsc>"));
        assert!(t.contains("<string>01:00:00:00</string><frame>86400</frame>"));
        assert_well_formed(t);
    }

    /// Every element that opens is closed, in order.
    fn assert_well_formed(text: &str) {
        let mut open: Vec<&str> = Vec::new();
        let body = text.split_once("<xmeml").map_or(text, |(_, b)| b);
        let doc = format!("<xmeml{body}");
        for piece in doc.split('<').skip(1) {
            let tag = piece.split_once('>').map_or(piece, |(t, _)| t);
            if let Some(name) = tag.strip_prefix('/') {
                assert_eq!(open.pop(), Some(name), "closing {name}");
            } else if !tag.ends_with('/') {
                open.push(tag.split(' ').next().unwrap_or(tag));
            }
        }
        assert!(open.is_empty(), "left open: {open:?}");
    }

    #[test]
    fn a_file_is_described_once_and_generated_media_is_skipped() {
        let mut p = project();
        let media = p.tracks[0].clips[0].media;
        let v1 = p.tracks[0].id;
        Command::AddClip {
            track: v1,
            media,
            start: 200,
            duration: Some(10),
            with_sound: false,
        }
        .apply(&mut p)
        .unwrap();
        let w = to_xmeml(&p);
        assert_well_formed(&w.text);
        assert_eq!(
            w.text
                .matches(&format!("<file id=\"file-{}\">", media.0))
                .count(),
            1
        );
        assert_eq!(
            w.text
                .matches(&format!("<file id=\"file-{}\"/>", media.0))
                .count(),
            1
        );
        let generated = to_xmeml(&Project::demo());
        assert_eq!(generated.skipped, 3);
        assert!(!generated.text.contains("<clipitem"));
    }
}
