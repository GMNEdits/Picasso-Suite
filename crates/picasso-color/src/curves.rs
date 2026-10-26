//! The Color page's custom curves: a Y curve that acts on every channel, then one curve each
//! for red, green and blue, how strongly each counts, and a soft clip at black and white.
//!
//! A curve is a list of points (input, output), both 0 to 1, sorted by input, with one point at
//! input 0 and one at input 1. Between them it is a monotone cubic (Fritsch-Carlson), so it is
//! smooth and never overshoots between two points. The default curves are straight and change
//! nothing.

use serde::{Deserialize, Serialize};

/// The curves, in the order of the Edit buttons.
pub const Y: usize = 0;
pub const RED: usize = 1;
pub const GREEN: usize = 2;
pub const BLUE: usize = 3;

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

    /// Whether the curves change nothing.
    pub fn is_identity(&self) -> bool {
        (0..4).all(|c| self.is_straight(c) || self.intensity[c] <= 0.0)
            && self.soft_clip == SOFT_CLIP_NONE
    }

    /// The same curves made safe: numbers in range, points sorted and apart, ends at 0 and 1.
    pub fn sanitized(mut self) -> Self {
        for points in &mut self.points {
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
        for v in &mut self.intensity {
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

    /// Channel `channel` (0 red, 1 green, 2 blue) of a pixel through the curves: the Y curve, then
    /// the channel's own, each by its intensity, then the soft clip.
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

    /// The curves of the three channels as tables, for grading many pixels.
    pub fn tables(&self) -> CurveTables {
        CurveTables {
            steps: std::array::from_fn(|c| {
                (0..=TABLE_STEPS)
                    .map(|i| self.apply_channel(c, i as f32 / TABLE_STEPS as f32))
                    .collect()
            }),
        }
    }

    /// Adds a point to curve `curve` at `[x, y]` and returns its index, or None when it is too
    /// close to another point or the curve is full.
    pub fn add_point(&mut self, curve: usize, x: f32, y: f32) -> Option<usize> {
        let points = self.points.get_mut(curve)?;
        let (x, y) = (x.clamp(MIN_GAP, 1.0 - MIN_GAP), y.clamp(0.0, 1.0));
        if points.len() >= MAX_POINTS || points.iter().any(|p| (p[0] - x).abs() < MIN_GAP) {
            return None;
        }
        let at = points.iter().position(|p| p[0] > x).unwrap_or(points.len());
        points.insert(at, [x, y]);
        Some(at)
    }

    /// Moves point `index` of curve `curve` to `[x, y]`. The end points only move up and down;
    /// the others stay between their neighbours.
    pub fn move_point(&mut self, curve: usize, index: usize, x: f32, y: f32) {
        let Some(points) = self.points.get_mut(curve) else {
            return;
        };
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

    /// Removes point `index` of curve `curve`; the end points stay.
    pub fn remove_point(&mut self, curve: usize, index: usize) {
        if let Some(points) = self.points.get_mut(curve) {
            if index > 0 && index + 1 < points.len() {
                points.remove(index);
            }
        }
    }

    /// The point of curve `curve` nearest `[x, y]`, if one is within `radius` (in the same 0..1
    /// units, measured separately along each axis with `aspect` = width / height of the graph).
    pub fn point_near(&self, curve: usize, x: f32, y: f32, radius: f32, aspect: f32) -> Option<usize> {
        let points = self.points.get(curve)?;
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
}

/// The three channels' curves sampled in fine steps; a value reads the nearest step.
pub struct CurveTables {
    steps: [Vec<f32>; 3],
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
