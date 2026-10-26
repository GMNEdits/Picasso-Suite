//! Why a command or a project file was refused.

use thiserror::Error;

/// An edit that could not be made, or a project that is not valid. The project is unchanged.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum EditError {
    #[error("there is no such track")]
    NoSuchTrack,
    #[error("there is no such clip")]
    NoSuchClip,
    #[error("there is no such media")]
    NoSuchMedia,
    /// The track is locked.
    #[error("the track is locked")]
    Locked,
    /// The clip is locked (Clip > Lock Clip).
    #[error("the clip is locked")]
    ClipLocked,
    /// The clip would overlap another one on the track.
    #[error("the clip would overlap another clip")]
    Overlap,
    /// A video clip on an audio track or the other way round.
    #[error("the media does not fit this kind of track")]
    WrongTrackKind,
    /// The edit would start before the beginning of the timeline or past the end of the media.
    #[error("the edit is outside the media or the timeline")]
    OutOfRange,
    /// A clip still uses the media.
    #[error("a clip on the timeline still uses this media")]
    MediaInUse,
    /// A parameter that cannot be right (an empty clip, a split outside the clip, NaN).
    #[error("invalid: {0}")]
    Invalid(&'static str),
    /// The project file cannot be read.
    #[error("the project file is not valid: {0}")]
    BadFile(String),
}
