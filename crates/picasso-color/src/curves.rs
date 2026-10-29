//! The Color page's curves: the custom ones (a Y curve that acts on every channel, then one
//! each for red, green and blue, how strongly each counts, and a soft clip at black and white)
//! and the hue/sat/lum ones (Hue vs Hue, Hue vs Sat, Hue vs Lum, Lum vs Sat, Sat vs Sat and
//! Sat vs Lum).
//!
//! A curve is a list of points (input, output), both 0 to 1, sorted by input, with one point at
//! input 0 and one at input 1. Between them it is a monotone cubic (Fritsch-Carlson), so it is
//! smooth and never overshoots between two points. The default curves are straight and change
//! nothing.
//!
//! A hue/sat/lum curve is read as a change: the output minus the input is how much the hue,
//! saturation or luminance moves at that place on the graph, so a straight line changes
//! nothing. Hue vs Hue turns the hue by up to half a turn either way, the others scale the
//! saturation or the luminance by 0 to 2 (a straight line scales by 1).

use serde::{Deserialize, Serialize};

use crate::grade::rotate_hue;

/// The curves, in the order of the Edit buttons.
pub const Y: usize = 0;
pub const RED: usize = 1;
pub const GREEN: usize = 2;
pub const BLUE: usize = 3;

/// The hue/sat/lum curves, in the order of the buttons after Custom.
pub const HUE_VS_HUE: usize = 0;
pub const HUE_VS_SAT: usize = 1;
pub const HUE_VS_LUM: usize = 2;
pub const LUM_VS_SAT: usize = 3;
pub const SAT_VS_SAT: usize = 4;
pub const SAT_VS_LUM: usize = 5;
/// How many hue/sat/lum curves there are.
pub const HUE_MODES: usize = 6;
/// The colourfulness at which a pixel's hue counts in full for the curves keyed on hue.
const HUE_FULL_CHROMA: f32 = 0.1;

/// The most points one curve keeps.
pub const MAX_POINTS: usize = 32;
/// Two points are at least this far apart in input.
const MIN_GAP: f32 = 0.01;
/// The number of steps of the tables a frame is graded with: fine enough that reading the
/// nearest step is closer than an 8-bit level for curves up to 16 times steeper than straight.
const TABLE_STEPS: usize = 4096;

/// The custom curves of a node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Curves {
    /// Y, R, G and B: points `[input, output]`, sorted by input, the first at 0 and the last at 1.
    pub points: [Vec<[f32; 2]>; 4],
    /// How much of each curve counts, 0 to 100.
    pub intensity: [f32; 4],
    /// Soft Clip: low, low soft, high and high soft, 0 to 1. Low above 0.5 lifts the black
    /// clip, high below 0.5 lowers the white clip; the soft values round the clip off over that
    /// much of the range. 0.5, 0, 0.5, 0 changes nothing.
    pub soft_clip: [f32; 4],
    /// The hue/sat/lum curves: points `[input, output]` over the hue, saturation or luminance
    /// of a pixel, straight until edited.
    pub hue_points: [Vec<[f32; 2]>; HUE_MODES],
    /// How much each hue/sat/lum curve counts, 0 to 100.
    pub hue_intensity: [f32; HUE_MODES],
}

/// Soft Clip as it starts: no clip.
pub const SOFT_CLIP_NONE: [f32; 4] = [0.5, 0.0, 0.5, 0.0];

fn straight() -> Vec<[f32; 2]> {
    vec![[0.0, 0.0], [1.0, 1.0]]
}

impl Default for Curves {
    fn default() -> Self {
        Self {
            points: [straight(), straight(), straight(), straight()],
            intensity: [100.0; 4],
            soft_clip: SOFT_CLIP_NONE,
            hue_points: std::array::from_fn(|_| straight()),
            hue_intensity: [100.0; HUE_MODES],
        }
    }
}

impl Curves {
    /// Whether curve `curve` (0..4) is the straight line.
    pub fn is_straight(&self, curve: usize) -> bool {
        self.points
            .get(curve)
            .is_none_or(|p| p.as_slice() == [[0.0, 0.0], [1.0, 1.0]])
    }

    /// Whether hue/sat/lum curve `mode` (0..[`HUE_MODES`]) is the straight line.
    pub fn is_hue_straight(&self, mode: usize) -> bool {
        self.hue_points
            .get(mode)
            .is_none_or(|p| p.as_slice() == [[0.0, 0.0], [1.0, 1.0]])
    }

    /// Whether the hue/sat/lum curves change nothing.
    pub fn hue_is_identity(&self) -> bool {
        (0..HUE_MODES).all(|m| self.is_hue_straight(m) || self.hue_intensity[m] <= 0.0)
    }

    /// Whether the curves change nothing.
    pub fn is_identity(&self) -> bool {
        (0..4).all(|c| self.is_straight(c) || self.intensity[c] <= 0.0)
            && self.soft_clip == SOFT_CLIP_NONE
            && self.hue_is_identity()
    }

    /// The same curves made safe: numbers in range, points sorted and apart, ends at 0 and 1.
    pub fn sanitized(mut self) -> Self {
        for points in self.points.iter_mut().chain(self.hue_points.iter_mut()) {
            sanitize_points(points);
        }
        for v in self.intensity.iter_mut().chain(self.hue_intensity.iter_mut()) {
            *v = if v.is_finite() { v.clamp(0.0, 100.0) } else { 100.0 };
        }
        for (v, d) in self.soft_clip.iter_mut().zip(SOFT_CLIP_NONE) {
            *v = if v.is_finite() { v.clamp(0.0, 1.0) } else { d };
        }
        self
    }

    /// Curve `curve` at input `x` (0 to 1), without its intensity.
    pub fn eval(&self, curve: usize, x: f32) -> f32 {
        match self.points.get(curve) {
            Some(p) => spline(p, x),
            None => x,
        }
    }

    /// Hue/sat/lum curve `mode` at input `x` (0 to 1), without its intensity.
    pub fn eval_hue(&self, mode: usize, x: f32) -> f32 {
        match self.hue_points.get(mode) {
            Some(p) => spline(p, x),
            None => x,
        }
    }

    /// Channel `channel` (0 red, 1 green, 2 blue) of a pixel through the custom curves: the Y
    /// curve, then the channel's own, each by its intensity, then the soft clip.
    pub fn apply_channel(&self, channel: usize, v: f32) -> f32 {
        let mut v = v.clamp(0.0, 1.0);
        for curve in [Y, channel + 1] {
            let k = self.intensity.get(curve).copied().unwrap_or(0.0) / 100.0;
            if k > 0.0 && !self.is_straight(curve) {
                v += (self.eval(curve, v) - v) * k;
            }
        }
        soft_clip(v, self.soft_clip)
    }

    /// A pixel through the hue/sat/lum curves: the hue turns, and the saturation and luminance
    /// scale, by how far each curve is from the straight line at that pixel's hue, saturation
    /// or luminance.
    pub fn apply_hue_modes(&self, rgb: [f32; 3]) -> [f32; 3] {
        if self.hue_is_identity() {
            return rgb;
        }
        hue_modes(rgb, |mode, x| self.hue_delta(mode, x))
    }

    /// How far hue/sat/lum curve `mode` is from the straight line at `x`, by its intensity.
    fn hue_delta(&self, mode: usize, x: f32) -> f32 {
        let k = self.hue_intensity.get(mode).copied().unwrap_or(0.0) / 100.0;
        if k <= 0.0 || self.is_hue_straight(mode) {
            0.0
        } else {
            (self.eval_hue(mode, x) - x) * k
        }
    }

    /// The curves of the three channels as tables, for grading many pixels.
    pub fn tables(&self) -> CurveTables {
        let hue_active = !self.hue_is_identity();
        CurveTables {
            steps: std::array::from_fn(|c| {
                (0..=TABLE_STEPS)
                    .map(|i| self.apply_channel(c, i as f32 / TABLE_STEPS as f32))
                    .collect()
            }),
            hue_active,
            hue_deltas: std::array::from_fn(|m| {
                if !hue_active || self.is_hue_straight(m) || self.hue_intensity[m] <= 0.0 {
                    return Vec::new();
                }
                (0..=TABLE_STEPS)
                    .map(|i| self.hue_delta(m, i as f32 / TABLE_STEPS as f32))
                    .collect()
            }),
        }
    }

    /// Adds a point to curve `curve` at `[x, y]` and returns its index, or None when it is too
    /// close to another point or the curve is full.
    pub fn add_point(&mut self, curve: usize, x: f32, y: f32) -> Option<usize> {
        add_to(self.points.get_mut(curve)?, x, y)
    }

    /// Adds a point to hue/sat/lum curve `mode` at `[x, y]`, like [`Curves::add_point`].
    pub fn add_hue_point(&mut self, mode: usize, x: f32, y: f32) -> Option<usize> {
        add_to(self.hue_points.get_mut(mode)?, x, y)
    }

    /// Moves point `index` of curve `curve` to `[x, y]`. The end points only move up and down;
    /// the others stay between their neighbours.
    pub fn move_point(&mut self, curve: usize, index: usize, x: f32, y: f32) {
        if let Some(points) = self.points.get_mut(curve) {
            move_in(points, index, x, y);
        }
    }

    /// Moves point `index` of hue/sat/lum curve `mode`, like [`Curves::move_point`].
    pub fn move_hue_point(&mut self, mode: usize, index: usize, x: f32, y: f32) {
        if let Some(points) = self.hue_points.get_mut(mode) {
            move_in(points, index, x, y);
        }
    }

    /// Removes point `index` of curve `curve`; the end points stay.
    pub fn remove_point(&mut self, curve: usize, index: usize) {
        if let Some(points) = self.points.get_mut(curve) {
            remove_from(points, index);
        }
    }

    /// Removes point `index` of hue/sat/lum curve `mode`; the end points stay.
    pub fn remove_hue_point(&mut self, mode: usize, index: usize) {
        if let Some(points) = self.hue_points.get_mut(mode) {
            remove_from(points, index);
        }
    }

    /// The point of curve `curve` nearest `[x, y]`, if one is within `radius` (in the same 0..1
    /// units, measured separately along each axis with `aspect` = width / height of the graph).
    pub fn point_near(&self, curve: usize, x: f32, y: f32, radius: f32, aspect: f32) -> Option<usize> {
        near_in(self.points.get(curve)?, x, y, radius, aspect)
    }

    /// The point of hue/sat/lum curve `mode` nearest `[x, y]`, like [`Curves::point_near`].
    pub fn hue_point_near(&self, mode: usize, x: f32, y: f32, radius: f32, aspect: f32) -> Option<usize> {
        near_in(self.hue_points.get(mode)?, x, y, radius, aspect)
    }
}

/// Makes `points` safe: numbers in range, sorted and apart, the ends at inputs 0 and 1.
fn sanitize_points(points: &mut Vec<[f32; 2]>) {
    points.retain(|p| p[0].is_finite() && p[1].is_finite());
    for p in points.iter_mut() {
        *p = [p[0].clamp(0.0, 1.0), p[1].clamp(0.0, 1.0)];
    }
    points.sort_by(|a, b| a[0].total_cmp(&b[0]));
    let mut kept: Vec<[f32; 2]> = Vec::with_capacity(points.len().min(MAX_POINTS));
    for p in points.iter() {
        if kept.last().is_none_or(|last| p[0] - last[0] >= MIN_GAP) {
            kept.push(*p);
        }
    }
    // The ends sit at inputs 0 and 1, keeping the outputs they had.
    let first = kept.first().map_or(0.0, |p| p[1]);
    let last = kept.last().map_or(1.0, |p| p[1]);
    kept.retain(|p| p[0] > MIN_GAP / 2.0 && p[0] < 1.0 - MIN_GAP / 2.0);
    kept.truncate(MAX_POINTS - 2);
    kept.insert(0, [0.0, first]);
    kept.push([1.0, last]);
    *points = kept;
}

fn add_to(points: &mut Vec<[f32; 2]>, x: f32, y: f32) -> Option<usize> {
    let (x, y) = (x.clamp(MIN_GAP, 1.0 - MIN_GAP), y.clamp(0.0, 1.0));
    if points.len() >= MAX_POINTS || points.iter().any(|p| (p[0] - x).abs() < MIN_GAP) {
        return None;
    }
    let at = points.iter().position(|p| p[0] > x).unwrap_or(points.len());
    points.insert(at, [x, y]);
    Some(at)
}

fn move_in(points: &mut [[f32; 2]], index: usize, x: f32, y: f32) {
    let n = points.len();
    if index >= n {
        return;
    }
    let x = if index == 0 {
        0.0
    } else if index == n - 1 {
        1.0
    } else {
        x.clamp(points[index - 1][0] + MIN_GAP, points[index + 1][0] - MIN_GAP)
    };
    points[index] = [x, y.clamp(0.0, 1.0)];
}

fn remove_from(points: &mut Vec<[f32; 2]>, index: usize) {
    if index > 0 && index + 1 < points.len() {
        points.remove(index);
    }
}

fn near_in(points: &[[f32; 2]], x: f32, y: f32, radius: f32, aspect: f32) -> Option<usize> {
    points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let dx = (p[0] - x) * aspect;
            let dy = p[1] - y;
            (i, dx * dx + dy * dy)
        })
        .filter(|(_, d)| *d <= radius * radius)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// The hue of `rgb` (0 to 1) around the colour wheel: 0 red, 1/3 green, 2/3 blue, wrapping at 1.
pub fn hue_of(rgb: [f32; 3]) -> f32 {
    let max = rgb[0].max(rgb[1]).max(rgb[2]);
    let min = rgb[0].min(rgb[1]).min(rgb[2]);
    let d = max - min;
    if d <= 1e-6 {
        return 0.0;
    }
    let h = if max == rgb[0] {
        (rgb[1] - rgb[2]) / d
    } else if max == rgb[1] {
        (rgb[2] - rgb[0]) / d + 2.0
    } else {
        (rgb[0] - rgb[1]) / d + 4.0
    };
    (h / 6.0).rem_euclid(1.0)
}

/// The colourfulness of `rgb`: the largest channel minus the smallest, 0 to 1.
pub fn chroma_of(rgb: [f32; 3]) -> f32 {
    rgb[0].max(rgb[1]).max(rgb[2]) - rgb[0].min(rgb[1]).min(rgb[2])
}

/// The Rec. 709 luma of `rgb`.
pub fn luma_of(rgb: [f32; 3]) -> f32 {
    rgb[0] * crate::LUMA[0] + rgb[1] * crate::LUMA[1] + rgb[2] * crate::LUMA[2]
}

/// The three channels' curves sampled in fine steps; a value reads the nearest step.
pub struct CurveTables {
    steps: [Vec<f32>; 3],
    hue_active: bool,
    /// Each hue/sat/lum curve's distance from straight, by its intensity; empty when it does
    /// nothing.
    hue_deltas: [Vec<f32>; HUE_MODES],
}

impl CurveTables {
    /// Channel `channel` (0 red, 1 green, 2 blue) of `v` (0 to 1) through the curves.
    #[inline]
    pub fn apply(&self, channel: usize, v: f32) -> f32 {
        let Some(table) = self.steps.get(channel) else {
            return v;
        };
        let i = (v.clamp(0.0, 1.0) * TABLE_STEPS as f32 + 0.5) as usize;
        table.get(i).copied().unwrap_or(v)
    }

    /// A pixel through the hue/sat/lum curves, like [`Curves::apply_hue_modes`].
    #[inline]
    pub fn apply_hue_modes(&self, rgb: [f32; 3]) -> [f32; 3] {
        if !self.hue_active {
            return rgb;
        }
        hue_modes(rgb, |mode, x| {
            let i = (x.clamp(0.0, 1.0) * TABLE_STEPS as f32 + 0.5) as usize;
            self.hue_deltas
                .get(mode)
                .and_then(|t| t.get(i))
                .copied()
                .unwrap_or(0.0)
        })
    }
}

/// A pixel through the hue/sat/lum curves, given how far each curve is from straight at a place
/// on its graph: the hue turns, and the saturation and luminance scale.
#[inline]
fn hue_modes(rgb: [f32; 3], delta: impl Fn(usize, f32) -> f32) -> [f32; 3] {
    let (h, s, l) = (hue_of(rgb), chroma_of(rgb), luma_of(rgb));
    // A grey pixel has no hue (it reads as 0, red), so the curves keyed on hue count for
    // less the less colour a pixel has, and not at all for a grey.
    let hue_weight = (s / HUE_FULL_CHROMA).min(1.0);
    // Half a turn each way over the height of the graph.
    let turn = delta(HUE_VS_HUE, h) * hue_weight * std::f32::consts::PI;
    let sat = (1.0 + delta(HUE_VS_SAT, h) * hue_weight + delta(LUM_VS_SAT, l) + delta(SAT_VS_SAT, s))
        .max(0.0);
    let lum = (1.0 + delta(HUE_VS_LUM, h) * hue_weight + delta(SAT_VS_LUM, s)).max(0.0);
    let mut out = rgb;
    if (sat - 1.0).abs() > 1e-6 {
        out = out.map(|v| l + (v - l) * sat);
    }
    if (lum - 1.0).abs() > 1e-6 {
        out = out.map(|v| v * lum);
    }
    if turn != 0.0 {
        out = rotate_hue(out, turn);
    }
    out.map(|v| v.clamp(0.0, 1.0))
}

/// The monotone cubic through `points` (sorted by input) at `x`.
fn spline(points: &[[f32; 2]], x: f32) -> f32 {
    let n = points.len();
    match n {
        0 => return x,
        1 => return points[0][1],
        _ => {}
    }
    if x <= points[0][0] {
        return points[0][1];
    }
    if x >= points[n - 1][0] {
        return points[n - 1][1];
    }
    // The slopes between the points, then the tangents at the points (Fritsch-Carlson).
    let secant = |k: usize| {
        let dx = points[k + 1][0] - points[k][0];
        if dx > 0.0 {
            (points[k + 1][1] - points[k][1]) / dx
        } else {
            0.0
        }
    };
    let tangent = |k: usize| -> f32 {
        if k == 0 {
            return secant(0);
        }
        if k == n - 1 {
            return secant(n - 2);
        }
        let (a, b) = (secant(k - 1), secant(k));
        if a * b <= 0.0 {
            return 0.0;
        }
        // The weighted harmonic mean keeps the curve from overshooting.
        let h0 = points[k][0] - points[k - 1][0];
        let h1 = points[k + 1][0] - points[k][0];
        let (w1, w2) = (2.0 * h1 + h0, h1 + 2.0 * h0);
        (w1 + w2) / (w1 / a + w2 / b)
    };
    let k = points
        .windows(2)
        .position(|w| x >= w[0][0] && x <= w[1][0])
        .unwrap_or(n - 2);
    let (x0, y0) = (points[k][0], points[k][1]);
    let (x1, y1) = (points[k + 1][0], points[k + 1][1]);
    let h = x1 - x0;
    if h <= 0.0 {
        return y0;
    }
    let t = (x - x0) / h;
    let (m0, m1) = (tangent(k) * h, tangent(k + 1) * h);
    let t2 = t * t;
    let t3 = t2 * t;
    let y = (2.0 * t3 - 3.0 * t2 + 1.0) * y0
        + (t3 - 2.0 * t2 + t) * m0
        + (-2.0 * t3 + 3.0 * t2) * y1
        + (t3 - t2) * m1;
    y.clamp(0.0, 1.0)
}

/// Clips `v` at black and white by `clip` (low, low soft, high, high soft).
fn soft_clip(v: f32, clip: [f32; 4]) -> f32 {
    let [low, low_soft, high, high_soft] = clip;
    let black = ((low - 0.5) * 2.0).clamp(0.0, 0.9);
    let white = (1.0 - (0.5 - high) * 2.0).clamp(0.1, 1.0);
    let mut v = v;
    // Above the knee, the value bends towards the white clip and never quite reaches it.
    let knee = white - high_soft * (white - black);
    if high_soft > 0.0 && v > knee {
        let s = white - knee;
        v = knee + s * (1.0 - (-(v - knee) / s).exp());
    } else {
        v = v.min(white);
    }
    let knee = black + low_soft * (white - black);
    if low_soft > 0.0 && v < knee {
        let s = knee - black;
        v = knee - s * (1.0 - (-(knee - v) / s).exp());
    } else {
        v = v.max(black);
    }
    v.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_changes_nothing() {
        let c = Curves::default();
        assert!(c.is_identity());
        for i in 0..=20 {
            let v = i as f32 / 20.0;
            for ch in 0..3 {
                assert!((c.apply_channel(ch, v) - v).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn a_point_bends_the_curve_through_it() {
        let mut c = Curves::default();
        assert_eq!(c.add_point(Y, 0.5, 0.7), Some(1));
        assert!(!c.is_identity());
        assert!((c.eval(Y, 0.5) - 0.7).abs() < 1e-6);
        // Smooth and rising on both sides, never above the points around it.
        let mut last = 0.0;
        for i in 0..=100 {
            let y = c.eval(Y, i as f32 / 100.0);
            assert!(y >= last - 1e-6 && y <= 1.0);
            last = y;
        }
        // The Y curve acts on every channel.
        assert!((c.apply_channel(0, 0.5) - 0.7).abs() < 1e-5);
        assert!((c.apply_channel(2, 0.5) - 0.7).abs() < 1e-5);
    }

    #[test]
    fn a_colour_curve_acts_on_its_channel_only() {
        let mut c = Curves::default();
        c.add_point(RED, 0.5, 0.3);
        assert!((c.apply_channel(0, 0.5) - 0.3).abs() < 1e-5);
        assert!((c.apply_channel(1, 0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn intensity_scales_the_change() {
        let mut c = Curves::default();
        c.add_point(Y, 0.5, 0.7);
        c.intensity[Y] = 50.0;
        assert!((c.apply_channel(1, 0.5) - 0.6).abs() < 1e-5);
        c.intensity[Y] = 0.0;
        assert!(c.is_identity());
    }

    #[test]
    fn points_move_between_their_neighbours_and_the_ends_stay_put() {
        let mut c = Curves::default();
        c.add_point(Y, 0.25, 0.25);
        c.add_point(Y, 0.75, 0.75);
        c.move_point(Y, 1, 0.9, 0.4);
        assert!(c.points[Y][1][0] < 0.75);
        c.move_point(Y, 0, 0.5, 0.2);
        assert_eq!(c.points[Y][0], [0.0, 0.2]);
        c.remove_point(Y, 0);
        assert_eq!(c.points[Y].len(), 4);
        c.remove_point(Y, 1);
        assert_eq!(c.points[Y].len(), 3);
    }

    #[test]
    fn soft_clip_lowers_white_and_lifts_black() {
        let mut c = Curves {
            soft_clip: [0.6, 0.0, 0.4, 0.0],
            ..Curves::default()
        };
        assert!((c.apply_channel(0, 1.0) - 0.8).abs() < 1e-5);
        assert!((c.apply_channel(0, 0.0) - 0.2).abs() < 1e-5);
        // Softness rounds the clip off below the white point.
        c.soft_clip = [0.5, 0.0, 0.5, 0.2];
        let near_white = c.apply_channel(0, 0.95);
        assert!(near_white < 0.95 && near_white > 0.8);
    }

    #[test]
    fn tables_match_the_curves() {
        let mut c = Curves::default();
        c.add_point(Y, 0.3, 0.5);
        c.add_point(BLUE, 0.6, 0.4);
        let t = c.tables();
        for i in 0..=50 {
            let v = i as f32 / 50.0;
            for ch in 0..3 {
                assert!((t.apply(ch, v) - c.apply_channel(ch, v)).abs() < 2e-3);
            }
        }
    }

    #[test]
    fn hostile_curves_are_made_safe() {
        let mut c = Curves::default();
        c.points[Y] = vec![[f32::NAN, 0.5], [0.4, 9.0], [0.4, 0.1], [-3.0, 0.2], [2.0, 0.9]];
        c.intensity[RED] = f32::INFINITY;
        c.soft_clip[2] = -5.0;
        let s = c.sanitized();
        let p = &s.points[Y];
        assert_eq!(p.first().map(|p| p[0]), Some(0.0));
        assert_eq!(p.last().map(|p| p[0]), Some(1.0));
        assert!(p.windows(2).all(|w| w[1][0] > w[0][0]));
        assert!(p.iter().all(|q| (0.0..=1.0).contains(&q[1])));
        assert_eq!(s.intensity[RED], 100.0);
        assert_eq!(s.soft_clip[2], 0.0);
    }

    #[test]
    fn the_default_changes_nothing_including_hue_curves() {
        let c = Curves::default();
        assert!(c.hue_is_identity());
        for m in 0..HUE_MODES {
            assert!(c.is_hue_straight(m));
            for i in 0..=20 {
                let v = i as f32 / 20.0;
                assert!((c.eval_hue(m, v) - v).abs() < 1e-6);
            }
        }
        let rgb = [0.2, 0.5, 0.8];
        assert!(c.apply_hue_modes(rgb) == rgb);
    }

    #[test]
    fn hue_curves_are_added_moved_and_removed_like_the_others() {
        let mut c = Curves::default();
        assert_eq!(c.add_hue_point(HUE_VS_SAT, 0.5, 0.8), Some(1));
        assert_eq!(c.hue_points[HUE_VS_SAT].len(), 3);
        c.move_hue_point(HUE_VS_SAT, 1, 0.1, 0.4);
        assert!(c.hue_points[HUE_VS_SAT][1][0] < 0.5);
        c.remove_hue_point(HUE_VS_SAT, 1);
        assert_eq!(c.hue_points[HUE_VS_SAT].len(), 2);
        assert!(c.hue_is_identity());
    }

    #[test]
    fn hue_vs_sat_colours_only_its_hue() {
        let mut c = Curves::default();
        // A bump in the middle of the graph: cyan (hue 0.5) is more colourful.
        c.add_hue_point(HUE_VS_SAT, 0.5, 0.8);
        let cyan = [0.2, 0.5, 0.5];
        let after = c.apply_hue_modes(cyan);
        assert!(chroma_of(after) > chroma_of(cyan) + 0.05, "{after:?}");
        // Red (hue 0) sits at the end of the graph, where the bump is zero.
        let red = [0.6, 0.4, 0.4];
        assert!((chroma_of(c.apply_hue_modes(red)) - chroma_of(red)).abs() < 1e-3);
    }

    #[test]
    fn hue_vs_hue_turns_the_chosen_hue() {
        let mut c = Curves::default();
        c.add_hue_point(HUE_VS_HUE, 0.5, 1.0); // cyan turns half a turn
        let cyan = [0.2, 0.5, 0.5];
        let after = c.apply_hue_modes(cyan);
        // Half a turn sends cyan towards magenta: red up, green down.
        assert!(after[0] > cyan[0] + 0.05, "{after:?}");
        assert!(after[1] < cyan[1]);
    }

    #[test]
    fn lum_and_sat_curves_scale_luminance_and_saturation() {
        let mut c = Curves::default();
        c.add_hue_point(HUE_VS_LUM, 0.5, 0.8);
        let cyan = [0.2, 0.5, 0.5];
        assert!(luma_of(c.apply_hue_modes(cyan)) > luma_of(cyan) + 0.05);
        let red = [0.6, 0.4, 0.4];
        assert!((luma_of(c.apply_hue_modes(red)) - luma_of(red)).abs() < 1e-3);
        // Lum vs Sat keys on the luminance, Sat vs Sat on the saturation.
        let mut c = Curves::default();
        c.move_hue_point(LUM_VS_SAT, 0, 0.0, 1.0); // delta = 1 - x
        c.move_hue_point(SAT_VS_SAT, 0, 0.0, 1.0);
        let pale = [0.5, 0.45, 0.45]; // low chroma, low luma
        let vivid = [0.5, 0.2, 0.2]; // high chroma, high luma
        let gain_chroma = |p: [f32; 3]| chroma_of(c.apply_hue_modes(p)) / chroma_of(p);
        assert!(gain_chroma(pale) > gain_chroma(vivid));
    }

    #[test]
    fn hue_keyed_curves_leave_greys_alone() {
        for mode in [HUE_VS_HUE, HUE_VS_SAT, HUE_VS_LUM] {
            let mut c = Curves::default();
            // Red (hue 0) is pushed all the way up; a grey must not be taken for red.
            c.move_hue_point(mode, 0, 0.0, 1.0);
            for grey in [[0.0; 3], [0.5; 3], [1.0; 3]] {
                assert_eq!(c.apply_hue_modes(grey), grey, "mode {mode}");
            }
            // A barely coloured pixel moves less than a vivid one.
            let faint = [0.52, 0.5, 0.5];
            let vivid = [0.5, 0.2, 0.2];
            let moved = |p: [f32; 3]| {
                let q = c.apply_hue_modes(p);
                (0..3).map(|i| (q[i] - p[i]).abs()).sum::<f32>()
            };
            assert!(moved(faint) < moved(vivid), "mode {mode}");
        }
    }

    #[test]
    fn tables_agree_with_the_direct_hue_curves() {
        let mut c = Curves::default();
        for m in 0..HUE_MODES {
            c.add_hue_point(m, 0.3, 0.7);
            c.add_hue_point(m, 0.7, 0.4);
        }
        c.hue_intensity[HUE_VS_LUM] = 60.0;
        let tables = c.tables();
        for i in 0..2000u32 {
            let rgb = [
                (i % 17) as f32 / 16.0,
                (i % 29) as f32 / 28.0,
                (i % 13) as f32 / 12.0,
            ];
            let (a, b) = (c.apply_hue_modes(rgb), tables.apply_hue_modes(rgb));
            for k in 0..3 {
                assert!((a[k] - b[k]).abs() < 0.01, "{rgb:?}: {a:?} vs {b:?}");
            }
        }
        // Curves that do nothing leave the tables doing nothing.
        let none = Curves::default().tables();
        assert_eq!(none.apply_hue_modes([0.2, 0.5, 0.8]), [0.2, 0.5, 0.8]);
    }

    #[test]
    fn hue_curves_survive_saving_and_old_files_load_straight() {
        let mut c = Curves::default();
        c.add_hue_point(HUE_VS_SAT, 0.5, 0.8);
        assert!(!c.is_identity());
        let json = serde_json::to_string(&c).unwrap();
        let back: Curves = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c);
        // A file from before the hue curves had none: they default to straight.
        let mut old: serde_json::Value = serde_json::to_value(Curves::default()).unwrap();
        old.as_object_mut().unwrap().remove("hue_points");
        old.as_object_mut().unwrap().remove("hue_intensity");
        let back: Curves = serde_json::from_value(old).unwrap();
        assert!(back.hue_is_identity());
    }

    #[test]
    fn hostile_hue_curves_are_made_safe() {
        let mut c = Curves::default();
        c.hue_points[HUE_VS_SAT] = vec![
            [f32::NAN, 0.5],
            [0.4, 9.0],
            [0.4, 0.1],
            [-3.0, 0.2],
            [2.0, 0.9],
        ];
        c.hue_intensity[HUE_VS_LUM] = f32::INFINITY;
        let s = c.sanitized();
        let p = &s.hue_points[HUE_VS_SAT];
        assert_eq!(p.first().map(|p| p[0]), Some(0.0));
        assert_eq!(p.last().map(|p| p[0]), Some(1.0));
        assert!(p.windows(2).all(|w| w[1][0] > w[0][0]));
        assert!(p.iter().all(|q| (0.0..=1.0).contains(&q[1])));
        assert_eq!(s.hue_intensity[HUE_VS_LUM], 100.0);
        assert!(!s.is_identity());
    }
}

#[cfg(test)]
mod grade_tests {
    use super::*;
    use crate::ColorGrade;

    #[test]
    fn a_grade_with_a_curve_changes_the_picture_and_saves_it() {
        let mut grade = ColorGrade::default();
        assert!(!grade.has_effect());
        grade.curves.add_point(Y, 0.5, 0.75);
        assert!(grade.has_effect());
        let mut px = [128u8, 128, 128, 255];
        grade.apply_rgba8(&mut px);
        assert!((i32::from(px[0]) - 191).abs() <= 2, "{px:?}");
        assert_eq!(px[3], 255);
        let one = grade.apply([0.5, 0.5, 0.5]);
        assert!((one[1] - 0.75).abs() < 0.01);
        let json = serde_json::to_string(&grade).unwrap_or_default();
        let back: ColorGrade = serde_json::from_str(&json).unwrap_or_default();
        assert_eq!(back.curves, grade.curves);
        // A grade without curves saves without them, as before.
        let plain = serde_json::to_string(&ColorGrade::default()).unwrap_or_default();
        assert!(!plain.contains("curves"));
    }
}

#[cfg(test)]
mod bench {
    use super::*;
    use crate::ColorGrade;

    /// Time to grade a 960 x 540 picture (run with `--release --ignored --nocapture`).
    #[test]
    #[ignore]
    fn grading_speed() {
        let frame: Vec<u8> = (0..960 * 540 * 4).map(|i| (i * 7 % 251) as u8).collect();
        let plain = ColorGrade {
            gain: [1.1, 1.0, 0.95],
            ..ColorGrade::default()
        };
        let mut curved = plain.clone();
        curved.curves.add_point(Y, 0.3, 0.2);
        curved.curves.add_point(Y, 0.7, 0.8);
        curved.curves.add_point(RED, 0.5, 0.6);
        let mut saturated = curved.clone();
        saturated.saturation = 1.2;
        for (name, grade) in [
            ("gain only", &plain),
            ("gain + curves", &curved),
            ("gain + curves + saturation", &saturated),
        ] {
            let mut best = f64::MAX;
            for _ in 0..15 {
                let mut px = frame.clone();
                let t = std::time::Instant::now();
                grade.apply_rgba8(&mut px);
                best = best.min(t.elapsed().as_secs_f64() * 1000.0);
            }
            println!("BENCH {name}: {best:.2} ms per 960x540 picture");
        }
        let t = std::time::Instant::now();
        for _ in 0..20 {
            std::hint::black_box(curved.curves.tables());
        }
        println!("BENCH building the tables: {:.3} ms", t.elapsed().as_secs_f64() * 1000.0 / 20.0);
    }
}
