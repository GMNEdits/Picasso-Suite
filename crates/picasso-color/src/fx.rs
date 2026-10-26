//! Video effects on a clip's picture, after its grade: blurs, sharpening, glow, mosaic, colour
//! looks, a vignette and film grain. Each effect has one strength, 0 to 100; sizes are in
//! thousandths of the picture's height, so an effect looks the same at any resolution.

use serde::{Deserialize, Serialize};

use crate::LUMA;

/// The most effects one clip has.
pub const MAX_EFFECTS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EffectKind {
    GaussianBlur,
    Sharpen,
    Mosaic,
    Invert,
    BlackAndWhite,
    Sepia,
    Glow,
    Vignette,
    FilmGrain,
}

impl EffectKind {
    pub const ALL: [Self; 9] = [
        Self::GaussianBlur,
        Self::Sharpen,
        Self::Mosaic,
        Self::Invert,
        Self::BlackAndWhite,
        Self::Sepia,
        Self::Glow,
        Self::Vignette,
        Self::FilmGrain,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::GaussianBlur => "Gaussian Blur",
            Self::Sharpen => "Sharpen",
            Self::Mosaic => "Mosaic",
            Self::Invert => "Invert Colors",
            Self::BlackAndWhite => "Black and White",
            Self::Sepia => "Sepia",
            Self::Glow => "Glow",
            Self::Vignette => "Vignette",
            Self::FilmGrain => "Film Grain",
        }
    }

    /// The strength a new effect starts with.
    pub fn default_amount(self) -> f32 {
        match self {
            Self::GaussianBlur => 20.0,
            Self::Sharpen => 50.0,
            Self::Mosaic => 20.0,
            Self::Invert | Self::BlackAndWhite | Self::Sepia => 100.0,
            Self::Glow => 50.0,
            Self::Vignette => 50.0,
            Self::FilmGrain => 30.0,
        }
    }
}

/// One effect of a clip.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    pub kind: EffectKind,
    /// The strength, 0 to 100.
    pub amount: f32,
    #[serde(default = "on")]
    pub enabled: bool,
}

fn on() -> bool {
    true
}

impl Effect {
    pub fn new(kind: EffectKind) -> Self {
        Self {
            kind,
            amount: kind.default_amount(),
            enabled: true,
        }
    }

    pub fn sanitized(mut self) -> Self {
        self.amount = if self.amount.is_finite() {
            self.amount.clamp(0.0, 100.0)
        } else {
            self.kind.default_amount()
        };
        self
    }

    /// Applies the effect to `rgba` (`w` x `h`, alpha kept). `frame` seeds the film grain, so
    /// it moves from frame to frame.
    pub fn apply(&self, rgba: &mut [u8], w: usize, h: usize, frame: u64) {
        let Some(len) = w.checked_mul(h) else {
            return;
        };
        if !self.enabled || self.amount <= 0.0 || len == 0 || rgba.len() < len * 4 {
            return;
        }
        let rgba = &mut rgba[..len * 4];
        let a = self.amount / 100.0;
        let unit = h as f32 / 1000.0;
        match self.kind {
            EffectKind::GaussianBlur => {
                let radius = (a * 40.0 * unit).round() as usize;
                if radius > 0 {
                    gaussian(rgba, w, h, radius);
                }
            }
            EffectKind::Sharpen => {
                let mut soft = rgba.to_vec();
                gaussian(&mut soft, w, h, (2.0 * unit).round().max(1.0) as usize);
                let k = a * 2.0;
                for (px, s) in rgba.chunks_exact_mut(4).zip(soft.chunks_exact(4)) {
                    for c in 0..3 {
                        let v = f32::from(px[c]) + (f32::from(px[c]) - f32::from(s[c])) * k;
                        px[c] = v.round().clamp(0.0, 255.0) as u8;
                    }
                }
            }
            EffectKind::Mosaic => {
                let size = ((a * 80.0 * unit).round() as usize).max(1);
                if size > 1 {
                    mosaic(rgba, w, h, size);
                }
            }
            EffectKind::Invert => per_pixel(rgba, a, |rgb| rgb.map(|v| 1.0 - v)),
            EffectKind::BlackAndWhite => per_pixel(rgba, a, |rgb| [luma(rgb); 3]),
            EffectKind::Sepia => per_pixel(rgba, a, |[r, g, b]| {
                [
                    (0.393 * r + 0.769 * g + 0.189 * b).min(1.0),
                    (0.349 * r + 0.686 * g + 0.168 * b).min(1.0),
                    (0.272 * r + 0.534 * g + 0.131 * b).min(1.0),
                ]
            }),
            EffectKind::Glow => {
                // The bright parts, blurred, added back over the picture (screen).
                let mut bright: Vec<u8> = rgba
                    .chunks_exact(4)
                    .flat_map(|px| {
                        let y = luma([0, 1, 2].map(|c| f32::from(px[c]) / 255.0));
                        let keep = ((y - 0.6) / 0.4).clamp(0.0, 1.0);
                        [
                            (f32::from(px[0]) * keep) as u8,
                            (f32::from(px[1]) * keep) as u8,
                            (f32::from(px[2]) * keep) as u8,
                            px[3],
                        ]
                    })
                    .collect();
                gaussian(&mut bright, w, h, (20.0 * unit).round().max(1.0) as usize);
                for (px, g) in rgba.chunks_exact_mut(4).zip(bright.chunks_exact(4)) {
                    for c in 0..3 {
                        let (x, y) = (f32::from(px[c]) / 255.0, f32::from(g[c]) / 255.0 * a);
                        px[c] = ((1.0 - (1.0 - x) * (1.0 - y)) * 255.0).round() as u8;
                    }
                }
            }
            EffectKind::Vignette => {
                let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
                let reach = (cx * cx + cy * cy).sqrt().max(1.0);
                for (i, px) in rgba.chunks_exact_mut(4).enumerate() {
                    let (x, y) = ((i % w) as f32 + 0.5 - cx, (i / w) as f32 + 0.5 - cy);
                    let d = (x * x + y * y).sqrt() / reach;
                    let t = ((d - 0.45) / 0.55).clamp(0.0, 1.0);
                    let dark = 1.0 - a * t * t * (3.0 - 2.0 * t);
                    for v in &mut px[..3] {
                        *v = (f32::from(*v) * dark).round() as u8;
                    }
                }
            }
            EffectKind::FilmGrain => {
                let strength = a * 40.0;
                for (i, px) in rgba.chunks_exact_mut(4).enumerate() {
                    let n = noise(i as u64, frame) * strength;
                    for v in &mut px[..3] {
                        *v = (f32::from(*v) + n).round().clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }
    }
}

fn luma(rgb: [f32; 3]) -> f32 {
    rgb[0] * LUMA[0] + rgb[1] * LUMA[1] + rgb[2] * LUMA[2]
}

/// Mixes each pixel towards `look` of it by `a` (0 to 1).
fn per_pixel(rgba: &mut [u8], a: f32, look: impl Fn([f32; 3]) -> [f32; 3]) {
    for px in rgba.chunks_exact_mut(4) {
        let rgb = [0, 1, 2].map(|c| f32::from(px[c]) / 255.0);
        let out = look(rgb);
        for c in 0..3 {
            let v = rgb[c] + (out[c] - rgb[c]) * a;
            px[c] = (v * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// Noise from -1 to 1 for pixel `i` of frame `frame` (a hash, the same every time).
fn noise(i: u64, frame: u64) -> f32 {
    let mut x = i ^ frame.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    x ^= x >> 33;
    (x & 0xffff) as f32 / 32767.5 - 1.0
}

/// Blocks of `size` pixels, each the average of its pixels.
fn mosaic(rgba: &mut [u8], w: usize, h: usize, size: usize) {
    for by in (0..h).step_by(size) {
        for bx in (0..w).step_by(size) {
            let (x1, y1) = ((bx + size).min(w), (by + size).min(h));
            let mut sum = [0u64; 4];
            let mut n = 0u64;
            for y in by..y1 {
                for x in bx..x1 {
                    let i = (y * w + x) * 4;
                    for c in 0..4 {
                        sum[c] += u64::from(rgba[i + c]);
                    }
                    n += 1;
                }
            }
            let mean = sum.map(|s| (s / n.max(1)) as u8);
            for y in by..y1 {
                for x in bx..x1 {
                    let i = (y * w + x) * 4;
                    rgba[i..i + 4].copy_from_slice(&mean);
                }
            }
        }
    }
}

/// A blur close to a Gaussian: three box blurs of `radius / 2` (rows, then columns).
fn gaussian(rgba: &mut [u8], w: usize, h: usize, radius: usize) {
    let r = (radius / 2).max(1).min(w.max(h));
    let mut buf: Vec<f32> = rgba.iter().map(|v| f32::from(*v)).collect();
    for _ in 0..3 {
        box_pass(&mut buf, w, h, r, true);
        box_pass(&mut buf, w, h, r, false);
    }
    for (v, b) in rgba.iter_mut().zip(buf) {
        *v = b.round().clamp(0.0, 255.0) as u8;
    }
}

/// One box blur of RGBA floats along rows (`along_rows`) or columns, edges held.
fn box_pass(buf: &mut [f32], w: usize, h: usize, r: usize, along_rows: bool) {
    let (lines, len) = if along_rows { (h, w) } else { (w, h) };
    let at = |line: usize, i: usize| {
        if along_rows {
            (line * w + i) * 4
        } else {
            (i * w + line) * 4
        }
    };
    let mut row = vec![0f32; len * 4];
    let window = (2 * r + 1) as f32;
    for line in 0..lines {
        for i in 0..len {
            row[i * 4..i * 4 + 4].copy_from_slice(&buf[at(line, i)..at(line, i) + 4]);
        }
        let get = |i: i64, c: usize| row[(i.clamp(0, len as i64 - 1) as usize) * 4 + c];
        for c in 0..4 {
            let mut sum: f32 = (-(r as i64)..=r as i64).map(|i| get(i, c)).sum();
            for i in 0..len {
                buf[at(line, i) + c] = sum / window;
                sum += get(i as i64 + r as i64 + 1, c) - get(i as i64 - r as i64, c);
            }
        }
    }
}

/// Applies `effects` in order.
pub fn apply_effects(effects: &[Effect], rgba: &mut [u8], w: usize, h: usize, frame: u64) {
    for e in effects {
        e.apply(rgba, w, h, frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 32 x 32 picture: left half black, right half white.
    fn halves() -> Vec<u8> {
        (0..32 * 32)
            .flat_map(|i| {
                let v = if i % 32 < 16 { 0 } else { 255 };
                [v, v, v, 255]
            })
            .collect()
    }

    fn at(p: &[u8], x: usize, y: usize) -> u8 {
        p[(y * 32 + x) * 4]
    }

    #[test]
    fn blur_softens_the_edge_and_sharpen_hardens_it() {
        let mut p = halves();
        Effect::new(EffectKind::GaussianBlur).apply(&mut p, 32, 32, 0);
        // A 32-pixel-high picture: strength 100 for a visible blur.
        let mut q = halves();
        Effect {
            amount: 100.0,
            ..Effect::new(EffectKind::GaussianBlur)
        }
        .apply(&mut q, 32, 32, 0);
        assert!(
            at(&q, 15, 16) > 0 && at(&q, 16, 16) < 255,
            "the edge is soft"
        );
        assert_eq!(at(&q, 0, 0), 0, "far from the edge it stays");
        let mut grey: Vec<u8> = halves().iter().map(|v| v / 2 + 64).collect();
        let before = grey.clone();
        Effect::new(EffectKind::Sharpen).apply(&mut grey, 32, 32, 0);
        assert!(
            at(&grey, 15, 16) < at(&before, 15, 16),
            "darker on the dark side"
        );
        assert!(
            at(&grey, 16, 16) > at(&before, 16, 16),
            "brighter on the bright side"
        );
    }

    #[test]
    fn colour_looks() {
        let red = || vec![200u8, 40, 40, 255];
        let mut p = red();
        Effect::new(EffectKind::Invert).apply(&mut p, 1, 1, 0);
        assert_eq!(p, vec![55, 215, 215, 255]);
        let mut p = red();
        Effect::new(EffectKind::BlackAndWhite).apply(&mut p, 1, 1, 0);
        assert!(p[0] == p[1] && p[1] == p[2]);
        let mut p = red();
        Effect::new(EffectKind::Sepia).apply(&mut p, 1, 1, 0);
        assert!(p[0] > p[1] && p[1] > p[2], "warm brown");
        let mut p = red();
        Effect {
            enabled: false,
            ..Effect::new(EffectKind::Invert)
        }
        .apply(&mut p, 1, 1, 0);
        assert_eq!(p, red(), "switched off");
    }

    #[test]
    fn vignette_mosaic_glow_and_grain() {
        let mut p = vec![200u8; 32 * 32 * 4];
        Effect {
            amount: 100.0,
            ..Effect::new(EffectKind::Vignette)
        }
        .apply(&mut p, 32, 32, 0);
        assert!(at(&p, 0, 0) < 100 && at(&p, 16, 16) == 200);
        let mut p = halves();
        Effect {
            amount: 100.0,
            ..Effect::new(EffectKind::Mosaic)
        }
        .apply(&mut p, 32, 32, 0);
        assert_eq!(at(&p, 0, 0), at(&p, 1, 1));
        let mut p = halves();
        Effect::new(EffectKind::Glow).apply(&mut p, 32, 32, 0);
        assert!(at(&p, 15, 16) > 0, "light spills into the dark");
        let (mut a, mut b) = (vec![128u8; 64 * 4], vec![128u8; 64 * 4]);
        Effect::new(EffectKind::FilmGrain).apply(&mut a, 8, 8, 1);
        Effect::new(EffectKind::FilmGrain).apply(&mut b, 8, 8, 2);
        assert_ne!(a, b, "the grain moves");
        assert!(a.chunks_exact(4).any(|p| p[0] != 128));
        // Sizes that do not fit are left alone.
        let mut tiny = vec![1u8; 3];
        Effect::new(EffectKind::GaussianBlur).apply(&mut tiny, 4, 4, 0);
        assert_eq!(tiny, vec![1, 1, 1]);
    }
}
