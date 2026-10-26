//! Loudness as ITU-R BS.1770 and EBU R 128 define it: the mix is K-weighted (a high shelf and a
//! high-pass filter, with the coefficients the recommendation publishes for 48 kHz), its power is
//! measured in blocks of 100 ms, and from those come the momentary (400 ms), short-term (3 s),
//! integrated (gated at -70 LUFS and 10 LU under the ungated level) and range (EBU Tech 3342:
//! short-term values gated at -70 LUFS and 20 LU under, the 10th to the 95th percentile) values.
//! The true peak is estimated by 4x oversampling with cubic interpolation.

use std::collections::VecDeque;

/// Samples in one 100 ms block of the 48 kHz mix.
pub const LOUDNESS_BLOCK: usize = 4800;

/// The loudness programmes aim at (EBU R 128); the meter's LU are relative to it.
pub const TARGET_LUFS: f64 = -23.0;

/// The most 400 ms blocks kept for the integrated value: ten hours.
const MAX_GATING_BLOCKS: usize = 360_000;

/// One biquad stage, direct form I.
#[derive(Clone, Copy, Debug, Default)]
struct Biquad {
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl Biquad {
    fn run(&mut self, x: f64, b: [f64; 3], a: [f64; 2]) -> f64 {
        let y = b[0] * x + b[1] * self.x1 + b[2] * self.x2 - a[0] * self.y1 - a[1] * self.y2;
        (self.x2, self.x1, self.y2, self.y1) = (self.x1, x, self.y1, y);
        y
    }
}

/// BS.1770's pre-filter (a high shelf of about +4 dB) and RLB filter (a high-pass), at 48 kHz.
const SHELF_B: [f64; 3] = [
    1.535_124_859_586_97,
    -2.691_696_189_406_38,
    1.198_392_810_852_85,
];
const SHELF_A: [f64; 2] = [-1.690_659_293_182_41, 0.732_480_774_215_85];
const HIGH_B: [f64; 3] = [1.0, -2.0, 1.0];
const HIGH_A: [f64; 2] = [-1.990_047_454_833_98, 0.990_072_250_366_21];

/// K-weights a stereo signal and sums its power into 100 ms blocks. Allocation-free, for the
/// audio thread.
#[derive(Clone, Debug, Default)]
pub struct KWeighting {
    stages: [[Biquad; 2]; 2],
    sum: f64,
    count: usize,
}

impl KWeighting {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one stereo sample; returns the block's mean power when it completes a block.
    pub fn push(&mut self, left: f32, right: f32) -> Option<f64> {
        for (c, x) in [left, right].into_iter().enumerate() {
            let [shelf, high] = &mut self.stages[c];
            let y = high.run(shelf.run(f64::from(x), SHELF_B, SHELF_A), HIGH_B, HIGH_A);
            self.sum += y * y;
        }
        self.count += 1;
        (self.count == LOUDNESS_BLOCK).then(|| {
            let mean = self.sum / LOUDNESS_BLOCK as f64;
            (self.sum, self.count) = (0.0, 0);
            mean
        })
    }
}

/// An estimate of the true (between-sample) peak: the samples and three points between each
/// two, by cubic interpolation. Allocation-free, for the audio thread.
#[derive(Clone, Debug, Default)]
pub struct TruePeak {
    last: [[f32; 3]; 2],
    /// The loudest value since it was last taken.
    pub peak: f32,
}

impl TruePeak {
    pub fn push(&mut self, left: f32, right: f32) {
        for (c, x3) in [left, right].into_iter().enumerate() {
            let [x0, x1, x2] = self.last[c];
            for t in [0.25f32, 0.5, 0.75] {
                // Catmull-Rom between x1 and x2.
                let v = 0.5
                    * (2.0 * x1
                        + (x2 - x0) * t
                        + (2.0 * x0 - 5.0 * x1 + 4.0 * x2 - x3) * t * t
                        + (3.0 * x1 - x0 - 3.0 * x2 + x3) * t * t * t);
                self.peak = self.peak.max(v.abs());
            }
            self.peak = self.peak.max(x3.abs());
            self.last[c] = [x1, x2, x3];
        }
    }
}

/// Loudness in LUFS of a mean power.
fn lufs(power: f64) -> f64 {
    -0.691 + 10.0 * power.max(1e-20).log10()
}

/// The meter: fed 100 ms block powers, it gives the BS.1770 / R 128 values.
#[derive(Clone, Debug, Default)]
pub struct Loudness {
    /// The last 30 blocks (3 s).
    recent: VecDeque<f64>,
    /// The power of every 400 ms block, one each 100 ms (75 % overlap), for the integrated value.
    gating: Vec<f64>,
    /// Each short-term loudness, one each 100 ms, for the range.
    short_terms: Vec<f64>,
    short_max: Option<f64>,
    true_peak: f32,
}

impl Loudness {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets everything measured.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Adds the mean power of one 100 ms block.
    pub fn push_block(&mut self, power: f64) {
        if !power.is_finite() {
            return;
        }
        self.recent.push_back(power.max(0.0));
        if self.recent.len() > 30 {
            self.recent.pop_front();
        }
        if let Some(m) = self.window_power(4) {
            if self.gating.len() < MAX_GATING_BLOCKS {
                self.gating.push(m);
            }
        }
        if let Some(s) = self.short_term() {
            self.short_max = Some(self.short_max.map_or(s, |m| m.max(s)));
            if self.short_terms.len() < MAX_GATING_BLOCKS {
                self.short_terms.push(s);
            }
        }
    }

    /// Notes a true-peak value (linear, 1.0 is full scale).
    pub fn push_peak(&mut self, peak: f32) {
        if peak.is_finite() {
            self.true_peak = self.true_peak.max(peak);
        }
    }

    fn window_power(&self, blocks: usize) -> Option<f64> {
        (self.recent.len() >= blocks)
            .then(|| self.recent.iter().rev().take(blocks).sum::<f64>() / blocks as f64)
    }

    /// The loudness of the last 400 ms, in LUFS.
    pub fn momentary(&self) -> Option<f64> {
        self.window_power(4).map(lufs)
    }

    /// The loudness of the last 3 s, in LUFS.
    pub fn short_term(&self) -> Option<f64> {
        self.window_power(30).map(lufs)
    }

    /// The loudest short-term value since the last reset, in LUFS.
    pub fn short_max(&self) -> Option<f64> {
        self.short_max
    }

    /// The gated loudness of everything measured, in LUFS.
    pub fn integrated(&self) -> Option<f64> {
        let loud: Vec<f64> = self
            .gating
            .iter()
            .copied()
            .filter(|p| lufs(*p) > -70.0)
            .collect();
        if loud.is_empty() {
            return None;
        }
        let relative = lufs(loud.iter().sum::<f64>() / loud.len() as f64) - 10.0;
        let kept: Vec<f64> = loud.into_iter().filter(|p| lufs(*p) > relative).collect();
        (!kept.is_empty()).then(|| lufs(kept.iter().sum::<f64>() / kept.len() as f64))
    }

    /// How much the short-term loudness varies (EBU Tech 3342), in LU.
    pub fn range(&self) -> Option<f64> {
        let loud: Vec<f64> = self
            .short_terms
            .iter()
            .copied()
            .filter(|l| *l > -70.0)
            .collect();
        if loud.len() < 2 {
            return None;
        }
        let mean_power = loud
            .iter()
            .map(|l| 10f64.powf((l + 0.691) / 10.0))
            .sum::<f64>()
            / loud.len() as f64;
        let relative = lufs(mean_power) - 20.0;
        let mut kept: Vec<f64> = loud.into_iter().filter(|l| *l > relative).collect();
        if kept.len() < 2 {
            return None;
        }
        kept.sort_by(f64::total_cmp);
        let at = |q: f64| kept[((kept.len() - 1) as f64 * q).round() as usize];
        Some(at(0.95) - at(0.10))
    }

    /// The highest true peak since the last reset, in dBTP (None before any sound).
    pub fn true_peak_db(&self) -> Option<f32> {
        (self.true_peak > 0.0).then(|| 20.0 * self.true_peak.log10())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeds `seconds` of a stereo sine of `freq` Hz and peak `amp` through the filter.
    fn feed(meter: &mut Loudness, k: &mut KWeighting, freq: f64, amp: f64, seconds: f64) {
        let n = (seconds * 48_000.0) as usize;
        for i in 0..n {
            let v = (amp * (std::f64::consts::TAU * freq * i as f64 / 48_000.0).sin()) as f32;
            if let Some(p) = k.push(v, v) {
                meter.push_block(p);
            }
        }
    }

    #[test]
    fn a_1khz_tone_reads_its_level() {
        // BS.1770's check: a 1 kHz sine at -20 dBFS (peak) in both channels reads about
        // -20 LUFS (the K-weighting gives +0.69 dB at 1 kHz, which -0.691 takes back).
        let (mut meter, mut k) = (Loudness::new(), KWeighting::new());
        feed(&mut meter, &mut k, 1000.0, 0.1, 5.0);
        let m = meter.momentary().unwrap();
        let s = meter.short_term().unwrap();
        let i = meter.integrated().unwrap();
        for v in [m, s, i] {
            assert!((v + 20.0).abs() < 0.2, "{v}");
        }
        assert!(meter.range().unwrap() < 0.5, "a steady tone hardly varies");
    }

    #[test]
    fn quiet_parts_are_gated_out_of_the_integrated_value() {
        let (mut meter, mut k) = (Loudness::new(), KWeighting::new());
        feed(&mut meter, &mut k, 1000.0, 0.1, 5.0);
        // Twenty seconds 40 dB quieter: under the relative gate, so left out.
        feed(&mut meter, &mut k, 1000.0, 0.001, 20.0);
        let i = meter.integrated().unwrap();
        assert!((i + 20.0).abs() < 0.5, "{i}");
        assert!(meter.short_max().unwrap() > -20.5);
        meter.reset();
        assert!(meter.integrated().is_none() && meter.momentary().is_none());
    }

    #[test]
    fn the_true_peak_sees_between_the_samples() {
        // A sine at a quarter of the rate, sampled away from its crests: the samples peak at
        // 0.707 of it, the true peak is 1. This is the hardest case for the estimate, which
        // finds most of what lies between the samples (0.88 here).
        let mut tp = TruePeak::default();
        for i in 0..64 {
            let v = (std::f32::consts::TAU * 0.25 * i as f32 + std::f32::consts::FRAC_PI_4).sin();
            tp.push(v, v);
        }
        assert!(tp.peak > 0.85, "{}", tp.peak);
        let mut meter = Loudness::new();
        assert!(meter.true_peak_db().is_none());
        meter.push_peak(0.5);
        assert!((meter.true_peak_db().unwrap() + 6.02).abs() < 0.05);
    }

    #[test]
    fn silence_and_nonsense_give_no_values() {
        let mut meter = Loudness::new();
        for _ in 0..40 {
            meter.push_block(0.0);
        }
        meter.push_block(f64::NAN);
        assert!(meter.integrated().is_none());
        assert!(meter.range().is_none());
    }
}
