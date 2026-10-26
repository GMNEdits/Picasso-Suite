//! The primary colour correction of a clip.

use serde::{Deserialize, Serialize};

use crate::curves::{CurveTables, Curves};
use crate::mask::{node_key, PowerWindow, Qualifier, MAX_WINDOWS};
use crate::{Lut, LUMA};

/// Gamma below this would divide by (almost) zero.
const MIN_GAMMA: f32 = 0.05;
/// How far the temperature control reaches either way.
pub const TEMPERATURE_RANGE: f32 = 4000.0;
/// How far the tint, midtone detail, colour boost, shadows and highlights controls reach.
pub const PERCENT_RANGE: f32 = 100.0;

fn luma(rgb: [f32; 3]) -> f32 {
    rgb[0] * LUMA[0] + rgb[1] * LUMA[1] + rgb[2] * LUMA[2]
}

/// The primary correction of a clip: lift, gamma, gain and offset per channel, with white
/// balance, contrast, tone, hue, saturation, midtone detail and a look-up table.
///
/// Per channel `c` (0..1, display referred) the order is
/// 1. white balance: temperature scales red up and blue down (or the other way), tint scales
///    green down (towards magenta) or up;
/// 2. `v = v * gain; v = v + lift * (1 - v); v = v ^ (1 / gamma); v = v + offset;`
/// 3. `v = (v - pivot) * contrast + pivot`;
/// 4. shadows and highlights bend the lower and upper part of the range, leaving black and
///    white where they are.
///
/// Then for the whole pixel: hue rotation around the grey axis, saturation (with colour boost
/// adding more to pixels that have little colour), luma mix (below 100 the luma of the input
/// comes back), midtone detail (local contrast in the midtones, frames only, see
/// [`ColorGrade::apply_frame`]), the custom curves ([`Curves`]) and last the look-up table, if
/// there is one.
/// The default grade changes nothing.
/// How a node takes its input and hands on its result.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeMode {
    /// Grades the picture the node before it produced.
    #[default]
    Serial,
    /// Grades the same input as the node before it; its change is added to the result, scaled
    /// by `mix`.
    Parallel,
    /// Grades the picture like a serial node, then lays the result over it by `mix`.
    Layer,
}

impl NodeMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Serial => "Serial",
            Self::Parallel => "Parallel",
            Self::Layer => "Layer",
        }
    }
}

fn full_mix() -> f32 {
    100.0
}

fn is_full_mix(v: &f32) -> bool {
    *v == 100.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorGrade {
    /// Raises the shadows; 0 is neutral.
    pub lift: [f32; 3],
    /// Bends the midtones; 1 is neutral, above 1 is brighter.
    pub gamma: [f32; 3],
    /// Scales the highlights; 1 is neutral.
    pub gain: [f32; 3],
    /// Added to the whole range; 0 is neutral.
    pub offset: [f32; 3],
    /// Stretches around `pivot`; 1 is neutral.
    pub contrast: f32,
    /// The value contrast leaves where it is.
    pub pivot: f32,
    /// 0 is greyscale, 1 is neutral.
    pub saturation: f32,
    /// Warmer above 0, cooler below; -4000 to 4000.
    pub temperature: f32,
    /// Towards magenta above 0, green below; -100 to 100.
    pub tint: f32,
    /// Local contrast in the midtones; -100 to 100.
    pub midtone_detail: f32,
    /// More saturation for the pixels with little colour; -100 to 100.
    pub color_boost: f32,
    /// Brightens (above 0) or darkens the lower part of the range; -100 to 100.
    pub shadows: f32,
    /// Brightens (above 0) or darkens the upper part of the range; -100 to 100.
    pub highlights: f32,
    /// Hue rotation: 50 is neutral, 0 and 100 are half a turn either way.
    pub hue: f32,
    /// How much the correction may change the luma: 100 is all of it, 0 keeps the input's.
    pub luma_mix: f32,
    /// The custom curves (Y, R, G, B, intensities and soft clip), after the rest and before the
    /// table.
    #[serde(skip_serializing_if = "is_default_curves")]
    pub curves: Curves,
    /// A table applied after everything else.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lut: Option<Lut>,
    /// A node that is switched off passes the picture through unchanged, keeping its settings.
    #[serde(default = "switched_on")]
    pub enabled: bool,
    /// Grades only the colours it keys (secondary correction).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualifier: Option<Qualifier>,
    /// Grades only inside these shapes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub windows: Vec<PowerWindow>,
    /// Shows the key: what the node grades in colour, the rest grey. Only the viewer sets it,
    /// on a copy of the project; a loaded or edited grade never has it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub highlight: bool,
    /// The node's name, shown on it in the node graph.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// How the node joins the ones before it.
    #[serde(skip_serializing_if = "is_serial")]
    pub mode: NodeMode,
    /// How strongly a parallel or layer node counts, 0 to 100.
    #[serde(default = "full_mix", skip_serializing_if = "is_full_mix")]
    pub mix: f32,
}

/// How a pixel goes through the curves: not at all, through tables made once for a whole
/// picture, or by evaluating them (one pixel at a time). Decided once, not for every pixel.
enum CurveStep<'a> {
    Off,
    Table(&'a CurveTables),
    Direct,
}

fn is_default_curves(c: &Curves) -> bool {
    *c == Curves::default()
}

fn is_serial(m: &NodeMode) -> bool {
    *m == NodeMode::Serial
}

fn switched_on() -> bool {
    true
}

impl Default for ColorGrade {
    fn default() -> Self {
        Self {
            lift: [0.0; 3],
            gamma: [1.0; 3],
            gain: [1.0; 3],
            offset: [0.0; 3],
            contrast: 1.0,
            pivot: 0.435,
            saturation: 1.0,
            temperature: 0.0,
            tint: 0.0,
            midtone_detail: 0.0,
            color_boost: 0.0,
            shadows: 0.0,
            highlights: 0.0,
            hue: 50.0,
            luma_mix: 100.0,
            curves: Curves::default(),
            lut: None,
            enabled: true,
            qualifier: None,
            windows: Vec::new(),
            highlight: false,
            label: String::new(),
            mode: NodeMode::Serial,
            mix: 100.0,
        }
    }
}

fn finite_or(v: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        fallback
    }
}

impl ColorGrade {
    /// True when the grade changes the picture: it is switched on and not neutral.
    pub fn has_effect(&self) -> bool {
        self.enabled && !self.is_identity()
    }

    /// True when every setting is neutral (whether or not the node is switched on).
    pub fn is_identity(&self) -> bool {
        let d = Self::default();
        self.lift == d.lift
            && self.gamma == d.gamma
            && self.gain == d.gain
            && self.offset == d.offset
            && self.contrast == d.contrast
            && self.saturation == d.saturation
            && self.temperature == d.temperature
            && self.tint == d.tint
            && self.midtone_detail == d.midtone_detail
            && self.color_boost == d.color_boost
            && self.shadows == d.shadows
            && self.highlights == d.highlights
            && self.hue == d.hue
            && self.luma_mix == d.luma_mix
            && self.curves.is_identity()
            && self.lut.is_none()
    }

    /// Replaces values that are not numbers, so a hostile project file cannot poison a render.
    pub fn sanitized(mut self) -> Self {
        let d = Self::default();
        for c in 0..3 {
            self.lift[c] = finite_or(self.lift[c], d.lift[c]).clamp(-2.0, 2.0);
            self.gamma[c] = finite_or(self.gamma[c], d.gamma[c]).clamp(MIN_GAMMA, 10.0);
            self.gain[c] = finite_or(self.gain[c], d.gain[c]).clamp(0.0, 10.0);
            self.offset[c] = finite_or(self.offset[c], d.offset[c]).clamp(-2.0, 2.0);
        }
        self.contrast = finite_or(self.contrast, d.contrast).clamp(0.0, 10.0);
        self.pivot = finite_or(self.pivot, d.pivot).clamp(0.01, 0.99);
        self.saturation = finite_or(self.saturation, d.saturation).clamp(0.0, 10.0);
        self.temperature =
            finite_or(self.temperature, d.temperature).clamp(-TEMPERATURE_RANGE, TEMPERATURE_RANGE);
        for v in [
            &mut self.tint,
            &mut self.midtone_detail,
            &mut self.color_boost,
            &mut self.shadows,
            &mut self.highlights,
        ] {
            *v = finite_or(*v, 0.0).clamp(-PERCENT_RANGE, PERCENT_RANGE);
        }
        self.hue = finite_or(self.hue, d.hue).clamp(0.0, 100.0);
        self.luma_mix = finite_or(self.luma_mix, d.luma_mix).clamp(0.0, 100.0);
        self.curves = self.curves.sanitized();
        if self.lut.as_ref().is_some_and(|l| !l.is_valid()) {
            self.lut = None;
        }
        self.qualifier = self.qualifier.map(Qualifier::sanitized);
        self.mix = finite_or(self.mix, 100.0).clamp(0.0, 100.0);
        self.highlight = false;
        self.label = self
            .label
            .chars()
            .filter(|c| !c.is_control())
            .take(64)
            .collect();
        self.windows.truncate(MAX_WINDOWS);
        self.windows = self
            .windows
            .into_iter()
            .map(PowerWindow::sanitized)
            .collect();
        self
    }

    /// The white balance factor of channel `c`.
    fn balance(&self, c: usize) -> f32 {
        let warm = self.temperature / TEMPERATURE_RANGE * 0.3;
        let magenta = self.tint / PERCENT_RANGE * 0.3;
        match c {
            0 => 1.0 + warm,
            1 => 1.0 - magenta,
            _ => 1.0 - warm,
        }
    }

    /// The per-channel part of the grade for channel `c`, clamped to 0..1.
    fn channel(&self, c: usize, v: f32) -> f32 {
        let mut v = v * self.balance(c) * self.gain[c];
        v += self.lift[c] * (1.0 - v);
        v = v.max(0.0).powf(1.0 / self.gamma[c].max(MIN_GAMMA));
        v += self.offset[c];
        v = (v - self.pivot) * self.contrast + self.pivot;
        let t = v.clamp(0.0, 1.0);
        // Both bends peak at 0.3 of the full range and leave 0 and 1 where they are.
        v += self.shadows / PERCENT_RANGE * 0.3 * 6.75 * t * (1.0 - t) * (1.0 - t);
        v += self.highlights / PERCENT_RANGE * 0.3 * 6.75 * t * t * (1.0 - t);
        v.clamp(0.0, 1.0)
    }

    /// The whole-pixel steps after the channels: hue, saturation with colour boost, luma mix,
    /// the curves and the table. `input` is the pixel before grading; `tables` are the curves
    /// sampled, when many pixels are graded.
    fn pixel(&self, input: [f32; 3], rgb: [f32; 3], curves: &CurveStep) -> [f32; 3] {
        let mut rgb = rgb;
        if self.hue != 50.0 {
            rgb = rotate_hue(rgb, (self.hue - 50.0) / 50.0 * std::f32::consts::PI);
        }
        let boost = self.color_boost / PERCENT_RANGE;
        if self.saturation != 1.0 || boost != 0.0 {
            let max = rgb[0].max(rgb[1]).max(rgb[2]);
            let min = rgb[0].min(rgb[1]).min(rgb[2]);
            let s = (self.saturation * (1.0 + boost * (1.0 - (max - min)))).max(0.0);
            let y = luma(rgb);
            rgb = [0, 1, 2].map(|c| y + (rgb[c] - y) * s);
        }
        if self.luma_mix < 100.0 {
            let back = (1.0 - self.luma_mix / 100.0) * (luma(input) - luma(rgb));
            rgb = rgb.map(|v| v + back);
        }
        let mut rgb = rgb.map(|v| v.clamp(0.0, 1.0));
        match curves {
            CurveStep::Off => {}
            CurveStep::Table(t) => rgb = [0, 1, 2].map(|c| t.apply(c, rgb[c])),
            CurveStep::Direct => rgb = [0, 1, 2].map(|c| self.curves.apply_channel(c, rgb[c])),
        }
        match &self.lut {
            Some(lut) => lut.apply(rgb).map(|v| v.clamp(0.0, 1.0)),
            None => rgb,
        }
    }

    /// Whether a step needs the whole pixel (hue, saturation, colour boost, luma mix or the
    /// table), so the channels cannot be graded each on its own.
    fn mixes_channels(&self) -> bool {
        self.hue != 50.0
            || self.saturation != 1.0
            || self.color_boost != 0.0
            || self.luma_mix < 100.0
            || self.lut.is_some()
    }

    /// Grades one pixel, `rgb` in 0..1. Midtone detail needs the pixels around, so it is left
    /// out here.
    pub fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        if !self.enabled {
            return rgb;
        }
        let curves = if self.curves.is_identity() {
            CurveStep::Off
        } else {
            CurveStep::Direct
        };
        self.pixel(rgb, [0, 1, 2].map(|c| self.channel(c, rgb[c])), &curves)
    }

    /// Grades `rgba` (8 bits per channel, alpha untouched) in place, without midtone detail. A
    /// trailing partial pixel is left as it is.
    pub fn apply_rgba8(&self, rgba: &mut [u8]) {
        if !self.has_effect() {
            return;
        }
        if !self.mixes_channels() {
            // Every step works on each channel alone, so the whole grade is one table per
            // channel from 8 bits to 8 bits: three look-ups a pixel.
            let curves = !self.curves.is_identity();
            let tables: [[u8; 256]; 3] = [0, 1, 2].map(|c| {
                std::array::from_fn(|i| {
                    let mut v = self.channel(c, i as f32 / 255.0);
                    if curves {
                        v = self.curves.apply_channel(c, v);
                    }
                    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
                })
            });
            for px in rgba.chunks_exact_mut(4) {
                for (v, table) in px.iter_mut().zip(&tables) {
                    *v = table[usize::from(*v)];
                }
            }
            return;
        }
        let curves: [[f32; 256]; 3] =
            [0, 1, 2].map(|c| std::array::from_fn(|i| self.channel(c, i as f32 / 255.0)));
        let tables = (!self.curves.is_identity()).then(|| self.curves.tables());
        let step = match &tables {
            Some(t) => CurveStep::Table(t),
            None => CurveStep::Off,
        };
        for px in rgba.chunks_exact_mut(4) {
            let input = [0, 1, 2].map(|c| f32::from(px[c]) / 255.0);
            let rgb = self.pixel(
                input,
                [0, 1, 2].map(|c| curves[c][usize::from(px[c])]),
                &step,
            );
            for c in 0..3 {
                px[c] = (rgb[c] * 255.0 + 0.5) as u8;
            }
        }
    }

    /// Grades a whole frame of `width` x `height` pixels in place, with midtone detail, only
    /// where the qualifier and windows key it. A buffer that does not match the size is graded
    /// without midtone detail or key.
    pub fn apply_frame(&self, rgba: &mut [u8], width: usize, height: usize) {
        if !self.has_effect() && !(self.highlight && self.enabled) {
            return;
        }
        let key = self.key(rgba, width, height);
        let before = key.as_ref().map(|_| rgba.to_vec());
        self.apply_rgba8(rgba);
        if self.midtone_detail != 0.0 {
            midtone_detail(rgba, width, height, self.midtone_detail / PERCENT_RANGE);
        }
        if let (Some(key), Some(before)) = (key, before) {
            for ((px, old), k) in rgba
                .chunks_exact_mut(4)
                .zip(before.chunks_exact(4))
                .zip(key)
            {
                for c in 0..3 {
                    // Highlight shows grey where the node does not grade.
                    let a = if self.highlight {
                        128.0
                    } else {
                        f32::from(old[c])
                    };
                    let b = f32::from(px[c]);
                    px[c] = (a + (b - a) * k + 0.5) as u8;
                }
            }
        }
    }

    /// Where the node grades `rgba` (its input), one value 0..1 per pixel, or None when it
    /// grades everything.
    pub fn key(&self, rgba: &[u8], width: usize, height: usize) -> Option<Vec<f32>> {
        node_key(self.qualifier.as_ref(), &self.windows, rgba, width, height)
    }

    /// Whether the node grades only part of the picture.
    pub fn is_secondary(&self) -> bool {
        self.qualifier.as_ref().is_some_and(|q| !q.takes_all())
            || self.windows.iter().any(|w| w.enabled)
    }
}

/// Turns the hue of `rgb` by `angle` radians around the grey axis.
fn rotate_hue(rgb: [f32; 3], angle: f32) -> [f32; 3] {
    let (s, c) = angle.sin_cos();
    let third = (1.0 - c) / 3.0;
    let root = s / 3f32.sqrt();
    let m = [
        [c + third, third - root, third + root],
        [third + root, c + third, third - root],
        [third - root, third + root, c + third],
    ];
    m.map(|row| row[0] * rgb[0] + row[1] * rgb[1] + row[2] * rgb[2])
}

/// Adds local contrast to the midtones: each pixel moves away from the mean luma around it, by
/// `amount` (-1 to 1) times how mid-grey it is.
fn midtone_detail(rgba: &mut [u8], width: usize, height: usize, amount: f32) {
    let Some(len) = width.checked_mul(height) else {
        return;
    };
    if len == 0 || rgba.len() < len * 4 {
        return;
    }
    let y: Vec<f32> = rgba
        .chunks_exact(4)
        .take(len)
        .map(|px| luma([0, 1, 2].map(|c| f32::from(px[c]) / 255.0)))
        .collect();
    let radius = (width.max(height) / 150).max(1);
    let rows = blur_rows(&y, width, radius);
    let blurred = blur_columns(&rows, width, height, radius);
    for ((px, &l), &b) in rgba.chunks_exact_mut(4).zip(&y).zip(&blurred) {
        let weight = 4.0 * l * (1.0 - l);
        let d = amount * 1.5 * (l - b) * weight;
        for v in px.iter_mut().take(3) {
            *v = ((f32::from(*v) / 255.0 + d).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        }
    }
}

/// The mean of `line` over `radius` either side of each place.
fn running_mean(
    line: impl Fn(usize) -> f32,
    n: usize,
    radius: usize,
    mut out: impl FnMut(usize, f32),
) {
    let mut sum = 0.0;
    let mut count = 0.0;
    for i in 0..radius.min(n) {
        sum += line(i);
        count += 1.0;
    }
    for i in 0..n {
        if i + radius < n {
            sum += line(i + radius);
            count += 1.0;
        }
        if i > radius {
            sum -= line(i - radius - 1);
            count -= 1.0;
        }
        out(i, sum / count);
    }
}

pub(crate) fn blur_rows(v: &[f32], width: usize, radius: usize) -> Vec<f32> {
    let mut out = vec![0.0; v.len()];
    for (row, line) in v.chunks_exact(width).enumerate() {
        running_mean(|i| line[i], width, radius, |i, m| out[row * width + i] = m);
    }
    out
}

pub(crate) fn blur_columns(v: &[f32], width: usize, height: usize, radius: usize) -> Vec<f32> {
    let mut out = vec![0.0; v.len()];
    for col in 0..width {
        running_mean(
            |i| v[i * width + col],
            height,
            radius,
            |i, m| out[i * width + col] = m,
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4)
    }

    #[test]
    fn default_is_identity() {
        let g = ColorGrade::default();
        assert!(g.is_identity());
        for v in [0.0, 0.2, 0.5, 0.9, 1.0] {
            assert!(close(g.apply([v, v, v]), [v, v, v]));
        }
        let mut px = [10, 120, 250, 77];
        g.apply_rgba8(&mut px);
        assert_eq!(px, [10, 120, 250, 77]);
    }

    #[test]
    fn gain_scales_and_clamps() {
        let g = ColorGrade {
            gain: [0.5, 2.0, 1.0],
            ..Default::default()
        };
        assert!(close(g.apply([0.8, 0.8, 0.8]), [0.4, 1.0, 0.8]));
    }

    #[test]
    fn lift_raises_black_and_leaves_white() {
        let g = ColorGrade {
            lift: [0.2; 3],
            ..Default::default()
        };
        assert!(close(g.apply([0.0; 3]), [0.2; 3]));
        assert!(close(g.apply([1.0; 3]), [1.0; 3]));
    }

    #[test]
    fn gamma_above_one_brightens_midtones() {
        let g = ColorGrade {
            gamma: [2.0; 3],
            ..Default::default()
        };
        let out = g.apply([0.25; 3]);
        assert!(close(out, [0.5; 3]));
    }

    #[test]
    fn zero_saturation_is_grey() {
        let g = ColorGrade {
            saturation: 0.0,
            ..Default::default()
        };
        let out = g.apply([1.0, 0.0, 0.0]);
        assert!((out[0] - out[1]).abs() < 1e-5 && (out[1] - out[2]).abs() < 1e-5);
        assert!((out[0] - LUMA[0]).abs() < 1e-5);
    }

    #[test]
    fn contrast_leaves_the_pivot() {
        let g = ColorGrade {
            contrast: 2.0,
            ..Default::default()
        };
        assert!(close(g.apply([0.435; 3]), [0.435; 3]));
        assert!(g.apply([0.6; 3])[0] > 0.6);
    }

    #[test]
    fn rgba8_matches_the_float_path_and_keeps_alpha() {
        let g = ColorGrade {
            gain: [1.2, 0.9, 1.0],
            saturation: 1.3,
            lift: [0.0, 0.05, 0.0],
            ..Default::default()
        };
        let mut px = [200u8, 100, 50, 9];
        g.apply_rgba8(&mut px);
        let f = g.apply([200.0 / 255.0, 100.0 / 255.0, 50.0 / 255.0]);
        for c in 0..3 {
            assert!(
                (f32::from(px[c]) - f[c] * 255.0).abs() <= 1.5,
                "channel {c}"
            );
        }
        assert_eq!(px[3], 9);
    }

    #[test]
    fn sanitized_replaces_hostile_numbers() {
        let g = ColorGrade {
            gamma: [f32::NAN, 0.0, f32::INFINITY],
            contrast: f32::NAN,
            ..Default::default()
        }
        .sanitized();
        assert_eq!(g.gamma[0], 1.0);
        assert!(g.gamma[1] >= MIN_GAMMA);
        assert_eq!(g.gamma[2], 1.0);
        assert_eq!(g.contrast, 1.0);
        let mut px = [1u8, 2, 3, 4];
        g.apply_rgba8(&mut px);
    }

    #[test]
    fn serde_round_trip_and_missing_fields_default() {
        let g = ColorGrade {
            gain: [1.5, 1.0, 0.7],
            saturation: 0.8,
            ..Default::default()
        };
        let text = serde_json::to_string(&g).unwrap();
        assert_eq!(serde_json::from_str::<ColorGrade>(&text).unwrap(), g);
        // A file from an older version that knows fewer fields still loads.
        let partial: ColorGrade = serde_json::from_str(r#"{"saturation": 0.5}"#).unwrap();
        assert_eq!(partial.saturation, 0.5);
        assert_eq!(partial.gain, [1.0; 3]);
    }

    #[test]
    fn the_lut_comes_last_and_survives_a_round_trip() {
        let text = "LUT_1D_SIZE 2
1 1 1
0 0 0
";
        let g = ColorGrade {
            gain: [0.5; 3],
            lut: Some(Lut::parse_cube(text, "invert").unwrap()),
            ..Default::default()
        };
        assert!(!g.is_identity());
        // Gain first (0.8 -> 0.4), then the inverting table (0.4 -> 0.6).
        assert!(close(g.apply([0.8; 3]), [0.6; 3]));
        let mut px = [204u8, 204, 204, 1];
        g.apply_rgba8(&mut px);
        assert!((i32::from(px[0]) - 153).abs() <= 1);
        let text = serde_json::to_string(&g).unwrap();
        assert_eq!(serde_json::from_str::<ColorGrade>(&text).unwrap(), g);
        // A grade without a table does not write the field at all.
        assert!(!serde_json::to_string(&ColorGrade::default())
            .unwrap()
            .contains("lut"));
    }

    #[test]
    fn a_broken_lut_in_a_file_is_dropped() {
        let mut lut = Lut::parse_cube(
            "LUT_1D_SIZE 2
0 0 0
1 1 1
",
            "x",
        )
        .unwrap();
        lut.table.truncate(1);
        let g = ColorGrade {
            lut: Some(lut),
            ..Default::default()
        }
        .sanitized();
        assert!(g.lut.is_none());
    }
    #[test]
    fn white_balance_moves_red_and_blue() {
        let warm = ColorGrade {
            temperature: 2000.0,
            ..Default::default()
        };
        let out = warm.apply([0.5; 3]);
        assert!(out[0] > 0.5 && out[2] < 0.5 && (out[1] - 0.5).abs() < 1e-5);
        let magenta = ColorGrade {
            tint: 50.0,
            ..Default::default()
        };
        assert!(magenta.apply([0.5; 3])[1] < 0.5);
    }

    #[test]
    fn shadows_and_highlights_keep_black_and_white() {
        let g = ColorGrade {
            shadows: 100.0,
            highlights: -100.0,
            ..Default::default()
        };
        assert!(close(g.apply([0.0; 3]), [0.0; 3]));
        assert!(close(g.apply([1.0; 3]), [1.0; 3]));
        assert!(g.apply([0.25; 3])[0] > 0.25);
        assert!(g.apply([0.75; 3])[0] < 0.75);
    }

    #[test]
    fn hue_turns_colours_and_leaves_grey() {
        let g = ColorGrade {
            hue: 100.0,
            ..Default::default()
        };
        assert!(close(g.apply([0.4; 3]), [0.4; 3]));
        // Half a turn takes red towards cyan.
        let out = g.apply([0.8, 0.2, 0.2]);
        assert!(out[0] < out[1] && out[0] < out[2]);
    }

    #[test]
    fn colour_boost_saturates_dull_pixels_more() {
        let g = ColorGrade {
            color_boost: 100.0,
            ..Default::default()
        };
        let dull = g.apply([0.5, 0.45, 0.45]);
        assert!(dull[0] - dull[1] > 0.075);
        assert!(close(g.apply([0.3; 3]), [0.3; 3]));
    }

    #[test]
    fn luma_mix_zero_keeps_the_input_luma() {
        let g = ColorGrade {
            gain: [1.5; 3],
            luma_mix: 0.0,
            ..Default::default()
        };
        assert!(close(g.apply([0.4; 3]), [0.4; 3]));
    }

    #[test]
    fn midtone_detail_adds_local_contrast_and_tolerates_bad_sizes() {
        // A vertical edge between two greys.
        let (w, h) = (16, 4);
        let mut frame = Vec::new();
        for _ in 0..h {
            for x in 0..w {
                let v = if x < w / 2 { 100u8 } else { 150 };
                frame.extend_from_slice(&[v, v, v, 255]);
            }
        }
        let g = ColorGrade {
            midtone_detail: 100.0,
            ..Default::default()
        };
        let mut graded = frame.clone();
        g.apply_frame(&mut graded, w, h);
        // Next to the edge the dark side gets darker and the light side lighter.
        assert!(graded[(w / 2 - 1) * 4] < 100);
        assert!(graded[(w / 2) * 4] > 150);
        // A buffer smaller than its size says is left alone, not read past its end.
        let mut short = frame[..8].to_vec();
        g.apply_frame(&mut short, w, h);
        assert_eq!(short, frame[..8].to_vec());
    }

    #[test]
    fn the_new_controls_are_sanitized() {
        let g = ColorGrade {
            temperature: f32::INFINITY,
            hue: f32::NAN,
            shadows: 1e9,
            ..Default::default()
        }
        .sanitized();
        assert_eq!(g.temperature, 0.0);
        assert_eq!(g.hue, 50.0);
        assert_eq!(g.shadows, PERCENT_RANGE);
    }

    #[test]
    fn a_window_grades_only_inside_it() {
        // A grey frame 4 pixels wide; a rectangle over the right half.
        let mut rgba = [100u8, 100, 100, 255].repeat(4);
        let g = ColorGrade {
            gain: [2.0; 3],
            windows: vec![PowerWindow {
                shape: crate::WindowShape::Linear,
                center: [0.75, 0.5],
                size: [0.5, 1.0],
                softness: 0.0,
                ..PowerWindow::default()
            }],
            ..Default::default()
        };
        assert!(g.is_secondary());
        g.apply_frame(&mut rgba, 4, 1);
        let reds: Vec<u8> = rgba.chunks_exact(4).map(|p| p[0]).collect();
        assert_eq!(reds, vec![100, 100, 200, 200]);
        // A qualifier for red leaves the grey alone.
        let mut grey = [100u8, 100, 100, 255];
        let q = ColorGrade {
            gain: [2.0; 3],
            qualifier: Some(Qualifier::pick([0.9, 0.1, 0.1])),
            ..Default::default()
        };
        q.apply_frame(&mut grey, 1, 1);
        assert_eq!(grey, [100, 100, 100, 255]);
        let back = serde_json::to_string(&g).unwrap();
        assert_eq!(serde_json::from_str::<ColorGrade>(&back).unwrap(), g);
    }
}
