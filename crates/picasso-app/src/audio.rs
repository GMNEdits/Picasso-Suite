//! Sound during playback: an `ffmpeg` process mixes the timeline's audio from the playhead on
//! (the same filter graph as an export) and a reader thread queues the samples for the sound
//! card. The card follows the video clock: whatever arrives late is skipped, so picture and sound
//! stay together.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream, StreamConfig};
use picasso_core::Project;
use picasso_media::tools::{Tool, command};
use picasso_render::export::preview_audio_args;
use std::collections::VecDeque;
use std::io::Read;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

/// Seconds of decoded sound kept ahead of the sound card.
const AHEAD_SECONDS: usize = 2;
/// Sound later than this behind the picture is skipped (seconds).
const LATE: f64 = 0.04;

struct Shared {
    /// Interleaved stereo samples.
    queue: Mutex<VecDeque<f32>>,
    space: Condvar,
    stop: AtomicBool,
    /// Loudest sample sent to the sound card (as f32 bits; non-negative floats order like their bits).
    peak: AtomicU32,
}

pub struct AudioPlayer {
    _stream: Stream,
    child: Child,
    shared: Arc<Shared>,
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.shared.space.notify_all();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl AudioPlayer {
    /// Plays the timeline's sound from frame `from`, which is on screen at `at`.
    pub fn start(project: &Project, from: i64, at: Instant) -> Result<Self, String> {
        let end = project.timeline.duration();
        if from >= end {
            return Err("nothing to play".into());
        }
        let device = cpal::default_host().default_output_device().ok_or("there is no sound output device")?;
        let config = device.default_output_config().map_err(|e| e.to_string())?;
        let rate = config.sample_rate();

        let mut cmd = command(Tool::Ffmpeg);
        cmd.args(preview_audio_args(project, from, end, rate)).stdout(Stdio::piped()).stderr(Stdio::null());
        let mut child = cmd.spawn().map_err(|e| format!("ffmpeg: {e}"))?;
        let stdout = child.stdout.take().ok_or("ffmpeg has no output")?;
        let shared = Arc::new(Shared { queue: Mutex::new(VecDeque::new()), space: Condvar::new(), stop: AtomicBool::new(false), peak: AtomicU32::new(0) });
        let reader = shared.clone();
        std::thread::spawn(move || read_samples(stdout, &reader, rate as usize * 2 * AHEAD_SECONDS));

        let format = config.sample_format();
        let config: StreamConfig = config.into();
        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, &config, &shared, at),
            SampleFormat::I16 => build::<i16>(&device, &config, &shared, at),
            SampleFormat::I32 => build::<i32>(&device, &config, &shared, at),
            SampleFormat::U16 => build::<u16>(&device, &config, &shared, at),
            other => Err(format!("the sound format {other} is not supported")),
        };
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                shared.stop.store(true, Ordering::Relaxed);
                let _ = child.kill();
                let _ = child.wait();
                return Err(e);
            }
        };
        stream.play().map_err(|e| e.to_string())?;
        Ok(Self { _stream: stream, child, shared })
    }
}

/// Moves samples from ffmpeg into the queue, keeping at most `limit` queued.
fn read_samples(mut pipe: impl Read, shared: &Shared, limit: usize) {
    let mut buf = vec![0u8; 16384];
    let mut carry = 0;
    loop {
        let n = match pipe.read(&mut buf[carry..]) {
            Ok(0) | Err(_) => return,
            Ok(n) => n + carry,
        };
        let whole = n / 4 * 4;
        let mut queue = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
        while queue.len() >= limit && !shared.stop.load(Ordering::Relaxed) {
            queue = shared.space.wait(queue).unwrap_or_else(|e| e.into_inner());
        }
        if shared.stop.load(Ordering::Relaxed) {
            return;
        }
        queue.extend(buf[..whole].as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)));
        drop(queue);
        buf.copy_within(whole..n, 0);
        carry = n - whole;
    }
}

fn build<T>(device: &cpal::Device, config: &StreamConfig, shared: &Arc<Shared>, at: Instant) -> Result<Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels).max(1);
    let rate = f64::from(config.sample_rate.max(1));
    let shared = shared.clone();
    // Stereo frames taken from the queue so far: played or skipped.
    let mut used: u64 = 0;
    device
        .build_output_stream(
            *config,
            move |data: &mut [T], _| {
                let Ok(mut queue) = shared.queue.try_lock() else {
                    data.fill(T::EQUILIBRIUM);
                    return;
                };
                // Catch up with the picture when sound arrived late (start-up, a slow disk).
                let due = (at.elapsed().as_secs_f64() * rate) as u64;
                if due > used + (LATE * rate) as u64 {
                    let skip = ((due - used) as usize).min(queue.len() / 2);
                    queue.drain(..skip * 2);
                    used += skip as u64;
                }
                for frame in data.chunks_exact_mut(channels) {
                    if queue.len() < 2 {
                        frame.fill(T::EQUILIBRIUM);
                        continue;
                    }
                    let (l, r) = (queue.pop_front().unwrap_or(0.0), queue.pop_front().unwrap_or(0.0));
                    used += 1;
                    shared.peak.fetch_max(l.abs().max(r.abs()).to_bits(), Ordering::Relaxed);
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
                drop(queue);
                shared.space.notify_one();
            },
            |e| log::warn!("sound output: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use picasso_core::{ProjectSettings, Rational};

    #[test]
    fn playback_sends_the_timeline_sound_to_the_sound_card() {
        if cpal::default_host().default_output_device().is_none() || picasso_media::tools::check_tools().is_err() {
            eprintln!("skipped: no sound card or no FFmpeg");
            return;
        }
        let dir = std::env::temp_dir().join(format!("picasso-audio-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let tone = dir.join("tone.wav");
        let ok = command(Tool::Ffmpeg)
            .args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", "sine=frequency=440:duration=4"])
            .arg(&tone)
            .status()
            .unwrap()
            .success();
        assert!(ok);
        let fps = Rational::new(25, 1);
        let mut project = Project::new("t", ProjectSettings { fps, ..Default::default() });
        let media = picasso_media::probe(&tone, fps).unwrap();
        let id = media.id;
        project.media.push(media);
        project.append_media(id).unwrap();

        let player = AudioPlayer::start(&project, 25, Instant::now()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1500));
        let peak = f32::from_bits(player.shared.peak.load(Ordering::Relaxed));
        drop(player);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(peak > 0.05, "the sound card got no sound (peak {peak})");
    }
}
