//! The Media page: the folders of the computer and their files at the top left, the viewer with
//! its transport, the level meters, and under them the bins, the media pool and what is known
//! about the clip chosen.
//!
//! Positions are those of the reference screen, in the 1872-point-wide window (see `chrome.rs`).

// Folder browsing, render settings and the queue only run on the desktop; the browser build keeps
// the types so the page still draws.
#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use egui::{pos2, vec2, Align2, Color32, Context, Painter, Rect, Sense, Stroke, Ui, UiBuilder};
use picasso_core::{MediaItem, MediaSource};

use crate::app::StudioApp;
use crate::chrome::{Screen, Tool, DESIGN_WIDTH, R};
use crate::icons::{paint, Icon};
use crate::panels::Split;
use crate::theme::{self, regular, semibold};

/// The page is laid out at a larger size than Resolve draws it and shown scaled down by this
/// much (see `media_scaled`), so that its columns, rows and text come out as Resolve's at 1080p
/// (`docs/reference/resolve-1080p`). Positions below are in the larger units.
pub(crate) const MEDIA_SCALE: f32 = 0.64;
/// A pixel of Resolve at 1080p in this page's units.
const fn rx(x: f32) -> f32 {
    x * R / MEDIA_SCALE
}
/// The x positions that split the page into columns: the storage, the files, the render queue,
/// the viewer.
const STORAGE_RIGHT: f32 = rx(251.0);
const FILES_RIGHT: f32 = rx(750.0);
const QUEUE_RIGHT: f32 = rx(1111.0);
const VIEWER_RIGHT: f32 = rx(1498.0);
const LEFT: f32 = 0.0;
const RIGHT: f32 = DESIGN_WIDTH / MEDIA_SCALE - 1.0;
/// The bar above each part of the page.
const BAR: f32 = rx(25.0);
const ROW: f32 = rx(27.0);
const TILE: f32 = 107.0;
const FILE_TEXT: Color32 = Color32::from_rgb(244, 240, 236);
const DIM_TEXT: Color32 = Color32::from_rgb(150, 150, 156);
/// Where the dB marks of the meters lie, from the top (0 dB) to the bottom of a bar, as shares
/// of its height.
const SCALE: [(i32, f32); 8] = [
    (0, 0.0),
    (-5, 0.126),
    (-10, 0.249),
    (-15, 0.375),
    (-20, 0.498),
    (-30, 0.699),
    (-40, 0.845),
    (-50, 0.922),
];
/// Channels the meters show.
const CHANNELS: usize = 16;
const IMAGE_EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum MeterTab {
    #[default]
    Meters,
    Waveform,
}

/// The page's own state: which folders are open and which file is chosen.
#[derive(Default)]
pub(crate) struct MediaPage {
    roots: Option<Vec<(PathBuf, String)>>,
    open: HashSet<PathBuf>,
    folders: HashMap<PathBuf, Vec<(PathBuf, String)>>,
    chosen: Option<PathBuf>,
    files: Option<(PathBuf, Vec<PathBuf>)>,
    chosen_file: Option<PathBuf>,
    tab: MeterTab,
}

/// What a file is, for the media storage listing.
fn is_media_file(path: &Path) -> bool {
    let name = path.to_string_lossy();
    crate::platform::is_path_media(&name)
        || path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

fn name_of(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// The places the tree starts from: the user's videos folder and the drives.
#[cfg(not(target_arch = "wasm32"))]
fn find_roots() -> Vec<(PathBuf, String)> {
    let mut roots = Vec::new();
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from);
    if let Some(home) = home {
        let videos = home.join("Videos");
        let start = if videos.is_dir() { videos } else { home };
        let label = start.display().to_string();
        roots.push((start, label));
    }
    #[cfg(windows)]
    for letter in b'C'..=b'Z' {
        let drive = PathBuf::from(format!("{}:\\", letter as char));
        if drive.exists() {
            let label = drive.display().to_string();
            roots.push((drive, label));
        }
    }
    #[cfg(not(windows))]
    roots.push((PathBuf::from("/"), "/".to_string()));
    roots
}

#[cfg(target_arch = "wasm32")]
fn find_roots() -> Vec<(PathBuf, String)> {
    Vec::new()
}

/// The folders inside `dir`, by name; hidden ones and what cannot be read are left out.
fn read_folders(dir: &Path) -> Vec<(PathBuf, String)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut folders: Vec<(PathBuf, String)> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| (e.path(), e.file_name().to_string_lossy().into_owned()))
        .filter(|(_, name)| !name.starts_with('.') && !name.starts_with('$'))
        .take(500)
        .collect();
    folders.sort_by_key(|(_, name)| name.to_lowercase());
    folders
}

/// The media files directly inside `dir`, by name.
fn read_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.path())
        .filter(|p| is_media_file(p))
        .take(2000)
        .collect();
    files.sort_by_key(|p| name_of(p).to_lowercase());
    files
}

/// How far down a bar a level lies, 0 at the top and 1 at the bottom; levels are 1.0 at 0 dB.
fn level_share(level: f32) -> f32 {
    let db = 20.0 * level.max(1e-6).log10();
    let last = SCALE[SCALE.len() - 1];
    if db >= 0.0 {
        return 0.0;
    }
    if db <= last.0 as f32 {
        return 1.0;
    }
    for pair in SCALE.windows(2) {
        let ((d0, y0), (d1, y1)) = (pair[0], pair[1]);
        if db <= d0 as f32 && db >= d1 as f32 {
            return y0 + (y1 - y0) * (d0 as f32 - db) / (d0 - d1) as f32;
        }
    }
    1.0
}

/// The regular font at `size`, or smaller so that `text` is no wider than `room`.
pub(crate) fn fitting(p: &Painter, text: &str, size: f32, room: f32) -> egui::FontId {
    let width = p
        .layout_no_wrap(text.to_string(), regular(size), Color32::WHITE)
        .size()
        .x;
    regular(size * (room.max(1.0) / width.max(1.0)).min(1.0))
}

/// `text` cut to fit in `room` points in `font`, ending in "…" when it was cut. A name from a
/// file can be as long as a sentence; drawn whole it runs over whatever is beside it.
pub(crate) fn elided(p: &Painter, text: &str, font: egui::FontId, room: f32) -> String {
    let width = |t: &str| {
        p.layout_no_wrap(t.to_string(), font.clone(), Color32::WHITE)
            .size()
            .x
    };
    if width(text) <= room {
        return text.to_string();
    }
    // The longest start of the text that fits with the ellipsis, found by halving.
    let chars: Vec<char> = text.chars().collect();
    let (mut lo, mut hi) = (0, chars.len());
    while lo < hi {
        let mid = (lo + hi + 1).div_ceil(2);
        let candidate: String = chars[..mid].iter().collect::<String>() + "…";
        if width(&candidate) <= room {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    if lo == 0 {
        return String::new();
    }
    chars[..lo].iter().collect::<String>().trim_end().to_string() + "…"
}

/// "No clips in media pool" in the panel from `x0`, `y0` to `x1`, `y1`, growing and shrinking
/// with it as the dividers move, and never wider than it.
fn empty_pool_message(p: &Painter, s: &Screen, x0: f32, x1: f32, y0: f32, y1: f32) {
    let area = s.r(x0, y0, x1, y1);
    let (tall, _) = s.grown(x0, y0, x1, y1);
    let k = tall.clamp(0.4, 2.0);
    let mid = area.top() + area.height() * 0.43;
    let room = area.width() - 24.0;
    let title = "No clips in media pool";
    crate::i18n::paint_text(&p,
        pos2(area.center().x, mid),
        Align2::CENTER_CENTER,
        title,
        fitting(p, title, rx(27.0) * k, room),
        theme::EMPTY_TITLE,
    );
    let sub = "Add clips from Media Storage to get started";
    crate::i18n::paint_text(&p,
        pos2(area.center().x, mid + 70.0 * k),
        Align2::CENTER_CENTER,
        sub,
        fitting(p, sub, rx(10.0) * k, room),
        theme::EMPTY_SUB,
    );
}

fn fill(p: &Painter, s: &Screen, x0: f32, y0: f32, x1: f32, y1: f32, color: Color32) {
    p.rect_filled(s.r(x0, y0, x1, y1), 0.0, color);
}

impl StudioApp {
    /// Draws the page scaled down by `MEDIA_SCALE` in a layer of its own, which also scales where
    /// the pointer is.
    pub(crate) fn media_scaled(&mut self, ctx: &Context, s: &Screen) {
        let id = egui::Id::new("media_layer");
        let layer = egui::LayerId::new(egui::Order::Middle, id);
        ctx.set_transform_layer(layer, egui::emath::TSTransform::from_scaling(MEDIA_SCALE));
        let k = MEDIA_SCALE;
        let sv = Screen {
            o: pos2(s.o.x / k, s.o.y / k),
            h: s.h / k,
            warp: Default::default(),
        };
        // egui draws an Area's first frame invisible (its size is not known yet); repeat that
        // pass before anything is shown, so the page does not flash black when first opened.
        let first = ctx.memory(|m| m.area_rect(id)).is_none();
        egui::Area::new(id)
            .order(egui::Order::Middle)
            .fixed_pos(sv.o)
            .constrain(false)
            .movable(false)
            // An Area fades in from transparent each time it shows again, which made the page
            // black for a moment on every visit; a page appears at once.
            .fade_in(false)
            .show(ctx, |ui| {
                ui.set_clip_rect(Rect::from_min_size(sv.o, vec2(DESIGN_WIDTH / k, sv.h)));
                self.media_page(ui, ctx, &sv);
            });
        if first {
            ctx.request_discard("page layer sizing");
        }
    }

    pub(crate) fn media_page(&mut self, ui: &mut Ui, ctx: &Context, s: &Screen) {
        let painter = ui.painter().clone();
        let top = 79.0 / MEDIA_SCALE;
        let bottom = s.h - 46.8 / MEDIA_SCALE;
        // Resolve's middle bar is at row 520 of 1080p, under the toolbar's 37.
        let middle = ((79.0 + (520.0 - 37.0) * R) / MEDIA_SCALE)
            .min(bottom - 300.0)
            .round();
        // The dividers: the files, the render queue and the viewer in the upper row; the
        // storage and bins column and the meters and metadata column, the whole height; the
        // bar between the two rows (it moves with them unstretched).
        let column = |id, x: f32, y0: f32, y1: f32, a: f32, b: f32| {
            Split::column(id, s, s.p(x, y0), s.p(x, y1), s.p(a, 0.0).x, s.p(b, 0.0).x)
        };
        let splits = [
            column(
                "media files",
                FILES_RIGHT,
                top,
                middle,
                STORAGE_RIGHT,
                QUEUE_RIGHT,
            ),
            column(
                "media queue",
                QUEUE_RIGHT,
                top,
                middle,
                FILES_RIGHT,
                VIEWER_RIGHT,
            ),
            column(
                "media storage",
                STORAGE_RIGHT,
                top,
                bottom,
                LEFT,
                VIEWER_RIGHT,
            ),
            column(
                "media meters",
                VIEWER_RIGHT,
                top,
                bottom,
                STORAGE_RIGHT,
                RIGHT,
            ),
            Split::row(
                "media middle",
                s,
                s.p(LEFT, middle),
                s.p(RIGHT, middle),
                s.p(0.0, top + BAR).y,
                s.p(0.0, bottom).y,
            )
            .keeping(BAR),
        ];
        let warped = self.warped(s, &splits);
        let s = &warped;

        // The panes and the lines between them.
        fill(&painter, s, LEFT, top, RIGHT, bottom, theme::POOL_BG);
        fill(
            &painter,
            s,
            LEFT,
            top,
            RIGHT,
            top + BAR,
            theme::POOL_HEADER_BG,
        );
        fill(
            &painter,
            s,
            LEFT,
            middle,
            RIGHT,
            middle + BAR,
            theme::POOL_HEADER_BG,
        );
        fill(
            &painter,
            s,
            VIEWER_RIGHT,
            top + BAR,
            RIGHT,
            middle,
            theme::METER_BG,
        );
        fill(
            &painter,
            s,
            FILES_RIGHT,
            top + BAR,
            VIEWER_RIGHT,
            middle,
            theme::POOL_BG,
        );
        self.media_jobs_panel(&painter, s, top + BAR, middle);
        let line = Color32::from_rgb(14, 14, 18);
        for y in [top + BAR, middle, middle + BAR] {
            fill(&painter, s, LEFT, y - 1.0, RIGHT, y + 1.0, line);
        }
        for x in [STORAGE_RIGHT, VIEWER_RIGHT] {
            fill(&painter, s, x - 1.0, top, x + 1.0, bottom, line);
        }
        fill(
            &painter,
            s,
            FILES_RIGHT - 1.0,
            top,
            FILES_RIGHT + 1.0,
            middle,
            line,
        );
        fill(
            &painter,
            s,
            QUEUE_RIGHT - 1.0,
            top,
            QUEUE_RIGHT + 1.0,
            middle,
            line,
        );

        self.media_bars(ui, s, top, middle);
        self.storage_tree(ui, s, top + BAR, middle);
        self.storage_files(ui, ctx, s, top + BAR, middle);
        self.source_viewer(ui, s, top + BAR, middle);
        self.embedded_audio(ui, s, top + BAR, middle);
        self.bins_panel(ui, s, middle + BAR, bottom);
        self.pool_area(ui, ctx, s, middle + BAR, bottom);
        self.metadata_panel(ui, s, middle + BAR, bottom);
        self.dividers(ui, s, &splits);
    }

    /// The render queue between the files and the viewer: empty, with its two buttons.
    fn media_jobs_panel(&self, p: &Painter, s: &Screen, y0: f32, y1: f32) {
        let area = s.r(FILES_RIGHT + 1.0, y0, QUEUE_RIGHT - 1.0, y1);
        // The message grows and shrinks with the panel, and stays inside it.
        let (tall, _) = s.grown(FILES_RIGHT + 1.0, y0, QUEUE_RIGHT - 1.0, y1);
        let title = "No jobs in Queue";
        crate::i18n::paint_text(&p,
            pos2(area.center().x, area.top() + area.height() * 0.43),
            Align2::CENTER_CENTER,
            title,
            fitting(
                p,
                title,
                rx(27.0) * tall.clamp(0.4, 2.0),
                area.width() - 24.0,
            ),
            theme::EMPTY_TITLE,
        );
        let bar = Rect::from_min_max(
            pos2(area.left(), area.bottom() - rx(48.0)),
            area.right_bottom(),
        );
        p.rect_filled(bar, 0.0, Color32::from_rgb(40, 40, 44));
        p.rect_filled(
            Rect::from_min_max(bar.left_top(), pos2(bar.right(), bar.top() + 1.0)),
            0.0,
            Color32::from_rgb(14, 14, 18),
        );
        for (x0, x1, label, c) in [
            (771.0, 881.0, "Add Job", Color32::from_rgb(120, 120, 126)),
            (981.0, 1091.0, "Clone", Color32::from_rgb(80, 80, 86)),
        ] {
            let r = Rect::from_min_max(
                pos2(s.p(rx(x0), y0).x, bar.center().y - rx(11.0)),
                pos2(s.p(rx(x1), y0).x, bar.center().y + rx(11.0)),
            );
            p.rect_stroke(
                r,
                rx(11.0),
                Stroke::new(1.0, Color32::from_rgb(60, 60, 66)),
                egui::StrokeKind::Inside,
            );
            crate::i18n::paint_text(&p,
                r.center(),
                Align2::CENTER_CENTER,
                label,
                regular(rx(10.0)),
                c,
            );
        }
    }

    /// The two bars: the tools above the folders and viewer, and above the pool.
    fn media_bars(&mut self, ui: &mut Ui, s: &Screen, top: f32, middle: f32) {
        let painter = ui.painter().clone();
        let t = |hint| Tool::new(hint).wide(40.0);
        let mut planned: Option<&str> = None;
        let y = top + BAR / 2.0;
        // The folders and the files.
        for (x, icon, hint, chev) in [
            (
                rx(18.0),
                Icon::Sidebar,
                "Show or hide the media storage",
                Some(rx(24.0)),
            ),
            (rx(51.0), Icon::ChevLeft, "Back", None),
            (rx(72.0), Icon::ChevRight, "Forward", None),
        ] {
            let tool = if let Some(dx) = chev {
                t(hint).chev(dx)
            } else {
                t(hint).wide(30.0)
            };
            if self
                .tool(ui, s.p(x, y), icon, s.squeezed(tool, x, y))
                .clicked()
            {
                planned = Some(hint);
            }
        }
        // The size of the thumbnails.
        let track = s.r(rx(458.0), y - 10.0, rx(570.0), y + 10.0);
        let slide = ui.interact(track, ui.id().with("thumb-size"), Sense::click_and_drag());
        if let Some(p) = slide
            .interact_pointer_pos()
            .filter(|_| slide.dragged() || slide.clicked())
        {
            self.pool.zoom = ((p.x - track.left() - 6.0) / (track.width() - 12.0)).clamp(0.0, 1.0);
        }
        painter.line_segment(
            [pos2(track.left() + 6.0, y), pos2(track.right() - 6.0, y)],
            Stroke::new(2.0, Color32::from_rgb(60, 60, 66)),
        );
        painter.circle_filled(
            pos2(
                track.left() + 6.0 + (track.width() - 12.0) * self.pool.zoom,
                y,
            ),
            rx(4.0),
            theme::ICON_BRIGHT,
        );
        for (x, icon, hint, chev) in [
            (rx(601.0), Icon::Sort, "Sort", None),
            (rx(630.0), Icon::Grid2, "Thumbnails", None),
            (rx(658.0), Icon::ListView, "List", None),
            (rx(695.0), Icon::Search, "Search", None),
            (rx(732.0), Icon::Menu, "More", None::<f32>),
        ] {
            let tool = if let Some(dx) = chev {
                t(hint).chev(dx)
            } else {
                t(hint).wide(36.0)
            };
            if self
                .tool(ui, s.p(x, y), icon, s.squeezed(tool, x, y))
                .clicked()
                && !self.pool_tool(hint)
            {
                planned = Some(hint);
            }
        }
        // The viewer: the size of the picture, the clip, the time and the size.
        crate::i18n::paint_text(&painter,
            s.p(rx(1125.0), y),
            Align2::LEFT_CENTER,
            "32%",
            regular(rx(10.5)),
            theme::ICON_BRIGHT,
        );
        paint(&painter, s.p(rx(1165.0), y), Icon::Chevron, theme::ICON);
        crate::i18n::paint_text(&painter,
            s.p(rx(1093.0), y),
            Align2::CENTER_CENTER,
            "...",
            regular(rx(12.0)),
            theme::ICON,
        );
        // The drop-down that names the clip in the viewer.
        painter.circle_filled(s.p(rx(1190.0), y), 2.0, Color32::from_rgb(70, 70, 76));
        let box_rect = s.r(rx(1290.0), top + 5.0, rx(1405.0), top + BAR - 5.0);
        crate::i18n::paint_text(&painter,
            box_rect.right_center(),
            Align2::RIGHT_CENTER,
            self.source_timecode(),
            regular(rx(10.5)),
            theme::TIMECODE_TEXT,
        );
        if self
            .tool(
                ui,
                s.p(rx(1427.0), y),
                Icon::FitView,
                s.squeezed(t("Viewer size").chev(rx(21.0)), rx(1427.0), y),
            )
            .clicked()
        {
            planned = Some("Viewer zoom");
        }
        if self
            .tool(
                ui,
                s.p(rx(1480.0), y),
                Icon::Menu,
                s.squeezed(t("More").wide(34.0), rx(1480.0), y),
            )
            .clicked()
        {
            planned = Some("The viewer menu");
        }
        // The meters.
        crate::i18n::paint_text(&painter,
            s.p(rx(1507.0), y),
            Align2::LEFT_CENTER,
            "Embedded Audio",
            semibold(rx(10.5)),
            FILE_TEXT,
        );

        // The lower bar: the bins and the pool.
        let y = middle + BAR / 2.0;
        for (x, icon, hint, chev) in [
            (
                rx(18.0),
                Icon::Sidebar,
                "Show or hide the bins",
                Some(rx(24.0)),
            ),
            (rx(70.0), Icon::ChevLeft, "Back", None),
            (rx(91.0), Icon::ChevRight, "Forward", None),
            (rx(124.0), Icon::LinkChain, "Link", None),
            (rx(160.0), Icon::ImportImage, "Import media", Some(rx(23.0))),
            // Clear of the bins' divider at `STORAGE_RIGHT`.
            (rx(230.0), Icon::Sync, "Sync", None),
        ] {
            let tool = if let Some(dx) = chev {
                t(hint).chev(dx)
            } else {
                t(hint).wide(30.0)
            };
            if self
                .tool(ui, s.p(x, y), icon, s.squeezed(tool, x, y))
                .clicked()
            {
                if hint == "Import media" {
                    self.import_dialog(&self.ctx.clone());
                } else if hint == "Link" {
                    self.do_act(&self.ctx.clone(), crate::menu::Act::LinkClips);
                } else {
                    planned = Some(hint);
                }
            }
        }
        crate::i18n::paint_text(&painter,
            s.p(rx(272.0), y),
            Align2::LEFT_CENTER,
            "Master",
            semibold(rx(10.5)),
            FILE_TEXT,
        );
        for (x, icon, hint, chev) in [
            (rx(1297.0), Icon::Grid, "Thumbnails", Some(rx(21.0))),
            (rx(1349.0), Icon::Overlay, "Clip details", Some(rx(23.0))),
            (rx(1406.0), Icon::Search, "Search", None),
            (rx(1442.0), Icon::Sort, "Sort", None),
            (rx(1480.0), Icon::Menu, "More", None::<f32>),
        ] {
            let tool = if let Some(dx) = chev {
                t(hint).chev(dx)
            } else {
                t(hint).wide(36.0)
            };
            if self
                .tool(ui, s.p(x, y), icon, s.squeezed(tool, x, y))
                .clicked()
                && !self.pool_tool(hint)
            {
                planned = Some(hint);
            }
        }
        crate::i18n::paint_text(&painter,
            s.p(rx(1507.0), y),
            Align2::LEFT_CENTER,
            "Metadata",
            semibold(rx(10.5)),
            FILE_TEXT,
        );
        if let Some(what) = planned {
            self.planned(what);
        }
    }

    /// The tree of the computer's folders (desktop only).
    fn storage_tree(&mut self, ui: &mut Ui, s: &Screen, y0: f32, y1: f32) {
        let area = s.r(LEFT, y0, STORAGE_RIGHT - 1.0, y1);
        #[cfg(target_arch = "wasm32")]
        {
            crate::i18n::paint_text(&ui.painter(),
                area.center(),
                Align2::CENTER_CENTER,
                "Folders need the desktop app",
                regular(16.0),
                theme::EMPTY_SUB,
            );
            let _ = ui;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.media.roots.is_none() {
                self.media.roots = Some(find_roots());
            }
            // The rows that show: a folder, how deep it is, whether it is open.
            let mut rows: Vec<(PathBuf, String, usize, bool)> = Vec::new();
            let roots = self.media.roots.clone().unwrap_or_default();
            let mut stack: Vec<(PathBuf, String, usize)> =
                roots.into_iter().rev().map(|(p, n)| (p, n, 0)).collect();
            while let Some((path, name, depth)) = stack.pop() {
                let open = self.media.open.contains(&path);
                rows.push((path.clone(), name, depth, open));
                if open && depth < 24 {
                    let children = self
                        .media
                        .folders
                        .entry(path.clone())
                        .or_insert_with(|| read_folders(&path))
                        .clone();
                    for (child, name) in children.into_iter().rev() {
                        stack.push((child, name, depth + 1));
                    }
                }
            }
            let mut toggled: Option<PathBuf> = None;
            let mut picked: Option<PathBuf> = None;
            ui.scope_builder(UiBuilder::new().max_rect(area), |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                egui::ScrollArea::both()
                    .id_salt("storage_tree")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (path, name, depth, open) in &rows {
                            let (rect, response) = ui.allocate_exact_size(
                                vec2(area.width().max(260.0 + *depth as f32 * 16.0), ROW),
                                Sense::click(),
                            );
                            let p = ui.painter_at(rect.intersect(area));
                            let on = self.media.chosen.as_ref() == Some(path);
                            if on {
                                p.rect_filled(rect, 0.0, Color32::from_rgb(58, 57, 64));
                            } else if response.hovered() {
                                p.rect_filled(rect, 0.0, Color32::from_white_alpha(8));
                            }
                            let x = rect.left() + 20.0 + *depth as f32 * 16.0;
                            let c = pos2(x, rect.center().y);
                            // The chevron: it points down when the folder is open.
                            let arrow = if *open {
                                vec![
                                    c + vec2(-4.0, -2.0),
                                    c + vec2(0.0, 3.0),
                                    c + vec2(4.0, -2.0),
                                ]
                            } else {
                                vec![
                                    c + vec2(-2.0, -4.0),
                                    c + vec2(3.0, 0.0),
                                    c + vec2(-2.0, 4.0),
                                ]
                            };
                            p.add(egui::Shape::line(arrow, Stroke::new(1.3, theme::ICON)));
                            // A drive or a folder.
                            let icon = Rect::from_center_size(
                                pos2(x + 24.0, rect.center().y),
                                vec2(22.0, 16.0),
                            );
                            p.rect_filled(icon, 2.0, Color32::from_rgb(92, 132, 196));
                            p.rect_filled(
                                Rect::from_min_size(icon.min, vec2(22.0, 4.0)),
                                2.0,
                                Color32::from_rgb(132, 168, 224),
                            );
                            crate::i18n::paint_text(&p,
                                pos2(x + 44.0, rect.center().y),
                                Align2::LEFT_CENTER,
                                name,
                                regular(rx(10.5)),
                                FILE_TEXT,
                            );
                            if response.clicked() {
                                let on_arrow = response
                                    .interact_pointer_pos()
                                    .is_some_and(|q| q.x < x + 12.0);
                                if on_arrow {
                                    toggled = Some(path.clone());
                                } else {
                                    picked = Some(path.clone());
                                }
                            }
                            if response.double_clicked() {
                                toggled = Some(path.clone());
                            }
                        }
                    });
            });
            if let Some(path) = toggled {
                if !self.media.open.remove(&path) {
                    self.media.open.insert(path);
                }
            }
            if let Some(path) = picked {
                self.media.files = Some((path.clone(), read_files(&path)));
                self.media.chosen_file = None;
                self.media.chosen = Some(path);
            }
        }
    }

    /// The media files of the folder chosen: a click chooses one, two clicks import it.
    fn storage_files(&mut self, ui: &mut Ui, _ctx: &Context, s: &Screen, y0: f32, y1: f32) {
        let area = s.r(STORAGE_RIGHT + 1.0, y0, FILES_RIGHT - 1.0, y1);
        // Borrowed, not copied, and only the tiles in view are drawn: a folder can hold
        // thousands of files.
        let Some((folder, files)) = &self.media.files else {
            if self.project().media.is_empty() {
                empty_pool_message(
                    ui.painter(),
                    s,
                    STORAGE_RIGHT + 1.0,
                    FILES_RIGHT - 1.0,
                    y0,
                    y1,
                );
            }
            return;
        };
        if files.is_empty() {
            crate::i18n::paint_text(&ui.painter(),
                area.center(),
                Align2::CENTER_CENTER,
                format!("No media in {}", name_of(folder)),
                fitting(
                    ui.painter(),
                    &format!("No media in {}", name_of(folder)),
                    16.0,
                    area.width() - 24.0,
                ),
                theme::EMPTY_SUB,
            );
            return;
        }
        let mut import: Option<PathBuf> = None;
        let mut pick: Option<PathBuf> = None;
        ui.scope_builder(UiBuilder::new().max_rect(area.shrink(6.0)), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("storage_files")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                        for path in files {
                            let (rect, response) = ui.allocate_exact_size(
                                vec2(TILE, TILE * 0.62 + 30.0),
                                Sense::click(),
                            );
                            if !ui.is_rect_visible(rect) {
                                continue;
                            }
                            let p = ui.painter_at(rect);
                            let on = self.media.chosen_file.as_ref() == Some(path);
                            if on {
                                p.rect_filled(rect, 3.0, Color32::from_rgb(58, 57, 64));
                            } else if response.hovered() {
                                p.rect_filled(rect, 3.0, Color32::from_white_alpha(8));
                            }
                            let picture = Rect::from_min_size(
                                rect.min + vec2(4.0, 4.0),
                                vec2(TILE - 8.0, TILE * 0.62 - 4.0),
                            );
                            p.rect_filled(picture, 0.0, Color32::from_rgb(22, 22, 25));
                            let audio = crate::platform::is_audio_name(&path.to_string_lossy());
                            paint(
                                &p,
                                picture.center(),
                                if audio { Icon::PageAudio } else { Icon::Photo },
                                theme::ICON,
                            );
                            crate::i18n::paint_text(&p,
                                pos2(picture.left(), picture.bottom() + 5.0),
                                Align2::LEFT_TOP,
                                name_of(path),
                                regular(14.0),
                                FILE_TEXT,
                            );
                            if response.clicked() {
                                pick = Some(path.clone());
                            }
                            if response.double_clicked() {
                                import = Some(path.clone());
                            }
                            response.on_hover_text(format!(
                                "{}\nDouble-click to add to the media pool",
                                path.display()
                            ));
                        }
                    });
                });
        });
        if let Some(path) = pick {
            self.media.chosen_file = Some(path);
        }
        if let Some(path) = import {
            #[cfg(not(target_arch = "wasm32"))]
            self.open_path(&path.to_string_lossy(), false);
            #[cfg(target_arch = "wasm32")]
            let _ = path;
        }
    }

    /// The picture of the clip chosen, a scrub line and the transport.
    fn source_viewer(&mut self, ui: &mut Ui, s: &Screen, y0: f32, y1: f32) {
        let painter = ui.painter().clone();
        let area = s.r(QUEUE_RIGHT + 1.0, y0, VIEWER_RIGHT - 1.0, y1);
        let pic_area = Rect::from_min_max(
            pos2(area.left() + 8.0, area.top() + 8.0),
            pos2(area.right() - 8.0, area.bottom() - 108.0),
        );
        let aspect = self.project().width as f32 / self.project().height.max(1) as f32;
        let picture = self.viewer.paint(
            ui,
            pic_area.expand2(vec2(8.0, 8.0)),
            aspect,
            theme::POOL_BG,
            true,
            self.overlays,
        );
        if let Some(id) = self.selected_media {
            let ctx = self.ctx.clone();
            if let Some(texture) = self.media_thumb(&ctx, id) {
                painter.image(
                    texture,
                    picture,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
        }
        // The scrub line with the playhead.
        let line_y = pic_area.bottom() + 40.0;
        let (x0, x1) = (pic_area.left() + 4.0, pic_area.right() + 8.0);
        painter.line_segment(
            [pos2(x0, line_y), pos2(x1, line_y)],
            Stroke::new(1.5, Color32::from_rgb(86, 86, 92)),
        );
        let duration = self.project().duration().max(1);
        let share = (self.playhead as f32 / duration as f32).clamp(0.0, 1.0);
        let at = x0 + (x1 - x0) * share;
        painter.rect_filled(
            Rect::from_center_size(pos2(at, line_y), vec2(4.0, 14.0)),
            1.0,
            theme::WHITE,
        );
        let scrub = ui.interact(
            Rect::from_min_max(pos2(x0, line_y - 10.0), pos2(x1, line_y + 10.0)),
            ui.id().with("scrub"),
            Sense::click_and_drag(),
        );
        if let Some(p) = scrub
            .interact_pointer_pos()
            .filter(|_| scrub.clicked() || scrub.dragged())
        {
            let share = ((p.x - x0) / (x1 - x0)).clamp(0.0, 1.0);
            self.go_to((share * duration as f32) as u64);
        }

        // The transport.
        let y = line_y + 44.0;
        let mut planned: Option<&str> = None;
        let wide = Tool::new("").wide(46.0).color(theme::TRANSPORT_ICON);
        let (cx, step) = ((area.left() + area.right()) / 2.0, 52.0);
        let mut button = |app: &mut Self, n: f32, icon: Icon, hint: &'static str, color| {
            app.tool(
                ui,
                pos2(cx + n * step, y),
                icon,
                Tool {
                    hint,
                    color: Some(color),
                    ..wide
                },
            )
            .clicked()
        };
        if button(
            self,
            -2.5,
            Icon::ToStart,
            "Go to the start (Home)",
            theme::TRANSPORT_ICON,
        ) {
            self.playing = false;
            self.go_to(0);
        }
        if button(
            self,
            -1.5,
            Icon::PlayReverse,
            "Play backwards",
            theme::TRANSPORT_ICON,
        ) {
            planned = Some("Playing backwards");
        }
        if button(self, -0.5, Icon::Stop, "Stop", theme::TRANSPORT_ICON) {
            self.playing = false;
        }
        let color = if self.playing {
            theme::ICON_BRIGHT
        } else {
            theme::TRANSPORT_ICON
        };
        if button(
            self,
            0.5,
            Icon::Play,
            if self.playing {
                "Pause (Space)"
            } else {
                "Play (Space)"
            },
            color,
        ) {
            self.toggle_play();
        }
        if button(
            self,
            1.5,
            Icon::ToEnd,
            "Go to the end (End)",
            theme::TRANSPORT_ICON,
        ) {
            self.playing = false;
            self.go_to(u64::MAX);
        }
        let color = if self.looping {
            theme::ICON_BRIGHT
        } else {
            theme::TRANSPORT_ICON
        };
        if button(self, 2.5, Icon::Loop, "Loop", color) {
            self.looping = !self.looping;
        }
        // The size of the picture, and the sound.
        if self
            .tool(
                ui,
                pos2(area.left() + 40.0, y),
                Icon::FitView,
                Tool::new("Viewer size").wide(44.0).chev(34.0),
            )
            .clicked()
        {
            planned = Some("Viewer zoom");
        }
        if self
            .tool(
                ui,
                pos2(area.left() + 124.0, y),
                Icon::Speaker,
                Tool::new("Audio").wide(44.0),
            )
            .clicked()
        {
            planned = Some("Audio");
        }
        if let Some(what) = planned {
            self.planned(what);
        }
    }

    /// The Meters and Waveform tabs of the sound.
    fn embedded_audio(&mut self, ui: &mut Ui, s: &Screen, y0: f32, y1: f32) {
        let painter = ui.painter().clone();
        let left = VIEWER_RIGHT + 1.0;
        // The tabs.
        let mut x = rx(1667.0) - 50.0;
        for (tab, name) in [
            (MeterTab::Meters, "Meters"),
            (MeterTab::Waveform, "Waveform"),
        ] {
            let width = if tab == MeterTab::Meters {
                100.0
            } else {
                120.0
            };
            let rect = s.r(x, y0 + 4.0, x + width, y0 + 40.0);
            let response = ui.interact(rect, ui.id().with(("meter_tab", name)), Sense::click());
            if response.clicked() {
                self.media.tab = tab;
            }
            let on = self.media.tab == tab;
            crate::i18n::paint_text(&painter,
                rect.center() - vec2(0.0, 2.0),
                Align2::CENTER_CENTER,
                name,
                regular(rx(10.5)),
                if on { FILE_TEXT } else { DIM_TEXT },
            );
            if on {
                painter.rect_filled(
                    Rect::from_min_max(
                        pos2(rect.left() + 8.0, rect.bottom() - 2.0),
                        pos2(rect.right() - 8.0, rect.bottom()),
                    ),
                    0.0,
                    Color32::from_rgb(226, 72, 64),
                );
            }
            x += width + 4.0;
        }
        let body = s.r(left, y0 + 44.0, RIGHT, y1);
        match self.media.tab {
            MeterTab::Waveform => self.media_waveform(&painter, body),
            MeterTab::Meters => self.media_meters(&painter, body),
        }
    }

    fn media_meters(&self, p: &Painter, body: Rect) {
        let top = body.top() + 56.0;
        let bottom = body.bottom() - 20.0;
        let height = (bottom - top).max(20.0);
        let first = body.left() + rx(32.0);
        let step = rx(21.0);
        let width = rx(8.0);
        let dim = Color32::from_rgb(110, 110, 118);
        for (db, share) in SCALE {
            let y = top + height * share;
            crate::i18n::paint_text(&p,
                pos2(first - 22.0, y),
                Align2::RIGHT_CENTER,
                db.to_string(),
                regular(rx(7.5)),
                dim,
            );
            p.line_segment(
                [pos2(first - 17.0, y), pos2(first - 12.0, y)],
                Stroke::new(1.0, dim),
            );
        }
        let levels: Vec<f32> = (0..CHANNELS)
            .map(|n| self.track_meters.get(n).map_or(0.0, |(l, r)| l.max(*r)))
            .collect();
        let draw_bar = |cx: f32, level: f32| {
            let well =
                Rect::from_min_max(pos2(cx - width / 2.0, top), pos2(cx + width / 2.0, bottom));
            p.rect_filled(well.expand(1.0), 0.0, Color32::from_rgb(11, 11, 13));
            p.rect_filled(well, 0.0, theme::POOL_BG);
            if level < 1e-4 {
                return;
            }
            let y = (top + height * level_share(level)).clamp(top, bottom);
            let red = top + height * level_share(0.316);
            let yellow = top + height * level_share(0.1);
            for (from, to, color) in [
                (y, red.min(bottom), Color32::from_rgb(230, 70, 60)),
                (
                    y.max(red),
                    yellow.min(bottom),
                    Color32::from_rgb(235, 200, 60),
                ),
                (y.max(yellow), bottom, Color32::from_rgb(70, 190, 100)),
            ] {
                if to > from {
                    p.rect_filled(
                        Rect::from_min_max(pos2(well.left(), from), pos2(well.right(), to)),
                        0.0,
                        color,
                    );
                }
            }
        };
        for (n, level) in levels.iter().enumerate() {
            let cx = first + step * n as f32;
            crate::i18n::paint_text(&p,
                pos2(cx, top - 28.0),
                Align2::CENTER_CENTER,
                (n + 1).to_string(),
                regular(rx(8.5)),
                DIM_TEXT,
            );
            draw_bar(cx, *level);
        }
        // The two channels of what is heard.
        let monitor = body.right() - 40.0;
        crate::i18n::paint_text(&p,
            pos2(monitor, top - 28.0),
            Align2::CENTER_CENTER,
            "Monitor",
            regular(rx(8.5)),
            DIM_TEXT,
        );
        draw_bar(monitor - 11.0, self.meter.0);
        draw_bar(monitor + 11.0, self.meter.1);
    }

    /// The waveform of the clip chosen, when it has sound that is measured.
    fn media_waveform(&self, p: &Painter, body: Rect) {
        let area = body.shrink2(vec2(24.0, 40.0));
        let peaks = self.selected_media.and_then(|id| {
            let item = self.project().media(id)?;
            let path = match &item.source {
                MediaSource::Video { path } | MediaSource::Audio { path } => path.clone(),
                _ => return None,
            };
            self.sound.peaks(id, &path)
        });
        let Some(peaks) = peaks.filter(|p| !p.is_empty()) else {
            crate::i18n::paint_text(&p,
                area.center(),
                Align2::CENTER_CENTER,
                "No sound to show",
                regular(20.0),
                theme::EMPTY_SUB,
            );
            return;
        };
        let columns = (area.width() as usize).max(1);
        let mid = area.center().y;
        for c in 0..columns {
            let from = c * peaks.len() / columns;
            let to = ((c + 1) * peaks.len() / columns)
                .max(from + 1)
                .min(peaks.len());
            let peak = peaks[from..to].iter().copied().max().unwrap_or(0);
            let h = f32::from(peak) / 255.0 * area.height() / 2.0;
            let x = area.left() + c as f32;
            p.line_segment(
                [pos2(x, mid - h), pos2(x, mid + h)],
                Stroke::new(1.0, Color32::from_rgb(150, 215, 205)),
            );
        }
    }

    /// Bins and smart bins.
    fn bins_panel(&mut self, ui: &mut Ui, s: &Screen, y0: f32, y1: f32) {
        let painter = ui.painter().clone();
        let area = s.r(LEFT, y0, STORAGE_RIGHT - 1.0, y1);
        let mut planned: Option<&str> = None;
        let smart_top = (y1 - 190.0).max(y0 + 120.0);
        // Bins.
        paint(
            &painter,
            pos2(area.left() + 28.0, y0 + 22.0),
            Icon::Sidebar,
            theme::ICON,
        );
        crate::i18n::paint_text(&painter,
            pos2(area.left() + 56.0, y0 + 22.0),
            Align2::LEFT_CENTER,
            "Bins",
            regular(rx(10.5)),
            FILE_TEXT,
        );
        if self
            .plus_button(ui, pos2(area.right() - 24.0, y0 + 22.0), "New bin")
            .clicked()
        {
            planned = Some("Bins");
        }
        let master = Rect::from_min_max(
            pos2(area.left(), y0 + 44.0),
            pos2(area.right(), y0 + 44.0 + ROW + 6.0),
        );
        painter.rect_filled(master, 0.0, Color32::from_rgb(20, 20, 23));
        crate::i18n::paint_text(&painter,
            pos2(master.left() + 40.0, master.center().y),
            Align2::LEFT_CENTER,
            "Master",
            regular(rx(10.5)),
            theme::WHITE,
        );
        // Smart bins.
        painter.rect_filled(
            Rect::from_min_max(
                pos2(area.left(), smart_top),
                pos2(area.right(), smart_top + 1.0),
            ),
            0.0,
            Color32::from_rgb(14, 14, 18),
        );
        paint(
            &painter,
            pos2(area.left() + 28.0, smart_top + 26.0),
            Icon::PanelStack,
            theme::ICON,
        );
        crate::i18n::paint_text(&painter,
            pos2(area.left() + 56.0, smart_top + 26.0),
            Align2::LEFT_CENTER,
            "Smart Bins",
            regular(rx(10.5)),
            FILE_TEXT,
        );
        if self
            .plus_button(
                ui,
                pos2(area.right() - 24.0, smart_top + 26.0),
                "New smart bin",
            )
            .clicked()
        {
            planned = Some("Smart bins");
        }
        for (n, (name, branch)) in [("Keywords", false), ("Collections", true)]
            .into_iter()
            .enumerate()
        {
            let y = smart_top + 52.0 + n as f32 * (ROW + 2.0);
            let rect = Rect::from_min_max(pos2(area.left(), y), pos2(area.right(), y + ROW));
            let response = ui.interact(rect, ui.id().with(("smart", name)), Sense::click());
            if response.hovered() {
                painter.rect_filled(rect, 0.0, Color32::from_white_alpha(8));
            }
            if branch {
                paint(
                    &painter,
                    pos2(area.left() + 24.0, rect.center().y),
                    Icon::ChevRight,
                    theme::ICON,
                );
            }
            crate::i18n::paint_text(&painter,
                pos2(area.left() + 40.0, rect.center().y),
                Align2::LEFT_CENTER,
                name,
                regular(rx(10.5)),
                DIM_TEXT,
            );
            if response.clicked() {
                planned = Some(name);
            }
        }
        if let Some(what) = planned {
            self.planned(what);
        }
    }

    /// A small "+" button.
    fn plus_button(&self, ui: &mut Ui, c: egui::Pos2, hint: &'static str) -> egui::Response {
        let rect = Rect::from_center_size(c, vec2(30.0, 30.0));
        let response = ui
            .interact(rect, ui.id().with(("plus", hint)), Sense::click())
            .on_hover_text(hint);
        let color = if response.hovered() {
            theme::ICON_HOVER
        } else {
            theme::ICON
        };
        let stroke = Stroke::new(1.5, color);
        ui.painter()
            .line_segment([c - vec2(6.0, 0.0), c + vec2(6.0, 0.0)], stroke);
        ui.painter()
            .line_segment([c - vec2(0.0, 6.0), c + vec2(0.0, 6.0)], stroke);
        response
    }

    /// The clips of the project, or the empty pool.
    fn pool_area(&mut self, ui: &mut Ui, ctx: &Context, s: &Screen, y0: f32, y1: f32) {
        // The row under the bar names the bin shown.
        let crumb = s.r(STORAGE_RIGHT + 1.0, y0, VIEWER_RIGHT - 1.0, y0 + rx(30.0));
        ui.painter().rect_filled(crumb, 0.0, theme::POOL_HEADER_BG);
        ui.painter().rect_filled(
            Rect::from_min_max(
                pos2(crumb.left(), crumb.bottom() - 1.0),
                crumb.right_bottom(),
            ),
            0.0,
            Color32::from_rgb(14, 14, 18),
        );
        crate::i18n::paint_text(&ui.painter(),
            pos2(crumb.left() + 10.0, crumb.center().y),
            Align2::LEFT_CENTER,
            self.pool_bin_title(),
            regular(rx(9.0)),
            FILE_TEXT,
        );
        let area = s.r(STORAGE_RIGHT + 1.0, y0 + rx(31.0), VIEWER_RIGHT - 1.0, y1);
        if self.project().media.is_empty() {
            // Shown in the file browser above while it has no folder open,
            // in the pool when the browser lists a folder's files.
            if self.media.files.is_some() {
                empty_pool_message(
                    ui.painter(),
                    s,
                    STORAGE_RIGHT + 1.0,
                    VIEWER_RIGHT - 1.0,
                    y0 + rx(31.0),
                    y1,
                );
            }
            // Right-clicking the empty pool offers the same menu as the clips do.
            let response = ui.interact(area, ui.id().with("empty_pool"), Sense::click());
            response.context_menu(|ui| self.pool_background_menu(ui));
            return;
        }
        ui.scope_builder(UiBuilder::new().max_rect(area.shrink(8.0)), |ui| {
            self.pool_grid(ui, ctx)
        });
    }

    /// What is known about the clip chosen.
    fn metadata_panel(&mut self, ui: &mut Ui, s: &Screen, y0: f32, y1: f32) {
        let painter = ui.painter().clone();
        let area = s.r(VIEWER_RIGHT + 1.0, y0 + 1.0, RIGHT, y1);
        let item: Option<MediaItem> = self
            .selected_media
            .and_then(|id| self.project().media(id).cloned());
        let Some(item) = item else {
            crate::i18n::paint_text(&painter,
                area.center() - vec2(0.0, 6.0),
                Align2::CENTER_CENTER,
                "Nothing to inspect",
                fitting(
                    &painter,
                    "Nothing to inspect",
                    rx(27.0),
                    area.width() - 24.0,
                ),
                theme::EMPTY_TITLE,
            );
            return;
        };
        let facts = self.media_facts.get(&item.id).cloned();
        let fps = self.project().fps;
        let mut rows: Vec<(&str, String)> = vec![("Clip Name", item.name.clone())];
        rows.push((
            "Type",
            match &item.source {
                MediaSource::Video { .. } => "Video",
                MediaSource::Audio { .. } => "Audio",
                MediaSource::Image { .. } => "Still",
                _ => "Generated",
            }
            .to_string(),
        ));
        if let Some(length) = item.length {
            rows.push(("Duration", fps.timecode(length)));
        }
        // The browser knows nothing about a file beyond its picture.
        #[cfg(target_arch = "wasm32")]
        let _ = &facts;
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(f) = &facts {
            if let (Some(w), Some(h)) = (f.width, f.height) {
                rows.push(("Resolution", format!("{w} x {h}")));
            }
            if let Some((n, d)) = f.fps.filter(|(n, d)| *n > 0 && *d > 0) {
                rows.push((
                    "Frame Rate",
                    format!("{:.3} fps", f64::from(n) / f64::from(d)),
                ));
            }
            if let Some(c) = &f.video_codec {
                rows.push(("Video Codec", c.clone()));
            }
            if let Some(c) = &f.audio_codec {
                rows.push(("Audio Codec", c.clone()));
            }
            if let Some(bytes) = f.file_size {
                rows.push(("File Size", format!("{:.1} MB", bytes as f64 / 1_048_576.0)));
            }
            if !f.path.is_empty() {
                rows.push(("File Path", f.path.clone()));
            }
        }
        let mut y = area.top() + 8.0;
        for (name, value) in rows {
            let row = Rect::from_min_max(pos2(area.left(), y), pos2(area.right(), y + ROW));
            crate::i18n::paint_text(&painter,
                pos2(row.left() + 16.0, row.center().y),
                Align2::LEFT_CENTER,
                name,
                regular(rx(10.0)),
                DIM_TEXT,
            );
            let galley = painter.layout_no_wrap(value, regular(rx(10.0)), FILE_TEXT);
            let clip = painter.with_clip_rect(Rect::from_min_max(
                pos2(row.left() + 170.0, row.top()),
                row.max,
            ));
            clip.galley(
                pos2(row.left() + 170.0, row.center().y - galley.size().y / 2.0),
                galley,
                FILE_TEXT,
            );
            y += ROW;
            if y > area.bottom() {
                break;
            }
        }
    }
}
