//! Picture format conversion for the encoders: 8-bit RGBA to BT.709 YUV, at limited ("TV") range or
//! full range.
//! Done here, not with FFmpeg's scaler, so the matrix is the one HD video uses.

/// BT.709 Y, Cb and Cr of a colour with channels in 0.0..=255.0, as numbers on the 8-bit scale:
/// limited ("video") range is Y 16..=235 and Cb and Cr 16..=240, full range uses all 0..=255.
fn yuv709(r: f32, g: f32, b: f32, full: bool) -> (f32, f32, f32) {
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let cb = (b - y) / 1.8556;
    let cr = (r - y) / 1.5748;
    if full {
        return (y, 128.0 + cb, 128.0 + cr);
    }
    (
        16.0 + y * 219.0 / 255.0,
        128.0 + cb * 224.0 / 255.0,
        128.0 + cr * 224.0 / 255.0,
    )
}

/// The pixel at (x, y) of an RGBA picture as floats; coordinates past the edge repeat the edge.
fn px(rgba: &[u8], w: usize, h: usize, x: usize, y: usize) -> [f32; 3] {
    let (x, y) = (x.min(w.saturating_sub(1)), y.min(h.saturating_sub(1)));
    let i = (y * w + x) * 4;
    match rgba.get(i..i + 3) {
        Some(p) => [f32::from(p[0]), f32::from(p[1]), f32::from(p[2])],
        None => [0.0; 3],
    }
}

fn byte(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

/// How an encoder wants a picture laid out in memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    /// How many pixels across and down one colour (Cb, Cr) sample covers: 2 x 2 is 4:2:0, 2 x 1
    /// 4:2:2 and 1 x 1 4:4:4.
    pub step: (usize, usize),
    /// 10 bits a sample in 16-bit little-endian words, instead of 8 bits in a byte.
    pub ten_bit: bool,
    /// Cb and Cr take turns in one plane (NV12, P010), instead of a plane each; their 10 bits
    /// are then the top ones of the word (P010).
    pub interleaved: bool,
}

impl Layout {
    /// The layout of an FFmpeg pixel format the encoders here use.
    pub fn of(pixel: ffmpeg_next::format::Pixel) -> Option<Self> {
        use ffmpeg_next::format::Pixel as P;
        let (step, ten_bit, interleaved) = match pixel {
            P::YUV420P => ((2, 2), false, false),
            P::YUV420P10LE => ((2, 2), true, false),
            P::YUV422P => ((2, 1), false, false),
            P::YUV422P10LE => ((2, 1), true, false),
            P::YUV444P10LE => ((1, 1), true, false),
            P::NV12 => ((2, 2), false, true),
            P::P010LE => ((2, 2), true, true),
            _ => return None,
        };
        Some(Self {
            step,
            ten_bit,
            interleaved,
        })
    }
}

/// Converts an RGBA picture of `w * h * 4` bytes to YUV in `layout`. `planes` are the
/// destination planes (two when interleaved, else three) and `strides` their row lengths in
/// bytes. A size that does not divide by the colour step repeats the last row and column.
pub fn rgba_to_yuv(
    rgba: &[u8],
    (w, h): (usize, usize),
    layout: Layout,
    planes: &mut [&mut [u8]],
    strides: &[usize],
    full: bool,
) {
    let bytes = if layout.ten_bit { 2 } else { 1 };
    // Writes a value on the 8-bit scale as a sample of the layout.
    let put = |plane: &mut [u8], at: usize, v: f32| {
        if layout.ten_bit {
            let mut word = (v * 4.0).round().clamp(0.0, 1023.0) as u16;
            if layout.interleaved {
                word <<= 6;
            }
            if let Some(dst) = plane.get_mut(at..at + 2) {
                dst.copy_from_slice(&word.to_le_bytes());
            }
        } else if let Some(slot) = plane.get_mut(at) {
            *slot = byte(v);
        }
    };
    let (sx, sy) = (layout.step.0.max(1), layout.step.1.max(1));
    let stride = |i: usize| strides.get(i).copied().unwrap_or(0);
    for cy in 0..h.div_ceil(sy) {
        for cx in 0..w.div_ceil(sx) {
            let (mut r, mut g, mut b) = (0.0, 0.0, 0.0);
            for dy in 0..sy {
                for dx in 0..sx {
                    let (x, y) = (cx * sx + dx, cy * sy + dy);
                    let p = px(rgba, w, h, x, y);
                    if x < w && y < h {
                        if let Some(plane) = planes.get_mut(0) {
                            put(
                                plane,
                                y * stride(0) + x * bytes,
                                yuv709(p[0], p[1], p[2], full).0,
                            );
                        }
                    }
                    r += p[0];
                    g += p[1];
                    b += p[2];
                }
            }
            let n = (sx * sy) as f32;
            let (_, cb, cr) = yuv709(r / n, g / n, b / n, full);
            if layout.interleaved {
                let at = cy * stride(1) + cx * 2 * bytes;
                if let Some(plane) = planes.get_mut(1) {
                    put(plane, at, cb);
                    put(plane, at + bytes, cr);
                }
            } else {
                for (i, v) in [(1, cb), (2, cr)] {
                    let at = cy * stride(i) + cx * bytes;
                    if let Some(plane) = planes.get_mut(i) {
                        put(plane, at, v);
                    }
                }
            }
        }
    }
}

/// Converts to planar YUV 4:2:0, 8 bits (see [`rgba_to_yuv`]).
#[cfg(test)]
pub fn rgba_to_yuv420p(
    rgba: &[u8],
    size: (usize, usize),
    planes: [&mut [u8]; 3],
    strides: [usize; 3],
    full: bool,
) {
    let layout = Layout {
        step: (2, 2),
        ten_bit: false,
        interleaved: false,
    };
    let mut planes = planes;
    rgba_to_yuv(rgba, size, layout, &mut planes, &strides, full);
}

/// Converts to planar YUV 4:2:2 with 10 bits per sample, stored in 16-bit little-endian words
/// (what ProRes 422 takes). `strides` are in bytes.
#[cfg(test)]
pub fn rgba_to_yuv422p10(
    rgba: &[u8],
    size: (usize, usize),
    planes: [&mut [u8]; 3],
    strides: [usize; 3],
    full: bool,
) {
    let layout = Layout {
        step: (2, 1),
        ten_bit: true,
        interleaved: false,
    };
    let mut planes = planes;
    rgba_to_yuv(rgba, size, layout, &mut planes, &strides, full);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: usize, h: usize, rgb: [u8; 3]) -> Vec<u8> {
        [rgb[0], rgb[1], rgb[2], 255].repeat(w * h)
    }

    fn yuv420(rgb: [u8; 3]) -> (u8, u8, u8) {
        let (mut y, mut u, mut v) = ([0u8; 16], [0u8; 4], [0u8; 4]);
        rgba_to_yuv420p(
            &solid(4, 4, rgb),
            (4, 4),
            [&mut y, &mut u, &mut v],
            [4, 2, 2],
            false,
        );
        assert!(y.iter().all(|p| *p == y[0]), "a flat colour has a flat Y");
        (y[0], u[0], v[0])
    }

    #[test]
    fn the_hd_matrix_and_the_limited_range() {
        assert_eq!(yuv420([0, 0, 0]), (16, 128, 128), "black");
        assert_eq!(yuv420([255, 255, 255]), (235, 128, 128), "white");
        assert_eq!(yuv420([255, 0, 0]), (63, 102, 240), "red");
        assert_eq!(yuv420([0, 255, 0]), (173, 42, 26), "green");
        assert_eq!(yuv420([0, 0, 255]), (32, 240, 118), "blue");
    }

    #[test]
    fn full_range_uses_every_value() {
        let full = |rgb: [u8; 3]| {
            let (mut y, mut u, mut v) = ([0u8; 16], [0u8; 4], [0u8; 4]);
            rgba_to_yuv420p(
                &solid(4, 4, rgb),
                (4, 4),
                [&mut y, &mut u, &mut v],
                [4, 2, 2],
                true,
            );
            (y[0], u[0], v[0])
        };
        assert_eq!(full([0, 0, 0]), (0, 128, 128), "black");
        assert_eq!(full([255, 255, 255]), (255, 128, 128), "white");
    }

    #[test]
    fn chroma_is_shared_by_each_two_by_two_block() {
        // Left half red, right half blue, in a 4 x 2 picture: each chroma sample covers 2 x 2.
        let mut rgba = Vec::new();
        for _ in 0..2 {
            for x in 0..4 {
                rgba.extend_from_slice(if x < 2 {
                    &[255, 0, 0, 255]
                } else {
                    &[0, 0, 255, 255]
                });
            }
        }
        let (mut y, mut u, mut v) = ([0u8; 8], [0u8; 2], [0u8; 2]);
        rgba_to_yuv420p(&rgba, (4, 2), [&mut y, &mut u, &mut v], [4, 2, 2], false);
        assert_eq!((y[0], y[3]), (63, 32));
        assert_eq!((u, v), ([102, 240], [240, 118]));
    }

    #[test]
    fn odd_sizes_do_not_overrun() {
        let (mut y, mut u, mut v) = ([0u8; 15], [0u8; 6], [0u8; 6]);
        rgba_to_yuv420p(
            &solid(5, 3, [255, 255, 255]),
            (5, 3),
            [&mut y, &mut u, &mut v],
            [5, 3, 3],
            false,
        );
        assert!(y.iter().all(|p| *p == 235));
        assert!(u.iter().all(|p| *p == 128));
    }

    #[test]
    fn nv12_and_p010_put_cb_and_cr_side_by_side() {
        let white = solid(4, 2, [255, 255, 255]);
        let nv12 = Layout::of(ffmpeg_next::format::Pixel::NV12).unwrap();
        let (mut y, mut uv) = ([0u8; 8], [0u8; 4]);
        rgba_to_yuv(
            &solid(4, 2, [255, 0, 0]),
            (4, 2),
            nv12,
            &mut [&mut y, &mut uv],
            &[4, 4],
            false,
        );
        assert_eq!(y[0], 63);
        assert_eq!(uv, [102, 240, 102, 240], "Cb then Cr for each 2 x 2 block");
        let p010 = Layout::of(ffmpeg_next::format::Pixel::P010LE).unwrap();
        let (mut y, mut uv) = ([0u8; 16], [0u8; 8]);
        rgba_to_yuv(&white, (4, 2), p010, &mut [&mut y, &mut uv], &[8, 8], false);
        let word = |p: &[u8], i: usize| u16::from_le_bytes([p[i * 2], p[i * 2 + 1]]);
        assert_eq!(word(&y, 0), 940 << 6, "10 bits in the top of the word");
        assert_eq!(word(&uv, 0), 512 << 6);
        assert_eq!(word(&uv, 1), 512 << 6);
    }

    #[test]
    fn four_four_four_keeps_every_colour_sample() {
        let layout = Layout::of(ffmpeg_next::format::Pixel::YUV444P10LE).unwrap();
        let mut rgba = solid(2, 1, [255, 0, 0]);
        rgba[4..8].copy_from_slice(&[0, 0, 255, 255]);
        let (mut y, mut u, mut v) = ([0u8; 4], [0u8; 4], [0u8; 4]);
        rgba_to_yuv(
            &rgba,
            (2, 1),
            layout,
            &mut [&mut y, &mut u, &mut v],
            &[4, 4, 4],
            false,
        );
        let word = |p: &[u8], i: usize| u16::from_le_bytes([p[i * 2], p[i * 2 + 1]]);
        assert_eq!(
            (word(&u, 0), word(&u, 1)),
            (409, 960),
            "red, then blue, each its own"
        );
        assert!(Layout::of(ffmpeg_next::format::Pixel::RGBA).is_none());
    }

    #[test]
    fn ten_bit_422_has_four_times_the_resolution_of_eight_bit() {
        let rgba = solid(4, 2, [255, 255, 255]);
        let (mut y, mut u, mut v) = ([0u8; 16], [0u8; 8], [0u8; 8]);
        rgba_to_yuv422p10(&rgba, (4, 2), [&mut y, &mut u, &mut v], [8, 4, 4], false);
        let word = |p: &[u8], i: usize| u16::from_le_bytes([p[i * 2], p[i * 2 + 1]]);
        assert_eq!(word(&y, 0), 940, "235 * 4");
        assert_eq!(word(&u, 0), 512, "128 * 4");
        let mut black = [0u8; 16];
        rgba_to_yuv422p10(
            &solid(4, 2, [0, 0, 0]),
            (4, 2),
            [&mut black, &mut [0u8; 8], &mut [0u8; 8]],
            [8, 4, 4],
            false,
        );
        assert_eq!(word(&black, 7), 64, "16 * 4");
    }
}
