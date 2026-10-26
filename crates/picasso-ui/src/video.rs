//! Video files: decoded with FFmpeg on the desktop, in background threads. The browser build has
//! no FFmpeg, so a video there shows the offline picture and cannot be imported.

use picasso_render::MediaCache;

/// A media cache for a new project, with the video source for this platform attached. `ctx` is
/// asked to repaint when a decoded picture arrives.
pub(crate) fn new_cache(ctx: &egui::Context) -> MediaCache {
    #[cfg(not(target_arch = "wasm32"))]
    {
        MediaCache::with_video_source(std::sync::Arc::new(native::VideoPool::new(ctx.clone())))
            .with_thumbnail_source(std::sync::Arc::new(native::VideoPool::new(ctx.clone())))
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = ctx;
        MediaCache::new()
    }
}

/// How many project frames the video at `path` lasts, at `fps` (frames, seconds). A file that
/// does not say how long it is gets a minute, which can be trimmed.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn length_in_frames(path: &str, fps: (u32, u32)) -> Result<u64, String> {
    let info = picasso_decode::probe(std::path::Path::new(path)).map_err(|e| e.to_string())?;
    let per_second = u64::from(fps.0) / u64::from(fps.1.max(1));
    let Some(us) = info.duration_us.and_then(|d| u128::try_from(d).ok()) else {
        return Ok(per_second.max(1) * 60);
    };
    let frames = us * u128::from(fps.0) / (1_000_000 * u128::from(fps.1.max(1)));
    Ok(u64::try_from(frames).unwrap_or(0).max(1))
}

/// The frame rate of the video at `path`, when it says one.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn frame_rate(path: &str) -> Option<picasso_core::Fps> {
    let info = picasso_decode::probe(std::path::Path::new(path)).ok()?;
    crate::fps_prompt::tidy(info.fps.0, info.fps.1)
}

/// How many project frames the sound file at `path` lasts, at `fps` (frames, seconds).
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn audio_length_in_frames(path: &str, fps: (u32, u32)) -> Result<u64, String> {
    let duration = picasso_decode::audio::probe_audio(std::path::Path::new(path))
        .map_err(|e| e.to_string())?;
    let per_second = u64::from(fps.0) / u64::from(fps.1.max(1));
    let Some(us) = duration.and_then(|d| u128::try_from(d).ok()) else {
        return Ok(per_second.max(1) * 60);
    };
    let frames = us * u128::from(fps.0) / (1_000_000 * u128::from(fps.1.max(1)));
    Ok(u64::try_from(frames).unwrap_or(0).max(1))
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::collections::{HashMap, HashSet, VecDeque};
    use std::fmt;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Condvar, Mutex, MutexGuard};
    use std::time::{Duration, Instant};

    use picasso_core::MediaId;
    use picasso_decode::VideoDecoder;
    use picasso_render::{Decoded, FrameSource, VideoFrame};

    /// Pictures kept per video, so stepping back and forth, or drawing the same frame twice,
    /// does not decode again.
    const KEEP: usize = 8;
    /// How many pictures a worker decodes ahead of the last one asked for while it has nothing
    /// else to do (playing forward asks for the next picture a moment later).
    const READ_AHEAD: u8 = 3;
    /// Two requests this close, forward, count as playing.
    const PLAY_STEP_US: i64 = 200_000;
    /// An exact picture that takes longer than this to arrive is given up on.
    const WAIT_LIMIT: Duration = Duration::from_secs(20);

    /// The newest thing asked of a video's worker. Only the latest request matters: while the
    /// playhead is dragged, the pictures in between are skipped.
    struct Want {
        path: String,
        time_us: i64,
        reduce: u32,
        /// How far apart the last two requests were, when that looks like playing forward.
        step_us: i64,
    }

    #[derive(Default)]
    struct State {
        /// Each picture with its time and how many times smaller than the file it was made.
        frames: HashMap<MediaId, VecDeque<(i64, u32, Arc<Decoded>)>>,
        want: HashMap<MediaId, Want>,
        last_asked: HashMap<MediaId, i64>,
        workers: HashSet<MediaId>,
        /// Videos that could not be opened or decoded; they show the offline picture.
        broken: HashSet<MediaId>,
        closed: bool,
    }

    struct Shared {
        state: Mutex<State>,
        changed: Condvar,
        generation: AtomicU64,
        ctx: egui::Context,
    }

    impl Shared {
        fn lock(&self) -> MutexGuard<'_, State> {
            // A panic in a worker poisons the lock; the data is still usable.
            self.state.lock().unwrap_or_else(|e| e.into_inner())
        }

        /// Wakes whoever waits for a picture and asks for a repaint.
        fn announce(&self) {
            self.generation.fetch_add(1, Ordering::Relaxed);
            self.changed.notify_all();
            self.ctx.request_repaint();
        }
    }

    /// Decodes the videos of one project, one worker thread per file, on demand.
    pub(super) struct VideoPool {
        shared: Arc<Shared>,
    }

    impl VideoPool {
        pub(super) fn new(ctx: egui::Context) -> Self {
            Self {
                shared: Arc::new(Shared {
                    state: Mutex::default(),
                    changed: Condvar::new(),
                    generation: AtomicU64::new(0),
                    ctx,
                }),
            }
        }
    }

    impl fmt::Debug for VideoPool {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("VideoPool")
        }
    }

    impl Drop for VideoPool {
        fn drop(&mut self) {
            self.shared.lock().closed = true;
            self.shared.changed.notify_all();
        }
    }

    fn exact(state: &State, media: MediaId, time_us: i64, reduce: u32) -> Option<Arc<Decoded>> {
        state
            .frames
            .get(&media)?
            .iter()
            .find(|(t, r, _)| *t == time_us && *r == reduce)
            .map(|(_, _, f)| f.clone())
    }

    /// The picture closest in time to the one asked for, however far: while the playhead is
    /// dragged the exact picture takes a moment (the decoder starts from the key frame before it),
    /// and the viewer shows this one meanwhile instead of flashing black.
    fn nearest(state: &State, media: MediaId, time_us: i64, reduce: u32) -> Option<Arc<Decoded>> {
        state
            .frames
            .get(&media)?
            .iter()
            .filter(|(_, r, _)| *r == reduce)
            .min_by_key(|(t, _, _)| (t - time_us).abs())
            .map(|(_, _, f)| f.clone())
    }

    impl FrameSource for VideoPool {
        fn video_frame(
            &self,
            media: MediaId,
            path: &str,
            time_us: i64,
            reduce: u32,
            wait: bool,
        ) -> VideoFrame {
            let shared = &self.shared;
            let mut state = shared.lock();
            if state.broken.contains(&media) {
                return VideoFrame::Offline;
            }
            if let Some(hit) = exact(&state, media, time_us, reduce) {
                return VideoFrame::Ready(hit);
            }
            let step_us = match state.last_asked.insert(media, time_us) {
                Some(before) if time_us > before && time_us - before <= PLAY_STEP_US => {
                    time_us - before
                }
                _ => 0,
            };
            state.want.insert(
                media,
                Want {
                    path: path.to_string(),
                    time_us,
                    reduce,
                    step_us,
                },
            );
            if state.workers.insert(media) {
                let worker = shared.clone();
                std::thread::spawn(move || run_worker(&worker, media));
            }
            shared.changed.notify_all();
            if !wait {
                return nearest(&state, media, time_us, reduce)
                    .map_or(VideoFrame::Pending, VideoFrame::StandIn);
            }
            let deadline = Instant::now() + WAIT_LIMIT;
            loop {
                if state.broken.contains(&media) {
                    return VideoFrame::Offline;
                }
                if let Some(hit) = exact(&state, media, time_us, reduce) {
                    return VideoFrame::Ready(hit);
                }
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return VideoFrame::Offline;
                }
                // Someone else's request may have replaced ours; ask again if nothing is queued.
                state.want.entry(media).or_insert_with(|| Want {
                    path: path.to_string(),
                    time_us,
                    reduce,
                    step_us: 0,
                });
                shared.changed.notify_all();
                state = shared
                    .changed
                    .wait_timeout(state, left.min(Duration::from_millis(200)))
                    .map(|(s, _)| s)
                    .unwrap_or_else(|e| e.into_inner().0);
            }
        }

        fn generation(&self) -> u64 {
            self.shared.generation.load(Ordering::Relaxed)
        }
    }

    /// The thread of one video: waits for a request, decodes it, then a few pictures ahead.
    fn run_worker(shared: &Shared, media: MediaId) {
        let mut decoder: Option<VideoDecoder> = None;
        let mut path = String::new();
        // The next time to read ahead, how many more after it, and the step between them.
        let mut ahead: Option<(i64, u8, i64)> = None;
        let mut reduce = 1;
        loop {
            // Whether this picture is only read ahead (nobody asked for it yet).
            let mut reading_ahead = false;
            let time_us = {
                let mut state = shared.lock();
                loop {
                    if state.closed {
                        return;
                    }
                    if let Some(w) = state.want.remove(&media) {
                        path = w.path;
                        reduce = w.reduce;
                        ahead = (w.step_us > 0).then_some((
                            w.time_us + w.step_us,
                            READ_AHEAD,
                            w.step_us,
                        ));
                        break w.time_us;
                    }
                    if let Some((next, left, step)) = ahead {
                        ahead = (left > 1).then_some((next + step, left - 1, step));
                        if exact(&state, media, next, reduce).is_none() {
                            reading_ahead = true;
                            break next;
                        }
                        continue;
                    }
                    state = shared
                        .changed
                        .wait(state)
                        .unwrap_or_else(|e| e.into_inner());
                }
            };
            if exact(&shared.lock(), media, time_us, reduce).is_some() {
                continue;
            }
            // Hardware decoding was turned on or off: open the file again the new way.
            if decoder.as_ref().is_some_and(VideoDecoder::is_stale) {
                decoder = None;
            }
            if decoder.is_none() {
                match VideoDecoder::open(Path::new(&path)) {
                    Ok(d) => decoder = Some(d),
                    Err(e) => {
                        tracing::warn!("cannot open {path}: {e}");
                        fail(shared, media);
                        return;
                    }
                }
            }
            let Some(d) = decoder.as_mut() else { return };
            let mut decoded = d.frame_at_reduced(time_us, reduce);
            if decoded.is_err() && reading_ahead {
                // Past the end of the file (playing up to the last frame): stop reading ahead.
                ahead = None;
                continue;
            }
            if decoded.is_err() {
                // A decoder left at the end of the file can fail to seek back (a loop to the
                // start): open the file again and try once more.
                decoder = VideoDecoder::open(Path::new(&path)).ok();
                if let Some(d) = decoder.as_mut() {
                    decoded = d.frame_at_reduced(time_us, reduce);
                }
            }
            if decoded.is_err() {
                // A time past the last picture keeps showing the nearest one.
                let mut state = shared.lock();
                let near = state
                    .frames
                    .get(&media)
                    .and_then(|q| {
                        q.iter()
                            .filter(|(_, r, _)| *r == reduce)
                            .max_by_key(|(t, _, _)| *t)
                    })
                    .filter(|(t, _, _)| *t < time_us)
                    .map(|(_, _, f)| f.clone());
                if let Some(image) = near {
                    let queue = state.frames.entry(media).or_default();
                    queue.push_back((time_us, reduce, image));
                    while queue.len() > KEEP {
                        queue.pop_front();
                    }
                    drop(state);
                    shared.announce();
                    continue;
                }
            }
            match decoded {
                Ok(frame) => {
                    let image = Arc::new(Decoded {
                        width: frame.width,
                        height: frame.height,
                        rgba: frame.rgba,
                    });
                    let mut state = shared.lock();
                    let queue = state.frames.entry(media).or_default();
                    queue.push_back((time_us, reduce, image));
                    while queue.len() > KEEP {
                        queue.pop_front();
                    }
                    drop(state);
                    shared.announce();
                }
                Err(e) => {
                    tracing::warn!("cannot decode at {time_us} us: {e}");
                    fail(shared, media);
                    return;
                }
            }
        }
    }

    fn fail(shared: &Shared, media: MediaId) {
        shared.lock().broken.insert(media);
        shared.announce();
    }
}
