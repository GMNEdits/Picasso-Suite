//! More menu entries: Add Outside Node, Create Timeline Using Bin and Bounce Mix to Track.

use picasso_core::{Command, Outcome, TrackKind};

use crate::app::StudioApp;
use crate::pool::BinView;

impl StudioApp {
    /// Dynamic Trim Mode (W): picks the edit nearest the playhead; from then on, when playback
    /// stops or a scrub ends, that edit rolls to the playhead.
    pub(crate) fn toggle_dynamic_trim(&mut self) {
        if self.extra.dynamic_trim {
            self.extra.dynamic_trim = false;
            self.extra.dynamic_edit = None;
            self.info("Dynamic Trim is off");
            return;
        }
        let at = self.playhead;
        let edit = self
            .project()
            .tracks
            .iter()
            .flat_map(|t| t.clips.windows(2))
            .filter(|w| w[0].end() == w[1].start)
            .min_by_key(|w| w[1].start.abs_diff(at))
            .map(|w| (w[0].id, w[1].id));
        match edit {
            Some(edit) => {
                self.extra.dynamic_trim = true;
                self.extra.dynamic_edit = Some(edit);
                self.info("Dynamic Trim is on: play, then stop where the edit should be");
            }
            None => self.fail("there is no edit between two clips"),
        }
    }

    /// Rolls the dynamic trim edit to the playhead.
    pub(crate) fn dynamic_trim_apply(&mut self) {
        if !self.extra.dynamic_trim {
            return;
        }
        let Some((left, right)) = self.extra.dynamic_edit else {
            return;
        };
        let to = self.playhead;
        let moves = self
            .project()
            .clip(right)
            .is_some_and(|(_, c)| c.start != to);
        if moves
            && self.run(Command::RollTrim { left, right, to }).is_some()
            && self.extra.loop_trim
        {
            self.loop_around(to);
        }
    }

    /// Called every frame: playback that just stopped ends a dynamic trim.
    pub(crate) fn dynamic_trim_tick(&mut self) {
        let stopped = self.extra.was_playing && !self.playing;
        self.extra.was_playing = self.playing;
        if stopped {
            self.dynamic_trim_apply();
        }
    }

    /// Plays around frame `at` to judge a trim.
    pub(crate) fn loop_around(&mut self, at: u64) {
        self.go_to(at);
        self.play_around();
    }

    /// Add Outside Node: a node after the current one that grades everything the current node's
    /// qualifier and windows leave alone.
    pub(crate) fn add_outside_node(&mut self) {
        let Some(clip) = self.current_clip() else {
            self.fail("there is no clip at the playhead");
            return;
        };
        let n = self.node_for(clip);
        let Some(current) = self
            .project()
            .clip(clip)
            .and_then(|(_, c)| c.node(n))
            .cloned()
        else {
            return;
        };
        if current.qualifier.is_none() && current.windows.is_empty() {
            self.fail("the current node has no qualifier or window to take the outside of");
            return;
        }
        let mut grade = picasso_color::ColorGrade {
            qualifier: current.qualifier,
            windows: current.windows,
            ..Default::default()
        };
        if let Some(q) = &mut grade.qualifier {
            q.invert = !q.invert;
        }
        for w in &mut grade.windows {
            w.invert = !w.invert;
        }
        let result = self.session.dispatch_all(vec![
            Command::AddSerialNode { clip, after: n },
            Command::SetGrade {
                clip,
                grade,
                node: n + 1,
            },
        ]);
        match result {
            Ok(_) => {
                self.color_node = n + 1;
                self.info(format!("Added outside node {:02}", n + 2));
            }
            Err(e) => self.fail(e),
        }
    }

    /// Create Timeline Using Bin: a new timeline holding the media of the bin the pool shows,
    /// one after the other (pictures on the first video track, sound files on the first sound
    /// track).
    pub(crate) fn timeline_from_bin(&mut self) {
        let items = self.pool_items();
        if items.is_empty() {
            self.fail("this bin has no media");
            return;
        }
        let name = match self.pool.bin {
            BinView::Master => self.project().name.clone(),
            _ => self.pool_bin_title(),
        };
        if self.run(Command::NewTimeline { name }).is_none() {
            return;
        }
        let mut audio = self
            .project()
            .tracks
            .iter()
            .find(|t| t.kind == TrackKind::Audio)
            .map(|t| t.id);
        if audio.is_none()
            && items
                .iter()
                .any(|m| m.source.track_kind() == TrackKind::Audio)
        {
            if let Some(Outcome::Track(t)) = self.run(Command::AddTrack {
                kind: TrackKind::Audio,
            }) {
                audio = Some(t);
            }
        }
        let video = self
            .project()
            .tracks
            .iter()
            .find(|t| t.kind == TrackKind::Video)
            .map(|t| t.id);
        let (mut at_video, mut at_audio) = (0u64, 0u64);
        let mut commands = Vec::new();
        for item in &items {
            let (track, at) = match item.source.track_kind() {
                TrackKind::Video => (video, &mut at_video),
                TrackKind::Audio => (audio, &mut at_audio),
            };
            let Some(track) = track else {
                continue;
            };
            commands.push(Command::AddClip {
                track,
                media: item.id,
                start: *at,
                duration: None,
                with_sound: true,
            });
            *at += item.default_duration;
        }
        match self.session.dispatch_all(commands) {
            Ok(_) => self.info(format!("Timeline made from {} clip(s)", items.len())),
            Err(e) => self.fail(e),
        }
    }

    /// Bounce Mix to Track: mixes the whole timeline's sound into a WAV file in the background,
    /// then puts it on a new sound track.
    pub(crate) fn bounce_mix(&mut self, ctx: &egui::Context) {
        if self.project().duration() == 0 {
            self.fail("there is nothing to bounce");
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = ctx;
            self.fail("bouncing needs the desktop app");
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let dir = if cfg!(test) {
                Some(std::env::temp_dir().join("picasso_bounces_test"))
            } else {
                crate::project_manager::data_dir().map(|d| d.join("Bounces"))
            };
            let Some(dir) = dir.filter(|d| std::fs::create_dir_all(d).is_ok()) else {
                self.fail("cannot make the Bounces folder");
                return;
            };
            let Some(path) = (1..)
                .map(|n| dir.join(format!("{} Bounce {n}.wav", self.project().name)))
                .find(|p| !p.exists())
            else {
                return;
            };
            let project = self.project().clone();
            let settings = picasso_export::Settings {
                preset: picasso_export::Preset::Wav,
                path: path.clone(),
                start: 0,
                end: project.duration(),
                options: picasso_export::Options::default(),
            };
            let (inbox, ctx) = (self.inbox.clone(), ctx.clone());
            self.info("Bouncing the mix...");
            std::thread::spawn(move || {
                let cache = picasso_render::MediaCache::new();
                let result = picasso_export::render(&project, &cache, &settings, &mut |_, _| true);
                inbox.push(
                    &ctx,
                    match result {
                        Ok(()) => {
                            crate::platform::Incoming::Bounced(path.to_string_lossy().into_owned())
                        }
                        Err(e) => crate::platform::Incoming::Error(format!("bounce failed: {e}")),
                    },
                );
            });
        }
    }

    /// The bounced file is ready: imports it and lays it on a new sound track from frame 0.
    pub(crate) fn finish_bounce(&mut self, path: &str) {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = path;
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.finish_bounce_native(path);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn finish_bounce_native(&mut self, path: &str) {
        let before = self.project().media.len();
        self.open_path_now(path);
        if self.project().media.len() == before {
            return;
        }
        let Some(media) = self.project().media.last().map(|m| m.id) else {
            return;
        };
        self.place_on_new_sound_track(media);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn place_on_new_sound_track(&mut self, media: picasso_core::MediaId) {
        let Some(Outcome::Track(track)) = self.run(Command::AddTrack {
            kind: TrackKind::Audio,
        }) else {
            return;
        };
        let placed = self.run(Command::AddClip {
            track,
            media,
            start: 0,
            duration: None,
            with_sound: false,
        });
        if placed.is_some() {
            self.info("The mix is on a new sound track; mute the others to hear only it");
        }
    }
}
