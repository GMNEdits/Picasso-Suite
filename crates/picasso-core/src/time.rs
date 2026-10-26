//! Frame rates and timecode.

use serde::{Deserialize, Serialize};

/// A frame rate as a fraction, so 29.97 is exactly 30000/1001.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fps {
    pub num: u32,
    pub den: u32,
}

impl Fps {
    pub const FPS_24: Fps = Fps { num: 24, den: 1 };
    pub const FPS_25: Fps = Fps { num: 25, den: 1 };
    pub const FPS_30: Fps = Fps { num: 30, den: 1 };
    pub const FPS_2997: Fps = Fps {
        num: 30000,
        den: 1001,
    };
    pub const FPS_60: Fps = Fps { num: 60, den: 1 };

    /// The frame rates the project settings offer.
    pub const PRESETS: [Fps; 5] = [
        Self::FPS_24,
        Self::FPS_25,
        Self::FPS_30,
        Self::FPS_2997,
        Self::FPS_60,
    ];

    /// False for a rate of zero or one above 1000.
    pub fn is_valid(self) -> bool {
        self.num > 0 && self.den > 0 && u64::from(self.num) <= 1000 * u64::from(self.den)
    }

    pub fn as_f64(self) -> f64 {
        if self.den == 0 {
            0.0
        } else {
            f64::from(self.num) / f64::from(self.den)
        }
    }

    /// The whole number of frames that make a second in timecode (30 for 29.97).
    pub fn nominal(self) -> u64 {
        (self.as_f64().round() as u64).max(1)
    }

    /// Frames in `seconds`, rounded to the nearest frame.
    pub fn frames_in(self, seconds: f64) -> u64 {
        if !seconds.is_finite() || seconds <= 0.0 {
            0
        } else {
            (seconds * self.as_f64()).round() as u64
        }
    }

    /// `HH:MM:SS:FF` (non-drop-frame) of a frame number.
    pub fn timecode(self, frame: u64) -> String {
        let nominal = self.nominal();
        let ff = frame % nominal;
        let total_seconds = frame / nominal;
        format!(
            "{:02}:{:02}:{:02}:{:02}",
            total_seconds / 3600,
            total_seconds / 60 % 60,
            total_seconds % 60,
            ff
        )
    }
}

impl Default for Fps {
    fn default() -> Self {
        Self::FPS_24
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timecode_counts_frames_seconds_minutes_hours() {
        let f = Fps::FPS_24;
        assert_eq!(f.timecode(0), "00:00:00:00");
        assert_eq!(f.timecode(23), "00:00:00:23");
        assert_eq!(f.timecode(24), "00:00:01:00");
        assert_eq!(f.timecode(24 * 60 + 5), "00:01:00:05");
        assert_eq!(f.timecode(24 * 3600), "01:00:00:00");
    }

    #[test]
    fn ntsc_uses_a_nominal_rate_of_30() {
        assert_eq!(Fps::FPS_2997.nominal(), 30);
        assert_eq!(Fps::FPS_2997.timecode(30), "00:00:01:00");
    }

    #[test]
    fn validity_and_frames_in() {
        assert!(Fps::FPS_60.is_valid());
        assert!(!Fps { num: 0, den: 1 }.is_valid());
        assert!(!Fps { num: 1, den: 0 }.is_valid());
        assert!(!Fps { num: 5000, den: 1 }.is_valid());
        assert_eq!(Fps::FPS_25.frames_in(2.0), 50);
        assert_eq!(Fps::FPS_25.frames_in(f64::NAN), 0);
        assert_eq!(Fps { num: 1, den: 0 }.nominal(), 1);
    }
}
