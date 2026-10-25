use crate::audio::AudioPlayer;
use crate::commands::{Cmd, Page, allowed_while_typing};
use crate::keymap::Keymap;
use crate::prefs::{Preferences, PreferencesWindow};
use crate::theme::{self, *};
use crate::timeline_ui::{Action, TimelineState, Tool};
use crate::workers::{ExportJob, ImportOutcome, OverviewResult, Overviews, Preview, spawn_export, spawn_import};
use egui::{Color32, RichText, TextureHandle, TextureOptions, pos2, vec2};
use picasso_core::time::frames_to_seconds;
use picasso_core::{Edge, History, Id, Project, ProjectSettings, Rational, TrackKind};
use picasso_media::encoders::{ExportPreset, Quality, available_encoders, available_presets};
use picasso_render::ExportSettings;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::Instant;

pub const APP_NAME: &str = "Picasso Suite";

pub const FRAME_RATES: &[(i64, i64)] =
    &[(24000, 1001), (24, 1), (25, 1), (30000, 1001), (30, 1), (48, 1), (50, 1), (60000, 1001), (60, 1)];
pub const RESOLUTIONS: &[(u32, u32, &str)] = &[
    (1280, 720, "HD 720p"),
    (1920, 1080, "HD 1080p"),
    (2560, 1440, "QHD 1440p"),
    (3840, 2160, "UHD 4K"),
    (4096, 2160, "DCI 4K"),
    (1080, 1920, "Vertical 1080×1920"),
    (1080, 1080, "Square 1080"),
];

/// Render settings shared by the Deliver page and Quick Export.
pub struct RenderSettings {
    pub presets: Vec<&'static ExportPreset>,
    pub preset: usize,
    pub extension: &'static str,
    pub quality: Quality,
    pub file_name: String,
    pub location: Option<PathBuf>,
    pub in_out_only: bool,
}

pub struct QueuedJob {
    pub name: String,
    pub settings: ExportSettings,
    pub project: Project,
    pub state: JobState,
}

pub enum JobState {
    Queued,
    Rendering,
    Done,
    Failed(String),
    Cancelled,
}

/// Playback: when it started, from which frame, and at what speed (negative = reverse).
pub struct Playback {
    pub t0: Instant,
    pub f0: i64,
    pub speed: f64,
}

pub struct PicassoApp {
    pub(crate) project: Project,
    pub(crate) snapshot: Arc<Project>,
    pub(crate) version: u64,
    pub(crate) path: Option<PathBuf>,
    pub(crate) dirty: bool,
    pub(crate) history: History,
    pub(crate) selection: HashSet<Id>,
    pub(crate) media_selection: Option<Id>,
    pub(crate) playhead: i64,
    pub(crate) playing: Option<Playback>,
    /// The sound of the current playback, and which playback and edit it was started for.
    audio: Option<((Instant, i64, u64), Option<AudioPlayer>)>,
    pub(crate) source_pos: f64,
    pub(crate) marks: (Option<i64>, Option<i64>),
    pub(crate) timeline: TimelineState,
    pub(crate) preview: Preview,
    pub(crate) source_preview: Preview,
    pub(crate) texture: Option<TextureHandle>,
    pub(crate) source_texture: Option<TextureHandle>,
    pub(crate) source_snapshot: Option<(Id, Arc<Project>)>,
    pub(crate) failures: HashMap<Id, String>,
    pub(crate) overviews: Overviews,
    pub(crate) thumbs: HashMap<Id, TextureHandle>,
    pub(crate) waves: HashMap<Id, Arc<Vec<f32>>>,
    pub(crate) imports: Vec<Receiver<ImportOutcome>>,
    pub(crate) render: RenderSettings,
    pub(crate) queue: Vec<QueuedJob>,
    pub(crate) job: Option<(usize, ExportJob)>,
    pub(crate) quick_export_open: bool,
    pub(crate) settings_open: bool,
    pub(crate) about_open: bool,
    pub(crate) shortcuts_open: bool,
    pub(crate) keymap: Keymap,
    pub(crate) prefs: Preferences,
    pub(crate) prefs_window: PreferencesWindow,
    /// `PICASSO_PERF`: frame times in ms, printed after 150 frames, then quit.
    perf: Option<Vec<f64>>,
    pub(crate) status: String,
    pub(crate) tools_error: Option<String>,
    /// First line of `ffmpeg -version`, read once at startup (spawning it per frame stalls the UI).
    tools_version: String,
    /// Last window title sent to the OS; it is only re-sent when it changes.
    last_title: String,
    pub(crate) first_layout: bool,
    pub(crate) adjusting: bool,
    pub(crate) page: Page,
    pub(crate) show_pool: bool,
    pub(crate) show_bins: bool,
    pub(crate) show_inspector: bool,
    pub(crate) dual_viewer: bool,
    pub(crate) pool_list: bool,
    pub(crate) logo: Option<TextureHandle>,
    pub(crate) cut_zoom: f32,
    /// `PICASSO_SCREENSHOT`: save a picture of the window there, then quit.
    screenshot: Option<(PathBuf, u32)>,
}

impl PicassoApp {
    pub fn new(cc: &eframe::CreationContext<'_>, open: Option<PathBuf>) -> Self {
        theme::install(&cc.egui_ctx);
        let tools_check = picasso_media::tools::check_tools();
        let tools_version = tools_check.as_ref().map(|v| v.clone()).unwrap_or_else(|e| e.to_string());
        let tools_error = tools_check.err().map(|e| e.to_string());
        let presets = available_encoders().map(|e| available_presets(&e)).unwrap_or_default();
        let logo = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).ok().map(|icon| {
            let img = egui::ColorImage::from_rgba_unmultiplied([icon.width as usize, icon.height as usize], &icon.rgba);
            cc.egui_ctx.load_texture("logo", img, TextureOptions::LINEAR)
        });
        let project = Project::new("New Project", ProjectSettings::default());
        let prefs = Preferences::load();
        picasso_media::set_hardware_decoding(prefs.hardware_decode);
        let mut app = Self {
            snapshot: Arc::new(project.clone()),
            project,
            version: 0,
            path: None,
            dirty: false,
            history: History::default(),
            selection: HashSet::new(),
            media_selection: None,
            playhead: 0,
            playing: None,
            audio: None,
            source_pos: 0.0,
            marks: (None, None),
            timeline: TimelineState::default(),
            preview: Preview::spawn(cc.egui_ctx.clone()),
            source_preview: Preview::spawn(cc.egui_ctx.clone()),
            texture: None,
            source_texture: None,
            source_snapshot: None,
            failures: HashMap::new(),
            overviews: Overviews::spawn(cc.egui_ctx.clone()),
            thumbs: HashMap::new(),
            waves: HashMap::new(),
            imports: Vec::new(),
            render: RenderSettings {
                extension: presets.first().map_or("mp4", |p| p.default_extension()),
                presets,
                preset: 0,
                quality: Quality::High,
                file_name: "New Project".into(),
                location: dirs_home(),
                in_out_only: false,
            },
            queue: Vec::new(),
            job: None,
            quick_export_open: false,
            settings_open: false,
            about_open: false,
            shortcuts_open: false,
            keymap: Keymap::load(),
            prefs,
            prefs_window: PreferencesWindow::default(),
            perf: std::env::var_os("PICASSO_PERF").map(|_| Vec::new()),
            status: "Ready".into(),
            tools_error,
            tools_version,
            last_title: String::new(),
            first_layout: true,
            adjusting: false,
            page: Page::Cut,
            show_pool: true,
            show_bins: true,
            show_inspector: false,
            dual_viewer: true,
            pool_list: false,
            logo,
            cut_zoom: 239.5,
            screenshot: std::env::var_os("PICASSO_SCREENSHOT").map(|p| (PathBuf::from(p), 0)),
        };
        app.shortcuts_open = std::env::var_os("PICASSO_SHORTCUTS").is_some();
        app.about_open = std::env::var_os("PICASSO_ABOUT").is_some();
        app.prefs_window.open = std::env::var_os("PICASSO_PREFERENCES").is_some();
        if let Ok(page) = std::env::var("PICASSO_PAGE") {
            let named = [
                ("media", Page::Media),
                ("photo", Page::Photo),
                ("cut", Page::Cut),
                ("edit", Page::Edit),
                ("fusion", Page::Effects),
                ("color", Page::Color),
                ("fairlight", Page::Audio),
                ("deliver", Page::Deliver),
            ];
            if let Some((_, found)) = named.iter().find(|(n, _)| page.eq_ignore_ascii_case(n)) {
                app.page = *found;
            }
            if page.eq_ignore_ascii_case("inspector") {
                app.show_inspector = true;
            }
        }
        if let Some(path) = open {
            app.open_path(path);
        }
        app
    }

    // ------------------------------------------------------------ model helpers

    pub(crate) fn touched(&mut self) {
        self.version += 1;
        self.snapshot = Arc::new(self.project.clone());
        self.dirty = true;
        self.overviews.ensure(&self.project.media);
    }

    /// Runs an edit with an undo checkpoint; drops the checkpoint on failure.
    pub(crate) fn edit<T>(&mut self, label: &str, f: impl FnOnce(&mut Project) -> Result<T, picasso_core::EditError>) -> Option<T> {
        self.history.checkpoint(label, &self.project);
        match f(&mut self.project) {
            Ok(v) => {
                self.touched();
                self.status = label.to_owned();
                Some(v)
            }
            Err(e) => {
                self.history.discard_last();
                self.status = format!("{label}: {e}");
                None
            }
        }
    }

    fn title(&self) -> String {
        format!("{}{} — {APP_NAME}", if self.dirty { "• " } else { "" }, self.project.name)
    }

    pub(crate) fn fps(&self) -> Rational {
        self.project.settings.fps
    }

    // ------------------------------------------------------------ commands

    pub(crate) fn cmd_enabled(&self, cmd: Cmd) -> bool {
        match cmd {
            Cmd::Undo => self.history.can_undo(),
            Cmd::Redo => self.history.can_redo(),
            Cmd::DeleteSelected | Cmd::RippleDelete | Cmd::ToggleClipEnabled | Cmd::DeselectAll => !self.selection.is_empty(),
            Cmd::LinkClips => self.selection.len() >= 2,
            Cmd::UnlinkClips => self.selection.iter().any(|id| self.project.timeline.clip(*id).is_some_and(|c| c.link.is_some())),
            Cmd::QuickExport => self.project.timeline.duration() > 0,
            Cmd::Page(p) => p.built(),
            Cmd::NotYet => false,
            _ => true,
        }
    }

    pub(crate) fn cmd_checked(&self, cmd: Cmd) -> bool {
        match cmd {
            Cmd::ToggleSnapping => self.timeline.snapping,
            Cmd::ToggleMediaPool => self.show_pool,
            Cmd::ToggleBins => self.show_bins,
            Cmd::ToggleInspector => self.show_inspector,
            Cmd::ToggleDualViewer => self.dual_viewer,
            _ => false,
        }
    }

    pub(crate) fn run(&mut self, cmd: Cmd, ctx: &egui::Context) {
        if !self.cmd_enabled(cmd) {
            if let Cmd::Page(p) = cmd {
                self.status = format!("The {} page is not available yet", p.label());
            }
            return;
        }
        match cmd {
            Cmd::NewProject => self.new_project(),
            Cmd::OpenProject => self.open_dialog(),
            Cmd::SaveProject => self.save(false),
            Cmd::SaveProjectAs => self.save(true),
            Cmd::ImportMedia => self.import_dialog(ctx),
            Cmd::RelinkMedia => self.relink_dialog(),
            Cmd::ExportOtio => self.export_otio(),
            Cmd::ProjectSettings => self.settings_open = true,
            Cmd::QuickExport => self.quick_export_open = true,
            Cmd::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Cmd::Undo => self.undo(),
            Cmd::Redo => self.redo(),
            Cmd::DeleteSelected => self.delete_selection(false),
            Cmd::RippleDelete => self.delete_selection(true),
            Cmd::SelectAll => {
                self.selection = self.project.timeline.tracks.iter().flat_map(|t| &t.clips).map(|c| c.id).collect();
            }
            Cmd::DeselectAll => self.selection.clear(),
            Cmd::TrimStart | Cmd::TrimEnd => self.trim_to_playhead(cmd == Cmd::TrimStart),
            Cmd::AddVideoTrack => {
                self.edit("Add video track", |p| Ok(p.timeline.add_track(TrackKind::Video)));
            }
            Cmd::AddAudioTrack => {
                self.edit("Add audio track", |p| Ok(p.timeline.add_track(TrackKind::Audio)));
            }
            Cmd::Razor => self.blade(),
            Cmd::ToggleSnapping => {
                self.timeline.snapping ^= true;
                self.status = format!("Snapping {}", if self.timeline.snapping { "on" } else { "off" });
            }
            Cmd::SelectTool => self.timeline.tool = Tool::Select,
            Cmd::BladeTool => self.timeline.tool = Tool::Blade,
            Cmd::ZoomIn => self.timeline.zoom_by(1.25),
            Cmd::ZoomOut => self.timeline.zoom_by(0.8),
            Cmd::ZoomFit => self.timeline.zoom_to_fit(self.project.timeline.duration(), theme::DESIGN_WIDTH - 450.0),
            Cmd::ToggleClipEnabled => {
                let ids = self.selection.clone();
                self.edit("Enable / disable clip", |p| {
                    for id in &ids {
                        if let Some(c) = p.timeline.clip_mut(*id) {
                            c.enabled ^= true;
                        }
                    }
                    Ok(())
                });
            }
            Cmd::LinkClips => {
                let ids = self.selection.clone();
                self.edit("Link clips", |p| {
                    let link = picasso_core::new_id();
                    for id in &ids {
                        if let Some(c) = p.timeline.clip_mut(*id) {
                            c.link = Some(link);
                        }
                    }
                    Ok(())
                });
            }
            Cmd::UnlinkClips => {
                let ids = self.selection.clone();
                self.edit("Unlink clips", |p| {
                    for id in &ids {
                        if let Some(c) = p.timeline.clip_mut(*id) {
                            c.link = None;
                        }
                    }
                    Ok(())
                });
            }
            Cmd::MarkIn => self.marks.0 = Some(self.playhead),
            Cmd::MarkOut => self.marks.1 = Some(self.playhead),
            Cmd::ClearInOut => self.marks = (None, None),
            Cmd::ToggleMediaPool => self.show_pool ^= true,
            Cmd::ToggleBins => self.show_bins ^= true,
            Cmd::ToggleInspector => self.show_inspector ^= true,
            Cmd::ToggleDualViewer => self.dual_viewer ^= true,
            Cmd::PlayStop => {
                if self.playing.take().is_none() {
                    self.play(1.0);
                }
            }
            Cmd::PlayForward => {
                let speed = match &self.playing {
                    Some(p) if p.speed > 0.0 => (p.speed * 2.0).min(32.0),
                    _ => 1.0,
                };
                self.play(speed);
            }
            Cmd::PlayReverse => {
                let speed = match &self.playing {
                    Some(p) if p.speed < 0.0 => (p.speed * 2.0).max(-32.0),
                    _ => -1.0,
                };
                self.play(speed);
            }
            Cmd::Pause => self.playing = None,
            Cmd::ToStart => self.seek(0),
            Cmd::ToEnd => self.seek(self.project.timeline.duration()),
            Cmd::PrevFrame => self.seek(self.playhead - 1),
            Cmd::NextFrame => self.seek(self.playhead + 1),
            Cmd::PrevEdit => {
                let p = self.playhead;
                let target = self.edit_points().into_iter().filter(|e| *e < p).max().unwrap_or(0);
                self.seek(target);
            }
            Cmd::NextEdit => {
                let p = self.playhead;
                if let Some(t) = self.edit_points().into_iter().filter(|e| *e > p).min() {
                    self.seek(t);
                }
            }
            Cmd::Page(p) => self.page = p,
            Cmd::FullScreen => {
                let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!full));
            }
            Cmd::Shortcuts => self.shortcuts_open = true,
            Cmd::About => self.about_open = true,
            Cmd::Preferences => self.prefs_window.open = true,
            Cmd::NotYet => {}
        }
    }

    fn edit_points(&self) -> Vec<i64> {
        let mut points: Vec<i64> = self.project.timeline.tracks.iter().flat_map(|t| &t.clips).flat_map(|c| [c.start, c.end()]).collect();
        points.sort_unstable();
        points.dedup();
        points
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        // While a new key is being picked, presses belong to the shortcuts window.
        if self.keymap.capturing.is_some() {
            return;
        }
        let typing = ctx.egui_wants_keyboard_input();
        let mut fired = Vec::new();
        let bindings = self.keymap.bindings();
        ctx.input_mut(|i| {
            for b in bindings {
                if typing && !allowed_while_typing(b.cmd) {
                    continue;
                }
                if i.consume_key(b.mods, b.key) {
                    fired.push(b.cmd);
                }
            }
        });
        for cmd in fired {
            self.run(cmd, ctx);
        }
    }

    // ------------------------------------------------------------ file actions

    fn new_project(&mut self) {
        self.project = Project::new("New Project", ProjectSettings::default());
        self.path = None;
        self.history.clear();
        self.selection.clear();
        self.playhead = 0;
        self.marks = (None, None);
        self.touched();
        self.dirty = false;
        self.status = "New project".into();
    }

    fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Projects", &[picasso_core::io::EXTENSION, "drp"])
            .add_filter("Picasso Suite project", &[picasso_core::io::EXTENSION])
            .add_filter("DaVinci Resolve project", &["drp"])
            .pick_file()
        {
            self.open_path(path);
        }
    }

    pub(crate) fn open_path(&mut self, path: PathBuf) {
        let is_drp = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("drp"));
        let loaded = if is_drp {
            picasso_interop::drp::import(&path, &picasso_interop::relink::Relinker::for_project_file(&path))
                .map(|r| {
                    self.status = r.summary();
                    r.project
                })
                .map_err(|e| e.to_string())
        } else {
            picasso_core::io::load(&path).map_err(|e| e.to_string())
        };
        match loaded {
            Ok(p) => {
                self.project = p;
                self.path = if is_drp { None } else { Some(path.clone()) };
                self.history.clear();
                self.selection.clear();
                self.playhead = 0;
                self.marks = (None, None);
                self.touched();
                self.dirty = is_drp;
                self.first_layout = true;
                self.render.file_name = self.project.name.clone();
                self.media_selection = self.project.media.first().map(|m| m.id);
                self.source_pos = 0.0;
                if !is_drp {
                    self.status = format!("Opened {}", path.display());
                }
            }
            Err(e) => self.status = format!("Could not open {}: {e}", path.display()),
        }
    }

    fn save(&mut self, save_as: bool) {
        let path = match (&self.path, save_as) {
            (Some(p), false) => p.clone(),
            _ => match rfd::FileDialog::new()
                .add_filter("Picasso Suite project", &[picasso_core::io::EXTENSION])
                .set_file_name(format!("{}.{}", self.project.name, picasso_core::io::EXTENSION))
                .save_file()
            {
                Some(p) => p.with_extension(picasso_core::io::EXTENSION),
                None => return,
            },
        };
        match picasso_core::io::save(&self.project, &path) {
            Ok(()) => {
                self.status = format!("Saved {}", path.display());
                self.path = Some(path);
                self.dirty = false;
            }
            Err(e) => self.status = format!("Save failed: {e}"),
        }
    }

    pub(crate) fn import_dialog(&mut self, ctx: &egui::Context) {
        if let Some(paths) = rfd::FileDialog::new().set_title("Import Media").pick_files() {
            self.status = format!("Importing {} file(s)…", paths.len());
            self.imports.push(spawn_import(ctx.clone(), paths, self.fps()));
        }
    }

    pub(crate) fn import_paths(&mut self, ctx: &egui::Context, paths: Vec<PathBuf>) {
        let (projects, media): (Vec<_>, Vec<_>) = paths.into_iter().partition(|p| {
            p.extension().is_some_and(|e| e.eq_ignore_ascii_case("drp") || e.eq_ignore_ascii_case(picasso_core::io::EXTENSION))
        });
        if let Some(p) = projects.into_iter().next() {
            self.open_path(p);
        }
        if !media.is_empty() {
            self.status = format!("Importing {} file(s)…", media.len());
            self.imports.push(spawn_import(ctx.clone(), media, self.fps()));
        }
    }

    fn export_otio(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("OpenTimelineIO", &["otio"])
            .set_file_name(format!("{}.otio", self.project.timeline.name))
            .save_file()
        {
            match picasso_interop::otio::write(&self.project, &path) {
                Ok(()) => self.status = format!("Exported {} — in Resolve use File ▸ Import ▸ Timeline", path.display()),
                Err(e) => self.status = format!("OTIO export failed: {e}"),
            }
        }
    }

    fn relink_dialog(&mut self) {
        if let Some(dir) = rfd::FileDialog::new().set_title("Folder to search for missing media").pick_folder() {
            let relinker = picasso_interop::relink::Relinker::with_roots(vec![dir]);
            let mut project = self.project.clone();
            let fixed = picasso_interop::relink::relink_missing(&mut project, &relinker);
            if fixed > 0 {
                self.history.checkpoint("Relink media", &self.project);
                for m in &project.media {
                    self.overviews.forget(m.id);
                }
                self.project = project;
                self.touched();
            }
            self.status = format!("Relinked {fixed} media file(s)");
        }
    }

    fn poll_workers(&mut self, ctx: &egui::Context) {
        let mut done = Vec::new();
        self.imports.retain(|rx| match rx.try_recv() {
            Ok(outcome) => {
                done.push(outcome);
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(_) => false,
        });
        for outcome in done {
            let mut ok = Vec::new();
            let mut errors = Vec::new();
            for (path, r) in outcome {
                match r {
                    Ok(item) => ok.push(item),
                    Err(e) => errors.push(format!("{}: {e}", path.display())),
                }
            }
            if !ok.is_empty() {
                let n = ok.len();
                let first_video = ok.iter().find_map(|m| m.info.video.clone());
                self.history.checkpoint("Import media", &self.project);
                // An empty project adopts the first clip's format.
                if self.project.media.is_empty() && self.project.timeline.duration() == 0
                    && let Some(v) = first_video.filter(|v| !v.is_still) {
                        self.project.settings.width = v.width & !1;
                        self.project.settings.height = v.height & !1;
                        self.project.settings.fps = v.fps;
                        self.project.timeline.start_timecode = v.fps.nominal() * 3600;
                    }
                if self.media_selection.is_none() {
                    self.media_selection = ok.first().map(|m| m.id);
                }
                self.project.media.extend(ok);
                self.touched();
                self.status = format!("Imported {n} item(s)");
            }
            if !errors.is_empty() {
                self.status = format!("{} file(s) could not be imported — {}", errors.len(), errors.join("; "));
            }
        }
        for r in self.overviews.poll() {
            match r {
                OverviewResult::Thumbnail(id, img) => {
                    let ci = egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.data);
                    self.thumbs.insert(id, ctx.load_texture(format!("thumb-{id}"), ci, TextureOptions::LINEAR));
                }
                OverviewResult::Waveform(id, w) => {
                    self.waves.insert(id, Arc::new(w));
                }
            }
        }
        // Render queue.
        if let Some((i, job)) = &self.job
            && let Ok(r) = job.result.try_recv() {
                let i = *i;
                self.queue[i].state = match r {
                    Ok(()) => {
                        self.status = format!("Rendered {}", self.queue[i].settings.output.display());
                        JobState::Done
                    }
                    Err(e) if e.contains("cancelled") => JobState::Cancelled,
                    Err(e) => {
                        self.status = format!("Render failed: {e}");
                        JobState::Failed(e)
                    }
                };
                self.job = None;
                self.start_next_job(ctx);
            }
    }

    pub(crate) fn current_render_settings(&self) -> Option<ExportSettings> {
        let r = &self.render;
        let preset = *r.presets.get(r.preset)?;
        let dir = r.location.clone().unwrap_or_default();
        let name = if r.file_name.trim().is_empty() { "Untitled".to_owned() } else { r.file_name.trim().to_owned() };
        let range = if r.in_out_only {
            let start = self.marks.0.unwrap_or(0);
            let end = self.marks.1.unwrap_or(self.project.timeline.duration());
            (end > start).then_some((start, end))
        } else {
            None
        };
        Some(ExportSettings { preset, quality: r.quality, output: dir.join(format!("{name}.{}", r.extension)), range })
    }

    pub(crate) fn add_to_queue(&mut self) {
        if self.project.timeline.duration() == 0 {
            self.status = "Nothing to render: the timeline is empty".into();
            return;
        }
        if let Some(settings) = self.current_render_settings() {
            let name = settings.output.file_name().map_or("job".into(), |n| n.to_string_lossy().into_owned());
            self.queue.push(QueuedJob { name, settings, project: self.project.clone(), state: JobState::Queued });
            self.status = "Added to the render queue".into();
        }
    }

    pub(crate) fn start_next_job(&mut self, ctx: &egui::Context) {
        if self.job.is_some() {
            return;
        }
        if let Some(i) = self.queue.iter().position(|j| matches!(j.state, JobState::Queued)) {
            let job = &mut self.queue[i];
            job.state = JobState::Rendering;
            let settings = ExportSettings {
                preset: job.settings.preset,
                quality: job.settings.quality,
                output: job.settings.output.clone(),
                range: job.settings.range,
            };
            self.job = Some((i, spawn_export(ctx.clone(), job.project.clone(), settings)));
        }
    }

    // ------------------------------------------------------------ transport

    fn play(&mut self, speed: f64) {
        if speed > 0.0 && self.playhead >= self.project.timeline.duration() {
            self.playhead = 0;
        }
        self.playing = Some(Playback { t0: Instant::now(), f0: self.playhead, speed });
    }

    pub(crate) fn seek(&mut self, frame: i64) {
        self.playhead = frame.max(0);
        if let Some(p) = &mut self.playing {
            p.t0 = Instant::now();
            p.f0 = self.playhead;
        }
    }

    /// Starts, restarts or stops the sound to match playback. Sound plays at normal speed only.
    fn sync_audio(&mut self) {
        let want = self.playing.as_ref().filter(|p| p.speed == 1.0).map(|p| (p.t0, p.f0, self.version));
        if want == self.audio.as_ref().map(|a| a.0) {
            return;
        }
        self.audio = None;
        if let (Some(key), Some(p)) = (want, &self.playing) {
            // After an edit, the new sound starts from where the picture is now.
            let from = self.playhead.max(p.f0);
            let at = p.t0 + std::time::Duration::from_secs_f64(frames_to_seconds(from - p.f0, self.fps()));
            let player = AudioPlayer::start(&self.project, from, at)
                .inspect_err(|e| log::warn!("no sound during playback: {e}"))
                .ok();
            self.audio = Some((key, player));
        }
    }

    fn advance_playback(&mut self, ctx: &egui::Context) {
        if let Some(p) = &self.playing {
            let frame = p.f0 + (p.t0.elapsed().as_secs_f64() * self.fps().as_f64() * p.speed) as i64;
            let end = self.project.timeline.duration();
            if frame >= end && p.speed > 0.0 {
                self.playhead = end;
                self.playing = None;
            } else if frame <= 0 && p.speed < 0.0 {
                self.playhead = 0;
                self.playing = None;
            } else {
                self.playhead = frame;
                ctx.request_repaint();
            }
        }
    }

    // ------------------------------------------------------------ edits

    pub(crate) fn apply(&mut self, action: Action) {
        match action {
            Action::Seek(f) => self.seek(f),
            Action::Select { id, additive } => {
                let group = self.project.linked_group(id);
                if !additive {
                    self.selection.clear();
                }
                if additive && self.selection.contains(&id) {
                    for g in group {
                        self.selection.remove(&g);
                    }
                } else {
                    self.selection.extend(group);
                }
            }
            Action::ClearSelection => self.selection.clear(),
            Action::Move { id, delta, track } => {
                let src = self.project.timeline.find_clip(id).map(|(t, _)| t);
                let target = (Some(track) != src).then_some(track);
                self.edit("Move clip", |p| p.move_clip(id, delta, target));
            }
            Action::Trim { id, edge, delta } => {
                self.edit("Trim clip", |p| p.trim_clip(id, edge, delta));
            }
            Action::Blade { id, frame } => {
                let only: HashSet<Id> = [id].into();
                self.edit("Razor", |p| {
                    let c = p.blade(frame, Some(&only));
                    if c.is_empty() { Err(picasso_core::EditError::NoOp) } else { Ok(c) }
                });
            }
            Action::DropMedia { media, frame, track } => self.place_media(media, frame, Some(track)),
            Action::ToggleTrackEnabled(t) => {
                self.edit("Toggle track", |p| {
                    p.timeline.tracks[t].enabled ^= true;
                    Ok(())
                });
            }
            Action::ToggleTrackLocked(t) => {
                self.edit("Lock track", |p| {
                    p.timeline.tracks[t].locked ^= true;
                    Ok(())
                });
            }
        }
    }

    /// Overwrites media at `frame`; `track` is the track it was dropped on.
    pub(crate) fn place_media(&mut self, media: Id, frame: i64, track: Option<usize>) {
        self.edit("Place clip", |p| {
            let (dv, da) = p.default_tracks();
            let kind = track.and_then(|t| p.timeline.tracks.get(t)).map(|t| t.kind);
            let v = if kind == Some(TrackKind::Video) { track.unwrap_or(dv) } else { dv };
            let a = if kind == Some(TrackKind::Audio) { track.unwrap_or(da) } else { da };
            p.place_media(media, frame, Some(v), Some(a))
        });
    }

    fn blade(&mut self) {
        let at = self.playhead;
        let only = (!self.selection.is_empty()).then(|| self.selection.clone());
        let created = self.edit("Razor", |p| {
            let c = p.blade(at, only.as_ref());
            if c.is_empty() { Err(picasso_core::EditError::NoOp) } else { Ok(c) }
        });
        if let Some(c) = created
            && !self.selection.is_empty() {
                self.selection.extend(c);
            }
    }

    /// Trims the selected clips (or the clips under the playhead) so their
    /// start or end lands on the playhead.
    fn trim_to_playhead(&mut self, start: bool) {
        let at = self.playhead;
        let ids: Vec<Id> = if self.selection.is_empty() {
            self.project.timeline.tracks.iter().filter(|t| !t.locked).filter_map(|t| t.clip_at(at)).map(|c| c.id).collect()
        } else {
            self.selection.iter().copied().collect()
        };
        let label = if start { "Trim start" } else { "Trim end" };
        self.edit(label, |p| {
            let mut done: HashSet<Id> = HashSet::new();
            for id in ids {
                if done.contains(&id) {
                    continue;
                }
                let Some(c) = p.timeline.clip(id).cloned() else { continue };
                if !c.contains(at) {
                    continue;
                }
                let (edge, delta) = if start { (Edge::Start, at - c.start) } else { (Edge::End, at - c.end()) };
                p.trim_clip(id, edge, delta)?;
                done.extend(p.linked_group(id));
            }
            if done.is_empty() { Err(picasso_core::EditError::NoOp) } else { Ok(()) }
        });
    }

    fn delete_selection(&mut self, ripple: bool) {
        if self.selection.is_empty() {
            return;
        }
        let ids = std::mem::take(&mut self.selection);
        let first = ids.iter().filter_map(|id| self.project.timeline.clip(*id)).map(|c| c.start).min();
        if self.edit(if ripple { "Ripple delete" } else { "Delete" }, |p| p.delete_clips(&ids, ripple)).is_some() && ripple
            && let Some(f) = first {
                self.seek(f);
            }
    }

    fn undo(&mut self) {
        if let Some(label) = self.history.undo(&mut self.project) {
            self.touched();
            self.status = format!("Undo {label}");
            self.selection.retain(|id| self.project.timeline.clip(*id).is_some());
        }
    }

    fn redo(&mut self) {
        if let Some(label) = self.history.redo(&mut self.project) {
            self.touched();
            self.status = format!("Redo {label}");
        }
    }

    // ------------------------------------------------------------ windows

    fn quick_export_window(&mut self, ctx: &egui::Context) {
        if !self.quick_export_open {
            return;
        }
        let mut open = true;
        let mut add = false;
        egui::Window::new("Quick Export").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
            ui.set_min_width(420.0);
            self.render_settings_form(ui);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Export").clicked() {
                    add = true;
                }
                ui.weak("Renders in the background; progress shows on the Deliver page.");
            });
        });
        if add {
            self.add_to_queue();
            self.start_next_job(ctx);
            self.quick_export_open = false;
        } else {
            self.quick_export_open = open;
        }
    }

    /// Format, codec, quality and destination: shared by Deliver and Quick Export.
    pub(crate) fn render_settings_form(&mut self, ui: &mut egui::Ui) {
        let r = &mut self.render;
        if r.presets.is_empty() {
            ui.colored_label(ERROR, "No usable encoders found — is FFmpeg installed?");
            return;
        }
        egui::Grid::new(ui.next_auto_id()).num_columns(2).spacing(vec2(12.0, 10.0)).show(ui, |ui| {
            ui.label("File name");
            ui.add(egui::TextEdit::singleline(&mut r.file_name).desired_width(260.0));
            ui.end_row();
            ui.label("Location");
            ui.horizontal(|ui| {
                let shown = r.location.as_ref().map_or("Choose…".to_owned(), |p| p.display().to_string());
                ui.add(egui::Label::new(RichText::new(shown).color(TEXT_DIM)).truncate()).on_hover_text("Where the file is written");
                if ui.button("Browse").clicked()
                    && let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        r.location = Some(dir);
                    }
            });
            ui.end_row();
            ui.label("Format");
            egui::ComboBox::from_id_salt("preset").selected_text(r.presets[r.preset].label).width(260.0).show_ui(ui, |ui| {
                for (i, p) in r.presets.iter().enumerate() {
                    if ui.selectable_value(&mut r.preset, i, p.label).changed() {
                        r.extension = p.default_extension();
                    }
                }
            });
            ui.end_row();
            let preset = r.presets[r.preset];
            if preset.extensions.len() > 1 {
                ui.label("Container");
                egui::ComboBox::from_id_salt("container").selected_text(r.extension.to_uppercase()).show_ui(ui, |ui| {
                    for e in preset.extensions {
                        ui.selectable_value(&mut r.extension, e, e.to_uppercase());
                    }
                });
                ui.end_row();
            }
            if preset.video.as_ref().is_some_and(|v| v.rc.is_some()) {
                ui.label("Quality");
                egui::ComboBox::from_id_salt("quality").selected_text(r.quality.label()).show_ui(ui, |ui| {
                    for q in Quality::ALL {
                        ui.selectable_value(&mut r.quality, q, q.label());
                    }
                });
                ui.end_row();
            }
            ui.label("Render");
            ui.horizontal(|ui| {
                ui.radio_value(&mut r.in_out_only, false, "Entire timeline");
                ui.radio_value(&mut r.in_out_only, true, "In/Out range");
            });
            ui.end_row();
        });
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut open = self.settings_open;
        let mut s: ProjectSettings = self.project.settings.clone();
        let mut name = self.project.name.clone();
        egui::Window::new("Project Settings").open(&mut open).resizable(false).show(ctx, |ui| {
            egui::Grid::new("settings").num_columns(2).spacing(vec2(12.0, 10.0)).show(ui, |ui| {
                ui.label("Project name");
                ui.text_edit_singleline(&mut name);
                ui.end_row();
                ui.label("Timeline resolution");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut s.width).range(16..=16384));
                    ui.label("×");
                    ui.add(egui::DragValue::new(&mut s.height).range(16..=16384));
                    egui::ComboBox::from_id_salt("res").selected_text("Presets").show_ui(ui, |ui| {
                        for (w, h, label) in RESOLUTIONS {
                            if ui.selectable_label(s.width == *w && s.height == *h, *label).clicked() {
                                s.width = *w;
                                s.height = *h;
                            }
                        }
                    });
                });
                ui.end_row();
                ui.label("Timeline frame rate");
                egui::ComboBox::from_id_salt("fps").selected_text(s.fps.to_string()).show_ui(ui, |ui| {
                    for (n, d) in FRAME_RATES {
                        let r = Rational::new(*n, *d);
                        ui.selectable_value(&mut s.fps, r, r.to_string());
                    }
                });
                ui.end_row();
                ui.label("Audio sample rate");
                egui::ComboBox::from_id_salt("sr").selected_text(format!("{} Hz", s.sample_rate)).show_ui(ui, |ui| {
                    for r in [44_100, 48_000, 96_000] {
                        ui.selectable_value(&mut s.sample_rate, r, format!("{r} Hz"));
                    }
                });
                ui.end_row();
            });
            if s.fps != self.project.settings.fps && self.project.timeline.duration() > 0 {
                ui.colored_label(WARN, "Changing the frame rate keeps clip frame counts, so timing shifts.");
            }
        });
        s.width &= !1;
        s.height &= !1;
        if s != self.project.settings || name != self.project.name {
            self.history.checkpoint("Project settings", &self.project);
            if s.fps != self.project.settings.fps {
                self.project.timeline.start_timecode = s.fps.nominal() * 3600;
            }
            self.project.settings = s;
            self.project.name = name;
            self.touched();
        }
        self.settings_open = open;
    }

    fn info_windows(&mut self, ctx: &egui::Context) {
        let mut about = self.about_open;
        egui::Window::new("About Picasso Suite").open(&mut about).resizable(false).collapsible(false).show(ctx, |ui| {
            ui.heading(format!("Picasso Suite {}", crate::chrome::VERSION));
            ui.label("A cross-platform video editor.");
            ui.label(RichText::new(self.tools_version.as_str()).color(TEXT_DIM));
            ui.label(RichText::new("Fonts: Noto Sans (SIL Open Font License 1.1).").color(TEXT_DIM));
        });
        self.about_open = about;
        self.shortcuts_window(ctx);
        self.preferences_window(ctx);
    }

    fn preferences_window(&mut self, ctx: &egui::Context) {
        if !self.prefs_window.show(ctx, &mut self.prefs) {
            return;
        }
        self.prefs.save();
        picasso_media::set_hardware_decoding(self.prefs.hardware_decode);
        // Running viewers keep the decoders they started with; recreate them so
        // the new setting is used without restarting the app.
        self.preview = Preview::spawn(ctx.clone());
        self.source_preview = Preview::spawn(ctx.clone());
    }
}

impl PicassoApp {
    /// Waits for thumbnails and the viewer to settle, then saves the window as PNG and quits.
    fn screenshot_step(&mut self, ctx: &egui::Context) {
        let Some((path, frames)) = &mut self.screenshot else { return };
        *frames += 1;
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
        if *frames == 40 {
            let path = path.clone();
            let ctx2 = ctx.clone();
            let cb = egui::ScreenshotCallback::new(move |img| {
                let [w, h] = img.size;
                let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
                    let _ = buf.save(&path);
                }
                ctx2.send_viewport_cmd(egui::ViewportCommand::Close);
                ctx2.request_repaint();
            });
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(cb));
        }
    }
}

fn dirs_home() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).map(PathBuf::from).map(|h| {
        let videos = h.join(if cfg!(target_os = "macos") { "Movies" } else { "Videos" });
        if videos.is_dir() { videos } else { h }
    })
}

impl eframe::App for PicassoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let started = std::time::Instant::now();
        self.frame(ui);
        if let Some(times) = &mut self.perf {
            times.push(started.elapsed().as_secs_f64() * 1000.0);
            ui.ctx().request_repaint();
            if times.len() == 150 {
                let mut t = times.split_off(30);
                t.sort_by(|a, b| a.total_cmp(b));
                let avg = t.iter().sum::<f64>() / t.len() as f64;
                println!("frames={} avg={avg:.2}ms p50={:.2}ms p95={:.2}ms max={:.2}ms", t.len(), t[t.len() / 2], t[t.len() * 95 / 100], t[t.len() - 1]);
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

impl PicassoApp {
    fn frame(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        theme::fit_to_design_width(&ctx);
        self.poll_workers(&ctx);
        self.advance_playback(&ctx);
        self.sync_audio();
        self.shortcuts(&ctx);
        let title = self.title();
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        if !dropped.is_empty() {
            self.import_paths(&ctx, dropped);
        }

        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, DIVIDER);
        let title = egui::Rect::from_min_size(full.min, vec2(full.width(), MENU_H));
        let toolbar = egui::Rect::from_min_size(pos2(full.left(), title.bottom() + DIVIDER_W), vec2(full.width(), TOOLBAR_H));
        let pagebar = egui::Rect::from_min_max(pos2(full.left(), full.bottom() - PAGEBAR_H), full.max);
        let body = egui::Rect::from_min_max(pos2(full.left(), toolbar.bottom() + DIVIDER_W), pos2(full.right(), pagebar.top() - 1.0));

        self.title_row(ui, title);
        match self.page {
            Page::Cut => self.cut_page(ui, full, pagebar.top()),
            Page::Deliver => {
                self.interface_toolbar(ui, toolbar);
                self.deliver_page(ui, body);
            }
            _ => {
                self.interface_toolbar(ui, toolbar);
                self.edit_page(ui, body);
            }
        }
        self.page_bar(ui, pagebar);
        self.resize_edges(ui, full);

        self.quick_export_window(&ctx);
        self.settings_window(&ctx);
        self.info_windows(&ctx);
        self.screenshot_step(&ctx);
        if let Some(err) = &self.tools_error {
            egui::Area::new(egui::Id::unique("tools-error")).fixed_pos(pos2(full.center().x - 380.0, full.top() + 90.0)).show(&ctx, |ui| {
                egui::Frame::popup(ui.style()).fill(Color32::from_rgb(70, 24, 24)).show(ui, |ui| {
                    ui.set_max_width(760.0);
                    ui.colored_label(Color32::WHITE, err);
                });
            });
        }
    }
}
