//! The text of the pages, panels and windows in the language chosen.
//!
//! The pages draw their text in many places (a painter at a fixed spot, a label, a button, a
//! tooltip), so the text is translated where it ends up, once a frame, rather than at each place:
//! every piece of text the frame painted that has a translation is laid out again in the
//! language chosen. Text with no translation (a name, a number, a timecode, a string made of
//! several parts) stays as it is. Nothing but the pixels change; what the program compares
//! and stores stays English.
//!
//! A translation is put where the English text was, at the same size: the tables are written with
//! short forms so that it is not wider than the English (`scripts/i18n/build.py` measures each
//! one), and a text is never drawn smaller than the others around it.

use egui::epaint::text::LayoutJob;
use egui::{Context, LayerId, Shape};

use super::{translate_ui, Language};

/// Translates the text painted this frame. Called at the end of the frame.
pub(crate) fn translate_frame(ctx: &Context) {
    translate_frame_in(ctx, super::current());
}

pub(crate) fn translate_frame_in(ctx: &Context, language: Language) {
    if language == Language::English {
        return;
    }
    let mut layers: Vec<LayerId> = ctx.memory(|m| m.layer_ids().collect());
    if !layers.contains(&LayerId::background()) {
        layers.push(LayerId::background());
    }

    // What is to be translated: where it is and the job to lay out.
    let mut wanted: Vec<(LayerId, usize, LayoutJob)> = Vec::new();
    ctx.graphics(|graphics| {
        for layer in &layers {
            let Some(list) = graphics.get(*layer) else {
                continue;
            };
            for (i, clipped) in list.all_entries().enumerate() {
                let Shape::Text(text) = &clipped.shape else {
                    continue;
                };
                let job = &text.galley.job;
                // Only plain text: one piece in one style, as every label and caption is.
                if job.sections.len() != 1 {
                    continue;
                }
                let Some(shown) = translate_ui(language, job.text.trim()) else {
                    continue;
                };
                let mut new = LayoutJob::single_section(shown.to_string(), job.sections[0].format.clone());
                new.wrap = job.wrap.clone();
                new.halign = job.halign;
                new.justify = job.justify;
                new.break_on_newline = job.break_on_newline;
                new.first_row_min_height = job.first_row_min_height;
                new.sections[0].leading_space = job.sections[0].leading_space;
                wanted.push((*layer, i, new));
            }
        }
    });
    if wanted.is_empty() {
        return;
    }

    let laid_out: Vec<_> = ctx.fonts_mut(|fonts| {
        wanted
            .iter()
            .map(|(_, _, job)| fonts.layout_job(job.clone()))
            .collect()
    });

    ctx.graphics_mut(|graphics| {
        for ((layer, i, _), galley) in wanted.into_iter().zip(laid_out) {
            if let Some(list) = graphics.get_mut(layer) {
                list.mutate_shape(egui::layers::ShapeIdx(i), |clipped| {
                    if let Shape::Text(text) = &mut clipped.shape {
                        text.galley = galley;
                    }
                });
            }
        }
    });
}
