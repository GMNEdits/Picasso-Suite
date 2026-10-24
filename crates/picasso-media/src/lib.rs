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

pub use decode::VideoDecoder;
pub use frame::RgbaImage;
pub use probe::probe;

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error(
        "{0} was not found. Install FFmpeg (Linux: `sudo apt install ffmpeg` or your distro's package; \
         Windows: `winget install ffmpeg`) or set PICASSO_FFMPEG / PICASSO_FFPROBE"
    )]
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
