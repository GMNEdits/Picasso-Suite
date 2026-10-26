//! Recording sound from the default input device (a microphone or a line in). The device's
//! callback hands fixed-size blocks of stereo samples to the interface through a bounded
//! channel, so it never allocates or waits; the interface gathers them.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream, StreamConfig};

/// Stereo frames in one block handed over.
const BLOCK_FRAMES: usize = 512;
/// Blocks the channel holds before the callback has to drop sound (about 10 s at 48 kHz).
const QUEUE_BLOCKS: usize = 1000;

type Block = ([f32; BLOCK_FRAMES * 2], usize);

/// A recording in progress. Dropping it stops the device.
pub struct Recorder {
    _stream: Stream,
    blocks: Receiver<Block>,
    /// The device's sample rate.
    pub sample_rate: u32,
    /// The loudest sample since the level was last read, as `f32` bits.
    level: Arc<AtomicU32>,
    /// Blocks dropped because the interface fell behind.
    dropped: Arc<AtomicU64>,
}

impl Recorder {
    /// Starts recording from the default input device.
    pub fn start() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("there is no sound input device (a microphone)")?;
        let config = device.default_input_config().map_err(|e| e.to_string())?;
        let (tx, rx) = sync_channel::<Block>(QUEUE_BLOCKS);
        let level = Arc::new(AtomicU32::new(0));
        let dropped = Arc::new(AtomicU64::new(0));
        let sample_rate = config.sample_rate();
        let format = config.sample_format();
        let config: StreamConfig = config.into();
        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, config, tx, &level, &dropped),
            SampleFormat::I16 => build::<i16>(&device, config, tx, &level, &dropped),
            SampleFormat::I32 => build::<i32>(&device, config, tx, &level, &dropped),
            SampleFormat::U16 => build::<u16>(&device, config, tx, &level, &dropped),
            other => Err(format!("the input format {other} is not supported")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Self {
            _stream: stream,
            blocks: rx,
            sample_rate,
            level,
            dropped,
        })
    }

    /// Moves what was recorded since the last call to the end of `out` (interleaved stereo).
    pub fn drain(&self, out: &mut Vec<f32>) {
        while let Ok((block, frames)) = self.blocks.try_recv() {
            out.extend_from_slice(&block[..frames * 2]);
        }
    }

    /// The loudest sample since the last call (0 to 1).
    pub fn take_level(&self) -> f32 {
        f32::from_bits(self.level.swap(0, Ordering::Relaxed))
    }

    /// How many blocks were lost because they were not gathered in time.
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    tx: SyncSender<Block>,
    level: &Arc<AtomicU32>,
    dropped: &Arc<AtomicU64>,
) -> Result<Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels).max(1);
    let level = level.clone();
    let dropped = dropped.clone();
    let mut block: Block = ([0.0; BLOCK_FRAMES * 2], 0);
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                let mut peak = 0.0f32;
                for frame in data.chunks_exact(channels) {
                    // Mono comes in both ears; past two channels only the first two are kept.
                    let l: f32 = frame[0].to_sample();
                    let r: f32 = frame.get(1).map_or(l, |s| (*s).to_sample());
                    peak = peak.max(l.abs()).max(r.abs());
                    let n = block.1;
                    block.0[n * 2] = l;
                    block.0[n * 2 + 1] = r;
                    block.1 += 1;
                    if block.1 == BLOCK_FRAMES {
                        if let Err(TrySendError::Full(_)) = tx.try_send(block) {
                            dropped.fetch_add(1, Ordering::Relaxed);
                        }
                        block.1 = 0;
                    }
                }
                level.fetch_max(peak.to_bits(), Ordering::Relaxed);
            },
            |e| tracing::warn!("recording: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}

/// A WAV file (32-bit float, stereo) of `samples` (interleaved) at `sample_rate`.
pub fn wav_file(samples: &[f32], sample_rate: u32) -> Option<Vec<u8>> {
    let frames = (samples.len() / 2) as u64;
    let mut out = crate::wav::header(crate::WavFormat::Float32, sample_rate, frames)?;
    crate::wav::encode(
        crate::WavFormat::Float32,
        &samples[..frames as usize * 2],
        &mut out,
    );
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_take_is_written_as_a_wav_file() {
        let samples: Vec<f32> = (0..96).map(|i| i as f32 / 96.0).collect();
        let file = wav_file(&samples, 44_100).unwrap();
        assert_eq!(&file[..4], b"RIFF");
        assert_eq!(file.len(), 44 + 96 * 4);
        assert_eq!(
            u32::from_le_bytes([file[24], file[25], file[26], file[27]]),
            44_100
        );
    }
}
