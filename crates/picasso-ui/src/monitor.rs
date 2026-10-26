//! What is heard, apart from the project: solo, the bus's level and mute, the volume and DIM.
//! These change only the sound played here, never the project or what an export renders.

use std::collections::BTreeSet;

use picasso_core::{Project, TrackId, MIN_GAIN_DB};

/// How much DIM lowers what is heard.
pub(crate) const DIM_DB: f32 = -20.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Monitor {
    /// The tracks soloed: while there are some, only they are heard.
    pub solo: BTreeSet<TrackId>,
    /// The level of Bus 1 (where every track goes), in dB.
    pub bus_gain_db: f32,
    pub bus_muted: bool,
    /// The volume of the speakers, 0 to 1.
    pub volume: f32,
    pub dim: bool,
}

impl Default for Monitor {
    fn default() -> Self {
        Self {
            solo: BTreeSet::new(),
            bus_gain_db: 0.0,
            bus_muted: false,
            volume: 1.0,
            dim: false,
        }
    }
}

impl Monitor {
    /// Whether it changes nothing, so the project can be played as it is.
    pub(crate) fn is_plain(&self) -> bool {
        *self == Self::default()
    }

    /// The level everything is changed by, in dB, or None for silence.
    fn master_db(&self) -> Option<f32> {
        if self.bus_muted || self.volume <= 0.001 {
            return None;
        }
        let volume = 20.0 * self.volume.clamp(0.001, 1.0).log10();
        Some(self.bus_gain_db + volume + if self.dim { DIM_DB } else { 0.0 })
    }

    /// The project as it is heard: tracks that are not soloed (while some are) muted, and every
    /// track's level changed by the bus, the volume and DIM.
    pub(crate) fn heard(&self, project: &Project) -> Project {
        let mut p = project.clone();
        let master = self.master_db();
        for t in &mut p.tracks {
            if !self.solo.is_empty() && !self.solo.contains(&t.id) {
                t.muted = true;
            }
            match master {
                Some(db) => t.gain_db = (t.gain_db + db).max(MIN_GAIN_DB),
                None => t.muted = true,
            }
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solo_mutes_the_others_and_the_bus_changes_every_track() {
        let p = Project::demo();
        let mut m = Monitor::default();
        assert!(m.is_plain());
        assert_eq!(m.heard(&p), p);
        m.solo.insert(p.tracks[0].id);
        m.bus_gain_db = -6.0;
        let heard = m.heard(&p);
        assert!(!heard.tracks[0].muted && heard.tracks[1].muted);
        assert_eq!(heard.tracks[0].gain_db, -6.0);
        m.dim = true;
        assert_eq!(m.heard(&p).tracks[0].gain_db, -26.0);
        m.bus_muted = true;
        assert!(m.heard(&p).tracks.iter().all(|t| t.muted));
        assert!(!p.tracks[0].muted, "the project itself is untouched");
    }
}
