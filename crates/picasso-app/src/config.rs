//! The per-user configuration folder, shared by the app's JSON settings files.
//!
//! `%APPDATA%\PicassoSuite` on Windows, `~/Library/Application Support/PicassoSuite`
//! on macOS, and `$XDG_CONFIG_HOME/PicassoSuite` (or `~/.config/PicassoSuite`)
//! everywhere else.

use std::path::PathBuf;

pub fn dir() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        PathBuf::from(std::env::var_os("APPDATA")?)
    } else if cfg!(target_os = "macos") {
        PathBuf::from(std::env::var_os("HOME")?).join("Library/Application Support")
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?
    };
    Some(base.join("PicassoSuite"))
}

/// A file inside the configuration folder, e.g. `keybinds.json`.
pub fn path(file: &str) -> Option<PathBuf> {
    Some(dir()?.join(file))
}
