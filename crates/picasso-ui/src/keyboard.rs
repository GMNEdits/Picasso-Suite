//! Keyboard Customization: every command and its keys. Clicking a command's keys waits for a key
//! to press, which becomes its key; a key another command has is moved to it after asking.
//! Changes are saved in the preferences and work at once.

use std::collections::HashMap;

use egui::{Context, Event, Key, RichText};

use crate::app::StudioApp;
use crate::menu::{self, Act};

/// The changed keys `custom` with `act` given the key `text` alone, taken from `others` (which
/// keep their other keys).
pub(crate) fn with_key(
    mut custom: HashMap<Act, Vec<String>>,
    act: Act,
    text: String,
    others: &[Act],
) -> HashMap<Act, Vec<String>> {
    for other in others {
        let rest: Vec<String> = menu::keys_in(&custom, *other)
            .into_iter()
            .filter(|k| !menu::same_key(k, &text))
            .collect();
        custom.insert(*other, rest);
    }
    custom.insert(act, vec![text]);
    custom
}

/// What the window is doing.
#[derive(Default)]
pub(crate) struct KeyEditor {
    /// The command waiting for a key.
    pub capture: Option<Act>,
    /// A key other commands have: the command it is for, the key, and those commands.
    pub conflict: Option<(Act, String, Vec<Act>)>,
    /// The words the list is narrowed to.
    pub search: String,
}

impl StudioApp {
    /// Gives `act` the key `text` alone, taking it from `others`.
    pub(crate) fn assign_key(&mut self, act: Act, text: String, others: &[Act]) {
        let custom = with_key(menu::custom_keys(), act, text, others);
        self.set_keys(custom);
    }

    /// Uses and saves a new set of changed keys.
    pub(crate) fn set_keys(&mut self, custom: HashMap<Act, Vec<String>>) {
        self.extra.prefs.keys = custom;
        self.extra.prefs.apply();
        #[cfg(not(test))]
        self.extra.prefs.save();
    }

    /// A key pressed while a command waits for one: it becomes the command's key, or asks first
    /// when another command has it. Escape gives up.
    fn take_pressed_key(&mut self, ctx: &Context, act: Act) {
        let pressed = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                // Ctrl, Shift and Alt on their own are held for the key that follows.
                Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } if !menu::is_modifier(*key) => Some((*key, *modifiers)),
                _ => None,
            })
        });
        let Some((key, modifiers)) = pressed else {
            return;
        };
        // The key does nothing else.
        ctx.input_mut(|i| i.consume_key(modifiers, key));
        self.extra.keys.capture = None;
        if key == Key::Escape && modifiers.is_none() {
            return;
        }
        let Some(text) = menu::key_text(modifiers, key) else {
            self.fail("that key cannot be used for a command");
            return;
        };
        let users = menu::key_users(&text, act);
        if users.is_empty() {
            self.assign_key(act, text, &[]);
        } else {
            self.extra.keys.conflict = Some((act, text, users));
        }
    }

    pub(crate) fn keyboard_window(&mut self, ctx: &Context) {
        if !self.show_shortcuts {
            self.extra.keys.capture = None;
            return;
        }
        if let Some(act) = self.extra.keys.capture {
            self.take_pressed_key(ctx, act);
        }
        let mut open = true;
        let mut wanted: Option<Act> = None;
        let mut remove: Option<Act> = None;
        let mut reset: Option<Act> = None;
        let mut reset_all = false;
        let mut answer: Option<bool> = None;
        let custom = menu::custom_keys();
        let capture = self.extra.keys.capture;
        let conflict = self.extra.keys.conflict.clone();
        let mut search = std::mem::take(&mut self.extra.keys.search);
        egui::Window::new("Keyboard Customization")
            .open(&mut open)
            .collapsible(false)
            .default_width(620.0)
            .show(ctx, |ui| {
                ui.label(
                    "Click a command's keys, then press the key it should have. Escape gives up.",
                );
                ui.horizontal(|ui| {
                    ui.label("Search");
                    ui.text_edit_singleline(&mut search);
                    if ui
                        .add_enabled(!custom.is_empty(), egui::Button::new("Reset All"))
                        .on_hover_text("Give every command the keys it came with")
                        .clicked()
                    {
                        reset_all = true;
                    }
                });
                if let Some((act, text, users)) = &conflict {
                    ui.separator();
                    let names: Vec<&str> = users.iter().map(|u| menu::act_label(*u)).collect();
                    ui.label(
                        RichText::new(format!(
                            "{text} is the key of {}. Give it to {} instead?",
                            names.join(", "),
                            menu::act_label(*act)
                        ))
                        .color(crate::theme::WHITE),
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Use it here").clicked() {
                            answer = Some(true);
                        }
                        if ui.button("Cancel").clicked() {
                            answer = Some(false);
                        }
                    });
                }
                ui.separator();
                let words = search.to_lowercase();
                egui::ScrollArea::vertical()
                    .min_scrolled_height(480.0)
                    .max_height(520.0)
                    .show(ui, |ui| {
                        egui::Grid::new("shortcut_list")
                            .num_columns(4)
                            .spacing([16.0, 3.0])
                            .striped(true)
                            .show(ui, |ui| {
                                for row in menu::rows() {
                                    if !words.is_empty()
                                        && !row.label.to_lowercase().contains(&words)
                                        && !row.menu.to_lowercase().contains(&words)
                                    {
                                        continue;
                                    }
                                    ui.label(row.menu);
                                    ui.label(row.label);
                                    let Some(act) = row.act else {
                                        // Planned: the key it will get, not yet changeable.
                                        ui.label(RichText::new(&row.default).monospace().weak())
                                            .on_hover_text("Planned: not built yet");
                                        ui.label("");
                                        ui.end_row();
                                        continue;
                                    };
                                    let keys = menu::keys_of(act);
                                    let text = if capture == Some(act) {
                                        "Press a key...".to_string()
                                    } else if keys.is_empty() {
                                        "-".to_string()
                                    } else {
                                        keys.join(" / ")
                                    };
                                    let button = egui::Button::new(RichText::new(text).monospace())
                                        .selected(capture == Some(act))
                                        .min_size(egui::vec2(150.0, 0.0));
                                    if ui
                                        .add(button)
                                        .on_hover_text("Click, then press the new key")
                                        .clicked()
                                    {
                                        wanted = Some(act);
                                    }
                                    ui.horizontal(|ui| {
                                        if !keys.is_empty() && ui.small_button("Remove").clicked() {
                                            remove = Some(act);
                                        }
                                        if custom.contains_key(&act)
                                            && ui
                                                .small_button("Reset")
                                                .on_hover_text(format!(
                                                    "Back to {}",
                                                    if row.default.is_empty() {
                                                        "no key"
                                                    } else {
                                                        &row.default
                                                    }
                                                ))
                                                .clicked()
                                        {
                                            reset = Some(act);
                                        }
                                    });
                                    ui.end_row();
                                }
                            });
                    });
            });
        self.extra.keys.search = search;
        if let Some(act) = wanted {
            self.extra.keys.capture = Some(act);
            self.extra.keys.conflict = None;
        }
        if let Some(yes) = answer {
            if let (true, Some((act, text, users))) = (yes, self.extra.keys.conflict.take()) {
                self.assign_key(act, text, &users);
            }
            self.extra.keys.conflict = None;
        }
        if let Some(act) = remove {
            let mut custom = menu::custom_keys();
            custom.insert(act, Vec::new());
            self.set_keys(custom);
        }
        if let Some(act) = reset {
            let mut custom = menu::custom_keys();
            custom.remove(&act);
            self.set_keys(custom);
        }
        if reset_all {
            self.set_keys(HashMap::new());
        }
        if !open {
            self.extra.keys = KeyEditor::default();
        }
        self.show_shortcuts = open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_moved_from_another_command_leaves_it() {
        // Ctrl+Z is Undo's: giving it to Redo takes it from Undo.
        let before = menu::bindings_with(&HashMap::new());
        let others = menu::key_users_in(&before, "Ctrl+Z", Act::Redo);
        assert_eq!(others, vec![Act::Undo]);
        let custom = with_key(HashMap::new(), Act::Redo, "Ctrl+Z".into(), &others);
        assert_eq!(
            menu::keys_in(&custom, Act::Redo),
            vec!["Ctrl+Z".to_string()]
        );
        assert!(menu::keys_in(&custom, Act::Undo)
            .iter()
            .all(|k| !menu::same_key(k, "Ctrl+Z")));
        let after = menu::bindings_with(&custom);
        assert!(menu::key_users_in(&after, "Ctrl+Z", Act::Redo).is_empty());
        assert!(after
            .iter()
            .any(|b| b.act == Act::Redo && b.text == "Ctrl+Z"));
    }
}
