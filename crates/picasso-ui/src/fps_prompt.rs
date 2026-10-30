//! "Change Project Frame Rate?": asked, as in DaVinci Resolve, when the first video of an empty
//! project has a different frame rate from the project. The import waits for the answer, so the
//! clip's length is counted at the frame rate chosen.

use egui::{vec2, Align2, Color32, Context, Rect, Sense, Stroke, StrokeKind};
use picasso_core::{Command, Fps, MediaSource};

use crate::app::StudioApp;
use crate::platform::ImportedFile;
use crate::theme;

/// The question being asked: the video's frame rate, and the files waiting for the answer (the
/// video first).
pub(crate) struct FpsPrompt {
    pub fps: Fps,
    pub files: Vec<ImportedFile>,
    /// What the first file's probe found, so answering does not read the file again.
    pub probed: Option<Result<picasso_media::MediaMetadata, String>>,
}

/// The common frame rates, to which a measured rate that is nearly one of them is rounded.
const COMMON: [(u32, u32); 12] = [
    (24000, 1001),
    (24, 1),
    (25, 1),
    (30000, 1001),
    (30, 1),
    (48, 1),
    (50, 1),
    (60000, 1001),
    (60, 1),
    (100, 1),
    (120000, 1001),
    (120, 1),
];

/// A frame rate read from a file, tidied: 2997/100 or 1000000/33367 become 30000/1001.
pub(crate) fn tidy(num: u32, den: u32) -> Option<Fps> {
    if num == 0 || den == 0 {
        return None;
    }
    let rate = f64::from(num) / f64::from(den);
    let common = COMMON
        .iter()
        .map(|&(num, den)| Fps { num, den })
        .find(|f| (f.as_f64() - rate).abs() < 0.01);
    let fps = common.unwrap_or(Fps { num, den });
    fps.is_valid().then_some(fps)
}

/// "60", "29.97", "23.976".
pub(crate) fn fps_text(fps: Fps) -> String {
    if fps.den == 1 {
        return fps.num.to_string();
    }
    let text = format!("{:.3}", fps.as_f64());
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn same_rate(a: Fps, b: Fps) -> bool {
    u64::from(a.num) * u64::from(b.den) == u64::from(b.num) * u64::from(a.den)
}

/// The box the program asks its questions in, drawn the same for every question: a title, three
/// lines under it, a grey button on the left (`left`, its left edge at `left_from`, in the box's
/// own units) and a red one on the right. Gives `Some(true)` when the red one is clicked and
/// `Some(false)` when the grey one is; keys are the caller's.
pub(crate) fn prompt_box(
    ctx: &Context,
    id: &str,
    title: &str,
    lines: &[String],
    (left, left_from): (&str, f32),
    right: &str,
) -> Option<bool> {
    let mut answer = None;
    egui::Area::new(egui::Id::new(id))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(605.0, 286.0), Sense::click());
            let p = ui.painter();
            let at = |x: f32, y: f32| rect.left_top() + vec2(x, y);
            p.rect_filled(rect, 4.0, Color32::from_rgb(40, 40, 45));
            let strip = Rect::from_min_size(rect.left_top(), vec2(rect.width(), 41.0));
            p.rect_filled(
                strip,
                egui::CornerRadius {
                    nw: 4,
                    ne: 4,
                    sw: 0,
                    se: 0,
                },
                Color32::from_rgb(31, 31, 34),
            );
            p.line_segment(
                [strip.left_bottom(), strip.right_bottom()],
                Stroke::new(1.0, Color32::from_rgb(20, 20, 22)),
            );
            p.rect_stroke(
                rect,
                4.0,
                Stroke::new(1.0, Color32::from_rgb(20, 20, 22)),
                StrokeKind::Outside,
            );
            crate::chrome::paint_logo(ui.ctx(), p, at(76.0, 110.0), 46.0);
            crate::i18n::paint_text(&p,
                at(129.0, 89.0),
                Align2::LEFT_CENTER,
                title,
                theme::semibold(16.5),
                Color32::from_rgb(236, 236, 238),
            );
            for (i, line) in lines.iter().enumerate() {
                crate::i18n::paint_text(&p,
                    at(129.0, 124.0 + 24.8 * i as f32),
                    Align2::LEFT_CENTER,
                    line,
                    theme::regular(13.8),
                    Color32::from_rgb(146, 146, 150),
                );
            }
            for (x0, x1, label, red, id) in [
                (left_from, 439.0, left, false, "prompt_left"),
                (453.0, 583.0, right, true, "prompt_right"),
            ] {
                let button = Rect::from_min_max(at(x0, 225.0), at(x1, 252.0));
                let resp = ui.interact(button, ui.id().with(id), Sense::click());
                let border = match (red, resp.hovered()) {
                    (true, _) => theme::TOOL_RED,
                    (false, true) => Color32::from_rgb(110, 110, 116),
                    (false, false) => Color32::from_rgb(76, 76, 82),
                };
                if resp.hovered() {
                    p.rect_filled(button, 13.5, Color32::from_rgb(48, 48, 54));
                }
                p.rect_stroke(button, 13.5, Stroke::new(1.2, border), StrokeKind::Inside);
                crate::i18n::paint_text(&p,
                    button.center(),
                    Align2::CENTER_CENTER,
                    label,
                    theme::regular(14.0),
                    if red {
                        Color32::from_rgb(232, 232, 234)
                    } else {
                        Color32::from_rgb(188, 188, 192)
                    },
                );
                if resp.clicked() {
                    answer = Some(red);
                }
            }
        });
    answer
}

impl StudioApp {
    /// Whether importing a video of `fps` should ask first: the project is still empty (no clips
    /// on the timeline, no video or sound in the media pool) and its frame rate is different.
    pub(crate) fn should_ask_fps(&self, fps: Fps) -> bool {
        let p = self.project();
        !same_rate(fps, p.fps)
            && p.tracks.iter().all(|t| t.clips.is_empty())
            && !p.media.iter().any(|m| {
                matches!(
                    m.source,
                    MediaSource::Video { .. } | MediaSource::Audio { .. }
                )
            })
    }

    /// Draws the question while one is waiting, and imports the files once it is answered.
    pub(crate) fn fps_prompt_window(&mut self, ctx: &Context) {
        let Some(prompt) = &self.fps_prompt else {
            return;
        };
        let fps = prompt.fps;
        let lines = [
            "The selected clips have a different frame rate".to_string(),
            "to the project. Would you like to change the project".to_string(),
            format!("frame rate to {} frames per second?", fps_text(fps)),
        ];
        let mut answer = prompt_box(
            ctx,
            "fps_prompt",
            "Change Project Frame Rate?",
            &lines,
            ("Don't Change", 281.0),
            "Change",
        );
        if answer.is_none() {
            answer = ctx.input(|i| {
                if i.key_pressed(egui::Key::Enter) {
                    Some(true)
                } else if i.key_pressed(egui::Key::Escape) {
                    Some(false)
                } else {
                    None
                }
            });
        }
        if let Some(change) = answer {
            self.answer_fps_prompt(change);
        }
    }

    /// Answers the question waiting: `change` takes the video's frame rate for the project. The
    /// files that waited for the answer are imported.
    pub(crate) fn answer_fps_prompt(&mut self, change: bool) {
        let Some(prompt) = self.fps_prompt.take() else {
            return;
        };
        if change {
            let (width, height) = (self.project().width, self.project().height);
            self.run(Command::SetFormat {
                width,
                height,
                fps: prompt.fps,
            });
        }
        let mut files = prompt.files.into_iter();
        if let Some(first) = files.next() {
            match prompt.probed {
                Some(probed) => self.add_probed(first, false, probed),
                None => self.import_file(first),
            }
        }
        for file in files {
            self.import_file(file);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measured_rates_are_tidied_and_written_like_resolve() {
        assert_eq!(tidy(2997, 100), Some(Fps::FPS_2997));
        assert_eq!(tidy(1_000_000, 33_367), Some(Fps::FPS_2997));
        assert_eq!(tidy(60, 1), Some(Fps::FPS_60));
        assert_eq!(tidy(0, 1), None);
        assert_eq!(fps_text(Fps::FPS_60), "60");
        assert_eq!(fps_text(Fps::FPS_2997), "29.97");
        assert_eq!(
            fps_text(Fps {
                num: 24000,
                den: 1001
            }),
            "23.976"
        );
        assert!(same_rate(Fps { num: 50, den: 2 }, Fps::FPS_25));
    }
}
