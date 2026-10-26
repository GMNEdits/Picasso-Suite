//! Video scopes: histogram, waveform and vectorscope of an RGBA8 frame.
//!
//! Every function checks that the pixel buffer is as long as `width * height * 4` and returns an
//! empty scope when it is not. Sizes of the output are capped.

use crate::LUMA;

/// Most columns a waveform may have.
pub const MAX_WAVEFORM_COLUMNS: usize = 2048;
/// Largest side of a vectorscope.
pub const MAX_VECTORSCOPE_SIZE: usize = 512;

/// The pixels of a frame, or `None` when the buffer does not match the size.
fn pixels(rgba: &[u8], width: usize, height: usize) -> Option<&[u8]> {
    let len = width.checked_mul(height)?.checked_mul(4)?;
    rgba.get(..len)
}

fn luma8(px: &[u8]) -> usize {
    let y = f32::from(px[0]) * LUMA[0] + f32::from(px[1]) * LUMA[1] + f32::from(px[2]) * LUMA[2];
    (y + 0.5).min(255.0) as usize
}

/// How many pixels have each value, per channel and for the luma.
#[derive(Clone, Debug, PartialEq)]
pub struct Histogram {
    pub r: [u32; 256],
    pub g: [u32; 256],
    pub b: [u32; 256],
    pub luma: [u32; 256],
}

impl Histogram {
    /// The biggest bin of any channel, for scaling a plot.
    pub fn max(&self) -> u32 {
        [&self.r, &self.g, &self.b, &self.luma]
            .iter()
            .flat_map(|c| c.iter().copied())
            .max()
            .unwrap_or(0)
    }
}

/// Counts the values of a frame.
pub fn histogram(rgba: &[u8], width: usize, height: usize) -> Histogram {
    let mut h = Histogram {
        r: [0; 256],
        g: [0; 256],
        b: [0; 256],
        luma: [0; 256],
    };
    let Some(data) = pixels(rgba, width, height) else {
        return h;
    };
    for px in data.chunks_exact(4) {
        h.r[usize::from(px[0])] += 1;
        h.g[usize::from(px[1])] += 1;
        h.b[usize::from(px[2])] += 1;
        h.luma[luma8(px)] += 1;
    }
    h
}

/// Luma against picture column: `counts[column * 256 + level]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Waveform {
    pub columns: usize,
    pub counts: Vec<u32>,
    pub max: u32,
}

/// Measures the luma of every pixel, binned into `columns` picture columns.
pub fn waveform(rgba: &[u8], width: usize, height: usize, columns: usize) -> Waveform {
    let columns = columns.clamp(1, MAX_WAVEFORM_COLUMNS);
    let mut w = Waveform {
        columns,
        counts: vec![0; columns * 256],
        max: 0,
    };
    let Some(data) = pixels(rgba, width, height) else {
        return w;
    };
    if width == 0 {
        return w;
    }
    for row in data.chunks_exact(width * 4) {
        for (x, px) in row.chunks_exact(4).enumerate() {
            let column = x * columns / width;
            let cell = &mut w.counts[column * 256 + luma8(px)];
            *cell += 1;
            w.max = w.max.max(*cell);
        }
    }
    w
}

/// Chroma of a frame on a `size` x `size` grid: Cb across, Cr up, neutral in the middle.
#[derive(Clone, Debug, PartialEq)]
pub struct Vectorscope {
    pub size: usize,
    pub counts: Vec<u32>,
    pub max: u32,
}

/// Plots the Cb/Cr (Rec. 709) of every pixel.
pub fn vectorscope(rgba: &[u8], width: usize, height: usize, size: usize) -> Vectorscope {
    let size = size.clamp(2, MAX_VECTORSCOPE_SIZE);
    let mut v = Vectorscope {
        size,
        counts: vec![0; size * size],
        max: 0,
    };
    let Some(data) = pixels(rgba, width, height) else {
        return v;
    };
    let last = (size - 1) as f32;
    for px in data.chunks_exact(4) {
        let (r, g, b) = (
            f32::from(px[0]) / 255.0,
            f32::from(px[1]) / 255.0,
            f32::from(px[2]) / 255.0,
        );
        let y = r * LUMA[0] + g * LUMA[1] + b * LUMA[2];
        let cb = (b - y) / 1.8556; // -0.5..0.5
        let cr = (r - y) / 1.5748;
        let x = ((cb + 0.5) * last + 0.5).clamp(0.0, last) as usize;
        // Cr grows upwards, rows grow downwards.
        let row = ((0.5 - cr) * last + 0.5).clamp(0.0, last) as usize;
        let cell = &mut v.counts[row * size + x];
        *cell += 1;
        v.max = v.max.max(*cell);
    }
    v
}

/// Red, green and blue levels of a frame side by side ("parade"): for each channel, `columns`
/// columns of 256 counts, with level 0 at index 0.
#[derive(Clone, Debug, PartialEq)]
pub struct Parade {
    pub columns: usize,
    /// One waveform per channel, red, green, blue; `columns * 256` counts each.
    pub counts: [Vec<u32>; 3],
    /// The largest count of all three.
    pub max: u32,
}

/// Measures the parade of a frame with `columns` columns per channel.
pub fn parade(rgba: &[u8], width: usize, height: usize, columns: usize) -> Parade {
    let columns = columns.clamp(1, MAX_WAVEFORM_COLUMNS);
    let mut p = Parade {
        columns,
        counts: [0, 1, 2].map(|_| vec![0; columns * 256]),
        max: 0,
    };
    let Some(data) = pixels(rgba, width, height) else {
        return p;
    };
    if width == 0 {
        return p;
    }
    for row in data.chunks_exact(width * 4) {
        for (x, px) in row.chunks_exact(4).enumerate() {
            let column = x * columns / width;
            for (c, counts) in p.counts.iter_mut().enumerate() {
                let cell = &mut counts[column * 256 + usize::from(px[c])];
                *cell += 1;
                p.max = p.max.max(*cell);
            }
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: usize, h: usize, rgb: [u8; 3]) -> Vec<u8> {
        (0..w * h)
            .flat_map(|_| [rgb[0], rgb[1], rgb[2], 255])
            .collect()
    }

    #[test]
    fn histogram_counts_every_pixel() {
        let h = histogram(&solid(4, 3, [10, 20, 30]), 4, 3);
        assert_eq!((h.r[10], h.g[20], h.b[30]), (12, 12, 12));
        assert_eq!(h.luma.iter().sum::<u32>(), 12);
        assert_eq!(h.max(), 12);
    }

    #[test]
    fn short_buffer_gives_empty_scopes() {
        let short = vec![0u8; 8];
        assert_eq!(histogram(&short, 4, 3).max(), 0);
        assert_eq!(waveform(&short, 4, 3, 4).max, 0);
        assert_eq!(vectorscope(&short, 4, 3, 16).max, 0);
        // A size whose product overflows is refused, not wrapped.
        assert_eq!(histogram(&short, usize::MAX, 2).max(), 0);
    }

    #[test]
    fn waveform_of_a_flat_frame_is_one_level_per_column() {
        let w = waveform(&solid(8, 4, [128, 128, 128]), 8, 4, 4);
        assert_eq!(w.columns, 4);
        for column in 0..4 {
            let lit: Vec<usize> = (0..256)
                .filter(|l| w.counts[column * 256 + l] > 0)
                .collect();
            assert_eq!(lit, vec![128]);
            assert_eq!(w.counts[column * 256 + 128], 8); // 2 pixels wide x 4 rows
        }
    }

    #[test]
    fn vectorscope_puts_grey_in_the_middle_and_red_towards_cr() {
        let grey = vectorscope(&solid(2, 2, [90, 90, 90]), 2, 2, 65);
        assert_eq!(grey.counts[32 * 65 + 32], 4);
        let red = vectorscope(&solid(2, 2, [255, 0, 0]), 2, 2, 65);
        let (idx, _) = red
            .counts
            .iter()
            .enumerate()
            .find(|(_, c)| **c > 0)
            .unwrap();
        let (row, col) = (idx / 65, idx % 65);
        assert!(
            row < 32,
            "red has positive Cr, so it plots above the middle"
        );
        assert!(col <= 32, "red has slightly negative Cb");
    }

    #[test]
    fn output_sizes_are_capped() {
        assert_eq!(
            waveform(&[], 0, 0, usize::MAX).columns,
            MAX_WAVEFORM_COLUMNS
        );
        assert_eq!(
            vectorscope(&[], 0, 0, usize::MAX).size,
            MAX_VECTORSCOPE_SIZE
        );
    }

    #[test]
    fn parade_keeps_the_channels_apart() {
        let p = parade(&solid(4, 2, [200, 100, 0]), 4, 2, 2);
        assert_eq!(p.columns, 2);
        for column in 0..2 {
            assert_eq!(p.counts[0][column * 256 + 200], 4);
            assert_eq!(p.counts[1][column * 256 + 100], 4);
            assert_eq!(p.counts[2][column * 256], 4);
        }
        assert_eq!(p.max, 4);
        // A frame too small for its size is measured as empty, not read past its end.
        assert_eq!(parade(&[1, 2, 3], 4, 4, 8).max, 0);
    }
}
