//! The Effects page (the wand tab): two viewers over a frame ruler, a transport row, a row of
//! effect tools, the node area and the inspector, laid out like the reference screen.
//!
//! Positions are written in pixels of the 2000-pixel-wide reference and scaled to the
//! 1872-point window (`K`). Across the page the bands keep their reference heights; only the
//! viewers and the node area grow or shrink with the window, in the reference's proportions.

#![allow(clippy::too_many_arguments)]

use egui::{pos2, Align2, Color32, Context, Painter, Pos2, Rect, Sense, Stroke, Ui};

use crate::app::StudioApp;
use crate::capture::fit;
use crate::chrome::{Screen, Tool, R};
use crate::icons::Icon;
use crate::menu::Act;
use crate::panels::Split;
use crate::theme;

/// Points per reference pixel.
const K: f32 = 0.936;

const BAR: Color32 = Color32::from_rgb(34, 33, 38);
const VIEWER: Color32 = Color32::from_rgb(26, 26, 26);
const PANEL: Color32 = Color32::from_rgb(41, 40, 47);
const LINE: Color32 = Color32::from_rgb(16, 16, 19);
const TEXT: Color32 = Color32::from_rgb(214, 214, 218);
const GREY: Color32 = Color32::from_rgb(150, 150, 154);
const ICON: Color32 = Color32::from_rgb(120, 120, 124);
const FIELD: Color32 = Color32::from_rgb(31, 31, 31);

/// Reference rows kept for the page's own bands.
const VIEW_TOP: f32 = 146.0;
const VIEW_BOTTOM: f32 = 548.0;
const NODES_TOP: f32 = 800.0;
const NODES_BOTTOM: f32 = 1049.0;
/// The top of the page bar in the reference.
const PAGE_BAR: f32 = 1087.0;

/// Points per reference pixel for widths and radii, and for text and icons: Resolve at 1080p
/// draws the page about two thirds the size of the older reference.
const Q: f32 = 0.64 * R;
const SZ: f32 = 0.68 * R;

/// Where the older reference's columns land on Resolve's at 1080p (x, in its pixels): the
/// viewers and the page's full-width bars...
const COLUMNS: [(f32, f32); 4] = [
    (3.0, 0.0),
    (675.0, 749.0),
    (1347.0, 1497.0),
    (2000.0, 1919.0),
];
/// ...the bar over the viewers...
const HEADER_X: [(f32, f32); 24] = [
    (3.0, 0.0),
    (12.0, 11.0),
    (75.0, 51.0),
    (126.0, 84.0),
    (237.0, 154.0),
    (259.0, 168.0),
    (557.0, 677.0),
    (585.0, 693.0),
    (644.0, 731.0),
    (675.0, 749.0),
    (704.0, 774.0),
    (754.0, 799.0),
    (805.0, 831.0),
    (842.0, 855.0),
    (879.0, 880.0),
    (915.0, 903.0),
    (996.0, 1123.0),
    (1086.0, 1192.0),
    (1122.0, 1216.0),
    (1207.0, 1270.0),
    (1277.0, 1315.0),
    (1316.0, 1340.0),
    (1347.0, 1497.0),
    (2000.0, 1919.0),
];
/// ...the transport row...
const TRANSPORT_X: [(f32, f32); 19] = [
    (3.0, 0.0),
    (29.0, 22.0),
    (67.0, 46.0),
    (106.0, 71.0),
    (122.0, 85.0),
    (162.0, 108.0),
    (201.0, 132.0),
    (237.0, 154.0),
    (540.0, 665.0),
    (592.0, 698.0),
    (642.0, 729.0),
    (690.0, 760.0),
    (743.0, 794.0),
    (803.0, 832.0),
    (1209.0, 1410.0),
    (1262.0, 1442.0),
    (1316.0, 1477.0),
    (1347.0, 1497.0),
    (2000.0, 1919.0),
];
/// ...and the row of tools.
const TOOLS_X: [(f32, f32); 23] = [
    (3.0, 0.0),
    (37.0, 182.0),
    (100.0, 222.0),
    (162.5, 262.0),
    (225.5, 302.0),
    (303.5, 352.0),
    (368.5, 393.0),
    (432.0, 434.0),
    (492.5, 473.0),
    (573.0, 523.0),
    (636.0, 563.0),
    (696.5, 603.0),
    (760.0, 643.0),
    (825.5, 684.0),
    (904.5, 735.0),
    (968.0, 775.0),
    (1031.0, 815.0),
    (1093.0, 855.0),
    (1156.0, 895.0),
    (1237.5, 946.0),
    (1298.5, 986.0),
    (1347.0, 1497.0),
    (2000.0, 1919.0),
];
/// The older reference's rows that land on Resolve's (y, in its pixels): the viewer bar, the
/// viewers, the ruler to the tool row, the node area and the status line.
const ROWS: [(f32, f32); 6] = [
    (107.0, 40.0),
    (146.0, 70.0),
    (548.0, 550.0),
    (800.0, 715.0),
    (1049.0, 980.0),
    (1087.0, 1005.0),
];
/// The inspector's rows: it is a column of fixed height rows in Resolve.
const INSPECTOR_ROWS: [(f32, f32); 15] = [
    (107.0, 43.0),
    (126.0, 55.0),
    (145.0, 67.0),
    (176.0, 83.0),
    (231.0, 119.0),
    (284.0, 153.0),
    (341.0, 190.0),
    (373.0, 210.0),
    (425.0, 243.0),
    (464.0, 268.0),
    (504.0, 293.0),
    (556.0, 327.0),
    (604.0, 358.0),
    (1049.0, 980.0),
    (1087.0, 1005.0),
];

fn piece(map: &[(f32, f32)], v: f32) -> f32 {
    for w in map.windows(2) {
        let ((a0, b0), (a1, b1)) = (w[0], w[1]);
        if v <= a1 {
            return b0 + (v - a0) / (a1 - a0) * (b1 - b0);
        }
    }
    let (a, b) = map[map.len() - 1];
    b + (v - a)
}

/// A drawing surface that takes reference pixels.
struct Fx<'a> {
    p: &'a Painter,
    s: &'a Screen,
    /// Whether it draws in the inspector, whose columns and rows are its own.
    ins: bool,
}

impl<'a> Fx<'a> {
    fn new(p: &'a Painter, s: &'a Screen) -> Self {
        Fx { p, s, ins: false }
    }
    /// The same surface for the inspector.
    fn inspector(&self) -> Fx<'a> {
        Fx {
            p: self.p,
            s: self.s,
            ins: true,
        }
    }
    /// A point given in pixels of Resolve at 1080p: its 965 rows of body fill the window's.
    fn rv(&self, x: f32, y: f32) -> Pos2 {
        let (x, y) = self.rv_points(x, y);
        self.s.p(x, y)
    }
    /// `rv` as the screen's own points, before the dividers move them.
    fn rv_points(&self, x: f32, y: f32) -> (f32, f32) {
        let vf = (self.s.h - 46.8 - 92.0).max(300.0) / 965.0;
        (x * R, 92.0 + (y - 40.0) * vf)
    }
    fn rrect(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        self.s
            .between(self.rv_points(x0, y0), self.rv_points(x1, y1))
    }
    fn at(&self, x: f32, y: f32) -> Pos2 {
        let (x, y) = self.at_points(x, y);
        self.s.p(x, y)
    }
    /// `at` as the screen's own points, before the dividers move them.
    fn at_points(&self, x: f32, y: f32) -> (f32, f32) {
        if self.ins {
            return self.rv_points(1498.0 + (x - 1350.0) * 0.648, piece(&INSPECTOR_ROWS, y));
        }
        let map = if (107.0..=146.0).contains(&y) {
            &HEADER_X[..]
        } else if (627.0..=680.0).contains(&y) {
            &TRANSPORT_X[..]
        } else if (682.0..=760.0).contains(&y) {
            &TOOLS_X[..]
        } else {
            &COLUMNS[..]
        };
        self.rv_points(piece(map, x), piece(&ROWS, y))
    }
    fn rect(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        self.s
            .between(self.at_points(x0, y0), self.at_points(x1, y1))
    }
    fn band(&self, x0: f32, y0: f32, x1: f32, y1: f32, c: Color32) {
        self.p.rect_filled(self.rect(x0, y0, x1, y1), 0.0, c);
    }
    fn text(&self, x: f32, y: f32, align: Align2, text: &str, width: f32, c: Color32, bold: bool) {
        self.p.text(
            self.at(x, y),
            align,
            text,
            fit(self.p, text, width * Q * self.shrink(x, y), bold),
            c,
        );
    }
    /// How much smaller than usual text and icons at a reference point are drawn: as much as
    /// the dividers squeezed the panel there, so they stay inside it.
    fn shrink(&self, x: f32, y: f32) -> f32 {
        let (x, y) = self.at_points(x, y);
        self.s.shrink(x, y)
    }
    /// Text at a fixed size in points.
    fn label(&self, x: f32, y: f32, align: Align2, text: &str, size: f32, c: Color32, bold: bool) {
        let size = size * SZ * self.shrink(x, y);
        let font = if bold {
            theme::semibold(size)
        } else {
            theme::regular(size)
        };
        self.p.text(self.at(x, y), align, text, font, c);
    }
    fn line(&self, a: (f32, f32), b: (f32, f32), w: f32, c: Color32) {
        let ends = self
            .s
            .segment(self.at_points(a.0, a.1), self.at_points(b.0, b.1));
        self.p.line_segment(ends, Stroke::new(w, c));
    }
    fn vline(&self, x: f32, y0: f32, y1: f32, c: Color32) {
        self.line((x, y0), (x, y1), 1.0, c);
    }
    fn icon(&self, x: f32, y: f32, icon: Icon, c: Color32, size: f32) {
        let size = size * SZ * self.shrink(x, y);
        crate::icons::paint_scaled(self.p, self.at(x, y), icon, c, size);
    }
    fn chevron(&self, x: f32, y: f32, c: Color32) {
        let c0 = self.at(x, y);
        let u = SZ;
        self.p.add(egui::Shape::line(
            vec![
                c0 + egui::vec2(-7.0 * u, -4.0 * u),
                c0 + egui::vec2(0.0, 3.5 * u),
                c0 + egui::vec2(7.0 * u, -4.0 * u),
            ],
            Stroke::new(1.3, c),
        ));
    }
    fn dots(&self, x: f32, y: f32, c: Color32) {
        let c0 = self.at(x, y);
        for dx in [-11.0, 0.0, 11.0] {
            self.p
                .circle_filled(c0 + egui::vec2(dx * SZ, 0.0), 3.0 * SZ, c);
        }
    }
    fn field(&self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let r = self.rect(x0, y0, x1, y1);
        self.p.rect_filled(r, 2.0, FIELD);
        self.p
            .rect_stroke(r, 2.0, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
    }
}

/// The clip at the playhead, which the page shows as its MediaIn node.
#[derive(Clone)]
struct FxClip {
    name: String,
    start: u64,
    duration: u64,
}

/// Shortens `text` to `max` characters with an ellipsis.
fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let cut: String = text.chars().take(max.saturating_sub(3)).collect();
        format!("{cut}...")
    }
}

impl StudioApp {
    fn fx_clip(&self) -> Option<FxClip> {
        let id = self.current_clip()?;
        let (_, clip) = self.project().clip(id)?;
        let name = self
            .project()
            .media(clip.media)
            .map_or_else(|| clip.name.clone(), |m| m.name.clone());
        Some(FxClip {
            name,
            start: clip.start,
            duration: clip.duration.max(1),
        })
    }

    /// The first toolbar of the Effects page.
    pub(crate) fn effects_toolbar1(&mut self, ui: &mut Ui, ctx: &Context, s: &Screen, y: f32) {
        // (x in reference pixels, icon, hint, size, colour)
        let dim = Color32::from_rgb(62, 62, 66);
        let tools: [(f32, Icon, &str, f32, Color32); 10] = [
            (42.0, Icon::ChevronBox, "Toolbar", 0.85, dim),
            (129.0, Icon::PhotoMusic, "Import media", 1.05, theme::ICON),
            (211.0, Icon::Wand, "Effects library", 0.95, theme::ICON),
            (290.0, Icon::ClipFrame, "Clips", 0.95, theme::ICON),
            (
                369.0,
                Icon::NodeTree,
                "Nodes",
                1.0,
                Color32::from_rgb(210, 210, 214),
            ),
            (1643.0, Icon::Spline, "Spline", 0.95, theme::ICON),
            (1726.0, Icon::Diamonds3, "Keyframes", 1.0, theme::ICON),
            (1803.0, Icon::LabelTag, "Metadata", 0.95, theme::ICON),
            (1883.0, Icon::Tools, "Inspector", 0.82, theme::WHITE),
            (1962.0, Icon::MonitorUp, "Viewer", 0.95, theme::ICON),
        ];
        let mut planned: Option<&str> = None;
        for (x, icon, hint, size, color) in tools {
            let tool = Tool::new(hint).size(size).color(color);
            if self.tool(ui, s.p(x * K, y), icon, tool).clicked() {
                match icon {
                    Icon::PhotoMusic => self.import_dialog(ctx),
                    Icon::Tools | Icon::Diamonds3 => {
                        self.do_act(ctx, crate::menu::Act::ShowInspector)
                    }
                    _ => planned = Some(hint),
                }
            }
        }
        if let Some(what) = planned {
            self.planned(what);
        }
    }

    pub(crate) fn effects_page(&mut self, ui: &mut Ui, ctx: &Context, s: &Screen) {
        let painter = ui.painter().clone();
        // The dividers: between the two viewers, between the viewers and the node area (the
        // ruler, transport and tool rows move with it), and before the inspector.
        let splits = {
            let c = Fx::new(&painter, s);
            let (left, right) = (c.at(3.0, VIEW_TOP).x, c.at(2000.0, VIEW_TOP).x);
            let inspector = c.at(1347.0, VIEW_TOP).x;
            [
                Split::column(
                    "fx viewers",
                    s,
                    c.at(675.0, 107.0),
                    c.at(675.0, VIEW_BOTTOM),
                    left,
                    inspector,
                ),
                Split::row(
                    "fx nodes",
                    s,
                    c.at(3.0, VIEW_BOTTOM),
                    c.at(1347.0, VIEW_BOTTOM),
                    c.at(3.0, VIEW_TOP).y,
                    c.at(3.0, NODES_BOTTOM).y,
                )
                .keeping(c.at(3.0, NODES_TOP).y - c.at(3.0, VIEW_BOTTOM).y),
                Split::column(
                    "fx inspector",
                    s,
                    c.at(1347.0, 107.0),
                    c.at(1347.0, NODES_BOTTOM),
                    left,
                    right,
                ),
            ]
        };
        let warped = self.warped(s, &splits);
        let s = &warped;
        let c = Fx::new(&painter, s);
        let inspector = 1350.0;
        let clip = self.fx_clip();

        // ---- the viewer headers (107 to 143) and the viewers
        c.band(3.0, 107.0, 2000.0, 146.0, BAR);
        c.band(3.0, 143.0, 2000.0, 146.0, LINE);
        self.viewer_header(&c, 0.0);
        if clip.is_some() {
            self.output_header(&c);
        } else {
            self.viewer_header(&c, 677.0);
        }
        c.band(3.0, VIEW_TOP, inspector, VIEW_BOTTOM, VIEWER);
        if clip.is_some() {
            // The right viewer shows the node tree's output: the picture at the playhead.
            let area = c.rect(677.0, VIEW_TOP, inspector - 2.0, VIEW_BOTTOM);
            let aspect = self.project().width as f32 / self.project().height.max(1) as f32;
            let shown = self
                .viewer
                .paint(ui, area, aspect, VIEWER, true, self.overlays);
            let clipped = c.p.with_clip_rect(area);
            clipped.text(
                pos2(shown.center().x, shown.top() - 12.0),
                Align2::CENTER_CENTER,
                format!(
                    "[Main]: {}x{} float16",
                    self.project().width,
                    self.project().height
                ),
                theme::regular(16.0 * SZ),
                Color32::from_rgb(170, 170, 176),
            );
        }
        c.band(672.0, 107.0, 675.0, VIEW_BOTTOM, LINE);

        // ---- the frame ruler (548 to 599) and the range bar (600 to 626)
        c.band(3.0, 548.0, inspector, 599.0, Color32::from_rgb(28, 28, 31));
        c.band(3.0, 548.0, inspector, 551.0, LINE);
        c.band(3.0, 590.0, inspector, 599.0, BAR);
        let frames = clip.as_ref().map_or(1000, |k| k.duration);
        let (x0, x1) = (5.0, 1343.0);
        let per_frame = (x1 - x0) / frames as f32;
        let step = [
            10u64, 25, 50, 100, 200, 500, 1000, 2000, 5000, 10_000, 50_000,
        ]
        .into_iter()
        .find(|s| *s as f32 * per_frame >= 48.0)
        .unwrap_or(100_000);
        let mut f = 0;
        while f < frames {
            let x = x0 + f as f32 * per_frame;
            if f > 0 {
                c.line((x, 557.0), (x, 591.0), 1.0, Color32::from_rgb(74, 74, 80));
            }
            c.label(
                x + 5.0,
                573.0,
                Align2::LEFT_CENTER,
                &f.to_string(),
                14.5,
                Color32::from_rgb(140, 140, 146),
                false,
            );
            f += step;
        }
        c.band(3.0, 553.0, 6.5, 597.0, Color32::from_rgb(200, 72, 36));
        c.band(
            1341.5,
            553.0,
            1345.0,
            597.0,
            Color32::from_rgb(176, 146, 64),
        );
        if let Some(k) = &clip {
            // The playhead in the clip, and a click or drag on the ruler moves it.
            let at = self.playhead.saturating_sub(k.start).min(k.duration - 1);
            let x = x0 + at as f32 * per_frame;
            c.line((x, 551.0), (x, 598.0), 2.0, Color32::from_rgb(222, 64, 52));
            let r = ui.interact(
                c.rect(x0, 551.0, x1, 598.0),
                ui.id().with("fx_ruler"),
                Sense::click_and_drag(),
            );
            if r.clicked() || r.dragged() {
                if let Some(p) = r.interact_pointer_pos() {
                    let left = c.at(x0, 0.0).x;
                    let right = c.at(x1, 0.0).x;
                    let f = ((p.x - left) / (right - left)).clamp(0.0, 1.0);
                    self.playing = false;
                    self.go_to(k.start + (f * (k.duration - 1) as f32) as u64);
                }
            }
        }
        c.band(3.0, 599.0, inspector, 627.0, BAR);
        c.band(3.0, 598.0, inspector, 600.0, LINE);
        c.p.rect_filled(
            c.rect(3.0, 612.0, 1342.0, 623.0),
            5.0,
            Color32::from_rgb(91, 95, 100),
        );

        // ---- the transport row (627 to 680)
        c.band(3.0, 627.0, inspector, 680.0, BAR);
        self.effects_transport(ui, ctx, &c, clip.as_ref());
        c.band(3.0, 680.0, inspector, 682.0, LINE);

        // ---- the tool row (682 to 760)
        c.band(3.0, 682.0, inspector, 760.0, BAR);
        self.effects_tools(ui, &c);
        c.band(3.0, 760.0, inspector, 762.0, LINE);

        // ---- the node area (762 to 1049)
        c.band(3.0, 762.0, inspector, 799.0, BAR);
        c.text(
            12.0,
            779.0,
            Align2::LEFT_CENTER,
            "Nodes",
            54.0,
            theme::WHITE,
            true,
        );
        c.p.rect_filled(c.rrect(1459.0, 686.0, 1460.0, 713.0), 0.0, LINE);
        for dx in [-8.0, 0.0, 8.0] {
            c.p.circle_filled(c.rv(1479.0 + dx, 700.0), 1.6 * R, GREY);
        }
        c.band(3.0, 798.0, inspector, NODES_TOP, LINE);
        c.band(3.0, NODES_TOP, inspector, NODES_BOTTOM, PANEL);
        let grid = Color32::from_rgb(37, 36, 43);
        for n in 0..19 {
            let x = 42.0 + 82.8 * n as f32;
            c.p.line_segment([c.rv(x, 715.0), c.rv(x, 980.0)], Stroke::new(1.0, grid));
        }
        for n in 0..6 {
            let y = 731.5 + 54.0 * n as f32;
            c.p.line_segment([c.rv(0.0, y), c.rv(1497.0, y)], Stroke::new(1.0, grid));
        }
        if let Some(k) = &clip {
            self.fx_nodes(&c, k);
        }

        // ---- the inspector (1350 to the right edge, 107 to 1049)
        self.effects_inspector(ui, &c.inspector(), inspector, clip.as_ref());
        c.band(1347.0, 107.0, 1350.0, NODES_BOTTOM, LINE);

        // ---- the status line over the page bar (1049 to 1087)
        c.band(3.0, NODES_BOTTOM, 2000.0, PAGE_BAR, BAR);
        c.band(3.0, NODES_BOTTOM, 2000.0, 1051.0, LINE);
        if clip.is_some() {
            c.label(
                1180.0,
                1068.0,
                Align2::CENTER_CENTER,
                &format!("Playback: {} frames/sec", self.project().fps.nominal()),
                16.0,
                GREY,
                false,
            );
        }
        c.text(
            1990.0,
            1068.0,
            Align2::RIGHT_CENTER,
            "5% - 1637 MB",
            116.0,
            GREY,
            false,
        );
        self.dividers(ui, s, &splits);
    }

    /// The MediaIn node's settings: its name, the clip's name, and the Image tab's controls.
    fn fx_media_in(&mut self, ui: &mut Ui, c: &Fx, k: &FxClip) {
        // The node row: on/off, name, colour, and the node's buttons.
        c.p.rect_filled(
            c.rect(1376.0, 220.0, 1412.0, 242.0),
            11.0,
            Color32::from_rgb(20, 20, 23),
        );
        c.p.circle_filled(
            c.at(1402.0, 231.0),
            7.0 * SZ,
            Color32::from_rgb(222, 64, 52),
        );
        c.label(
            1428.0,
            231.0,
            Align2::LEFT_CENTER,
            &format!("MediaIn1: {}", shorten(&k.name, 22)),
            19.0,
            theme::WHITE,
            false,
        );
        c.p.circle_filled(
            c.at(1749.0, 230.0),
            6.5 * SZ,
            Color32::from_rgb(110, 160, 230),
        );
        c.chevron(1778.0, 231.0, GREY);
        c.vline(1803.0, 214.0, 248.0, LINE);
        for (x, icon) in [
            (1836.0, Icon::ViewerFrame),
            (1880.0, Icon::Needle),
            (1925.0, Icon::PositionLock),
            (1968.0, Icon::Reset),
        ] {
            c.icon(x, 231.0, icon, ICON, 0.85);
        }
        c.line((1366.0, 262.0), (1992.0, 262.0), 1.0, LINE);
        // The clip's name.
        c.label(
            1582.0,
            284.0,
            Align2::RIGHT_CENTER,
            "Clip Name",
            17.0,
            GREY,
            false,
        );
        c.field(1597.0, 268.0, 1915.0, 300.0);
        c.label(
            1607.0,
            284.0,
            Align2::LEFT_CENTER,
            &shorten(&k.name, 32),
            17.5,
            TEXT,
            false,
        );
        c.line((1366.0, 314.0), (1992.0, 314.0), 1.0, LINE);
        // The tabs: Image is the one shown.
        let id = ui.id().with("fx_media_tab");
        let tab = ui.data(|d| d.get_temp::<usize>(id)).unwrap_or(0);
        let mut chosen = tab;
        for (n, (x, icon, name)) in [
            (1587.0, Icon::Photo, "Image"),
            (1680.0, Icon::SparkleFrame, "Audio"),
            (1774.0, Icon::Sizing, "Settings"),
        ]
        .into_iter()
        .enumerate()
        {
            let on = n == tab;
            c.icon(x, 341.0, icon, if on { TEXT } else { ICON }, 1.0);
            c.label(
                x,
                373.0,
                Align2::CENTER_CENTER,
                name,
                16.5,
                if on { TEXT } else { GREY },
                false,
            );
            if on {
                c.band(
                    x - 46.0,
                    392.0,
                    x + 46.0,
                    395.0,
                    Color32::from_rgb(222, 64, 52),
                );
            }
            if ui
                .interact(
                    c.rect(x - 46.0, 320.0, x + 46.0, 395.0),
                    ui.id().with(("fx_media_tab", n)),
                    Sense::click(),
                )
                .clicked()
            {
                chosen = n;
            }
        }
        ui.data_mut(|d| d.insert_temp(id, chosen));
        c.line((1352.0, 397.0), (2000.0, 397.0), 1.0, LINE);
        if tab != 0 {
            c.label(
                1680.0,
                440.0,
                Align2::CENTER_CENTER,
                "Nothing to set here yet",
                16.0,
                GREY,
                false,
            );
            return;
        }
        for (y, label, value) in [
            (425.0, "Process Mode", "Full Frames"),
            (464.0, "Media Source", "Timeline"),
            (504.0, "Layer", "0"),
        ] {
            c.label(1582.0, y, Align2::RIGHT_CENTER, label, 17.0, GREY, false);
            c.field(1597.0, y - 15.0, 1915.0, y + 15.0);
            c.label(1610.0, y, Align2::LEFT_CENTER, value, 17.0, TEXT, false);
            c.chevron(1896.0, y, GREY);
        }
        c.line((1376.0, 532.0), (1988.0, 532.0), 1.0, LINE);
        for (y, label) in [(556.0, "Source Color Space"), (604.0, "Source Gamma Space")] {
            c.line((1385.0, y - 6.0), (1391.0, y), 1.5, GREY);
            c.line((1385.0, y + 6.0), (1391.0, y), 1.5, GREY);
            c.label(1408.0, y, Align2::LEFT_CENTER, label, 18.0, TEXT, false);
            c.line((1376.0, y + 24.0), (1988.0, y + 24.0), 1.0, LINE);
        }
    }

    /// The right viewer's bar while it shows the output: fit, channels, the output node, the
    /// look-up table and the region.
    fn output_header(&self, c: &Fx) {
        let sep = |x: f32| c.vline(x, 107.0, 143.0, LINE);
        c.label(704.0, 125.0, Align2::LEFT_CENTER, "Fit", 17.5, TEXT, false);
        c.chevron(754.0, 125.0, GREY);
        sep(775.0);
        c.icon(805.0, 125.0, Icon::ViewerA, ICON, 1.15);
        c.chevron(842.0, 125.0, GREY);
        c.icon(879.0, 125.0, Icon::ViewerInset, ICON, 1.15);
        c.chevron(915.0, 125.0, GREY);
        sep(937.0);
        c.label(
            996.0,
            125.0,
            Align2::CENTER_CENTER,
            "MediaOut1",
            18.5,
            theme::WHITE,
            true,
        );
        sep(1057.0);
        c.icon(1086.0, 125.0, Icon::ViewerFrame, ICON, 0.95);
        c.chevron(1122.0, 125.0, GREY);
        c.label(
            1207.0,
            125.0,
            Align2::CENTER_CENTER,
            "Default",
            17.5,
            GREY,
            false,
        );
        c.chevron(1277.0, 125.0, GREY);
        c.p.circle_filled(
            c.at(1316.0, 125.0),
            9.0 * SZ,
            Color32::from_rgb(110, 110, 116),
        );
        c.line((1344.0, 120.0), (1348.0, 125.0), 1.2, GREY);
        c.line((1344.0, 130.0), (1348.0, 125.0), 1.2, GREY);
    }

    /// The clip's MediaIn node at the lower left, its link towards the output, and the
    /// navigator that shows the whole node tree small.
    fn fx_nodes(&self, c: &Fx, k: &FxClip) {
        // Resolve's pixels at 1080p: the clip's MediaIn node, the link and the MediaOut node.
        let red = Color32::from_rgb(222, 64, 52);
        let node = c.rrect(60.0, 828.0, 188.0, 850.0);
        c.p.rect_filled(node, 3.0, Color32::from_rgb(58, 58, 64));
        c.p.rect_stroke(
            node,
            3.0,
            egui::Stroke::new(1.5, red),
            egui::StrokeKind::Outside,
        );
        c.p.text(
            c.rv(65.0, 839.0),
            Align2::LEFT_CENTER,
            shorten(&k.name, 18),
            theme::regular(9.5 * R),
            TEXT,
        );
        c.p.add(egui::Shape::convex_polygon(
            vec![c.rv(50.0, 834.0), c.rv(56.0, 839.0), c.rv(50.0, 844.0)],
            Color32::from_rgb(70, 140, 230),
            egui::Stroke::NONE,
        ));
        c.p.rect_filled(c.rrect(190.0, 836.0, 196.0, 842.0), 0.0, theme::WHITE);
        c.p.line_segment(
            [c.rv(196.0, 839.0), c.rv(878.0, 839.0)],
            egui::Stroke::new(1.5, Color32::from_rgb(222, 196, 40)),
        );
        c.p.add(egui::Shape::convex_polygon(
            vec![c.rv(874.0, 834.0), c.rv(882.0, 839.0), c.rv(874.0, 844.0)],
            Color32::from_rgb(222, 196, 40),
            egui::Stroke::NONE,
        ));
        let out = c.rrect(886.0, 829.0, 1011.0, 850.0);
        c.p.rect_filled(out, 3.0, Color32::from_rgb(40, 40, 46));
        c.p.rect_stroke(
            out,
            3.0,
            egui::Stroke::new(1.0, Color32::from_rgb(90, 90, 96)),
            egui::StrokeKind::Outside,
        );
        c.p.text(
            c.rv(948.0, 838.0),
            Align2::CENTER_CENTER,
            "MediaOut1",
            theme::regular(9.5 * R),
            TEXT,
        );
        c.p.rect_filled(c.rrect(1014.0, 836.0, 1021.0, 842.0), 0.0, theme::WHITE);
    }

    /// The bar over one viewer: zoom, channel and overlay menus, the frame and the menu.
    fn viewer_header(&self, c: &Fx, x0: f32) {
        let off = x0 - if x0 > 0.0 { 2.0 } else { 0.0 };
        let sep = |x: f32| c.vline(off + x, 107.0, 143.0, LINE);
        c.text(
            off + 12.0,
            125.0,
            Align2::LEFT_CENTER,
            "100%",
            44.0,
            TEXT,
            false,
        );
        c.chevron(off + 75.0, 125.0, GREY);
        sep(97.0);
        c.icon(off + 126.0, 125.0, Icon::ViewerA, ICON, 1.15);
        c.chevron(off + 162.0, 125.0, GREY);
        c.icon(off + 200.0, 125.0, Icon::ViewerInset, ICON, 1.15);
        c.chevron(off + 237.0, 125.0, GREY);
        sep(259.0);
        sep(557.0);
        c.icon(off + 585.0, 125.0, Icon::ViewerFrame, GREY, 1.1);
        sep(615.0);
        c.dots(off + 644.0, 125.0, GREY);
    }

    fn effects_transport(&mut self, ui: &mut Ui, ctx: &Context, c: &Fx, clip: Option<&FxClip>) {
        let (last, here) = match clip {
            Some(k) => (
                k.duration - 1,
                self.playhead.saturating_sub(k.start).min(k.duration - 1),
            ),
            None => (1000, 0),
        };
        c.field(29.0, 640.0, 106.0, 669.0);
        c.label(67.0, 654.0, Align2::CENTER_CENTER, "0.0", 17.5, TEXT, false);
        c.field(122.0, 640.0, 201.0, 669.0);
        c.label(
            162.0,
            654.0,
            Align2::CENTER_CENTER,
            &format!("{last}.0"),
            17.5,
            TEXT,
            false,
        );
        c.icon(237.0, 654.0, Icon::Speaker, theme::WHITE, 0.85);
        let buttons: [(f32, Icon, Act, Color32); 6] = [
            (540.0, Icon::ToStart, Act::GoFirst, GREY),
            (592.0, Icon::PlayReverse, Act::PreviousFrame, GREY),
            (642.0, Icon::Stop, Act::Stop, GREY),
            (690.0, Icon::Play, Act::TogglePlay, GREY),
            (743.0, Icon::ToEnd, Act::GoLast, GREY),
            (
                803.0,
                Icon::Loop,
                Act::ToggleLoop,
                Color32::from_rgb(226, 60, 50),
            ),
        ];
        for (x, icon, act, color) in buttons {
            let color = if icon == Icon::Play && self.playing {
                theme::WHITE
            } else {
                color
            };
            c.icon(x, 654.0, icon, color, 0.95);
            let r = ui.interact(
                c.rect(x - 22.0, 630.0, x + 22.0, 678.0),
                ui.id().with(("fx_transport", x as i32)),
                Sense::click(),
            );
            if r.clicked() {
                self.do_act(ctx, act);
            }
        }
        c.field(1209.0, 634.0, 1316.0, 670.0);
        c.label(
            1262.0,
            652.0,
            Align2::CENTER_CENTER,
            &format!("{here}.0"),
            21.0,
            TEXT,
            false,
        );
    }

    fn effects_tools(&mut self, ui: &mut Ui, c: &Fx) {
        // (x in reference pixels, icon, name)
        let tools: [(f32, Icon, &str); 20] = [
            (37.0, Icon::FxBackground, "Background"),
            (100.0, Icon::FxNoise, "Fast Noise"),
            (162.5, Icon::FxText, "Text"),
            (225.5, Icon::FxPaint, "Paint"),
            (303.5, Icon::FxParticles, "Color Corrector"),
            (368.5, Icon::FxCurves, "Color Curves"),
            (432.0, Icon::FxBrightness, "Brightness Contrast"),
            (492.5, Icon::FxDrop, "Blur"),
            (573.0, Icon::FxMerge, "Merge"),
            (636.0, Icon::FxMergeSplit, "Multi-merge"),
            (696.5, Icon::FxDissolve, "Dissolve"),
            (760.0, Icon::FxMatte, "Matte Control"),
            (825.5, Icon::FxTransform, "Transform"),
            (904.5, Icon::FxRectMask, "Rectangle mask"),
            (968.0, Icon::FxEllipseMask, "Ellipse mask"),
            (1031.0, Icon::FxPolyMask, "Polygon mask"),
            (1093.0, Icon::FxBSpline, "B-spline mask"),
            (1156.0, Icon::FxWave, "Wave mask"),
            (1237.5, Icon::FxEmitter, "Particle emitter"),
            (1298.5, Icon::FxRender, "Particle render"),
        ];
        let mut planned: Option<&str> = None;
        for (x, icon, name) in tools {
            let r = ui
                .interact(
                    c.rect(x - 30.0, 690.0, x + 30.0, 752.0),
                    ui.id().with(("fx_tool", x as i32)),
                    Sense::click(),
                )
                .on_hover_text(name);
            let color = if r.hovered() { theme::ICON_HOVER } else { ICON };
            c.icon(x, 721.0, icon, color, 1.12);
            if r.clicked() {
                use crate::menu::Act;
                use picasso_core::EffectKind;
                let ctx = self.ctx.clone();
                match name {
                    "Background" => self.do_act(&ctx, Act::GenerateSolid),
                    "Fast Noise" => self.do_act(&ctx, Act::AddEffect(EffectKind::FilmGrain)),
                    "Text" => self.do_act(&ctx, Act::AddTextPlus),
                    "Blur" => self.do_act(&ctx, Act::AddEffect(EffectKind::GaussianBlur)),
                    "Dissolve" => self.do_act(&ctx, Act::AddCrossDissolve),
                    "Transform" => self.do_act(&ctx, Act::ShowInspector),
                    "Color Corrector" | "Color Curves" | "Brightness Contrast" => {
                        self.page = crate::app::Page::Color;
                    }
                    "Rectangle mask" | "Ellipse mask" => {
                        self.add_window_from_effects(if name == "Rectangle mask" {
                            picasso_color::WindowShape::Linear
                        } else {
                            picasso_color::WindowShape::Circle
                        })
                    }
                    _ => planned = Some(name),
                }
            }
        }
        for x in [265.0, 533.0, 866.0, 1196.0] {
            c.line((x, 699.0), (x, 743.0), 1.0, Color32::from_rgb(64, 64, 70));
        }
        if let Some(name) = planned {
            self.planned(name);
        }
    }

    fn effects_inspector(&mut self, ui: &mut Ui, c: &Fx, x0: f32, clip: Option<&FxClip>) {
        c.band(x0, 107.0, 2000.0, 143.0, BAR);
        c.text(
            1364.0,
            126.0,
            Align2::LEFT_CENTER,
            "Inspector",
            83.0,
            theme::WHITE,
            true,
        );
        c.icon(1921.0, 125.0, Icon::PanelStack, GREY, 0.95);
        c.vline(1950.0, 107.0, 143.0, LINE);
        c.dots(1980.0, 125.0, GREY);
        c.band(x0, 143.0, 2000.0, 145.0, LINE);
        c.band(x0, 145.0, 2000.0, NODES_BOTTOM, PANEL);

        // The two tabs, Tools and Modifiers.
        let id = ui.id().with("fx_inspector_tab");
        let mut modifiers = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
        let tabs = [
            (1366.0, 1678.0, "Tools", 45.0),
            (1679.0, 1992.0, "Modifiers", 79.0),
        ];
        let outer = c.rect(1365.0, 158.0, 1993.0, 194.0);
        c.p.rect_filled(outer, 3.0, Color32::from_rgb(35, 34, 39));
        for (i, (a, b, name, width)) in tabs.into_iter().enumerate() {
            let on = (i == 1) == modifiers;
            let r = c.rect(a, 159.0, b, 193.0);
            if on {
                c.p.rect_filled(r, 2.0, Color32::from_rgb(23, 25, 26));
            }
            c.text(
                (a + b) / 2.0,
                176.0,
                Align2::CENTER_CENTER,
                name,
                width,
                if on {
                    theme::WHITE
                } else {
                    Color32::from_rgb(140, 140, 146)
                },
                false,
            );
            if ui
                .interact(r, ui.id().with(("fx_tab", i)), Sense::click())
                .clicked()
            {
                modifiers = i == 1;
            }
        }
        c.p.rect_stroke(outer, 3.0, Stroke::new(1.5, LINE), egui::StrokeKind::Inside);
        c.vline(1678.5, 159.0, 193.0, LINE);
        ui.data_mut(|d| d.insert_temp(id, modifiers));
        if let (Some(k), false) = (clip, modifiers) {
            self.fx_media_in(ui, c, k);
        }
    }
}
