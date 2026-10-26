//! The desktop half: decoding sound in background threads and playing the mix through the sound
//! card.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, TryLockError};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream, StreamConfig};
use picasso_core::MediaId;
use picasso_decode::audio::{waveform, AudioDecoder};

use crate::mix::{AudioPlan, Mixer, SampleSource, METERED_TRACKS, SAMPLE_RATE};

/// Samples (per channel) in one piece of decoded sound: one second.
const CHUNK: u64 = SAMPLE_RATE as u64;
/// Pieces kept in memory in all, about 46 MB.
const MAX_CHUNKS: usize = 120;

/// A waveform: one byte (the peak, 0..=255) per fiftieth of a second of a file.
#[derive(Clone, Debug)]
pub enum Peaks {
    /// Being measured.
    Pending,
    /// The file has no sound, or could not be read.
    None,
    Ready(Arc<Vec<u8>>),
}

struct Chunk {
    data: Arc<Vec<f32>>,
    used: u64,
}

#[derive(Default)]
struct Wanted {
    path: String,
    chunks: BTreeSet<u64>,
}

#[derive(Default)]
struct State {
    chunks: HashMap<(MediaId, u64), Chunk>,
    tick: u64,
    wanted: HashMap<MediaId, Wanted>,
    workers: HashSet<MediaId>,
    /// Files without sound, or that could not be read: silence, and nothing is missing.
    silent: HashSet<MediaId>,
    peaks: HashMap<MediaId, Peaks>,
    closed: bool,
}

struct Shared {
    state: Mutex<State>,
    changed: Condvar,
    /// Called when something finished in the background (to redraw).
    notify: Box<dyn Fn() + Send + Sync>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Decodes the sound of the files of one project, one worker thread per file, on demand.
pub struct AudioPool {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for AudioPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AudioPool")
    }
}

impl Drop for AudioPool {
    fn drop(&mut self) {
        self.shared.lock().closed = true;
        self.shared.changed.notify_all();
    }
}

impl AudioPool {
    /// `notify` is called from background threads when a waveform is ready.
    pub fn new(notify: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            shared: Arc::new(Shared {
                state: Mutex::default(),
                changed: Condvar::new(),
                notify: Box::new(notify),
            }),
        }
    }

    fn request(&self, state: &mut State, media: MediaId, path: &str, chunk: u64, spawn: bool) {
        let w = state.wanted.entry(media).or_default();
        if w.path.is_empty() {
            w.path = path.to_string();
        }
        w.chunks.insert(chunk);
        if spawn && state.workers.insert(media) {
            let shared = self.shared.clone();
            std::thread::spawn(move || run_worker(&shared, media));
        }
        self.shared.changed.notify_all();
    }

    /// Starts decoding the sound that will be needed for `seconds` from sample `from` on. Called
    /// from the interface (never from the audio thread) when the playhead moves.
    pub fn prefetch(&self, plan: &AudioPlan, from: u64, seconds: u64) {
        let to = from.saturating_add(seconds * u64::from(SAMPLE_RATE));
        let mut state = self.shared.lock();
        for clip in &plan.clips {
            let end = clip.start.saturating_add(clip.len);
            if clip.start >= to || end <= from || state.silent.contains(&clip.media) {
                continue;
            }
            let (first, last) = (clip.start.max(from), end.min(to));
            let speed = f64::from(clip.speed);
            let offset = if clip.reverse {
                // Backwards, the part ends where the clip's sound is nearest its start.
                clip.source_in + ((clip.start + clip.len - last) as f64 * speed) as u64
            } else {
                clip.source_in + ((first - clip.start) as f64 * speed) as u64
            };
            let span = ((last - first) as f64 * speed).ceil() as u64 + 1;
            for chunk in offset / CHUNK..=(offset + span) / CHUNK {
                if !state.chunks.contains_key(&(clip.media, chunk)) {
                    self.request(&mut state, clip.media, &clip.path, chunk, true);
                }
            }
        }
    }

    /// The waveform of a file, measured in the background the first time it is asked for.
    pub fn peaks(&self, media: MediaId, path: &str) -> Peaks {
        let mut state = self.shared.lock();
        if let Some(p) = state.peaks.get(&media) {
            return p.clone();
        }
        state.peaks.insert(media, Peaks::Pending);
        let shared = self.shared.clone();
        let path = path.to_string();
        std::thread::spawn(move || {
            let result = match waveform(Path::new(&path)) {
                Ok(Some(peaks)) => Peaks::Ready(Arc::new(peaks)),
                Ok(None) => Peaks::None,
                Err(e) => {
                    tracing::warn!("cannot measure {path}: {e}");
                    Peaks::None
                }
            };
            shared.lock().peaks.insert(media, result);
            (shared.notify)();
        });
        Peaks::Pending
    }

    /// Adds `gain` times the samples at `source_sample` of a file to `out`, waiting until they
    /// are decoded (for a mixdown). A file that cannot be decoded adds nothing.
    pub fn read_exact(
        &self,
        media: MediaId,
        path: &str,
        source_sample: u64,
        gain: f32,
        out: &mut [f32],
    ) {
        let n = (out.len() / 2) as u64;
        let mut state = self.shared.lock();
        if state.silent.contains(&media) {
            return;
        }
        for chunk in source_sample / CHUNK..=(source_sample + n) / CHUNK {
            self.request(&mut state, media, path, chunk, true);
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            let ready = (source_sample / CHUNK..=(source_sample + n) / CHUNK)
                .all(|c| state.chunks.contains_key(&(media, c)));
            if ready || state.silent.contains(&media) || std::time::Instant::now() > deadline {
                break;
            }
            state = self
                .shared
                .changed
                .wait_timeout(state, std::time::Duration::from_millis(100))
                .map(|(s, _)| s)
                .unwrap_or_else(|e| e.into_inner().0);
        }
        add_from_chunks(&mut state, media, source_sample, gain, out);
    }
}

/// Reads sound from a pool and waits for whatever is not decoded yet: for a mixdown, where
/// nothing may be left out.
pub struct Offline<'a>(pub &'a AudioPool);

impl SampleSource for Offline<'_> {
    fn add_samples(
        &self,
        media: MediaId,
        path: &str,
        source_sample: u64,
        gain: f32,
        out: &mut [f32],
    ) -> bool {
        self.0.read_exact(media, path, source_sample, gain, out);
        true
    }
}

/// Adds the available pieces to `out`; returns false when a piece was missing.
fn add_from_chunks(
    state: &mut State,
    media: MediaId,
    source_sample: u64,
    gain: f32,
    out: &mut [f32],
) -> bool {
    let n = (out.len() / 2) as u64;
    let mut complete = true;
    let mut done = 0u64;
    while done < n {
        let at = source_sample + done;
        let (idx, within) = (at / CHUNK, (at % CHUNK) as usize);
        let take = ((CHUNK - within as u64).min(n - done)) as usize;
        state.tick += 1;
        let tick = state.tick;
        match state.chunks.get_mut(&(media, idx)) {
            Some(chunk) => {
                chunk.used = tick;
                if let Some(src) = chunk.data.get(within * 2..(within + take) * 2) {
                    let dst = (done as usize) * 2;
                    for (o, s) in out[dst..dst + take * 2].iter_mut().zip(src) {
                        *o += s * gain;
                    }
                }
            }
            None => complete = false,
        }
        done += take as u64;
    }
    complete
}

impl SampleSource for AudioPool {
    fn add_samples(
        &self,
        media: MediaId,
        path: &str,
        source_sample: u64,
        gain: f32,
        out: &mut [f32],
    ) -> bool {
        // The audio thread must not wait: if the lock is busy, this block is silent.
        let mut state = match self.shared.state.try_lock() {
            Ok(s) => s,
            Err(TryLockError::Poisoned(e)) => e.into_inner(),
            Err(TryLockError::WouldBlock) => return false,
        };
        if state.silent.contains(&media) {
            return true;
        }
        let complete = add_from_chunks(&mut state, media, source_sample, gain, out);
        if !complete {
            let n = (out.len() / 2) as u64;
            for chunk in source_sample / CHUNK..=(source_sample + n) / CHUNK {
                if !state.chunks.contains_key(&(media, chunk)) {
                    self.request(&mut state, media, path, chunk, false);
                }
            }
        }
        complete
    }
}

/// The thread of one file: decodes the pieces that are wanted, nearest to the start first.
fn run_worker(shared: &Shared, media: MediaId) {
    let mut decoder: Option<AudioDecoder> = None;
    loop {
        let (path, chunk) = {
            let mut state = shared.lock();
            loop {
                if state.closed {
                    return;
                }
                let next = state.wanted.get_mut(&media).and_then(|w| {
                    let c = w.chunks.pop_first()?;
                    Some((w.path.clone(), c))
                });
                match next {
                    Some((path, c)) if !state.chunks.contains_key(&(media, c)) => break (path, c),
                    Some(_) => continue,
                    None => {
                        state = shared
                            .changed
                            .wait(state)
                            .unwrap_or_else(|e| e.into_inner());
                    }
                }
            }
        };
        if decoder.is_none() {
            match AudioDecoder::open(Path::new(&path)) {
                Ok(Some(d)) => decoder = Some(d),
                Ok(None) => return mark_silent(shared, media),
                Err(e) => {
                    tracing::warn!("cannot open the sound of {path}: {e}");
                    return mark_silent(shared, media);
                }
            }
        }
        let Some(d) = decoder.as_mut() else { return };
        let Ok(data) = d.read_at(chunk * CHUNK, CHUNK as usize) else {
            return mark_silent(shared, media);
        };
        let mut state = shared.lock();
        state.tick += 1;
        let tick = state.tick;
        state.chunks.insert(
            (media, chunk),
            Chunk {
                data: Arc::new(data),
                used: tick,
            },
        );
        while state.chunks.len() > MAX_CHUNKS {
            let oldest = state
                .chunks
                .iter()
                .min_by_key(|(_, c)| c.used)
                .map(|(k, _)| *k);
            match oldest {
                Some(k) => {
                    state.chunks.remove(&k);
                }
                None => break,
            }
        }
        drop(state);
        shared.changed.notify_all();
    }
}

fn mark_silent(shared: &Shared, media: MediaId) {
    let mut state = shared.lock();
    state.silent.insert(media);
    state.wanted.remove(&media);
    drop(state);
    shared.changed.notify_all();
}

/// What the audio thread and the interface share.
struct Playback {
    position: AtomicU64,
    playing: AtomicBool,
    plan: Mutex<Arc<AudioPlan>>,
    /// Loudest sample of each channel since the interface last asked, as `f32` bits.
    peak_left: AtomicU32,
    peak_right: AtomicU32,
    /// The same for each track (left, right), as `f32` bits.
    track_peaks: Vec<AtomicU32>,
    /// The K-weighted power of the last 100 ms blocks played (`f64` bits), a ring of
    /// [`LOUDNESS_RING`] written in turn; `loudness_written` counts the blocks ever written.
    loudness: Vec<AtomicU64>,
    loudness_written: AtomicU64,
    /// The highest true peak since the interface last asked, as `f32` bits.
    true_peak: AtomicU32,
}

/// How many loudness blocks the audio thread keeps for the interface (6.4 s).
const LOUDNESS_RING: usize = 64;

/// The sound card output: plays the mix of a plan from a position.
pub struct AudioOutput {
    _stream: Stream,
    playback: Arc<Playback>,
}

impl AudioOutput {
    /// Opens the default output device. Fails when there is none.
    pub fn new(pool: Arc<AudioPool>) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("there is no sound output device")?;
        let config = device.default_output_config().map_err(|e| e.to_string())?;
        let playback = Arc::new(Playback {
            position: AtomicU64::new(0),
            playing: AtomicBool::new(false),
            plan: Mutex::new(Arc::new(AudioPlan::default())),
            peak_left: AtomicU32::new(0),
            peak_right: AtomicU32::new(0),
            track_peaks: (0..METERED_TRACKS * 2).map(|_| AtomicU32::new(0)).collect(),
            loudness: (0..LOUDNESS_RING).map(|_| AtomicU64::new(0)).collect(),
            loudness_written: AtomicU64::new(0),
            true_peak: AtomicU32::new(0),
        });
        let format = config.sample_format();
        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, config.into(), &playback, &pool),
            SampleFormat::I16 => build::<i16>(&device, config.into(), &playback, &pool),
            SampleFormat::I32 => build::<i32>(&device, config.into(), &playback, &pool),
            SampleFormat::U16 => build::<u16>(&device, config.into(), &playback, &pool),
            other => Err(format!("the sound format {other} is not supported")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Self {
            _stream: stream,
            playback,
        })
    }

    /// Replaces what is played (after an edit); takes effect at the next block.
    pub fn set_plan(&self, plan: AudioPlan) {
        if let Ok(mut slot) = self.playback.plan.lock() {
            *slot = Arc::new(plan);
        }
    }

    /// Starts playing from timeline sample `from`.
    pub fn play(&self, from: u64) {
        self.playback.position.store(from, Ordering::Relaxed);
        self.playback.playing.store(true, Ordering::Relaxed);
    }

    pub fn pause(&self) {
        self.playback.playing.store(false, Ordering::Relaxed);
    }

    /// Moves the play position (while playing, the sound jumps).
    pub fn seek(&self, to: u64) {
        self.playback.position.store(to, Ordering::Relaxed);
    }

    /// The timeline sample being played now (the sound card's buffer is a few milliseconds
    /// ahead of what is heard).
    pub fn position(&self) -> u64 {
        self.playback.position.load(Ordering::Relaxed)
    }

    /// The loudest left and right samples of each track since the last call.
    pub fn take_track_levels(&self) -> Vec<(f32, f32)> {
        self.playback
            .track_peaks
            .chunks_exact(2)
            .map(|pair| {
                (
                    f32::from_bits(pair[0].swap(0, Ordering::Relaxed)),
                    f32::from_bits(pair[1].swap(0, Ordering::Relaxed)),
                )
            })
            .collect()
    }

    /// The loudness blocks (K-weighted mean power of 100 ms) played since block `seen`, which
    /// is moved past them. Blocks older than the ring holds are skipped.
    pub fn take_loudness(&self, seen: &mut u64) -> Vec<f64> {
        let written = self.playback.loudness_written.load(Ordering::Acquire);
        let from = (*seen).max(written.saturating_sub(LOUDNESS_RING as u64));
        let blocks = (from..written)
            .map(|n| {
                f64::from_bits(
                    self.playback.loudness[(n % LOUDNESS_RING as u64) as usize]
                        .load(Ordering::Relaxed),
                )
            })
            .collect();
        *seen = written;
        blocks
    }

    /// The highest true peak since the last call (linear).
    pub fn take_true_peak(&self) -> f32 {
        f32::from_bits(self.playback.true_peak.swap(0, Ordering::Relaxed))
    }

    /// The loudest left and right samples since the last call (0.0 to 1.0 and beyond).
    pub fn take_levels(&self) -> (f32, f32) {
        (
            f32::from_bits(self.playback.peak_left.swap(0, Ordering::Relaxed)),
            f32::from_bits(self.playback.peak_right.swap(0, Ordering::Relaxed)),
        )
    }
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    playback: &Arc<Playback>,
    pool: &Arc<AudioPool>,
) -> Result<Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels).max(1);
    let rate = config.sample_rate.max(1);
    let playback = playback.clone();
    let pool = pool.clone();
    let mut mixed: Vec<f32> = Vec::with_capacity(8192);
    let mut mixer = Mixer::new();
    let mut plan = Arc::new(AudioPlan::default());
    let mut weighting = crate::KWeighting::new();
    let mut true_peak = crate::TruePeak::default();
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                let frames = data.len() / channels;
                if !playback.playing.load(Ordering::Relaxed) {
                    data.fill(T::EQUILIBRIUM);
                    return;
                }
                if let Ok(slot) = playback.plan.try_lock() {
                    if !Arc::ptr_eq(&slot, &plan) {
                        plan = slot.clone();
                    }
                }
                // How many samples of the 48 kHz mix these device frames use.
                let need =
                    (frames as u64 * u64::from(SAMPLE_RATE)).div_ceil(u64::from(rate)) as usize;
                mixed.resize(need * 2, 0.0);
                let from = playback.position.load(Ordering::Relaxed);
                mixer.mix(&plan, &*pool, from, &mut mixed);
                // Non-negative floats order like their bits, so the loudest is a plain maximum.
                for (i, peak) in mixer.track_peaks.iter().enumerate() {
                    playback.track_peaks[i * 2].fetch_max(peak[0].to_bits(), Ordering::Relaxed);
                    playback.track_peaks[i * 2 + 1].fetch_max(peak[1].to_bits(), Ordering::Relaxed);
                }
                playback
                    .position
                    .store(from + need as u64, Ordering::Relaxed);
                // Loudness and true peak, measured on the 48 kHz mix as it is played.
                true_peak.peak = 0.0;
                for frame in mixed.chunks_exact(2) {
                    true_peak.push(frame[0], frame[1]);
                    if let Some(power) = weighting.push(frame[0], frame[1]) {
                        let n = playback.loudness_written.load(Ordering::Relaxed);
                        playback.loudness[(n % LOUDNESS_RING as u64) as usize]
                            .store(power.to_bits(), Ordering::Relaxed);
                        playback.loudness_written.store(n + 1, Ordering::Release);
                    }
                }
                playback
                    .true_peak
                    .fetch_max(true_peak.peak.to_bits(), Ordering::Relaxed);
                let (mut pl, mut pr) = (0.0f32, 0.0f32);
                for (i, frame) in data.chunks_exact_mut(channels).enumerate() {
                    // Straight copy at 48 kHz, linear interpolation at any other rate.
                    let pos = i as f32 * SAMPLE_RATE as f32 / rate as f32;
                    let (a, f) = (pos as usize, pos.fract());
                    let b = (a + 1).min(need - 1);
                    let a = a.min(need - 1);
                    let l = mixed[a * 2] * (1.0 - f) + mixed[b * 2] * f;
                    let r = mixed[a * 2 + 1] * (1.0 - f) + mixed[b * 2 + 1] * f;
                    pl = pl.max(l.abs());
                    pr = pr.max(r.abs());
                    for (c, out) in frame.iter_mut().enumerate() {
                        let v = match (channels, c) {
                            (1, _) => (l + r) * 0.5,
                            (_, 0) => l,
                            (_, 1) => r,
                            _ => 0.0,
                        };
                        *out = T::from_sample(v.clamp(-1.0, 1.0));
                    }
                }
                let _ =
                    playback
                        .peak_left
                        .try_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
                            (pl > f32::from_bits(old)).then(|| pl.to_bits())
                        });
                let _ =
                    playback
                        .peak_right
                        .try_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
                            (pr > f32::from_bits(old)).then(|| pr.to_bits())
                        });
            },
            |e| tracing::warn!("sound output: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mix::{mix_block, ClipSound};

    /// A two-second clip with a picture and a 440 Hz tone; `None` without the `ffmpeg` program.
    fn clip_with_sound() -> Option<String> {
        let path = std::env::temp_dir().join("picasso_pool_test.mp4");
        std::process::Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-f", "lavfi", "-i"])
            .arg("testsrc=duration=2:size=64x64:rate=24")
            .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=2"])
            .args(["-pix_fmt", "yuv420p", "-shortest"])
            .arg(&path)
            .status()
            .is_ok_and(|s| s.success())
            .then(|| path.to_string_lossy().into_owned())
    }

    #[test]
    fn the_pool_decodes_ahead_and_the_mix_then_has_sound() {
        let Some(path) = clip_with_sound() else {
            return;
        };
        let pool = AudioPool::new(|| {});
        let plan = AudioPlan {
            clips: vec![ClipSound::plain(MediaId(1), &path, 48_000, 96_000, 0)],
            ..Default::default()
        };
        let mut block = vec![0.0; 2 * 4800];
        // Nothing is decoded at first: silence, and the pool is asked to decode.
        assert!(!mix_block(&plan, &pool, 48_000, &mut block));
        assert!(block.iter().all(|s| *s == 0.0));
        pool.prefetch(&plan, 48_000, 3);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !mix_block(&plan, &pool, 48_000, &mut block) {
            assert!(
                std::time::Instant::now() < deadline,
                "decoding took too long"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(block.iter().any(|s| s.abs() > 0.05), "the tone is mixed in");
        // Before the clip starts there is nothing to wait for and nothing to hear.
        let mut early = vec![0.0; 2 * 100];
        assert!(mix_block(&plan, &pool, 0, &mut early));
        assert!(early.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn the_sound_card_output_plays_the_mix_and_reports_levels() {
        let Some(path) = clip_with_sound() else {
            return;
        };
        let pool = Arc::new(AudioPool::new(|| {}));
        // No sound device (a build machine): nothing to test.
        let Ok(out) = AudioOutput::new(pool.clone()) else {
            return;
        };
        let plan = AudioPlan {
            clips: vec![{
                let mut clip = ClipSound::plain(MediaId(1), &path, 0, 96_000, 0);
                clip.gain = 0.5;
                clip
            }],
            ..Default::default()
        };
        out.set_plan(plan.clone());
        pool.prefetch(&plan, 0, 2);
        std::thread::sleep(std::time::Duration::from_millis(500));
        out.play(0);
        std::thread::sleep(std::time::Duration::from_millis(600));
        let position = out.position();
        let (left, right) = out.take_levels();
        out.pause();
        assert!(position > 10_000, "the position moves: {position}");
        assert!(
            left > 0.02 && right > 0.02,
            "the tone reached the output: {left} {right}"
        );
        assert!(left <= 0.6, "the gain is applied: {left}");
        // Loudness was measured on what was played: a few 100 ms blocks with power in them.
        let mut seen = 0;
        let blocks = out.take_loudness(&mut seen);
        assert!(blocks.len() >= 3, "{} blocks", blocks.len());
        assert!(blocks.iter().any(|p| *p > 1e-6));
        assert!(
            out.take_loudness(&mut seen).is_empty(),
            "each block is read once"
        );
        let tp = out.take_true_peak();
        assert!(
            tp >= left * 0.9,
            "the true peak is at least the sample peak: {tp} {left}"
        );
    }
}
