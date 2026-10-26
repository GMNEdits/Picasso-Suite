//! The application: state shared by the pages, commands, shortcuts and the frame loop.

use std::collections::BTreeSet;

use egui::{vec2, Context, Ui};
use picasso_core::{ClipId, Command, MediaId, MediaSource, Outcome, Project, Session, TrackKind};
use picasso_render::{Decoded, MediaCache};

use crate::actions::{ClipCopy, MarkerDialog, Overlays, PlayRange};
use crate::chrome::{Screen, DESIGN_WIDTH};
use crate::color_page::ColorPage;
use crate::platform::{self, ImportedFile, Inbox, Incoming};
use crate::pool::Pool;
use crate::queue::Deliver;
use crate::theme;
use crate::timeline::{Mode, TimelineView};
use crate::viewer::{preview_size, Viewer, ViewerKey};

/// A page of the interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Media,
    Cut,
    Edit,
    Color,
    Capture,
    Effects,
    Audio,
    Deliver,
}

/// The timeline starts at this time of day, as in most editing programs (01:00:00:00).
const START_SECONDS: i64 = 3600;

/// The line at the bottom that says what just happened.
#[derive(Default)]
pub(crate) struct Status {
    pub text: String,
    pub error: bool,
}

/// What a highlighted copy of the project was made for: the project's stamp, the clip and node.
type HighlightStamp = ((u64, u64, u64), picasso_core::ClipId, usize);

pub struct StudioApp {
    pub(crate) session: Session,
    pub(crate) cache: MediaCache,
    /// Plays the project's sound and measures its level.
    pub(crate) sound: crate::sound::Sound,
    /// The level of the left and right channels, for the meter.
    pub(crate) meter: (f32, f32),
    /// The same for each track, in the order of the project's tracks.
    pub(crate) track_meters: Vec<(f32, f32)>,
    /// Asked to repaint when something finishes in the background.
    pub(crate) ctx: Context,
    /// Changes when a picture is added to `cache`.
    pub(crate) media_generation: u64,
    /// Changes when another project is opened, so renders of the old one are not reused.
    pub(crate) epoch: u64,
    pub(crate) page: Page,
    /// "Change Project Frame Rate?", while it waits for an answer.
    pub(crate) fps_prompt: Option<crate::fps_prompt::FpsPrompt>,
    pub(crate) playhead: u64,
    pub(crate) playing: bool,
    pub(crate) play_accum: f64,
    pub(crate) selection: Option<ClipId>,
    /// The other selected clips, when several are selected.
    pub(crate) selected_more: BTreeSet<ClipId>,
    /// The words of the title being typed in the Inspector, kept until the field is left.
    pub(crate) title_draft: Option<(picasso_core::MediaId, String)>,
    /// Sound tracks armed for recording (the R buttons of the Audio page).
    pub(crate) armed: BTreeSet<picasso_core::TrackId>,
    /// Set while the Audio page draws its timeline: only sound tracks, rows this tall.
    pub(crate) audio_row_h: Option<f32>,
    /// The loudness meter of the Audio page, and whether it is paused.
    pub(crate) loudness: picasso_audio::Loudness,
    pub(crate) loudness_paused: bool,
    /// What is heard apart from the project: solo, the bus, the volume and DIM.
    pub(crate) monitor: crate::monitor::Monitor,
    /// Counts the changes of `monitor`, so the sound is planned again.
    monitor_generation: u64,
    /// The project as heard, for the stamp (epoch, revision, monitor generation) it was made for.
    heard: Option<((u64, u64, u64), picasso_core::Project)>,
    /// The project with the Color page's node showing its key (Highlight), for the stamp,
    /// clip and node it was made for, with its content hash.
    pub(crate) highlighted: Option<(HighlightStamp, picasso_core::Project, u64)>,
    /// The codec of each media item as the Deliver page shows it.
    pub(crate) codec_labels: std::collections::HashMap<picasso_core::MediaId, String>,
    pub(crate) clipboard: Vec<ClipCopy>,
    pub(crate) play_speed: f64,
    pub(crate) play_range: Option<PlayRange>,
    pub(crate) overlays: Overlays,
    pub(crate) viewer_full: bool,
    pub(crate) cinema: bool,
    pub(crate) scopes_visible: bool,
    pub(crate) marker_dialog: Option<MarkerDialog>,
    pub(crate) show_about: bool,
    /// Help > Check for Updates.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) updater: crate::update::Updater,
    /// The project as last saved or opened, for Revert.
    pub(crate) saved_text: Option<String>,
    /// What is known about each imported file (size, codecs, length), kept in the project file.
    pub(crate) media_facts: std::collections::HashMap<MediaId, crate::projectfile::MediaMetadata>,
    pub(crate) pending_saved: Option<String>,
    pub(crate) selected_media: Option<MediaId>,
    pub(crate) viewer: Viewer,
    pub(crate) timeline: TimelineView,
    pub(crate) cut_detail: TimelineView,
    pub(crate) cut_overview: TimelineView,
    /// The rows the Cut page's timelines showed last frame: name, sound or not, top and height
    /// (screen points), for the labels beside them.
    pub(crate) cut_rows_overview: Vec<(String, bool, f32, f32)>,
    pub(crate) cut_rows_detail: Vec<(String, bool, f32, f32)>,
    pub(crate) looping: bool,
    /// Timeline proxy mode: the viewer shows video this many times smaller on each side
    /// (1 is off, then 2, 4 or 8). Exports always use the whole picture.
    pub(crate) proxy: u32,
    /// The colour node the Color page edits (0 is the first), kept within the clip's nodes.
    pub(crate) color_node: usize,
    /// Viewer pictures already made, in memory and on disk.
    pub(crate) render_cache: crate::render_cache::RenderCache,
    /// The graphics card and the graphics interface the window draws with, when the platform
    /// says.
    pub(crate) graphics: Option<(String, String)>,
    pub(crate) show_settings: bool,
    /// Whether the Edit page shows the inspector in place of the timeline viewer.
    pub(crate) inspector_open: bool,
    /// The window title last sent, so it is sent again only when it changes.
    window_title: String,
    pub(crate) show_projects: bool,
    /// The project manager shown before the editor.
    pub(crate) manager: crate::project_manager::ProjectManager,
    pub(crate) pool: Pool,
    pub(crate) media: crate::media::MediaPage,
    pub(crate) deliver_ui: crate::deliver::DeliverUi,
    pub(crate) color: ColorPage,
    /// The small windows and settings of the menus (Change Clip Speed, Position Lock...).
    pub(crate) extra: crate::menu_extra::Extra,
    pub(crate) multicam: crate::multicam_ui::MulticamUi,
    /// The Edit page's source viewer.
    pub(crate) source: crate::source_viewer::SourceViewer,
    /// A recording in progress on the Audio page.
    pub(crate) recording: crate::recording::Recording,
    pub(crate) deliver: Deliver,
    pub(crate) status: Status,
    pub(crate) inbox: Inbox,
    /// Where the project was opened from or saved to (desktop only).
    pub(crate) project_path: Option<String>,
    pub(crate) show_shortcuts: bool,
}

impl StudioApp {
    /// A new app with an empty project.
    pub fn new(ctx: &Context) -> Self {
        theme::install(ctx);
        // The files the last update renamed while they were in use.
        #[cfg(not(target_arch = "wasm32"))]
        crate::update::clean_up();
        #[allow(unused_mut)] // set below outside the tests
        let mut app = Self::with_project(ctx, Project::start("New Project"));
        // The tests keep the defaults: a user's own keys must not change what they check.
        #[cfg(not(test))]
        {
            app.extra.prefs = crate::prefs::Preferences::load();
            app.extra.prefs.apply();
        }
        // Which graphics cards' encoders work here, for the Deliver page.
        #[cfg(not(target_arch = "wasm32"))]
        picasso_export::check_hardware();
        app
    }

    fn with_project(ctx: &Context, project: Project) -> Self {
        Self {
            session: Session::new(project),
            cache: crate::video::new_cache(ctx),
            sound: crate::sound::Sound::new(ctx),
            meter: (0.0, 0.0),
            track_meters: Vec::new(),
            ctx: ctx.clone(),
            media_generation: 0,
            epoch: 0,
            page: Page::Cut,
            playhead: 0,
            playing: false,
            play_accum: 0.0,
            selection: None,
            selected_more: BTreeSet::new(),
            audio_row_h: None,
            armed: BTreeSet::new(),
            title_draft: None,
            loudness: picasso_audio::Loudness::new(),
            loudness_paused: false,
            monitor: crate::monitor::Monitor::default(),
            monitor_generation: 0,
            heard: None,
            highlighted: None,
            codec_labels: std::collections::HashMap::new(),
            clipboard: Vec::new(),
            play_speed: 1.0,
            play_range: None,
            overlays: Overlays::default(),
            viewer_full: false,
            cinema: false,
            scopes_visible: true,
            marker_dialog: None,
            show_about: false,
            #[cfg(not(target_arch = "wasm32"))]
            updater: crate::update::Updater::default(),
            saved_text: None,
            media_facts: std::collections::HashMap::new(),
            pending_saved: None,
            selected_media: None,
            viewer: Viewer::default(),
            timeline: TimelineView::new(Mode::Edit),
            cut_detail: TimelineView::new(Mode::Detail),
            cut_overview: TimelineView::new(Mode::Overview),
            cut_rows_overview: Vec::new(),
            cut_rows_detail: Vec::new(),
            looping: false,
            proxy: 1,
            source: crate::source_viewer::SourceViewer::default(),
            recording: crate::recording::Recording::default(),
            extra: crate::menu_extra::Extra::default(),
            fps_prompt: None,
            multicam: crate::multicam_ui::MulticamUi::default(),
            color_node: 0,
            render_cache: crate::render_cache::RenderCache::new(),
            graphics: None,
            show_settings: false,
            inspector_open: false,
            window_title: String::new(),
            show_projects: false,
            manager: crate::project_manager::ProjectManager::default(),
            pool: Pool::default(),
            media: crate::media::MediaPage::default(),
            deliver_ui: crate::deliver::DeliverUi::default(),
            color: ColorPage::default(),
            // In the browser `Deliver` has no fields; on the desktop it does.
            #[allow(clippy::default_constructed_unit_structs)]
            deliver: Deliver::default(),
            status: Status::default(),
            inbox: Inbox::default(),
            project_path: None,
            show_shortcuts: false,
        }
    }

    // ----- status -----

    pub(crate) fn info(&mut self, text: impl Into<String>) {
        self.status = Status {
            text: text.into(),
            error: false,
        };
    }

    pub(crate) fn fail(&mut self, error: impl std::fmt::Display) {
        self.status = Status {
            text: error.to_string(),
            error: true,
        };
    }

    // ----- commands -----

    /// Runs a command as one undo step; a refusal is shown in the status line.
    /// Changes what is heard (solo, the bus, the volume, DIM).
    pub(crate) fn set_monitor(&mut self, change: impl FnOnce(&mut crate::monitor::Monitor)) {
        change(&mut self.monitor);
        self.monitor_generation += 1;
    }

    pub(crate) fn run(&mut self, command: Command) -> Option<Outcome> {
        match self.session.dispatch(command) {
            Ok(outcome) => Some(outcome),
            Err(e) => {
                self.fail(e);
                None
            }
        }
    }

    /// Runs a command that is part of a drag: the whole drag is one undo step.
    pub(crate) fn run_gesture(&mut self, ctx: &Context, command: Command) {
        let dragging = ctx.input(|i| i.pointer.any_down() || i.pointer.any_released());
        let result = if dragging {
            self.session.dispatch_gesture(command)
        } else {
            self.session.dispatch(command)
        };
        if let Err(e) = result {
            self.fail(e);
        }
    }

    pub(crate) fn project(&self) -> &Project {
        self.session.project()
    }

    /// The node of clip `id` the Color page edits: the chosen one, or the clip's last node when
    /// it has fewer.
    pub(crate) fn node_for(&self, id: ClipId) -> usize {
        self.project()
            .clip(id)
            .map_or(0, |(_, c)| self.color_node.min(c.node_count() - 1))
    }

    /// Sets the timeline proxy mode (1, 2, 4 or 8 times smaller on each side).
    pub(crate) fn set_proxy(&mut self, reduce: u32) {
        self.proxy = reduce.clamp(1, 8);
        self.info(match self.proxy {
            1 => "Timeline proxy mode is off".to_string(),
            n => format!("Timeline proxy mode: 1/{n} resolution while viewing"),
        });
    }

    /// Records the graphics card the window draws with, and warns when it is a software
    /// renderer, which makes playback slow.
    pub fn set_graphics(&mut self, name: &str, api: &str, software: bool) {
        // Shown on the Deliver page and in About; only a software renderer is worth a message.
        if software {
            self.fail(format!(
                "drawing with a software renderer, {name} ({api}): playback will be slow. \
                 Update the graphics driver"
            ));
        }
        self.graphics = Some((name.to_string(), api.to_string()));
    }

    pub(crate) fn go_to(&mut self, frame: u64) {
        self.playhead = frame.min(self.project().duration());
        self.play_accum = 0.0;
    }

    pub(crate) fn toggle_play(&mut self) {
        if self.playing {
            self.playing = false;
        } else {
            if self.playhead >= self.project().duration() {
                self.playhead = 0;
            }
            self.play_accum = 0.0;
            self.play_speed = 1.0;
            self.play_range = None;
            self.playing = self.project().duration() > 0;
        }
    }

    pub(crate) fn advance(&mut self, ctx: &Context) {
        if !self.playing {
            return;
        }
        let end = self.project().duration();
        self.play_accum +=
            f64::from(ctx.input(|i| i.stable_dt)) * self.project().fps.as_f64() * self.play_speed;
        let whole = self.play_accum.trunc();
        if whole != 0.0 {
            self.play_accum -= whole;
            let next = self.playhead as i64 + whole as i64;
            let limit = self.play_range.map_or(end, |r| r.stop_at.min(end));
            if next <= 0 {
                self.playhead = 0;
                self.finish_playing(end);
            } else if next as u64 >= limit {
                self.playhead = limit;
                self.finish_playing(end);
            } else {
                self.playhead = next as u64;
            }
        }
        ctx.request_repaint();
    }

    /// What happens when playback reaches the end of the timeline or of its range.
    fn finish_playing(&mut self, end: u64) {
        if let Some(range) = self.play_range.take() {
            if let Some(back) = range.return_to {
                self.playhead = back;
            }
            self.playing = false;
        } else if self.looping {
            self.playhead = if self.play_speed < 0.0 { end } else { 0 };
        } else {
            self.playing = false;
        }
    }

    /// The clip the Color page and the inspector work on: the selected clip if the playhead is
    /// in it, else the topmost clip at the playhead.
    pub(crate) fn current_clip(&self) -> Option<ClipId> {
        let project = self.project();
        let at = self.playhead;
        if let Some((_, clip)) = self.selection.and_then(|id| project.clip(id)) {
            if clip.contains(at) {
                return Some(clip.id);
            }
        }
        project
            .tracks
            .iter()
            .rev()
            .filter(|t| t.kind == TrackKind::Video)
            .find_map(|t| Project::clip_at(t, at))
            .map(|c| c.id)
    }

    /// Puts a media item on the timeline at the playhead, on the first track where it fits.
    pub(crate) fn insert_media(&mut self, media: MediaId) {
        let Some(item) = self.project().media(media) else {
            return;
        };
        let duration = item.default_duration;
        let kind = item.source.track_kind();
        let at = self.playhead;
        let fits = |app: &Self| {
            app.project()
                .tracks
                .iter()
                .find(|t| {
                    t.kind == kind && !t.locked && app.project().can_place(t.id, at, duration, None)
                })
                .map(|t| t.id)
        };
        let mut track = fits(self);
        // The first sound on a project that has no audio track makes one.
        if track.is_none()
            && kind == TrackKind::Audio
            && !self.project().tracks.iter().any(|t| t.kind == kind)
        {
            if let Some(Outcome::Track(id)) = self.run(Command::AddTrack { kind }) {
                track = Some(id);
            }
        }
        let Some(track) = track else {
            let what = if kind == TrackKind::Audio {
                "audio"
            } else {
                "video"
            };
            self.fail(format!(
                "there is no free space at the playhead on an unlocked {what} track"
            ));
            return;
        };
        let with_sound = self.has_sound(media);
        if let Some(Outcome::Clip(id)) = self.run(Command::AddClip {
            track,
            media,
            start: at,
            duration: None,
            with_sound,
        }) {
            self.selection = Some(id);
        }
    }

    /// The track an edit at the playhead goes on: the track of the selected clip if it is of the
    /// right kind, else the first unlocked track of that kind.
    fn edit_track(&self, kind: TrackKind) -> Option<picasso_core::TrackId> {
        let project = self.project();
        self.selection
            .and_then(|id| project.clip(id))
            .map(|(t, _)| t)
            .filter(|t| t.kind == kind && !t.locked)
            .or_else(|| project.tracks.iter().find(|t| t.kind == kind && !t.locked))
            .map(|t| t.id)
    }

    /// Insert edit: puts the selected media at the playhead and pushes the rest of the track later.
    pub(crate) fn insert_edit(&mut self) {
        let Some(media) = self.selected_media else {
            self.fail("select media in the pool first");
            return;
        };
        let Some(kind) = self.project().media(media).map(|m| m.source.track_kind()) else {
            return;
        };
        let Some(track) = self.edit_track(kind) else {
            self.fail("there is no unlocked track for that media");
            return;
        };
        let start = self.playhead;
        let with_sound = self.has_sound(media);
        if let Some(Outcome::Clip(id)) = self.run(Command::InsertClip {
            track,
            media,
            start,
            duration: None,
            with_sound,
        }) {
            self.selection = Some(id);
        }
    }

    /// Overwrite edit: puts the selected media at the playhead over what is there.
    pub(crate) fn overwrite_edit(&mut self) {
        let Some(media) = self.selected_media else {
            self.fail("select media in the pool first");
            return;
        };
        let Some(kind) = self.project().media(media).map(|m| m.source.track_kind()) else {
            return;
        };
        let Some(track) = self.edit_track(kind) else {
            self.fail("there is no unlocked track for that media");
            return;
        };
        let start = self.playhead;
        let with_sound = self.has_sound(media);
        if let Some(Outcome::Clip(id)) = self.run(Command::OverwriteClip {
            track,
            media,
            start,
            duration: None,
            with_sound,
        }) {
            self.selection = Some(id);
        }
    }

    /// Fit to Fill: puts all of the selected media between the In and Out marks, over what is
    /// there, at the speed that makes it fit.
    pub(crate) fn fit_to_fill_edit(&mut self) {
        let Some(media) = self.selected_media else {
            self.fail("select media in the pool first");
            return;
        };
        let (Some(start), Some(end)) = (self.project().mark_in, self.project().mark_out) else {
            self.fail("mark an In and an Out on the timeline first");
            return;
        };
        let Some(kind) = self.project().media(media).map(|m| m.source.track_kind()) else {
            return;
        };
        let Some(track) = self.edit_track(kind) else {
            self.fail("there is no unlocked track for that media");
            return;
        };
        let with_sound = self.has_sound(media);
        if let Some(Outcome::Clip(id)) = self.run(Command::FitToFill {
            track,
            media,
            start,
            duration: end.saturating_sub(start),
            with_sound,
        }) {
            self.selection = Some(id);
        }
    }

    /// Place on Top: puts the selected media at the playhead on a free track above the clips
    /// there.
    pub(crate) fn place_on_top_edit(&mut self) {
        let Some(media) = self.selected_media else {
            self.fail("select media in the pool first");
            return;
        };
        let with_sound = self.has_sound(media);
        if let Some(Outcome::Clip(id)) = self.run(Command::PlaceOnTop {
            media,
            start: self.playhead,
            with_sound,
        }) {
            self.selection = Some(id);
        }
    }

    /// Replace edit: swaps the media of the clip at the playhead for the selected media.
    pub(crate) fn replace_edit(&mut self) {
        let Some(media) = self.selected_media else {
            self.fail("select media in the pool first");
            return;
        };
        let Some(kind) = self.project().media(media).map(|m| m.source.track_kind()) else {
            return;
        };
        // The topmost clip of the right kind under the playhead.
        let at = self.playhead;
        let clip = self
            .project()
            .tracks
            .iter()
            .rev()
            .filter(|t| t.kind == kind)
            .find_map(|t| Project::clip_at(t, at))
            .map(|c| c.id);
        let Some(clip) = clip else {
            self.fail("there is no clip at the playhead to replace");
            return;
        };
        self.run(Command::ReplaceClip { clip, media });
    }

    pub(crate) fn split_at_playhead(&mut self) {
        let at = self.playhead;
        let project = self.project();
        let selected = self
            .selection
            .and_then(|id| project.clip(id))
            .filter(|(_, c)| c.start < at && at < c.end())
            .map(|(_, c)| c.id);
        let targets: Vec<ClipId> = match selected {
            Some(id) => vec![id],
            None => project
                .tracks
                .iter()
                .filter(|t| !t.locked)
                .flat_map(|t| t.clips.iter())
                .filter(|c| c.start < at && at < c.end())
                .map(|c| c.id)
                .collect(),
        };
        if targets.is_empty() {
            self.fail("there is no clip to cut at the playhead");
            return;
        }
        for clip in targets {
            // Cutting a clip also cuts the clips linked to it, so a later target may already
            // end at the playhead.
            let still_across = self
                .project()
                .clip(clip)
                .is_some_and(|(_, c)| c.start < at && at < c.end());
            if !still_across {
                continue;
            }
            if let Some(Outcome::Split(_, right)) = self.run(Command::Split { clip, at }) {
                if self.selection == Some(clip) {
                    self.selection = Some(right);
                }
            }
        }
    }

    pub(crate) fn undo(&mut self) {
        if self.session.undo() {
            self.info("Undone");
        }
    }

    pub(crate) fn redo(&mut self) {
        if self.session.redo() {
            self.info("Redone");
        }
    }

    // ----- projects and files -----

    pub(crate) fn new_project(&mut self) {
        self.replace_session(Project::start("New Project"), None);
        self.info("New project");
    }

    pub(crate) fn load_demo(&mut self) {
        self.replace_session(Project::demo(), None);
        self.info("Loaded the demo project");
    }

    /// Timecode of a timeline position, which starts at 01:00:00:00.
    pub(crate) fn tc(&self, frame: i64) -> String {
        let fps = self.project().fps;
        fps.timecode((frame + START_SECONDS * fps.nominal() as i64).max(0) as u64)
    }

    pub(crate) fn replace_session(&mut self, project: Project, path: Option<String>) {
        self.session = Session::new(project);
        self.epoch += 1;
        self.cache = crate::video::new_cache(&self.ctx);
        self.media_generation += 1;
        self.clear_selection();
        self.selected_media = None;
        self.saved_text = None;
        self.media_facts.clear();
        self.pending_saved = None;
        self.play_range = None;
        self.play_speed = 1.0;
        self.marker_dialog = None;
        self.playhead = 0;
        self.playing = false;
        self.project_path = path;
    }

    pub(crate) fn open_text(&mut self, name: &str, path: Option<String>, text: &str) -> bool {
        match Project::from_json(text) {
            Ok(project) => {
                self.replace_session(project, path);
                self.saved_text = Some(text.to_string());
                let offline = self.reload_media();
                self.info(if offline == 0 {
                    format!("Opened {name}")
                } else {
                    format!(
                        "Opened {name}; {offline} image(s) could not be found and show as offline"
                    )
                });
                true
            }
            Err(e) => {
                self.fail(e);
                false
            }
        }
    }

    /// Opens a project file: a project database or JSON text.
    pub(crate) fn open_bytes(&mut self, name: &str, path: Option<String>, bytes: Vec<u8>) {
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".srt") || lower.ends_with(".vtt") {
            self.import_subtitles(name, &String::from_utf8_lossy(&bytes));
            return;
        }
        if lower.ends_with(".edl") {
            self.import_edl(name, &String::from_utf8_lossy(&bytes));
            return;
        }
        if platform::is_resolve_name(name) {
            self.open_resolve_bytes(name, &bytes);
            return;
        }
        match crate::projectfile::decode(bytes) {
            Ok((text, facts)) => {
                if self.open_text(name, path, &text) {
                    self.media_facts = facts.into_iter().map(|f| (MediaId(f.media), f)).collect();
                    self.manager.open = false;
                    self.remember_project();
                }
            }
            Err(e) => self.fail(format!("{name}: {e}")),
        }
    }

    /// Opens a DaVinci Resolve file (desktop only: its media is read from disk).
    fn open_resolve_bytes(&mut self, name: &str, bytes: &[u8]) {
        #[cfg(not(target_arch = "wasm32"))]
        self.open_resolve(name, bytes);
        #[cfg(target_arch = "wasm32")]
        {
            let _ = bytes;
            self.fail(format!(
                "{name}: DaVinci Resolve files need the desktop app, which can read their media"
            ));
        }
    }

    /// Reads the images of the project again from where they were imported. Returns how many
    /// could not be read (every one of them, in the browser, which cannot read paths).
    fn reload_media(&mut self) -> usize {
        let wanted: Vec<(MediaId, String)> = self
            .project()
            .media
            .iter()
            .filter_map(|m| match &m.source {
                MediaSource::Image { path } => Some((m.id, path.clone())),
                _ => None,
            })
            .collect();
        let mut missing = 0;
        for (id, path) in wanted {
            match platform::read_path(&path).and_then(|bytes| Decoded::from_bytes(&bytes).ok()) {
                Some(image) => self.cache.insert(id, image),
                None => missing += 1,
            }
        }
        self.media_generation += 1;
        missing
    }

    pub(crate) fn import_file(&mut self, file: ImportedFile) {
        // While "Change Project Frame Rate?" waits for an answer, later files wait with it.
        if let Some(prompt) = &mut self.fps_prompt {
            prompt.files.push(file);
            return;
        }
        // A dropped look-up table goes on the clip being graded.
        if platform::is_lut_name(&file.name) {
            self.apply_lut(&file.name, &file.bytes);
            return;
        }
        if platform::is_resolve_name(&file.name) {
            self.open_resolve_bytes(&file.name, &file.bytes);
            return;
        }
        if platform::is_project_name(&file.name) {
            self.open_bytes(&file.name, file.path, file.bytes);
            return;
        }
        if platform::is_video_name(&file.name) {
            self.import_path_media(file, false);
            return;
        }
        if platform::is_audio_name(&file.name) {
            self.import_path_media(file, true);
            return;
        }
        let image = match Decoded::from_bytes(&file.bytes) {
            Ok(image) => image,
            Err(e) => {
                self.fail(format!("{}: {e}", file.name));
                return;
            }
        };
        let name = file
            .name
            .rsplit_once('.')
            .map_or(file.name.as_str(), |(stem, _)| stem)
            .to_string();
        let default_duration = self.project().fps.nominal() * 5;
        let (image_w, image_h) = (image.width, image.height);
        let path = file.path.unwrap_or(file.name);
        let source = MediaSource::Image { path: path.clone() };
        if let Some(Outcome::Media(id)) = self.run(Command::AddMedia {
            name,
            source,
            length: None,
            default_duration,
        }) {
            // The facts keep the picture's own size; the browser keeps a smaller copy.
            self.cache.insert(id, platform::fit_picture(image));
            if let Some(facts) = crate::projectfile::image_facts(id.0, &path, image_w, image_h) {
                self.media_facts.insert(id, facts);
            }
            self.media_generation += 1;
            self.selected_media = Some(id);
            self.info("Imported");
        }
    }

    /// Adds a video or sound file to the media pool, first asking whether the project should take
    /// the frame rate of a video that differs (see [`crate::fps_prompt`]).
    fn import_path_media(&mut self, file: ImportedFile, sound_only: bool) {
        #[cfg(not(target_arch = "wasm32"))]
        if !sound_only {
            let fps = file.path.as_deref().and_then(crate::video::frame_rate);
            if let Some(fps) = fps.filter(|f| self.should_ask_fps(*f)) {
                self.fps_prompt = Some(crate::fps_prompt::FpsPrompt {
                    fps,
                    files: vec![file],
                });
                return;
            }
        }
        self.import_path_media_now(file, sound_only);
    }

    /// Adds a video or sound file to the media pool (desktop only; the browser has no decoder).
    pub(crate) fn import_path_media_now(&mut self, file: ImportedFile, sound_only: bool) {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = sound_only;
            self.fail(format!(
                "{}: video and sound need the desktop app (this browser build only reads images)",
                file.name
            ));
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(path) = file.path else {
                self.fail(format!("{}: the file has no path", file.name));
                return;
            };
            let imported_path = path.clone();
            let fps = self.project().fps;
            let probed = if sound_only {
                crate::video::audio_length_in_frames(&path, (fps.num, fps.den))
            } else {
                crate::video::length_in_frames(&path, (fps.num, fps.den))
            };
            let length = match probed {
                Ok(length) => length,
                Err(e) => {
                    self.fail(format!("{}: {e}", file.name));
                    return;
                }
            };
            let name = file
                .name
                .rsplit_once('.')
                .map_or(file.name.as_str(), |(stem, _)| stem)
                .to_string();
            if let Some(Outcome::Media(id)) = self.run(Command::AddMedia {
                name,
                source: if sound_only {
                    MediaSource::Audio { path }
                } else {
                    MediaSource::Video { path }
                },
                length: Some(length),
                default_duration: length,
            }) {
                if let Some(facts) =
                    crate::projectfile::file_facts(id.0, &imported_path, sound_only)
                {
                    self.media_facts.insert(id, facts);
                }
                self.media_generation += 1;
                self.selected_media = Some(id);
                self.info(if sound_only {
                    "Imported sound"
                } else {
                    "Imported video"
                });
            }
        }
    }

    /// Shows a page by name ("media", "cut", "edit", "color", "audio", "deliver"); anything else
    /// is ignored. For the command line.
    pub fn show_page(&mut self, name: &str) {
        self.page = match name {
            "media" => Page::Media,
            "cut" => Page::Cut,
            "edit" => Page::Edit,
            "color" => Page::Color,
            "audio" => Page::Audio,
            "capture" => Page::Capture,
            "effects" => Page::Effects,
            "deliver" => Page::Deliver,
            _ => return,
        };
    }

    /// Opens a project or imports media from a path given on the command line (desktop only).
    /// With `place`, the imported media is also put on the timeline at the playhead.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_path(&mut self, path: &str, place: bool) {
        let name = std::path::Path::new(path)
            .file_name()
            .map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned());
        let bytes = if platform::is_path_media(&name) {
            Ok(Vec::new())
        } else {
            std::fs::read(path)
        };
        match bytes {
            Ok(bytes) => {
                let before = self.project().media.len();
                self.import_file(ImportedFile {
                    name,
                    path: Some(path.to_string()),
                    bytes,
                });
                if place && self.project().media.len() > before {
                    if let Some(id) = self.selected_media {
                        self.insert_media(id);
                    }
                }
            }
            Err(e) => self.fail(format!("{path}: {e}")),
        }
    }

    pub(crate) fn import_dialog(&mut self, ctx: &Context) {
        platform::pick_media(self.inbox.clone(), ctx.clone());
    }

    pub(crate) fn lut_dialog(&mut self, ctx: &Context) {
        platform::pick_lut(self.inbox.clone(), ctx.clone());
    }

    pub(crate) fn open_dialog(&mut self, ctx: &Context) {
        platform::pick_project(self.inbox.clone(), ctx.clone());
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn save_to_known_path(&mut self, ctx: &Context, bytes: Vec<u8>) -> bool {
        let Some(path) = self.project_path.clone() else {
            return false;
        };
        self.info(format!("Saving {path}..."));
        platform::write_project(self.inbox.clone(), ctx.clone(), path, bytes);
        true
    }

    #[cfg(target_arch = "wasm32")]
    fn save_to_known_path(&mut self, _ctx: &Context, _bytes: Vec<u8>) -> bool {
        false
    }

    pub(crate) fn save(&mut self, ctx: &Context, ask_where: bool) {
        let text = match self.project().to_json() {
            Ok(text) => text,
            Err(e) => {
                self.fail(e);
                return;
            }
        };
        let bytes = match self.project_file_bytes(&text) {
            Ok(bytes) => bytes,
            Err(e) => {
                self.fail(e);
                return;
            }
        };
        self.pending_saved = Some(text);
        if !ask_where && self.save_to_known_path(ctx, bytes.clone()) {
            return;
        }
        let stem: String = self
            .project()
            .name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        platform::save_as(
            self.inbox.clone(),
            ctx.clone(),
            format!("{stem}.picasso"),
            (
                "Picasso Suite project",
                platform::PROJECT_EXTENSIONS.to_vec(),
            ),
            bytes,
            true,
        );
    }

    /// The bytes of a project file for the project as it is, with what is known of its media.
    pub(crate) fn project_file_bytes(&self, text: &str) -> Result<Vec<u8>, String> {
        let mut facts: Vec<_> = self
            .project()
            .media
            .iter()
            .filter_map(|m| self.media_facts.get(&m.id).cloned())
            .collect();
        facts.sort_by_key(|f| f.media);
        crate::projectfile::encode(text, &facts)
    }

    pub(crate) fn process_inbox(&mut self) {
        for item in self.inbox.take() {
            match item {
                Incoming::Media(files) => files.into_iter().for_each(|f| self.import_file(f)),
                Incoming::Project { name, path, bytes } => self.open_bytes(&name, path, bytes),
                Incoming::Saved {
                    name,
                    path,
                    project,
                } => {
                    if project {
                        self.project_path = path;
                        self.session.mark_saved();
                        self.saved_text = self.pending_saved.take();
                        self.remember_project();
                        self.info(format!("Saved {name}"));
                    } else {
                        self.info(format!("Exported {name}"));
                    }
                }
                Incoming::Folder(path) | Incoming::ExportFile(path) => self.enqueue_export(path),
                Incoming::BinFolder(path) => self.import_folder_as_bin(&path),
                Incoming::Bounced(path) => self.finish_bounce(&path),
                Incoming::Rendered { clip, path } => self.finish_render_in_place(clip, &path),
                Incoming::Lut { name, bytes } => self.apply_lut(&name, &bytes),
                Incoming::Error(e) => self.fail(e),
            }
        }
    }

    fn take_dropped_files(&mut self, ctx: &Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped.is_empty() {
            platform::queue_dropped(dropped, self.inbox.clone(), ctx.clone());
        }
    }

    // ----- the frame -----

    /// Draws the whole interface into `ui`, the root of the window.
    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.process_inbox();
        self.dynamic_trim_tick();
        self.take_dropped_files(&ctx);
        self.deliver.poll(&mut self.status, &ctx);
        self.run_shortcuts(&ctx);
        self.advance(&ctx);
        let normal_speed = self.playing && (self.play_speed - 1.0).abs() < 1e-9;
        // The sound plays the project as the monitor changes it.
        let stamp = (self.epoch, self.session.revision(), self.monitor_generation);
        if !self.monitor.is_plain() && self.heard.as_ref().is_none_or(|(s, _)| *s != stamp) {
            self.heard = Some((stamp, self.monitor.heard(self.session.project())));
        }
        let heard = match &self.heard {
            Some((s, p)) if *s == stamp && !self.monitor.is_plain() => p,
            _ => self.session.project(),
        };
        self.sound.sync(
            heard,
            (
                self.epoch,
                self.session
                    .revision()
                    .wrapping_add(self.monitor_generation << 40),
            ),
            self.playhead,
            normal_speed,
        );
        let (blocks, true_peak) = self.sound.loudness();
        if !self.loudness_paused {
            for power in blocks {
                self.loudness.push_block(power);
            }
            self.loudness.push_peak(true_peak);
        }
        self.meter = self.sound.meter();
        self.track_meters = self.sound.track_meters();
        if self.meter != (0.0, 0.0) || self.track_meters.iter().any(|m| *m != (0.0, 0.0)) {
            ctx.request_repaint();
        }
        if self
            .selection
            .is_some_and(|id| self.project().clip(id).is_none())
        {
            self.selection = None;
        }
        self.prune_selection();
        self.playhead = self.playhead.min(self.project().duration());
        self.update_viewer(&ctx);

        let title = format!(
            "{}{} - Picasso Suite",
            self.project().name,
            if self.session.is_dirty() { " *" } else { "" }
        );
        // Only when it changes, and not in the browser, which cannot set it (and would warn on
        // every frame).
        if cfg!(not(target_arch = "wasm32")) && title != self.window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }

        self.frame_ui(ui, &ctx);
        self.action_windows(&ctx);
        // A drag ends when the pointer is up; the next change is a new undo step.
        if !ctx.input(|i| i.pointer.any_down()) {
            self.session.end_gesture();
        }
    }

    /// Lays out the window: the rows measured from the reference, then the page in the middle.
    fn frame_ui(&mut self, ui: &mut Ui, ctx: &Context) {
        let rect = ui.max_rect();
        // Scale the interface so the window is always DESIGN_WIDTH points wide; the layout
        // below uses fixed positions of that width.
        let ppp = ctx.pixels_per_point();
        let target = (rect.width() * ppp / DESIGN_WIDTH).clamp(0.4, 4.0);
        if (target / ppp - 1.0).abs() > 0.003 {
            ctx.set_pixels_per_point(target);
        }
        let s = Screen {
            o: rect.min,
            h: rect.height(),
        };
        ui.painter().rect_filled(rect, 0.0, theme::MENU_BG);

        // The project manager comes before the editor.
        if self.manager.open {
            self.project_manager(ui, ctx, &s);
            return;
        }

        // The viewer alone, over the whole window.
        if self.viewer_full {
            let aspect = self.project().width as f32 / self.project().height.max(1) as f32;
            ui.painter().rect_filled(rect, 0.0, egui::Color32::BLACK);
            self.viewer
                .paint(ui, rect, aspect, egui::Color32::BLACK, true, self.overlays);
            let hint = format!(
                "{}   Esc or a click to leave",
                self.tc(self.playhead as i64)
            );
            ui.painter().text(
                rect.left_bottom() + vec2(14.0, -12.0),
                egui::Align2::LEFT_BOTTOM,
                hint,
                theme::regular(15.0),
                egui::Color32::from_white_alpha(70),
            );
            if ui
                .interact(rect, ui.id().with("viewer_full"), egui::Sense::click())
                .clicked()
            {
                self.do_act(ctx, crate::menu::Act::LeaveViewer);
            }
            return;
        }

        // The other pages are still laid out below a 50-point toolbar; Resolve's is thinner.
        let shift = 92.0 - (40.0 + 40.0 * crate::chrome::R);
        let base = s;
        let s = if self.page == Page::Cut {
            s
        } else {
            Screen {
                o: egui::pos2(s.o.x, s.o.y - shift),
                h: s.h + shift,
            }
        };
        if self.page == Page::Cut {
            self.cut_page(ui, ctx, &s);
        } else if self.page == Page::Deliver {
            self.deliver_scaled(ctx, &base);
        } else if self.page == Page::Media {
            self.media_scaled(ctx, &base);
        } else if self.page == Page::Capture {
            self.capture_page(ui, ctx, &s);
        } else if self.page == Page::Audio {
            self.fairlight_page(ui, ctx, &s);
        } else if self.page == Page::Effects {
            self.effects_page(ui, ctx, &s);
        } else if self.page == Page::Color {
            self.color_page(ui, ctx, &s);
        } else if self.page == Page::Edit {
            self.edit_page(ui, ctx, &s);
        }
        let s = Screen {
            o: egui::pos2(
                s.o.x,
                s.o.y + if self.page == Page::Cut { 0.0 } else { shift },
            ),
            h: s.h - if self.page == Page::Cut { 0.0 } else { shift },
        };
        self.toolbar1(ui, ctx, &s);
        self.menu_row(ui, ctx, &s);
        self.page_bar(ui, &s);
        // The window's thin black border.
        ui.painter()
            .rect_filled(s.r(0.0, 0.0, 3.0, s.h), 0.0, theme::WINDOW_BORDER);
        ui.painter().rect_filled(
            s.r(DESIGN_WIDTH - 1.0, 0.0, DESIGN_WIDTH, s.h),
            0.0,
            theme::WINDOW_BORDER,
        );
        #[cfg(not(target_arch = "wasm32"))]
        self.resize_edges(ui, ctx, &s);
        self.windows(ctx);
    }

    fn update_viewer(&mut self, ctx: &Context) {
        let (width, height) = preview_size(self.project());
        // Videos are decoded at the size the viewer shows them (half of a 1080p picture for the
        // 960-point preview), times the proxy: the decoder thread makes the picture small, and
        // drawing it costs a copy instead of a scale on every frame.
        let fit = (self.project().width / width.max(1)).max(1);
        let reduce = (fit * self.proxy.max(1)).clamp(1, 8);
        // A proxy renders a smaller picture too; the viewer scales it up to the panel.
        let (width, height) = (
            (width / self.proxy.max(1)).max(16),
            (height / self.proxy.max(1)).max(16),
        );
        let key = ViewerKey {
            epoch: self.epoch,
            revision: self.session.revision(),
            media_generation: self.media_generation,
            video: self.cache.video_generation(),
            frame: self.playhead,
            width,
            height,
            alpha: self.overlays.checkerboard,
            highlight: 0,
        };
        let cache = self.cache.with_reduction(reduce);
        let stamp = (self.epoch, self.session.revision(), self.media_generation);
        self.update_highlight(stamp);
        self.follow_playhead();
        self.tick_recording(ctx);
        let Self {
            viewer,
            session,
            render_cache,
            highlighted,
            ..
        } = self;
        let plain = render_cache.content(stamp, session.project());
        let (project, content) = match highlighted {
            Some((_, project, hash)) => (&*project, *hash),
            None => (session.project(), plain),
        };
        let key = ViewerKey {
            highlight: if content == plain { 0 } else { content },
            ..key
        };
        let frame_key = picasso_render::FrameKey {
            content,
            frame: key.frame,
            width: key.width,
            height: key.height,
            reduce,
            alpha: key.alpha,
        };
        viewer.ensure(ctx, project, &cache, key, render_cache, frame_key);
    }

    /// Makes (or forgets) the copy of the project whose edited node shows its key, when the
    /// Color page's Highlight is on and the node grades only part of the picture.
    pub(crate) fn update_highlight(&mut self, stamp: (u64, u64, u64)) {
        let wanted = (self.page == Page::Color && self.color.highlight)
            .then(|| self.current_clip())
            .flatten()
            .map(|clip| (stamp, clip, self.node_for(clip)))
            .filter(|(_, clip, node)| {
                self.project()
                    .clip(*clip)
                    .and_then(|(_, c)| c.node(*node))
                    .is_some_and(|g| g.enabled && g.is_secondary())
            });
        let Some(wanted) = wanted else {
            self.highlighted = None;
            return;
        };
        if self
            .highlighted
            .as_ref()
            .is_some_and(|(s, _, _)| *s == wanted)
        {
            return;
        }
        let (_, clip, node) = wanted;
        let mut project = self.project().clone();
        if let Some(c) = project.clip_mut(clip) {
            // The node shows its key; the nodes after it are left out.
            let count = c.node_count();
            for n in 0..count {
                if let Some(g) = c.node_mut(n) {
                    if n == node {
                        g.highlight = true;
                    } else if n > node {
                        g.enabled = false;
                    }
                }
            }
        }
        let json = project.to_json().unwrap_or_default();
        let plain = self.render_cache.content(stamp, self.session.project());
        let hash = picasso_render::content_hash(&[json.as_bytes(), &plain.to_le_bytes()]);
        self.highlighted = Some((wanted, project, hash));
    }

    // ----- shared pieces of the pages -----
}
