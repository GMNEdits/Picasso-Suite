//! Subtitles exported next to a render: one cue for each title clip on the timeline, written as
//! SRT, WebVTT or IMSC1 (a TTML profile, as XML).

// The browser build renders nothing, so it writes no subtitles.
#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use picasso_core::{Fps, MediaSource, Project, TrackKind};

/// The kinds of subtitle file, in the order of the Deliver page's list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SubtitleKind {
    Srt,
    WebVtt,
    Imsc1,
}

impl SubtitleKind {
    pub(crate) const ALL: [Self; 3] = [Self::Srt, Self::WebVtt, Self::Imsc1];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Srt => "SRT",
            Self::WebVtt => "WebVTT",
            Self::Imsc1 => "IMSC1",
        }
    }

    pub(crate) fn extension(self) -> &'static str {
        match self {
            Self::Srt => "srt",
            Self::WebVtt => "vtt",
            Self::Imsc1 => "ttml",
        }
    }
}

/// One subtitle: from frame `start` to frame `end` of the render (0 is its first frame).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Cue {
    pub start: u64,
    pub end: u64,
    pub text: String,
}

/// The cues of the title clips between timeline frames `start` and `end`, in time order: the
/// enabled title clips of unmuted video tracks, cut to the range.
pub(crate) fn cues(project: &Project, start: u64, end: u64) -> Vec<Cue> {
    let mut cues: Vec<Cue> = project
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Video && !t.muted)
        .flat_map(|t| t.clips.iter())
        .filter(|c| c.enabled)
        .filter_map(|c| {
            let MediaSource::Title(title) = &project.media(c.media)?.source else {
                return None;
            };
            let (a, b) = (c.start.max(start), c.end().min(end));
            let text = title.text.trim();
            (a < b && !text.is_empty()).then(|| Cue {
                start: a - start,
                end: b - start,
                text: text.to_string(),
            })
        })
        .collect();
    cues.sort_by_key(|c| (c.start, c.end));
    cues
}

/// Milliseconds from the start of the render to frame `frame`.
fn millis(fps: Fps, frame: u64) -> u64 {
    let ms = u128::from(frame) * 1000 * u128::from(fps.den) / u128::from(fps.num.max(1));
    u64::try_from(ms).unwrap_or(u64::MAX)
}

fn clock(ms: u64, separator: char) -> String {
    format!(
        "{:02}:{:02}:{:02}{separator}{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The most cues read from one file.
pub(crate) const MAX_CUES: usize = 20_000;

/// Milliseconds of a subtitle time: `HH:MM:SS,mmm` (SRT), `HH:MM:SS.mmm` or `MM:SS.mmm`
/// (WebVTT).
fn parse_clock(text: &str) -> Option<u64> {
    let (clock, ms) = text.trim().split_once([',', '.'])?;
    let ms: u64 = ms.get(..3.min(ms.len()))?.parse().ok()?;
    let parts: Vec<u64> = clock
        .split(':')
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    let (h, m, s) = match parts[..] {
        [h, m, s] => (h, m, s),
        [m, s] => (0, m, s),
        _ => return None,
    };
    (m < 60 && s < 60).then(|| ((h * 60 + m) * 60 + s) * 1000 + ms)
}

/// The cues of an SRT or WebVTT file, in frames at `fps` (numbers, styles and notes are
/// skipped; the text keeps its line breaks, without tags like `<i>`).
pub(crate) fn parse(text: &str, fps: Fps) -> Vec<Cue> {
    let to_frame = |ms: u64| {
        let f = u128::from(ms) * u128::from(fps.num) / (1000 * u128::from(fps.den.max(1)));
        u64::try_from(f).unwrap_or(u64::MAX)
    };
    let strip_tags = |line: &str| {
        let mut out = String::new();
        let mut in_tag = false;
        for c in line.chars() {
            match c {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => out.push(c),
                _ => {}
            }
        }
        out
    };
    let mut cues = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let Some((a, b)) = line.split_once("-->") else {
            continue;
        };
        // WebVTT puts settings after the end time.
        let b = b.split_whitespace().next().unwrap_or("");
        let (Some(a), Some(b)) = (parse_clock(a), parse_clock(b)) else {
            continue;
        };
        let mut words = Vec::new();
        while let Some(next) = lines.peek() {
            if next.trim().is_empty() {
                break;
            }
            words.push(strip_tags(next.trim()));
            lines.next();
        }
        let (start, end) = (to_frame(a), to_frame(b));
        let text = words.join("\n");
        if end > start && !text.trim().is_empty() {
            cues.push(Cue { start, end, text });
        }
        if cues.len() >= MAX_CUES {
            break;
        }
    }
    cues
}

/// The subtitle file for `cues`, timed at `fps`.
pub(crate) fn write(kind: SubtitleKind, cues: &[Cue], fps: Fps) -> String {
    let mut out = String::new();
    match kind {
        SubtitleKind::Srt => {
            for (n, c) in cues.iter().enumerate() {
                out.push_str(&format!(
                    "{}\n{} --> {}\n{}\n\n",
                    n + 1,
                    clock(millis(fps, c.start), ','),
                    clock(millis(fps, c.end), ','),
                    c.text
                ));
            }
        }
        SubtitleKind::WebVtt => {
            out.push_str("WEBVTT\n\n");
            for c in cues {
                // A cue may not hold an empty line or "-->".
                let text = c.text.replace("-->", "->").replace("\n\n", "\n");
                out.push_str(&format!(
                    "{} --> {}\n{}\n\n",
                    clock(millis(fps, c.start), '.'),
                    clock(millis(fps, c.end), '.'),
                    text
                ));
            }
        }
        SubtitleKind::Imsc1 => {
            out.push_str(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                 <tt xmlns=\"http://www.w3.org/ns/ttml\" \
                 xmlns:ttp=\"http://www.w3.org/ns/ttml#parameter\" \
                 ttp:profile=\"http://www.w3.org/ns/ttml/profile/imsc1/text\" xml:lang=\"\">\n\
                 <body><div>\n",
            );
            for c in cues {
                let lines: Vec<String> = c.text.lines().map(xml_escape).collect();
                out.push_str(&format!(
                    "<p begin=\"{}\" end=\"{}\">{}</p>\n",
                    clock(millis(fps, c.start), '.'),
                    clock(millis(fps, c.end), '.'),
                    lines.join("<br/>")
                ));
            }
            out.push_str("</div></body>\n</tt>\n");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use picasso_core::{Command, Outcome, TitleText};

    fn project() -> Project {
        let mut p = Project::new("t");
        let v1 = p.tracks[0].id;
        for (text, start) in [("Hello <you> & me", 24), ("Second\nline", 96)] {
            let Ok(Outcome::Media(m)) = (Command::AddMedia {
                name: "Text".into(),
                source: MediaSource::Title(TitleText {
                    text: text.into(),
                    ..Default::default()
                }),
                length: None,
                default_duration: 48,
            })
            .apply(&mut p) else {
                panic!()
            };
            Command::AddClip {
                track: v1,
                media: m,
                start,
                duration: Some(48),
                with_sound: false,
            }
            .apply(&mut p)
            .unwrap();
        }
        p
    }

    #[test]
    fn title_clips_become_cues_cut_to_the_range() {
        let p = project();
        let all = cues(&p, 0, 1000);
        assert_eq!(all.len(), 2);
        assert_eq!((all[0].start, all[0].end), (24, 72));
        let part = cues(&p, 48, 120);
        assert_eq!(
            part.iter().map(|c| (c.start, c.end)).collect::<Vec<_>>(),
            [(0, 24), (48, 72)]
        );
    }

    #[test]
    fn srt_and_webvtt_are_read_back() {
        let fps = Fps { num: 25, den: 1 };
        let cues = vec![
            Cue {
                start: 25,
                end: 50,
                text: "Hello".into(),
            },
            Cue {
                start: 75,
                end: 100,
                text: "Two\nlines".into(),
            },
        ];
        for kind in [SubtitleKind::Srt, SubtitleKind::WebVtt] {
            assert_eq!(parse(&write(kind, &cues, fps), fps), cues, "{kind:?}");
        }
        let vtt = "WEBVTT\n\n00:01.000 --> 00:02.000 align:start\n<i>Hi</i>\n";
        assert_eq!(
            parse(vtt, fps),
            vec![Cue {
                start: 25,
                end: 50,
                text: "Hi".into()
            }]
        );
        assert!(parse("1\nnot a time --> nor this\nx\n", fps).is_empty());
    }

    #[test]
    fn the_three_kinds_of_file() {
        let p = project();
        let c = cues(&p, 0, 1000);
        let srt = write(SubtitleKind::Srt, &c, p.fps);
        assert!(srt.starts_with("1\n00:00:01,000 --> 00:00:03,000\nHello <you> & me\n\n2\n"));
        let vtt = write(SubtitleKind::WebVtt, &c, p.fps);
        assert!(vtt.starts_with("WEBVTT\n\n00:00:01.000 --> 00:00:03.000\n"));
        let ttml = write(SubtitleKind::Imsc1, &c, p.fps);
        assert!(ttml.contains(
            "<p begin=\"00:00:01.000\" end=\"00:00:03.000\">Hello &lt;you&gt; &amp; me</p>"
        ));
        assert!(ttml.contains("Second<br/>line"));
        assert!(write(SubtitleKind::Srt, &[], p.fps).is_empty());
    }
}
