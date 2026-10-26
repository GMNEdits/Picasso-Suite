//! Multicam clips: several cameras that filmed the same event, lined up so the editor can cut
//! between them while the timeline plays.
//!
//! A multicam clip is a list of angles. Each angle is a media item and an `offset`: the frame
//! of that media that shows the shared sync moment. Placing the first angle on the timeline
//! and switching angle at a frame (`SwitchAngle`) cuts the clip there and continues with the
//! same moment of another camera.

use serde::{Deserialize, Serialize};

use crate::MediaId;

/// The most angles one multicam clip has.
pub const MAX_ANGLES: usize = 16;
/// The most multicam clips a project keeps.
pub const MAX_MULTICAMS: usize = 200;
/// The largest offset, in frames, either way.
pub const MAX_ANGLE_OFFSET: i64 = 10_000_000;

/// One camera of a multicam clip.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Angle {
    pub media: MediaId,
    /// The frame of `media` that shows the sync moment (can be negative: the camera started
    /// rolling after it).
    #[serde(default)]
    pub offset: i64,
}

/// A multicam clip.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Multicam {
    pub id: u64,
    pub name: String,
    pub angles: Vec<Angle>,
}

impl Multicam {
    /// The angle that shows `media`.
    pub fn angle_of(&self, media: MediaId) -> Option<usize> {
        self.angles.iter().position(|a| a.media == media)
    }

    /// Cleans a name from a file or a dialog.
    pub fn clean_name(name: &str) -> String {
        let n: String = name
            .chars()
            .filter(|c| !c.is_control())
            .take(crate::project::MAX_BIN_NAME)
            .collect();
        if n.trim().is_empty() {
            "Multicam".into()
        } else {
            n.trim().to_string()
        }
    }
}
