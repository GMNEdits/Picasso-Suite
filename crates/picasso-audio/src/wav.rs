//! Writing WAV files: a header, then blocks of samples.

/// How the samples are stored in the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WavFormat {
    Pcm16,
    Pcm24,
    Float32,
}

impl WavFormat {
    pub fn bytes_per_sample(self) -> usize {
        match self {
            Self::Pcm16 => 2,
            Self::Pcm24 => 3,
            Self::Float32 => 4,
        }
    }

    /// A short name for menus.
    pub fn label(self) -> &'static str {
        match self {
            Self::Pcm16 => "16-bit PCM",
            Self::Pcm24 => "24-bit PCM",
            Self::Float32 => "32-bit float",
        }
    }

    pub const ALL: [Self; 3] = [Self::Pcm16, Self::Pcm24, Self::Float32];
}

/// The most sample data a WAV file can hold (its sizes are 32 bits).
const MAX_DATA_BYTES: u64 = u32::MAX as u64 - 64;

/// The 44 bytes that start a stereo WAV file of `frames` samples per channel, or `None` when
/// that would not fit in a WAV file.
pub fn header(format: WavFormat, sample_rate: u32, frames: u64) -> Option<Vec<u8>> {
    let channels = 2u16;
    let bytes = format.bytes_per_sample() as u64;
    let data = frames.checked_mul(bytes * u64::from(channels))?;
    if data > MAX_DATA_BYTES {
        return None;
    }
    let data = data as u32;
    let block_align = channels * format.bytes_per_sample() as u16;
    let tag: u16 = if format == WavFormat::Float32 { 3 } else { 1 };
    let mut h = Vec::with_capacity(44);
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(36 + data).to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&16u32.to_le_bytes());
    h.extend_from_slice(&tag.to_le_bytes());
    h.extend_from_slice(&channels.to_le_bytes());
    h.extend_from_slice(&sample_rate.to_le_bytes());
    h.extend_from_slice(&(sample_rate * u32::from(block_align)).to_le_bytes());
    h.extend_from_slice(&block_align.to_le_bytes());
    h.extend_from_slice(&((format.bytes_per_sample() * 8) as u16).to_le_bytes());
    h.extend_from_slice(b"data");
    h.extend_from_slice(&data.to_le_bytes());
    Some(h)
}

/// Appends `samples` (any real numbers; integer formats clip at full scale) to `out` in `format`.
pub fn encode(format: WavFormat, samples: &[f32], out: &mut Vec<u8>) {
    for s in samples {
        match format {
            WavFormat::Float32 => out.extend_from_slice(&s.to_le_bytes()),
            WavFormat::Pcm16 => {
                let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
                out.extend_from_slice(&v.to_le_bytes());
            }
            WavFormat::Pcm24 => {
                let v = (s.clamp(-1.0, 1.0) * 8_388_607.0).round() as i32;
                out.extend_from_slice(&v.to_le_bytes()[..3]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u32_at(b: &[u8], i: usize) -> u32 {
        u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
    }

    #[test]
    fn the_header_describes_the_data() {
        let h = header(WavFormat::Pcm24, 48_000, 100).unwrap();
        assert_eq!(h.len(), 44);
        assert_eq!(&h[..4], b"RIFF");
        assert_eq!(&h[8..16], b"WAVEfmt ");
        // 100 frames * 2 channels * 3 bytes.
        assert_eq!(u32_at(&h, 40), 600);
        assert_eq!(u32_at(&h, 4), 636);
        assert_eq!(u32_at(&h, 24), 48_000);
        assert_eq!(u32_at(&h, 28), 48_000 * 6, "bytes per second");
        assert_eq!(u16::from_le_bytes([h[20], h[21]]), 1, "PCM");
        let f = header(WavFormat::Float32, 48_000, 1).unwrap();
        assert_eq!(u16::from_le_bytes([f[20], f[21]]), 3, "float");
    }

    #[test]
    fn too_much_data_for_a_wav_file_is_refused() {
        assert!(header(WavFormat::Float32, 48_000, u64::MAX).is_none());
        assert!(header(WavFormat::Float32, 48_000, 600_000_000).is_none());
        assert!(header(WavFormat::Pcm16, 48_000, 600_000_000).is_some());
    }

    #[test]
    fn samples_are_encoded_and_clipped() {
        let mut b = Vec::new();
        encode(WavFormat::Pcm16, &[0.0, 1.0, -1.0, 5.0, -5.0], &mut b);
        let v: Vec<i16> = b
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(v, [0, 32767, -32767, 32767, -32767]);
        let mut b = Vec::new();
        encode(WavFormat::Pcm24, &[1.0, -1.0], &mut b);
        assert_eq!(b, [0xFF, 0xFF, 0x7F, 0x01, 0x00, 0x80]);
        let mut b = Vec::new();
        encode(WavFormat::Float32, &[0.5], &mut b);
        assert_eq!(b, 0.5f32.to_le_bytes());
    }
}
