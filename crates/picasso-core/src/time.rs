//! Rational frame rates and timecode helpers.
//!
//! Timeline positions are integer frames at the project frame rate. Source media
//! is addressed in seconds so that clips with a different native rate map
//! cleanly onto the timeline.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rational {
    pub num: i64,
    pub den: i64,
}

impl Rational {
    pub const fn new(num: i64, den: i64) -> Self {
        Self { num, den }
    }

    pub fn as_f64(self) -> f64 {
        if self.den == 0 {
            0.0
        } else {
            self.num as f64 / self.den as f64
        }
    }

    pub fn is_valid(self) -> bool {
        self.num > 0 && self.den > 0
    }

    /// Parses "30000/1001", "25", or "29.97".
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if let Some((n, d)) = s.split_once('/') {
            let r = Self::new(n.trim().parse().ok()?, d.trim().parse().ok()?);
            return r.is_valid().then_some(r);
        }
        if let Ok(n) = s.parse::<i64>() {
            return (n > 0).then_some(Self::new(n, 1));
        }
        let f: f64 = s.parse().ok()?;
        Self::from_f64(f)
    }

    /// Maps common NTSC decimals back to their exact rational form.
    pub fn from_f64(f: f64) -> Option<Self> {
        if !(f.is_finite() && f > 0.0) {
            return None;
        }
        for base in [24, 30, 48, 60, 120] {
            let ntsc = base as f64 * 1000.0 / 1001.0;
            if (f - ntsc).abs() < 0.005 {
                return Some(Self::new(base * 1000, 1001));
            }
        }
        if (f - f.round()).abs() < 1e-6 {
            return Some(Self::new(f.round() as i64, 1));
        }
        Some(Self::new((f * 1000.0).round() as i64, 1000))
    }

    /// Nominal integer rate used for timecode labels (29.97 -> 30).
    pub fn nominal(self) -> i64 {
        self.as_f64().round().max(1.0) as i64
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.den == 1 {
            write!(f, "{}", self.num)
        } else {
            write!(f, "{:.3}", self.as_f64())
        }
    }
}

pub fn frames_to_seconds(frames: i64, fps: Rational) -> f64 {
    frames as f64 * fps.den as f64 / fps.num as f64
}

/// Converts seconds to the frame containing that instant.
pub fn seconds_to_frames(seconds: f64, fps: Rational) -> i64 {
    // The epsilon keeps exact frame boundaries (e.g. 1/30 s * 30) from rounding down.
    (seconds * fps.num as f64 / fps.den as f64 + 1e-6).floor() as i64
}

/// Non-drop-frame timecode `HH:MM:SS:FF`.
pub fn format_timecode(frame: i64, fps: Rational) -> String {
    let nominal = fps.nominal();
    let sign = if frame < 0 { "-" } else { "" };
    let f = frame.abs();
    let ff = f % nominal;
    let total_secs = f / nominal;
    format!(
        "{sign}{:02}:{:02}:{:02}:{:02}",
        total_secs / 3600,
        (total_secs / 60) % 60,
        total_secs % 60,
        ff
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rates() {
        assert_eq!(Rational::parse("30000/1001"), Some(Rational::new(30000, 1001)));
        assert_eq!(Rational::parse("25"), Some(Rational::new(25, 1)));
        assert_eq!(Rational::parse("29.97"), Some(Rational::new(30000, 1001)));
        assert_eq!(Rational::parse("0/0"), None);
    }

    #[test]
    fn frame_second_roundtrip() {
        let fps = Rational::new(30000, 1001);
        for f in [0, 1, 29, 30, 1799, 107_892] {
            assert_eq!(seconds_to_frames(frames_to_seconds(f, fps), fps), f);
        }
    }

    #[test]
    fn timecode() {
        let fps = Rational::new(25, 1);
        assert_eq!(format_timecode(0, fps), "00:00:00:00");
        assert_eq!(format_timecode(25 * 3661 + 7, fps), "01:01:01:07");
    }
}
