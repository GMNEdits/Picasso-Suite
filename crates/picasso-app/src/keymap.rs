//! User-editable keyboard bindings, saved to the per-user config folder.
//!
//! The defaults live in [`BINDINGS`]; the user's changes are stored per command in
//! `keybinds.json`. Assigning a key that another command already uses asks first.

use crate::app::PicassoApp;
use crate::commands::{BINDINGS, Binding, Cmd, Entry, MENUS};
use crate::theme::{TEXT_DIM, WARN};
use egui::{Event, Key, Modifiers, RichText};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
struct Saved {
    cmd: String,
    alt: bool,
    shift: bool,
    command: bool,
    key: String,
}

/// A key the user picked that another command already owns, waiting for a decision.
#[derive(Clone, Copy)]
pub struct Conflict {
    pub cmd: Cmd,
    pub mods: Modifiers,
    pub key: Key,
    pub other: Cmd,
}

pub struct Keymap {
    bindings: Vec<Binding>,
    /// The command whose new key is being captured.
    pub capturing: Option<Cmd>,
    pub conflict: Option<Conflict>,
}

fn normalize(m: Modifiers) -> Modifiers {
    Modifiers { alt: m.alt, shift: m.shift, command: m.command, ctrl: false, mac_cmd: false }
}

fn config_path() -> Option<PathBuf> {
    crate::config::path("keybinds.json")
}

fn same_combo(a: &Binding, mods: Modifiers, key: Key) -> bool {
    a.key == key && normalize(a.mods) == normalize(mods)
}

impl Keymap {
    pub fn load() -> Self {
        let mut map = Keymap { bindings: BINDINGS.to_vec(), capturing: None, conflict: None };
        if let Some(saved) = config_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str::<Vec<Saved>>(&t).ok())
        {
            map.apply_saved(saved);
        }
        map
    }

    fn apply_saved(&mut self, saved: Vec<Saved>) {
        let mut replaced: Vec<Cmd> = Vec::new();
        for s in saved {
            let Some(cmd) = BINDINGS.iter().map(|b| b.cmd).find(|c| format!("{c:?}") == s.cmd) else { continue };
            if !s.key.is_empty() && Key::from_name(&s.key).is_none() {
                continue;
            }
            if !replaced.contains(&cmd) {
                self.bindings.retain(|b| b.cmd != cmd);
                replaced.push(cmd);
            }
            let Some(key) = Key::from_name(&s.key) else { continue };
            let mods = Modifiers { alt: s.alt, shift: s.shift, command: s.command, ..Modifiers::NONE };
            if !self.bindings.iter().any(|b| same_combo(b, mods, key)) {
                self.bindings.push(Binding { cmd, mods, key });
            }
        }
        self.sort();
    }

    /// More specific modifiers first, so Ctrl+Shift+S wins over Ctrl+S.
    fn sort(&mut self) {
        let weight = |m: Modifiers| m.alt as u8 + m.shift as u8 + m.command as u8;
        self.bindings.sort_by_key(|b| std::cmp::Reverse(weight(b.mods)));
    }

    fn save(&self) {
        let Some(path) = config_path() else { return };
        let changed: Vec<Cmd> = Self::commands().into_iter().filter(|c| !self.is_default(*c)).collect();
        let saved: Vec<Saved> = self
            .bindings
            .iter()
            .filter(|b| changed.contains(&b.cmd))
            .map(|b| Saved { cmd: format!("{:?}", b.cmd), alt: b.mods.alt, shift: b.mods.shift, command: b.mods.command, key: b.key.name().to_owned() })
            .collect();
        // A command the user cleared has no entry; record it with an unknown key so it stays unbound.
        let cleared = changed.iter().filter(|c| !self.bindings.iter().any(|b| b.cmd == **c));
        let saved: Vec<Saved> = saved
            .into_iter()
            .chain(cleared.map(|c| Saved { cmd: format!("{c:?}"), alt: false, shift: false, command: false, key: String::new() }))
            .collect();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(&saved) {
            let _ = std::fs::write(path, text);
        }
    }

    /// Every command that has a default key, in table order.
    pub fn commands() -> Vec<Cmd> {
        let mut out: Vec<Cmd> = Vec::new();
        for b in BINDINGS {
            if !out.contains(&b.cmd) {
                out.push(b.cmd);
            }
        }
        out
    }

    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }

    pub fn primary(&self, cmd: Cmd) -> Option<&Binding> {
        self.bindings.iter().find(|b| b.cmd == cmd)
    }

    pub fn text(&self, ctx: &egui::Context, cmd: Cmd) -> Option<String> {
        self.primary(cmd).map(|b| ctx.format_shortcut(&egui::KeyboardShortcut::new(b.mods, b.key)))
    }

    pub fn is_default(&self, cmd: Cmd) -> bool {
        let mine: Vec<(Modifiers, Key)> = self.bindings.iter().filter(|b| b.cmd == cmd).map(|b| (normalize(b.mods), b.key)).collect();
        let theirs: Vec<(Modifiers, Key)> = BINDINGS.iter().filter(|b| b.cmd == cmd).map(|b| (normalize(b.mods), b.key)).collect();
        mine.len() == theirs.len() && theirs.iter().all(|t| mine.contains(t))
    }

    pub fn find_conflict(&self, cmd: Cmd, mods: Modifiers, key: Key) -> Option<Cmd> {
        self.bindings.iter().find(|b| b.cmd != cmd && same_combo(b, mods, key)).map(|b| b.cmd)
    }

    /// Binds `cmd` to the key unless another command has it; then a [`Conflict`] is left to resolve.
    pub fn try_assign(&mut self, cmd: Cmd, mods: Modifiers, key: Key) {
        match self.find_conflict(cmd, mods, key) {
            Some(other) => self.conflict = Some(Conflict { cmd, mods, key, other }),
            None => self.assign(cmd, mods, key),
        }
    }

    /// Binds `cmd` to the key, taking it from any other command.
    pub fn assign(&mut self, cmd: Cmd, mods: Modifiers, key: Key) {
        let mods = normalize(mods);
        self.bindings.retain(|b| b.cmd != cmd && !same_combo(b, mods, key));
        self.bindings.push(Binding { cmd, mods, key });
        self.sort();
        self.conflict = None;
        self.save();
    }

    pub fn reset(&mut self, cmd: Cmd) {
        // Another command may have taken a default key since; it keeps it.
        self.bindings.retain(|b| b.cmd != cmd);
        for d in BINDINGS.iter().filter(|d| d.cmd == cmd) {
            if !self.bindings.iter().any(|b| same_combo(b, d.mods, d.key)) {
                self.bindings.push(*d);
            }
        }
        self.sort();
        self.save();
    }

    pub fn reset_all(&mut self) {
        self.bindings = BINDINGS.to_vec();
        self.sort();
        self.conflict = None;
        self.capturing = None;
        self.save();
    }
}

/// Menu label for a command, falling back to its name.
fn label(cmd: Cmd) -> String {
    fn walk(entries: &[Entry], cmd: Cmd) -> Option<&'static str> {
        entries.iter().find_map(|e| match e {
            Entry::Item(l, c) | Entry::Toggle(l, c) if *c == cmd => Some(*l),
            Entry::Sub(_, inner) => walk(inner, cmd),
            _ => None,
        })
    }
    MENUS.iter().find_map(|(_, entries)| walk(entries, cmd)).map(|l| l.trim_end_matches('…').to_owned()).unwrap_or_else(|| format!("{cmd:?}"))
}

impl PicassoApp {
    pub(crate) fn shortcuts_window(&mut self, ctx: &egui::Context) {
        if !self.shortcuts_open {
            self.keymap.capturing = None;
            self.keymap.conflict = None;
            return;
        }
        // Take the next key press as the new binding.
        if let Some(cmd) = self.keymap.capturing {
            let pressed = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    Event::Key { key, pressed: true, modifiers, repeat: false, .. } => Some((*key, *modifiers)),
                    _ => None,
                })
            });
            if let Some((key, mods)) = pressed {
                self.keymap.capturing = None;
                if key != Key::Escape {
                    self.keymap.try_assign(cmd, mods, key);
                }
            }
        }

        let mut open = self.shortcuts_open;
        egui::Window::new("Keyboard Shortcuts").open(&mut open).resizable(true).default_width(520.0).default_pos(egui::pos2(700.0, 120.0)).show(ctx, |ui| {
            ui.label(RichText::new("Click a key to change it, then press the new combination. Esc cancels.").color(TEXT_DIM));
            if let Some(c) = self.keymap.conflict {
                let text = ctx.format_shortcut(&egui::KeyboardShortcut::new(c.mods, c.key));
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.colored_label(WARN, format!("⚠ {text} is already used by “{}”.", label(c.other)));
                    ui.horizontal(|ui| {
                        if ui.button(format!("Replace (unbinds {})", label(c.other))).clicked() {
                            self.keymap.assign(c.cmd, c.mods, c.key);
                        }
                        if ui.button("Cancel").clicked() {
                            self.keymap.conflict = None;
                        }
                    });
                });
            }
            ui.add_space(4.0);
            egui::ScrollArea::vertical().max_height(460.0).show(ui, |ui| {
                egui::Grid::new("keys").striped(true).num_columns(3).spacing([16.0, 6.0]).show(ui, |ui| {
                    for cmd in Keymap::commands() {
                        ui.label(label(cmd));
                        let waiting = self.keymap.capturing == Some(cmd);
                        let text = if waiting {
                            "Press a key…".to_owned()
                        } else {
                            self.keymap.text(ctx, cmd).unwrap_or_else(|| "Unbound".to_owned())
                        };
                        let mut button = egui::Button::new(RichText::new(text).monospace()).min_size(egui::vec2(170.0, 0.0));
                        if waiting {
                            button = button.selected(true);
                        }
                        if ui.add(button).clicked() {
                            self.keymap.capturing = if waiting { None } else { Some(cmd) };
                            self.keymap.conflict = None;
                        }
                        let default = self.keymap.is_default(cmd);
                        if ui.add_enabled(!default, egui::Button::new("Reset")).clicked() {
                            self.keymap.reset(cmd);
                        }
                        ui.end_row();
                    }
                });
            });
            ui.add_space(4.0);
            if ui.button("Reset all to defaults").clicked() {
                self.keymap.reset_all();
            }
        });
        self.shortcuts_open = open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> Keymap {
        Keymap { bindings: BINDINGS.to_vec(), capturing: None, conflict: None }
    }

    #[test]
    fn taken_key_is_reported_not_assigned() {
        let mut m = map();
        // Ctrl+N belongs to New Project.
        m.try_assign(Cmd::Razor, Modifiers::COMMAND, Key::N);
        let c = m.conflict.expect("conflict");
        assert_eq!(c.other, Cmd::NewProject);
        assert!(m.primary(Cmd::NewProject).is_some());
        assert_ne!(m.primary(Cmd::Razor).map(|b| b.key), Some(Key::N));
    }

    #[test]
    fn replace_takes_the_key_and_unbinds_the_other() {
        let mut m = map();
        m.assign(Cmd::Razor, Modifiers::COMMAND, Key::N);
        assert!(m.primary(Cmd::NewProject).is_none());
        assert_eq!(m.primary(Cmd::Razor).map(|b| b.key), Some(Key::N));
    }

    #[test]
    fn free_key_is_assigned_and_reset_restores_default() {
        let mut m = map();
        m.assign(Cmd::SaveProject, Modifiers::COMMAND, Key::F9);
        assert!(!m.is_default(Cmd::SaveProject));
        m.reset(Cmd::SaveProject);
        assert!(m.is_default(Cmd::SaveProject));
    }

    #[test]
    fn specific_modifiers_sort_first() {
        let mut m = map();
        m.sort();
        let s = m.bindings().iter().position(|b| b.cmd == Cmd::SaveProjectAs).unwrap();
        let p = m.bindings().iter().position(|b| b.cmd == Cmd::SaveProject).unwrap();
        assert!(s < p);
    }

    #[test]
    fn saved_overrides_keep_other_defaults() {
        let mut m = map();
        m.apply_saved(vec![Saved { cmd: "SaveProject".into(), alt: false, shift: false, command: true, key: Key::F9.name().into() }]);
        assert_eq!(m.primary(Cmd::SaveProject).map(|b| b.key), Some(Key::F9));
        assert!(m.is_default(Cmd::NewProject));
    }
}
