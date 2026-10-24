//! Locating and launching the FFmpeg command-line tools.
//!
//! Lookup order: `PICASSO_FFMPEG` / `PICASSO_FFPROBE` environment variables,
//! a copy next to the running executable (for bundled releases), then `PATH`.

use crate::MediaError;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug)]
pub enum Tool {
    Ffmpeg,
    Ffprobe,
}

impl Tool {
    fn name(self) -> &'static str {
        match self {
            Tool::Ffmpeg => "ffmpeg",
            Tool::Ffprobe => "ffprobe",
        }
    }

    fn env_var(self) -> &'static str {
        match self {
            Tool::Ffmpeg => "PICASSO_FFMPEG",
            Tool::Ffprobe => "PICASSO_FFPROBE",
        }
    }
}

pub fn tool_path(tool: Tool) -> PathBuf {
    if let Some(p) = std::env::var_os(tool.env_var()) {
        return PathBuf::from(p);
    }
    let file = format!("{}{}", tool.name(), std::env::consts::EXE_SUFFIX);
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(PathBuf::from)) {
        let bundled = dir.join(&file);
        if bundled.is_file() {
            return bundled;
        }
    }
    PathBuf::from(file)
}

/// A `Command` for the tool with no console window and no stdin.
pub fn command(tool: Tool) -> Command {
    let mut cmd = Command::new(tool_path(tool));
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Returns the first line of `ffmpeg -version`, or an error explaining how to install it.
pub fn check_tools() -> Result<String, MediaError> {
    for tool in [Tool::Ffprobe, Tool::Ffmpeg] {
        let out = command(tool).arg("-version").output().map_err(|_| MediaError::ToolMissing(tool.name()))?;
        if !out.status.success() {
            return Err(MediaError::ToolMissing(tool.name()));
        }
        if let Tool::Ffmpeg = tool {
            let text = String::from_utf8_lossy(&out.stdout);
            return Ok(text.lines().next().unwrap_or_default().to_owned());
        }
    }
    unreachable!()
}
