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

/// Converts to planar YUV 4:2:0, 8 bits. `planes` are the three destination planes and `strides`
/// their row lengths in bytes. The picture must be `w * h * 4` bytes; an odd size is rounded up
/// by repeating the last row and column.
pub fn rgba_to_yuv420p(
    rgba: &[u8],
    (w, h): (usize, usize),
    planes: [&mut [u8]; 3],
    strides: [usize; 3],
    full: bool,
) {
    let [yp, up, vp] = planes;
    for cy in 0..h.div_ceil(2) {
        for cx in 0..w.div_ceil(2) {
            let (mut r, mut g, mut b) = (0.0, 0.0, 0.0);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (x, y) = (cx * 2 + dx, cy * 2 + dy);
                let p = px(rgba, w, h, x, y);
                if x < w && y < h {
                    if let Some(slot) = yp.get_mut(y * strides[0] + x) {
                        *slot = byte(yuv709(p[0], p[1], p[2], full).0);
                    }
                }
                r += p[0];
                g += p[1];
                b += p[2];
            }
            let (_, cb, cr) = yuv709(r / 4.0, g / 4.0, b / 4.0, full);
            if let Some(slot) = up.get_mut(cy * strides[1] + cx) {
                *slot = byte(cb);
            }
            if let Some(slot) = vp.get_mut(cy * strides[2] + cx) {
                *slot = byte(cr);
            }
        }
    }
}

/// Converts to planar YUV 4:2:2 with 10 bits per sample, stored in 16-bit little-endian words
/// (what ProRes 422 takes). `strides` are in bytes.
pub fn rgba_to_yuv422p10(
    rgba: &[u8],
    (w, h): (usize, usize),
    planes: [&mut [u8]; 3],
    strides: [usize; 3],
    full: bool,
) {
    let [yp, up, vp] = planes;
    let put = |plane: &mut [u8], stride: usize, row: usize, col: usize, v: f32| {
        let word = ((v * 4.0).round().clamp(0.0, 1023.0) as u16).to_le_bytes();
        let at = row * stride + col * 2;
        if let Some(dst) = plane.get_mut(at..at + 2) {
            dst.copy_from_slice(&word);
        }
    };
    for y in 0..h {
        for cx in 0..w.div_ceil(2) {
            let (mut r, mut g, mut b) = (0.0, 0.0, 0.0);
            for dx in 0..2 {
                let x = cx * 2 + dx;
                let p = px(rgba, w, h, x, y);
                if x < w {
                    put(yp, strides[0], y, x, yuv709(p[0], p[1], p[2], full).0);
                }
                r += p[0];
                g += p[1];
                b += p[2];
            }
            let (_, cb, cr) = yuv709(r / 2.0, g / 2.0, b / 2.0, full);
            put(up, strides[1], y, cx, cb);
            put(vp, strides[2], y, cx, cr);
        }
    }
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
