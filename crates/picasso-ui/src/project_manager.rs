//! The project manager: the window shown before the editor when there are projects, laid out
//! like the reference screen. It lists the projects saved or opened on this computer, newest
//! first, after a card for a new project; a click selects a card and a double-click opens it.
//!
//! The list is a text file in the user's data folder (`%APPDATA%\Picasso Suite` on Windows,
//! `$XDG_DATA_HOME/picasso-suite` or `~/.local/share/picasso-suite` elsewhere), one
//! `path<TAB>name` per line, with a small picture of each project in `thumbs/`. New projects are
//! saved in `Projects/` there. The browser keeps no list, so it never shows the manager first.
//!
//! Positions are written in pixels of the 2000-pixel-wide reference and scaled to the
//! 1872-point window (`K`); the buttons at the bottom keep their distance from the bottom edge.

#![allow(clippy::too_many_arguments)]

use std::collections::HashMap;

use egui::{
    pos2, vec2, Align2, Color32, Context, Painter, Pos2, Rect, Sense, Stroke, StrokeKind,
    TextureHandle, TextureOptions, Ui,
};

use crate::app::StudioApp;
use crate::chrome::Screen;
use crate::icons::Icon;
use crate::theme;

/// Points per reference pixel.
const K: f32 = 0.936;
/// The reference screen's height, for things placed from the bottom.
const REF_H: f32 = 1332.0;
/// Projects remembered at most.
const MAX_PROJECTS: usize = 64;

const TOP_BAR: Color32 = Color32::from_rgb(28, 28, 32);
const BAR: Color32 = Color32::from_rgb(38, 38, 43);
const BG: Color32 = Color32::from_rgb(33, 33, 38);
const CARD: Color32 = Color32::from_rgb(43, 43, 48);
const CARD_EDGE: Color32 = Color32::from_rgb(58, 58, 64);
const RED: Color32 = Color32::from_rgb(228, 72, 58);
const LINE: Color32 = Color32::from_rgb(20, 20, 24);
const TEXT: Color32 = Color32::from_rgb(234, 234, 238);
const GREY: Color32 = Color32::from_rgb(150, 150, 156);
const ICON: Color32 = Color32::from_rgb(126, 126, 132);

/// A project the manager knows: where its file is and what it is called.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProjectEntry {
    pub path: String,
    pub name: String,
}

/// The manager's state.
pub(crate) struct ProjectManager {
    pub open: bool,
    entries: Vec<ProjectEntry>,
    /// 0 is the New Project card, `n` the `n - 1`th project.
    selected: usize,
    /// Pictures of the projects, by file path (`None`: there is no picture).
    thumbs: HashMap<String, Option<TextureHandle>>,
    /// The name being typed for a new project, while its dialog is open.
    naming: Option<String>,
    /// Card size from the slider, 0.6 to 1.4.
    scale: f32,
    /// Sort by name instead of by when the project was last used.
    by_name: bool,
}

impl Default for ProjectManager {
    fn default() -> Self {
        let entries = load();
        Self {
            open: !entries.is_empty(),
            entries,
            selected: 0,
            thumbs: HashMap::new(),
            naming: None,
            scale: 1.0,
            by_name: false,
        }
    }
}

impl ProjectManager {
    /// Puts a project at the front of the list (newest first) and writes the list.
    pub(crate) fn remember(&mut self, path: &str, name: &str) {
        self.entries.retain(|e| e.path != path);
        self.entries.insert(
            0,
            ProjectEntry {
                path: path.to_string(),
                name: name.to_string(),
            },
        );
        self.entries.truncate(MAX_PROJECTS);
        self.thumbs.remove(path);
        save(&self.entries);
    }

    /// Takes a project off the list; its file stays where it is.
    fn forget(&mut self, path: &str) {
        self.entries.retain(|e| e.path != path);
        save(&self.entries);
    }

    /// The projects in the order shown.
    fn shown(&self) -> Vec<ProjectEntry> {
        let mut entries = self.entries.clone();
        if self.by_name {
            entries.sort_by_key(|e| e.name.to_lowercase());
        }
        entries
    }

    fn thumb(&mut self, ctx: &Context, path: &str) -> Option<egui::TextureId> {
        if let Some(t) = self.thumbs.get(path) {
            return t.as_ref().map(TextureHandle::id);
        }
        let texture = thumb_file(path)
            .and_then(|f| std::fs::read(f).ok())
            .and_then(|bytes| picasso_render::Decoded::from_bytes(&bytes).ok())
            .map(|d| {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [d.width as usize, d.height as usize],
                    &d.rgba,
                );
                ctx.load_texture(
                    format!("project_thumb_{path}"),
                    image,
                    TextureOptions::LINEAR,
                )
            });
        let id = texture.as_ref().map(TextureHandle::id);
        self.thumbs.insert(path.to_string(), texture);
        id
    }
}

// ----- the list on disk -----

/// The folder the manager keeps its list, pictures and new projects in.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn data_dir() -> Option<std::path::PathBuf> {
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty());
    if cfg!(windows) {
        env("APPDATA").map(|d| std::path::PathBuf::from(d).join("Picasso Suite"))
    } else if let Some(d) = env("XDG_DATA_HOME") {
        Some(std::path::PathBuf::from(d).join("picasso-suite"))
    } else {
        env("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share/picasso-suite"))
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn data_dir() -> Option<std::path::PathBuf> {
    None
}

/// Reads the list from its text; lines that are not `path<TAB>name` are skipped.
pub(crate) fn parse_list(text: &str) -> Vec<ProjectEntry> {
    text.lines()
        .filter_map(|line| {
            let (path, name) = line.split_once('\t')?;
            (!path.is_empty()).then(|| ProjectEntry {
                path: path.to_string(),
                name: name.to_string(),
            })
        })
        .take(MAX_PROJECTS)
        .collect()
}

pub(crate) fn list_text(entries: &[ProjectEntry]) -> String {
    entries
        .iter()
        .map(|e| {
            format!(
                "{}\t{}\n",
                e.path.replace(['\t', '\n'], " "),
                e.name.replace(['\t', '\n'], " ")
            )
        })
        .collect()
}

/// The projects whose files are still there.
fn load() -> Vec<ProjectEntry> {
    let Some(dir) = data_dir() else {
        return Vec::new();
    };
    let text = std::fs::read_to_string(dir.join("projects.txt")).unwrap_or_default();
    parse_list(&text)
        .into_iter()
        .filter(|e| std::path::Path::new(&e.path).is_file())
        .collect()
}

fn save(entries: &[ProjectEntry]) {
    let Some(dir) = data_dir() else { return };
    if std::fs::create_dir_all(&dir).is_ok() {
        let _ = std::fs::write(dir.join("projects.txt"), list_text(entries));
    }
}

/// A stable short name for a path (FNV-1a), for its picture's file name.
pub(crate) fn path_hash(path: &str) -> u64 {
    path.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn thumb_file(path: &str) -> Option<std::path::PathBuf> {
    Some(
        data_dir()?
            .join("thumbs")
            .join(format!("{:016x}.png", path_hash(path))),
    )
}

/// Keeps a small picture of a project (its viewer frame), for its card.
pub(crate) fn save_thumb(path: &str, frame: &picasso_render::Frame) {
    let Some(file) = thumb_file(path) else { return };
    let Some(dir) = file.parent() else { return };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    if let Ok(png) = picasso_render::encode_png(frame) {
        let _ = std::fs::write(file, png);
    }
}

/// Where a new project called `name` is saved: `Projects/<name>.picasso`, numbered when that
/// file is taken.
fn new_project_path(name: &str) -> Option<String> {
    let dir = data_dir()?.join("Projects");
    std::fs::create_dir_all(&dir).ok()?;
    let clean: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_().".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let clean = clean.trim();
    let base = if clean.is_empty() {
        "Untitled Project"
    } else {
        clean
    };
    (0..1000).find_map(|n| {
        let file = if n == 0 {
            format!("{base}.picasso")
        } else {
            format!("{base} {n}.picasso")
        };
        let p = dir.join(file);
        (!p.exists()).then(|| p.to_string_lossy().into_owned())
    })
}

// ----- drawing -----

/// A drawing surface that takes reference pixels.
struct Pm<'a> {
    p: &'a Painter,
    s: &'a Screen,
}

impl Pm<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        self.s.p(x * K, y * K)
    }
    /// A point measured from the bottom of the reference.
    fn low(&self, x: f32, y: f32) -> Pos2 {
        self.s.p(x * K, self.s.h - (REF_H - y) * K)
    }
    fn rect(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(self.at(x0, y0), self.at(x1, y1))
    }
    fn label(&self, at: Pos2, align: Align2, text: &str, size: f32, c: Color32, bold: bool) {
        let font = if bold {
            theme::semibold(size)
        } else {
            theme::regular(size)
        };
        crate::i18n::paint_text(&self.p,at, align, text, font, c);
    }
    fn icon(&self, x: f32, y: f32, icon: Icon, c: Color32, size: f32) {
        crate::icons::paint_scaled(self.p, self.at(x, y), icon, c, size);
    }
}

/// The grey Picasso Suite logo drawn `size` points wide around `c`.
fn grey_logo(ctx: &Context, painter: &Painter, c: Pos2, size: f32) {
    const SIDE: usize = 160;
    let id = egui::Id::new("picasso_logo_grey");
    let texture = ctx.data_mut(|d| d.get_temp::<TextureHandle>(id));
    let texture = texture.unwrap_or_else(|| {
        let bytes: &[u8] = include_bytes!("../assets/logo-grey-160.rgba");
        let image = egui::ColorImage::from_rgba_unmultiplied([SIDE, SIDE], bytes);
        let handle = ctx.load_texture("picasso_logo_grey", image, TextureOptions::LINEAR);
        ctx.data_mut(|d| d.insert_temp(id, handle.clone()));
        handle
    });
    painter.image(
        texture.id(),
        Rect::from_center_size(c, vec2(size, size)),
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Color32::WHITE,
    );
}

/// A pill-shaped button; returns whether it was clicked.
fn pill(ui: &mut Ui, pm: &Pm, rect: Rect, text: &str, enabled: bool) -> bool {
    let r = ui.interact(rect, ui.id().with(("pm_pill", text)), Sense::click());
    let hot = enabled && r.hovered();
    if hot {
        pm.p.rect_filled(rect, rect.height() / 2.0, Color32::from_rgb(46, 46, 52));
    }
    pm.p.rect_stroke(
        rect,
        rect.height() / 2.0,
        Stroke::new(1.5, Color32::from_rgb(70, 70, 76)),
        StrokeKind::Inside,
    );
    pm.label(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        24.0,
        if enabled {
            GREY
        } else {
            Color32::from_rgb(84, 84, 90)
        },
        false,
    );
    enabled && r.clicked()
}

impl StudioApp {
    /// Opens the project manager over everything.
    pub fn show_project_manager(&mut self) {
        self.manager.open = true;
        self.manager.naming = None;
    }

    /// Goes straight to the editor.
    pub fn hide_project_manager(&mut self) {
        self.manager.open = false;
    }

    /// Remembers the open project in the manager's list, with a picture of the viewer.
    pub(crate) fn remember_project(&mut self) {
        let Some(path) = self.project_path.clone() else {
            return;
        };
        let name = std::path::Path::new(&path).file_stem().map_or_else(
            || self.project().name.clone(),
            |s| s.to_string_lossy().into_owned(),
        );
        if let Some(frame) = &self.viewer.frame {
            save_thumb(&path, frame);
        }
        self.manager.remember(&path, &name);
    }

    pub(crate) fn project_manager(&mut self, ui: &mut Ui, ctx: &Context, s: &Screen) {
        let painter = ui.painter().clone();
        let pm = Pm { p: &painter, s };
        let full = Rect::from_min_max(s.p(0.0, 0.0), pos2(s.p(1872.0, 0.0).x, s.p(0.0, s.h).y));
        painter.rect_filled(full, 0.0, BG);
        self.pm_top_bar(ui, ctx, &pm);
        self.pm_toolbar(ui, &pm);
        self.pm_cards(ui, ctx, &pm);
        self.pm_buttons(ui, ctx, &pm);
        self.pm_naming(ui, ctx, &pm);
        // A thin line round the window, as on the other pages.
        painter.rect_stroke(full, 0.0, Stroke::new(1.0, LINE), StrokeKind::Inside);
    }

    fn pm_top_bar(&mut self, ui: &mut Ui, ctx: &Context, pm: &Pm) {
        let bar = pm.rect(0.0, 0.0, 2000.0, 92.0);
        pm.p.rect_filled(bar, 0.0, TOP_BAR);
        pm.p.line_segment(
            [bar.left_bottom(), bar.right_bottom()],
            Stroke::new(1.0, LINE),
        );
        // The empty part of the bar moves the window, as a title bar does.
        let drag = ui.interact(bar, ui.id().with("pm_drag"), Sense::click_and_drag());
        if drag.drag_started() && cfg!(not(target_arch = "wasm32")) {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        // Close: on to the editor with the project that is open.
        let x = pm.at(50.0, 47.0);
        let close = ui
            .interact(
                Rect::from_center_size(x, vec2(34.0, 34.0)),
                ui.id().with("pm_close"),
                Sense::click(),
            )
            .on_hover_text("Close the project manager");
        let c = if close.hovered() { TEXT } else { GREY };
        for (a, b) in [((-7.0, -7.0), (7.0, 7.0)), ((-7.0, 7.0), (7.0, -7.0))] {
            pm.p.line_segment(
                [x + vec2(a.0, a.1), x + vec2(b.0, b.1)],
                Stroke::new(1.6, c),
            );
        }
        if close.clicked() {
            self.manager.open = false;
        }
        // The one tab: Local, where the projects are.
        pm.icon(161.0, 47.0, Icon::Drive, TEXT, 1.0);
        pm.label(
            pm.at(200.0, 47.0),
            Align2::LEFT_CENTER,
            "Local",
            24.5,
            TEXT,
            false,
        );
        pm.p.rect_filled(pm.rect(118.0, 87.0, 281.0, 91.0), 0.0, RED);
        // The name and version in the middle.
        crate::chrome::paint_logo(ctx, pm.p, pm.at(876.0, 47.0), 34.0);
        pm.label(
            pm.at(902.0, 47.0),
            Align2::LEFT_CENTER,
            &format!("Picasso Suite {}", env!("CARGO_PKG_VERSION")),
            25.5,
            TEXT,
            true,
        );
    }

    fn pm_toolbar(&mut self, ui: &mut Ui, pm: &Pm) {
        pm.p.rect_filled(pm.rect(0.0, 92.0, 2000.0, 172.0), 0.0, BAR);
        pm.p.line_segment(
            [pm.at(0.0, 172.0), pm.at(2000.0, 172.0)],
            Stroke::new(1.5, LINE),
        );
        let y = 132.0;
        pm.icon(47.0, y, Icon::Sidebar, ICON, 1.0);
        pm.label(
            pm.at(86.0, y),
            Align2::LEFT_CENTER,
            "Projects",
            21.5,
            TEXT,
            true,
        );
        pm.icon(1247.0, y, Icon::FolderPlus, ICON, 1.0);
        pm.icon(1322.0, y, Icon::ViewerFrame, ICON, 1.0);
        // The card size.
        pm.p.line_segment(
            [pm.at(1385.0, y), pm.at(1628.0, y)],
            Stroke::new(1.5, Color32::from_rgb(64, 64, 70)),
        );
        let t = (self.manager.scale - 0.6) / 0.8;
        let knob = 1385.0 + 243.0 * t;
        pm.p.circle_filled(pm.at(knob, y), 7.0, Color32::from_rgb(190, 190, 194));
        let slider = ui
            .interact(
                pm.rect(1375.0, y - 18.0, 1638.0, y + 18.0),
                ui.id().with("pm_size"),
                Sense::click_and_drag(),
            )
            .on_hover_text("Card size");
        if slider.dragged() || slider.clicked() {
            if let Some(p) = slider.interact_pointer_pos() {
                let x0 = pm.at(1385.0, y).x;
                let x1 = pm.at(1628.0, y).x;
                let f = ((p.x - x0) / (x1 - x0)).clamp(0.0, 1.0);
                self.manager.scale = 0.6 + 0.8 * f;
            }
        }
        pm.icon(
            1684.0,
            y,
            Icon::Sort,
            if self.manager.by_name { TEXT } else { ICON },
            1.0,
        );
        pm.icon(1757.0, y, Icon::Info, ICON, 1.0);
        pm.icon(1830.0, y, Icon::ThumbGrid, TEXT, 1.05);
        pm.icon(1892.0, y, Icon::ListBullets, ICON, 0.9);
        pm.icon(1962.0, y, Icon::Search, ICON, 1.0);
        let hit = |ui: &mut Ui, x: f32, hint: &str| {
            ui.interact(
                pm.rect(x - 26.0, y - 26.0, x + 26.0, y + 26.0),
                ui.id().with(("pm_tool", x as i32)),
                Sense::click(),
            )
            .on_hover_text(hint)
            .clicked()
        };
        if hit(ui, 1684.0, "Sort by name or by last used") {
            self.manager.by_name = !self.manager.by_name;
        }
        for (x, name) in [
            (47.0, "Project folders"),
            (1247.0, "New folder"),
            (1322.0, "Project preview"),
            (1757.0, "Project information"),
            (1892.0, "List view"),
            (1962.0, "Search projects"),
        ] {
            if hit(ui, x, name) {
                self.planned(name);
            }
        }
    }

    fn pm_cards(&mut self, ui: &mut Ui, ctx: &Context, pm: &Pm) {
        let entries = self.manager.shown();
        let scale = self.manager.scale;
        let (w, h, step) = (405.0 * scale, 228.0 * scale, 477.0 * scale);
        let columns = (((2000.0 - 88.0) / step).floor() as usize).max(1);
        let total = entries.len() + 1;
        let mut open: Option<usize> = None;
        let area = pm.rect(0.0, 173.0, 2000.0, 1215.0);
        let clip = pm.p.with_clip_rect(area.intersect(pm.p.clip_rect()));
        let cp = Pm { p: &clip, s: pm.s };
        for n in 0..total {
            let (col, row) = (n % columns, n / columns);
            let x0 = 88.0 + step * col as f32;
            let y0 = 242.0 + (h + 120.0 * scale) * row as f32;
            let card = cp.rect(x0, y0, x0 + w, y0 + h);
            if card.top() > area.bottom() {
                break;
            }
            let selected = self.manager.selected == n;
            cp.p.rect_filled(card, 4.0, CARD);
            if n == 0 {
                // The New Project card: the logo on a lighter square with a plus.
                let c = card.center();
                let side = 128.0 * scale * K;
                cp.p.rect_filled(
                    Rect::from_center_size(c, vec2(side, side)),
                    18.0 * scale,
                    Color32::from_rgb(52, 52, 57),
                );
                grey_logo(ctx, cp.p, c, side * 0.62);
                let plus = c + vec2(side * 0.45, side * 0.37);
                cp.p.circle_filled(plus, 22.0 * scale * K, Color32::from_rgb(52, 52, 57));
                cp.p.circle_filled(plus, 18.0 * scale * K, Color32::from_rgb(120, 120, 124));
                let arm = 10.0 * scale * K;
                for (a, b) in [
                    (vec2(-arm, 0.0), vec2(arm, 0.0)),
                    (vec2(0.0, -arm), vec2(0.0, arm)),
                ] {
                    cp.p.line_segment(
                        [plus + a, plus + b],
                        Stroke::new(3.2 * scale, Color32::from_rgb(52, 52, 57)),
                    );
                }
            } else {
                let entry = &entries[n - 1];
                match self.manager.thumb(ctx, &entry.path) {
                    Some(texture) => {
                        cp.p.rect_filled(card, 4.0, Color32::BLACK);
                        let inner = card.shrink2(vec2(0.0, card.height() * 0.06));
                        cp.p.image(
                            texture,
                            inner,
                            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }
                    None => grey_logo(ctx, cp.p, card.center(), 132.0 * scale * K),
                }
            }
            cp.p.rect_stroke(
                card,
                4.0,
                if selected {
                    Stroke::new(3.0, RED)
                } else {
                    Stroke::new(1.5, CARD_EDGE)
                },
                StrokeKind::Inside,
            );
            let name = if n == 0 {
                "New Project".to_string()
            } else {
                entries[n - 1].name.clone()
            };
            cp.label(
                cp.at(x0 + w / 2.0, y0 + h + 44.0 * scale),
                Align2::CENTER_CENTER,
                &name,
                25.5 * scale.sqrt(),
                if selected { TEXT } else { GREY },
                selected,
            );
            let r = ui.interact(
                card.union(Rect::from_center_size(
                    cp.at(x0 + w / 2.0, y0 + h + 44.0 * scale),
                    vec2(w * K, 34.0),
                )),
                ui.id().with(("pm_card", n)),
                Sense::click(),
            );
            if r.clicked() {
                self.manager.selected = n;
            }
            if r.double_clicked() {
                open = Some(n);
            }
            if n > 0 {
                let path = entries[n - 1].path.clone();
                r.context_menu(|ui| {
                    if ui.button("Open").clicked() {
                        open = Some(n);
                        ui.close();
                    }
                    if ui.button("Remove from list").clicked() {
                        self.manager.forget(&path);
                        self.manager.selected = 0;
                        ui.close();
                    }
                });
            }
        }
        if let Some(n) = open {
            self.pm_open(ctx, n);
        }
    }

    /// The projects last used, newest first, at most `n` (for File > Recent Projects).
    pub(crate) fn recent(&self, n: usize) -> Vec<ProjectEntry> {
        self.manager.entries.iter().take(n).cloned().collect()
    }

    /// Opens a project of the list by its path.
    pub(crate) fn open_recent(&mut self, ctx: &Context, entry: &ProjectEntry) {
        match std::fs::read(&entry.path) {
            Ok(bytes) => {
                self.open_bytes(&entry.name, Some(entry.path.clone()), bytes);
                self.remember_project();
            }
            Err(e) => {
                self.fail(format!("{}: {e}", entry.name));
                self.manager.forget(&entry.path);
            }
        }
        ctx.request_repaint();
    }

    /// Opens card `n`: a new project for the first card, else the project's file.
    fn pm_open(&mut self, ctx: &Context, n: usize) {
        if n == 0 {
            self.manager.naming = Some("Untitled Project".to_string());
            return;
        }
        let entries = self.manager.shown();
        let Some(entry) = entries.get(n - 1).cloned() else {
            return;
        };
        match std::fs::read(&entry.path) {
            Ok(bytes) => {
                self.open_bytes(&entry.name, Some(entry.path.clone()), bytes);
                self.manager.open = false;
                self.remember_project();
            }
            Err(e) => {
                self.fail(format!("{}: {e}", entry.name));
                self.manager.forget(&entry.path);
            }
        }
        ctx.request_repaint();
    }

    fn pm_buttons(&mut self, ui: &mut Ui, ctx: &Context, pm: &Pm) {
        let selected_project = self.manager.selected > 0;
        let row = |x0: f32, x1: f32| Rect::from_min_max(pm.low(x0, 1242.0), pm.low(x1, 1292.0));
        if pill(ui, pm, row(55.0, 283.0), "Export", selected_project) {
            let entries = self.manager.shown();
            if let Some(entry) = entries.get(self.manager.selected - 1) {
                match std::fs::read(&entry.path) {
                    Ok(bytes) => crate::platform::save_as(
                        self.inbox.clone(),
                        ctx.clone(),
                        format!("{}.picasso", entry.name),
                        ("Picasso Suite project", vec!["picasso"]),
                        bytes,
                        false,
                    ),
                    Err(e) => self.fail(format!("{}: {e}", entry.name)),
                }
            }
        }
        if pill(ui, pm, row(305.0, 532.0), "Import", true) {
            self.open_dialog(ctx);
        }
        if pill(ui, pm, row(1460.0, 1715.0), "New Project", true) {
            self.manager.naming = Some("Untitled Project".to_string());
        }
        if pill(ui, pm, row(1737.0, 1965.0), "Open", true) {
            let n = self.manager.selected;
            self.pm_open(ctx, n);
        }
    }

    /// The small dialog that asks a new project's name.
    fn pm_naming(&mut self, ui: &mut Ui, ctx: &Context, pm: &Pm) {
        let Some(mut name) = self.manager.naming.clone() else {
            return;
        };
        let full = Rect::from_min_max(pm.at(0.0, 0.0), pm.low(2000.0, REF_H));
        pm.p.rect_filled(full, 0.0, Color32::from_black_alpha(120));
        let mut create = false;
        let mut cancel = false;
        egui::Window::new("Create New Project")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_min_width(340.0);
                ui.label("Project name");
                let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(320.0));
                r.request_focus();
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    create = true;
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                    if ui.button("Create").clicked() {
                        create = true;
                    }
                });
            });
        let _ = ui;
        if cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.manager.naming = None;
            return;
        }
        if create {
            self.manager.naming = None;
            self.manager.open = false;
            let name = name.trim().to_string();
            let name = if name.is_empty() {
                "Untitled Project".to_string()
            } else {
                name
            };
            self.new_named_project(ctx, &name);
            return;
        }
        self.manager.naming = Some(name);
    }

    /// Starts a project called `name`, saved straight away in the manager's folder so that it
    /// is on the list (in the browser it stays unsaved).
    fn new_named_project(&mut self, ctx: &Context, name: &str) {
        self.replace_session(picasso_core::Project::start(name), None);
        if let Some(path) = new_project_path(name) {
            self.project_path = Some(path);
            self.save(ctx, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_survives_a_round_trip_and_skips_bad_lines() {
        let entries = vec![
            ProjectEntry {
                path: "C:\\p\\a.picasso".into(),
                name: "A".into(),
            },
            ProjectEntry {
                path: "/home/x/b.picasso".into(),
                name: "B\twith tab".into(),
            },
        ];
        let text = list_text(&entries);
        let back = parse_list(&text);
        assert_eq!(back[0], entries[0]);
        assert_eq!(back[1].name, "B with tab");
        assert!(parse_list("no tab here\n\t\n").is_empty());
    }

    #[test]
    fn path_hashes_are_stable_and_differ() {
        assert_eq!(path_hash("a"), path_hash("a"));
        assert_ne!(path_hash("a"), path_hash("b"));
        assert_eq!(path_hash(""), 0xcbf2_9ce4_8422_2325);
    }
}
