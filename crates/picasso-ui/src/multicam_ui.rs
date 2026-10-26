//! Multicam clips: the New Multicam Clip window (pick the cameras and their sync frames) and
//! the multicam viewer, where a click on an angle (or the keys 1 to 9) cuts the timeline there
//! and goes on with that camera.

use egui::{pos2, Color32, Context, Rect, Sense, Stroke, StrokeKind};
use picasso_core::{Command, MediaId, MediaSource, Multicam, Outcome, TrackKind, MAX_ANGLES};

use crate::app::StudioApp;

/// The New Multicam Clip window: a name, and each video of the media pool with a tick and the
/// frame of it that shows the sync moment.
pub(crate) struct NewMulticam {
    name: String,
    /// (media, its name, used, sync frame)
    picks: Vec<(MediaId, String, bool, i64)>,
}

/// The state of both windows.
#[derive(Default)]
pub(crate) struct MulticamUi {
    pub viewer: bool,
    pub selected: Option<u64>,
    pub creating: Option<NewMulticam>,
}

impl StudioApp {
    /// Opens the New Multicam Clip window with every video of the pool ticked.
    pub(crate) fn open_new_multicam(&mut self) {
        let picks: Vec<(MediaId, String, bool, i64)> = self
            .project()
            .media
            .iter()
            .filter(|m| matches!(m.source, MediaSource::Video { .. }))
            .enumerate()
            .map(|(i, m)| (m.id, m.name.clone(), i < MAX_ANGLES, 0))
            .collect();
        if picks.len() < 2 {
            self.fail("a multicam clip needs two videos in the media pool");
            return;
        }
        let name = format!("Multicam {}", self.project().multicams.len() + 1);
        self.multicam.creating = Some(NewMulticam { name, picks });
    }

    pub(crate) fn toggle_multicam_viewer(&mut self) {
        self.multicam.viewer = !self.multicam.viewer;
        if self.multicam.viewer && self.multicam.selected.is_none() {
            self.multicam.selected = self.project().multicams.first().map(|m| m.id);
        }
    }

    pub(crate) fn multicam_windows(&mut self, ctx: &Context) {
        self.new_multicam_window(ctx);
        self.multicam_viewer_window(ctx);
    }

    fn new_multicam_window(&mut self, ctx: &Context) {
        let Some(mut dialog) = self.multicam.creating.take() else {
            return;
        };
        let mut open = true;
        let mut done = None;
        egui::Window::new("New Multicam Clip")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut dialog.name);
                });
                ui.separator();
                ui.label("Cameras (the frame of each that shows the same moment):");
                egui::Grid::new("new_multicam").show(ui, |ui| {
                    for (_, name, used, offset) in &mut dialog.picks {
                        ui.checkbox(used, name.as_str());
                        ui.add(
                            egui::DragValue::new(offset)
                                .range(-100_000..=100_000)
                                .suffix(" fr"),
                        );
                        ui.end_row();
                    }
                });
                let chosen = dialog.picks.iter().filter(|p| p.2).count();
                ui.label(format!("{chosen} camera(s) of {MAX_ANGLES} at most"));
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            (2..=MAX_ANGLES).contains(&chosen),
                            egui::Button::new("Create"),
                        )
                        .clicked()
                    {
                        done = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        done = Some(false);
                    }
                });
            });
        if done == Some(true) {
            let angles = dialog
                .picks
                .iter()
                .filter(|p| p.2)
                .map(|p| (p.0, p.3))
                .collect();
            let made = self.run(Command::CreateMulticam {
                name: dialog.name.clone(),
                angles,
            });
            if made.is_some() {
                self.multicam.selected = self.project().multicams.last().map(|m| m.id);
                self.multicam.viewer = true;
                self.info("Multicam clip made; place it on the timeline from the Multicam window");
            }
        } else if open && done.is_none() {
            self.multicam.creating = Some(dialog);
        }
    }

    /// The angle the timeline shows at the playhead: the topmost video clip of the multicam.
    fn shown_angle(&self, mc: &Multicam) -> Option<usize> {
        let at = self.playhead;
        self.project()
            .tracks
            .iter()
            .rev()
            .filter(|t| t.kind == TrackKind::Video)
            .flat_map(|t| t.clips.iter())
            .find(|c| c.contains(at) && mc.angle_of(c.media).is_some())
            .and_then(|c| mc.angle_of(c.media))
    }

    /// Switches to `angle` at the playhead.
    pub(crate) fn switch_angle(&mut self, multicam: u64, angle: usize) {
        let at = self.playhead;
        if self
            .run(Command::SwitchAngle {
                multicam,
                angle,
                at,
            })
            .is_some()
        {
            self.info(format!("Angle {}", angle + 1));
        }
    }

    /// Puts the multicam clip on the first video track at the playhead, showing angle 1.
    fn place_multicam(&mut self, mc: &Multicam) {
        let Some(track) = self
            .project()
            .tracks
            .iter()
            .find(|t| t.kind == TrackKind::Video && !t.locked)
            .map(|t| t.id)
        else {
            self.fail("there is no video track");
            return;
        };
        let first = mc.angles[0].offset;
        // The longest stretch every camera has footage for, from the first frame of angle 1.
        let duration = mc
            .angles
            .iter()
            .filter_map(|a| {
                let len = self.project().media(a.media)?.length?;
                Some(len as i64 + first - a.offset)
            })
            .min()
            .and_then(|d| u64::try_from(d).ok())
            .filter(|d| *d > 0);
        let start = self.playhead;
        self.run(Command::OverwriteClip {
            track,
            media: mc.angles[0].media,
            start,
            duration,
            with_sound: true,
        });
    }

    fn multicam_viewer_window(&mut self, ctx: &Context) {
        if !self.multicam.viewer {
            return;
        }
        let multicams: Vec<(u64, String)> = self
            .project()
            .multicams
            .iter()
            .map(|m| (m.id, m.name.clone()))
            .collect();
        let selected = self
            .multicam
            .selected
            .filter(|id| multicams.iter().any(|m| m.0 == *id))
            .or_else(|| multicams.first().map(|m| m.0));
        self.multicam.selected = selected;
        let mc = selected.and_then(|id| {
            self.project()
                .multicams
                .iter()
                .find(|m| m.id == id)
                .cloned()
        });
        let mut open = true;
        let mut choose = None;
        let mut sync = None;
        let mut place = false;
        let mut delete = false;
        let mut new = false;
        let shown = mc.as_ref().and_then(|m| self.shown_angle(m));
        let thumbs: Vec<Option<egui::TextureId>> = mc
            .iter()
            .flat_map(|m| m.angles.iter())
            .map(|a| self.media_thumb(ctx, a.media))
            .collect();
        egui::Window::new("Multicam")
            .open(&mut open)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("multicam_pick")
                        .selected_text(
                            mc.as_ref()
                                .map_or("No multicam clip".to_string(), |m| m.name.clone()),
                        )
                        .show_ui(ui, |ui| {
                            for (id, name) in &multicams {
                                ui.selectable_value(&mut self.multicam.selected, Some(*id), name);
                            }
                        });
                    if ui.button("New...").clicked() {
                        new = true;
                    }
                    if mc.is_some() {
                        if ui.button("Place on Timeline").clicked() {
                            place = true;
                        }
                        if ui.button("Delete").clicked() {
                            delete = true;
                        }
                    }
                });
                let Some(mc) = &mc else {
                    ui.label("Make a multicam clip from the videos of the media pool.");
                    return;
                };
                ui.label("Play, then click an angle (or press 1 to 9) to cut to it.");
                ui.separator();
                let size = egui::vec2(132.0, 74.0);
                ui.horizontal_wrapped(|ui| {
                    for (i, angle) in mc.angles.iter().enumerate() {
                        ui.vertical(|ui| {
                            let (rect, r) = ui.allocate_exact_size(size, Sense::click());
                            ui.painter()
                                .rect_filled(rect, 2.0, Color32::from_rgb(16, 16, 18));
                            if let Some(Some(tex)) = thumbs.get(i) {
                                ui.painter().image(
                                    *tex,
                                    rect,
                                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                    Color32::WHITE,
                                );
                            }
                            let live = shown == Some(i);
                            ui.painter().rect_stroke(
                                rect,
                                2.0,
                                if live {
                                    Stroke::new(2.5, Color32::from_rgb(226, 62, 52))
                                } else {
                                    Stroke::new(1.0, Color32::from_gray(90))
                                },
                                StrokeKind::Inside,
                            );
                            ui.painter().text(
                                rect.left_top() + egui::vec2(5.0, 3.0),
                                egui::Align2::LEFT_TOP,
                                format!("{}", i + 1),
                                egui::FontId::proportional(14.0),
                                Color32::WHITE,
                            );
                            if r.on_hover_text("Cut to this angle at the playhead")
                                .clicked()
                            {
                                choose = Some(i);
                            }
                            let name = self
                                .project()
                                .media(angle.media)
                                .map_or(String::new(), |m| m.name.clone());
                            ui.label(egui::RichText::new(name).small());
                            let mut offset = angle.offset;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut offset)
                                        .range(-100_000..=100_000)
                                        .prefix("sync ")
                                        .suffix(" fr"),
                                )
                                .changed()
                            {
                                sync = Some((i, offset));
                            }
                        });
                    }
                });
            });
        // The number keys cut to an angle while the window is open and no field has the keys.
        if choose.is_none() && !ctx.egui_wants_keyboard_input() {
            let keys = [
                egui::Key::Num1,
                egui::Key::Num2,
                egui::Key::Num3,
                egui::Key::Num4,
                egui::Key::Num5,
                egui::Key::Num6,
                egui::Key::Num7,
                egui::Key::Num8,
                egui::Key::Num9,
            ];
            let count = mc.as_ref().map_or(0, |m| m.angles.len());
            choose = keys
                .iter()
                .take(count)
                .position(|k| ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, *k)));
        }
        self.multicam.viewer = open;
        if new {
            self.open_new_multicam();
        }
        let Some(mc) = mc else {
            return;
        };
        if let Some(angle) = choose {
            self.switch_angle(mc.id, angle);
        }
        if let Some((angle, offset)) = sync {
            self.run_gesture(
                ctx,
                Command::SetAngleOffset {
                    multicam: mc.id,
                    angle,
                    offset,
                },
            );
        }
        if place {
            self.place_multicam(&mc);
        }
        if delete
            && matches!(
                self.run(Command::DeleteMulticam { id: mc.id }),
                Some(Outcome::Nothing)
            )
        {
            self.multicam.selected = None;
        }
    }
}
