//! The Capture page (the camera tab): a photo browser with an empty album strip. Positions are
//! measured from the reference screen (see `chrome.rs`): x in points of the 1872-point-wide
//! window, y from the top of the window for the top bands and from the bottom for the bottom
//! bands.

use egui::{Align2, Color32, Context, FontId, Painter, Pos2, Rect, Shape, Stroke, Ui};

use crate::app::{Page, StudioApp};
use crate::chrome::{Screen, Tool, R};
use crate::icons::{paint, Icon};
use crate::theme::{self, regular, semibold};

/// A font whose text is exactly `width` points wide.
pub(crate) fn fit(p: &Painter, text: &str, width: f32, bold: bool) -> FontId {
    let probe = if bold {
        semibold(100.0)
    } else {
        regular(100.0)
    };
    let w = p
        .layout_no_wrap(text.to_string(), probe, Color32::WHITE)
        .size()
        .x
        .max(1.0);
    let size = 100.0 * width / w;
    if bold {
        semibold(size)
    } else {
        regular(size)
    }
}

const FAINT: Color32 = Color32::from_rgb(58, 58, 63);
const DIM_ICON: Color32 = Color32::from_rgb(104, 104, 108);
const BRIGHT: Color32 = Color32::from_rgb(196, 196, 200);

/// One button of a toolbar: where, which icon, what it says, a drop-down arrow, its size and
/// its colour.
struct Button {
    x: f32,
    icon: Icon,
    hint: &'static str,
    chevron: Option<f32>,
    size: f32,
    color: Option<Color32>,
}

const fn button(x: f32, icon: Icon, hint: &'static str) -> Button {
    Button {
        x,
        icon,
        hint,
        chevron: None,
        size: 1.0,
        color: None,
    }
}

impl Button {
    const fn chev(mut self, dx: f32) -> Self {
        self.chevron = Some(dx);
        self
    }
    const fn size(mut self, k: f32) -> Self {
        self.size = k;
        self
    }
    const fn color(mut self, c: Color32) -> Self {
        self.color = Some(c);
        self
    }
}

impl StudioApp {
    /// Draws `buttons` on the row at `y` and says so when one is pressed that does nothing yet.
    fn button_row(&mut self, ui: &mut Ui, s: &Screen, y: f32, width: f32, buttons: &[Button]) {
        let mut planned: Option<&str> = None;
        for b in buttons {
            let mut tool = Tool::new(b.hint).wide(width).size(b.size);
            if let Some(dx) = b.chevron {
                tool = tool.chev(dx);
            }
            if let Some(c) = b.color {
                tool = tool.color(c);
            }
            if self.tool(ui, s.p(b.x, y), b.icon, tool).clicked() {
                planned = Some(b.hint);
            }
        }
        if let Some(what) = planned {
            self.planned(what);
        }
    }

    /// The icons of the first toolbar of the Capture page.
    pub(crate) fn capture_toolbar1(&mut self, ui: &mut Ui, ctx: &Context, s: &Screen, y: f32) {
        if self
            .tool(
                ui,
                s.p(40.0, y),
                Icon::PhotoMusic,
                Tool::new("Import media").color(BRIGHT),
            )
            .clicked()
        {
            self.import_dialog(ctx);
        }
        if self
            .tool(
                ui,
                s.p(1686.0, y),
                Icon::Export,
                Tool::new("Export").size(0.85),
            )
            .clicked()
        {
            self.page = Page::Deliver;
        }
        if self
            .tool(
                ui,
                s.p(1834.0, y),
                Icon::Tools,
                Tool::new("Project settings").size(0.82),
            )
            .clicked()
        {
            self.show_settings = true;
        }
        self.button_row(
            ui,
            s,
            y,
            46.0,
            &[
                button(115.0, Icon::Wand, "Effects").size(0.95),
                button(189.5, Icon::PhotoStack, "Photo albums")
                    .chev(42.5)
                    .color(BRIGHT),
                button(1761.0, Icon::LabelTag, "Labels").size(0.95),
            ],
        );
    }

    /// The page in pixels of Resolve at 1080p (`rv`): its body, from the second toolbar to the page
    /// bar, fills the window's.
    pub(crate) fn capture_page(&mut self, ui: &mut Ui, ctx: &Context, s: &Screen) {
        let painter = ui.painter().clone();
        let vf = (s.h - 46.8 - 92.0).max(300.0) / 965.0;
        let rv = |x: f32, y: f32| s.p(x * R, 92.0 + (y - 35.0) * vf);
        let rr = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(rv(x0, y0), rv(x1, y1));
        let fill = |x0: f32, y0: f32, x1: f32, y1: f32, c: Color32| {
            painter.rect_filled(rr(x0, y0, x1, y1), 0.0, c);
        };
        let text = |x: f32, y: f32, a: Align2, t: &str, size: f32, c: Color32, bold: bool| {
            let font = if bold {
                semibold(size * R)
            } else {
                regular(size * R)
            };
            painter.text(rv(x, y), a, t, font, c);
        };

        // The second toolbar, the pool, the viewer and the inspector.
        fill(0.0, 35.0, 1919.0, 59.0, theme::TOOLBAR2_BG);
        fill(0.0, 58.0, 1919.0, 59.0, Color32::from_rgb(12, 11, 16));
        fill(0.0, 59.0, 461.0, 864.0, theme::POOL_BG);
        fill(0.0, 59.0, 461.0, 87.0, theme::POOL_HEADER_BG);
        fill(0.0, 87.0, 461.0, 88.0, Color32::from_rgb(37, 37, 39));
        fill(461.0, 59.0, 463.0, 864.0, Color32::from_rgb(19, 18, 21));
        text(
            13.0,
            73.0,
            Align2::LEFT_CENTER,
            "Master",
            9.0,
            theme::WHITE,
            false,
        );
        fill(463.0, 59.0, 1497.0, 822.0, theme::VIEWER_BG);
        fill(463.0, 821.0, 1497.0, 822.0, Color32::from_rgb(9, 9, 9));
        fill(463.0, 822.0, 1497.0, 864.0, theme::TOOLBAR2_BG);
        fill(1497.0, 59.0, 1499.0, 864.0, Color32::from_rgb(19, 18, 21));
        fill(1499.0, 59.0, 1919.0, 864.0, theme::POOL_BG);
        fill(1499.0, 59.0, 1919.0, 111.0, Color32::from_rgb(33, 32, 37));
        fill(1499.0, 111.0, 1919.0, 112.0, Color32::from_rgb(19, 18, 21));
        for (x, icon, name) in [
            (1613.0, Icon::PageCapture, "RAW"),
            (1677.0, Icon::Photo, "Photo"),
            (1741.0, Icon::Wand, "Effects"),
            (1805.0, Icon::Sliders, "File"),
        ] {
            crate::icons::paint_scaled(&painter, rv(x, 78.0), icon, FAINT, 0.55 * R);
            text(x, 100.0, Align2::CENTER_CENTER, name, 8.5, FAINT, false);
        }
        text(
            1704.0,
            489.0,
            Align2::CENTER_CENTER,
            "Nothing to inspect",
            27.0,
            theme::EMPTY_TITLE,
            false,
        );

        // The album strip: filters on the left, an empty album on the right.
        fill(0.0, 864.0, 100.0, 1000.0, Color32::from_rgb(35, 34, 39));
        fill(100.0, 864.0, 1919.0, 1000.0, theme::POOL_BG);
        fill(100.0, 864.0, 102.0, 1000.0, Color32::from_rgb(22, 22, 26));
        fill(0.0, 864.0, 1919.0, 865.0, Color32::from_rgb(19, 18, 21));
        painter.rect_filled(
            rr(16.0, 877.0, 44.0, 890.0),
            6.0 * R,
            Color32::from_rgb(28, 27, 31),
        );
        painter.circle_filled(rv(32.0, 883.5), 3.0 * R, Color32::from_rgb(166, 58, 56));
        text(
            49.0,
            883.0,
            Align2::LEFT_CENTER,
            "Filters",
            9.0,
            Color32::from_rgb(64, 64, 70),
            false,
        );
        for (x, y, icon) in [
            (31.0, 913.0, Icon::Heart),
            (71.0, 913.0, Icon::Star),
            (31.0, 945.0, Icon::Bookmark),
            (71.0, 945.0, Icon::Circles3),
            (32.0, 977.0, Icon::Tag),
            (72.0, 977.0, Icon::Sort),
        ] {
            let size = if icon == Icon::Heart { 0.8 } else { 0.62 };
            crate::icons::paint_scaled(&painter, rv(x, y), icon, FAINT, size * R);
        }
        text(
            1005.0,
            919.0,
            Align2::CENTER_CENTER,
            "Create an Album",
            27.0,
            theme::EMPTY_TITLE,
            false,
        );
        text(
            1009.0,
            957.0,
            Align2::CENTER_CENTER,
            "Drag photos into the Photo Album to start editing",
            12.5,
            theme::EMPTY_SUB,
            false,
        );

        self.capture_toolbar2(ui, s, &rv);
        self.capture_view_bar(ui, s, &rv);
        let _ = ctx;
    }

    #[allow(clippy::too_many_arguments)]
    /// One button given in Resolve's pixels: its icon at `x`, an arrow at `chevron`.
    fn photo_button(
        &mut self,
        ui: &mut Ui,
        at: Pos2,
        icon: Icon,
        hint: &'static str,
        size: f32,
        color: Option<Color32>,
        chevron: Option<f32>,
        wide: f32,
    ) -> bool {
        let mut tool = Tool::new(hint).wide(wide * R).size(size * R);
        if let Some(dx) = chevron {
            tool = tool.chev(dx * R);
        }
        if let Some(c) = color {
            tool = tool.color(c);
        }
        let clicked = self.tool(ui, at, icon, tool).clicked();
        if clicked {
            self.planned(hint);
        }
        clicked
    }

    fn capture_toolbar2(&mut self, ui: &mut Ui, s: &Screen, rv: &dyn Fn(f32, f32) -> Pos2) {
        let _ = s;
        let painter = ui.painter().clone();
        let y = 46.0;
        // (x, icon, hint, size, colour, the arrow's distance)
        #[allow(clippy::type_complexity)]
        let buttons: [(f32, Icon, &'static str, f32, Option<Color32>, Option<f32>); 15] = [
            (
                18.0,
                Icon::Sidebar,
                "Show or hide the media pool",
                0.58,
                None,
                Some(24.0),
            ),
            (75.0, Icon::NewBin, "New album", 0.58, None, Some(20.0)),
            (129.0, Icon::Unlink, "Link", 0.62, None, None),
            (
                166.0,
                Icon::ImportImage,
                "Import photos",
                0.62,
                None,
                Some(23.0),
            ),
            (
                317.0,
                Icon::Grid3,
                "Thumbnails",
                0.56,
                Some(theme::WHITE),
                Some(20.0),
            ),
            (369.0, Icon::Search, "Search", 0.56, Some(BRIGHT), None),
            (406.0, Icon::Heart, "Favourites", 0.8, Some(BRIGHT), None),
            (443.0, Icon::Sort, "Sort", 0.62, None, None),
            (480.0, Icon::Photo, "Photos", 0.55, None, None),
            (
                510.0,
                Icon::PhotoStackSmall,
                "Albums",
                0.6,
                Some(BRIGHT),
                None,
            ),
            (
                1297.0,
                Icon::SingleView,
                "Single viewer",
                0.6,
                Some(BRIGHT),
                None,
            ),
            (1326.0, Icon::DenseGrid, "Grid viewer", 0.8, None, None),
            (
                1365.0,
                Icon::FitView,
                "Fit to window",
                0.5,
                Some(DIM_ICON),
                Some(23.0),
            ),
            (
                1424.0,
                Icon::WindowLayout,
                "Viewer layout",
                0.6,
                None,
                Some(22.0),
            ),
            (
                1477.0,
                Icon::ColorWheel,
                "Image enhancements",
                0.65,
                None,
                None,
            ),
        ];
        for (x, icon, hint, size, color, chev) in buttons {
            self.photo_button(ui, rv(x, y), icon, hint, size, color, chev, 30.0);
        }
        self.photo_button(
            ui,
            rv(1864.0, y),
            Icon::Search,
            "Search the inspector",
            0.56,
            Some(BRIGHT),
            None,
            24.0,
        );
        for (x, a) in [(234.0, "Master"), (554.0, "81%")] {
            painter.text(
                rv(x, y),
                Align2::CENTER_CENTER,
                a,
                semibold(10.5 * R),
                theme::WHITE,
            );
        }
        for x in [581.0, 999.0] {
            paint_chevron(&painter, rv(x, y), theme::ICON);
        }
        for dx in [-6.0, 0.0, 6.0] {
            painter.circle_filled(rv(1901.0 + dx, y), 1.2 * R, DIM_ICON);
        }
    }

    fn capture_view_bar(&mut self, ui: &mut Ui, s: &Screen, rv: &dyn Fn(f32, f32) -> Pos2) {
        let _ = s;
        let painter = ui.painter().clone();
        let y = 843.0;
        for (x, icon, hint) in [
            (491.0, Icon::Sliders, "Image controls"),
            (1005.0, Icon::PhotoHeart, "Mark as favourite"),
            (1043.0, Icon::Photo, "Keep"),
            (1085.0, Icon::PhotoX, "Reject"),
        ] {
            self.photo_button(ui, rv(x, y), icon, hint, 0.62, Some(DIM_ICON), None, 36.0);
        }
        for x in [974.0, 1114.0, 1250.0, 1357.0, 1428.0] {
            painter.line_segment(
                [rv(x, 823.0), rv(x, 864.0)],
                Stroke::new(1.0, Color32::from_rgb(12, 12, 14)),
            );
        }
        // A drop-down field with a dash in it.
        painter.rect_filled(
            Rect::from_min_max(rv(1130.0, 833.0), rv(1235.0, 853.0)),
            2.0,
            Color32::from_rgb(33, 32, 35),
        );
        painter.text(
            rv(1142.0, 844.0),
            Align2::LEFT_CENTER,
            "--",
            regular(9.0 * R),
            FAINT,
        );
        paint_chevron(&painter, rv(1219.0, y), FAINT);
        // Five stars to rate with.
        for n in 0..5 {
            let x = 1273.0 + 15.0 * n as f32;
            let star: Vec<Pos2> = (0..10)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 10.0;
                    let r = if i % 2 == 0 { 4.2 } else { 1.8 } * R;
                    rv(x, y) + egui::vec2(r * a.cos(), r * a.sin())
                })
                .collect();
            painter.add(Shape::convex_polygon(
                star,
                Color32::from_rgb(52, 52, 57),
                Stroke::NONE,
            ));
        }
        painter.circle_stroke(
            rv(1381.0, y),
            3.4 * R,
            Stroke::new(1.0, Color32::from_rgb(110, 110, 114)),
        );
        paint_chevron(&painter, rv(1402.0, y), DIM_ICON);
        paint(
            &painter,
            rv(1452.0, y),
            Icon::Bookmark,
            Color32::from_rgb(104, 104, 108),
        );
        paint_chevron(&painter, rv(1475.0, y), DIM_ICON);
    }
}

/// A small drop-down arrow at Resolve's size.
fn paint_chevron(p: &Painter, c: Pos2, color: Color32) {
    let u = 0.68 * R;
    p.add(Shape::line(
        vec![
            c + egui::vec2(-5.0 * u, -2.5 * u),
            c + egui::vec2(0.0, 2.5 * u),
            c + egui::vec2(5.0 * u, -2.5 * u),
        ],
        Stroke::new(1.2, color),
    ));
}
