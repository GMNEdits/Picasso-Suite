//! Window chrome: the title row with the menus and window buttons, the
//! interface toolbar, and the page bar.

use crate::app::PicassoApp;
use crate::commands::{Cmd, Entry, MENUS, Page, shortcut_text};
use crate::icons::{self, Icon};
use crate::theme::*;
use egui::{Align2, Color32, Rect, ResizeDirection, Sense, Stroke, ViewportCommand, pos2, vec2};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Positions of the menu titles in the reference title row (left edge, points).
const MENU_X: [f32; 13] = [216.0, 267.0, 322.0, 384.0, 480.0, 535.0, 601.0, 665.0, 763.0, 842.0, 910.0, 1003.0, 1122.0];

impl PicassoApp {
    pub(crate) fn title_row(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, MENU_BG);

        // Drag the window by the empty title row; double-click maximises.
        let drag = ui.interact(rect, ui.scope_id().with("title-drag"), Sense::click_and_drag());
        if drag.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        if drag.double_clicked() {
            let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!max));
        }

        if let Some(logo) = &self.logo {
            let r = Rect::from_center_size(pos2(rect.left() + 27.0, rect.center().y), vec2(24.0, 24.0));
            painter.image(logo.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        painter.text(pos2(rect.left() + 52.0, rect.center().y), Align2::LEFT_CENTER, "Picasso Suite", regular(18.0), MENU_TEXT);

        let mut fired = None;
        for ((title, entries), x) in MENUS.iter().zip(MENU_X) {
            let galley = painter.layout_no_wrap(title.to_string(), regular(18.0), MENU_TEXT);
            let r = Rect::from_min_size(pos2(rect.left() + x - 6.0, rect.top() + 4.0), vec2(galley.size().x + 12.0, rect.height() - 8.0));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r));
            child.style_mut().visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
            child.style_mut().visuals.widgets.inactive.bg_stroke = Stroke::NONE;
            child.style_mut().spacing.button_padding = vec2(6.0, 4.0);
            child.menu_button(egui::RichText::new(*title).font(regular(18.0)).color(MENU_TEXT), |ui| {
                ui.set_min_width(260.0);
                self.menu_entries(ui, entries, &mut fired);
            });
        }
        if let Some(cmd) = fired {
            self.run(cmd, &ctx);
        }

        // Window buttons, as in the reference: minimise, maximise/restore, close.
        let buttons = [(1741.0, Icon::Minimize), (1794.0, Icon::Maximize), (1847.0, Icon::Close)];
        for (x, icon) in buttons {
            let r = Rect::from_center_size(pos2(rect.left() + x, rect.center().y), vec2(44.0, rect.height()));
            let resp = ui.interact(r, ui.scope_id().with(("win", x as i32)), Sense::click());
            if resp.hovered() {
                painter.rect_filled(r, 0.0, if icon == Icon::Close { Color32::from_rgb(196, 43, 28) } else { Color32::from_gray(45) });
            }
            icons::paint(&painter, Rect::from_center_size(r.center(), vec2(15.0, 15.0)), icon, if resp.hovered() { ICON_ACTIVE } else { ICON });
            if resp.clicked() {
                match icon {
                    Icon::Minimize => ctx.send_viewport_cmd(ViewportCommand::Minimized(true)),
                    Icon::Maximize => {
                        let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                        ctx.send_viewport_cmd(ViewportCommand::Maximized(!max));
                    }
                    _ => self.run(Cmd::Quit, &ctx),
                }
            }
        }
    }

    fn menu_entries(&self, ui: &mut egui::Ui, entries: &[Entry], fired: &mut Option<Cmd>) {
        for e in entries {
            match e {
                Entry::Sep => {
                    ui.separator();
                }
                Entry::Sub(label, inner) => {
                    ui.menu_button(*label, |ui| {
                        ui.set_min_width(240.0);
                        self.menu_entries(ui, inner, fired);
                    });
                }
                Entry::Item(label, cmd) | Entry::Toggle(label, cmd) => {
                    let enabled = self.cmd_enabled(*cmd);
                    let checked = matches!(e, Entry::Toggle(..)).then(|| self.cmd_checked(*cmd));
                    let text = match checked {
                        Some(true) => format!("✔ {label}"),
                        Some(false) => format!("    {label}"),
                        None => label.to_string(),
                    };
                    let mut button = egui::Button::new(text);
                    if let Some(s) = shortcut_text(ui.ctx(), *cmd) {
                        button = button.shortcut_text(s);
                    }
                    if ui.add_enabled(enabled, button).clicked() {
                        *fired = Some(*cmd);
                        ui.close();
                    }
                }
            }
        }
    }

    /// Lets the borderless window be resized from its edges.
    pub(crate) fn resize_edges(&self, ui: &mut egui::Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)) {
            return;
        }
        let m = 5.0;
        let zones = [
            (Rect::from_min_max(rect.left_top(), pos2(rect.left() + m, rect.bottom())), ResizeDirection::West, egui::CursorIcon::ResizeWest),
            (Rect::from_min_max(pos2(rect.right() - m, rect.top()), rect.right_bottom()), ResizeDirection::East, egui::CursorIcon::ResizeEast),
            (Rect::from_min_max(pos2(rect.left(), rect.bottom() - m), rect.right_bottom()), ResizeDirection::South, egui::CursorIcon::ResizeSouth),
            (Rect::from_min_max(rect.left_top(), pos2(rect.right(), rect.top() + 3.0)), ResizeDirection::North, egui::CursorIcon::ResizeNorth),
            (Rect::from_min_max(pos2(rect.right() - 12.0, rect.bottom() - 12.0), rect.right_bottom()), ResizeDirection::SouthEast, egui::CursorIcon::ResizeSouthEast),
        ];
        for (i, (r, dir, cursor)) in zones.into_iter().enumerate() {
            let resp = ui.interact(r, ui.scope_id().with(("resize", i)), Sense::drag()).on_hover_cursor(cursor);
            if resp.drag_started() {
                ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
            }
        }
    }

    /// The interface toolbar: panel toggles with labels, the project name in the middle.
    pub(crate) fn interface_toolbar(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, TOOLBAR_BG);
        let cy = rect.center().y;

        let left: [(Icon, &str, Option<Cmd>, bool); 5] = [
            (Icon::MediaPool, "Media Pool", Some(Cmd::ToggleMediaPool), self.show_pool),
            (Icon::Effects, "Effects", None, false),
            (Icon::Index, "Edit Index", None, false),
            (Icon::SoundLibrary, "Sound Library", None, false),
            (Icon::Keyframes, "Keyframes", None, false),
        ];
        let mut x = rect.left() + 14.0;
        for (icon, label, cmd, active) in left {
            if self.page == Page::Deliver && cmd.is_some() {
                continue;
            }
            x += self.toolbar_button(ui, &painter, x, cy, icon, label, cmd, active, false) + 22.0;
        }

        let right: [(Icon, &str, Option<Cmd>, bool); 4] = [
            (Icon::QuickExport, "Quick Export", Some(Cmd::QuickExport), false),
            (Icon::Mixer, "Mixer", None, false),
            (Icon::Metadata, "Metadata", None, false),
            (Icon::Inspector, "Inspector", Some(Cmd::ToggleInspector), self.show_inspector),
        ];
        let mut x = rect.right() - 14.0;
        for (icon, label, cmd, active) in right.iter().rev() {
            let w = self.toolbar_button(ui, &painter, x, cy, *icon, label, *cmd, *active, true);
            x -= w + 22.0;
        }

        // Project name, centred, with "Edited" while there are unsaved changes.
        let name = self.project.name.clone();
        let g = painter.layout_no_wrap(name.clone(), semibold(19.0), TEXT);
        let pos = pos2(rect.center().x - g.size().x / 2.0, cy - g.size().y / 2.0);
        painter.galley(pos, g.clone(), TEXT);
        if self.dirty {
            painter.text(pos2(pos.x + g.size().x + 12.0, cy), Align2::LEFT_CENTER, "Edited", regular(13.0), TEXT_DIM);
        }
        let _ = ctx;
    }

    /// One icon+label button; returns its width. `from_right` lays it out leftwards from `x`.
    #[allow(clippy::too_many_arguments)]
    fn toolbar_button(
        &mut self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        x: f32,
        cy: f32,
        icon: Icon,
        label: &str,
        cmd: Option<Cmd>,
        active: bool,
        from_right: bool,
    ) -> f32 {
        let g = painter.layout_no_wrap(label.to_owned(), regular(14.0), TEXT);
        let w = 18.0 + 7.0 + g.size().x;
        let left = if from_right { x - w } else { x };
        let r = Rect::from_min_size(pos2(left - 4.0, cy - 14.0), vec2(w + 8.0, 28.0));
        let enabled = cmd.is_some();
        let resp = ui.interact(r, ui.scope_id().with(("tb", label)), if enabled { Sense::click() } else { Sense::hover() });
        let resp = if enabled { resp } else { resp.on_hover_text(format!("{label} — not available yet")) };
        if active {
            painter.rect_filled(r, 3.0, Color32::from_rgb(40, 40, 46));
        }
        let color = if !enabled {
            ICON_DIM
        } else if active || resp.hovered() {
            ICON_ACTIVE
        } else {
            ICON
        };
        icons::paint(painter, Rect::from_min_size(pos2(left, cy - 9.0), vec2(18.0, 18.0)), icon, color);
        painter.text(pos2(left + 25.0, cy), Align2::LEFT_CENTER, label, regular(14.0), if enabled { if active { TEXT } else { TEXT_DIM } } else { TEXT_FAINT });
        if resp.clicked()
            && let Some(c) = cmd {
                let ctx = ui.ctx().clone();
                self.run(c, &ctx);
            }
        w
    }

    /// The page bar along the bottom, measured from the reference.
    pub(crate) fn page_bar(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, PAGEBAR_BG);
        painter.line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0, DIVIDER));
        if let Some(logo) = &self.logo {
            let r = Rect::from_center_size(pos2(rect.left() + 50.0, rect.center().y), vec2(26.0, 26.0));
            painter.image(logo.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        painter.text(pos2(rect.left() + 75.0, rect.center().y), Align2::LEFT_CENTER, format!("Picasso Suite {VERSION}"), semibold(17.0), TEXT);

        // Tabs: 146 points wide, centred 149 apart, the first centred at x=417.
        for (i, page) in Page::ALL.iter().enumerate() {
            let cx = rect.left() + 417.0 + 148.7 * i as f32;
            let tab = Rect::from_center_size(pos2(cx, rect.center().y + 1.0), vec2(146.0, rect.height() - 2.0));
            let selected = self.page == *page;
            let resp = ui.interact(tab, ui.scope_id().with(("page", i)), Sense::click());
            let resp = if page.built() { resp.on_hover_text(page.label()) } else { resp.on_hover_text(format!("{} — not available yet", page.label())) };
            if selected {
                painter.rect_filled(tab, 0.0, TAB_SELECTED_BG);
            }
            let icon = match page {
                Page::Media => Icon::PageMedia,
                Page::Photo => Icon::PagePhoto,
                Page::Cut => Icon::PageCut,
                Page::Edit => Icon::PageEdit,
                Page::Effects => Icon::PageEffects,
                Page::Color => Icon::PageColor,
                Page::Audio => Icon::PageAudio,
                Page::Deliver => Icon::PageDeliver,
            };
            let color = if selected {
                page_color(*page)
            } else if !page.built() {
                ICON_DIM
            } else if resp.hovered() {
                ICON_HOVER
            } else {
                ICON
            };
            icons::paint(&painter, Rect::from_center_size(tab.center(), vec2(26.0, 26.0)), icon, color);
            if resp.clicked() {
                self.run(Cmd::Page(*page), &ctx);
            }
        }
        let home = Rect::from_center_size(pos2(rect.left() + 1751.0, rect.center().y), vec2(26.0, 26.0));
        let gear = Rect::from_center_size(pos2(rect.left() + 1823.0, rect.center().y), vec2(26.0, 26.0));
        icons::button(ui, home, Icon::Home, false, false, "Project manager — not available yet");
        if icons::button(ui, gear, Icon::Settings, false, true, "Project settings").clicked() {
            self.run(Cmd::ProjectSettings, &ctx);
        }
    }
}

/// The colour a page's icon takes when its tab is selected.
fn page_color(page: Page) -> Color32 {
    match page {
        Page::Media => Color32::from_rgb(230, 179, 72),
        Page::Photo => Color32::from_rgb(220, 120, 150),
        Page::Cut => Color32::from_rgb(64, 160, 236),
        Page::Edit => Color32::from_rgb(64, 160, 236),
        Page::Effects => Color32::from_rgb(170, 120, 226),
        Page::Color => Color32::from_rgb(232, 96, 96),
        Page::Audio => Color32::from_rgb(64, 200, 140),
        Page::Deliver => Color32::from_rgb(230, 150, 60),
    }
}
