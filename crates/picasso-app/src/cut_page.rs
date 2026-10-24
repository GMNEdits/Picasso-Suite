//! The Cut page, laid out on the 1872-point grid measured from the reference
//! capture (`reference/target.png`, 1872×999). Rows: icon toolbar 40–90,
//! panel toolbar 93–133, pool/viewer/meter 136–519, transport 522–583,
//! overview timeline 587–711, detail timeline 714–959 (stretches with the
//! window), page bar below.

use crate::app::PicassoApp;
use crate::commands::Cmd;
use crate::icons::{self, Icon};
use crate::theme::*;
use crate::timeline_ui::MediaDrag;
use crate::workers::WAVE_PER_SECOND;
use egui::{Align2, Color32, Rect, Sense, Stroke, TextureOptions, pos2, vec2};
use picasso_core::time::format_timecode;
use picasso_core::{Id, TrackKind};

const POOL_RIGHT: f32 = 594.0;
const VIEWER_RIGHT: f32 = 1777.0;
const OVERVIEW_LEFT: f32 = 294.0;
const DETAIL_LEFT: f32 = 339.0;
const PLAYHEAD_X: f32 = 1106.0;
/// Detail timeline scale from the reference: 479 points per 2 seconds.
const DETAIL_PX_PER_SEC: f32 = 239.5;

const TOOLBAR2_BG: Color32 = Color32::from_rgb(34, 33, 38);
const CELL_LINE: Color32 = Color32::from_rgb(17, 17, 21);
const VIEWER_CUT_BG: Color32 = Color32::from_rgb(26, 26, 26);
const METER_BG: Color32 = Color32::from_rgb(34, 33, 38);
const METER_BAR: Color32 = Color32::from_rgb(39, 38, 45);
const OVERVIEW_BG: Color32 = Color32::from_rgb(30, 30, 32);
const OVERVIEW_PANEL: Color32 = Color32::from_rgb(34, 33, 38);
const DETAIL_BG_PAST: Color32 = Color32::from_rgb(25, 24, 28);
const DETAIL_BG: Color32 = Color32::from_rgb(20, 19, 22);
const DETAIL_PANEL: Color32 = Color32::from_rgb(32, 31, 36);
const RULER_LABEL_OVERVIEW: Color32 = Color32::from_rgb(146, 145, 148);
const RULER_LABEL_DETAIL: Color32 = Color32::from_rgb(70, 70, 72);
const PLAYHEAD_GRAY: Color32 = Color32::from_rgb(147, 147, 147);
const TIMECODE_BOX_BG: Color32 = Color32::from_rgb(14, 14, 16);

impl PicassoApp {
    /// Draws everything between the title row and the page bar.
    pub(crate) fn cut_page(&mut self, ui: &mut egui::Ui, full: Rect, page_top: f32) {
        let ctx = ui.ctx().clone();
        let x0 = full.left();
        let y0 = full.top();
        let p = |x: f32, y: f32| pos2(x0 + x, y0 + y);
        let r = |a: f32, b: f32, c: f32, d: f32| Rect::from_min_max(p(a, b), p(c, d));

        // Vertical layout: everything above the detail timeline is fixed;
        // extra height goes to the viewer row and the detail timeline.
        let extra = (page_top - y0 - 960.0).max(-120.0);
        let grow_main = extra * 0.55;
        let main_bottom = 519.0 + grow_main;
        let transport_top = main_bottom + 3.0;
        let transport_bottom = transport_top + 61.0;
        let overview_top = transport_bottom + 3.0;
        let overview_bottom = overview_top + 124.0;
        let detail_top = overview_bottom + 3.0;
        let detail_bottom = page_top - y0 - 1.0;
        let painter = ui.painter_at(full);

        // ---------------- icon toolbar (40–90)
        let tb = r(0.0, 40.0, 1872.0, 90.0);
        painter.rect_filled(tb, 0.0, TOOLBAR_BG);
        let cy = tb.center().y;
        let tb_icons: [(f32, Icon, &str, Option<Cmd>); 6] = [
            (44.0, Icon::MediaPool, "Media Pool", Some(Cmd::ToggleMediaPool)),
            (116.0, Icon::SyncBin, "Sync Bin", None),
            (193.0, Icon::SourceTape, "Source Tape", None),
            (266.0, Icon::Titles, "Titles", None),
            (342.0, Icon::Effects, "Effects", None),
            (414.0, Icon::Transitions, "Transitions", None),
        ];
        for (i, (x, icon, tip, cmd)) in tb_icons.iter().enumerate() {
            let rect = Rect::from_center_size(pos2(x0 + x, cy), vec2(30.0, 30.0));
            let active = *cmd == Some(Cmd::ToggleMediaPool) && self.show_pool;
            if icons::button(ui, rect, *icon, active, cmd.is_some(), tip).clicked() {
                if let Some(c) = cmd {
                    self.run(*c, &ctx);
                }
            }
            if i < 5 {
                let sx = x0 + (x + tb_icons[i + 1].0) / 2.0;
                painter.line_segment([pos2(sx, cy - 18.0), pos2(sx, cy + 18.0)], Stroke::new(1.0, CELL_LINE));
            }
        }
        let name = self.project.name.clone();
        painter.text(pos2(x0 + 937.0, cy), Align2::CENTER_CENTER, name, semibold(22.0), Color32::WHITE);
        let right: [(f32, Icon, &str, Option<Cmd>); 4] = [
            (1612.0, Icon::QuickExport, "Quick Export", Some(Cmd::QuickExport)),
            (1687.0, Icon::FullScreen, "Full Screen Window", Some(Cmd::FullScreen)),
            (1761.0, Icon::Mixer, "Audio Mixer", None),
            (1835.0, Icon::Tools, "Tools", None),
        ];
        for (i, (x, icon, tip, cmd)) in right.iter().enumerate() {
            let rect = Rect::from_center_size(pos2(x0 + x, cy), vec2(30.0, 30.0));
            if icons::button(ui, rect, *icon, false, cmd.is_some() && self.cmd_enabled(cmd.unwrap()), tip).clicked() {
                if let Some(c) = cmd {
                    self.run(*c, &ctx);
                }
            }
            if i > 0 {
                let sx = x0 + (x + right[i - 1].0) / 2.0;
                painter.line_segment([pos2(sx, cy - 18.0), pos2(sx, cy + 18.0)], Stroke::new(1.0, CELL_LINE));
            }
        }

        // ---------------- panel toolbar (93–133)
        let tb2 = r(0.0, 93.0, 1872.0, 133.0);
        painter.rect_filled(tb2, 0.0, TOOLBAR2_BG);
        let cy2 = tb2.center().y;
        let small = |x: f32| Rect::from_center_size(pos2(x0 + x, cy2), vec2(24.0, 24.0));
        let chev = |painter: &egui::Painter, x: f32| {
            painter.add(egui::Shape::line(
                vec![pos2(x0 + x - 5.0, cy2 - 2.5), pos2(x0 + x, cy2 + 2.5), pos2(x0 + x + 5.0, cy2 - 2.5)],
                Stroke::new(1.4, ICON),
            ));
        };
        let cell = |painter: &egui::Painter, x: f32| {
            painter.line_segment([pos2(x0 + x, tb2.top()), pos2(x0 + x, tb2.bottom())], Stroke::new(2.0, CELL_LINE));
        };
        if icons::button(ui, small(25.0), Icon::Sidebar, self.show_bins, true, "Bins").clicked() {
            self.run(Cmd::ToggleBins, &ctx);
        }
        chev(&painter, 59.0);
        cell(&painter, 81.0);
        if icons::button(ui, small(107.0), Icon::Import, false, true, "Import Media").clicked() {
            self.run(Cmd::ImportMedia, &ctx);
        }
        chev(&painter, 137.0);
        cell(&painter, 160.0);
        icons::button(ui, small(186.0), Icon::Refresh, false, false, "Sync");
        cell(&painter, 213.0);
        icons::button(ui, small(240.0), Icon::Unlink, false, false, "Unlink");
        cell(&painter, 267.0);
        icons::button(ui, small(293.0), Icon::AddImage, false, false, "Add stills");
        chev(&painter, 328.0);
        cell(&painter, 357.0);
        if icons::button(ui, small(383.0), Icon::Grid, !self.pool_list, true, "Thumbnail / list view").clicked() {
            self.pool_list ^= true;
        }
        chev(&painter, 411.0);
        cell(&painter, 434.0);
        icons::button(ui, small(460.0), Icon::Search, false, false, "Search");
        cell(&painter, 487.0);
        icons::button(ui, small(513.0), Icon::Sort, false, false, "Sort");
        cell(&painter, 540.0);
        icons::button(ui, small(567.0), Icon::Refresh, false, false, "Refresh");
        cell(&painter, POOL_RIGHT);
        icons::button(ui, small(620.0), Icon::ViewImage, false, false, "Source clip");
        icons::button(ui, small(661.0), Icon::ViewFilmstrip, false, false, "Source tape");
        icons::button(ui, small(704.0), Icon::ViewTimeline, true, true, "Timeline");
        cell(&painter, 733.0);
        icons::button(ui, small(764.0), Icon::Grid4, false, false, "Multi-view");
        icons::button(ui, small(807.0), Icon::FiveMinutes, false, false, "Five-minute view");
        cell(&painter, 833.0);
        icons::button(ui, small(859.0), Icon::Antenna, false, false, "Live overwrite");
        cell(&painter, 886.0);
        chev(&painter, 1213.0);
        icons::paint(&painter, small(1313.0), Icon::Clock, ICON_DIM);
        let tc_box = r(1336.0, 101.0, 1444.0, 125.0);
        painter.rect_filled(tc_box, 3.0, TIMECODE_BOX_BG);
        let tc0 = self.project.timeline.start_timecode;
        let fps = self.fps();
        painter.text(tc_box.center(), Align2::CENTER_CENTER, format_timecode(self.project.timeline.duration(), fps), regular(19.0), Color32::WHITE);
        cell(&painter, 1462.0);
        icons::button(ui, small(1492.0), Icon::SafeArea, false, false, "Safe area");
        chev(&painter, 1526.0);
        cell(&painter, 1552.0);
        icons::button(ui, small(1582.0), Icon::HighQuality, false, false, "Playback quality");
        chev(&painter, 1614.0);
        cell(&painter, 1636.0);
        icons::button(ui, small(1664.0), Icon::Overlay, false, false, "Overlays");
        chev(&painter, 1696.0);
        cell(&painter, 1720.0);
        icons::paint(&painter, small(1750.0), Icon::ColorSparkle, ICON);
        painter.rect_filled(r(1778.0, 93.0, 1872.0, 133.0), 0.0, TOOLBAR_BG);
        icons::paint(&painter, small(1826.0), Icon::Speaker, Color32::WHITE);

        // ---------------- pool, viewer, meter (136–main_bottom)
        let main_top = 136.0;
        if self.show_pool {
            let pool = r(0.0, main_top, POOL_RIGHT, main_bottom);
            self.cut_pool(ui, pool);
        }
        let viewer = r(if self.show_pool { POOL_RIGHT + 1.0 } else { 0.0 }, main_top, VIEWER_RIGHT, main_bottom);
        painter.rect_filled(viewer, 0.0, VIEWER_CUT_BG);
        self.cut_viewer(ui, viewer);
        let meter = r(VIEWER_RIGHT + 1.0, main_top, 1872.0, main_bottom);
        painter.rect_filled(meter, 0.0, METER_BG);
        let marks: [(&str, f32); 8] = [("0", 0.0), ("-5", 5.0), ("-10", 10.0), ("-15", 15.0), ("-20", 20.0), ("-30", 30.0), ("-40", 40.0), ("-50", 50.0)];
        let scale_top = main_top + 10.0;
        let scale_bottom = main_bottom - 46.0;
        let db_y = |db: f32| {
            // Measured positions: 0..-20 linear (44 pt per 5 dB), then compressed.
            let t = if db <= 20.0 { db / 20.0 * 0.5 } else { 0.5 + (db - 20.0) / 30.0 * 0.5 };
            scale_top + t * (scale_bottom - scale_top)
        };
        for (label, db) in marks {
            let y = y0 + db_y(db);
            painter.text(pos2(x0 + 1814.0, y), Align2::RIGHT_CENTER, format!("{label} -"), regular(11.0), TEXT_DIM);
        }
        painter.rect_filled(Rect::from_min_max(pos2(x0 + 1829.0, y0 + scale_top), pos2(x0 + 1839.0, y0 + main_bottom - 18.0)), 0.0, METER_BAR);
        painter.rect_filled(Rect::from_min_max(pos2(x0 + 1842.0, y0 + scale_top), pos2(x0 + 1852.0, y0 + main_bottom - 18.0)), 0.0, METER_BAR);

        // ---------------- transport row
        let tr = r(0.0, transport_top, 1872.0, transport_bottom);
        painter.rect_filled(tr, 0.0, TRANSPORT_BG);
        let ty = tr.center().y;
        let at = |x: f32, s: f32| Rect::from_center_size(pos2(x0 + x, ty), vec2(s, s));
        let tchev = |painter: &egui::Painter, x: f32| {
            painter.add(egui::Shape::line(vec![pos2(x0 + x - 5.0, ty - 2.5), pos2(x0 + x, ty + 2.5), pos2(x0 + x + 5.0, ty - 2.5)], Stroke::new(1.4, ICON)));
        };
        if icons::button(ui, at(40.0, 30.0), Icon::SmartInsert, false, true, "Smart insert at playhead").clicked() {
            if let Some(m) = self.media_selection {
                self.place_media(m, self.playhead, None);
            }
        }
        tchev(&painter, 74.0);
        icons::button(ui, at(183.0, 30.0), Icon::Append, false, false, "Append");
        icons::button(ui, at(245.0, 30.0), Icon::RippleOverwrite, false, false, "Ripple overwrite");
        icons::button(ui, at(307.0, 30.0), Icon::CloseUp, false, false, "Close up");
        icons::button(ui, at(369.0, 30.0), Icon::PlaceOnTop, false, false, "Place on top");
        tchev(&painter, 404.0);
        icons::button(ui, at(466.0, 30.0), Icon::Split, false, false, "Source overwrite");
        icons::button(ui, at(528.0, 30.0), Icon::SourceOverwrite, false, false, "Cut to");
        tchev(&painter, 563.0);
        painter.line_segment([pos2(x0 + POOL_RIGHT, tr.top()), pos2(x0 + POOL_RIGHT, tr.bottom())], Stroke::new(2.0, CELL_LINE));
        icons::button(ui, at(638.0, 28.0), Icon::Sliders, false, false, "Tools");
        icons::button(ui, at(703.0, 28.0), Icon::Mic, false, false, "Voice-over");
        icons::button(ui, at(762.0, 28.0), Icon::PoiIn, false, false, "Mark in");
        icons::button(ui, at(809.0, 28.0), Icon::PoiOut, false, false, "Mark out");
        icons::button(ui, at(866.0, 30.0), Icon::PlayFilm, false, false, "Fast review");
        if icons::button(ui, at(918.0, 18.0), Icon::Prev, false, true, "Previous frame").clicked() {
            self.run(Cmd::PrevFrame, &ctx);
        }
        icons::paint(&painter, at(946.0, 18.0), Icon::Dot, ICON);
        if icons::button(ui, at(975.0, 18.0), Icon::Next, false, true, "Next frame").clicked() {
            self.run(Cmd::NextFrame, &ctx);
        }
        let playing_fwd = self.playing.as_ref().is_some_and(|p| p.speed > 0.0);
        let transport: [(f32, Icon, Cmd, &str); 6] = [
            (1028.0, Icon::ToStart, Cmd::ToStart, "Go to first frame"),
            (1089.0, Icon::PlayReverse, Cmd::PlayReverse, "Play reverse (J)"),
            (1142.0, Icon::Stop, Cmd::Pause, "Stop (K)"),
            (1194.0, if playing_fwd { Icon::Pause } else { Icon::Play }, Cmd::PlayStop, "Play (Space)"),
            (1255.0, Icon::ToEnd, Cmd::ToEnd, "Go to last frame"),
            (1320.0, Icon::Loop, Cmd::NotYet, "Loop"),
        ];
        for (x, icon, cmd, tip) in transport {
            if icons::button(ui, at(x, 32.0), icon, false, cmd != Cmd::NotYet, tip).clicked() {
                self.run(cmd, &ctx);
            }
        }
        if icons::button(ui, at(1431.0, 26.0), Icon::NextEdit, false, true, "Next edit").clicked() {
            self.run(Cmd::NextEdit, &ctx);
        }
        if icons::button(ui, at(1490.0, 26.0), Icon::PrevEdit, false, true, "Previous edit").clicked() {
            self.run(Cmd::PrevEdit, &ctx);
        }
        painter.text(pos2(x0 + 1548.0, ty), Align2::LEFT_CENTER, format!("TCG {}", format_timecode(self.playhead + tc0, fps)), semibold(23.0), Color32::WHITE);
        tchev(&painter, 1745.0);
        painter.line_segment([pos2(x0 + VIEWER_RIGHT, tr.top()), pos2(x0 + VIEWER_RIGHT, tr.bottom())], Stroke::new(2.0, CELL_LINE));
        icons::button(ui, at(1826.0, 24.0), Icon::Hamburger, false, false, "Timeline options");

        // ---------------- overview timeline
        let ov = r(0.0, overview_top, 1872.0, overview_bottom);
        painter.rect_filled(ov, 0.0, OVERVIEW_BG);
        let ov_panel = r(0.0, overview_top, OVERVIEW_LEFT - 1.0, overview_bottom);
        painter.rect_filled(ov_panel, 0.0, OVERVIEW_PANEL);
        painter.rect_filled(r(OVERVIEW_LEFT + 1.0, overview_top, 338.0, overview_bottom), 0.0, Color32::from_rgb(32, 31, 36));
        let oy = ov_panel.center().y;
        icons::button(ui, Rect::from_center_size(pos2(x0 + 48.0, oy), vec2(30.0, 30.0)), Icon::TrimTools, false, false, "Timeline actions");
        icons::button(ui, Rect::from_center_size(pos2(x0 + 145.0, oy), vec2(30.0, 30.0)), Icon::TrimRipple, false, false, "Audio trim");
        icons::button(ui, Rect::from_center_size(pos2(x0 + 242.0, oy), vec2(30.0, 30.0)), Icon::TrimAudio, false, false, "Split view");
        self.cut_overview(ui, r(357.0, overview_top, 1872.0, overview_bottom));

        // ---------------- detail timeline
        let dt = r(0.0, detail_top, 1872.0, detail_bottom);
        painter.rect_filled(dt, 0.0, DETAIL_BG);
        let panel = r(0.0, detail_top, DETAIL_LEFT - 1.0, detail_bottom);
        painter.rect_filled(panel, 0.0, DETAIL_PANEL);
        painter.line_segment([pos2(x0, y0 + detail_top + 56.0), pos2(x0 + DETAIL_LEFT - 1.0, y0 + detail_top + 56.0)], Stroke::new(1.0, CELL_LINE));
        let dy = y0 + detail_top + 27.0;
        let dicon = |x: f32| Rect::from_center_size(pos2(x0 + x, dy), vec2(28.0, 28.0));
        if icons::button(ui, dicon(44.0), Icon::ZoomFrame, false, true, "Zoom to fit (Shift+Z)").clicked() {
            self.cut_zoom = DETAIL_PX_PER_SEC;
        }
        icons::button(ui, dicon(106.0), Icon::ZoomDetailDim, false, false, "Detail zoom");
        if icons::button(ui, dicon(168.0), Icon::Blade, false, true, "Split clip at playhead (Ctrl+B)").clicked() {
            self.run(Cmd::Razor, &ctx);
        }
        icons::paint(&painter, dicon(229.0), Icon::BlueMarker, MARKER_BLUE);
        icons::button(ui, dicon(291.0), Icon::Transitions, false, false, "Transitions");
        self.cut_detail(ui, r(DETAIL_LEFT, detail_top, 1872.0, detail_bottom));
        self.status_toast(ui, r(DETAIL_LEFT, detail_top, 1872.0, detail_bottom));
    }

    fn cut_pool(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter_at(rect);
        let header = Rect::from_min_size(rect.min, vec2(rect.width(), 42.0));
        painter.rect_filled(header, 0.0, Color32::from_rgb(34, 33, 38));
        painter.text(pos2(header.left() + 17.0, header.center().y), Align2::LEFT_CENTER, "Master", regular(16.0), TEXT);
        let body = Rect::from_min_max(pos2(rect.left(), header.bottom() + 2.0), rect.max);
        painter.rect_filled(body, 0.0, POOL_BG);
        if self.project.media.is_empty() {
            painter.text(pos2(body.center().x, rect.top() + 198.0), Align2::CENTER_CENTER, "No clips in media pool", regular(42.0), EMPTY_TITLE);
            painter.text(pos2(body.center().x, rect.top() + 276.0), Align2::CENTER_CENTER, "Import media or drop files here to get started", regular(21.0), EMPTY_SUB);
            if ui.interact(body, ui.scope_id().with("cut-pool-empty"), Sense::click()).double_clicked() {
                self.run(Cmd::ImportMedia, &ctx);
            }
            return;
        }
        let media = self.project.media.clone();
        let mut place: Option<Id> = None;
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body.shrink(12.0)));
        egui::ScrollArea::vertical().id_salt("cut-pool").show(&mut child, |ui| {
            let cell = vec2(176.0, 128.0);
            let cols = ((body.width() - 24.0) / (cell.x + 10.0)).floor().max(1.0) as usize;
            for row in media.chunks(cols) {
                ui.horizontal(|ui| {
                    for m in row {
                        let resp = ui
                            .dnd_drag_source(egui::Id::unique(("cut-media", m.id)), MediaDrag(m.id), |ui| {
                                let (r, _) = ui.allocate_exact_size(cell, Sense::hover());
                                let pic = Rect::from_min_size(r.min, vec2(r.width(), 99.0));
                                let painter = ui.painter();
                                painter.rect_filled(pic, 3.0, Color32::from_rgb(20, 20, 24));
                                if let Some(tex) = self.thumbs.get(&m.id) {
                                    let size = tex.size_vec2();
                                    let s = (pic.width() / size.x).min(pic.height() / size.y);
                                    painter.image(tex.id(), Rect::from_center_size(pic.center(), size * s), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                                } else if m.info.video.is_none() {
                                    painter.rect_filled(pic, 3.0, Color32::from_rgb(24, 52, 40));
                                    icons::paint(painter, Rect::from_center_size(pic.center(), vec2(34.0, 34.0)), Icon::PageAudio, AUDIO_WAVE);
                                }
                                if self.media_selection == Some(m.id) {
                                    painter.rect_stroke(pic, 3.0, Stroke::new(2.0, SELECT_ORANGE), egui::StrokeKind::Inside);
                                }
                                let lp = painter.with_clip_rect(r);
                                lp.text(pos2(r.center().x, pic.bottom() + 15.0), Align2::CENTER_CENTER, &m.name, regular(13.0), TEXT);
                            })
                            .response;
                        let resp = ui.interact(resp.rect, egui::Id::unique(("cut-media-click", m.id)), Sense::click()).on_hover_text(m.path.display().to_string());
                        if resp.clicked() {
                            self.media_selection = Some(m.id);
                        }
                        if resp.double_clicked() {
                            place = Some(m.id);
                        }
                    }
                });
                ui.add_space(8.0);
            }
        });
        if let Some(m) = place {
            self.place_media(m, self.playhead, None);
        }
    }

    fn cut_viewer(&mut self, ui: &mut egui::Ui, area: Rect) {
        let ctx = ui.ctx().clone();
        let s = self.project.settings.clone();
        let aspect = s.width as f32 / s.height.max(1) as f32;
        let avail = area.shrink(6.0);
        let (mut w, mut h) = (avail.width(), avail.width() / aspect);
        if h > avail.height() {
            h = avail.height();
            w = h * aspect;
        }
        let pic = Rect::from_center_size(avail.center(), vec2(w.max(2.0), h.max(2.0)));
        let scale = ((w * ctx.pixels_per_point()) / s.width.max(1) as f32).min(1.0);
        let pw = ((s.width as f32 * scale) as u32).max(2) & !1;
        let ph = ((s.height as f32 * scale) as u32).max(2) & !1;
        if let Some(frame) = self.preview.poll() {
            let ci = egui::ColorImage::from_rgba_unmultiplied([frame.image.width as usize, frame.image.height as usize], &frame.image.data);
            match &mut self.texture {
                Some(t) => t.set(ci, TextureOptions::LINEAR),
                None => self.texture = Some(ctx.load_texture("viewer", ci, TextureOptions::LINEAR)),
            }
            self.failures = frame.failures;
        }
        self.preview.request(&self.snapshot, self.version, self.playhead, pw, ph);
        if self.project.timeline.duration() > 0 {
            if let Some(t) = &self.texture {
                ui.painter_at(area).image(t.id(), pic, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
        }
    }

    /// The whole timeline compressed into one strip, with a draggable playhead.
    fn cut_overview(&mut self, ui: &mut egui::Ui, area: Rect) {
        let fps = self.fps();
        let tc0 = self.project.timeline.start_timecode;
        let painter = ui.painter_at(area);
        let duration = self.project.timeline.duration();
        // Labels every 10 s at 351 points apart in the reference (40 s visible).
        let secs_visible = ((duration as f64 / fps.as_f64()) * 1.1).max(43.0);
        let px_per_frame = (area.width() as f64 / (secs_visible * fps.as_f64())) as f32;
        let x_of = |f: i64| area.left() + 3.0 + f as f32 * px_per_frame;
        let ruler = Rect::from_min_size(area.min, vec2(area.width(), 24.0));
        let label_secs = [10, 30, 60, 120, 300, 600].into_iter().find(|s| *s as f32 * fps.as_f64() as f32 * px_per_frame >= 300.0).unwrap_or(1200);
        let step = label_secs * fps.nominal();
        let mut i = 0i64;
        loop {
            let f = i * step / 20;
            if x_of(f) >= area.right() {
                break;
            }
            let x = x_of(f);
            let major = i % 20 == 0;
            let mid = i % 10 == 0;
            painter.line_segment([pos2(x, ruler.top() + 2.0), pos2(x, ruler.top() + if major { 22.0 } else if mid { 12.0 } else { 6.0 })], Stroke::new(1.0, RULER_TICK));
            if major {
                painter.text(pos2(x + 8.0, ruler.top() + 13.0), Align2::LEFT_CENTER, format_timecode(f + tc0, fps), regular(13.5), RULER_LABEL_OVERVIEW);
            }
            i += 1;
        }
        // Clips as compact rows: video on top, audio below.
        let lanes = Rect::from_min_max(pos2(area.left(), ruler.bottom() + 6.0), pos2(area.right(), area.bottom() - 6.0));
        let video: Vec<_> = self.project.timeline.tracks.iter().filter(|t| t.kind == TrackKind::Video).collect();
        let audio: Vec<_> = self.project.timeline.tracks.iter().filter(|t| t.kind == TrackKind::Audio).collect();
        let n = (video.len() + audio.len()).max(1) as f32;
        let lane_h = (lanes.height() / n).min(20.0);
        for (i, t) in video.iter().rev().chain(audio.iter()).enumerate() {
            let y = lanes.top() + i as f32 * lane_h;
            let color = if t.kind == TrackKind::Video { VIDEO_CLIP_LABEL } else { AUDIO_CLIP_LABEL };
            for c in &t.clips {
                let rect = Rect::from_min_max(pos2(x_of(c.start), y + 1.0), pos2(x_of(c.end()).max(x_of(c.start) + 1.0), y + lane_h - 1.0));
                painter.rect_filled(rect, 1.0, if c.enabled && t.enabled { color } else { CLIP_DISABLED });
                if self.selection.contains(&c.id) {
                    painter.rect_stroke(rect, 1.0, Stroke::new(1.5, SELECT_ORANGE), egui::StrokeKind::Inside);
                }
            }
        }
        let resp = ui.interact(area, ui.scope_id().with("cut-overview"), Sense::click_and_drag());
        if resp.clicked() || resp.dragged() {
            if let Some(p) = resp.interact_pointer_pos() {
                let frame = ((p.x - area.left() - 3.0) / px_per_frame).round() as i64;
                self.seek(frame.clamp(0, duration.max(0)));
            }
        }
        // Gray playhead.
        let x = x_of(self.playhead);
        painter.line_segment([pos2(x, area.top() + 6.0), pos2(x, area.bottom())], Stroke::new(1.5, PLAYHEAD_GRAY));
        painter.add(egui::Shape::convex_polygon(
            vec![pos2(x - 8.0, area.top() + 2.0), pos2(x + 8.0, area.top() + 2.0), pos2(x + 8.0, area.top() + 10.0), pos2(x, area.top() + 17.0), pos2(x - 8.0, area.top() + 10.0)],
            PLAYHEAD_GRAY,
            Stroke::NONE,
        ));
    }

    /// The zoomed timeline with a fixed centre playhead; dragging scrubs.
    fn cut_detail(&mut self, ui: &mut egui::Ui, area: Rect) {
        let ctx = ui.ctx().clone();
        let fps = self.fps();
        let tc0 = self.project.timeline.start_timecode;
        let px_per_frame = self.cut_zoom / fps.as_f64() as f32;
        let head_x = area.left() + (PLAYHEAD_X - DETAIL_LEFT);
        let x_of = |f: f64| head_x + ((f - self.playhead as f64) as f32) * px_per_frame;
        let painter = ui.painter_at(area);
        // Before the timeline start the background is lighter, as in the reference.
        let start_x = x_of(0.0).clamp(area.left(), area.right());
        painter.rect_filled(Rect::from_min_max(area.min, pos2(start_x, area.bottom())), 0.0, DETAIL_BG_PAST);
        let ruler = Rect::from_min_size(area.min, vec2(area.width(), 26.0));
        let first = ((self.playhead as f32) - (head_x - area.left()) / px_per_frame).floor() as i64 - 1;
        let last = ((self.playhead as f32) + (area.right() - head_x) / px_per_frame).ceil() as i64 + 1;
        let label_frames = 2 * fps.nominal();
        let minor = (fps.nominal() / 5).max(1);
        let mut f = first.div_euclid(minor) * minor;
        while f <= last {
            let x = x_of(f as f64);
            let major = f.rem_euclid(label_frames) == 0;
            let sec = f.rem_euclid(fps.nominal()) == 0;
            painter.line_segment([pos2(x, ruler.top()), pos2(x, ruler.top() + if major { 26.0 } else if sec { 14.0 } else { 7.0 })], Stroke::new(1.0, RULER_TICK));
            if major {
                painter.text(pos2(x + 9.0, ruler.top() + 26.0), Align2::LEFT_CENTER, format_timecode(f + tc0, fps), regular(14.5), RULER_LABEL_DETAIL);
                painter.line_segment([pos2(x, ruler.bottom()), pos2(x, area.bottom())], Stroke::new(1.0, Color32::from_rgb(28, 27, 32)));
            }
            f += minor;
        }

        // Tracks: video rows above, audio below, no headers (Cut page style).
        let tracks = &self.project.timeline.tracks;
        let mut order: Vec<usize> = (0..tracks.len()).filter(|&i| tracks[i].kind == TrackKind::Video).rev().collect();
        order.extend((0..tracks.len()).filter(|&i| tracks[i].kind == TrackKind::Audio));
        let mut y = ruler.bottom() + 46.0;
        let mut clicked_clip = None;
        for ti in order {
            let t = &tracks[ti];
            let h = if t.kind == TrackKind::Video { 72.0 } else { 46.0 };
            for c in &t.clips {
                let x0 = x_of(c.start as f64);
                let x1 = x_of(c.end() as f64);
                if x1 < area.left() || x0 > area.right() {
                    continue;
                }
                let rect = Rect::from_min_max(pos2(x0, y), pos2(x1, y + h - 2.0));
                let cp = painter.with_clip_rect(rect.intersect(area));
                let off = !c.enabled || !t.enabled;
                let (body, label) = match t.kind {
                    _ if off => (CLIP_DISABLED, Color32::from_rgb(84, 84, 90)),
                    TrackKind::Video => (VIDEO_CLIP, VIDEO_CLIP_LABEL),
                    TrackKind::Audio => (AUDIO_CLIP, AUDIO_CLIP_LABEL),
                };
                cp.rect_filled(rect, 2.0, body);
                if t.kind == TrackKind::Video {
                    if let Some(tex) = self.thumbs.get(&c.media_id) {
                        let pic_h = rect.height() - 18.0;
                        let size = tex.size_vec2();
                        let pic_w = (pic_h * size.x / size.y.max(1.0)).max(4.0);
                        let mut x = rect.left().max(area.left() - pic_w);
                        while x < rect.right().min(area.right()) {
                            cp.image(tex.id(), Rect::from_min_size(pos2(x, rect.top()), vec2(pic_w, pic_h)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                            x += pic_w;
                        }
                    }
                    let lr = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 18.0), rect.max);
                    cp.rect_filled(lr, 0.0, label);
                    cp.text(pos2(lr.left().max(area.left()) + 6.0, lr.center().y), Align2::LEFT_CENTER, &c.name, regular(12.0), Color32::WHITE);
                } else {
                    let lr = Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + 17.0));
                    cp.rect_filled(lr, 0.0, label);
                    cp.text(pos2(lr.left().max(area.left()) + 6.0, lr.center().y), Align2::LEFT_CENTER, &c.name, regular(12.0), Color32::WHITE);
                    if let Some(wave) = self.waves.get(&c.media_id) {
                        let mid = (lr.bottom() + rect.bottom()) / 2.0;
                        let half = (rect.bottom() - lr.bottom()) / 2.0 - 2.0;
                        let mut x = rect.left().max(area.left());
                        while x < rect.right().min(area.right()) {
                            let frame = self.playhead as f64 + ((x - head_x) / px_per_frame) as f64;
                            let secs = (c.source_in as f64 + frame - c.start as f64) / fps.as_f64();
                            let i = (secs.max(0.0) * WAVE_PER_SECOND as f64) as usize;
                            let v = wave.get(i).copied().unwrap_or(0.0) * c.volume;
                            let hh = v.min(1.0) * half;
                            if hh > 0.3 {
                                cp.line_segment([pos2(x, mid - hh), pos2(x, mid + hh)], Stroke::new(1.0, AUDIO_WAVE));
                            }
                            x += 1.0;
                        }
                    }
                }
                if self.selection.contains(&c.id) {
                    cp.rect_stroke(rect, 2.0, Stroke::new(2.0, SELECT_ORANGE), egui::StrokeKind::Inside);
                }
                let hit = rect.intersect(area);
                if hit.width() > 1.0 && ui.interact(hit, ui.scope_id().with(("cut-clip", c.id)), Sense::click()).clicked() {
                    clicked_clip = Some(c.id);
                }
            }
            y += h + 4.0;
        }
        if let Some(id) = clicked_clip {
            let additive = ui.input(|i| i.modifiers.command || i.modifiers.shift);
            self.apply(crate::timeline_ui::Action::Select { id, additive });
        }

        // Dragging the timeline scrubs under the fixed playhead; the wheel zooms.
        let resp = ui.interact(area, ui.scope_id().with("cut-detail"), Sense::click_and_drag());
        if resp.dragged() {
            let dx = resp.drag_delta().x;
            let accum = ui.ctx().data_mut(|d| {
                let v = d.get_temp_mut_or_default::<f32>(ui.scope_id().with("cut-drag"));
                *v -= dx / px_per_frame;
                let whole = v.trunc();
                *v -= whole;
                whole
            });
            if accum != 0.0 {
                self.seek((self.playhead + accum as i64).clamp(0, self.project.timeline.duration()));
            }
        }
        if resp.clicked() && clicked_clip.is_none() {
            self.selection.clear();
        }
        if resp.hovered() {
            let (zoom, scroll) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta));
            if zoom != 1.0 {
                self.cut_zoom = (self.cut_zoom * zoom).clamp(4.0, 4000.0);
            } else if scroll.y != 0.0 || scroll.x != 0.0 {
                let d = if scroll.x != 0.0 { scroll.x } else { scroll.y };
                let frames = (-d / px_per_frame).round() as i64;
                if frames != 0 {
                    self.seek((self.playhead + frames).clamp(0, self.project.timeline.duration()));
                }
            }
        }
        if let Some(payload) = resp.dnd_release_payload::<MediaDrag>() {
            if let Some(p) = ctx.pointer_interact_pos() {
                let frame = self.playhead + ((p.x - head_x) / px_per_frame).round() as i64;
                self.place_media(payload.0, frame.max(0), None);
            }
        }

        // Fixed centre playhead (gray, as in the reference).
        painter.line_segment([pos2(head_x, area.top()), pos2(head_x, area.bottom())], Stroke::new(1.5, PLAYHEAD_GRAY));
        painter.add(egui::Shape::convex_polygon(
            vec![pos2(head_x - 9.0, area.top()), pos2(head_x + 9.0, area.top()), pos2(head_x + 9.0, area.top() + 9.0), pos2(head_x, area.top() + 17.0), pos2(head_x - 9.0, area.top() + 9.0)],
            PLAYHEAD_GRAY,
            Stroke::NONE,
        ));
    }
}
