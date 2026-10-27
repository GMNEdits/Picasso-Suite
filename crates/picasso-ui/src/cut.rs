//! The Cut page: a fast page with the media pool, the viewer, a level meter, a transport row and
//! two timelines, an overview of the whole timeline above and the detail around the playhead
//! below.
//!
//! Laid out on Resolve at 1920 x 1080 (`docs/reference/resolve-1080p/cut.png`): positions are its
//! pixels, scaled by `R`. The media pool and viewer band and the detail timeline share the height
//! the window has beyond the fixed rows. The divider between the pool and the viewer, and the one
//! below the transport row, can be dragged (`crate::panels`).

use egui::{
    pos2, Align2, Color32, Context, Painter, Pos2, Rect, Stroke, StrokeKind, Ui, UiBuilder,
};

use crate::app::StudioApp;
use crate::chrome::{Screen, Tool, ToolbarItem, R};
use crate::icons::{paint_scaled, Icon};
use crate::media::fitting;
use crate::menu::Act;
use crate::panels::{divider, Divider, Panels};
use crate::theme::{self, regular, semibold};
use crate::timeline::Mode;

/// The red of the track names on the Cut page.
const TRACK_RED: Color32 = Color32::from_rgb(221, 74, 62);
/// Icons on this page are drawn at Resolve's size.
const ICON: f32 = 0.6;

/// The labels of the level meter and where they sit, measured from the top of the meter as a
/// share of its height (the reference meter runs from row 75 to 465).
const METER_LABELS: [(&str, f32); 8] = [
    ("0", 0.0),
    ("-5", 0.127),
    ("-10", 0.249),
    ("-15", 0.374),
    ("-20", 0.497),
    ("-30", 0.697),
    ("-40", 0.846),
    ("-50", 0.921),
];

/// Where a level (1.0 is 0 dB) sits on the meter, as a share of its height from the top.
fn level_to_share(level: f32) -> f32 {
    let db = 20.0 * level.max(1e-6).log10();
    let marks: Vec<(f32, f32)> = METER_LABELS
        .iter()
        .filter_map(|(label, y)| Some((label.parse::<f32>().ok()?, *y)))
        .collect();
    let (top, bottom) = (marks[0], marks[marks.len() - 1]);
    if db >= top.0 {
        return top.1;
    }
    if db <= bottom.0 {
        return 1.0;
    }
    for pair in marks.windows(2) {
        let ((d0, y0), (d1, y1)) = (pair[0], pair[1]);
        if db <= d0 && db >= d1 {
            return y0 + (y1 - y0) * (d0 - db) / (d0 - d1);
        }
    }
    bottom.1
}

/// Turns the reference's pixels into points on this window. Rows 0 to 100 (the toolbars and
/// the pool's header) and 478 to 654 (transport, overview and the detail's tools) keep their
/// size; rows 100 to 478 (pool and viewer) and 654 to 1004 (the detail timeline) share the rest,
/// as the panel sizes say. In rows 40 to 523 the pool and the viewer are as wide as they say too.
#[derive(Clone, Copy)]
struct Lay<'a> {
    s: &'a Screen,
    /// Points per reference pixel in the pool and viewer band and in the detail timeline.
    upper: f32,
    lower: f32,
    /// The height in points the two bands share.
    room: f32,
    /// Where the media pool ends, in reference pixels.
    pool: f32,
}

impl<'a> Lay<'a> {
    fn new(s: &'a Screen, panels: &Panels) -> Self {
        let fixed = (100.0 + 176.0) * R;
        let room = ((s.h - 48.0 * R) - 40.0 - fixed).max(0.2 * (378.0 + 350.0));
        let upper = room * f32::from(panels.cut_upper) / 1000.0;
        Lay {
            s,
            upper: upper / 378.0,
            lower: (room - upper) / 350.0,
            room,
            pool: f32::from(panels.cut_pool),
        }
    }
    fn y(&self, r: f32) -> f32 {
        let top = 40.0;
        if r <= 100.0 {
            top + r * R
        } else if r <= 478.0 {
            top + 100.0 * R + (r - 100.0) * self.upper
        } else if r <= 654.0 {
            top + 100.0 * R + 378.0 * self.upper + (r - 478.0) * R
        } else {
            top + 276.0 * R + 378.0 * self.upper + (r - 654.0) * self.lower
        }
    }
    /// A reference x in points: in the rows of the pool and the viewer, the pool's columns are
    /// stretched to where its divider is; the level meter keeps its place.
    fn x(&self, x: f32, y: f32) -> f32 {
        let (pool, meter) = (960.0, 1850.0);
        if !(40.0..523.0).contains(&y) || x >= meter {
            return x * R;
        }
        let x = if x <= pool {
            x * self.pool / pool
        } else {
            self.pool + (x - pool) * (meter - self.pool) / (meter - pool)
        };
        x * R
    }
    fn p(&self, x: f32, y: f32) -> Pos2 {
        self.s.p(self.x(x, y), self.y(y))
    }
    fn r(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(self.p(x0, y0), self.p(x1, y1))
    }
    fn band(&self, p: &Painter, x0: f32, y0: f32, x1: f32, y1: f32, c: Color32) {
        p.rect_filled(self.r(x0, y0, x1, y1), 0.0, c);
    }
    fn vline(&self, p: &Painter, x: f32, y0: f32, y1: f32, c: Color32) {
        p.line_segment([self.p(x, y0), self.p(x, y1)], Stroke::new(1.0, c));
    }
    fn hline(&self, p: &Painter, x0: f32, x1: f32, y: f32, c: Color32) {
        p.line_segment([self.p(x0, y), self.p(x1, y)], Stroke::new(1.0, c));
    }
}

/// A button the size of Resolve's, `w` reference pixels wide.
fn t(hint: &'static str, w: f32) -> Tool {
    Tool::new(hint).wide(w * R).size(ICON)
}

impl StudioApp {
    /// The Cut page's top toolbar.
    pub(crate) fn cut_toolbar1(&mut self, ui: &mut Ui, ctx: &Context, s: &Screen) {
        let it = |icon, label, lit| ToolbarItem {
            icon,
            label,
            lit,
            drop: false,
        };
        let left = [
            it(Icon::MediaImport, "Media Pool", true),
            it(Icon::SyncBin, "Sync Bin", false),
            it(Icon::Mail, "Transitions", false),
            it(Icon::TitleT, "Titles", false),
            it(Icon::Wand, "Effects", false),
            it(Icon::Diamonds, "Keyframes", false),
        ];
        let right = [
            it(Icon::Export, "Quick Export", false),
            it(Icon::Fullscreen, "Full Screen", false),
            it(Icon::Mixer, "Mixer", false),
            it(Icon::Tools, "Inspector", false),
        ];
        match self.labeled_toolbar(ui, s, &left, &right) {
            Some((false, 0)) => self.import_dialog(ctx),
            Some((false, 1)) => self.planned("Sync bin"),
            Some((false, 2)) => self.do_act(ctx, Act::AddCrossDissolve),
            Some((false, 3)) => self.do_act(ctx, Act::AddTitle),
            Some((false, 4)) => self.page = crate::app::Page::Effects,
            Some((false, _)) => self.do_act(ctx, Act::ChangeClipSpeed),
            Some((true, 0)) => self.page = crate::app::Page::Deliver,
            Some((true, 1)) => {
                let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
            }
            Some((true, 2)) => self.page = crate::app::Page::Audio,
            Some((true, _)) => self.show_settings = true,
            None => {}
        }
    }

    pub(crate) fn cut_page(&mut self, ui: &mut Ui, ctx: &Context, s: &Screen) {
        let painter = ui.painter().clone();
        let l = Lay::new(s, &self.extra.prefs.panels);
        let dark = Color32::from_rgb(10, 10, 12);

        self.toolbar2(ui, ctx, &l);
        self.pool_region(ui, ctx, &l);
        self.viewer_region(ui, &l);
        self.meter_region(&painter, &l);
        self.transport_row(ui, &l);

        // The overview: three tools, the track names, then the strip.
        l.band(&painter, 0.0, 523.0, 1919.0, 615.0, theme::RAIL_BG);
        l.hline(&painter, 0.0, 1919.0, 522.5, dark);
        l.vline(&painter, 203.5, 523.0, 615.0, Color32::from_rgb(8, 7, 12));
        l.vline(&painter, 233.5, 523.0, 615.0, dark);
        let rail = Color32::from_rgb(92, 92, 96);
        if self
            .tool(
                ui,
                l.p(35.0, 569.0),
                Icon::RailChecks,
                t("Select clips", 50.0).color(rail),
            )
            .clicked()
        {
            self.do_act(ctx, Act::SelectAll);
        }
        if self
            .tool(
                ui,
                l.p(100.0, 569.0),
                Icon::RailPin,
                t("Cut at markers", 50.0).color(rail),
            )
            .clicked()
        {
            self.cut_at_markers();
        }
        self.tool(ui, l.p(166.0, 569.0), Icon::RailFilm, t("", 50.0).dim());
        ui.scope_builder(
            UiBuilder::new().max_rect(l.r(234.0, 523.0, 1919.0, 615.0)),
            |ui| self.timeline_ui(ui, ctx, Mode::Overview),
        );
        for (name, _, top, h) in &self.cut_rows_overview {
            painter.text(
                pos2(l.p(219.0, 0.0).x, top + h / 2.0),
                Align2::CENTER_CENTER,
                name,
                semibold(10.0 * R),
                TRACK_RED,
            );
        }

        // The detail: the timeline's tools on the left, the timeline on the right.
        let bottom = 1003.0;
        l.hline(&painter, 0.0, 1919.0, 615.5, Color32::from_rgb(6, 5, 10));
        l.band(&painter, 0.0, 616.0, 233.0, bottom, theme::RAIL_BG);
        l.hline(&painter, 0.0, 233.0, 654.5, Color32::from_rgb(20, 20, 24));
        l.vline(&painter, 233.5, 616.0, bottom, Color32::from_rgb(7, 7, 9));
        let y = 635.0;
        if self
            .tool(
                ui,
                l.p(32.0, y),
                Icon::Fit,
                t("Fit the detail to the clips", 40.0).color(theme::ICON_BRIGHT),
            )
            .clicked()
        {
            self.cut_detail = crate::timeline::TimelineView::new(Mode::Detail);
        }
        self.tool(ui, l.p(74.0, y), Icon::TrimTool, t("", 40.0).dim());
        if self
            .tool(
                ui,
                l.p(116.0, y),
                Icon::Scissors,
                t("Split at the playhead (S)", 40.0),
            )
            .clicked()
        {
            self.split_at_playhead();
        }
        if self
            .tool(
                ui,
                l.p(158.0, y),
                Icon::Marker,
                t("Add a marker (M)", 40.0).color(theme::MARKER_BLUE),
            )
            .clicked()
        {
            self.do_act(ctx, Act::AddMarker);
        }
        if self
            .tool(ui, l.p(203.0, y), Icon::Diamonds, t("Transition", 40.0))
            .clicked()
        {
            self.do_act(ctx, Act::AddCrossDissolve);
        }
        ui.scope_builder(
            UiBuilder::new().max_rect(l.r(234.0, 616.0, 1919.0, bottom)),
            |ui| self.timeline_ui(ui, ctx, Mode::Detail),
        );
        self.cut_track_controls(&painter, &l, bottom);
        self.cut_dividers(ui, &l);
    }

    /// The divider between the media pool and the viewer, and the one between the transport row
    /// and the timelines. They move on the next frame.
    fn cut_dividers(&mut self, ui: &mut Ui, l: &Lay) {
        let panels = &mut self.extra.prefs.panels;
        let mut save = false;
        match divider(ui, "cut pool", l.r(960.0, 70.0, 961.0, 522.0), false) {
            Divider::Moved(at) => panels.set_cut_pool((at.x - l.s.o.x) / R),
            Divider::Reset => {
                panels.cut_pool = Panels::default().cut_pool;
                save = true;
            }
            Divider::Released => save = true,
            Divider::Still => {}
        }
        match divider(ui, "cut timeline", l.r(0.0, 522.0, 1919.0, 523.0), true) {
            Divider::Moved(at) => {
                // The divider sits a fixed height below the pool and viewer band.
                let upper = at.y - (l.y(522.5) - 378.0 * l.upper);
                panels.cut_upper = Panels::share(upper / l.room);
            }
            Divider::Reset => {
                panels.cut_upper = Panels::default().cut_upper;
                save = true;
            }
            Divider::Released => save = true,
            Divider::Still => {}
        }
        if save {
            self.extra.prefs.save();
        }
    }

    /// Beside the detail timeline's tracks, as Resolve: a column with the track names (V1, A1)
    /// and, left of it, the track's buttons (height, lock, solo, mute, picture).
    fn cut_track_controls(&self, painter: &Painter, l: &Lay, bottom: f32) {
        let rows = &self.cut_rows_detail;
        let Some(first) = rows.first() else { return };
        let top = first.2 - 2.0 * R;
        let left = l.p(0.0, 0.0).x;
        let column = l.p(203.5, 0.0).x;
        painter.line_segment(
            [pos2(left, top), pos2(l.p(233.0, 0.0).x, top)],
            Stroke::new(1.0, Color32::from_rgb(20, 20, 24)),
        );
        painter.line_segment(
            [pos2(column, top), pos2(column, l.p(0.0, bottom).y)],
            Stroke::new(1.0, Color32::from_rgb(8, 7, 12)),
        );
        let label_x = l.p(219.0, 0.0).x;
        for (name, audio, row_top, h) in rows {
            let y = if *audio {
                row_top - 7.0 * R
            } else {
                row_top + 24.0 * R
            };
            let y = if *audio {
                y.max(row_top - h)
            } else {
                y.min(row_top + h / 2.0)
            };
            painter.text(
                pos2(label_x, y),
                Align2::CENTER_CENTER,
                name,
                semibold(10.0 * R),
                TRACK_RED,
            );
        }
        // One row of buttons, level with V1 (the lowest picture row).
        let dim = Color32::from_rgb(92, 92, 98);
        if let Some((_, _, row_top, h)) = rows.iter().rfind(|r| !r.1) {
            let y = row_top + h / 2.0 + 9.0 * R;
            let at = |x: f32| pos2(l.p(x, 0.0).x, y);
            // Up and down arrows: the track's height.
            let c = at(28.0);
            for dir in [-1.0f32, 1.0] {
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        c + egui::vec2(-3.0 * R, dir * 2.0 * R),
                        c + egui::vec2(3.0 * R, dir * 2.0 * R),
                        c + egui::vec2(0.0, dir * 6.0 * R),
                    ],
                    dim,
                    Stroke::NONE,
                ));
            }
            paint_scaled(painter, at(66.0), Icon::PositionLock, dim, ICON);
            // Headphones: solo.
            let c = at(101.0);
            painter.add(egui::Shape::line(
                (0..=12)
                    .map(|i| {
                        let a = std::f32::consts::PI * (1.0 + i as f32 / 12.0);
                        c + egui::vec2(5.5 * R * a.cos(), 5.5 * R * a.sin() + 1.5 * R)
                    })
                    .collect(),
                Stroke::new(1.2, dim),
            ));
            for dx in [-5.5, 3.5] {
                painter.rect_filled(
                    Rect::from_min_size(
                        c + egui::vec2(dx * R, 0.5 * R),
                        egui::vec2(2.0 * R, 5.0 * R),
                    ),
                    1.0,
                    dim,
                );
            }
            paint_scaled(painter, at(137.0), Icon::Speaker, theme::ICON_BRIGHT, ICON);
            paint_scaled(
                painter,
                at(173.0),
                Icon::FilmView,
                Color32::from_rgb(150, 150, 156),
                ICON,
            );
        }
    }

    /// The second toolbar: the browser's tools on the left, the viewer's on the right.
    fn toolbar2(&mut self, ui: &mut Ui, ctx: &Context, l: &Lay) {
        let painter = ui.painter().clone();
        l.band(&painter, 0.0, 40.0, 1919.0, 69.0, theme::TOOLBAR2_BG);
        l.hline(&painter, 0.0, 1919.0, 69.5, Color32::from_rgb(10, 10, 15));
        for x in [
            58.0, 157.0, 192.0, 229.0, 313.0, 850.0, 886.0, 925.0, 960.5, 1053.0, 1124.0, 1160.0,
            1636.0, 1697.0, 1755.0, 1813.0, 1858.0,
        ] {
            l.vline(&painter, x, 41.0, 68.0, Color32::from_rgb(22, 22, 26));
        }
        let y = 54.0;
        let mut planned: Option<&str> = None;
        if self
            .tool(
                ui,
                l.p(13.0, y),
                Icon::Sidebar,
                t("Show or hide the media pool", 30.0).chev(29.0 * R),
            )
            .clicked()
        {
            planned = Some("Hiding the media pool");
        }
        if self
            .tool(ui, l.p(75.0, y), Icon::MediaImport, t("Import media", 28.0))
            .clicked()
        {
            self.import_dialog(ctx);
        }
        if self
            .tool(ui, l.p(106.0, y), Icon::BinFolder, t("New bin", 28.0))
            .clicked()
        {
            self.new_bin();
        }
        if self
            .tool(
                ui,
                l.p(135.0, y),
                Icon::Cloud,
                t("Import from the cloud", 28.0),
            )
            .clicked()
        {
            planned = Some("Cloud import");
        }
        if self
            .tool(ui, l.p(173.0, y), Icon::Sync, t("Sync", 28.0))
            .clicked()
        {
            planned = Some("Syncing");
        }
        if self
            .tool(ui, l.p(209.0, y), Icon::Unlink, t("Link", 28.0))
            .clicked()
        {
            self.do_act(ctx, Act::LinkClips);
        }
        if self
            .tool(
                ui,
                l.p(247.0, y),
                Icon::ImportImage,
                t("Import media", 30.0).chev(24.0 * R),
            )
            .clicked()
        {
            self.import_dialog(ctx);
        }
        if self
            .tool(ui, l.p(294.0, y), Icon::SubtitleBox, t("Subtitles", 28.0))
            .clicked()
        {
            planned = Some("Subtitles");
        }
        if self
            .tool(
                ui,
                l.p(815.0, y),
                Icon::Grid,
                t("Sort and view", 30.0).chev(20.0 * R),
            )
            .clicked()
        {
            planned = Some("View options");
        }
        if self
            .tool(ui, l.p(868.0, y), Icon::Search, t("Search", 28.0))
            .clicked()
        {
            planned = Some("Search");
        }
        if self
            .tool(ui, l.p(905.0, y), Icon::Sort, t("Sort", 28.0))
            .clicked()
        {
            planned = Some("Sorting");
        }
        self.tool(ui, l.p(941.0, y), Icon::Refresh, t("", 28.0).dim());
        if self
            .tool(ui, l.p(978.0, y), Icon::ThumbView, t("Thumbnails", 26.0))
            .clicked()
        {
            planned = Some("The thumbnail view");
        }
        if self
            .tool(ui, l.p(1006.0, y), Icon::FilmView, t("Filmstrip", 26.0))
            .clicked()
        {
            planned = Some("The filmstrip view");
        }
        if self
            .tool(
                ui,
                l.p(1035.0, y),
                Icon::ListView,
                t("List", 26.0).color(theme::ICON_BRIGHT),
            )
            .clicked()
        {
            planned = Some("The list view");
        }
        if self
            .tool(ui, l.p(1076.0, y), Icon::Grid2, t("Bins", 28.0))
            .clicked()
        {
            self.next_bin();
        }
        self.tool(ui, l.p(1106.0, y), Icon::Clip5m, t("", 28.0).dim());
        if self
            .tool(ui, l.p(1141.0, y), Icon::Antenna, t("Live sources", 28.0))
            .clicked()
        {
            planned = Some("Live sources");
        }
        // The timeline's name with its drop-down, the clock and the source timecode.
        painter.text(
            l.p(1397.0, y),
            Align2::CENTER_CENTER,
            self.timeline_name(),
            semibold(14.5 * R),
            theme::WHITE,
        );
        paint_scaled(&painter, l.p(1441.0, y), Icon::Chevron, theme::ICON, ICON);
        paint_scaled(
            &painter,
            l.p(1536.0, y + 1.0),
            Icon::Clock,
            theme::ICON_DIM,
            ICON,
        );
        let box_rect = l.r(1549.0, 44.0, 1621.0, 64.0);
        painter.rect_filled(box_rect, 3.0, theme::TIMECODE_BOX_BG);
        painter.rect_stroke(
            box_rect,
            3.0,
            Stroke::new(1.0, Color32::from_rgb(42, 42, 46)),
            StrokeKind::Inside,
        );
        painter.text(
            box_rect.center(),
            Align2::CENTER_CENTER,
            self.source_timecode(),
            regular(13.0 * R),
            theme::TIMECODE_TEXT,
        );
        if self
            .tool(
                ui,
                l.p(1657.0, y),
                Icon::FitView,
                t("Viewer size", 30.0).chev(24.0 * R),
            )
            .clicked()
        {
            self.do_act(ctx, Act::FullScreenViewer);
        }
        if self
            .tool(
                ui,
                l.p(1719.0, y),
                Icon::Hq,
                t("Viewer quality", 30.0).chev(22.0 * R),
            )
            .clicked()
        {
            self.set_proxy(if self.proxy > 1 { 1 } else { 2 });
        }
        if self
            .tool(
                ui,
                l.p(1775.0, y),
                Icon::Overlay,
                t("Overlays", 30.0).chev(22.0 * R),
            )
            .clicked()
        {
            self.do_act(ctx, Act::OverlayAll);
        }
        if self
            .tool(
                ui,
                l.p(1834.0, y),
                Icon::ColorWheel,
                t("Color wheels", 30.0).color(Color32::WHITE),
            )
            .clicked()
        {
            self.page = crate::app::Page::Color;
        }
        if self
            .tool(
                ui,
                l.p(1886.0, y),
                Icon::Speaker,
                t("Audio", 30.0).color(theme::WHITE),
            )
            .clicked()
        {
            let muted = !self.monitor.bus_muted;
            self.set_monitor(|m| m.bus_muted = muted);
            self.info(if muted {
                "Sound is muted"
            } else {
                "Sound is on"
            });
        }
        if let Some(what) = planned {
            if !self.pool_tool(what) {
                self.planned(what);
            }
        }
    }

    /// The name of the timeline being edited.
    fn timeline_name(&self) -> String {
        self.project().timeline_name.clone()
    }

    /// The time in the source of the clip under the playhead, `00:00:00:00` when there is none.
    pub(crate) fn source_timecode(&self) -> String {
        let fps = self.project().fps;
        self.current_clip()
            .and_then(|id| self.project().clip(id))
            .map_or_else(
                || fps.timecode(0),
                |(_, c)| fps.timecode(c.source_in + self.playhead.saturating_sub(c.start)),
            )
    }

    fn pool_region(&mut self, ui: &mut Ui, ctx: &Context, l: &Lay) {
        let painter = ui.painter().clone();
        l.band(&painter, 0.0, 70.0, 960.0, 99.0, theme::POOL_HEADER_BG);
        l.hline(&painter, 0.0, 960.0, 99.5, Color32::from_rgb(7, 6, 11));
        l.band(
            &painter,
            0.0,
            100.0,
            960.0,
            478.0,
            Color32::from_rgb(41, 40, 45),
        );
        l.vline(&painter, 960.5, 70.0, 478.0, Color32::from_rgb(8, 8, 10));
        painter.text(
            l.p(13.0, 85.0),
            Align2::LEFT_CENTER,
            self.pool_bin_title(),
            regular(12.0 * R),
            Color32::from_rgb(244, 240, 236),
        );
        if self.project().media.is_empty() {
            let mid = (l.y(100.0) + l.y(478.0)) / 2.0;
            // The message grows and shrinks with the pool, and stays inside it.
            let usual = f32::from(Panels::default().cut_upper);
            let k = (f32::from(self.extra.prefs.panels.cut_upper) / usual).clamp(0.4, 2.0);
            let room = l.r(0.0, 100.0, 960.0, 478.0).width() - 24.0;
            let (title, sub) = (
                "No clips in media pool",
                "Add clips with Import, or drop files here",
            );
            painter.text(
                pos2(l.p(480.0, 300.0).x, mid - 14.0 * R * k),
                Align2::CENTER_CENTER,
                title,
                fitting(&painter, title, 30.0 * R * k, room),
                theme::EMPTY_TITLE,
            );
            painter.text(
                pos2(l.p(480.0, 300.0).x, mid + 26.0 * R * k),
                Align2::CENTER_CENTER,
                sub,
                fitting(&painter, sub, 14.0 * R * k, room),
                theme::EMPTY_SUB,
            );
        } else {
            ui.scope_builder(
                UiBuilder::new().max_rect(l.r(12.0, 106.0, 950.0, 472.0)),
                |ui| self.pool_grid(ui, ctx),
            );
        }
        // Files dragged over the window are imported into the pool, so the pool shows where they
        // go. Drawn last, over the clips.
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            let area = l.r(0.0, 100.0, 960.0, 478.0);
            painter.rect_filled(area, 0.0, Color32::from_rgba_unmultiplied(20, 20, 24, 210));
            painter.rect_stroke(
                area.shrink(4.0),
                0.0,
                Stroke::new(2.0, theme::ACCENT),
                StrokeKind::Inside,
            );
            painter.text(
                area.center(),
                Align2::CENTER_CENTER,
                "Drop to add to the media pool",
                regular(24.0 * R),
                theme::ACCENT,
            );
        }
    }

    fn viewer_region(&mut self, ui: &mut Ui, l: &Lay) {
        let painter = ui.painter().clone();
        let area = l.r(961.0, 70.0, 1850.0, 478.0);
        l.vline(&painter, 1850.5, 70.0, 478.0, Color32::from_rgb(14, 14, 16));
        let aspect = self.project().width as f32 / self.project().height.max(1) as f32;
        let has_picture = self.project().duration() > 0;
        self.viewer.paint(
            ui,
            area,
            aspect,
            Color32::from_rgb(27, 27, 27),
            has_picture,
            self.overlays,
        );
    }

    /// The level meter at the right.
    fn meter_region(&self, p: &Painter, l: &Lay) {
        l.band(p, 1851.0, 70.0, 1919.0, 478.0, theme::METER_BG);
        let top = l.y(75.0);
        let bottom = l.y(465.0);
        let span = bottom - top;
        for (label, share) in METER_LABELS {
            let y = top + span * share;
            p.text(
                pos2(l.p(1880.0, 0.0).x, y),
                Align2::RIGHT_CENTER,
                label,
                regular(9.0 * R),
                Color32::from_rgb(110, 110, 118),
            );
            p.line_segment(
                [pos2(l.p(1882.0, 0.0).x, y), pos2(l.p(1885.0, 0.0).x, y)],
                Stroke::new(1.0, Color32::from_rgb(110, 110, 118)),
            );
        }
        let (x0, x1) = (l.p(1888.0, 0.0).x, l.p(1904.0, 0.0).x);
        p.rect_filled(
            Rect::from_min_max(pos2(x0, top - 1.0), pos2(x1, bottom + 1.0)),
            0.0,
            Color32::from_rgb(11, 11, 13),
        );
        let half = (x1 - x0 - 2.0) / 2.0;
        // The level of each channel, from the bottom up, green then yellow then red.
        for (n, level) in [self.meter.0, self.meter.1].into_iter().enumerate() {
            let cx0 = x0 + n as f32 * (half + 2.0);
            p.rect_filled(
                Rect::from_min_max(pos2(cx0, top), pos2(cx0 + half, bottom)),
                0.0,
                theme::POOL_BG,
            );
            if level < 1e-4 {
                continue;
            }
            let at = |lv: f32| top + span * level_to_share(lv);
            let y = at(level).clamp(top, bottom);
            for (from, to, color) in [
                (y, bottom.min(at(0.316)), Color32::from_rgb(230, 70, 60)),
                (
                    y.max(at(0.316)),
                    bottom.min(at(0.1)),
                    Color32::from_rgb(235, 200, 60),
                ),
                (y.max(at(0.1)), bottom, Color32::from_rgb(70, 190, 100)),
            ] {
                if to > from {
                    p.rect_filled(
                        Rect::from_min_max(pos2(cx0, from), pos2(cx0 + half, to)),
                        0.0,
                        color,
                    );
                }
            }
        }
    }

    fn transport_row(&mut self, ui: &mut Ui, l: &Lay) {
        let painter = ui.painter().clone();
        l.band(&painter, 0.0, 478.0, 1919.0, 522.0, theme::TRANSPORT_BG);
        l.hline(&painter, 0.0, 1919.0, 478.5, Color32::from_rgb(10, 10, 12));
        l.vline(&painter, 960.5, 479.0, 522.0, Color32::from_rgb(11, 11, 13));
        l.vline(
            &painter,
            1850.5,
            479.0,
            522.0,
            Color32::from_rgb(11, 11, 13),
        );
        let y = 500.0;
        let mut planned: Option<&str> = None;

        // Sending clips to the timeline: picture and sound, then the edits.
        if self
            .tool(
                ui,
                l.p(32.0, y),
                Icon::InsertClip,
                t("Append to the end of the timeline", 36.0),
            )
            .clicked()
        {
            self.insert_edit();
        }
        if self
            .tool(
                ui,
                l.p(73.0, y),
                Icon::MusicLines,
                t("Append the sound", 36.0),
            )
            .clicked()
        {
            self.insert_edit();
        }
        self.tool(ui, l.p(415.0, y), Icon::SmartInsert, t("", 36.0).dim());
        if self
            .tool(
                ui,
                l.p(453.0, y),
                Icon::Overwrite,
                t("Overwrite at the playhead (F10)", 36.0),
            )
            .clicked()
        {
            self.overwrite_edit();
        }
        if self
            .tool(
                ui,
                l.p(498.0, y),
                Icon::Replace,
                t("Replace the clip at the playhead (F11)", 36.0),
            )
            .clicked()
        {
            self.replace_edit();
        }
        if self
            .tool(
                ui,
                l.p(537.0, y),
                Icon::PlaceOnTop,
                t("Place on top of the clips at the playhead (F12)", 36.0),
            )
            .clicked()
        {
            self.place_on_top_edit();
        }
        self.tool(ui, l.p(577.0, y), Icon::RippleOverwrite, t("", 36.0).dim());
        if self
            .tool(
                ui,
                l.p(620.0, y),
                Icon::SourceOverlay,
                t("Source overwrite", 36.0),
            )
            .clicked()
        {
            self.place_on_top_edit();
        }
        for x in [845.0, 885.0, 930.0] {
            self.tool(ui, l.p(x, y), Icon::Overlay, t("", 34.0).dim());
        }

        // Tools of the viewer.
        for (x, icon, hint) in [
            (990.0, Icon::Sliders, "Audio levels"),
            (1035.0, Icon::Mic, "Voice-over"),
            (1080.0, Icon::PoiStart, "Mark an important point"),
            (1126.0, Icon::PoiEnd, "Mark the end of a point"),
            (1173.0, Icon::FilmPlay, "Preview"),
        ] {
            if self.tool(ui, l.p(x, y), icon, t(hint, 36.0)).clicked() {
                match icon {
                    Icon::Sliders => self.page = crate::app::Page::Audio,
                    Icon::PoiStart => self.do_act(&self.ctx.clone(), Act::MarkIn),
                    Icon::PoiEnd => self.do_act(&self.ctx.clone(), Act::MarkOut),
                    Icon::FilmPlay => self.do_act(&self.ctx.clone(), Act::PlayAround),
                    Icon::Mic => self.voice_over(),
                    _ => planned = Some(hint),
                }
            }
        }

        // Transport.
        let go = |hint| t(hint, 36.0).color(theme::TRANSPORT_ICON);
        if self
            .tool(
                ui,
                l.p(1206.0, y),
                Icon::ChevLeft,
                t("Previous frame", 20.0).color(theme::TRANSPORT_ICON),
            )
            .clicked()
        {
            self.go_to(self.playhead.saturating_sub(1));
        }
        if self
            .tool(
                ui,
                l.p(1226.0, y),
                Icon::Dot,
                t("Jog", 20.0).color(theme::TRANSPORT_ICON),
            )
            .clicked()
        {
            self.do_act(&self.ctx.clone(), Act::PlayAround);
        }
        if self
            .tool(
                ui,
                l.p(1246.0, y),
                Icon::ChevRight,
                t("Next frame", 20.0).color(theme::TRANSPORT_ICON),
            )
            .clicked()
        {
            self.go_to(self.playhead + 1);
        }
        if self
            .tool(
                ui,
                l.p(1284.0, y),
                Icon::ToStart,
                go("Go to the start (Home)"),
            )
            .clicked()
        {
            self.playing = false;
            self.go_to(0);
        }
        if self
            .tool(ui, l.p(1326.0, y), Icon::PlayReverse, go("Play backwards"))
            .clicked()
        {
            self.do_act(&self.ctx.clone(), Act::PlayReverse);
        }
        if self
            .tool(ui, l.p(1362.0, y), Icon::Stop, go("Stop"))
            .clicked()
        {
            self.playing = false;
        }
        let play_hint = if self.playing {
            "Pause (Space)"
        } else {
            "Play (Space)"
        };
        let play_color = if self.playing {
            theme::ICON_BRIGHT
        } else {
            theme::TRANSPORT_ICON
        };
        if self
            .tool(
                ui,
                l.p(1398.0, y),
                Icon::Play,
                go(play_hint).color(play_color),
            )
            .clicked()
        {
            self.toggle_play();
        }
        if self
            .tool(ui, l.p(1439.0, y), Icon::ToEnd, go("Go to the end (End)"))
            .clicked()
        {
            self.playing = false;
            self.go_to(u64::MAX);
        }
        let loop_color = if self.looping {
            theme::ICON_BRIGHT
        } else {
            theme::TRANSPORT_ICON
        };
        if self
            .tool(ui, l.p(1484.0, y), Icon::Loop, go("Loop").color(loop_color))
            .clicked()
        {
            self.looping = !self.looping;
        }
        if self
            .tool(ui, l.p(1590.0, y), Icon::NextEdit, go("Next clip boundary"))
            .clicked()
        {
            self.go_to_edit(true);
        }
        if self
            .tool(
                ui,
                l.p(1630.0, y),
                Icon::PrevEdit,
                go("Previous clip boundary"),
            )
            .clicked()
        {
            self.go_to_edit(false);
        }
        let tcg = format!("TCG {}", self.tc(self.playhead as i64));
        painter.text(
            l.p(1696.0, y),
            Align2::LEFT_CENTER,
            tcg,
            semibold(16.5 * R),
            theme::WHITE,
        );
        paint_scaled(&painter, l.p(1830.0, y), Icon::Chevron, theme::ICON, ICON);
        if self
            .tool(ui, l.p(1886.0, y), Icon::Menu, t("More", 40.0))
            .clicked()
        {
            planned = Some("The transport menu");
        }
        if let Some(what) = planned {
            self.planned(what);
        }
    }
}
