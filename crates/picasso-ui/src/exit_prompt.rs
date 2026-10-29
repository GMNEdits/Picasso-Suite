//! "Exit Picasso Suite?": asked when the window is closed (the close button, File > Quit, Alt+F4
//! or the taskbar) while the project has changes that were not saved. The box is the one
//! "Change Project Frame Rate?" is asked in ([`crate::fps_prompt::prompt_box`]).
//!
//! - "Exit without saving" closes the window.
//! - "Save Project" saves as File > Save Project does (to the project's file, or after asking
//!   where on the computer to put it), and the window closes once the project is written. If the
//!   save is cancelled or fails, the program stays open.
//! - Escape leaves the box and the program open.

use std::sync::atomic::{AtomicBool, Ordering};

use egui::Context;
#[cfg(not(target_arch = "wasm32"))]
use egui::ViewportCommand;

use crate::app::StudioApp;
use crate::fps_prompt::prompt_box;

/// Set when the program closes itself (an update restarting it, a timing run, or an exit that was
/// answered), so the close request that follows is not asked about.
static EXIT_APPROVED: AtomicBool = AtomicBool::new(false);

/// Lets the window close without asking about unsaved changes. For the program's own exits.
pub fn allow_exit() {
    EXIT_APPROVED.store(true, Ordering::Relaxed);
}

fn exit_approved() -> bool {
    EXIT_APPROVED.load(Ordering::Relaxed)
}

impl StudioApp {
    /// Looks for a request to close the window: with unsaved changes it is cancelled and the
    /// question is asked instead. Also closes the window once a save made for the exit is done.
    pub(crate) fn watch_for_close(&mut self, ctx: &Context) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.exit_now {
                self.exit_now = false;
                allow_exit();
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            if !ctx.input(|i| i.viewport().close_requested()) {
                return;
            }
            if exit_approved() || !self.session.is_dirty() {
                return;
            }
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.exit_prompt = true;
        }
        #[cfg(target_arch = "wasm32")]
        let _ = ctx;
    }

    /// Draws the question while one is waiting, and acts on the answer.
    pub(crate) fn exit_prompt_window(&mut self, ctx: &Context) {
        if !self.exit_prompt {
            return;
        }
        let lines = [
            "Do you really want to exit, without saving?".to_string(),
            "Any changes made to the project will be lost.".to_string(),
        ];
        let mut answer = prompt_box(
            ctx,
            "exit_prompt",
            "Exit Picasso Suite?",
            &lines,
            ("Exit without saving", 245.0),
            "Save Project",
        );
        if answer.is_none() {
            answer = ctx.input(|i| {
                if i.key_pressed(egui::Key::Enter) {
                    Some(true)
                } else {
                    None
                }
            });
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.exit_prompt = false;
                return;
            }
        }
        match answer {
            Some(true) => self.save_and_exit(ctx),
            Some(false) => self.exit_without_saving(ctx),
            None => {}
        }
    }

    /// "Exit without saving".
    pub(crate) fn exit_without_saving(&mut self, ctx: &Context) {
        self.exit_prompt = false;
        // The changes were given up on purpose: a copy kept for a crash would only ask again.
        let path = self.project_path.clone().map(std::path::PathBuf::from);
        let title = self.project().name.clone();
        crate::autosave::clear(path.as_deref(), &title);
        allow_exit();
        #[cfg(not(target_arch = "wasm32"))]
        ctx.send_viewport_cmd(ViewportCommand::Close);
        #[cfg(target_arch = "wasm32")]
        let _ = ctx;
    }

    /// "Save Project": saves as File > Save Project does; the window closes when the project has
    /// been written ([`StudioApp::exit_after_save`]).
    pub(crate) fn save_and_exit(&mut self, ctx: &Context) {
        self.exit_prompt = false;
        self.pending_saved = None;
        self.exit_after_save = true;
        self.save(ctx, false);
        // Nothing was started (the project could not be written out): stay open.
        if self.pending_saved.is_none() {
            self.exit_after_save = false;
        }
    }
}
