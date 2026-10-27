//! The Picasso Suite interface (egui).
//!
//! [`StudioApp`] is the whole window: give it the root `Ui` of a frame with [`StudioApp::ui`].
//! The crate does not depend on eframe, so the native app and the WebAssembly build share it.
//! Every change to the project goes through a command of `picasso-core`.

mod actions;
mod app;
mod capture;
mod chrome;
mod clip_more;
mod color_page;
mod cut;
mod deliver;
mod deliver_clips;
mod deliver_form;
mod edit_page;
mod effects;
mod fairlight;
mod fps_prompt;
mod icons;
mod keyboard;
mod keyframes_ui;
mod media;
mod menu;
mod menu_extra;
mod menu_more;
mod monitor;
mod multicam_ui;
mod panels;
mod platform;
mod pool;
mod prefs;
mod project_manager;
mod projectfile;
mod queue;
mod recording;
mod render_cache;
#[cfg(not(target_arch = "wasm32"))]
mod render_presets;
#[cfg(not(target_arch = "wasm32"))]
mod resolve;
mod sound;
mod source_viewer;
mod subtitles;
mod theme;
mod timeline;
mod track_fx;
#[cfg(not(target_arch = "wasm32"))]
mod update;
mod video;
mod viewer;
mod wheel;
mod widgets;

pub use app::{Page, StudioApp};

/// A Markdown report of every menu entry and whether it works yet.
pub fn menu_status_markdown() -> String {
    menu::status_markdown()
}
