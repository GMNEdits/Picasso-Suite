//! Recording on the Audio page: R arms sound tracks, the record button starts the input device
//! and plays the timeline from the playhead; stopping writes the take to a WAV file, imports
//! it and lays it on each armed track where recording started (over what was there).

// The browser build cannot record, so it leaves most of this unused.
#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports))]

use picasso_core::{Command, MediaId, TrackId};

use crate::app::StudioApp;

/// A recording in progress.
#[derive(Default)]
pub(crate) struct Recording {
    #[cfg(not(target_arch = "wasm32"))]
    recorder: Option<picasso_audio::recorder::Recorder>,
    /// Where on the timeline it started, and the tracks it goes on.
    pub start: u64,
    pub tracks: Vec<TrackId>,
    /// What was recorded so far (interleaved stereo at `rate`).
    pub samples: Vec<f32>,
    pub rate: u32,
    /// The input level, for the record button's light.
    pub level: f32,
}

impl Recording {
    pub(crate) fn active(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        return self.recorder.is_some();
        #[cfg(target_arch = "wasm32")]
        false
    }
}

impl StudioApp {
    /// The record button: starts recording, or stops and keeps the take.
    pub(crate) fn toggle_recording(&mut self) {
        if self.recording.active() {
            self.stop_recording();
        } else {
            self.start_recording();
        }
    }

    /// Voice-over: records onto the armed sound tracks, or arms the first one (making a sound
    /// track when there is none).
    pub(crate) fn voice_over(&mut self) {
        if self.recording.active() {
            self.stop_recording();
            return;
        }
        let armed = self
            .project()
            .tracks
            .iter()
            .any(|t| self.armed.contains(&t.id));
        if !armed {
            let first = self
                .project()
                .tracks
                .iter()
                .find(|t| t.kind == picasso_core::TrackKind::Audio && !t.locked)
                .map(|t| t.id);
            let track = match first {
                Some(t) => Some(t),
                None => match self.run(Command::AddTrack {
                    kind: picasso_core::TrackKind::Audio,
                }) {
                    Some(picasso_core::Outcome::Track(t)) => Some(t),
                    _ => None,
                },
            };
            if let Some(t) = track {
                self.armed.insert(t);
            }
        }
        self.start_recording();
    }

    fn start_recording(&mut self) {
        let tracks: Vec<TrackId> = self
            .project()
            .tracks
            .iter()
            .filter(|t| self.armed.contains(&t.id) && !t.locked)
            .map(|t| t.id)
            .collect();
        if tracks.is_empty() {
            self.fail("arm a sound track first (its R button)");
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.fail("recording needs the desktop app");
        }
        #[cfg(not(target_arch = "wasm32"))]
        match picasso_audio::recorder::Recorder::start() {
            Ok(recorder) => {
                self.recording = Recording {
                    rate: recorder.sample_rate,
                    recorder: Some(recorder),
                    start: self.playhead,
                    tracks,
                    samples: Vec::new(),
                    level: 0.0,
                };
                if !self.playing {
                    self.toggle_play();
                }
                self.info("Recording... press record again to stop");
            }
            Err(e) => self.fail(format!("cannot record: {e}")),
        }
    }

    /// Gathers what the device recorded (called every frame while recording).
    pub(crate) fn tick_recording(&mut self, ctx: &egui::Context) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(recorder) = &self.recording.recorder else {
                return;
            };
            recorder.drain(&mut self.recording.samples);
            self.recording.level = recorder.take_level();
            ctx.request_repaint();
            // Playback stopping (or reaching the end) ends the take too.
            if !self.playing {
                self.stop_recording();
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = ctx;
    }

    fn stop_recording(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(recorder) = self.recording.recorder.take() else {
                return;
            };
            recorder.drain(&mut self.recording.samples);
            let dropped = recorder.dropped();
            drop(recorder);
            self.playing = false;
            let take = std::mem::take(&mut self.recording);
            if dropped > 0 {
                tracing::warn!("recording: {dropped} blocks were lost");
            }
            self.keep_take(&take.samples, take.rate, take.start, &take.tracks);
        }
    }

    /// Writes a take to a WAV file, imports it and puts it on `tracks` at `start`. Returns the
    /// new media.
    pub(crate) fn keep_take(
        &mut self,
        samples: &[f32],
        rate: u32,
        start: u64,
        tracks: &[TrackId],
    ) -> Option<MediaId> {
        if samples.len() < 2 || rate == 0 {
            self.fail("nothing was recorded");
            return None;
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (start, tracks);
            None
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(bytes) = picasso_audio::recorder::wav_file(samples, rate) else {
                self.fail("the take is too long for a WAV file");
                return None;
            };
            let dir = if cfg!(test) {
                Some(std::env::temp_dir().join("picasso_recordings_test"))
            } else {
                crate::project_manager::data_dir().map(|d| d.join("Recordings"))
            };
            let Some(dir) = dir.filter(|d| std::fs::create_dir_all(d).is_ok()) else {
                self.fail("cannot make the Recordings folder");
                return None;
            };
            let take = (1..)
                .map(|n| dir.join(format!("{} Take {n}.wav", self.project().name)))
                .find(|p| !p.exists())?;
            if let Err(e) = std::fs::write(&take, bytes) {
                self.fail(format!("cannot write the take: {e}"));
                return None;
            }
            let path = take.to_string_lossy().into_owned();
            let before = self.project().media.len();
            self.open_path_now(&path);
            if self.project().media.len() == before {
                return None;
            }
            let media = self.project().media.last().map(|m| m.id)?;
            let commands: Vec<Command> = tracks
                .iter()
                .map(|&track| Command::OverwriteClip {
                    track,
                    media,
                    start,
                    duration: None,
                    with_sound: false,
                })
                .collect();
            match self.session.dispatch_all(commands) {
                Ok(_) => {
                    let seconds = samples.len() as f32 / 2.0 / rate as f32;
                    self.info(format!(
                        "Recorded {seconds:.1} s onto {} track(s)",
                        tracks.len()
                    ));
                }
                Err(e) => self.fail(e),
            }
            Some(media)
        }
    }
}
