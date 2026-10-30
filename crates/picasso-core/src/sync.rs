//! Finding how far one recording is shifted from another, by what they sound like: the sound of a
//! clip from the camera and the sound of the same moment from a recorder, say, so that the clips
//! can be lined up (Clip > Auto Align Clips > Based on Waveform).
//!
//! The recordings are compared with a generalised cross-correlation with phase transform
//! (GCC-PHAT): each frequency votes only with the delay it shows, not with how loud it is, so two
//! microphones that heard the same thing at different levels and with different tone still agree.
//! Only the voice band counts (150 Hz to 3.4 kHz), where that agreement is.
//!
//! The recordings are mono `f32` at one sample rate (the caller brings them to 8 kHz); the answer is
//! in samples. A match that is not clearly better than every other delay is not an answer: a wrong
//! alignment is worse than none.

/// The sample rate the recordings are expected at, for the band that is compared.
pub const SYNC_RATE: u32 = 8_000;
/// How much better than the best other delay a match has to be to count.
pub const MIN_STRENGTH: f32 = 2.0;

/// Where the match was found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lag {
    /// `b[n]` sounds like `a[n + samples]`: positive when what `b` has at its start comes later in
    /// `a`, negative when it comes earlier.
    pub samples: i64,
    /// The height of the match over the best other one: 1 is a coin toss, [`MIN_STRENGTH`] or more
    /// is a match.
    pub strength: f32,
}

/// In-place radix-2 FFT of `re`/`im` (the length is a power of two); `inverse` also divides by it.
fn fft(re: &mut [f64], im: &mut [f64], inverse: bool) {
    let n = re.len();
    debug_assert!(n.is_power_of_two() && im.len() == n);
    // Bit reversal.
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = 2.0 * std::f64::consts::PI / len as f64 * if inverse { 1.0 } else { -1.0 };
        let (w_re, w_im) = (angle.cos(), angle.sin());
        let half = len / 2;
        for start in (0..n).step_by(len) {
            let (mut c_re, mut c_im) = (1.0, 0.0);
            for k in 0..half {
                let (a, b) = (start + k, start + k + half);
                let t_re = re[b] * c_re - im[b] * c_im;
                let t_im = re[b] * c_im + im[b] * c_re;
                re[b] = re[a] - t_re;
                im[b] = im[a] - t_im;
                re[a] += t_re;
                im[a] += t_im;
                let next = c_re * w_re - c_im * w_im;
                c_im = c_re * w_im + c_im * w_re;
                c_re = next;
            }
        }
        len <<= 1;
    }
    if inverse {
        let scale = 1.0 / n as f64;
        re.iter_mut().for_each(|v| *v *= scale);
        im.iter_mut().for_each(|v| *v *= scale);
    }
}

/// The most samples a recording may have: about eight minutes at [`SYNC_RATE`]. More is cut.
pub const MAX_SAMPLES: usize = 4_000_000;

/// Whether a recording has anything to compare: it is not silent and not a constant.
fn has_sound(x: &[f32]) -> bool {
    if x.len() < 256 {
        return false;
    }
    let mean = x.iter().map(|v| f64::from(*v)).sum::<f64>() / x.len() as f64;
    x.iter().map(|v| (f64::from(*v) - mean).powi(2)).sum::<f64>() / x.len() as f64 > 1e-10
}

/// How `b` is shifted from `a`: see [`Lag`]. `None` when either has no sound, or when no delay is
/// clearly the one (the recordings do not share a sound, or it is buried in noise).
pub fn best_lag(a: &[f32], b: &[f32]) -> Option<Lag> {
    let (a, b) = (&a[..a.len().min(MAX_SAMPLES)], &b[..b.len().min(MAX_SAMPLES)]);
    if !has_sound(a) || !has_sound(b) {
        return None;
    }
    let n = (a.len() + b.len()).next_power_of_two();
    let load = |x: &[f32]| {
        let mean = x.iter().map(|v| f64::from(*v)).sum::<f64>() / x.len() as f64;
        let mut re = vec![0.0f64; n];
        for (slot, v) in re.iter_mut().zip(x) {
            *slot = f64::from(*v) - mean;
        }
        (re, vec![0.0f64; n])
    };
    let (mut a_re, mut a_im) = load(a);
    let (mut b_re, mut b_im) = load(b);
    fft(&mut a_re, &mut a_im, false);
    fft(&mut b_re, &mut b_im, false);
    // The cross spectrum A * conj(B), with every frequency kept to its phase and only the voice
    // band kept at all.
    let (low, high) = (150.0, 3_400.0);
    for i in 0..n {
        let hz = i.min(n - i) as f64 * f64::from(SYNC_RATE) / n as f64;
        let re = a_re[i] * b_re[i] + a_im[i] * b_im[i];
        let im = a_im[i] * b_re[i] - a_re[i] * b_im[i];
        let size = re.hypot(im);
        if !(low..=high).contains(&hz) || size < 1e-12 {
            a_re[i] = 0.0;
            a_im[i] = 0.0;
        } else {
            a_re[i] = re / size;
            a_im[i] = im / size;
        }
    }
    fft(&mut a_re, &mut a_im, true);
    // The delays that can be: index k >= 0 is `a` ahead by k, index n - k is `b` ahead by k.
    let max_positive = a.len() - 1;
    let max_negative = b.len() - 1;
    let at = |lag: i64| -> f64 {
        let i = if lag >= 0 { lag as usize } else { n - (-lag) as usize };
        a_re[i].abs()
    };
    let (mut best, mut best_lag) = (0.0f64, 0i64);
    for lag in -(max_negative as i64)..=(max_positive as i64) {
        let v = at(lag);
        if v > best {
            (best, best_lag) = (v, lag);
        }
    }
    if best <= 0.0 {
        return None;
    }
    // The best other delay: further from the peak than a few samples, which belong to it.
    let mut other = 0.0f64;
    for lag in -(max_negative as i64)..=(max_positive as i64) {
        if (lag - best_lag).abs() > 40 {
            other = other.max(at(lag));
        }
    }
    let strength = if other <= 0.0 { f32::MAX } else { (best / other) as f32 };
    (strength >= MIN_STRENGTH).then_some(Lag {
        samples: best_lag,
        strength,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sound that does not repeat: noise from a fixed seed.
    fn noise(len: usize, seed: u64) -> Vec<f32> {
        let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (0..len)
            .map(|_| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                ((state >> 33) as f32 / (1u64 << 31) as f32) - 1.0
            })
            .collect()
    }

    #[test]
    fn the_fft_goes_there_and_back() {
        let signal = noise(1024, 1);
        let mut re: Vec<f64> = signal.iter().map(|v| f64::from(*v)).collect();
        let mut im = vec![0.0; 1024];
        fft(&mut re, &mut im, false);
        fft(&mut re, &mut im, true);
        for (a, b) in re.iter().zip(&signal) {
            assert!((a - f64::from(*b)).abs() < 1e-9);
        }
    }

    #[test]
    fn a_recording_that_starts_later_in_the_other_is_found() {
        let a = noise(40_000, 7);
        // b is a, from sample 12_345 on.
        let b = a[12_345..30_000].to_vec();
        let lag = best_lag(&a, &b).expect("the same sound is found");
        assert_eq!(lag.samples, 12_345);
        assert!(lag.strength > 10.0, "{lag:?}");
    }

    #[test]
    fn a_recording_that_starts_earlier_has_a_negative_lag() {
        let a = noise(40_000, 7);
        let b = a[12_345..30_000].to_vec();
        // The other way round: a's start is 12_345 before b's.
        let lag = best_lag(&b, &a).expect("found");
        assert_eq!(lag.samples, -12_345);
    }

    #[test]
    fn a_recording_with_noise_and_another_level_is_still_found() {
        let a = noise(60_000, 3);
        let hiss = noise(60_000, 99);
        let b: Vec<f32> = a[5_000..45_000]
            .iter()
            .zip(&hiss[5_000..45_000])
            .map(|(s, h)| s * 0.2 + h * 0.15)
            .collect();
        let lag = best_lag(&a, &b).expect("found under the noise");
        assert_eq!(lag.samples, 5_000);
    }

    #[test]
    fn recordings_that_share_nothing_are_not_given_an_answer() {
        assert_eq!(best_lag(&noise(40_000, 1), &noise(30_000, 2)), None);
    }

    #[test]
    fn silence_and_a_hum_of_one_note_are_not_given_an_answer() {
        let silence = vec![0.0; 20_000];
        assert_eq!(best_lag(&silence, &noise(20_000, 1)), None);
        assert_eq!(best_lag(&noise(20_000, 1), &silence), None);
        assert_eq!(best_lag(&[0.5; 20_000], &noise(20_000, 1)), None);
        assert_eq!(best_lag(&[], &[]), None);
    }
}
