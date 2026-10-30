//! A tool for the translators, not a check: draws every page and the main windows without a
//! screen and writes the text they paint, one line each, so the translations are made for the
//! text the program really shows.
//!
//! `PICASSO_I18N_COLLECT=scripts/i18n/ui_labels.txt cargo test -p picasso-ui collect_ui_text -- --ignored`

use std::collections::BTreeSet;

use egui::{Context, Pos2, RawInput, Rect, Shape};

use crate::app::{Page, StudioApp};

fn texts(shape: &Shape, out: &mut BTreeSet<String>) {
    match shape {
        Shape::Text(t) => {
            let text = t.galley.job.text.trim();
            if !text.is_empty() {
                out.insert(text.to_string());
            }
        }
        Shape::Vec(shapes) => shapes.iter().for_each(|s| texts(s, out)),
        _ => {}
    }
}

fn frames(app: &mut StudioApp, ctx: &Context, size: (f32, f32), out: &mut BTreeSet<String>) {
    for _ in 0..3 {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(size.0, size.1))),
            ..RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| app.ui(ui));
        // Nothing is drawn, so nothing is uploaded.
        output.textures_delta.clear();
        for clipped in &output.shapes {
            texts(&clipped.shape, out);
        }
    }
}

#[test]
#[ignore = "a tool: writes the file named by PICASSO_I18N_COLLECT"]
fn collect_ui_text() {
    let Some(path) = std::env::var_os("PICASSO_I18N_COLLECT") else {
        return;
    };
    let ctx = Context::default();
    let mut app = StudioApp::new(&ctx);
    app.load_demo();
    let mut found = BTreeSet::new();
    // The project manager comes first, then the pages.
    frames(&mut app, &ctx, (1920.0, 1080.0), &mut found);
    app.manager.open = false;
    for page in [
        Page::Media,
        Page::Cut,
        Page::Edit,
        Page::Color,
        Page::Capture,
        Page::Effects,
        Page::Audio,
        Page::Deliver,
    ] {
        app.page = page;
        frames(&mut app, &ctx, (1920.0, 1080.0), &mut found);
    }
    // The windows that open over the pages.
    app.show_about = true;
    app.show_settings = true;
    app.show_shortcuts = true;
    app.extra.preferences = true;
    app.extra.media_management = true;
    frames(&mut app, &ctx, (1920.0, 1080.0), &mut found);
    let text = found.into_iter().collect::<Vec<_>>().join("\n");
    std::fs::write(path, text).expect("the collected text is written");
}

/// What the first frame paints, with the language `language` in use.
fn painted_in(language: crate::i18n::Language) -> BTreeSet<String> {
    let ctx = Context::default();
    let mut app = StudioApp::new(&ctx);
    app.load_demo();
    crate::i18n::set(language);
    let mut found = BTreeSet::new();
    frames(&mut app, &ctx, (1920.0, 1080.0), &mut found);
    app.manager.open = false;
    app.page = Page::Media;
    frames(&mut app, &ctx, (1920.0, 1080.0), &mut found);
    crate::i18n::set(crate::i18n::Language::English);
    found
}

#[test]
fn the_pages_are_painted_in_the_language_chosen() {
    use crate::i18n::{Language, CHANGING};
    let _only = CHANGING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let english = painted_in(Language::English);
    assert!(english.contains("Projects"), "{english:?}");
    assert!(english.contains("Media Storage"), "{english:?}");

    let spanish = painted_in(Language::Spanish);
    // The text with a translation is painted in it, and not in English.
    assert!(spanish.contains("Proyectos"), "{spanish:?}");
    assert!(spanish.contains("Almacenamiento"), "{spanish:?}");
    assert!(!spanish.contains("Projects"));
    assert!(!spanish.contains("Media Storage"));
    // What has no translation (a name) is as it was.
    assert!(spanish.contains("Picasso Suite"));

    // Back in English, nothing stays translated: the text is only changed on the way to the screen.
    let again = painted_in(Language::English);
    assert!(again.contains("Projects") && !again.contains("Proyectos"));
}
