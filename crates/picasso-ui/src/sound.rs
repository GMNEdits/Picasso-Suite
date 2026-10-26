//! The sound of the project: plays the mix while the timeline plays, measures the level for the
//! meter and gives the waveforms the timeline draws. The browser build has no sound yet.

use std::sync::Arc;

use picasso_core::{MediaId, Project};

/// Plays the project's sound, in step with the playhead.
pub(crate) struct Sound {
    #[cfg(not(target_arch = "wasm32"))]
    engine: native::Engine,
}

impl Sound {
    /// `ctx` is asked to repaint when a waveform has been measured.
    pub(crate) fn new(ctx: &egui::Context) -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = ctx;
            Self {}
        }
        #[cfg(not(target_arch = "wasm32"))]
        Self {
            engine: native::Engine::new(ctx.clone()),
        }
    }

    /// Called every frame. `play_edit_key` changes when the project was edited; `playing` is true
    /// while the timeline plays forward at normal speed (the only time sound is played).
    pub(crate) fn sync(
        &mut self,
        project: &Project,
        edit_key: (u64, u64),
        playhead: u64,
        playing: bool,
    ) {
        #[cfg(not(target_arch = "wasm32"))]
        self.engine.sync(project, edit_key, playhead, playing);
        #[cfg(target_arch = "wasm32")]
        let _ = (project, edit_key, playhead, playing);
    }

    /// The level of the left and right channels since the last call, 0.0 to 1.0 and beyond.
    pub(crate) fn meter(&mut self) -> (f32, f32) {
        #[cfg(not(target_arch = "wasm32"))]
        return self.engine.meter();
        #[cfg(target_arch = "wasm32")]
        (0.0, 0.0)
    }

    /// The loudness blocks played since the last call, and the highest true peak among them.
    pub(crate) fn loudness(&mut self) -> (Vec<f64>, f32) {
        #[cfg(not(target_arch = "wasm32"))]
        return self.engine.loudness();
        #[cfg(target_arch = "wasm32")]
        (Vec::new(), 0.0)
    }

    /// The level of each track's sound (left, right) since the last call, in the order of the
    /// project's tracks.
    pub(crate) fn track_meters(&mut self) -> Vec<(f32, f32)> {
        #[cfg(not(target_arch = "wasm32"))]
        return self.engine.track_meters();
        #[cfg(target_arch = "wasm32")]
        Vec::new()
    }

    /// The waveform of a video's sound: one byte per fiftieth of a second. `None` while it is
    /// being measured, or when the file has no sound.
    pub(crate) fn peaks(&self, media: MediaId, path: &str) -> Option<Arc<Vec<u8>>> {
        #[cfg(not(target_arch = "wasm32"))]
        return self.engine.peaks(media, path);
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (media, path);
            None
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::sync::Arc;

    use picasso_audio::{plan, AudioOutput, AudioPlan, AudioPool, Peaks, SAMPLE_RATE};
    use picasso_core::{MediaId, Project};

    /// The playhead and the sound may drift this far apart (in samples) before the sound jumps.
    const DRIFT: i64 = SAMPLE_RATE as i64 / 15;
    /// How far ahead of the play position sound is decoded, in seconds.
    const AHEAD: u64 = 4;

    pub(super) struct Engine {
        ctx: egui::Context,
        pool: Arc<AudioPool>,
        output: Option<AudioOutput>,
        /// Opening the sound device failed; do not try again every frame.
        unavailable: bool,
        plan: AudioPlan,
        key: Option<(u64, u64)>,
        active: bool,
        /// The sample the pool was last asked to decode from.
        warmed: Option<u64>,
        meter: (f32, f32),
        /// The loudness blocks already read from the output.
        loudness_seen: u64,
        track_meters: Vec<(f32, f32)>,
    }

    impl Engine {
        pub(super) fn new(ctx: egui::Context) -> Self {
            let repaint = ctx.clone();
            Self {
                pool: Arc::new(AudioPool::new(move || repaint.request_repaint())),
                ctx,
                output: None,
                unavailable: false,
                plan: AudioPlan::default(),
                key: None,
                active: false,
                warmed: None,
                meter: (0.0, 0.0),
                track_meters: Vec::new(),
                loudness_seen: 0,
            }
        }

        pub(super) fn sync(
            &mut self,
            project: &Project,
            key: (u64, u64),
            playhead: u64,
            playing: bool,
        ) {
            if self.key != Some(key) {
                // A different project (or an edit): new pool for a new project, new plan always.
                if self.key.is_some_and(|k| k.0 != key.0) {
                    let repaint = self.ctx.clone();
                    self.pool = Arc::new(AudioPool::new(move || repaint.request_repaint()));
                    self.output = None;
                    self.active = false;
                }
                self.key = Some(key);
                self.plan = plan(project);
                if let Some(out) = &self.output {
                    out.set_plan(self.plan.clone());
                }
                self.warmed = None;
            }
            let fps = project.fps;
            let at = u64::try_from(
                u128::from(playhead) * u128::from(SAMPLE_RATE) * u128::from(fps.den)
                    / u128::from(fps.num.max(1)),
            )
            .unwrap_or(0);
            if !playing {
                if self.active {
                    if let Some(out) = &self.output {
                        out.pause();
                    }
                    self.active = false;
                }
                // Decode what the next play will need, so it starts at once.
                if self.warmed != Some(at) && !self.plan.clips.is_empty() {
                    self.pool.prefetch(&self.plan, at, AHEAD);
                    self.warmed = Some(at);
                }
                return;
            }
            if self.output.is_none() && !self.unavailable {
                match AudioOutput::new(self.pool.clone()) {
                    Ok(out) => {
                        out.set_plan(self.plan.clone());
                        self.output = Some(out);
                    }
                    Err(e) => {
                        tracing::warn!("no sound: {e}");
                        self.unavailable = true;
                    }
                }
            }
            let Some(out) = &self.output else { return };
            if !self.active {
                out.play(at);
                self.active = true;
            } else if (out.position() as i64 - at as i64).abs() > DRIFT {
                out.seek(at);
            }
            // Keep decoding ahead of the play position, once a second.
            let position = out.position();
            if self.warmed.is_none_or(|w| {
                position.saturating_sub(w) >= u64::from(SAMPLE_RATE) / 2 || position < w
            }) {
                self.pool.prefetch(&self.plan, position, AHEAD);
                self.warmed = Some(position);
            }
            // The meter falls back smoothly.
            let (l, r) = out.take_levels();
            self.meter = (l.max(self.meter.0 * 0.85), r.max(self.meter.1 * 0.85));
            let tracks = out.take_track_levels();
            self.track_meters.resize(tracks.len(), (0.0, 0.0));
            for (shown, (l, r)) in self.track_meters.iter_mut().zip(tracks) {
                *shown = (l.max(shown.0 * 0.85), r.max(shown.1 * 0.85));
            }
        }

        pub(super) fn loudness(&mut self) -> (Vec<f64>, f32) {
            match &self.output {
                Some(out) if self.active => (
                    out.take_loudness(&mut self.loudness_seen),
                    out.take_true_peak(),
                ),
                _ => (Vec::new(), 0.0),
            }
        }

        pub(super) fn meter(&mut self) -> (f32, f32) {
            if !self.active {
                self.meter = (self.meter.0 * 0.8, self.meter.1 * 0.8);
                if self.meter.0 < 0.001 && self.meter.1 < 0.001 {
                    self.meter = (0.0, 0.0);
                }
            }
            self.meter
        }

        pub(super) fn track_meters(&mut self) -> Vec<(f32, f32)> {
            if !self.active {
                for m in &mut self.track_meters {
                    *m = (m.0 * 0.8, m.1 * 0.8);
                    if m.0 < 0.001 && m.1 < 0.001 {
                        *m = (0.0, 0.0);
                    }
                }
            }
            self.track_meters.clone()
        }

        pub(super) fn peaks(&self, media: MediaId, path: &str) -> Option<Arc<Vec<u8>>> {
            match self.pool.peaks(media, path) {
                Peaks::Ready(p) => Some(p),
                Peaks::Pending | Peaks::None => None,
            }
        }
    }
}
