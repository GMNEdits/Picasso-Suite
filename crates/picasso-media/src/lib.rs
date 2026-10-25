//! Media I/O for Picasso Suite, built on the FFmpeg command-line tools so any
//! format the installed FFmpeg supports can be imported and exported, on
//! Linux, Windows and macOS alike.

pub mod decode;
pub mod encoders;
pub mod frame;
pub mod overview;
pub mod probe;
pub mod sequence;
pub mod tools;

pub use decode::{VideoDecoder, hardware_decoding, set_hardware_decoding};
pub use frame::RgbaImage;
pub use probe::probe;

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("{}", tools::missing_help(.0))]
    ToolMissing(&'static str),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not read media: {0}")]
    Probe(String),
    #[error("no usable audio or video streams in {0}")]
    NoStreams(String),
    #[error("ffmpeg failed: {0}")]
    Ffmpeg(String),
}

pub type Result<T> = std::result::Result<T, MediaError>;
