//! The Edit page: media pool (full height on the left), source and timeline
//! viewers, the timeline tool row, the timeline, and the Inspector.

use crate::app::PicassoApp;
use crate::commands::Cmd;
use crate::icons::{self, Icon};
use crate::theme::*;
use crate::timeline_ui::{self, MediaDrag, Overviews, Tool};
use egui::{Align2, Color32, Rect, RichText, Sense, Stroke, TextureOptions, pos2, vec2};
use picasso_core::time::{format_timecode, frames_to_seconds, seconds_to_frames};
use picasso_core::{Id, MediaItem, Project, TrackKind};
use std::sync::Arc;

const POOL_W: f32 = 449.0;
const BINS_W: f32 = 145.0;
const INSPECTOR_W: f32 = 430.0;
const SCRUB_H: f32 = 16.0;

#[derive(Clone, Copy, PartialEq)]
enum Which {
    Source,
    Timeline,
}

impl PicassoApp {
    pub(crate) fn edit_page(&mut self, ui: &mut egui::Ui, body: Rect) {
        let ctx = ui.ctx().clone();
        let pool_w = if self.show_pool { POOL_W } else { 0.0 };
        let insp_w = if self.show_inspector { INSPECTOR_W } else { 0.0 };
        let main = Rect::from_min_max(pos2(body.left() + pool_w + if self.show_pool { DIVIDER_W } else { 0.0 }, body.top()), pos2(body.right() - insp_w - if self.show_inspector { DIVIDER_W } else { 0.0 }, body.bottom()));

        if self.show_pool {
            self.media_pool(ui, Rect::from_min_max(body.min, pos2(body.left() + pool_w, body.bottom())));
        }
        if self.show_inspector {
            self.inspector(ui, Rect::from_min_max(pos2(body.right() - insp_w, body.top()), body.max));
        }

        // Viewers above, timeline below (the reference splits roughly in half).
        let viewer_bottom = main.top() + ((main.height() - TL_TOOLBAR_H) * 0.49).max(220.0);
        let viewers = Rect::from_min_max(main.min, pos2(main.right(), viewer_bottom));
        let tl_tools = Rect::from_min_max(pos2(main.left(), viewer_bottom + DIVIDER_W), pos2(main.right(), viewer_bottom + DIVIDER_W + TL_TOOLBAR_H));
        let tl_area = Rect::from_min_max(pos2(main.left(), tl_tools.bottom() + DIVIDER_W), main.max);

        if self.dual_viewer {
            let mid = viewers.center().x;
            self.viewer(ui, Rect::from_min_max(viewers.min, pos2(mid - 1.0, viewers.bottom())), Which::Source);
            self.viewer(ui, Rect::from_min_max(pos2(mid + 1.0, viewers.top()), viewers.max), Which::Timeline);
        } else {
            self.viewer(ui, viewers, Which::Timeline);
        }
        self.timeline_tools(ui, tl_tools);

        if self.first_layout && self.project.timeline.duration() > 0 {
            self.timeline.zoom_to_fit(self.project.timeline.duration(), tl_area.width());
            self.first_layout = false;
        }
        if self.playing.is_some() {
            self.timeline.follow(self.playhead, tl_area.width());
        }
        let overviews = Overviews { thumbs: &self.thumbs, waves: &self.waves, marks: self.marks };
        let actions = timeline_ui::show(ui, tl_area, &mut self.timeline, &self.project, &self.selection, self.playhead, &overviews);
        for a in actions {
            self.apply(a);
        }
        self.status_toast(ui, tl_area);
        let _ = ctx;
    }

    // ------------------------------------------------------------ media pool

    fn media_pool(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter_at(rect);
        let header = Rect::from_min_size(rect.min, vec2(rect.width(), HEADER_H));
        painter.rect_filled(header, 0.0, HEADER_BG);
        let cy = header.center().y;
        let ib = |x: f32| Rect::from_center_size(pos2(header.left() + x, cy), vec2(18.0, 18.0));
        if icons::button(ui, ib(18.0), Icon::Sidebar, self.show_bins, true, "Show / hide bins").clicked() {
            self.run(Cmd::ToggleBins, &ctx);
        }
        if icons::button(ui, ib(50.0), Icon::Import, false, true, "Import media").clicked() {
            self.run(Cmd::ImportMedia, &ctx);
        }
        painter.text(pos2(header.left() + 72.0, cy), Align2::LEFT_CENTER, "Master", semibold(14.0), TEXT);
        if icons::button(ui, ib(rect.width() - 90.0), Icon::Grid, !self.pool_list, true, "Thumbnail view").clicked() {
            self.pool_list = false;
        }
        if icons::button(ui, ib(rect.width() - 62.0), Icon::List, self.pool_list, true, "List view").clicked() {
            self.pool_list = true;
        }
        icons::button(ui, ib(rect.width() - 34.0), Icon::Search, false, false, "Search");
        icons::button(ui, ib(rect.width() - 118.0), Icon::Sort, false, false, "Sort");
        painter.line_segment([header.left_bottom(), header.right_bottom()], Stroke::new(1.0, DIVIDER));

        let content = Rect::from_min_max(pos2(rect.left(), header.bottom() + 1.0), rect.max);
        let bins_w = if self.show_bins { BINS_W } else { 0.0 };
        if self.show_bins {
            let bins = Rect::from_min_max(content.min, pos2(content.left() + bins_w, content.bottom()));
            painter.rect_filled(bins, 0.0, BINS_BG);
            painter.text(pos2(bins.left() + 12.0, bins.top() + 14.0), Align2::LEFT_CENTER, "Bins", semibold(13.0), TEXT_DIM);
            let row = Rect::from_min_size(pos2(bins.left(), bins.top() + 28.0), vec2(bins_w, 24.0));
            painter.rect_filled(row, 0.0, Color32::from_rgb(52, 51, 58));
            painter.text(pos2(row.left() + 14.0, row.center().y), Align2::LEFT_CENTER, "⏷  Master", regular(13.0), TEXT);
            painter.text(pos2(bins.left() + 12.0, bins.bottom() - 40.0), Align2::LEFT_CENTER, "Smart Bins", semibold(13.0), TEXT_DIM);
            painter.line_segment([pos2(bins.right(), bins.top()), pos2(bins.right(), bins.bottom())], Stroke::new(1.0, DIVIDER));
        }
        let grid = Rect::from_min_max(pos2(content.left() + bins_w + 1.0, content.top()), content.max);
        painter.rect_filled(grid, 0.0, POOL_BG);

        if self.project.media.is_empty() {
            painter.text(grid.center() - vec2(0.0, 24.0), Align2::CENTER_CENTER, "No clips in media pool", regular(25.0), EMPTY_TITLE);
            painter.text(grid.center() + vec2(0.0, 14.0), Align2::CENTER_CENTER, "Import media or drop files here to get started", regular(13.0), EMPTY_SUB);
            let resp = ui.interact(grid, ui.scope_id().with("pool-empty"), Sense::click());
            if resp.double_clicked() {
                self.run(Cmd::ImportMedia, &ctx);
            }
            return;
        }

        let mut place = None;
        let mut remove = None;
        let media = self.project.media.clone();
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(grid.shrink(8.0)));
        egui::ScrollArea::vertical().id_salt("pool-scroll").show(&mut child, |ui| {
            if self.pool_list {
                for m in &media {
                    let selected = self.media_selection == Some(m.id);
                    let resp = ui
                        .dnd_drag_source(egui::Id::unique(("media", m.id)), MediaDrag(m.id), |ui| {
                            ui.horizontal(|ui| {
                                ui.set_min_width(grid.width() - 30.0);
                                let name = RichText::new(&m.name).size(13.0);
                                ui.label(if selected { name.color(SELECT_ORANGE) } else { name });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(RichText::new(media_details(m)).size(11.0).color(TEXT_DIM));
                                });
                            });
                        })
                        .response;
                    self.pool_item_interaction(ui, resp, m, &mut place, &mut remove);
                }
            } else {
                let cell = vec2(132.0, 98.0);
                let cols = ((grid.width() - 16.0) / (cell.x + 8.0)).floor().max(1.0) as usize;
                for row in media.chunks(cols) {
                    ui.horizontal(|ui| {
                        for m in row {
                            let resp = ui
                                .dnd_drag_source(egui::Id::unique(("media", m.id)), MediaDrag(m.id), |ui| {
                                    let (r, _) = ui.allocate_exact_size(cell, Sense::hover());
                                    self.paint_pool_tile(ui, r, m);
                                })
                                .response;
                            self.pool_item_interaction(ui, resp, m, &mut place, &mut remove);
                        }
                    });
                    ui.add_space(6.0);
                }
            }
        });
        if let Some(m) = place {
            self.place_media(m, self.playhead, None);
        }
        if let Some(m) = remove {
            self.edit("Remove media", |p| p.remove_media(m));
        }
    }

    fn paint_pool_tile(&self, ui: &egui::Ui, r: Rect, m: &MediaItem) {
        let painter = ui.painter();
        let pic = Rect::from_min_size(r.min, vec2(r.width(), 74.0));
        let selected = self.media_selection == Some(m.id);
        painter.rect_filled(pic, 3.0, Color32::from_rgb(20, 20, 24));
        if let Some(tex) = self.thumbs.get(&m.id) {
            let size = tex.size_vec2();
            let s = (pic.width() / size.x).min(pic.height() / size.y);
            painter.image(tex.id(), Rect::from_center_size(pic.center(), size * s), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        } else if m.info.video.is_none() {
            // Audio-only media: a waveform badge.
            painter.rect_filled(pic, 3.0, Color32::from_rgb(24, 52, 40));
            if let Some(w) = self.waves.get(&m.id) {
                let n = w.len().max(1);
                for i in 0..(pic.width() as usize) {
                    let v = w[(i * n / pic.width() as usize).min(n - 1)];
                    let x = pic.left() + i as f32;
                    let h = v * (pic.height() / 2.0 - 4.0);
                    painter.line_segment([pos2(x, pic.center().y - h), pos2(x, pic.center().y + h)], Stroke::new(1.0, AUDIO_WAVE));
                }
            } else {
                icons::paint(painter, Rect::from_center_size(pic.center(), vec2(28.0, 28.0)), Icon::PageAudio, AUDIO_WAVE);
            }
        }
        if self.failures.contains_key(&m.id) || m.info.format_name == "offline" {
            painter.rect_filled(pic, 3.0, Color32::from_rgba_unmultiplied(120, 20, 20, 140));
            painter.text(pic.center(), Align2::CENTER_CENTER, "Media Offline", semibold(12.0), Color32::WHITE);
        }
        if selected {
            painter.rect_stroke(pic, 3.0, Stroke::new(2.0, SELECT_ORANGE), egui::StrokeKind::Inside);
        }
        let name = elide(painter, &m.name, regular(11.5), r.width());
        painter.text(pos2(r.center().x, pic.bottom() + 12.0), Align2::CENTER_CENTER, name, regular(11.5), if selected { SELECT_ORANGE } else { TEXT });
    }

    fn pool_item_interaction(&mut self, ui: &egui::Ui, resp: egui::Response, m: &MediaItem, place: &mut Option<Id>, remove: &mut Option<Id>) {
        let resp = ui.interact(resp.rect, egui::Id::unique(("media-click", m.id)), Sense::click());
        let resp = match self.failures.get(&m.id) {
            Some(err) => resp.on_hover_text(format!("Offline: {err}\n{}", m.path.display())),
            None => resp.on_hover_text(format!("{}\n{}", m.path.display(), media_details(m))),
        };
        if resp.clicked() {
            self.media_selection = Some(m.id);
            self.source_pos = 0.0;
        }
        if resp.double_clicked() {
            *place = Some(m.id);
        }
        resp.context_menu(|ui| {
            if ui.button("Insert at Playhead").clicked() {
                *place = Some(m.id);
            }
            if ui.button("Remove from Media Pool").clicked() {
                *remove = Some(m.id);
            }
        });
    }

    // ------------------------------------------------------------ viewers

    fn viewer(&mut self, ui: &mut egui::Ui, rect: Rect, which: Which) {
        let ctx = ui.ctx().clone();
        let fps = self.fps();
        let painter = ui.painter_at(rect);
        let header = Rect::from_min_size(rect.min, vec2(rect.width(), HEADER_H));
        let transport = Rect::from_min_max(pos2(rect.left(), rect.bottom() - TRANSPORT_H), rect.max);
        let scrub = Rect::from_min_max(pos2(rect.left(), transport.top() - SCRUB_H), pos2(rect.right(), transport.top()));
        let image_area = Rect::from_min_max(pos2(rect.left(), header.bottom() + 1.0), pos2(rect.right(), scrub.top()));
        painter.rect_filled(header, 0.0, HEADER_BG);
        painter.rect_filled(image_area, 0.0, VIEWER_BG);
        painter.rect_filled(scrub, 0.0, TRANSPORT_BG);
        painter.rect_filled(transport, 0.0, TRANSPORT_BG);

        // What this viewer shows, its length and position.
        let source_media = self.media_selection.and_then(|id| self.project.media(id).cloned());
        let (title, length, pos) = match which {
            Which::Timeline => (self.project.timeline.name.clone(), self.project.timeline.duration(), self.playhead),
            Which::Source => match &source_media {
                Some(m) => {
                    let len = self.project.media_frames(m.id).unwrap_or_else(|| seconds_to_frames(5.0, fps));
                    (m.name.clone(), len, seconds_to_frames(self.source_pos, fps))
                }
                None => (String::new(), 0, 0),
            },
        };
        let tc0 = if which == Which::Timeline { self.project.timeline.start_timecode } else { 0 };
        painter.text(pos2(header.left() + 12.0, header.center().y), Align2::LEFT_CENTER, "Fit", regular(13.0), TEXT_DIM);
        painter.text(pos2(header.left() + 52.0, header.center().y), Align2::LEFT_CENTER, format_timecode(length, fps), regular(13.0), TEXT_DIM);
        painter.text(header.center(), Align2::CENTER_CENTER, &title, semibold(14.0), TEXT);
        painter.text(pos2(header.right() - 12.0, header.center().y), Align2::RIGHT_CENTER, format_timecode(pos + tc0, fps), semibold(14.0), TIMECODE);

        // Picture.
        let s = &self.project.settings;
        let aspect = s.width as f32 / s.height.max(1) as f32;
        let avail = image_area.shrink(8.0);
        let (mut w, mut h) = (avail.width(), avail.width() / aspect);
        if h > avail.height() {
            h = avail.height();
            w = h * aspect;
        }
        let pic = Rect::from_center_size(avail.center(), vec2(w.max(2.0), h.max(2.0)));
        let ppp = ctx.pixels_per_point();
        let scale = ((w * ppp) / s.width.max(1) as f32).min(1.0);
        let pw = ((s.width as f32 * scale) as u32).max(2) & !1;
        let ph = ((s.height as f32 * scale) as u32).max(2) & !1;
        match which {
            Which::Timeline => {
                if let Some(frame) = self.preview.poll() {
                    upload(&ctx, &mut self.texture, "viewer", &frame.image);
                    self.failures = frame.failures;
                }
                self.preview.request(&self.snapshot, self.version, self.playhead, pw, ph);
                if let Some(t) = &self.texture {
                    painter.image(t.id(), pic, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
            }
            Which::Source => {
                if let Some(m) = &source_media {
                    let snap = match &self.source_snapshot {
                        Some((id, p)) if *id == m.id && p.settings == self.project.settings => p.clone(),
                        _ => {
                            let p = Arc::new(source_project(&self.project, m));
                            self.source_snapshot = Some((m.id, p.clone()));
                            p
                        }
                    };
                    if let Some(frame) = self.source_preview.poll() {
                        upload(&ctx, &mut self.source_texture, "source", &frame.image);
                    }
                    let key = self.version ^ (m.id.as_u128() as u64);
                    self.source_preview.request(&snap, key, pos, pw, ph);
                    if let Some(t) = &self.source_texture {
                        painter.image(t.id(), pic, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    }
                    // Drag the source picture onto the timeline.
                    ui.dnd_drag_source(egui::Id::unique(("source-drag", m.id)), MediaDrag(m.id), |ui| {
                        ui.allocate_rect(pic, Sense::hover());
                    });
                    if m.info.video.is_none() {
                        painter.text(pic.center(), Align2::CENTER_CENTER, "Audio only", regular(16.0), TEXT_DIM);
                    }
                } else {
                    painter.text(image_area.center(), Align2::CENTER_CENTER, "Select a clip in the media pool", regular(14.0), TEXT_FAINT);
                }
            }
        }

        // Scrubber.
        let track = scrub.shrink2(vec2(14.0, 7.0));
        painter.line_segment([track.left_center(), track.right_center()], Stroke::new(2.0, Color32::from_rgb(58, 57, 63)));
        let scrub_resp = ui.interact(scrub, ui.scope_id().with(("scrub", which as u8)), Sense::click_and_drag());
        if length > 0 {
            let x = track.left() + track.width() * (pos as f32 / length as f32).clamp(0.0, 1.0);
            painter.add(egui::Shape::convex_polygon(vec![pos2(x - 5.0, track.top() - 2.0), pos2(x + 5.0, track.top() - 2.0), pos2(x, track.bottom() + 1.0)], PLAYHEAD, Stroke::NONE));
            if (scrub_resp.dragged() || scrub_resp.clicked())
                && let Some(p) = scrub_resp.interact_pointer_pos() {
                    let f = (((p.x - track.left()) / track.width()).clamp(0.0, 1.0) * length as f32) as i64;
                    match which {
                        Which::Timeline => self.seek(f),
                        Which::Source => self.source_pos = frames_to_seconds(f, fps),
                    }
                }
        }

        // Transport, centred, as in the reference.
        let cy = transport.center().y;
        let icons_row: [(Icon, Cmd, &str); 6] = [
            (Icon::ToStart, Cmd::ToStart, "Go to first frame"),
            (Icon::PlayReverse, Cmd::PlayReverse, "Play reverse (J)"),
            (Icon::Stop, Cmd::Pause, "Stop (K)"),
            (if self.playing.as_ref().is_some_and(|p| p.speed > 0.0) && which == Which::Timeline { Icon::Pause } else { Icon::Play }, Cmd::PlayStop, "Play (Space / L)"),
            (Icon::ToEnd, Cmd::ToEnd, "Go to last frame"),
            (Icon::Loop, Cmd::NotYet, "Loop"),
        ];
        let spacing = 44.0;
        let x0 = transport.center().x - spacing * 2.5;
        for (i, (icon, cmd, tip)) in icons_row.iter().enumerate() {
            let r = Rect::from_center_size(pos2(x0 + spacing * i as f32, cy), vec2(22.0, 22.0));
            let enabled = *cmd != Cmd::NotYet;
            if icons::button(ui, r, *icon, false, enabled, tip).clicked() {
                match which {
                    Which::Timeline => self.run(*cmd, &ctx),
                    Which::Source => self.source_transport(*cmd, length),
                }
            }
        }
        icons::button(ui, Rect::from_center_size(pos2(transport.left() + 26.0, cy), vec2(18.0, 18.0)), Icon::Speaker, false, false, "Audio playback is not available yet");
        if which == Which::Source
            && let Some(m) = &source_media {
                let id = m.id;
                let r = Rect::from_center_size(pos2(transport.right() - 70.0, cy), vec2(120.0, 26.0));
                if ui.put(r, egui::Button::new(RichText::new("Insert ▸").size(13.0))).on_hover_text("Overwrite this clip onto the timeline at the playhead").clicked() {
                    self.place_media(id, self.playhead, None);
                }
            }
        painter.line_segment([pos2(rect.left(), rect.bottom()), pos2(rect.right(), rect.bottom())], Stroke::new(1.0, DIVIDER));
    }

    fn source_transport(&mut self, cmd: Cmd, length: i64) {
        let fps = self.fps();
        let pos = seconds_to_frames(self.source_pos, fps);
        let f = match cmd {
            Cmd::ToStart => 0,
            Cmd::ToEnd => length.saturating_sub(1),
            Cmd::PlayReverse => pos - fps.nominal(),
            Cmd::PlayStop => pos + fps.nominal(),
            _ => pos,
        };
        self.source_pos = frames_to_seconds(f.clamp(0, length.max(1) - 1), fps);
    }

    // ------------------------------------------------------------ timeline tool row

    fn timeline_tools(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, HEADER_BG);
        let cy = rect.center().y;
        let at = |x: f32| Rect::from_center_size(pos2(rect.left() + x, cy), vec2(20.0, 20.0));
        icons::button(ui, at(24.0), Icon::List, false, false, "Timeline view options");
        let center = rect.left() + timeline_ui::HEADER_W + 140.0;
        let tool = self.timeline.tool;
        let select = Rect::from_center_size(pos2(center, cy), vec2(20.0, 20.0));
        let resp = ui.interact(select, ui.scope_id().with("tool-select"), Sense::click()).on_hover_text("Selection mode (A)");
        icons::paint(&painter, select, Icon::Select, if tool == Tool::Select { TOOL_RED } else if resp.hovered() { ICON_HOVER } else { ICON });
        if resp.clicked() {
            self.run(Cmd::SelectTool, &ctx);
        }
        icons::button(ui, Rect::from_center_size(pos2(center + 40.0, cy), vec2(20.0, 20.0)), Icon::Trim, false, false, "Trim edit mode");
        if icons::button(ui, Rect::from_center_size(pos2(center + 80.0, cy), vec2(20.0, 20.0)), Icon::Blade, tool == Tool::Blade, true, "Blade edit mode (B)").clicked() {
            self.run(Cmd::BladeTool, &ctx);
        }
        painter.line_segment([pos2(center + 108.0, cy - 10.0), pos2(center + 108.0, cy + 10.0)], Stroke::new(1.0, SEPARATOR));
        if icons::button(ui, Rect::from_center_size(pos2(center + 136.0, cy), vec2(20.0, 20.0)), Icon::Snapping, self.timeline.snapping, true, "Snapping (N)").clicked() {
            self.run(Cmd::ToggleSnapping, &ctx);
        }
        icons::button(ui, Rect::from_center_size(pos2(center + 176.0, cy), vec2(20.0, 20.0)), Icon::Link, true, false, "Linked selection");
        icons::button(ui, Rect::from_center_size(pos2(center + 216.0, cy), vec2(20.0, 20.0)), Icon::PositionLock, false, false, "Position lock");
        painter.line_segment([pos2(center + 244.0, cy - 10.0), pos2(center + 244.0, cy + 10.0)], Stroke::new(1.0, SEPARATOR));
        icons::button(ui, Rect::from_center_size(pos2(center + 272.0, cy), vec2(18.0, 18.0)), Icon::Flag, false, false, "Flags");
        icons::button(ui, Rect::from_center_size(pos2(center + 312.0, cy), vec2(18.0, 18.0)), Icon::Marker, false, false, "Markers");
        painter.line_segment([pos2(center + 340.0, cy - 10.0), pos2(center + 340.0, cy + 10.0)], Stroke::new(1.0, SEPARATOR));
        if icons::button(ui, Rect::from_center_size(pos2(center + 368.0, cy), vec2(20.0, 20.0)), Icon::ZoomFit, false, true, "Zoom to fit (Shift+Z)").clicked() {
            self.run(Cmd::ZoomFit, &ctx);
        }
        if icons::button(ui, Rect::from_center_size(pos2(center + 404.0, cy), vec2(20.0, 20.0)), Icon::ZoomDetail, false, true, "Zoom in (Ctrl+=)").clicked() {
            self.run(Cmd::ZoomIn, &ctx);
        }
        // Zoom slider.
        let slider = Rect::from_center_size(pos2(center + 500.0, cy), vec2(130.0, 20.0));
        let mut z = self.timeline.px_per_frame.ln();
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(slider));
        child.spacing_mut().slider_width = 130.0;
        if child.add(egui::Slider::new(&mut z, (0.02f32).ln()..=(40f32).ln()).show_value(false)).changed() {
            self.timeline.px_per_frame = z.exp();
        }
        // Monitor level (shown, not functional yet), as in the reference.
        icons::button(ui, Rect::from_center_size(pos2(rect.right() - 200.0, cy), vec2(18.0, 18.0)), Icon::Speaker, false, false, "Audio playback is not available yet");
        painter.line_segment([pos2(rect.right() - 180.0, cy), pos2(rect.right() - 80.0, cy)], Stroke::new(2.0, Color32::from_rgb(40, 110, 70)));
        painter.circle_filled(pos2(rect.right() - 80.0, cy), 5.0, ICON);
        let dim = Rect::from_center_size(pos2(rect.right() - 38.0, cy), vec2(44.0, 20.0));
        painter.rect_stroke(dim, 9.0, Stroke::new(1.0, ICON_DIM), egui::StrokeKind::Inside);
        painter.text(dim.center(), Align2::CENTER_CENTER, "DIM", regular(11.0), TEXT_FAINT);
    }

    // ------------------------------------------------------------ inspector

    fn inspector(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, INSPECTOR_BG);
        let header = Rect::from_min_size(rect.min, vec2(rect.width(), HEADER_H));
        painter.rect_filled(header, 0.0, HEADER_BG);
        painter.text(pos2(header.left() + 14.0, header.center().y), Align2::LEFT_CENTER, "Inspector", semibold(14.0), TEXT);
        painter.line_segment([header.left_bottom(), header.right_bottom()], Stroke::new(1.0, DIVIDER));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(pos2(rect.left() + 14.0, header.bottom() + 12.0), rect.max - vec2(14.0, 8.0))));
        let ui = &mut child;

        let ids: Vec<Id> = self.selection.iter().copied().collect();
        let first_video = ids.iter().find(|id| {
            self.project.timeline.find_clip(**id).is_some_and(|(t, _)| self.project.timeline.tracks[t].kind == TrackKind::Video)
        });
        let Some(&id) = first_video.or(ids.first()) else {
            ui.label(RichText::new("Nothing to inspect").color(TEXT_FAINT).size(15.0));
            return;
        };
        let Some(clip) = self.project.timeline.clip(id).cloned() else { return };
        let kind = self.project.timeline.find_clip(id).map(|(t, _)| self.project.timeline.tracks[t].kind);
        ui.label(RichText::new(&clip.name).strong().size(15.0));
        let fps = self.fps();
        ui.label(RichText::new(format!("Duration {}", format_timecode(clip.duration, fps))).color(TEXT_DIM).size(12.0));
        ui.add_space(10.0);
        let mut c = clip.clone();
        ui.checkbox(&mut c.enabled, "Clip enabled");
        ui.add_space(6.0);
        if kind == Some(TrackKind::Video) {
            section(ui, "Transform");
            egui::Grid::new("xform").num_columns(2).spacing(vec2(14.0, 8.0)).show(ui, |ui| {
                ui.label("Zoom");
                ui.add(egui::Slider::new(&mut c.transform.zoom, 0.1..=4.0));
                ui.end_row();
                ui.label("Position X");
                ui.add(egui::DragValue::new(&mut c.transform.pan_x).speed(1.0));
                ui.end_row();
                ui.label("Position Y");
                ui.add(egui::DragValue::new(&mut c.transform.pan_y).speed(1.0));
                ui.end_row();
            });
            section(ui, "Composite");
            egui::Grid::new("comp").num_columns(2).spacing(vec2(14.0, 8.0)).show(ui, |ui| {
                ui.label("Opacity");
                ui.add(egui::Slider::new(&mut c.transform.opacity, 0.0..=1.0));
                ui.end_row();
            });
            if ui.button("Reset").clicked() {
                c.transform = Default::default();
            }
        } else {
            section(ui, "Volume");
            let mut db = if c.volume > 0.0 { 20.0 * c.volume.log10() } else { -60.0 };
            if ui.add(egui::Slider::new(&mut db, -60.0..=12.0).suffix(" dB")).changed() {
                c.volume = if db <= -60.0 { 0.0 } else { 10f32.powf(db / 20.0) };
            }
        }
        let pointer_down = ui.ctx().input(|i| i.pointer.any_down());
        if c != clip {
            // One undo step per slider drag or edit gesture.
            if !self.adjusting {
                self.history.checkpoint("Adjust clip", &self.project);
                self.adjusting = pointer_down;
            }
            if let Some(target) = self.project.timeline.clip_mut(id) {
                *target = c;
            }
            self.touched();
        }
        if !pointer_down {
            self.adjusting = false;
        }
    }

    /// The last status message, shown briefly over the timeline.
    pub(crate) fn status_toast(&mut self, ui: &mut egui::Ui, area: Rect) {
        let now = ui.input(|i| i.time);
        let id = ui.scope_id().with("status-toast");
        let (last, since) = ui.ctx().data_mut(|d| d.get_temp::<(String, f64)>(id)).unwrap_or_default();
        let since = if last != self.status {
            ui.ctx().data_mut(|d| d.insert_temp(id, (self.status.clone(), now)));
            now
        } else {
            since
        };
        let age = now - since;
        if age < 5.0 && self.status != "Ready" {
            let alpha = ((5.0 - age) as f32).min(1.0);
            let painter = ui.painter_at(area);
            let g = painter.layout_no_wrap(self.status.clone(), regular(13.0), TEXT);
            let r = Rect::from_min_size(pos2(area.right() - g.size().x - 36.0, area.bottom() - 40.0), g.size() + vec2(20.0, 12.0));
            painter.rect_filled(r, 4.0, Color32::from_rgba_unmultiplied(12, 12, 14, (220.0 * alpha) as u8));
            painter.galley(r.min + vec2(10.0, 6.0), g, Color32::WHITE.gamma_multiply(alpha));
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

/// Shortens `text` with an ellipsis so it fits in `width`.
fn elide(painter: &egui::Painter, text: &str, font: egui::FontId, width: f32) -> String {
    let fits = |t: &str| painter.layout_no_wrap(t.to_owned(), font.clone(), Color32::WHITE).size().x <= width;
    if fits(text) {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    for keep in (1..chars.len()).rev() {
        let candidate: String = chars[..keep].iter().collect::<String>() + "…";
        if fits(&candidate) {
            return candidate;
        }
    }
    "…".into()
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).strong().size(14.0));
    ui.separator();
}

fn upload(ctx: &egui::Context, slot: &mut Option<egui::TextureHandle>, name: &str, img: &picasso_media::RgbaImage) {
    let ci = egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.data);
    match slot {
        Some(t) => t.set(ci, TextureOptions::LINEAR),
        None => *slot = Some(ctx.load_texture(name, ci, TextureOptions::LINEAR)),
    }
}

pub(crate) fn media_details(m: &MediaItem) -> String {
    let mut details = Vec::new();
    if let Some(v) = &m.info.video {
        details.push(format!("{} {}×{}", v.codec.to_uppercase(), v.width, v.height));
        if !v.is_still {
            details.push(format!("{} fps", v.fps));
        }
    }
    if let Some(a) = &m.info.audio {
        details.push(format!("{} {} ch", a.codec.to_uppercase(), a.channels));
    }
    if m.info.duration_secs > 0.0 {
        details.push(format!("{:.1} s", m.info.duration_secs));
    }
    details.join(" · ")
}

/// A one-clip project used to show a media item in the source viewer.
fn source_project(project: &Project, media: &MediaItem) -> Project {
    let mut p = Project::new("source", project.settings.clone());
    p.media.push(media.clone());
    let (v, _) = p.default_tracks();
    let _ = p.place_media(media.id, 0, Some(v), None);
    p
}
