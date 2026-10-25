//! User preferences, saved to `preferences.json` next to `keybinds.json`.
//!
//! Hardware decoding is a property of the machine (the GPU and its driver), not
//! of a project, so it is kept here rather than in the project settings - the
//! same place DaVinci Resolve keeps its Decode Options.

use crate::config;
use crate::theme::TEXT_DIM;
use egui::RichText;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const FILE: &str = "preferences.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// Use FFmpeg's hardware decoders (`-hwaccel auto`) where the GPU supports them.
    pub hardware_decode: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        // Hardware decoding has always been on by default; the preference only
        // gives users a way to turn it off.
        Self { hardware_decode: true }
    }
}

impl Preferences {
    pub fn load() -> Self {
        Self::load_from(config::path(FILE))
    }

    fn load_from(path: Option<PathBuf>) -> Self {
        path.and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        self.save_to(config::path(FILE));
    }

    fn save_to(&self, path: Option<PathBuf>) {
        let Some(path) = path else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, text + "\n");
        }
    }
}

#[derive(Default)]
pub struct PreferencesWindow {
    pub open: bool,
}

impl PreferencesWindow {
    /// Draws the window. Returns `true` when a preference changed.
    pub fn show(&mut self, ctx: &egui::Context, prefs: &mut Preferences) -> bool {
        if !self.open {
            return false;
        }
        let mut open = self.open;
        let mut changed = false;
        egui::Window::new("Preferences").open(&mut open).resizable(false).show(ctx, |ui| {
            ui.set_min_width(470.0);
            ui.set_max_width(470.0);
            ui.label(RichText::new("System").strong());
            ui.add_space(4.0);
            ui.label(RichText::new("Decode Options").strong());
            changed |= ui
                .checkbox(&mut prefs.hardware_decode, "Decode supported video using hardware acceleration")
                .changed();
            ui.add_space(2.0);
            ui.add(
                egui::Label::new(
                    RichText::new(
                        "H.264, H.265/HEVC, AV1, VP9, MPEG-2 and VC-1 use the GPU decoder when the \
                         graphics driver supports it. Turn this off if clips show green frames, \
                         corruption, or go offline.",
                    )
                    .color(TEXT_DIM)
                    .size(12.0),
                )
                .wrap(),
            );
            ui.add_space(6.0);
            ui.add(
                egui::Label::new(
                    RichText::new(
                        "Changes apply immediately. PICASSO_NO_HWACCEL=1 disables hardware decoding \
                         for a single run.",
                    )
                    .color(TEXT_DIM)
                    .size(12.0),
                )
                .wrap(),
            );
        });
        self.open = open;
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardware_decoding_is_on_by_default() {
        assert!(Preferences::default().hardware_decode);
    }

    #[test]
    fn files_without_the_field_get_the_default() {
        let p: Preferences = serde_json::from_str("{}").unwrap();
        assert!(p.hardware_decode);
        let p: Preferences = serde_json::from_str(r#"{"hardware_decode":false}"#).unwrap();
        assert!(!p.hardware_decode);
    }

    #[test]
    fn round_trips_through_json() {
        let p = Preferences { hardware_decode: false };
        let text = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<Preferences>(&text).unwrap(), p);
    }

    #[test]
    fn saves_and_loads_the_file() {
        let dir = std::env::temp_dir().join(format!("picasso-prefs-{}", std::process::id()));
        let path = dir.join(FILE);
        Preferences { hardware_decode: false }.save_to(Some(path.clone()));
        assert_eq!(Preferences::load_from(Some(path)), Preferences { hardware_decode: false });
        let _ = std::fs::remove_dir_all(dir);
    }
}
