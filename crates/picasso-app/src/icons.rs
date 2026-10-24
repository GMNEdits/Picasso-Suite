//! Vector icons drawn in code. Each is an original drawing laid out on a unit
//! square and scaled into its rectangle.

use egui::epaint::{CircleShape, PathShape, PathStroke};
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    // interface toolbar
    MediaPool,
    Effects,
    Index,
    SoundLibrary,
    Keyframes,
    QuickExport,
    Mixer,
    Metadata,
    Inspector,
    // panel headers
    Sidebar,
    Grid,
    List,
    Search,
    Sort,
    Import,
    // pages
    PageMedia,
    PagePhoto,
    PageCut,
    PageEdit,
    PageEffects,
    PageColor,
    PageAudio,
    PageDeliver,
    Home,
    Settings,
    // transport
    ToStart,
    Stop,
    Play,
    Pause,
    PlayReverse,
    ToEnd,
    Loop,
    // timeline tools
    Select,
    Trim,
    Blade,
    Snapping,
    Link,
    PositionLock,
    Flag,
    Marker,
    ZoomFit,
    ZoomDetail,
    Speaker,
    Lock,
    Eye,
    AutoSelect,
    Chain,
    // cut page
    SyncBin,
    SourceTape,
    Titles,
    Transitions,
    FullScreen,
    Tools,
    Unlink,
    AddImage,
    Refresh,
    ViewImage,
    ViewFilmstrip,
    ViewTimeline,
    Grid4,
    FiveMinutes,
    Antenna,
    Clock,
    SafeArea,
    HighQuality,
    Overlay,
    ColorSparkle,
    SmartInsert,
    Append,
    RippleOverwrite,
    CloseUp,
    PlaceOnTop,
    SourceOverwrite,
    Split,
    Sliders,
    Mic,
    PoiIn,
    PoiOut,
    PlayFilm,
    Prev,
    Dot,
    Next,
    NextEdit,
    PrevEdit,
    Hamburger,
    TrimTools,
    TrimRipple,
    TrimAudio,
    ZoomFrame,
    ZoomDetailDim,
    BlueMarker,
    // window
    Minimize,
    Maximize,
    Close,
}

struct Pen<'a> {
    painter: &'a Painter,
    rect: Rect,
    color: Color32,
    width: f32,
}

impl Pen<'_> {
    fn p(&self, x: f32, y: f32) -> Pos2 {
        pos2(self.rect.left() + x * self.rect.width(), self.rect.top() + y * self.rect.height())
    }
    fn s(&self) -> Stroke {
        Stroke::new(self.width, self.color)
    }
    fn line(&self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.painter.line_segment([self.p(x0, y0), self.p(x1, y1)], self.s());
    }
    fn poly(&self, pts: &[(f32, f32)], closed: bool) {
        let pts: Vec<Pos2> = pts.iter().map(|(x, y)| self.p(*x, *y)).collect();
        let shape = if closed { PathShape::closed_line(pts, self.s()) } else { PathShape::line(pts, self.s()) };
        self.painter.add(shape);
    }
    fn fill(&self, pts: &[(f32, f32)]) {
        let pts: Vec<Pos2> = pts.iter().map(|(x, y)| self.p(*x, *y)).collect();
        self.painter.add(Shape::Path(PathShape { points: pts, closed: true, fill: self.color, stroke: PathStroke::NONE }));
    }
    fn rect(&self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.painter.rect_stroke(Rect::from_min_max(self.p(x0, y0), self.p(x1, y1)), 1.0, self.s(), egui::StrokeKind::Middle);
    }
    fn rect_fill(&self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.painter.rect_filled(Rect::from_min_max(self.p(x0, y0), self.p(x1, y1)), 1.0, self.color);
    }
    fn circle(&self, x: f32, y: f32, r: f32) {
        self.painter.add(CircleShape::stroke(self.p(x, y), r * self.rect.width(), self.s()));
    }
    fn dot(&self, x: f32, y: f32, r: f32) {
        self.painter.add(CircleShape::filled(self.p(x, y), r * self.rect.width(), self.color));
    }
    fn arc(&self, cx: f32, cy: f32, r: f32, a0: f32, a1: f32) {
        let n = 20;
        let pts: Vec<(f32, f32)> =
            (0..=n).map(|i| a0 + (a1 - a0) * i as f32 / n as f32).map(|a| (cx + r * a.cos(), cy + r * a.sin())).collect();
        self.poly(&pts, false);
    }
}

/// Draws `icon` centred in `rect` (drawn in a square of the smaller side).
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let side = rect.width().min(rect.height());
    let rect = Rect::from_center_size(rect.center(), vec2(side, side));
    let pen = Pen { painter, rect, color, width: (side / 13.0).clamp(1.0, 2.2) };
    use Icon::*;
    let t = std::f32::consts::PI;
    match icon {
        MediaPool => {
            pen.rect(0.08, 0.2, 0.62, 0.62);
            pen.rect(0.38, 0.38, 0.92, 0.8);
            pen.poly(&[(0.44, 0.74), (0.58, 0.56), (0.68, 0.66), (0.76, 0.58), (0.88, 0.74)], false);
        }
        Effects => {
            pen.line(0.15, 0.85, 0.62, 0.38);
            for (x, y) in [(0.72, 0.16), (0.84, 0.3), (0.7, 0.44), (0.56, 0.28)] {
                pen.dot(x, y, 0.045);
            }
        }
        Index => {
            for y in [0.25, 0.5, 0.75] {
                pen.dot(0.14, y, 0.05);
                pen.line(0.3, y, 0.9, y);
            }
        }
        SoundLibrary => {
            pen.rect(0.1, 0.18, 0.62, 0.82);
            pen.line(0.36, 0.34, 0.36, 0.66);
            pen.dot(0.3, 0.66, 0.07);
            pen.line(0.74, 0.3, 0.74, 0.74);
            pen.line(0.88, 0.4, 0.88, 0.64);
        }
        Keyframes => {
            pen.poly(&[(0.3, 0.5), (0.5, 0.3), (0.7, 0.5), (0.5, 0.7)], true);
            pen.line(0.05, 0.5, 0.25, 0.5);
            pen.line(0.75, 0.5, 0.95, 0.5);
        }
        QuickExport => {
            pen.poly(&[(0.2, 0.45), (0.2, 0.86), (0.8, 0.86), (0.8, 0.45)], false);
            pen.line(0.5, 0.12, 0.5, 0.62);
            pen.poly(&[(0.32, 0.3), (0.5, 0.12), (0.68, 0.3)], false);
        }
        Mixer => {
            for (x, k) in [(0.22, 0.65), (0.5, 0.35), (0.78, 0.55)] {
                pen.line(x, 0.12, x, 0.88);
                pen.rect_fill(x - 0.09, k - 0.06, x + 0.09, k + 0.06);
            }
        }
        Metadata => {
            pen.rect(0.18, 0.1, 0.82, 0.9);
            for y in [0.3, 0.48, 0.66] {
                pen.line(0.32, y, 0.68, y);
            }
        }
        Inspector => {
            for (y, k) in [(0.25, 0.65), (0.5, 0.32), (0.75, 0.55)] {
                pen.line(0.1, y, 0.9, y);
                pen.dot(k, y, 0.08);
            }
        }
        Sidebar => {
            pen.rect(0.1, 0.18, 0.9, 0.82);
            pen.rect_fill(0.1, 0.18, 0.38, 0.82);
        }
        Grid => {
            for x in [0.12, 0.42, 0.72] {
                for y in [0.22, 0.56] {
                    pen.rect_fill(x, y, x + 0.18, y + 0.22);
                }
            }
        }
        List => {
            for y in [0.25, 0.5, 0.75] {
                pen.line(0.12, y, 0.88, y);
            }
        }
        Search => {
            pen.circle(0.42, 0.42, 0.24);
            pen.line(0.6, 0.6, 0.86, 0.86);
        }
        Sort => {
            pen.line(0.3, 0.15, 0.3, 0.85);
            pen.poly(&[(0.15, 0.7), (0.3, 0.85), (0.45, 0.7)], false);
            for (y, w) in [(0.3, 0.86), (0.5, 0.78), (0.7, 0.7)] {
                pen.line(0.58, y, w, y);
            }
        }
        Import => {
            pen.poly(&[(0.15, 0.55), (0.15, 0.88), (0.85, 0.88), (0.85, 0.55)], false);
            pen.line(0.5, 0.1, 0.5, 0.66);
            pen.poly(&[(0.3, 0.46), (0.5, 0.66), (0.7, 0.46)], false);
        }
        PageMedia => {
            pen.rect(0.1, 0.2, 0.9, 0.8);
            pen.poly(&[(0.18, 0.72), (0.4, 0.45), (0.56, 0.62), (0.66, 0.52), (0.82, 0.72)], false);
            pen.dot(0.7, 0.34, 0.06);
        }
        PagePhoto => {
            pen.poly(&[(0.1, 0.3), (0.32, 0.3), (0.4, 0.18), (0.6, 0.18), (0.68, 0.3), (0.9, 0.3), (0.9, 0.82), (0.1, 0.82)], true);
            pen.circle(0.5, 0.56, 0.17);
        }
        PageCut => {
            for (y, x0, x1) in [(0.3, 0.3, 0.9), (0.5, 0.15, 0.75), (0.7, 0.35, 0.85)] {
                pen.line(x0, y, x1, y);
            }
            pen.line(0.55, 0.12, 0.42, 0.88);
        }
        PageEdit => {
            pen.rect_fill(0.08, 0.24, 0.5, 0.38);
            pen.rect_fill(0.56, 0.24, 0.92, 0.38);
            pen.rect_fill(0.2, 0.46, 0.7, 0.6);
            pen.rect_fill(0.08, 0.68, 0.4, 0.82);
            pen.rect_fill(0.46, 0.68, 0.92, 0.82);
        }
        PageEffects => {
            for (x, y) in [(0.5, 0.18), (0.22, 0.4), (0.78, 0.4), (0.32, 0.76), (0.68, 0.76), (0.5, 0.5)] {
                pen.dot(x, y, 0.07);
            }
            pen.poly(&[(0.5, 0.18), (0.5, 0.5), (0.32, 0.76)], false);
            pen.line(0.5, 0.5, 0.78, 0.4);
        }
        PageColor => {
            pen.circle(0.5, 0.5, 0.36);
            pen.dot(0.5, 0.3, 0.07);
            pen.dot(0.33, 0.6, 0.07);
            pen.dot(0.67, 0.6, 0.07);
        }
        PageAudio => {
            pen.line(0.4, 0.2, 0.4, 0.72);
            pen.line(0.78, 0.12, 0.78, 0.62);
            pen.line(0.4, 0.2, 0.78, 0.12);
            pen.dot(0.32, 0.74, 0.1);
            pen.dot(0.7, 0.64, 0.1);
        }
        PageDeliver => {
            pen.poly(&[(0.5, 0.08), (0.66, 0.3), (0.66, 0.68), (0.34, 0.68), (0.34, 0.3)], true);
            pen.circle(0.5, 0.4, 0.07);
            pen.poly(&[(0.34, 0.52), (0.2, 0.7), (0.34, 0.68)], false);
            pen.poly(&[(0.66, 0.52), (0.8, 0.7), (0.66, 0.68)], false);
            pen.line(0.5, 0.74, 0.5, 0.92);
        }
        Home => {
            pen.poly(&[(0.12, 0.5), (0.5, 0.15), (0.88, 0.5)], false);
            pen.poly(&[(0.22, 0.42), (0.22, 0.86), (0.78, 0.86), (0.78, 0.42)], false);
        }
        Settings => {
            pen.circle(0.5, 0.5, 0.2);
            for i in 0..8 {
                let a = i as f32 * t / 4.0;
                pen.line(0.5 + 0.28 * a.cos(), 0.5 + 0.28 * a.sin(), 0.5 + 0.4 * a.cos(), 0.5 + 0.4 * a.sin());
            }
        }
        ToStart => {
            pen.rect_fill(0.14, 0.25, 0.22, 0.75);
            pen.fill(&[(0.52, 0.25), (0.22, 0.5), (0.52, 0.75)]);
            pen.fill(&[(0.84, 0.25), (0.54, 0.5), (0.84, 0.75)]);
        }
        Stop => pen.rect_fill(0.28, 0.28, 0.72, 0.72),
        Play => pen.fill(&[(0.3, 0.22), (0.78, 0.5), (0.3, 0.78)]),
        Pause => {
            pen.rect_fill(0.3, 0.25, 0.43, 0.75);
            pen.rect_fill(0.57, 0.25, 0.7, 0.75);
        }
        PlayReverse => pen.fill(&[(0.7, 0.22), (0.22, 0.5), (0.7, 0.78)]),
        ToEnd => {
            pen.fill(&[(0.16, 0.25), (0.46, 0.5), (0.16, 0.75)]);
            pen.fill(&[(0.48, 0.25), (0.78, 0.5), (0.48, 0.75)]);
            pen.rect_fill(0.78, 0.25, 0.86, 0.75);
        }
        Loop => {
            pen.arc(0.5, 0.5, 0.3, 0.2 * t, 1.75 * t);
            pen.poly(&[(0.7, 0.3), (0.8, 0.44), (0.88, 0.28)], false);
        }
        Select => pen.fill(&[(0.3, 0.12), (0.3, 0.82), (0.46, 0.66), (0.58, 0.9), (0.68, 0.85), (0.56, 0.62), (0.78, 0.62)]),
        Trim => {
            pen.poly(&[(0.42, 0.15), (0.3, 0.15), (0.3, 0.85), (0.42, 0.85)], false);
            pen.poly(&[(0.58, 0.15), (0.7, 0.15), (0.7, 0.85), (0.58, 0.85)], false);
            pen.line(0.5, 0.3, 0.5, 0.7);
        }
        Blade => {
            pen.circle(0.3, 0.72, 0.12);
            pen.circle(0.7, 0.72, 0.12);
            pen.line(0.36, 0.62, 0.66, 0.12);
            pen.line(0.64, 0.62, 0.34, 0.12);
        }
        Snapping => {
            pen.poly(&[(0.22, 0.15), (0.22, 0.55), (0.3, 0.75), (0.5, 0.85), (0.7, 0.75), (0.78, 0.55), (0.78, 0.15)], false);
            pen.poly(&[(0.38, 0.15), (0.38, 0.55), (0.5, 0.66), (0.62, 0.55), (0.62, 0.15)], false);
        }
        Link => {
            pen.poly(&[(0.42, 0.35), (0.3, 0.23), (0.15, 0.38), (0.32, 0.55)], false);
            pen.poly(&[(0.58, 0.65), (0.7, 0.77), (0.85, 0.62), (0.68, 0.45)], false);
            pen.line(0.36, 0.64, 0.64, 0.36);
        }
        PositionLock => {
            pen.rect(0.22, 0.45, 0.78, 0.86);
            pen.arc(0.5, 0.45, 0.18, t, 2.0 * t);
        }
        Flag => {
            pen.line(0.25, 0.1, 0.25, 0.9);
            pen.fill(&[(0.25, 0.12), (0.8, 0.28), (0.25, 0.46)]);
        }
        Marker => pen.fill(&[(0.28, 0.14), (0.72, 0.14), (0.72, 0.62), (0.5, 0.86), (0.28, 0.62)]),
        ZoomFit => {
            pen.rect(0.12, 0.3, 0.88, 0.7);
            pen.poly(&[(0.3, 0.42), (0.2, 0.5), (0.3, 0.58)], false);
            pen.poly(&[(0.7, 0.42), (0.8, 0.5), (0.7, 0.58)], false);
        }
        ZoomDetail => {
            pen.rect(0.12, 0.3, 0.88, 0.7);
            pen.line(0.5, 0.18, 0.5, 0.82);
        }
        Speaker => {
            pen.fill(&[(0.12, 0.38), (0.3, 0.38), (0.52, 0.18), (0.52, 0.82), (0.3, 0.62), (0.12, 0.62)]);
            pen.arc(0.52, 0.5, 0.18, -0.3 * t, 0.3 * t);
            pen.arc(0.52, 0.5, 0.32, -0.3 * t, 0.3 * t);
        }
        Lock => {
            pen.rect_fill(0.24, 0.46, 0.76, 0.86);
            pen.arc(0.5, 0.46, 0.17, t, 2.0 * t);
        }
        Eye => {
            pen.arc(0.5, 0.95, 0.55, 1.22 * t, 1.78 * t);
            pen.arc(0.5, 0.05, 0.55, 0.22 * t, 0.78 * t);
            pen.dot(0.5, 0.5, 0.11);
        }
        AutoSelect => {
            pen.rect(0.15, 0.25, 0.85, 0.75);
            pen.fill(&[(0.38, 0.36), (0.38, 0.64), (0.62, 0.5)]);
        }
        Chain => {
            pen.rect(0.1, 0.36, 0.52, 0.64);
            pen.rect(0.48, 0.36, 0.9, 0.64);
        }
        SyncBin => {
            pen.rect(0.12, 0.2, 0.62, 0.48);
            pen.rect(0.38, 0.55, 0.88, 0.83);
            pen.poly(&[(0.72, 0.2), (0.88, 0.2), (0.88, 0.42)], false);
            pen.poly(&[(0.28, 0.83), (0.12, 0.83), (0.12, 0.6)], false);
        }
        SourceTape => {
            pen.rect(0.1, 0.22, 0.9, 0.78);
            pen.line(0.1, 0.22, 0.5, 0.52);
            pen.line(0.9, 0.22, 0.5, 0.52);
            pen.line(0.1, 0.78, 0.5, 0.5);
            pen.line(0.9, 0.78, 0.5, 0.5);
        }
        Titles => {
            pen.rect(0.12, 0.24, 0.88, 0.76);
            pen.line(0.36, 0.38, 0.64, 0.38);
            pen.line(0.5, 0.38, 0.5, 0.64);
        }
        Transitions => {
            pen.poly(&[(0.3, 0.32), (0.14, 0.5), (0.3, 0.68)], false);
            pen.poly(&[(0.44, 0.32), (0.28, 0.5), (0.44, 0.68)], false);
            pen.fill(&[(0.66, 0.3), (0.86, 0.5), (0.66, 0.7), (0.46, 0.5)]);
        }
        FullScreen | SafeArea => {
            pen.rect(0.12, 0.22, 0.88, 0.78);
            pen.poly(&[(0.24, 0.4), (0.24, 0.32), (0.34, 0.32)], false);
            pen.poly(&[(0.66, 0.32), (0.76, 0.32), (0.76, 0.4)], false);
            pen.poly(&[(0.24, 0.6), (0.24, 0.68), (0.34, 0.68)], false);
            pen.poly(&[(0.66, 0.68), (0.76, 0.68), (0.76, 0.6)], false);
        }
        Tools => {
            pen.line(0.2, 0.2, 0.8, 0.8);
            pen.line(0.8, 0.2, 0.2, 0.8);
            pen.dot(0.2, 0.2, 0.07);
            pen.dot(0.8, 0.8, 0.07);
        }
        Unlink => {
            pen.poly(&[(0.4, 0.3), (0.28, 0.2), (0.14, 0.34), (0.26, 0.46)], false);
            pen.poly(&[(0.6, 0.7), (0.72, 0.8), (0.86, 0.66), (0.74, 0.54)], false);
            pen.line(0.45, 0.15, 0.5, 0.28);
            pen.line(0.55, 0.85, 0.5, 0.72);
        }
        AddImage => {
            pen.rect(0.1, 0.22, 0.74, 0.72);
            pen.poly(&[(0.16, 0.66), (0.34, 0.46), (0.48, 0.6), (0.58, 0.52), (0.7, 0.64)], false);
            pen.line(0.8, 0.62, 0.8, 0.9);
            pen.line(0.66, 0.76, 0.94, 0.76);
        }
        Refresh => {
            pen.arc(0.5, 0.5, 0.3, 0.15 * t, 1.1 * t);
            pen.arc(0.5, 0.5, 0.3, 1.15 * t, 2.1 * t);
            pen.poly(&[(0.82, 0.38), (0.8, 0.56), (0.66, 0.5)], false);
        }
        ViewImage => {
            pen.rect(0.12, 0.24, 0.88, 0.76);
            pen.poly(&[(0.2, 0.68), (0.4, 0.46), (0.54, 0.6), (0.64, 0.52), (0.8, 0.68)], false);
        }
        ViewFilmstrip => {
            pen.rect(0.26, 0.28, 0.74, 0.72);
            for x in [0.08, 0.16, 0.84, 0.92] {
                pen.line(x, 0.24, x, 0.76);
            }
        }
        ViewTimeline => {
            pen.rect(0.1, 0.24, 0.9, 0.76);
            pen.rect_fill(0.1, 0.24, 0.9, 0.48);
            pen.line(0.5, 0.5, 0.5, 0.76);
        }
        Grid4 => {
            for (x, y) in [(0.14, 0.2), (0.54, 0.2), (0.14, 0.54), (0.54, 0.54)] {
                pen.rect(x, y, x + 0.32, y + 0.26);
            }
        }
        FiveMinutes => {
            pen.rect(0.12, 0.26, 0.88, 0.74);
            pen.painter.text(pen.p(0.5, 0.5), egui::Align2::CENTER_CENTER, "5m", egui::FontId::proportional(pen.rect.height() * 0.38), pen.color);
        }
        Antenna => {
            pen.dot(0.5, 0.4, 0.06);
            pen.arc(0.5, 0.4, 0.2, 1.15 * t, 1.85 * t);
            pen.arc(0.5, 0.4, 0.34, 1.15 * t, 1.85 * t);
            pen.line(0.5, 0.46, 0.34, 0.88);
            pen.line(0.5, 0.46, 0.66, 0.88);
            pen.line(0.4, 0.72, 0.6, 0.72);
        }
        Clock => {
            pen.circle(0.5, 0.52, 0.34);
            pen.line(0.5, 0.52, 0.5, 0.3);
            pen.line(0.5, 0.52, 0.68, 0.6);
        }
        HighQuality => {
            pen.rect(0.08, 0.2, 0.8, 0.68);
            pen.rect_fill(0.42, 0.5, 0.94, 0.86);
            pen.painter.text(pen.p(0.68, 0.68), egui::Align2::CENTER_CENTER, "HQ", egui::FontId::proportional(pen.rect.height() * 0.3), Color32::from_rgb(20, 20, 22));
        }
        Overlay => {
            pen.rect(0.1, 0.24, 0.9, 0.76);
            pen.fill(&[(0.22, 0.7), (0.5, 0.36), (0.78, 0.7)]);
        }
        ColorSparkle => {
            let colors = [
                Color32::from_rgb(235, 80, 70),
                Color32::from_rgb(240, 200, 60),
                Color32::from_rgb(80, 200, 120),
                Color32::from_rgb(80, 150, 240),
                Color32::from_rgb(200, 90, 220),
            ];
            for (i, c) in colors.iter().enumerate() {
                let a0 = -0.2 * t + i as f32 * 0.32 * t;
                let pen2 = Pen { painter: pen.painter, rect: pen.rect, color: *c, width: pen.width * 1.2 };
                pen2.arc(0.55, 0.48, 0.3, a0, a0 + 0.3 * t);
            }
            pen.dot(0.3, 0.72, 0.06);
            pen.dot(0.42, 0.62, 0.04);
        }
        SmartInsert => {
            pen.rect(0.12, 0.42, 0.66, 0.78);
            pen.line(0.52, 0.12, 0.52, 0.34);
            pen.poly(&[(0.42, 0.26), (0.52, 0.36), (0.62, 0.26)], false);
            pen.line(0.74, 0.5, 0.74, 0.86);
            pen.dot(0.74, 0.86, 0.06);
        }
        Append => {
            pen.rect(0.1, 0.3, 0.5, 0.64);
            pen.rect(0.46, 0.44, 0.86, 0.78);
        }
        RippleOverwrite => {
            pen.rect(0.1, 0.38, 0.9, 0.7);
            pen.rect(0.34, 0.24, 0.66, 0.5);
            pen.poly(&[(0.2, 0.46), (0.12, 0.54), (0.2, 0.62)], false);
            pen.poly(&[(0.8, 0.46), (0.88, 0.54), (0.8, 0.62)], false);
        }
        CloseUp => {
            pen.rect(0.16, 0.3, 0.84, 0.86);
            pen.circle(0.5, 0.56, 0.1);
            pen.poly(&[(0.3, 0.86), (0.5, 0.7), (0.7, 0.86)], false);
            pen.line(0.62, 0.12, 0.62, 0.28);
            pen.poly(&[(0.54, 0.22), (0.62, 0.3), (0.7, 0.22)], false);
        }
        PlaceOnTop | Split => {
            pen.rect(0.12, 0.3, 0.88, 0.7);
            pen.line(0.5, 0.2, 0.5, 0.8);
        }
        SourceOverwrite => {
            pen.rect(0.12, 0.3, 0.88, 0.7);
            pen.fill(&[(0.3, 0.64), (0.5, 0.42), (0.7, 0.64)]);
        }
        Sliders => {
            for (y, k) in [(0.28, 0.66), (0.5, 0.36), (0.72, 0.58)] {
                pen.line(0.1, y, 0.9, y);
                pen.rect_fill(k - 0.06, y - 0.07, k + 0.06, y + 0.07);
            }
        }
        Mic => {
            pen.rect(0.36, 0.12, 0.6, 0.56);
            pen.arc(0.48, 0.52, 0.2, 0.0, t);
            pen.line(0.48, 0.72, 0.48, 0.88);
            pen.line(0.7, 0.24, 0.7, 0.44);
            pen.line(0.78, 0.3, 0.78, 0.4);
        }
        PoiIn | PoiOut => {
            pen.rect(0.12, 0.3, 0.88, 0.7);
            pen.painter.text(pen.p(0.5, 0.5), egui::Align2::CENTER_CENTER, "POI", egui::FontId::proportional(pen.rect.height() * 0.3), pen.color);
            pen.line(if icon == PoiIn { 0.42 } else { 0.58 }, 0.7, 0.5, 0.84);
        }
        PlayFilm => {
            pen.rect(0.08, 0.26, 0.58, 0.74);
            pen.fill(&[(0.46, 0.22), (0.92, 0.5), (0.46, 0.78)]);
        }
        Prev => pen.poly(&[(0.6, 0.3), (0.4, 0.5), (0.6, 0.7)], false),
        Next => pen.poly(&[(0.4, 0.3), (0.6, 0.5), (0.4, 0.7)], false),
        Dot => pen.dot(0.5, 0.5, 0.16),
        NextEdit => {
            pen.fill(&[(0.24, 0.25), (0.66, 0.5), (0.24, 0.75)]);
            pen.rect_fill(0.68, 0.25, 0.76, 0.75);
        }
        PrevEdit => {
            pen.rect_fill(0.24, 0.25, 0.32, 0.75);
            pen.fill(&[(0.76, 0.25), (0.34, 0.5), (0.76, 0.75)]);
        }
        Hamburger => {
            for y in [0.32, 0.5, 0.68] {
                pen.line(0.2, y, 0.8, y);
            }
        }
        TrimTools => {
            for (y, w) in [(0.3, 0.8), (0.46, 0.62), (0.62, 0.7)] {
                pen.line(0.14, y, w, y);
            }
            pen.poly(&[(0.6, 0.76), (0.68, 0.84), (0.86, 0.66)], false);
        }
        TrimRipple => {
            for (y, x0, x1) in [(0.34, 0.12, 0.88), (0.5, 0.24, 0.76), (0.66, 0.12, 0.88)] {
                pen.line(x0, y, x1, y);
            }
            pen.line(0.5, 0.16, 0.5, 0.84);
        }
        TrimAudio => {
            pen.rect(0.18, 0.3, 0.82, 0.76);
            pen.line(0.5, 0.2, 0.5, 0.86);
        }
        ZoomFrame => {
            pen.rect(0.18, 0.28, 0.82, 0.72);
            pen.line(0.08, 0.2, 0.08, 0.8);
            pen.line(0.92, 0.2, 0.92, 0.8);
            pen.poly(&[(0.36, 0.42), (0.28, 0.5), (0.36, 0.58)], false);
            pen.poly(&[(0.64, 0.42), (0.72, 0.5), (0.64, 0.58)], false);
        }
        ZoomDetailDim => {
            pen.poly(&[(0.22, 0.3), (0.12, 0.3), (0.12, 0.7), (0.22, 0.7)], false);
            pen.poly(&[(0.78, 0.3), (0.88, 0.3), (0.88, 0.7), (0.78, 0.7)], false);
            pen.line(0.5, 0.2, 0.5, 0.8);
            pen.poly(&[(0.38, 0.42), (0.3, 0.5), (0.38, 0.58)], false);
            pen.poly(&[(0.62, 0.42), (0.7, 0.5), (0.62, 0.58)], false);
        }
        BlueMarker => pen.fill(&[(0.3, 0.16), (0.7, 0.16), (0.7, 0.66), (0.5, 0.84), (0.3, 0.66)]),
        Minimize => pen.line(0.25, 0.5, 0.75, 0.5),
        Maximize => pen.rect(0.25, 0.25, 0.75, 0.75),
        Close => {
            pen.line(0.25, 0.25, 0.75, 0.75);
            pen.line(0.75, 0.25, 0.25, 0.75);
        }
    }
}

/// An icon button: returns the response, drawn brighter on hover and when `active`.
pub fn button(ui: &mut egui::Ui, rect: Rect, icon: Icon, active: bool, enabled: bool, tip: &str) -> egui::Response {
    let id = ui.scope_id().with(("icon", tip, rect.min.x as i32, rect.min.y as i32));
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let resp = ui.interact(rect, id, sense).on_hover_text(tip);
    let color = if !enabled {
        crate::theme::ICON_DIM
    } else if active {
        crate::theme::ICON_ACTIVE
    } else if resp.hovered() {
        crate::theme::ICON_HOVER
    } else {
        crate::theme::ICON
    };
    paint(ui.painter(), rect, icon, color);
    resp
}
