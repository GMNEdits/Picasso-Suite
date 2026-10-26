//! Secondary correction: which part of the picture a node grades. A [`Qualifier`] keys the
//! pixels whose hue, saturation and luminance fall in its ranges; [`PowerWindow`]s are shapes
//! (an ellipse, a rectangle or a gradient). A node with either grades only inside its key: the
//! qualifier's key times the union of its windows, softened by the matte blur.

use serde::{Deserialize, Serialize};

use crate::LUMA;

/// The most windows one node has.
pub const MAX_WINDOWS: usize = 8;

/// The HSL qualifier of a node. Hue is in degrees (0 red, 120 green, 240 blue); the other
/// ranges are 0..1. Each range has a soft edge outside it, where the key falls to nothing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Qualifier {
    pub enabled: bool,
    /// The middle of the hue range, 0..360.
    pub hue_center: f32,
    /// How wide the hue range is, 0..360 (360 takes every hue).
    pub hue_width: f32,
    /// The hue's soft edge either side, in degrees.
    pub hue_soft: f32,
    pub sat_low: f32,
    pub sat_high: f32,
    pub sat_soft: f32,
    pub lum_low: f32,
    pub lum_high: f32,
    pub lum_soft: f32,
    /// Grades what is outside the key instead.
    pub invert: bool,
    /// Matte finesse: the blur radius of the key, 0..100 (hundredths of the picture's height).
    pub blur: f32,
    /// Matte finesse: key values under this become 0, 0..1.
    pub clean_black: f32,
    /// Matte finesse: key values over 1 minus this become 1, 0..1.
    pub clean_white: f32,
}

impl Default for Qualifier {
    fn default() -> Self {
        Self {
            enabled: true,
            hue_center: 0.0,
            hue_width: 360.0,
            hue_soft: 10.0,
            sat_low: 0.0,
            sat_high: 1.0,
            sat_soft: 0.05,
            lum_low: 0.0,
            lum_high: 1.0,
            lum_soft: 0.05,
            invert: false,
            blur: 0.0,
            clean_black: 0.0,
            clean_white: 0.0,
        }
    }
}

fn finite(v: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        fallback
    }
}

/// 1 inside `low..=high`, falling smoothly to 0 over `soft` outside.
fn range_key(v: f32, low: f32, high: f32, soft: f32) -> f32 {
    let below = if v >= low {
        1.0
    } else if soft <= 0.0 {
        0.0
    } else {
        smooth(1.0 - (low - v) / soft)
    };
    let above = if v <= high {
        1.0
    } else if soft <= 0.0 {
        0.0
    } else {
        smooth(1.0 - (v - high) / soft)
    };
    below.min(above)
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Hue (degrees), saturation and luminance of `rgb`.
pub fn hsl(rgb: [f32; 3]) -> (f32, f32, f32) {
    let max = rgb[0].max(rgb[1]).max(rgb[2]);
    let min = rgb[0].min(rgb[1]).min(rgb[2]);
    let d = max - min;
    let lum = rgb[0] * LUMA[0] + rgb[1] * LUMA[1] + rgb[2] * LUMA[2];
    if d <= 1e-6 {
        return (0.0, 0.0, lum);
    }
    let l = (max + min) / 2.0;
    let sat = d / (1.0 - (2.0 * l - 1.0).abs()).max(1e-6);
    let hue = if max == rgb[0] {
        60.0 * ((rgb[1] - rgb[2]) / d).rem_euclid(6.0)
    } else if max == rgb[1] {
        60.0 * ((rgb[2] - rgb[0]) / d + 2.0)
    } else {
        60.0 * ((rgb[0] - rgb[1]) / d + 4.0)
    };
    (hue, sat.clamp(0.0, 1.0), lum)
}

impl Qualifier {
    pub fn sanitized(mut self) -> Self {
        let d = Self::default();
        self.hue_center = finite(self.hue_center, d.hue_center).rem_euclid(360.0);
        self.hue_width = finite(self.hue_width, d.hue_width).clamp(0.0, 360.0);
        self.hue_soft = finite(self.hue_soft, d.hue_soft).clamp(0.0, 180.0);
        for (v, fallback) in [
            (&mut self.sat_low, d.sat_low),
            (&mut self.sat_high, d.sat_high),
            (&mut self.lum_low, d.lum_low),
            (&mut self.lum_high, d.lum_high),
        ] {
            *v = finite(*v, fallback).clamp(0.0, 1.0);
        }
        for v in [&mut self.sat_soft, &mut self.lum_soft] {
            *v = finite(*v, 0.0).clamp(0.0, 1.0);
        }
        self.blur = finite(self.blur, 0.0).clamp(0.0, 100.0);
        self.clean_black = finite(self.clean_black, 0.0).clamp(0.0, 0.99);
        self.clean_white = finite(self.clean_white, 0.0).clamp(0.0, 0.99);
        self
    }

    /// True when the qualifier keys every pixel fully (and so changes nothing).
    pub fn takes_all(&self) -> bool {
        !self.enabled
            || (!self.invert
                && self.hue_width >= 360.0
                && self.sat_low <= 0.0
                && self.sat_high >= 1.0
                && self.lum_low <= 0.0
                && self.lum_high >= 1.0)
    }

    /// How much of `rgb` (0..1) the qualifier keys, 0..1, before the matte finesse.
    pub fn key(&self, rgb: [f32; 3]) -> f32 {
        if !self.enabled {
            return 1.0;
        }
        let (h, s, l) = hsl(rgb);
        let hue = if self.hue_width >= 360.0 {
            1.0
        } else {
            let distance = (h - self.hue_center).rem_euclid(360.0);
            let distance = distance.min(360.0 - distance);
            range_key(distance, 0.0, self.hue_width / 2.0, self.hue_soft)
        };
        let k = hue
            * range_key(s, self.sat_low, self.sat_high, self.sat_soft)
            * range_key(l, self.lum_low, self.lum_high, self.lum_soft);
        if self.invert {
            1.0 - k
        } else {
            k
        }
    }

    /// Picks the colour `rgb`: a narrow range around its hue, saturation and luminance.
    pub fn pick(rgb: [f32; 3]) -> Self {
        let (h, s, l) = hsl(rgb);
        Self {
            hue_center: h,
            hue_width: 30.0,
            hue_soft: 12.0,
            sat_low: (s - 0.25).max(0.0),
            sat_high: (s + 0.25).min(1.0),
            sat_soft: 0.1,
            lum_low: (l - 0.3).max(0.0),
            lum_high: (l + 0.3).min(1.0),
            lum_soft: 0.1,
            ..Self::default()
        }
    }
}

/// The shape of a window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowShape {
    /// An ellipse.
    #[default]
    Circle,
    /// A rectangle.
    Linear,
    /// Everything on one side of a line, with a soft ramp across it.
    Gradient,
}

impl WindowShape {
    pub fn label(self) -> &'static str {
        match self {
            WindowShape::Circle => "Circle",
            WindowShape::Linear => "Linear",
            WindowShape::Gradient => "Gradient",
        }
    }
}

/// A power window. Places and sizes are fractions of the picture (0.5 is the middle), so
/// they fit every resolution; the rotation is in degrees.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PowerWindow {
    pub enabled: bool,
    pub shape: WindowShape,
    pub center: [f32; 2],
    /// Width and height, as fractions of the picture's width and height.
    pub size: [f32; 2],
    pub rotation: f32,
    /// How wide the soft edge is, 0..1 of the window's size (half inside, half outside).
    pub softness: f32,
    /// Grades what is outside the window instead.
    pub invert: bool,
}

impl Default for PowerWindow {
    fn default() -> Self {
        Self {
            enabled: true,
            shape: WindowShape::Circle,
            center: [0.5, 0.5],
            size: [0.4, 0.5],
            rotation: 0.0,
            softness: 0.2,
            invert: false,
        }
    }
}

impl PowerWindow {
    pub fn new(shape: WindowShape) -> Self {
        Self {
            shape,
            ..Self::default()
        }
    }

    pub fn sanitized(mut self) -> Self {
        let d = Self::default();
        for c in 0..2 {
            self.center[c] = finite(self.center[c], d.center[c]).clamp(-1.0, 2.0);
            self.size[c] = finite(self.size[c], d.size[c]).clamp(0.005, 4.0);
        }
        self.rotation = finite(self.rotation, 0.0).clamp(-360.0, 360.0);
        self.softness = finite(self.softness, d.softness).clamp(0.0, 1.0);
        self
    }

    /// The window's value at pixel (`x`, `y`) of a `width` x `height` picture, 0..1.
    pub fn value(&self, x: f32, y: f32, width: f32, height: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        let (s, c) = (-self.rotation.to_radians()).sin_cos();
        let dx = x - self.center[0] * width;
        let dy = y - self.center[1] * height;
        // In the window's own (unrotated) frame, in pixels.
        let (u, v) = (dx * c - dy * s, dx * s + dy * c);
        let rx = (self.size[0] * width / 2.0).max(0.5);
        let ry = (self.size[1] * height / 2.0).max(0.5);
        let soft = self.softness;
        let inside = |d: f32| {
            // d is 1 on the edge.
            if soft <= 0.0 {
                if d <= 1.0 {
                    1.0
                } else {
                    0.0
                }
            } else {
                smooth((1.0 + soft / 2.0 - d) / soft)
            }
        };
        let k = match self.shape {
            WindowShape::Circle => inside(((u / rx).powi(2) + (v / ry).powi(2)).sqrt()),
            WindowShape::Linear => inside((u / rx).abs().max((v / ry).abs())),
            // Above the line through the centre: 1; the ramp is the window's height.
            WindowShape::Gradient => smooth(0.5 - v / (2.0 * ry).max(1.0)),
        };
        if self.invert {
            1.0 - k
        } else {
            k
        }
    }
}

/// The key of a node over a whole picture (`rgba` before the node), one value per pixel, or
/// None when the node grades everything.
pub fn node_key(
    qualifier: Option<&Qualifier>,
    windows: &[PowerWindow],
    rgba: &[u8],
    width: usize,
    height: usize,
) -> Option<Vec<f32>> {
    let qualifier = qualifier.filter(|q| !q.takes_all());
    let windows: Vec<&PowerWindow> = windows.iter().filter(|w| w.enabled).collect();
    if qualifier.is_none() && windows.is_empty() {
        return None;
    }
    let len = width.checked_mul(height)?;
    if len == 0 || rgba.len() < len * 4 {
        return None;
    }
    let mut key: Vec<f32> = match qualifier {
        Some(q) => rgba
            .chunks_exact(4)
            .take(len)
            .map(|px| q.key([0, 1, 2].map(|c| f32::from(px[c]) / 255.0)))
            .collect(),
        None => vec![1.0; len],
    };
    if let Some(q) = qualifier {
        if q.clean_black > 0.0 || q.clean_white > 0.0 {
            let (lo, hi) = (
                q.clean_black,
                (1.0 - q.clean_white).max(q.clean_black + 0.01),
            );
            for k in &mut key {
                *k = ((*k - lo) / (hi - lo)).clamp(0.0, 1.0);
            }
        }
        if q.blur > 0.0 {
            let radius = ((q.blur / 100.0) * height as f32 * 0.1).round() as usize;
            if radius > 0 {
                let rows = crate::grade::blur_rows(&key, width, radius);
                key = crate::grade::blur_columns(&rows, width, height, radius);
            }
        }
    }
    if !windows.is_empty() {
        let (w, h) = (width as f32, height as f32);
        for (i, k) in key.iter_mut().enumerate() {
            let (x, y) = ((i % width) as f32 + 0.5, (i / width) as f32 + 0.5);
            let shape = windows
                .iter()
                .map(|win| win.value(x, y, w, h))
                .fold(0.0f32, f32::max);
            *k *= shape;
        }
    }
    Some(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsl_reads_the_primaries() {
        assert_eq!(hsl([1.0, 0.0, 0.0]).0, 0.0);
        assert!((hsl([0.0, 1.0, 0.0]).0 - 120.0).abs() < 1e-3);
        assert!((hsl([0.0, 0.0, 1.0]).0 - 240.0).abs() < 1e-3);
        assert_eq!(hsl([0.5, 0.5, 0.5]).1, 0.0);
    }

    #[test]
    fn a_picked_colour_keys_itself_and_not_its_opposite() {
        let q = Qualifier::pick([0.8, 0.2, 0.2]);
        assert!(q.key([0.8, 0.2, 0.2]) > 0.99);
        assert_eq!(q.key([0.2, 0.2, 0.8]), 0.0);
        assert_eq!(q.key([0.2, 0.8, 0.2]), 0.0);
        let inverted = Qualifier { invert: true, ..q };
        assert_eq!(inverted.key([0.2, 0.2, 0.8]), 1.0);
        // The hue range wraps around red.
        let reds = Qualifier {
            hue_center: 355.0,
            hue_width: 20.0,
            ..Qualifier::default()
        };
        assert!(reds.key([1.0, 0.0, 0.05]) > 0.99);
    }

    #[test]
    fn a_circle_is_full_inside_and_empty_outside() {
        let w = PowerWindow {
            softness: 0.0,
            ..PowerWindow::default()
        };
        assert_eq!(w.value(50.0, 50.0, 100.0, 100.0), 1.0);
        assert_eq!(w.value(2.0, 2.0, 100.0, 100.0), 0.0);
        let soft = PowerWindow::default();
        let edge = soft.value(70.0, 50.0, 100.0, 100.0);
        assert!(edge > 0.0 && edge < 1.0, "{edge}");
        let rect = PowerWindow {
            shape: WindowShape::Linear,
            softness: 0.0,
            ..PowerWindow::default()
        };
        // The rectangle has corners the circle does not.
        assert_eq!(rect.value(69.0, 73.0, 100.0, 100.0), 1.0);
        assert_eq!(w.value(69.0, 73.0, 100.0, 100.0), 0.0);
        let grad = PowerWindow::new(WindowShape::Gradient);
        assert!(grad.value(50.0, 0.0, 100.0, 100.0) > 0.99);
        assert!(grad.value(50.0, 99.0, 100.0, 100.0) < 0.01);
    }

    #[test]
    fn the_key_combines_the_qualifier_and_the_windows() {
        // Two pixels: red on the left, blue on the right.
        let rgba = [200, 20, 20, 255, 20, 20, 200, 255];
        let q = Qualifier::pick([200.0 / 255.0, 20.0 / 255.0, 20.0 / 255.0]);
        let key = node_key(Some(&q), &[], &rgba, 2, 1).unwrap();
        assert!(key[0] > 0.99 && key[1] == 0.0);
        let right = PowerWindow {
            shape: WindowShape::Linear,
            center: [0.75, 0.5],
            size: [0.5, 1.0],
            softness: 0.0,
            ..PowerWindow::default()
        };
        let key = node_key(None, &[right], &rgba, 2, 1).unwrap();
        assert_eq!(key, vec![0.0, 1.0]);
        assert!(node_key(Some(&Qualifier::default()), &[], &rgba, 2, 1).is_none());
    }

    #[test]
    fn nonsense_is_put_in_range() {
        let q = Qualifier {
            hue_center: f32::NAN,
            sat_low: -4.0,
            blur: 1e9,
            ..Qualifier::default()
        }
        .sanitized();
        assert_eq!((q.hue_center, q.sat_low, q.blur), (0.0, 0.0, 100.0));
        let w = PowerWindow {
            size: [f32::INFINITY, 0.0],
            ..PowerWindow::default()
        }
        .sanitized();
        assert!(w.size[0] <= 4.0 && w.size[1] > 0.0);
    }
}
