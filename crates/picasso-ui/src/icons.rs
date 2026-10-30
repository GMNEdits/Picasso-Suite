//! The icons, drawn with a few lines and shapes each (no image files, so they stay sharp at any
//! scale). Every icon is designed in a box of about 28 x 28 points around its centre.

use std::f32::consts::TAU;

use egui::{pos2, vec2, Align2, Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};

use crate::theme::semibold;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(target_arch = "wasm32", allow(dead_code))] // the window buttons exist on the desktop only
pub(crate) enum Icon {
    // title bar
    Minimize,
    Restore,
    Close,
    // first toolbar
    MediaImport,
    SyncBin,
    Mail,
    TitleT,
    Wand,
    Diamonds,
    Export,
    Fullscreen,
    Mixer,
    Tools,
    // second toolbar
    Sidebar,
    NewBin,
    Sync,
    Unlink,
    ImportImage,
    Grid,
    Search,
    Sort,
    Refresh,
    ThumbView,
    FilmView,
    ListView,
    Grid2,
    Clip5m,
    Antenna,
    Clock,
    FitView,
    Hq,
    Overlay,
    ColorWheel,
    Speaker,
    Chevron,
    // transport row
    InsertClip,
    SmartInsert,
    Overwrite,
    Replace,
    PlaceOnTop,
    RippleOverwrite,
    SourceOverlay,
    Sliders,
    Mic,
    PoiStart,
    PoiEnd,
    FilmPlay,
    ChevLeft,
    ChevRight,
    Dot,
    ToStart,
    PlayReverse,
    Stop,
    Play,
    ToEnd,
    Loop,
    NextEdit,
    PrevEdit,
    Menu,
    // timeline rails
    RailChecks,
    RailPin,
    RailFilm,
    Fit,
    TrimTool,
    Scissors,
    Marker,
    // pages
    PageMedia,
    PageCapture,
    PageCut,
    PageEdit,
    PageEffects,
    PageColor,
    PageAudio,
    PageDeliver,
    Home,
    Settings,
    // the Capture page
    PageCaptureOn,
    PageAudioOn,
    PageMediaOn,
    PageCutOn,
    PageEditOn,
    PageEffectsOn,
    PageColorOn,
    PageDeliverOn,
    ListBullets,
    SubtitleBox,
    MusicLines,
    Bars,
    SkipStartSmall,
    SkipEndSmall,
    FaderGear,
    FaderGear2,
    LinkChain,
    Grid3,
    PhotoMusic,
    PhotoStack,
    PhotoStackSmall,
    Photo,
    PhotoHeart,
    PhotoX,
    Heart,
    Star,
    Bookmark,
    Circles3,
    Tag,
    LabelTag,
    DenseGrid,
    SingleView,
    WindowLayout,
    // the Effects page
    ChevronBox,
    ClipFrame,
    NodeTree,
    Spline,
    Diamonds3,
    MonitorUp,
    ViewerA,
    ViewerInset,
    ViewerFrame,
    PanelStack,
    FxBackground,
    FxNoise,
    FxText,
    FxPaint,
    FxParticles,
    FxCurves,
    FxBrightness,
    FxDrop,
    FxMerge,
    FxMergeSplit,
    FxDissolve,
    FxMatte,
    FxTransform,
    FxRectMask,
    FxEllipseMask,
    FxPolyMask,
    FxBSpline,
    FxWave,
    FxEmitter,
    FxRender,
    // the Color page
    Gallery,
    LutGrid,
    FxBox,
    Lightbox,
    Download,
    Expand,
    ViewerSplit,
    ColorWipe,
    Pointer,
    Layers,
    Picker,
    CamRaw,
    ColorMatch,
    WheelDot,
    WheelHdr,
    RgbMixer,
    MotionFx,
    Qualifier,
    Hexagon,
    Needle,
    CircleWindow,
    Tracker,
    MagicMask,
    BlurBox,
    Sizing,
    Box3D,
    ScopeGraph,
    BarsVert,
    WheelLog,
    Reset,
    PlusDot,
    AutoA,
    ThumbGrid,
    // the Edit page
    BinFolder,
    SmartBin,
    Cloud,
    SnapHook,
    PositionLock,
    ZoomRuler,
    StripView,
    SparkleFrame,
    Info,
    Drive,
    FolderPlus,
}

/// Draws strokes and fills relative to the centre of an icon.
struct Pen<'a> {
    p: &'a Painter,
    c: Pos2,
    col: Color32,
    w: f32,
    /// Scales the whole drawing.
    k: f32,
}

impl Pen<'_> {
    fn pt(&self, x: f32, y: f32) -> Pos2 {
        pos2(self.c.x + x * self.k, self.c.y + y * self.k)
    }
    fn stroke(&self) -> Stroke {
        Stroke::new(self.w * self.k, self.col)
    }
    fn line(&self, x1: f32, y1: f32, x2: f32, y2: f32) {
        self.p
            .line_segment([self.pt(x1, y1), self.pt(x2, y2)], self.stroke());
    }
    fn path(&self, pts: &[(f32, f32)]) {
        self.p.add(Shape::line(
            pts.iter().map(|(x, y)| self.pt(*x, *y)).collect(),
            self.stroke(),
        ));
    }
    fn closed(&self, pts: &[(f32, f32)]) {
        self.p.add(Shape::closed_line(
            pts.iter().map(|(x, y)| self.pt(*x, *y)).collect(),
            self.stroke(),
        ));
    }
    fn rect(&self, x: f32, y: f32, w: f32, h: f32, r: f32) {
        self.p.rect_stroke(
            Rect::from_min_size(self.pt(x, y), vec2(w * self.k, h * self.k)),
            r * self.k,
            self.stroke(),
            StrokeKind::Inside,
        );
    }
    fn fill(&self, x: f32, y: f32, w: f32, h: f32, r: f32) {
        self.p.rect_filled(
            Rect::from_min_size(self.pt(x, y), vec2(w * self.k, h * self.k)),
            r * self.k,
            self.col,
        );
    }
    fn circle(&self, x: f32, y: f32, r: f32) {
        self.p
            .circle_stroke(self.pt(x, y), r * self.k, self.stroke());
    }
    fn dot(&self, x: f32, y: f32, r: f32) {
        self.p.circle_filled(self.pt(x, y), r * self.k, self.col);
    }
    fn tri(&self, pts: [(f32, f32); 3]) {
        self.p.add(Shape::convex_polygon(
            pts.iter().map(|(x, y)| self.pt(*x, *y)).collect(),
            self.col,
            Stroke::NONE,
        ));
    }
    /// An arc of the circle at (x, y), from angle `a0` to `a1` (radians, 0 = right, clockwise).
    fn arc(&self, x: f32, y: f32, r: f32, a0: f32, a1: f32) {
        let n = 16;
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let a = a0 + (a1 - a0) * i as f32 / n as f32;
                self.pt(x + r * a.cos(), y + r * a.sin())
            })
            .collect();
        self.p.add(Shape::line(pts, self.stroke()));
    }
    fn text(&self, x: f32, y: f32, s: &str, size: f32) {
        crate::i18n::paint_text(&self.p,
            self.pt(x, y),
            Align2::CENTER_CENTER,
            s,
            semibold(size * self.k),
            self.col,
        );
    }
    fn chevron_down(&self, x: f32, y: f32, s: f32) {
        self.path(&[(x - s, y - s * 0.5), (x, y + s * 0.5), (x + s, y - s * 0.5)]);
    }
    fn sparkle(&self, x: f32, y: f32, s: f32) {
        self.line(x - s, y, x + s, y);
        self.line(x, y - s, x, y + s);
    }
    fn dim(&self, a: u8) -> Color32 {
        Color32::from_rgba_unmultiplied(self.col.r(), self.col.g(), self.col.b(), a)
    }
    fn tint(&self, x: f32, y: f32, w: f32, h: f32, r: f32, a: u8) {
        self.p.rect_filled(
            Rect::from_min_size(self.pt(x, y), vec2(w * self.k, h * self.k)),
            r * self.k,
            self.dim(a),
        );
    }
    /// A filled four-pointed star.
    fn star4(&self, x: f32, y: f32, r: f32) {
        for (rx, ry) in [(0.34 * r, r), (r, 0.34 * r)] {
            self.p.add(Shape::convex_polygon(
                vec![
                    self.pt(x, y - ry),
                    self.pt(x + rx, y),
                    self.pt(x, y + ry),
                    self.pt(x - rx, y),
                ],
                self.col,
                Stroke::NONE,
            ));
        }
    }
    /// A filled ellipse, turned by `turn` radians.
    fn ellipse(&self, x: f32, y: f32, rx: f32, ry: f32, turn: f32) {
        let (sin, cos) = turn.sin_cos();
        let pts = (0..20)
            .map(|i| {
                let a = i as f32 / 20.0 * TAU;
                let (ex, ey) = (rx * a.cos(), ry * a.sin());
                self.pt(x + ex * cos - ey * sin, y + ex * sin + ey * cos)
            })
            .collect();
        self.p
            .add(Shape::convex_polygon(pts, self.col, Stroke::NONE));
    }
    /// A picture in a rounded frame: a sun and two peaks. `w`, `h` are the frame size.
    fn picture(&self, x: f32, y: f32, w: f32, h: f32) {
        self.rect(x, y, w, h, 2.0);
        self.dot(x + w * 0.28, y + h * 0.32, w * 0.07);
        self.path(&[
            (x + w * 0.08, y + h * 0.88),
            (x + w * 0.38, y + h * 0.5),
            (x + w * 0.58, y + h * 0.72),
            (x + w * 0.72, y + h * 0.56),
            (x + w * 0.94, y + h * 0.88),
        ]);
    }
    fn film(&self, x: f32, y: f32, w: f32, h: f32) {
        self.rect(x, y, w, h, 2.0);
        let mut yy = y + 3.0;
        while yy < y + h - 2.0 {
            self.fill(x + 1.5, yy, 2.0, 2.0, 0.0);
            self.fill(x + w - 3.5, yy, 2.0, 2.0, 0.0);
            yy += 4.0;
        }
    }
}

/// How much bigger than its design box an icon is drawn.
fn scale_of(icon: Icon) -> f32 {
    match icon {
        Icon::Minimize | Icon::Restore | Icon::Close | Icon::Chevron => 1.0,
        Icon::ToStart
        | Icon::PlayReverse
        | Icon::Stop
        | Icon::Play
        | Icon::ToEnd
        | Icon::NextEdit
        | Icon::PrevEdit
        | Icon::Loop
        | Icon::Dot => 1.12,
        Icon::ChevLeft | Icon::ChevRight | Icon::Menu => 1.1,
        Icon::PageMedia
        | Icon::PageCapture
        | Icon::PageCut
        | Icon::PageEdit
        | Icon::PageEffects
        | Icon::PageColor
        | Icon::PageAudio
        | Icon::PageDeliver
        | Icon::PageMediaOn
        | Icon::PageCutOn
        | Icon::PageEditOn
        | Icon::PageEffectsOn
        | Icon::PageColorOn
        | Icon::PageDeliverOn => 1.0,
        Icon::Home | Icon::Settings => 1.2,
        Icon::PageCaptureOn
        | Icon::PageAudioOn
        | Icon::ListBullets
        | Icon::SubtitleBox
        | Icon::MusicLines
        | Icon::Bars
        | Icon::SkipStartSmall
        | Icon::SkipEndSmall
        | Icon::FaderGear
        | Icon::FaderGear2
        | Icon::LinkChain
        | Icon::Grid3
        | Icon::PhotoMusic
        | Icon::PhotoStack
        | Icon::PhotoStackSmall
        | Icon::Photo
        | Icon::PhotoHeart
        | Icon::PhotoX
        | Icon::Heart
        | Icon::Star
        | Icon::Bookmark
        | Icon::Circles3
        | Icon::Tag
        | Icon::LabelTag
        | Icon::DenseGrid
        | Icon::SingleView
        | Icon::WindowLayout
        | Icon::ChevronBox
        | Icon::ClipFrame
        | Icon::NodeTree
        | Icon::Spline
        | Icon::Diamonds3
        | Icon::MonitorUp
        | Icon::ViewerA
        | Icon::ViewerInset
        | Icon::ViewerFrame
        | Icon::PanelStack
        | Icon::FxBackground
        | Icon::FxNoise
        | Icon::FxText
        | Icon::FxPaint
        | Icon::FxParticles
        | Icon::FxCurves
        | Icon::FxBrightness
        | Icon::FxDrop
        | Icon::FxMerge
        | Icon::FxMergeSplit
        | Icon::FxDissolve
        | Icon::FxMatte
        | Icon::FxTransform
        | Icon::FxRectMask
        | Icon::FxEllipseMask
        | Icon::FxPolyMask
        | Icon::FxBSpline
        | Icon::FxWave
        | Icon::FxEmitter
        | Icon::FxRender
        | Icon::Gallery
        | Icon::LutGrid
        | Icon::FxBox
        | Icon::Lightbox
        | Icon::Download
        | Icon::Expand
        | Icon::ViewerSplit
        | Icon::ColorWipe
        | Icon::Pointer
        | Icon::Layers
        | Icon::Picker
        | Icon::CamRaw
        | Icon::ColorMatch
        | Icon::WheelDot
        | Icon::WheelHdr
        | Icon::RgbMixer
        | Icon::MotionFx
        | Icon::Qualifier
        | Icon::Hexagon
        | Icon::Needle
        | Icon::CircleWindow
        | Icon::Tracker
        | Icon::MagicMask
        | Icon::BlurBox
        | Icon::Sizing
        | Icon::Box3D
        | Icon::ScopeGraph
        | Icon::BarsVert
        | Icon::WheelLog
        | Icon::Reset
        | Icon::PlusDot
        | Icon::AutoA
        | Icon::ThumbGrid
        | Icon::BinFolder
        | Icon::SmartBin
        | Icon::Cloud
        | Icon::SnapHook
        | Icon::PositionLock
        | Icon::ZoomRuler
        | Icon::StripView
        | Icon::SparkleFrame
        | Icon::Info
        | Icon::Drive
        | Icon::FolderPlus => 1.0,
        _ => 1.22,
    }
}

/// Draws `icon` centred on `c`.
pub(crate) fn paint(p: &Painter, c: Pos2, icon: Icon, col: Color32) {
    paint_scaled(p, c, icon, col, 1.0);
}

/// Draws `icon` centred on `c`, `mult` times its usual size.
pub(crate) fn paint_scaled(p: &Painter, c: Pos2, icon: Icon, col: Color32, mult: f32) {
    let s = Pen {
        p,
        c,
        col,
        w: 1.55,
        k: scale_of(icon) * mult,
    };
    match icon {
        Icon::Minimize => {
            let s = Pen { w: 1.4, ..s };
            s.line(-6.0, 0.0, 6.0, 0.0);
        }
        Icon::Restore => {
            let s = Pen { w: 1.4, ..s };
            s.rect(-6.0, -3.0, 9.0, 9.0, 1.0);
            s.path(&[
                (-3.0, -3.0),
                (-3.0, -6.0),
                (6.0, -6.0),
                (6.0, 3.0),
                (3.0, 3.0),
            ]);
        }
        Icon::Close => {
            let s = Pen { w: 1.4, ..s };
            s.line(-6.0, -6.0, 6.0, 6.0);
            s.line(6.0, -6.0, -6.0, 6.0);
        }
        Icon::MediaImport => {
            let b = Pen {
                col: s.dim(120),
                w: 1.2,
                ..s
            };
            b.path(&[(-10.0, -12.0), (-4.0, -12.0), (-1.0, -10.0)]);
            b.path(&[(-13.0, -9.0), (-13.0, -5.0)]);
            s.rect(-12.0, -9.0, 19.0, 15.0, 3.0);
            s.path(&[
                (-12.0, 3.0),
                (-6.0, -3.0),
                (-1.0, 2.0),
                (2.0, -1.0),
                (7.0, 4.0),
            ]);
            s.dot(-6.0, -5.0, 1.5);
            s.dot(8.0, 8.0, 2.4);
            s.dot(12.0, 6.0, 2.4);
            s.line(10.0, 8.0, 10.0, -1.0);
            s.line(14.0, 6.0, 14.0, -3.0);
            s.line(10.0, -1.0, 14.0, -3.0);
        }
        Icon::SyncBin => {
            s.rect(-12.0, -11.0, 17.0, 14.0, 3.0);
            s.tint(-9.0, -8.0, 5.0, 4.0, 1.0, 120);
            s.tint(-2.0, -8.0, 5.0, 4.0, 1.0, 120);
            s.tint(-9.0, -2.0, 5.0, 3.0, 1.0, 120);
            s.rect(-1.0, 2.0, 14.0, 9.0, 2.0);
            s.path(&[(-9.0, 4.0), (-9.0, 9.0), (-4.0, 9.0)]);
            s.path(&[(-6.0, 6.5), (-3.5, 9.0), (-6.0, 11.5)]);
        }
        Icon::Mail => {
            s.rect(-13.0, -10.0, 26.0, 20.0, 3.0);
            for tri in [
                [(-13.0, -10.0), (13.0, -10.0), (0.0, 0.0)],
                [(-13.0, 10.0), (13.0, 10.0), (0.0, 0.0)],
            ] {
                s.p.add(Shape::convex_polygon(
                    tri.iter().map(|(x, y)| s.pt(*x, *y)).collect(),
                    s.dim(70),
                    Stroke::NONE,
                ));
            }
            s.line(-13.0, -10.0, 13.0, 10.0);
            s.line(13.0, -10.0, -13.0, 10.0);
        }
        Icon::TitleT => {
            s.rect(-13.0, -9.0, 26.0, 18.0, 5.0);
            s.line(-4.5, -3.5, 4.5, -3.5);
            s.line(0.0, -3.5, 0.0, 5.0);
        }
        Icon::Wand => {
            let t = Pen { w: 3.0, ..s };
            t.line(-10.0, 11.0, 2.0, -1.0);
            s.star4(5.0, -9.0, 4.2);
            s.star4(11.5, -1.0, 2.8);
            s.star4(-1.5, -9.5, 2.6);
            s.star4(5.5, 5.5, 2.4);
        }
        Icon::Diamonds => {
            let back = Pen {
                col: s.dim(90),
                ..s
            };
            back.path(&[(-4.0, -6.0), (-10.0, 0.0), (-4.0, 6.0)]);
            back.path(&[(-9.0, -6.0), (-15.0, 0.0), (-9.0, 6.0)]);
            back.path(&[(-4.0, -6.0), (2.0, -12.0)]);
            s.p.add(Shape::convex_polygon(
                vec![
                    s.pt(0.0, 0.0),
                    s.pt(6.0, -6.0),
                    s.pt(12.0, 0.0),
                    s.pt(6.0, 6.0),
                ],
                s.dim(150),
                Stroke::NONE,
            ));
            s.closed(&[(0.0, 0.0), (6.0, -6.0), (12.0, 0.0), (6.0, 6.0)]);
        }
        Icon::Export => {
            s.path(&[
                (-6.0, 0.0),
                (-10.0, 0.0),
                (-10.0, 12.0),
                (10.0, 12.0),
                (10.0, 0.0),
                (6.0, 0.0),
            ]);
            s.line(0.0, -12.0, 0.0, 5.0);
            s.path(&[(-5.0, -7.0), (0.0, -12.0), (5.0, -7.0)]);
            for x in [-8.0, -2.5, 3.0] {
                s.tint(x, 6.0, 5.0, 4.5, 1.0, 70);
            }
        }
        Icon::Fullscreen => {
            s.tint(-14.0, -10.0, 28.0, 20.0, 4.0, 40);
            s.rect(-14.0, -10.0, 28.0, 20.0, 4.0);
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                s.path(&[
                    (sx * 9.0, sy * 3.5),
                    (sx * 9.0, sy * 6.0),
                    (sx * 6.0, sy * 6.0),
                ]);
            }
        }
        Icon::Mixer => {
            for (x, y) in [(-8.0, 0.0), (0.0, -4.0), (8.0, 4.0)] {
                let t = Pen {
                    col: s.dim(150),
                    w: 2.4,
                    ..s
                };
                t.line(x, -11.0, x, 11.0);
                s.fill(x - 3.5, y - 4.0, 7.0, 8.0, 1.5);
                s.p.rect_filled(
                    Rect::from_center_size(s.pt(x, y), vec2(3.0 * s.k, 1.2 * s.k)),
                    0.0,
                    Color32::from_rgb(34, 33, 38),
                );
            }
        }
        Icon::Tools => {
            // A screwdriver and a brush crossed, with a wrench head at the top right.
            s.p.add(Shape::convex_polygon(
                vec![
                    s.pt(-12.0, -11.0),
                    s.pt(-8.0, -12.0),
                    s.pt(1.0, -1.0),
                    s.pt(-2.0, 1.0),
                ],
                col,
                Stroke::NONE,
            ));
            let t = Pen { w: 5.0, ..s };
            t.line(-10.0, 9.0, -3.0, 3.0);
            s.arc(7.0, -7.0, 5.0, 2.4, 7.2);
            s.p.add(Shape::convex_polygon(
                vec![
                    s.pt(2.0, 3.0),
                    s.pt(8.0, 2.0),
                    s.pt(12.0, 8.0),
                    s.pt(10.0, 11.0),
                    s.pt(4.0, 10.0),
                ],
                col,
                Stroke::NONE,
            ));
        }
        Icon::Sidebar => {
            s.rect(-12.0, -10.0, 24.0, 20.0, 2.0);
            s.p.rect_filled(
                Rect::from_min_size(s.pt(-12.0, -10.0), vec2(9.0 * s.k, 20.0 * s.k)),
                2.0,
                Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 110),
            );
        }
        Icon::NewBin => {
            s.closed(&[
                (-9.0, -11.0),
                (2.0, -11.0),
                (8.0, -5.0),
                (8.0, 11.0),
                (-9.0, 11.0),
            ]);
            s.line(2.0, -11.0, 2.0, -5.0);
            s.line(2.0, -5.0, 8.0, -5.0);
            s.line(-1.0, 1.0, -1.0, 8.0);
            s.path(&[(-4.0, 5.0), (-1.0, 8.0), (2.0, 5.0)]);
        }
        Icon::Sync => {
            s.arc(0.0, 0.0, 9.0, 3.4, 5.9);
            s.arc(0.0, 0.0, 9.0, 0.25, 2.75);
            s.tri([(6.0, -8.0), (11.0, -6.0), (7.0, -3.0)]);
            s.tri([(-6.0, 8.0), (-11.0, 6.0), (-7.0, 3.0)]);
        }
        Icon::Unlink => {
            let link = |cx: f32, cy: f32| {
                let (sin, cos) = (-0.785f32).sin_cos();
                let pts: Vec<(f32, f32)> = (0..24)
                    .map(|i| {
                        let a = i as f32 / 24.0 * TAU;
                        // A capsule: a long rounded shape.
                        let (ex, ey) = (
                            7.0 * a.cos().signum() * a.cos().abs().powf(0.5),
                            3.4 * a.sin(),
                        );
                        (cx + ex * cos - ey * sin, cy + ex * sin + ey * cos)
                    })
                    .collect();
                s.closed(&pts);
            };
            link(-3.5, 3.0);
            link(3.5, -3.0);
            let sp = Pen { w: 1.2, ..s };
            sp.line(-9.0, -9.0, -6.5, -6.5);
            sp.line(-4.0, -11.0, -3.0, -8.0);
            sp.line(-11.0, -3.5, -8.0, -3.0);
            sp.line(9.0, 9.0, 6.5, 6.5);
            sp.line(4.0, 11.0, 3.0, 8.0);
            sp.line(11.0, 3.5, 8.0, 3.0);
        }
        Icon::ImportImage => {
            s.path(&[(-12.0, 2.0), (-12.0, -9.0), (4.0, -9.0)]);
            s.path(&[(-12.0, 2.0), (-12.0, 4.0)]);
            s.line(-12.0, -9.0, -12.0, 2.0);
            s.path(&[(-11.0, 1.0), (-6.5, -3.5), (-3.0, 0.0), (0.0, -2.5)]);
            s.dot(-6.0, -6.0, 1.3);
            s.star4(7.5, -5.5, 3.6);
            s.star4(11.5, -0.5, 2.0);
            s.line(-12.0, 8.0, -2.0, 8.0);
            s.path(&[(-5.0, 5.0), (-2.0, 8.0), (-5.0, 11.0)]);
        }
        Icon::Grid => {
            for x in [-10.0, -3.5, 3.0] {
                for y in [-9.0, -2.5, 4.0] {
                    s.fill(x, y, 6.0, 6.0, 1.0);
                }
            }
        }
        Icon::Search => {
            let t = Pen { w: 2.0, ..s };
            t.circle(-2.5, -2.5, 7.5);
            t.line(3.0, 3.0, 10.0, 10.0);
        }
        Icon::Sort => {
            s.line(-11.0, -7.0, 3.0, -7.0);
            s.line(-11.0, -1.0, -1.0, -1.0);
            s.line(-11.0, 5.0, -5.0, 5.0);
            s.line(8.0, -9.0, 8.0, 8.0);
            s.path(&[(4.0, 4.0), (8.0, 8.5), (12.0, 4.0)]);
        }
        Icon::Refresh => {
            s.arc(0.0, 0.0, 8.5, 0.3, 4.3);
            s.arc(0.0, 0.0, 8.5, 3.4, 5.4);
            s.tri([(8.0, -8.0), (10.5, -2.5), (4.0, -3.5)]);
        }
        Icon::ThumbView => {
            s.rect(-11.0, -9.0, 22.0, 18.0, 2.0);
            s.path(&[
                (-11.0, 5.0),
                (-4.0, -1.0),
                (1.0, 4.0),
                (5.0, 0.0),
                (11.0, 6.0),
            ]);
            s.dot(-5.0, -4.0, 2.0);
        }
        Icon::FilmView => {
            s.path(&[(-12.0, -9.0), (-12.0, 9.0)]);
            s.path(&[(12.0, -9.0), (12.0, 9.0)]);
            s.rect(-8.0, -7.0, 16.0, 14.0, 1.0);
            s.rect(-4.0, -3.0, 8.0, 6.0, 1.0);
            for y in [-6.0, 0.0, 6.0] {
                s.line(-12.0, y, -9.0, y);
                s.line(12.0, y, 9.0, y);
            }
        }
        Icon::ListView => {
            s.rect(-12.0, -10.0, 24.0, 20.0, 2.0);
            s.fill(-12.0, -10.0, 24.0, 8.0, 2.0);
            s.line(-12.0, 3.0, 12.0, 3.0);
            s.line(0.0, 3.0, 0.0, 10.0);
        }
        Icon::Grid2 => {
            for x in [-11.0, 1.0] {
                for y in [-10.0, 1.0] {
                    s.rect(x, y, 10.0, 9.0, 2.0);
                }
            }
        }
        Icon::Clip5m => {
            s.rect(-12.0, -8.0, 24.0, 16.0, 2.0);
            s.text(0.0, 0.0, "5m", 11.0);
        }
        Icon::Antenna => {
            s.line(0.0, -2.0, -5.0, 10.0);
            s.line(0.0, -2.0, 5.0, 10.0);
            s.line(-3.0, 5.0, 3.0, 5.0);
            s.dot(0.0, -3.0, 2.2);
            s.arc(0.0, -3.0, 6.5, 3.6, 5.8);
            s.arc(0.0, -3.0, 10.0, 3.6, 5.8);
        }
        Icon::Clock => {
            s.circle(1.0, 1.0, 9.0);
            s.path(&[(1.0, -4.0), (1.0, 1.0), (5.0, 3.0)]);
            s.path(&[(-9.0, -4.0), (-7.0, -1.0), (-4.0, -4.0)]);
        }
        Icon::FitView => {
            s.rect(-14.0, -10.0, 28.0, 20.0, 4.0);
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                s.path(&[
                    (sx * 9.0, sy * 3.5),
                    (sx * 9.0, sy * 6.0),
                    (sx * 6.0, sy * 6.0),
                ]);
            }
        }
        Icon::Hq => {
            s.rect(-13.0, -10.0, 24.0, 17.0, 3.0);
            for y in [-6.5, -2.5, 1.5] {
                s.fill(-10.0, y, 2.0, 2.0, 0.0);
            }
            s.p.rect_filled(
                Rect::from_min_size(s.pt(-1.0, 2.0), vec2(15.0 * s.k, 10.0 * s.k)),
                2.0,
                col,
            );
            crate::i18n::paint_text(&s.p,
                s.pt(6.5, 7.3),
                Align2::CENTER_CENTER,
                "HQ",
                semibold(9.0 * s.k),
                Color32::from_rgb(34, 33, 38),
            );
        }
        Icon::Overlay => {
            s.tint(-9.0, -7.0, 18.0, 14.0, 1.0, 60);
            s.p.add(Shape::convex_polygon(
                vec![s.pt(-7.0, 5.0), s.pt(-1.0, -3.0), s.pt(7.0, 5.0)],
                s.dim(120),
                Stroke::NONE,
            ));
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                s.path(&[
                    (sx * 13.0, sy * 9.0),
                    (sx * 13.0, sy * 12.0),
                    (sx * 10.0, sy * 12.0),
                ]);
            }
        }
        Icon::ColorWheel => {
            let n = 36;
            for i in 0..n {
                let a0 = i as f32 / n as f32 * TAU * 0.92 + 0.5;
                let a1 = (i + 1) as f32 / n as f32 * TAU * 0.92 + 0.5;
                let hue =
                    Color32::from(egui::ecolor::Hsva::new(i as f32 / n as f32, 0.7, 0.95, 1.0));
                let t = Pen {
                    col: hue,
                    w: 2.4,
                    ..s
                };
                t.line(
                    8.5 * a0.cos(),
                    8.5 * a0.sin(),
                    8.5 * a1.cos(),
                    8.5 * a1.sin(),
                );
            }
            let y = Pen {
                col: Color32::from_rgb(244, 200, 70),
                ..s
            };
            y.sparkle(-6.0, 6.0, 4.0);
            y.sparkle(-2.0, 1.0, 2.0);
        }
        Icon::Speaker => {
            s.p.add(Shape::convex_polygon(
                vec![
                    s.pt(-12.0, -4.0),
                    s.pt(-6.0, -4.0),
                    s.pt(0.0, -10.0),
                    s.pt(0.0, 10.0),
                    s.pt(-6.0, 4.0),
                    s.pt(-12.0, 4.0),
                ],
                col,
                Stroke::NONE,
            ));
            s.arc(0.0, 0.0, 5.0, -0.9, 0.9);
            s.arc(0.0, 0.0, 9.0, -0.9, 0.9);
        }
        Icon::Chevron => s.chevron_down(0.0, 0.0, 4.5),
        Icon::InsertClip => {
            s.film(-13.0, -2.0, 17.0, 13.0);
            s.dot(6.0, 9.0, 2.6);
            s.dot(11.0, 7.0, 2.6);
            s.line(8.0, 9.0, 8.0, 2.0);
            s.line(13.0, 7.0, 13.0, 0.0);
            s.line(8.0, 2.0, 13.0, 0.0);
            s.path(&[(1.0, -13.0), (5.0, -13.0), (5.0, -9.0)]);
            s.path(&[(2.5, -10.5), (5.0, -8.0), (7.5, -10.5)]);
        }
        Icon::SmartInsert => {
            let d = Pen {
                col: s.dim(90),
                ..s
            };
            d.rect(-14.0, -3.0, 5.0, 10.0, 1.0);
            d.rect(9.0, -3.0, 5.0, 10.0, 1.0);
            s.rect(-6.0, -11.0, 12.0, 10.0, 1.5);
            s.path(&[(-3.0, 5.0), (0.0, 8.0), (3.0, 5.0)]);
        }
        Icon::Overwrite => {
            let d = Pen {
                col: s.dim(150),
                ..s
            };
            d.rect(-13.0, -2.0, 15.0, 11.0, 1.5);
            s.rect(-3.0, -11.0, 16.0, 11.0, 1.5);
            s.path(&[(3.0, 5.0), (6.0, 8.0), (9.0, 5.0)]);
        }
        Icon::Replace => {
            s.path(&[(-9.0, -7.0), (-13.0, -3.0), (-9.0, 1.0)]);
            s.path(&[(9.0, -7.0), (13.0, -3.0), (9.0, 1.0)]);
            s.rect(-6.0, -11.0, 12.0, 11.0, 1.5);
            let d = Pen {
                col: s.dim(110),
                ..s
            };
            d.path(&[(-6.0, 4.0), (-6.0, 9.0), (6.0, 9.0), (6.0, 4.0)]);
        }
        Icon::PlaceOnTop => {
            let d = Pen {
                col: s.dim(150),
                ..s
            };
            d.path(&[(-12.0, 2.0), (-12.0, -5.0), (0.0, -5.0)]);
            s.rect(-6.0, -1.0, 15.0, 13.0, 1.0);
            s.dot(1.5, 4.0, 2.0);
            s.p.add(Shape::convex_polygon(
                vec![
                    s.pt(-3.0, 10.0),
                    s.pt(-2.0, 7.0),
                    s.pt(5.0, 7.0),
                    s.pt(6.0, 10.0),
                ],
                col,
                Stroke::NONE,
            ));
            for (x, y) in [(-6.0, -1.0), (9.0, -1.0), (-6.0, 12.0), (9.0, 12.0)] {
                s.circle(x, y, 1.1);
            }
            s.path(&[(2.0, -13.0), (6.0, -13.0), (6.0, -9.0)]);
            s.path(&[(3.5, -10.5), (6.0, -8.0), (8.5, -10.5)]);
        }
        Icon::RippleOverwrite => {
            s.p.rect_filled(
                Rect::from_min_size(s.pt(-13.0, -8.0), vec2(26.0 * s.k, 16.0 * s.k)),
                1.0,
                Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 70),
            );
            s.line(0.0, -10.0, 0.0, 10.0);
            s.line(-13.0, -8.0, 13.0, -8.0);
            s.line(-13.0, 8.0, 13.0, 8.0);
        }
        Icon::SourceOverlay => {
            s.line(-13.0, -9.0, 13.0, -9.0);
            s.line(-13.0, 9.0, 13.0, 9.0);
            s.rect(-9.0, -9.0, 18.0, 18.0, 0.0);
            s.p.add(Shape::convex_polygon(
                vec![
                    s.pt(-9.0, 9.0),
                    s.pt(-9.0, -3.0),
                    s.pt(0.0, 3.0),
                    s.pt(9.0, -3.0),
                    s.pt(9.0, 9.0),
                ],
                col,
                Stroke::NONE,
            ));
        }
        Icon::Sliders => {
            for (y, x) in [(-7.0, 4.0), (0.0, -4.0), (7.0, 2.0)] {
                s.line(-12.0, y, x - 2.0, y);
                Pen {
                    col: s.dim(120),
                    ..s
                }
                .line(x + 2.5, y, 12.0, y);
                s.dot(x, y, 2.6);
            }
        }
        Icon::Mic => {
            s.rect(-3.5, -11.0, 7.0, 13.0, 3.5);
            s.arc(0.0, -1.0, 7.0, 0.2, 2.94);
            s.line(0.0, 6.0, 0.0, 10.0);
            s.line(-4.0, 10.0, 4.0, 10.0);
            s.line(9.0, -8.0, 9.0, -3.0);
            s.line(12.0, -10.0, 12.0, -1.0);
        }
        Icon::PoiStart | Icon::PoiEnd => {
            s.rect(-11.0, -8.0, 22.0, 14.0, 2.0);
            crate::i18n::paint_text(&s.p,
                s.pt(0.0, -1.0),
                Align2::CENTER_CENTER,
                "POI",
                semibold(9.5 * s.k),
                col,
            );
            s.path(&[(-4.0, 6.0), (0.0, 10.0), (4.0, 6.0)]);
            if icon == Icon::PoiEnd {
                s.line(8.0, 6.0, 8.0, 11.0);
                s.line(8.0, 11.0, 12.0, 11.0);
            } else {
                s.line(-8.0, -12.0, -8.0, -9.0);
                s.line(-4.0, -13.0, -4.0, -9.0);
            }
        }
        Icon::FilmPlay => {
            s.film(-14.0, -10.0, 20.0, 18.0);
            s.tri([(1.0, -8.0), (1.0, 12.0), (15.0, 2.0)]);
        }
        Icon::ChevLeft => {
            let t = Pen { w: 2.0, ..s };
            t.path(&[(3.0, -6.0), (-3.0, 0.0), (3.0, 6.0)]);
        }
        Icon::ChevRight => {
            let t = Pen { w: 2.0, ..s };
            t.path(&[(-3.0, -6.0), (3.0, 0.0), (-3.0, 6.0)]);
        }
        Icon::Dot => s.dot(0.0, 0.0, 6.0),
        Icon::ToStart => {
            s.fill(-14.0, -10.0, 2.6, 20.0, 0.0);
            s.tri([(-1.0, -9.0), (-1.0, 9.0), (-12.0, 0.0)]);
            s.tri([(11.0, -9.0), (11.0, 9.0), (0.0, 0.0)]);
        }
        Icon::PlayReverse => s.tri([(8.0, -10.0), (8.0, 10.0), (-9.0, 0.0)]),
        Icon::Stop => s.fill(-9.0, -9.0, 18.0, 18.0, 1.0),
        Icon::Play => s.tri([(-8.0, -10.0), (-8.0, 10.0), (9.0, 0.0)]),
        Icon::ToEnd => {
            s.fill(11.4, -10.0, 2.6, 20.0, 0.0);
            s.tri([(1.0, -9.0), (1.0, 9.0), (12.0, 0.0)]);
            s.tri([(-11.0, -9.0), (-11.0, 9.0), (0.0, 0.0)]);
        }
        Icon::Loop => {
            // A rounded loop running clockwise, its arrow pointing into the gap at the top.
            let (l, t, r, b, rad) = (-12.0f32, -8.0f32, 12.0f32, 8.0f32, 5.0f32);
            let corner = |cx: f32, cy: f32, from: f32| {
                (0..=6).map(move |i| {
                    let a = (from + 0.25 * i as f32 / 6.0) * TAU;
                    (cx + rad * a.cos(), cy + rad * a.sin())
                })
            };
            let mut pts = vec![(1.0, t)];
            pts.extend(corner(r - rad, t + rad, 0.75));
            pts.extend(corner(r - rad, b - rad, 0.0));
            pts.extend(corner(l + rad, b - rad, 0.25));
            pts.extend(corner(l + rad, t + rad, 0.5));
            pts.push((-3.0, t));
            let pen = Pen { w: 1.8, ..s };
            pen.path(&pts);
            pen.path(&[(-6.0, t - 3.5), (-2.5, t), (-6.0, t + 3.5)]);
        }
        Icon::NextEdit => {
            s.tri([(-9.0, -9.0), (-9.0, 9.0), (6.0, 0.0)]);
            s.fill(7.0, -9.0, 3.0, 18.0, 0.0);
        }
        Icon::PrevEdit => {
            s.tri([(9.0, -9.0), (9.0, 9.0), (-6.0, 0.0)]);
            s.fill(-10.0, -9.0, 3.0, 18.0, 0.0);
        }
        Icon::Menu => {
            for y in [-6.0, 0.0, 6.0] {
                s.line(-9.0, y, 9.0, y);
            }
        }
        Icon::RailChecks => {
            s.fill(-12.0, -8.0, 24.0, 3.0, 1.0);
            s.fill(-12.0, -1.0, 17.0, 3.0, 1.0);
            s.fill(-12.0, 6.0, 12.0, 3.0, 1.0);
            s.path(&[(3.0, 7.0), (6.0, 10.0), (12.0, 3.0)]);
        }
        Icon::RailPin => {
            s.fill(-13.0, -3.0, 22.0, 3.0, 1.0);
            s.fill(-13.0, 4.0, 22.0, 3.0, 1.0);
            s.fill(-13.0, -10.0, 8.0, 3.0, 1.0);
            s.line(0.0, -14.0, 0.0, 10.0);
            s.line(-3.0, -14.0, 3.0, -14.0);
            s.line(8.0, -12.0, 12.0, -10.0);
            s.line(10.0, -14.0, 11.0, -8.0);
        }
        Icon::RailFilm => {
            s.p.rect_filled(
                Rect::from_min_size(s.pt(-12.0, -7.0), vec2(24.0 * s.k, 17.0 * s.k)),
                2.0,
                Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 90),
            );
            s.line(0.0, -9.0, 0.0, 11.0);
            s.path(&[(4.0, -12.0), (7.0, -9.0), (10.0, -12.0)]);
        }
        Icon::Fit => {
            s.rect(-9.0, -8.0, 18.0, 16.0, 2.0);
            s.path(&[(-3.0, -3.0), (-6.0, 0.0), (-3.0, 3.0)]);
            s.path(&[(3.0, -3.0), (6.0, 0.0), (3.0, 3.0)]);
            s.path(&[(-13.0, -10.0), (-10.0, -10.0)]);
            s.line(-12.0, -9.0, -12.0, 9.0);
            s.line(12.0, -9.0, 12.0, 9.0);
            s.line(-13.0, 9.0, -10.0, 9.0);
            s.line(10.0, -10.0, 13.0, -10.0);
            s.line(10.0, 9.0, 13.0, 9.0);
        }
        Icon::TrimTool => {
            s.line(0.0, -11.0, 0.0, 11.0);
            s.line(-3.0, -11.0, 3.0, -11.0);
            s.path(&[(-4.0, -4.0), (-8.0, 0.0), (-4.0, 4.0)]);
            s.path(&[(4.0, -4.0), (8.0, 0.0), (4.0, 4.0)]);
            s.path(&[(-10.0, -8.0), (-12.0, -8.0), (-12.0, 8.0), (-10.0, 8.0)]);
            s.path(&[(10.0, -8.0), (12.0, -8.0), (12.0, 8.0), (10.0, 8.0)]);
        }
        Icon::Scissors => {
            s.line(-4.0, -11.0, 4.0, 3.0);
            s.line(4.0, -11.0, -4.0, 3.0);
            s.circle(-5.0, 7.0, 3.5);
            s.circle(5.0, 7.0, 3.5);
        }
        Icon::Marker => {
            s.p.add(Shape::convex_polygon(
                vec![
                    s.pt(-6.0, -10.0),
                    s.pt(6.0, -10.0),
                    s.pt(6.0, 4.0),
                    s.pt(0.0, 10.0),
                    s.pt(-6.0, 4.0),
                ],
                col,
                Stroke::NONE,
            ));
        }
        Icon::PageMedia | Icon::PageMediaOn => {
            // A picture in a frame with two sheets behind it; while the page is open the
            // frame turns orange, the hills green and the sun yellow.
            let on = icon == Icon::PageMediaOn;
            let orange = Color32::from_rgb(214, 104, 68);
            let d = Pen {
                col: if on { orange } else { s.dim(150) },
                w: 1.1,
                ..s
            };
            if on {
                let back = |r: u8, g: u8, b: u8| Pen {
                    col: Color32::from_rgb(r, g, b),
                    w: 1.3,
                    ..s
                };
                back(112, 56, 40).path(&[
                    (-8.6, -9.6),
                    (-8.6, -11.0),
                    (-7.2, -12.4),
                    (8.2, -12.4),
                    (9.6, -11.0),
                    (9.6, -9.6),
                ]);
                back(160, 78, 52).path(&[
                    (-10.6, -7.0),
                    (-10.6, -8.2),
                    (-9.2, -9.6),
                    (10.2, -9.6),
                    (11.6, -8.2),
                    (11.6, -7.0),
                ]);
            } else {
                d.line(-9.4, -12.4, 11.0, -12.4);
                d.line(-10.9, -9.5, 12.5, -9.5);
            }
            let f = Pen {
                col: if on { orange } else { col },
                w: 1.3,
                ..s
            };
            if on {
                s.p.rect_filled(
                    Rect::from_min_size(s.pt(-12.0, -6.2), vec2(25.0 * s.k, 15.8 * s.k)),
                    s.k,
                    Color32::from_rgb(12, 20, 26),
                );
                Pen { w: 1.5, ..f }.rect(-12.5, -7.0, 26.0, 17.2, 2.6);
                Pen {
                    col: Color32::from_rgb(232, 202, 84),
                    ..s
                }
                .dot(-6.8, -1.8, 2.1);
                let g = Pen {
                    col: Color32::from_rgb(46, 134, 66),
                    ..s
                };
                g.tri([(-8.4, 7.6), (-2.2, 0.6), (4.0, 7.6)]);
                g.tri([(-0.4, 7.6), (4.8, 1.8), (10.0, 7.6)]);
            } else {
                s.tint(-12.0, -6.2, 25.0, 15.8, 1.0, 40);
                f.rect(-12.5, -7.0, 26.0, 17.2, 1.2);
                s.dot(-5.6, -1.0, 2.0);
                f.path(&[(-6.9, 7.3), (-2.3, 0.8), (1.5, 4.5), (5.3, 0.8), (9.0, 7.3)]);
            }
        }
        Icon::PageCapture => {
            // A camera, open at the corner where the flash goes off.
            let c = Pen { w: 1.4, ..s };
            let pi = std::f32::consts::PI;
            c.line(-15.0, 5.2, -15.0, -4.9);
            c.arc(-10.0, -4.9, 5.0, pi, 1.5 * pi);
            c.line(-10.0, -9.9, 4.2, -9.9);
            c.line(12.4, 0.5, 12.4, 5.2);
            c.arc(7.4, 5.2, 5.0, 0.0, 0.5 * pi);
            c.line(7.4, 10.2, -10.0, 10.2);
            c.arc(-10.0, 5.2, 5.0, 0.5 * pi, pi);
            c.circle(-1.3, 0.1, 5.6);
            Pen {
                col: s.dim(70),
                ..s
            }
            .dot(9.4, -7.7, 2.6);
            let r = Pen {
                col: s.dim(190),
                w: 1.1,
                ..s
            };
            r.line(10.3, -15.6, 11.0, -13.0);
            r.line(13.6, -12.8, 15.5, -11.8);
            r.line(14.8, -9.0, 17.3, -8.3);
        }
        Icon::PageCut | Icon::PageCutOn => {
            // A film strip with speed lines and a slash through it; while the page is open
            // the strip is blue and the slash red.
            let on = icon == Icon::PageCutOn;
            let blue = if on {
                Color32::from_rgb(78, 148, 226)
            } else {
                col
            };
            let b = Pen {
                col: blue,
                w: 1.5,
                ..s
            };
            let film = [(-6.3, -11.4), (17.3, -10.6), (14.6, 5.8), (-11.5, 5.8)];
            s.p.add(Shape::convex_polygon(
                film.iter().map(|(x, y)| s.pt(*x, *y)).collect(),
                if on {
                    Color32::from_rgb(12, 24, 46)
                } else {
                    s.dim(30)
                },
                Stroke::NONE,
            ));
            b.closed(&film);
            let pale = Pen {
                col: if on {
                    Color32::from_rgb(196, 202, 212)
                } else {
                    col
                },
                w: 1.3,
                ..s
            };
            pale.line(-12.5, -8.0, -6.3, -8.0);
            pale.line(-15.7, -4.0, -5.6, -4.0);
            pale.line(-18.8, 0.8, -5.6, 0.8);
            for (x, y) in [(8.5, -7.8), (12.8, -7.8), (5.9, 1.8), (9.8, 1.8)] {
                Pen { col: blue, ..s }.dot(x, y, 1.0);
            }
            let red = Pen {
                col: if on {
                    Color32::from_rgb(226, 72, 52)
                } else {
                    col
                },
                w: 1.7,
                ..s
            };
            red.line(5.3, -14.9, -1.3, 9.5);
        }
        Icon::PageEdit | Icon::PageEditOn => {
            // Bars on both sides of a pin; while the page is open the video bars are blue,
            // the audio bar green and the pin red.
            let on = icon == Icon::PageEditOn;
            for (i, (x0, x1, y0)) in [
                (-16.3, -1.9, -8.7),
                (-9.8, -1.5, -3.0),
                (-16.3, -1.9, 3.3),
                (5.0, 14.4, -8.7),
                (2.5, 13.2, -3.0),
                (3.8, 16.3, 3.3),
            ]
            .into_iter()
            .enumerate()
            {
                let bar = match (on, i % 3) {
                    (false, _) => col,
                    (true, 2) => Color32::from_rgb(70, 156, 76),
                    (true, _) => Color32::from_rgb(78, 146, 222),
                };
                Pen { col: bar, ..s }.fill(x0, y0, x1 - x0, 4.4, 2.2);
            }
            let l = Pen {
                col: if on {
                    Color32::from_rgb(222, 64, 52)
                } else {
                    s.dim(215)
                },
                w: 1.4,
                ..s
            };
            if on {
                // A hollow marker head on a longer, thicker pin.
                let pin = Pen { w: 1.7, ..l };
                pin.line(0.4, -11.6, 0.4, 12.4);
                Pen { w: 1.3, ..l }.closed(&[
                    (-2.0, -15.6),
                    (2.8, -15.6),
                    (2.8, -13.4),
                    (0.4, -11.4),
                    (-2.0, -13.4),
                ]);
            } else {
                l.line(0.4, -12.0, 0.4, 11.4);
                l.rect(-1.5, -15.0, 3.8, 2.8, 0.6);
            }
        }
        Icon::PageEffects | Icon::PageEffectsOn => {
            // A wand with a trail of sparkles; while the page is open the sparkles are
            // yellow and the wand bright.
            let on = icon == Icon::PageEffectsOn;
            let wand = if on {
                Color32::from_rgb(196, 196, 200)
            } else {
                col
            };
            Pen {
                col: wand,
                w: 3.4,
                ..s
            }
            .line(-0.4, 8.6, 10.6, -8.8);
            Pen {
                col: if on {
                    Color32::from_rgb(70, 70, 74)
                } else {
                    Color32::from_rgb(34, 34, 36)
                },
                w: 1.4,
                ..s
            }
            .line(0.0, 7.6, 9.6, -7.6);
            Pen {
                col: if on {
                    Color32::from_rgba_unmultiplied(150, 140, 40, 90)
                } else {
                    s.dim(40)
                },
                w: 2.6,
                ..s
            }
            .arc(-3.0, -1.0, 9.5, 1.5, 4.5);
            let star = Pen {
                col: if on {
                    Color32::from_rgb(238, 222, 70)
                } else {
                    col
                },
                ..s
            };
            star.star4(-10.7, -9.5, 2.6);
            star.star4(2.5, -14.3, 3.2);
            star.star4(-3.5, -3.9, 3.0);
        }
        Icon::PageColor | Icon::PageColorOn => {
            // Nine dots round a middle one, a loading wheel; while the page is open the
            // dots run round the colour wheel.
            let on = icon == Icon::PageColorOn;
            for (x, y, a, hue) in [
                (-5.0, -11.4, 150, (198, 80, 168)),
                (2.8, -12.2, 70, (168, 100, 176)),
                (9.0, -7.8, 95, (106, 98, 222)),
                (10.0, 0.1, 125, (62, 140, 222)),
                (5.3, 6.4, 135, (44, 168, 80)),
                (-2.5, 7.3, 140, (230, 200, 52)),
                (-8.8, 3.0, 130, (230, 150, 60)),
                (-9.8, -4.6, 140, (222, 82, 80)),
            ] {
                let dot = if on {
                    Color32::from_rgb(hue.0, hue.1, hue.2)
                } else {
                    s.dim(a)
                };
                Pen { col: dot, ..s }.dot(x, y, 2.2);
            }
            Pen {
                col: if on {
                    Color32::from_rgb(150, 150, 152)
                } else {
                    s.dim(160)
                },
                ..s
            }
            .dot(0.0, -2.0, 2.5);
        }
        Icon::PageAudio => {
            // Two beamed notes.
            s.ellipse(-6.3, 5.5, 3.6, 2.8, -0.35);
            s.ellipse(5.6, 4.3, 3.6, 2.8, -0.35);
            s.fill(-2.7, -7.5, 1.5, 13.0, 0.0);
            s.fill(9.2, -10.8, 1.5, 13.2, 0.0);
            s.p.add(Shape::convex_polygon(
                vec![
                    s.pt(-2.7, -7.2),
                    s.pt(10.7, -11.6),
                    s.pt(10.7, -8.0),
                    s.pt(-2.7, -3.6),
                ],
                col,
                Stroke::NONE,
            ));
        }
        Icon::PageDeliver | Icon::PageDeliverOn => {
            // A rocket climbing to the right: grey, or blue with an orange flame while the
            // page is open.
            rocket(&s, icon == Icon::PageDeliverOn);
        }
        Icon::Home => {
            s.tri([(-13.0, 0.0), (0.0, -11.0), (13.0, 0.0)]);
            s.fill(-9.0, -1.0, 18.0, 11.0, 0.0);
            s.p.rect_filled(
                Rect::from_min_size(s.pt(-2.5, 4.0), vec2(5.0 * s.k, 6.0 * s.k)),
                0.0,
                Color32::from_rgb(23, 25, 26),
            );
        }
        Icon::Settings => {
            // A gear: eight teeth around a ring.
            let teeth = 8;
            let mut pts = Vec::new();
            for n in 0..teeth {
                let a = n as f32 / teeth as f32 * TAU;
                let half = TAU / teeth as f32 * 0.22;
                for (da, r) in [
                    (-1.6 * half, 8.2),
                    (-half, 11.2),
                    (half, 11.2),
                    (1.6 * half, 8.2),
                ] {
                    pts.push(s.pt(r * (a + da).cos(), r * (a + da).sin()));
                }
            }
            s.p.add(Shape::convex_polygon(pts, col, Stroke::NONE));
            s.p.circle_filled(s.pt(0.0, 0.0), 4.2 * s.k, Color32::from_rgb(23, 25, 26));
        }

        Icon::PhotoMusic => {
            let b = Pen { w: 1.7, ..s };
            let d = Pen {
                col: s.dim(150),
                w: 1.5,
                ..s
            };
            d.path(&[(-13.0, -7.0), (-13.0, -10.5), (-10.5, -12.5), (-1.0, -12.5)]);
            d.path(&[(-10.5, -8.5), (-10.5, -11.5)]);
            s.tint(-11.0, -9.0, 21.0, 16.0, 2.5, 50);
            b.picture(-11.0, -9.0, 21.0, 16.0);
            s.ellipse(8.4, 10.0, 2.6, 2.1, -0.4);
            s.ellipse(14.0, 8.8, 2.6, 2.1, -0.4);
            s.fill(10.5, 1.4, 1.4, 9.0, 0.0);
            s.fill(16.0, 0.2, 1.4, 9.0, 0.0);
            s.p.add(Shape::convex_polygon(
                vec![
                    s.pt(10.5, 1.4),
                    s.pt(17.4, -0.6),
                    s.pt(17.4, 1.8),
                    s.pt(10.5, 3.8),
                ],
                col,
                Stroke::NONE,
            ));
        }
        Icon::PhotoStack => {
            let d = Pen {
                col: s.dim(130),
                w: 1.5,
                ..s
            };
            d.path(&[(-6.0, -12.0), (8.0, -12.0), (11.5, -9.0), (11.5, 2.0)]);
            d.path(&[(-9.5, -9.0), (5.0, -9.0), (8.5, -6.0), (8.5, 2.0)]);
            s.tint(-13.0, -6.5, 24.0, 17.0, 2.5, 50);
            Pen { w: 1.7, ..s }.picture(-13.0, -6.5, 24.0, 17.0);
        }
        Icon::PhotoStackSmall => {
            let d = Pen {
                col: s.dim(130),
                w: 1.1,
                ..s
            };
            d.path(&[(-8.0, -9.0), (8.0, -9.0), (9.0, -8.0)]);
            d.path(&[(-9.5, -6.5), (7.0, -6.5)]);
            s.picture(-11.0, -5.0, 19.0, 14.0);
        }
        Icon::Photo => {
            s.tint(-11.0, -8.0, 22.0, 16.0, 2.0, 40);
            s.picture(-11.0, -8.0, 22.0, 16.0);
        }
        Icon::PhotoHeart => {
            s.picture(-13.0, -9.0, 21.0, 16.0);
            let h = Pen { w: 1.4, ..s };
            let heart: Vec<(f32, f32)> = (0..24)
                .map(|i| {
                    let t = i as f32 / 24.0 * TAU;
                    (
                        9.0 + 5.2 * t.sin().powi(3) / 1.0 * 0.62,
                        5.5 - 0.31
                            * (13.0 * t.cos()
                                - 5.0 * (2.0 * t).cos()
                                - 2.0 * (3.0 * t).cos()
                                - (4.0 * t).cos())
                            * 0.62,
                    )
                })
                .collect();
            Pen {
                col: Color32::from_rgb(34, 33, 38),
                ..s
            }
            .p
            .add(Shape::convex_polygon(
                heart.iter().map(|(x, y)| s.pt(*x, *y)).collect(),
                Color32::from_rgb(34, 33, 38),
                Stroke::NONE,
            ));
            h.closed(&heart);
        }
        Icon::PhotoX => {
            s.picture(-13.0, -9.0, 21.0, 16.0);
            let x = Pen { w: 1.7, ..s };
            x.line(6.0, 3.0, 12.0, 9.0);
            x.line(12.0, 3.0, 6.0, 9.0);
        }
        Icon::Heart => {
            let heart: Vec<(f32, f32)> = (0..40)
                .map(|i| {
                    let t = i as f32 / 40.0 * TAU;
                    (
                        0.56 * 16.0 * t.sin().powi(3),
                        -0.56
                            * (13.0 * t.cos()
                                - 5.0 * (2.0 * t).cos()
                                - 2.0 * (3.0 * t).cos()
                                - (4.0 * t).cos())
                            + 0.6,
                    )
                })
                .collect();
            s.closed(&heart);
        }
        Icon::Star => {
            let pts: Vec<(f32, f32)> = (0..10)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + i as f32 * TAU / 10.0;
                    let r = if i % 2 == 0 { 9.0 } else { 3.9 };
                    (r * a.cos(), r * a.sin() + 0.8)
                })
                .collect();
            s.closed(&pts);
        }
        Icon::Bookmark => {
            s.closed(&[
                (-6.0, -9.0),
                (6.0, -9.0),
                (6.0, 9.0),
                (0.0, 4.5),
                (-6.0, 9.0),
            ]);
        }
        Icon::Circles3 => {
            s.circle(0.0, -4.5, 4.6);
            s.circle(-4.2, 3.2, 4.6);
            s.circle(4.2, 3.2, 4.6);
        }
        Icon::Tag => {
            s.closed(&[
                (-2.0, -9.0),
                (7.0, -9.0),
                (9.0, -6.5),
                (3.0, 4.0),
                (-4.0, 9.0),
                (-9.0, 4.5),
                (-8.5, -2.0),
            ]);
            s.circle(-2.5, 3.0, 2.0);
        }
        Icon::LabelTag => {
            let t = Pen { w: 2.0, ..s };
            t.closed(&[(-12.0, -1.0), (3.0, -6.0), (8.0, 7.0), (-8.0, 11.0)]);
            Pen { w: 1.8, ..s }.line(-8.0, 2.5, 0.0, 0.0);
            Pen { w: 1.8, ..s }.line(-7.0, 6.5, -1.5, 5.0);
            t.circle(8.5, -6.0, 3.6);
            Pen {
                col: s.dim(110),
                w: 1.3,
                ..s
            }
            .line(-9.0, 13.0, 5.0, 10.0);
        }
        Icon::DenseGrid => {
            for ix in 0..6 {
                for iy in 0..3 {
                    s.fill(
                        -9.5 + ix as f32 * 3.7,
                        -5.0 + iy as f32 * 3.7,
                        2.5,
                        2.5,
                        0.3,
                    );
                }
            }
        }
        Icon::SingleView => {
            s.tint(-11.0, -6.5, 22.0, 13.0, 1.5, 90);
            Pen { w: 1.9, ..s }.rect(-12.0, -7.5, 24.0, 15.0, 2.0);
        }
        Icon::WindowLayout => {
            s.rect(-13.0, -8.0, 26.0, 16.0, 1.2);
            s.rect(-4.0, -1.0, 14.0, 8.0, 0.8);
            s.line(-13.0, -4.5, -6.0, -4.5);
        }
        Icon::PageCaptureOn => {
            let pi = std::f32::consts::PI;
            // The body runs through the colours of the rainbow, from orange at the top round
            // to violet at the right; the lens stays white.
            let c = |r: u8, g: u8, b: u8| Pen {
                col: Color32::from_rgb(r, g, b),
                w: 1.5,
                ..s
            };
            c(226, 140, 70).line(-10.0, -9.9, -5.3, -9.9);
            c(222, 118, 108).line(-5.3, -9.9, -0.6, -9.9);
            c(212, 96, 158).line(-0.6, -9.9, 4.2, -9.9);
            c(228, 160, 70).arc(-10.0, -4.9, 5.0, pi, 1.5 * pi);
            c(224, 196, 72).line(-15.0, 5.2, -15.0, -4.9);
            c(150, 196, 72).arc(-10.0, 5.2, 5.0, 0.5 * pi, pi);
            c(72, 184, 98).line(-10.0, 10.2, -4.0, 10.2);
            c(56, 160, 170).line(-4.0, 10.2, 2.0, 10.2);
            c(72, 110, 222).line(2.0, 10.2, 7.4, 10.2);
            c(104, 96, 228).arc(7.4, 5.2, 5.0, 0.0, 0.5 * pi);
            c(130, 88, 224).line(12.4, 0.5, 12.4, 5.2);
            Pen {
                col: Color32::from_rgb(200, 200, 204),
                w: 1.6,
                ..s
            }
            .circle(-1.3, 0.1, 5.2);
            Pen {
                col: Color32::from_rgb(70, 70, 72),
                ..s
            }
            .dot(9.4, -7.7, 2.6);
            let r = Pen {
                col: Color32::from_rgb(200, 200, 204),
                w: 1.2,
                ..s
            };
            r.line(10.3, -15.6, 11.0, -13.0);
            r.line(13.6, -12.8, 15.5, -11.8);
            r.line(14.8, -9.0, 17.3, -8.3);
        }
        Icon::Grid3 => {
            for x in [-7.8, 0.0, 7.8] {
                for y in [-7.8, 0.0, 7.8] {
                    s.fill(x - 2.7, y - 2.7, 5.4, 5.4, 0.5);
                }
            }
        }
        Icon::PageAudioOn => {
            // The Audio page's note shows in green while the page is open.
            // Green at the top fading to teal at the heads.
            let teal = Pen {
                col: Color32::from_rgb(70, 204, 172),
                ..s
            };
            teal.ellipse(-6.4, 5.5, 3.7, 2.9, -0.35);
            teal.ellipse(5.5, 4.3, 3.7, 2.9, -0.35);
            // Thick stems, shading from the beam's green down to the heads' teal.
            let steps = 8;
            for (x, top, bottom) in [(-3.4, -7.4, 5.0), (8.6, -10.9, 3.8)] {
                for i in 0..steps {
                    let f = i as f32 / steps as f32;
                    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * f) as u8;
                    let h = (bottom - top) / steps as f32;
                    Pen {
                        col: Color32::from_rgb(mix(112, 70), mix(210, 204), mix(108, 172)),
                        ..s
                    }
                    .fill(x, top + h * i as f32, 2.2, h + 0.2, 0.0);
                }
            }
            s.p.add(Shape::convex_polygon(
                vec![
                    s.pt(-3.4, -7.0),
                    s.pt(10.8, -11.8),
                    s.pt(10.8, -7.4),
                    s.pt(-3.4, -2.6),
                ],
                Color32::from_rgb(124, 214, 104),
                Stroke::NONE,
            ));
        }
        Icon::ListBullets => {
            for y in [-7.0, 0.0, 7.0] {
                s.dot(-9.0, y, 1.6);
                s.line(-4.0, y, 10.0, y);
            }
        }
        Icon::SubtitleBox => {
            s.rect(-11.0, -8.0, 22.0, 16.0, 4.0);
            s.line(-6.0, -2.0, 6.0, -2.0);
            s.line(-6.0, 3.0, 2.0, 3.0);
        }
        Icon::MusicLines => {
            s.line(-14.0, -7.0, -2.0, -7.0);
            s.line(-14.0, -1.0, -5.0, -1.0);
            s.line(-14.0, 5.0, -8.0, 5.0);
            s.ellipse(2.0, 7.0, 3.0, 2.4, -0.35);
            s.ellipse(9.0, 5.5, 3.0, 2.4, -0.35);
            s.fill(4.2, -6.0, 1.3, 12.5, 0.0);
            s.fill(11.2, -7.5, 1.3, 12.5, 0.0);
            s.p.add(Shape::convex_polygon(
                vec![
                    s.pt(4.2, -6.0),
                    s.pt(12.5, -8.2),
                    s.pt(12.5, -5.6),
                    s.pt(4.2, -3.4),
                ],
                col,
                Stroke::NONE,
            ));
        }
        Icon::Bars => {
            for (i, h) in [4.0, 10.0, 7.0, 13.0, 9.0, 12.0, 5.0]
                .into_iter()
                .enumerate()
            {
                let x = -12.0 + i as f32 * 4.0;
                s.fill(x, 12.0 - h, 2.6, h, 0.4);
            }
        }
        Icon::SkipStartSmall => {
            s.fill(-6.0, -5.0, 2.0, 10.0, 0.0);
            s.tri([(-3.0, -5.0), (-3.0, 5.0), (6.0, 0.0)]);
        }
        Icon::SkipEndSmall => {
            s.fill(4.0, -5.0, 2.0, 10.0, 0.0);
            s.tri([(3.0, -5.0), (3.0, 5.0), (-6.0, 0.0)]);
        }
        Icon::FaderGear => {
            s.rect(-12.0, 0.0, 14.0, 7.0, 3.5);
            s.dot(-8.5, 3.5, 2.0);
            s.arc(5.0, -5.0, 6.0, 3.6, 5.8);
            s.line(2.0, -10.0, 12.0, -10.0);
        }
        Icon::FaderGear2 => {
            s.circle(-1.0, 3.0, 4.2);
            for i in 0..8 {
                let a = i as f32 / 8.0 * TAU;
                s.line(
                    -1.0 + 5.2 * a.cos(),
                    3.0 + 5.2 * a.sin(),
                    -1.0 + 7.2 * a.cos(),
                    3.0 + 7.2 * a.sin(),
                );
            }
            s.arc(8.0, -6.0, 5.0, 3.6, 5.8);
        }
        Icon::LinkChain => {
            let link = |cx: f32, cy: f32| {
                let (sin, cos) = (-0.785f32).sin_cos();
                let pts: Vec<(f32, f32)> = (0..24)
                    .map(|i| {
                        let a = i as f32 / 24.0 * TAU;
                        let (ex, ey) = (
                            7.5 * a.cos().signum() * a.cos().abs().powf(0.5),
                            3.6 * a.sin(),
                        );
                        (cx + ex * cos - ey * sin, cy + ex * sin + ey * cos)
                    })
                    .collect();
                Pen { w: 2.0, ..s }.closed(&pts);
            };
            link(-3.8, 3.8);
            link(3.8, -3.8);
        }
        Icon::ChevronBox => {
            s.rect(-10.0, -10.0, 20.0, 20.0, 3.0);
            s.chevron_down(0.0, 0.0, 4.6);
        }
        Icon::ClipFrame => {
            // A picture between two brackets, as a clip in a timeline.
            s.path(&[(-17.0, -8.0), (-15.0, -6.0), (-15.0, 6.0), (-17.0, 8.0)]);
            s.path(&[(17.0, -8.0), (15.0, -6.0), (15.0, 6.0), (17.0, 8.0)]);
            s.tint(-11.0, -8.0, 22.0, 16.0, 3.0, 45);
            Pen { w: 1.6, ..s }.rect(-11.0, -8.0, 22.0, 16.0, 3.0);
            s.dot(-4.5, -2.5, 1.8);
            s.path(&[(-7.0, 4.5), (-2.0, 0.5), (1.5, 3.0), (4.0, 1.0), (7.5, 4.5)]);
        }
        Icon::NodeTree => {
            let n = Pen { w: 1.7, ..s };
            n.rect(-13.0, -2.0, 9.0, 6.0, 3.0);
            n.rect(2.0, -9.5, 9.0, 6.0, 3.0);
            n.rect(2.0, 4.5, 9.0, 6.0, 3.0);
            n.line(-4.0, 1.0, 2.0, -6.5);
            n.line(-4.0, 1.0, 2.0, 7.5);
            n.line(-17.0, 1.0, -13.0, 1.0);
            n.line(11.0, -6.5, 14.0, -6.5);
            n.line(11.0, 7.5, 14.0, 7.5);
        }
        Icon::Spline => {
            s.circle(-1.0, -3.0, 3.0);
            s.line(-14.0, 4.0, -3.8, -1.6);
            s.line(-3.0, -5.2, -7.0, -9.0);
            s.rect(-10.5, -12.5, 3.6, 3.6, 0.6);
            s.path(&[(0.5, -0.4), (3.0, 5.0), (2.5, 12.0)]);
            s.line(1.8, -1.6, 10.0, 3.0);
            s.rect(10.0, 2.0, 3.6, 3.6, 0.6);
        }
        Icon::Diamonds3 => {
            let diamond = |x: f32| [(x, -8.0), (x + 8.0, 0.0), (x, 8.0), (x - 8.0, 0.0)];
            Pen {
                col: s.dim(70),
                w: 1.4,
                ..s
            }
            .closed(&diamond(-7.0));
            Pen {
                col: s.dim(110),
                w: 1.4,
                ..s
            }
            .closed(&diamond(-1.5));
            s.p.add(Shape::convex_polygon(
                diamond(4.0).iter().map(|(x, y)| s.pt(*x, *y)).collect(),
                s.dim(120),
                Stroke::NONE,
            ));
            Pen { w: 1.4, ..s }.closed(&diamond(4.0));
        }
        Icon::MonitorUp => {
            s.rect(-12.0, -12.0, 24.0, 13.0, 2.0);
            s.path(&[(-5.5, 11.0), (0.0, 5.5), (5.5, 11.0)]);
        }
        Icon::ViewerA => {
            s.rect(-10.0, -7.0, 20.0, 14.0, 0.8);
            s.text(0.0, 0.3, "A", 12.0);
        }
        Icon::ViewerInset => {
            s.rect(-10.0, -7.0, 20.0, 14.0, 0.8);
            s.fill(-1.0, -5.0, 9.0, 7.0, 0.4);
        }
        Icon::ViewerFrame => {
            s.rect(-12.0, -7.0, 24.0, 14.0, 0.8);
        }
        Icon::PanelStack => {
            Pen {
                col: s.dim(110),
                ..s
            }
            .path(&[
                (-7.0, -7.0),
                (-7.0, -10.0),
                (12.0, -10.0),
                (12.0, 5.0),
                (9.0, 5.0),
            ]);
            s.rect(-11.0, -7.0, 20.0, 15.0, 1.5);
            s.line(-8.0, -2.5, 6.0, -2.5);
            s.line(-8.0, 3.0, 6.0, 3.0);
            s.dot(-3.0, -2.5, 1.5);
            s.dot(2.0, 3.0, 1.5);
        }
        Icon::FxBackground => {
            // A frame filled with a gradient, drawn as stripes in its lower corner.
            s.p.add(Shape::convex_polygon(
                vec![s.pt(-11.0, 7.0), s.pt(11.0, 7.0), s.pt(11.0, -7.0)],
                s.dim(70),
                Stroke::NONE,
            ));
            let st = Pen { w: 1.3, ..s };
            for i in 0..5 {
                let x = -6.0 + i as f32 * 4.2;
                st.line(x, 7.0, (x + 6.0).min(11.0), 7.0 - (x + 6.0).min(11.0) + x);
            }
            Pen { w: 1.6, ..s }.rect(-12.0, -8.5, 24.0, 17.0, 1.0);
        }
        Icon::FxNoise => {
            for ix in 0..4 {
                for iy in 0..3 {
                    let a = if (ix + iy) % 2 == 0 { 80 } else { 35 };
                    Pen { col: s.dim(a), ..s }.fill(
                        -12.0 + ix as f32 * 6.0,
                        -9.0 + iy as f32 * 6.0,
                        6.0,
                        6.0,
                        1.0,
                    );
                }
            }
        }
        Icon::FxText => {
            s.tint(-12.0, -11.0, 24.0, 22.0, 2.0, 60);
            s.text(0.0, 0.5, "T", 18.0);
        }
        Icon::FxPaint => {
            Pen { w: 2.4, ..s }.line(11.0, -11.0, 0.5, 2.0);
            s.ellipse(-2.4, 4.6, 4.2, 2.4, -0.85);
            Pen {
                col: s.dim(110),
                w: 1.6,
                ..s
            }
            .arc(-6.0, 1.0, 7.0, 1.7, 3.3);
        }
        Icon::FxParticles => {
            for (i, a) in [150u8, 70, 95, 125, 135, 140, 130, 140]
                .into_iter()
                .enumerate()
            {
                let t = -2.0 + i as f32 / 8.0 * TAU;
                Pen { col: s.dim(a), ..s }.dot(9.0 * t.cos(), 9.0 * t.sin(), 1.9);
            }
        }
        Icon::FxCurves => {
            let g = Pen {
                col: s.dim(70),
                w: 1.0,
                ..s
            };
            for i in 0..4 {
                let x = -12.0 + i as f32 * 8.0;
                g.line(x, -10.0, x, 10.0);
            }
            for i in 0..3 {
                let y = -10.0 + i as f32 * 10.0;
                g.line(-12.0, y, 12.0, y);
            }
            g.line(-12.0, 10.0, 12.0, 10.0);
            let pts: Vec<(f32, f32)> = (0..=16)
                .map(|i| {
                    let t = i as f32 / 16.0;
                    let y = 10.0 - 20.0 * (t * t * (3.0 - 2.0 * t));
                    (-12.0 + 24.0 * t, y)
                })
                .collect();
            Pen { w: 1.6, ..s }.path(&pts);
        }
        Icon::FxBrightness => {
            s.circle(0.0, 0.0, 5.8);
            let half: Vec<Pos2> = (0..=12)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + i as f32 / 12.0 * std::f32::consts::PI;
                    s.pt(5.8 * a.cos(), 5.8 * a.sin())
                })
                .collect();
            s.p.add(Shape::convex_polygon(half, col, Stroke::NONE));
            let r = Pen { w: 1.6, ..s };
            for i in 0..8 {
                let a = i as f32 / 8.0 * TAU;
                r.line(9.0 * a.cos(), 9.0 * a.sin(), 11.5 * a.cos(), 11.5 * a.sin());
            }
        }
        Icon::FxDrop => {
            let mut pts = vec![s.pt(0.0, -11.0)];
            for i in 0..=16 {
                let a = -0.55 + i as f32 / 16.0 * (std::f32::consts::PI + 1.1);
                pts.push(s.pt(6.5 * a.cos(), 4.0 + 6.5 * a.sin()));
            }
            pts.reverse();
            s.p.add(Shape::convex_polygon(pts, col, Stroke::NONE));
        }
        Icon::FxMerge | Icon::FxMergeSplit => {
            // Pictures stacked, with an arrow coming in from the left.
            Pen {
                col: s.dim(110),
                ..s
            }
            .path(&[
                (-4.0, -7.0),
                (-4.0, -11.0),
                (16.0, -11.0),
                (16.0, 3.0),
                (12.0, 3.0),
            ]);
            s.p.add(Shape::convex_polygon(
                [(-8.0, -6.0), (12.0, -6.0), (12.0, 8.0), (-8.0, 8.0)]
                    .iter()
                    .map(|(x, y)| s.pt(*x, *y))
                    .collect(),
                s.dim(70),
                Stroke::NONE,
            ));
            s.rect(-8.0, -7.0, 21.0, 16.0, 1.5);
            if icon == Icon::FxMergeSplit {
                s.path(&[(-16.0, -6.0), (-16.0, 5.0), (-9.0, 5.0)]);
                s.path(&[(-16.0, -1.0), (-11.0, -1.0)]);
                s.path(&[(-11.5, 2.5), (-9.0, 5.0), (-11.5, 7.5)]);
            } else {
                s.path(&[(-16.0, -4.0), (-16.0, 3.0), (-9.0, 3.0)]);
                s.path(&[(-11.5, 0.5), (-9.0, 3.0), (-11.5, 5.5)]);
            }
        }
        Icon::FxDissolve => {
            Pen {
                col: s.dim(90),
                ..s
            }
            .fill(-13.0, -11.0, 18.0, 13.0, 1.5);
            Pen {
                col: Color32::from_rgb(34, 33, 38),
                ..s
            }
            .fill(-9.0, -5.0, 22.0, 15.0, 1.5);
            for ix in 0..9 {
                for iy in 0..5 {
                    if (ix + iy) % 2 == 0 {
                        Pen {
                            col: s.dim(110),
                            ..s
                        }
                        .fill(
                            -7.5 + ix as f32 * 2.2,
                            -3.5 + iy as f32 * 2.4,
                            1.2,
                            1.2,
                            0.0,
                        );
                    }
                }
            }
            s.rect(-9.0, -5.0, 22.0, 15.0, 1.5);
        }
        Icon::FxMatte => {
            Pen {
                col: s.dim(150),
                ..s
            }
            .fill(-12.0, -11.0, 24.0, 16.0, 1.0);
            Pen {
                col: Color32::from_rgb(20, 20, 22),
                ..s
            }
            .dot(-1.0, -3.0, 4.6);
            s.line(-11.0, 10.0, 11.0, 10.0);
            s.dot(-1.0, 10.0, 2.0);
        }
        Icon::FxTransform => {
            s.tint(-9.0, -7.0, 18.0, 14.0, 1.5, 70);
            s.rect(-9.0, -7.0, 18.0, 14.0, 1.5);
            s.path(&[(3.0, -11.0), (12.0, -11.0), (12.0, -4.0)]);
            s.path(&[(9.5, -6.5), (12.0, -4.0), (14.5, -6.5)]);
            s.path(&[(-3.0, 11.0), (-12.0, 11.0), (-12.0, 4.0)]);
            s.path(&[(-14.5, 6.5), (-12.0, 4.0), (-9.5, 6.5)]);
        }
        Icon::FxRectMask => {
            s.rect(-9.5, -9.5, 19.0, 19.0, 0.0);
            for (x, y) in [(-9.5, -9.5), (9.5, -9.5), (9.5, 9.5), (-9.5, 9.5)] {
                Pen {
                    col: Color32::from_rgb(34, 33, 38),
                    ..s
                }
                .dot(x, y, 2.2);
                Pen { w: 1.2, ..s }.circle(x, y, 2.0);
            }
        }
        Icon::FxEllipseMask => {
            s.circle(0.0, 0.0, 10.0);
            for (x, y) in [(0.0, -10.0), (10.0, 0.0), (0.0, 10.0), (-10.0, 0.0)] {
                Pen {
                    col: Color32::from_rgb(34, 33, 38),
                    ..s
                }
                .dot(x, y, 2.2);
                Pen { w: 1.2, ..s }.circle(x, y, 2.0);
            }
        }
        Icon::FxPolyMask => {
            let b = (-1.0, -4.0);
            for (x, y) in [(-13.0, 6.0), (11.0, 5.0), (1.0, 13.0), (4.0, -12.0)] {
                s.line(b.0, b.1, x, y);
                Pen {
                    col: Color32::from_rgb(34, 33, 38),
                    ..s
                }
                .dot(x, y, 2.0);
                Pen { w: 1.2, ..s }.circle(x, y, 1.8);
            }
        }
        Icon::FxBSpline => {
            let pts: Vec<(f32, f32)> = (0..=16)
                .map(|i| {
                    let t = i as f32 / 16.0;
                    (-13.0 + 26.0 * t, 6.0 - 26.0 * t * (1.0 - t))
                })
                .collect();
            s.path(&pts);
            let dim = Pen {
                col: s.dim(110),
                w: 1.0,
                ..s
            };
            dim.line(-13.0, 6.0, -7.0, -6.0);
            dim.line(13.0, 6.0, 7.0, -6.0);
            for (x, y) in [(-13.0, 6.0), (13.0, 6.0)] {
                Pen { w: 1.2, ..s }.circle(x, y, 1.8);
            }
            s.dot(-7.0, -6.0, 1.3);
            s.dot(7.0, -6.0, 1.3);
        }
        Icon::FxWave => {
            for (y0, phase) in [(-4.0, 0.0), (5.0, std::f32::consts::PI)] {
                let pts: Vec<(f32, f32)> = (0..=20)
                    .map(|i| {
                        let t = i as f32 / 20.0;
                        (-13.0 + 26.0 * t, y0 + 3.5 * (t * TAU + phase).sin())
                    })
                    .collect();
                s.path(&pts);
            }
            for (x, y) in [(-13.0, -4.0), (13.0, -4.0), (-13.0, 5.0), (13.0, 5.0)] {
                s.dot(x, y, 1.5);
            }
        }
        Icon::FxEmitter => {
            // A plate giving off a cloud of points.
            Pen { w: 1.3, ..s }.closed(&[(-14.0, 7.0), (-6.0, 2.0), (2.0, 7.0), (-6.0, 12.0)]);
            for (x, y, r) in [
                (-4.0, 3.0, 1.2),
                (0.0, 0.0, 1.2),
                (3.0, -3.0, 1.3),
                (7.0, -6.0, 1.3),
                (10.0, -10.0, 1.2),
                (-1.0, 4.0, 1.0),
                (4.0, 1.0, 1.1),
                (8.0, -2.0, 1.2),
                (12.0, -5.0, 1.1),
                (5.0, -8.0, 1.1),
                (9.0, -12.0, 1.0),
                (13.0, -9.0, 1.0),
                (1.0, -5.0, 1.0),
            ] {
                s.dot(x, y, r);
            }
        }
        Icon::FxRender => {
            // A magnet among points, with arrows four ways.
            Pen { w: 3.0, ..s }.arc(-5.0, 2.0, 5.0, 0.2, std::f32::consts::PI - 0.2);
            Pen { w: 3.0, ..s }.line(-10.0, 2.0, -10.0, -2.0);
            Pen { w: 3.0, ..s }.line(0.0, 2.0, 0.0, -2.0);
            for (x, y) in [
                (-12.0, -9.0),
                (-6.0, -11.0),
                (0.0, -9.0),
                (6.0, -11.0),
                (12.0, -9.0),
                (-9.0, -6.0),
                (3.0, -6.0),
            ] {
                s.dot(x, y, 1.2);
            }
            let a = Pen { w: 1.3, ..s };
            a.line(8.0, -3.0, 8.0, 11.0);
            a.line(1.0, 4.0, 15.0, 4.0);
            a.path(&[(6.0, -1.0), (8.0, -3.0), (10.0, -1.0)]);
            a.path(&[(6.0, 9.0), (8.0, 11.0), (10.0, 9.0)]);
            a.path(&[(3.0, 2.0), (1.0, 4.0), (3.0, 6.0)]);
            a.path(&[(13.0, 2.0), (15.0, 4.0), (13.0, 6.0)]);
        }
        Icon::Gallery => {
            // A camera body with a sparkle: the stills gallery.
            Pen { w: 1.6, ..s }.rect(-12.0, -5.0, 20.0, 15.0, 3.0);
            s.circle(-2.0, 2.5, 4.0);
            s.path(&[(-7.0, -5.0), (-5.0, -8.0), (1.0, -8.0), (3.0, -5.0)]);
            s.star4(10.0, -9.0, 3.6);
            s.star4(13.0, -2.0, 2.0);
        }
        Icon::LutGrid => {
            Pen { w: 1.5, ..s }.rect(-13.0, -9.0, 18.0, 14.0, 2.0);
            s.dot(-8.0, -4.5, 1.4);
            s.path(&[(-11.0, 3.0), (-6.0, -1.0), (-2.0, 2.0)]);
            for ix in 0..4 {
                for iy in 0..3 {
                    Pen {
                        col: s.dim(170),
                        ..s
                    }
                    .fill(
                        1.0 + ix as f32 * 3.4,
                        2.0 + iy as f32 * 3.4,
                        2.0,
                        2.0,
                        0.3,
                    );
                }
            }
        }
        Icon::FxBox => {
            Pen { w: 1.5, ..s }.rect(-12.0, -11.0, 24.0, 22.0, 3.0);
            s.text(0.0, 0.5, "fx", 15.0);
        }
        Icon::Lightbox => {
            Pen { w: 1.5, ..s }.rect(-14.0, -10.0, 28.0, 20.0, 3.0);
            for ix in 0..5 {
                for iy in 0..3 {
                    Pen {
                        col: s.dim(180),
                        ..s
                    }
                    .fill(
                        -10.5 + ix as f32 * 4.6,
                        -6.0 + iy as f32 * 4.4,
                        2.6,
                        2.6,
                        0.3,
                    );
                }
            }
        }
        Icon::Download => {
            s.line(0.0, -10.0, 0.0, 3.0);
            s.path(&[(-5.0, -2.0), (0.0, 3.0), (5.0, -2.0)]);
            s.path(&[(-11.0, 4.0), (-11.0, 9.0), (11.0, 9.0), (11.0, 4.0)]);
        }
        Icon::Expand => {
            for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                s.path(&[
                    (sx * 10.0, sy * 5.0),
                    (sx * 10.0, sy * 10.0),
                    (sx * 5.0, sy * 10.0),
                ]);
                s.line(sx * 10.0, sy * 10.0, sx * 4.0, sy * 4.0);
            }
        }
        Icon::ViewerSplit => {
            s.tint(-12.0, -8.0, 24.0, 16.0, 1.0, 50);
            Pen { w: 1.3, ..s }.rect(-12.0, -8.0, 24.0, 16.0, 1.0);
            Pen {
                col: s.dim(170),
                ..s
            }
            .fill(-2.0, -6.0, 12.0, 12.0, 0.5);
        }
        Icon::ColorWipe => {
            // A circular arrow in two colours with a sparkle: the split-screen wipe.
            let pi = std::f32::consts::PI;
            Pen {
                col: Color32::from_rgb(232, 196, 60),
                w: 2.0,
                ..s
            }
            .arc(0.0, 0.0, 8.0, pi * 0.9, pi * 1.9);
            Pen {
                col: Color32::from_rgb(220, 60, 60),
                w: 2.0,
                ..s
            }
            .arc(0.0, 0.0, 8.0, pi * 1.9, pi * 2.6);
            Pen {
                col: Color32::from_rgb(170, 90, 220),
                w: 2.0,
                ..s
            }
            .arc(0.0, 0.0, 8.0, pi * 2.6, pi * 2.85);
            Pen {
                col: Color32::from_rgb(232, 196, 60),
                ..s
            }
            .star4(-4.0, 3.0, 3.6);
        }
        Icon::Pointer => {
            // DaVinci Resolve's selection arrow. The outline is concave (the notch under the
            // head), so it is filled as triangles and edged with a thin line for smooth sides.
            let pts: Vec<Pos2> = [
                (-5.8, -9.0),
                (-5.8, 6.8),
                (-2.2, 3.7),
                (0.7, 9.0),
                (3.5, 8.0),
                (1.6, 3.0),
                (5.6, 1.8),
            ]
            .iter()
            .map(|(x, y)| s.pt(*x, *y))
            .collect();
            let mut mesh = egui::epaint::Mesh::default();
            for p in &pts {
                mesh.colored_vertex(*p, col);
            }
            for [a, b, c] in [[0, 1, 2], [0, 2, 5], [0, 5, 6], [2, 3, 4], [2, 4, 5]] {
                mesh.add_triangle(a, b, c);
            }
            s.p.add(mesh);
            s.p.add(Shape::closed_line(pts, Stroke::new(0.8 * s.k, col)));
        }
        Icon::Layers => {
            for (i, y) in [-6.0f32, -2.0, 2.0, 6.0].into_iter().enumerate() {
                let p = Pen {
                    col: if i == 0 { col } else { s.dim(160) },
                    w: 1.4,
                    ..s
                };
                p.rect(-11.0, y - 1.6, 22.0, 3.2, 1.6);
            }
        }
        Icon::Picker => {
            Pen { w: 2.6, ..s }.line(-9.0, 9.0, 2.0, -2.0);
            s.ellipse(5.0, -5.0, 4.5, 3.0, -0.785);
            Pen { w: 1.3, ..s }.line(-1.0, -8.0, 8.0, 1.0);
            s.star4(10.0, 6.0, 3.0);
        }
        Icon::CamRaw => {
            // A film camera seen at an angle.
            Pen { w: 1.4, ..s }.closed(&[(-13.0, -2.0), (-3.0, -7.0), (8.0, -3.0), (-2.0, 3.0)]);
            Pen { w: 1.4, ..s }.closed(&[(-13.0, -2.0), (-13.0, 7.0), (-2.0, 12.0), (-2.0, 3.0)]);
            Pen { w: 1.4, ..s }.closed(&[(-2.0, 3.0), (8.0, -3.0), (8.0, 6.0), (-2.0, 12.0)]);
            s.circle(-7.0, 5.0, 2.6);
            s.circle(10.0, -8.0, 3.2);
        }
        Icon::ColorMatch => {
            for ix in 0..5 {
                for iy in 0..4 {
                    let a = if (ix + iy) % 2 == 0 { 220 } else { 120 };
                    Pen { col: s.dim(a), ..s }.dot(
                        -9.0 + ix as f32 * 4.5,
                        -7.0 + iy as f32 * 4.5,
                        1.4,
                    );
                }
            }
        }
        Icon::WheelDot => {
            Pen { w: 1.8, ..s }.circle(0.0, 0.0, 11.0);
            s.dot(0.0, 0.0, 3.5);
        }
        Icon::WheelHdr => {
            Pen { w: 1.6, ..s }.arc(-2.0, -2.0, 10.0, 0.4, 5.6);
            s.dot(-2.0, -2.0, 3.0);
            s.text(7.0, 7.5, "HDR", 7.0);
        }
        Icon::RgbMixer => {
            s.dot(-4.0, -5.0, 3.6);
            s.dot(5.0, -1.0, 3.6);
            s.dot(-3.0, 5.0, 3.6);
            Pen {
                col: s.dim(120),
                ..s
            }
            .dot(5.0, 7.0, 2.4);
        }
        Icon::MotionFx => {
            Pen { w: 1.4, ..s }.rect(-13.0, -8.0, 26.0, 16.0, 1.5);
            s.dot(-6.0, -2.0, 2.4);
            s.line(-2.0, -2.0, 9.0, -2.0);
            s.line(-9.0, 3.5, 9.0, 3.5);
        }
        Icon::Qualifier => {
            let mut pts = vec![s.pt(-4.0, -10.0)];
            for i in 0..=14 {
                let a = -0.5 + i as f32 / 14.0 * (std::f32::consts::PI + 1.0);
                pts.push(s.pt(-4.0 + 6.0 * a.cos(), 3.0 + 6.0 * a.sin()));
            }
            pts.reverse();
            s.p.add(Shape::closed_line(pts, Stroke::new(1.5 * s.k, col)));
            for ix in 0..3 {
                for iy in 0..4 {
                    Pen {
                        col: s.dim(150),
                        ..s
                    }
                    .dot(5.0 + ix as f32 * 3.2, -6.0 + iy as f32 * 3.6, 0.9);
                }
            }
        }
        Icon::Hexagon => {
            let pts: Vec<(f32, f32)> = (0..6)
                .map(|i| {
                    let a = i as f32 / 6.0 * TAU;
                    (11.0 * a.cos(), 11.0 * a.sin())
                })
                .collect();
            Pen { w: 1.4, ..s }.closed(&pts);
            let thin = Pen {
                col: s.dim(150),
                w: 1.0,
                ..s
            };
            for i in 0..3 {
                let (a, b) = (pts[i], pts[i + 3]);
                thin.line(a.0, a.1, b.0, b.1);
            }
            thin.closed(&[pts[0], pts[2], pts[4]]);
            thin.closed(&[pts[1], pts[3], pts[5]]);
        }
        Icon::Needle => {
            Pen { w: 2.0, ..s }.line(-9.0, 10.0, 5.0, -4.0);
            s.ellipse(7.0, -7.0, 4.0, 2.6, -0.785);
            Pen {
                col: s.dim(120),
                w: 1.0,
                ..s
            }
            .line(-12.0, 13.0, -9.0, 10.0);
        }
        Icon::CircleWindow => {
            Pen { w: 1.4, ..s }.circle(0.0, 0.0, 10.5);
            for (x, y) in [(0.0, -10.5), (10.5, 0.0), (0.0, 10.5), (-10.5, 0.0)] {
                s.dot(x, y, 2.0);
            }
        }
        Icon::Tracker => {
            Pen { w: 1.4, ..s }.circle(0.0, 0.0, 8.5);
            s.circle(0.0, 0.0, 3.0);
            for (a, b, c, d) in [
                (0.0, -12.0, 0.0, -6.0),
                (0.0, 6.0, 0.0, 12.0),
                (-12.0, 0.0, -6.0, 0.0),
                (6.0, 0.0, 12.0, 0.0),
            ] {
                s.line(a, b, c, d);
            }
        }
        Icon::MagicMask => {
            let mut pts = vec![s.pt(-5.0, -11.0)];
            for i in 0..=12 {
                let a = -0.5 + i as f32 / 12.0 * (std::f32::consts::PI + 1.0);
                pts.push(s.pt(-5.0 + 5.0 * a.cos(), -1.0 + 5.0 * a.sin()));
            }
            pts.reverse();
            s.p.add(Shape::convex_polygon(pts, col, Stroke::NONE));
            s.tri([(4.0, -4.0), (12.0, 10.0), (-4.0, 10.0)]);
        }
        Icon::BlurBox => {
            s.tint(-14.0, -10.0, 28.0, 20.0, 3.0, 90);
            Pen {
                col: Color32::from_rgb(40, 40, 44),
                ..s
            }
            .dot(0.0, 1.0, 5.0);
            Pen { w: 1.4, ..s }.rect(-14.0, -10.0, 28.0, 20.0, 3.0);
        }
        Icon::Sizing => {
            Pen { w: 1.4, ..s }.rect(-10.0, -7.0, 20.0, 14.0, 1.0);
            s.path(&[(4.0, -11.0), (14.0, -11.0), (14.0, -3.0)]);
            s.path(&[(-4.0, 11.0), (-14.0, 11.0), (-14.0, 3.0)]);
        }
        Icon::Box3D => {
            Pen { w: 1.4, ..s }.rect(-14.0, -10.0, 28.0, 20.0, 3.0);
            s.text(0.0, 0.5, "3D", 11.0);
        }
        Icon::ScopeGraph => {
            Pen { w: 1.6, ..s }.path(&[(-12.0, -12.0), (-12.0, 10.0), (13.0, 10.0)]);
            Pen { w: 1.6, ..s }.path(&[
                (-9.0, 6.0),
                (-7.0, -4.0),
                (-5.0, 3.0),
                (-2.0, -7.0),
                (1.0, 4.0),
                (4.0, -3.0),
                (7.0, 1.0),
                (9.0, -6.0),
                (12.0, 3.0),
            ]);
        }
        Icon::BarsVert => {
            for i in 0..5 {
                let x = -8.0 + i as f32 * 4.0;
                Pen { w: 1.5, ..s }.line(x, -9.0, x, 9.0);
            }
        }
        Icon::WheelLog => {
            Pen { w: 1.5, ..s }.arc(-2.0, -2.0, 10.0, 0.4, 5.6);
            s.dot(-2.0, -2.0, 2.5);
            s.text(7.0, 7.5, "LOG", 6.5);
        }
        Icon::Reset => {
            Pen { w: 1.5, ..s }.arc(0.0, 0.0, 8.0, -1.9, 3.6);
            s.tri([(-9.0, -10.0), (-3.0, -8.5), (-8.0, -4.0)]);
        }
        Icon::PlusDot => {
            Pen { w: 1.4, ..s }.line(-10.0, 0.0, 10.0, 0.0);
            Pen { w: 1.4, ..s }.line(0.0, -10.0, 0.0, 10.0);
            s.dot(6.0, 6.0, 3.0);
        }
        Icon::AutoA => {
            Pen { w: 1.4, ..s }.circle(0.0, 0.0, 11.0);
            s.text(0.0, 0.5, "A", 13.0);
        }
        Icon::ThumbGrid => {
            for ix in 0..3 {
                for iy in 0..2 {
                    s.fill(
                        -10.0 + ix as f32 * 7.2,
                        -6.0 + iy as f32 * 7.2,
                        5.6,
                        5.6,
                        0.5,
                    );
                }
            }
        }
        Icon::BinFolder => {
            s.p.add(Shape::convex_polygon(
                [
                    (-11.0, -8.0),
                    (-3.0, -8.0),
                    (-1.0, -5.5),
                    (11.0, -5.5),
                    (11.0, 8.0),
                    (-11.0, 8.0),
                ]
                .iter()
                .map(|(x, y)| s.pt(*x, *y))
                .collect(),
                s.dim(150),
                Stroke::NONE,
            ));
            Pen {
                col: Color32::from_rgb(40, 40, 45),
                ..s
            }
            .fill(-9.0, -3.0, 18.0, 9.0, 0.5);
            Pen {
                col: s.dim(150),
                ..s
            }
            .fill(-9.0, -3.0, 18.0, 1.5, 0.0);
        }
        Icon::SmartBin => {
            Pen { w: 1.4, ..s }.rect(-11.0, -8.0, 22.0, 16.0, 1.5);
            for (i, h) in [4.0f32, 8.0, 6.0].into_iter().enumerate() {
                s.fill(-6.0 + i as f32 * 4.5, 5.0 - h, 2.6, h, 0.3);
            }
        }
        Icon::Cloud => {
            let c = Pen { w: 1.5, ..s };
            c.arc(-5.0, 1.0, 5.0, 1.57, 4.4);
            c.arc(1.0, -2.0, 6.5, 3.6, 6.2);
            c.arc(7.0, 3.0, 4.0, -1.2, 1.57);
            c.line(-5.0, 6.0, 7.0, 7.0);
            c.line(1.0, 1.0, 1.0, 11.0);
            c.path(&[(-2.0, 8.0), (1.0, 11.0), (4.0, 8.0)]);
        }
        Icon::SnapHook => {
            // A magnet's hook: snapping.
            let h = Pen { w: 2.0, ..s };
            h.arc(1.0, 3.0, 7.0, 0.3, 3.6);
            h.line(8.0, 1.0, 8.0, -9.0);
            h.path(&[(5.0, -6.0), (8.0, -9.0), (11.0, -6.0)]);
        }
        Icon::PositionLock => {
            Pen { w: 1.6, ..s }.rect(-8.0, -2.0, 16.0, 12.0, 2.0);
            Pen { w: 1.6, ..s }.arc(0.0, -3.0, 5.0, std::f32::consts::PI, TAU);
            s.dot(0.0, 4.0, 1.8);
        }
        Icon::ZoomRuler => {
            for i in 0..8 {
                let x = -13.0 + i as f32 * 3.0;
                let h = if i % 4 == 0 { 5.0 } else { 2.5 };
                s.line(x, -10.0, x, -10.0 + h);
            }
            s.line(-13.0, -10.0, 9.0, -10.0);
            Pen { w: 1.8, ..s }.circle(0.0, 2.0, 5.5);
            Pen { w: 2.4, ..s }.line(4.0, 6.0, 9.0, 11.0);
        }
        Icon::StripView => {
            Pen { w: 1.3, ..s }.rect(-12.0, -8.0, 24.0, 16.0, 1.0);
            s.line(-12.0, -2.5, 12.0, -2.5);
            s.line(-12.0, 2.5, 12.0, 2.5);
            for x in [-6.0, 0.0, 6.0] {
                Pen {
                    col: s.dim(120),
                    w: 1.0,
                    ..s
                }
                .line(x, -2.5, x, 2.5);
            }
        }
        Icon::SparkleFrame => {
            Pen { w: 1.3, ..s }.rect(-12.0, -8.0, 24.0, 16.0, 1.0);
            s.star4(-1.0, 0.0, 4.5);
            s.star4(6.0, -3.0, 2.2);
            s.dot(-7.0, 4.0, 1.0);
        }
        Icon::Info => {
            Pen { w: 1.6, ..s }.circle(0.0, 0.0, 11.0);
            s.dot(0.0, -5.5, 1.6);
            Pen { w: 2.0, ..s }.line(0.0, -2.0, 0.0, 6.5);
        }
        Icon::Drive => {
            // A hard drive: a box with a slanted top and a slot.
            Pen { w: 1.6, ..s }.closed(&[(-11.0, 2.0), (-8.0, -10.0), (8.0, -10.0), (11.0, 2.0)]);
            Pen { w: 1.6, ..s }.rect(-12.0, 2.0, 24.0, 9.0, 1.5);
            s.line(-7.0, 6.5, -2.0, 6.5);
            s.dot(6.0, 6.5, 1.2);
        }
        Icon::FolderPlus => {
            let f = Pen { w: 1.5, ..s };
            f.path(&[
                (2.0, 8.0),
                (-12.0, 8.0),
                (-12.0, -9.0),
                (-5.0, -9.0),
                (-3.0, -6.0),
                (10.0, -6.0),
                (10.0, -1.0),
            ]);
            f.line(-12.0, -3.0, 10.0, -3.0);
            f.circle(8.0, 7.0, 5.0);
            s.line(8.0, 4.0, 8.0, 10.0);
            s.line(5.0, 7.0, 11.0, 7.0);
        }
    }
}

/// The Deliver page's rocket: a body with a lit side, a window, two fins, a nozzle and a
/// flame. While the page is open it is blue with a red nozzle and a yellow flame; otherwise it
/// is drawn in the colour `s` carries, in shades of grey.
fn rocket(s: &Pen, on: bool) {
    // Points along the rocket's axis: `t` from the nozzle towards the nose, `w` across it.
    let (bx, by) = (-5.8, 4.6);
    let (ux, uy) = (0.703, -0.711);
    let (vx, vy) = (0.711, 0.703);
    let at = |t: f32, w: f32| (bx + ux * t + vx * w, by + uy * t + vy * w);
    let fill = |pts: &[(f32, f32)], col: Color32| {
        s.p.add(Shape::convex_polygon(
            pts.iter().map(|(x, y)| s.pt(*x, *y)).collect(),
            col,
            Stroke::NONE,
        ));
    };
    let rgb = |r: u8, g: u8, b: u8| Color32::from_rgb(r, g, b);
    // (outline, fin, body, lit side, window, window ring, nozzle, flame)
    let [line, fin_fill, body_fill, lit, window, ring, nozzle, flame] = if on {
        [
            rgb(146, 198, 240),
            rgb(10, 22, 44),
            rgb(22, 62, 128),
            rgb(78, 140, 210),
            rgb(10, 26, 52),
            rgb(206, 224, 242),
            rgb(178, 78, 62),
            rgb(218, 174, 40),
        ]
    } else {
        [
            s.col,
            Color32::TRANSPARENT,
            s.dim(28),
            s.dim(70),
            Color32::from_rgb(28, 28, 31),
            s.col,
            s.dim(150),
            s.col,
        ]
    };
    let pen = |col: Color32, w: f32| Pen { col, w, ..*s };
    let outline = pen(line, if on { 1.8 } else { 1.5 });

    // The fins, behind the body.
    for side in [-1.0, 1.0] {
        let fin = [
            at(10.5, 4.6 * side),
            at(3.2, 9.8 * side),
            at(0.4, 9.0 * side),
            at(2.0, 4.6 * side),
        ];
        fill(&fin, fin_fill);
        outline.closed(&fin);
    }

    let profile = [
        (0.0, 3.6),
        (3.0, 5.1),
        (8.0, 5.8),
        (13.0, 5.5),
        (17.0, 4.3),
        (20.0, 2.8),
        (22.6, 0.7),
    ];
    let mut body: Vec<(f32, f32)> = profile.iter().map(|&(t, w)| at(t, w)).collect();
    body.extend(profile.iter().rev().map(|&(t, w)| at(t, -w)));
    fill(&body, body_fill);
    // The lit side of the body.
    for pair in profile.windows(2) {
        let [(t0, w0), (t1, w1)] = [pair[0], pair[1]];
        fill(
            &[
                at(t0, -w0 + 0.6),
                at(t1, -w1 + 0.6),
                at(t1, -w1 * 0.35),
                at(t0, -w0 * 0.35),
            ],
            lit,
        );
    }
    outline.closed(&body);

    // The window.
    let (wx, wy) = at(12.0, 0.0);
    pen(window, 1.0).dot(wx, wy, 2.0);
    pen(ring, 1.2).circle(wx, wy, 2.1);

    // The nozzle and the flame.
    fill(
        &[at(0.4, -2.4), at(0.4, 2.4), at(-1.8, 1.8), at(-1.8, -1.8)],
        nozzle,
    );
    pen(flame, 2.0).path(&[(-10.2, 4.4), (-10.2, 9.4), (-5.0, 9.4)]);
}
