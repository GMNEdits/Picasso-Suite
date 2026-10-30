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
    // The Deliver page's other tabs and the sections that open.
    app.page = Page::Deliver;
    app.deliver_ui.advanced_open = true;
    app.deliver_ui.subtitle_open = true;
    frames(&mut app, &ctx, (1920.0, 1080.0), &mut found);
    app.deliver_ui.tab = crate::deliver::Tab::Audio;
    frames(&mut app, &ctx, (1920.0, 1080.0), &mut found);
    app.deliver_ui.tab = crate::deliver::Tab::File;
    frames(&mut app, &ctx, (1920.0, 1080.0), &mut found);
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

/// Every piece of text a frame paints with the box it takes, in the order painted.
fn boxes(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Text(t) => {
            let text = t.galley.job.text.trim();
            if !text.is_empty() {
                out.push((text.to_string(), t.visual_bounding_rect()));
            }
        }
        Shape::Vec(shapes) => shapes.iter().for_each(|s| boxes(s, out)),
        _ => {}
    }
}

/// What each page and window paints in `language`: its texts with their boxes.
fn painted_boxes(language: crate::i18n::Language) -> Vec<(String, Vec<(String, Rect)>)> {
    let ctx = Context::default();
    let mut app = StudioApp::new(&ctx);
    app.load_demo();
    crate::i18n::set(language);
    let mut all = Vec::new();
    let mut take = |name: &str, app: &mut StudioApp| {
        let mut found = Vec::new();
        for _ in 0..3 {
            found.clear();
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1920.0, 1080.0))),
                ..RawInput::default()
            };
            let mut output = ctx.run_ui(input, |ui| app.ui(ui));
            output.textures_delta.clear();
            for clipped in &output.shapes {
                boxes(&clipped.shape, &mut found);
            }
        }
        all.push((name.to_string(), found));
    };
    take("project manager", &mut app);
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
        take(&format!("{page:?}"), &mut app);
    }
    // The Deliver page's other tabs and the sections that open.
    app.page = Page::Deliver;
    app.deliver_ui.advanced_open = true;
    app.deliver_ui.subtitle_open = true;
    take("Deliver, advanced", &mut app);
    app.deliver_ui.tab = crate::deliver::Tab::Audio;
    take("Deliver, audio", &mut app);
    app.deliver_ui.tab = crate::deliver::Tab::File;
    take("Deliver, file", &mut app);
    // (Not the windows: they open over each other, and over the menu behind, in English too.)
    crate::i18n::set(crate::i18n::Language::English);
    all
}

/// Two texts of one row that run into each other.
fn collisions(boxes: &[(String, Rect)]) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    for (i, (_, a)) in boxes.iter().enumerate() {
        for (j, (_, b)) in boxes.iter().enumerate().skip(i + 1) {
            let both = a.intersect(*b);
            if both.width() > 2.0 && both.height() > 0.5 * a.height().min(b.height()) {
                found.push((i, j));
            }
        }
    }
    found
}

#[test]
fn no_language_puts_a_text_on_top_of_another_that_english_did_not() {
    use crate::i18n::{Language, CHANGING};
    let _only = CHANGING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let english = painted_boxes(Language::English);
    let mut report = String::new();
    for language in Language::ALL.into_iter().skip(1) {
        let shown = painted_boxes(language);
        for ((page, then), (_, now)) in english.iter().zip(&shown) {
            if then.len() != now.len() {
                report.push_str(&format!("{language:?} {page}: {} texts, English {}
", now.len(), then.len()));
                continue;
            }
            let before: std::collections::HashSet<_> = collisions(then).into_iter().collect();
            for (i, j) in collisions(now) {
                if !before.contains(&(i, j)) {
                    report.push_str(&format!(
                        "{language:?} {page}: {:?} runs into {:?} (English {:?} / {:?})
",
                        now[i].0, now[j].0, then[i].0, then[j].0
                    ));
                }
            }
        }
    }
    assert!(report.is_empty(), "texts on top of each other:
{report}");
}

#[test]
fn a_right_aligned_label_keeps_its_right_edge_in_every_language() {
    use crate::i18n::{Language, CHANGING};
    let _only = CHANGING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    // On the Deliver page "Graphics card" is a label that ends where the value begins.
    let find = |all: &[(String, Vec<(String, Rect)>)], text: &str| {
        all.iter()
            .filter(|(page, _)| page == "Deliver")
            .flat_map(|(_, boxes)| boxes.iter())
            .find(|(t, _)| t == text)
            .map(|(_, r)| *r)
    };
    let english = painted_boxes(Language::English);
    let was = find(&english, "Graphics card").expect("the label is painted");
    for (language, text) in [
        (Language::French, "Carte graphique"),
        (Language::German, "Grafikkarte"),
        (Language::Spanish, "Tarjeta gráfica"),
        (Language::Portuguese, "Placa gráfica"),
    ] {
        let shown = painted_boxes(language);
        let now = find(&shown, text).unwrap_or_else(|| panic!("{language:?}: {text:?} is painted"));
        assert!(
            (now.right() - was.right()).abs() < 2.0,
            "{language:?}: the label ends at {} and not where the English ended, {}",
            now.right(),
            was.right()
        );
    }
}
