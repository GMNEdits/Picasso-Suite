//! The language of the menus: Help > Language. English is the default; a language's menus are
//! the English labels with their translations (`tr`), so a label with no translation (yet) shows in
//! English. The choice is kept in `preferences.txt` (`crate::prefs`) and is back at the next start.
//!
//! A translation has to fit where the English label sits: the menu names of the title row have
//! a slot each, so those are short forms (`Arch.` for File in Spanish), and the entries of a menu
//! are at most a third longer than the English ones (`scripts/i18n/build.py` checks both, and the
//! tests here check what it wrote). The tables in this folder are made from
//! `scripts/i18n/<code>.txt`; change those and run `python scripts/i18n/build.py <code>`.
//!
//! The menus are translated where they are drawn (`tr`); the text of the pages, panels and windows
//! is translated where it is painted (`paint`), once a frame, for the text that has a translation.
//! Every language has every menu entry and the text the pages show first (about 190 pieces of text
//! each); what has no translation (yet), such as a dialog or a tooltip, shows in English. The languages here are those the bundled
//! font draws (Latin, Greek and Cyrillic scripts): Chinese, Japanese, Korean, Arabic, Hindi and the
//! like need a font of their own first.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

pub(crate) use paint::translate_frame;

#[cfg(test)]
mod collect;
mod de;
mod paint;
mod es;
mod fr;
mod it;
mod nl;
mod pl;
mod pt;
mod ru;
mod sv;
mod tr;
mod uk;

/// A language the menus can be shown in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub(crate) enum Language {
    #[default]
    English,
    Spanish,
    Portuguese,
    French,
    German,
    Italian,
    Dutch,
    Polish,
    Turkish,
    Swedish,
    Russian,
    Ukrainian,
}

impl Language {
    /// Every language, in the order of the Help > Language menu.
    pub(crate) const ALL: [Language; 12] = [
        Language::English,
        Language::Spanish,
        Language::Portuguese,
        Language::French,
        Language::German,
        Language::Italian,
        Language::Dutch,
        Language::Polish,
        Language::Turkish,
        Language::Swedish,
        Language::Russian,
        Language::Ukrainian,
    ];

    /// The short code kept in `preferences.txt`.
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Spanish => "es",
            Self::Portuguese => "pt",
            Self::French => "fr",
            Self::German => "de",
            Self::Italian => "it",
            Self::Dutch => "nl",
            Self::Polish => "pl",
            Self::Turkish => "tr",
            Self::Swedish => "sv",
            Self::Russian => "ru",
            Self::Ukrainian => "uk",
        }
    }

    /// The language's own name for itself, as the menu shows it.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::English => "English (Default)",
            Self::Spanish => "Español",
            Self::Portuguese => "Português",
            Self::French => "Français",
            Self::German => "Deutsch",
            Self::Italian => "Italiano",
            Self::Dutch => "Nederlands",
            Self::Polish => "Polski",
            Self::Turkish => "Türkçe",
            Self::Swedish => "Svenska",
            Self::Russian => "Русский",
            Self::Ukrainian => "Українська",
        }
    }

    /// The language with this code; `None` for one this version does not have.
    pub(crate) fn from_code(code: &str) -> Option<Self> {
        let code = code.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|l| l.code() == code)
    }

    fn index(self) -> u8 {
        Self::ALL.iter().position(|l| *l == self).unwrap_or(0) as u8
    }

    /// The menu names (File, Edit, ...) and the entries, when it is not English.
    fn data(self) -> Option<(&'static [&'static str; 13], Entries, Entries)> {
        Some(match self {
            Self::English => return None,
            Self::Spanish => (&es::MENUS, es::ENTRIES, es::UI),
            Self::Portuguese => (&pt::MENUS, pt::ENTRIES, pt::UI),
            Self::French => (&fr::MENUS, fr::ENTRIES, fr::UI),
            Self::German => (&de::MENUS, de::ENTRIES, de::UI),
            Self::Italian => (&it::MENUS, it::ENTRIES, it::UI),
            Self::Dutch => (&nl::MENUS, nl::ENTRIES, nl::UI),
            Self::Polish => (&pl::MENUS, pl::ENTRIES, pl::UI),
            Self::Turkish => (&tr::MENUS, tr::ENTRIES, tr::UI),
            Self::Swedish => (&sv::MENUS, sv::ENTRIES, sv::UI),
            Self::Russian => (&ru::MENUS, ru::ENTRIES, ru::UI),
            Self::Ukrainian => (&uk::MENUS, uk::ENTRIES, uk::UI),
        })
    }
}

/// A language's entries: the English label and what it is called there.
type Entries = &'static [(&'static str, &'static str)];

/// The English names of the menus of the title row, in the order of the tables.
const MENU_NAMES: [&str; 13] = [
    "File", "Edit", "Trim", "Timeline", "Clip", "Mark", "View", "Playback", "Effects", "Color",
    "Audio", "Workspace", "Help",
];

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// Held by a test that changes the language in use, so two of them never run at once.
#[cfg(test)]
pub(crate) static CHANGING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The language the menus are shown in now.
pub(crate) fn current() -> Language {
    Language::ALL
        .get(usize::from(CURRENT.load(Ordering::Relaxed)))
        .copied()
        .unwrap_or_default()
}

/// Shows the menus in this language from the next frame on.
pub(crate) fn set(language: Language) {
    CURRENT.store(language.index(), Ordering::Relaxed);
}

/// A label in the language chosen; the English one when it has no translation.
pub(crate) fn tr(label: &'static str) -> &'static str {
    translate(current(), label)
}

/// A menu name of the title row in the language chosen: a short form that fits its slot.
pub(crate) fn menu_name(name: &'static str) -> &'static str {
    menu_name_in(current(), name)
}

pub(crate) fn translate(language: Language, label: &'static str) -> &'static str {
    let Some(table) = table(language) else {
        return label;
    };
    table.get(label).copied().unwrap_or(label)
}

pub(crate) fn menu_name_in(language: Language, name: &'static str) -> &'static str {
    let Some((menus, _, _)) = language.data() else {
        return name;
    };
    MENU_NAMES
        .iter()
        .position(|n| *n == name)
        .and_then(|i| menus.get(i).copied())
        .unwrap_or(name)
}

/// The entries of a language by their English label, made once.
fn table(language: Language) -> Option<&'static HashMap<&'static str, &'static str>> {
    static TABLES: [OnceLock<HashMap<&'static str, &'static str>>; 12] =
        [const { OnceLock::new() }; 12];
    let (_, entries, _) = language.data()?;
    Some(
        TABLES[usize::from(language.index())]
            .get_or_init(|| entries.iter().copied().collect()),
    )
}

/// The text of the pages and windows of a language by its English text, made once.
fn ui_table(language: Language) -> Option<&'static HashMap<&'static str, &'static str>> {
    static TABLES: [OnceLock<HashMap<&'static str, &'static str>>; 12] =
        [const { OnceLock::new() }; 12];
    let (_, menus, ui) = language.data()?;
    Some(TABLES[usize::from(language.index())].get_or_init(|| {
        // A menu label is also text on a page ("Media", "Color"): one table for both.
        menus.iter().chain(ui).copied().collect()
    }))
}

/// A page text in the language chosen, for text a page draws itself (the page tabs are centred, so
/// they are translated where they are drawn); English when it has no translation.
pub(crate) fn tr_ui(text: &'static str) -> &'static str {
    translate_ui(current(), text).unwrap_or(text)
}

/// `Painter::text` with the text in the language chosen. The anchor is the caller's, so a label that is
/// right-aligned or centred stays so when the translation is wider or narrower than the English:
/// it grows to the side it should, and not into what is next to it.
pub(crate) fn paint_text(
    painter: &egui::Painter,
    pos: egui::Pos2,
    anchor: egui::Align2,
    text: impl ToString,
    font: egui::FontId,
    color: egui::Color32,
) -> egui::Rect {
    let text = text.to_string();
    let language = current();
    let translated = match translate_ui(language, text.trim()) {
        Some(translated) if language != Language::English => Some(translated.to_string()),
        _ => None,
    };
    let Some(shown) = translated else {
        return painter.text(pos, anchor, text, font, color);
    };
    // A sentence (a hint in the middle of a panel) that is wider than the English one is drawn smaller,
    // not over the panel beside it. A short label is never drawn smaller than the ones around it.
    let font = if shown.chars().count() >= LONG && !shown.contains('\n') {
        let width = |t: &str| painter.layout_no_wrap(t.to_string(), font.clone(), color).size().x;
        let (was, now) = (width(&text), width(&shown));
        match smaller(was, now) {
            Some(scale) => egui::FontId::new(font.size * scale, font.family.clone()),
            None => font,
        }
    } else {
        font
    };
    painter.text(pos, anchor, shown, font, color)
}

/// From this many characters a text is a sentence, which may be drawn smaller to fit its place.
pub(crate) const LONG: usize = 18;
/// The smallest a sentence is drawn.
const SMALLEST: f32 = 0.7;

/// How much smaller a text of width `now` is drawn to be as wide as the English one, `was`; `None`
/// when it already is (or would have to go below [`SMALLEST`], when it is left to overflow instead
/// of being unreadable).
pub(crate) fn smaller(was: f32, now: f32) -> Option<f32> {
    if now <= was + 1.0 || now <= 0.0 {
        return None;
    }
    Some((was / now).max(SMALLEST))
}

/// Text painted on a page or in a window, in the language; `None` when it has no translation.
pub(crate) fn translate_ui(language: Language, text: &str) -> Option<&'static str> {
    ui_table(language)?.get(text).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The longest a menu name of the title row can be, by the slot each has there.
    const MENU_MAX: [usize; 13] = [5, 5, 5, 9, 5, 5, 5, 9, 7, 5, 8, 11, 8];

    #[test]
    fn english_is_the_default_and_every_language_has_a_code() {
        assert_eq!(Language::default(), Language::English);
        assert_eq!(Language::English.name(), "English (Default)");
        for l in Language::ALL {
            assert_eq!(Language::from_code(l.code()), Some(l));
        }
        assert_eq!(Language::from_code(" ES "), Some(Language::Spanish));
        assert_eq!(Language::from_code("xx"), None);
        // A language is called by its own name.
        assert_eq!(Language::Spanish.name(), "Español");
        assert_eq!(Language::Portuguese.name(), "Português");
        assert_eq!(Language::German.name(), "Deutsch");
    }

    #[test]
    fn a_label_is_translated_or_stays_english() {
        assert_eq!(translate(Language::English, "Save Project"), "Save Project");
        assert_eq!(translate(Language::Spanish, "Save Project"), "Guardar proyecto");
        assert_eq!(translate(Language::French, "Quit"), "Quitter");
        assert_eq!(translate(Language::Dutch, "Save Project"), "Project opslaan");
        // Not in the table: English, not blank.
        assert_eq!(translate(Language::Dutch, "A new label"), "A new label");
        assert_eq!(translate(Language::Spanish, "No such label"), "No such label");
        assert_eq!(menu_name_in(Language::Spanish, "File"), "Arch.");
        assert_eq!(menu_name_in(Language::English, "File"), "File");
        assert_eq!(menu_name_in(Language::German, "Help"), "Hilfe");
    }

    #[test]
    fn the_language_chosen_is_the_one_in_use() {
        // It puts the language back.
        let _only = CHANGING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let before = current();
        set(Language::Italian);
        assert_eq!(current(), Language::Italian);
        assert_eq!(tr("Quit"), "Esci");
        assert_eq!(menu_name("Edit"), "Mod.");
        set(before);
    }

    #[test]
    fn every_translation_fits_where_the_english_label_sits() {
        for language in Language::ALL {
            let Some((menus, entries, _)) = language.data() else {
                continue;
            };
            for (i, name) in menus.iter().enumerate() {
                assert!(
                    name.chars().count() <= MENU_MAX[i],
                    "{language:?}: the menu {} is {name:?}, too long for its slot",
                    MENU_NAMES[i]
                );
            }
            for (english, shown) in entries {
                let most = (english.chars().count() + 6)
                    .max((english.chars().count() * 135).div_ceil(100))
                    .clamp(20, 48);
                assert!(
                    shown.chars().count() <= most,
                    "{language:?}: {english:?} -> {shown:?} is too long ({} > {most})",
                    shown.chars().count()
                );
                assert!(!shown.trim().is_empty(), "{language:?}: {english:?} is empty");
            }
        }
    }

    #[test]
    fn a_translation_is_for_a_label_a_menu_has() {
        use crate::menu::{Entry, MENUS};
        fn collect(entries: &[Entry], out: &mut std::collections::HashSet<&'static str>) {
            for e in entries {
                match e {
                    Entry::Item { label, .. } => {
                        out.insert(*label);
                    }
                    Entry::Sub { label, items } => {
                        out.insert(*label);
                        collect(items, out);
                    }
                    Entry::Sep => {}
                }
            }
        }
        let mut labels = std::collections::HashSet::new();
        for (_, _, entries) in MENUS {
            collect(entries, &mut labels);
        }
        for language in Language::ALL {
            let Some((_, entries, _)) = language.data() else {
                continue;
            };
            for (english, _) in entries {
                assert!(
                    labels.contains(english),
                    "{language:?} translates {english:?}, which no menu has"
                );
            }
        }
    }

    #[test]
    fn the_help_menu_lists_every_language_by_its_own_name() {
        use crate::menu::{Act, Entry, MENUS};
        let help = MENUS.iter().find(|m| m.0 == "Help").map(|m| m.2).unwrap_or(&[]);
        let items = help
            .iter()
            .find_map(|e| match e {
                Entry::Sub { label: "Language", items } => Some(*items),
                _ => None,
            })
            .expect("Help has a Language submenu");
        let listed: Vec<(&str, Act)> = items
            .iter()
            .filter_map(|e| match e {
                Entry::Item { label, act: Some(act), .. } => Some((*label, *act)),
                _ => None,
            })
            .collect();
        assert_eq!(listed.len(), Language::ALL.len());
        for (l, (label, act)) in Language::ALL.iter().zip(&listed) {
            assert_eq!((*label, *act), (l.name(), Act::Language(*l)));
        }
        assert_eq!(listed[0].0, "English (Default)", "English is first");
    }

    #[test]
    fn a_sentence_wider_than_the_english_is_made_smaller_but_not_unreadable() {
        assert_eq!(smaller(300.0, 290.0), None, "narrower: as it is");
        assert_eq!(smaller(300.0, 300.5), None, "a pixel over: as it is");
        let scale = smaller(300.0, 330.0).expect("wider is made smaller");
        assert!((scale - 300.0 / 330.0).abs() < 1e-6);
        assert_eq!(smaller(100.0, 400.0), Some(SMALLEST), "never below the smallest");
    }

    #[test]
    fn a_translated_text_is_not_translated_again() {
        // A text the program paints twice (once where it is drawn, once when the frame ends) would be
        // changed twice if a translation were itself a text that has a translation of its own.
        for language in Language::ALL {
            let Some((_, entries, ui)) = language.data() else {
                continue;
            };
            for (english, shown) in entries.iter().chain(ui) {
                if shown != english {
                    if let Some(again) = translate_ui(language, shown) {
                        assert_eq!(
                            again, *shown,
                            "{language:?}: {english:?} -> {shown:?}, which is translated again to {again:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_translation_is_for_a_label_that_exists_once() {
        for language in Language::ALL {
            let Some((_, entries, _)) = language.data() else {
                continue;
            };
            let mut seen = std::collections::HashSet::new();
            for (english, _) in entries {
                assert!(seen.insert(*english), "{language:?} has {english:?} twice");
            }
        }
    }
}
