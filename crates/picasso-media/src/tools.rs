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

/// What to tell the user when `missing` (`ffmpeg` or `ffprobe`) cannot be run: both programs are
/// needed, named as this operating system names them, and where to put them.
pub fn missing_help(missing: &str) -> String {
    let exe = std::env::consts::EXE_SUFFIX;
    let how = if cfg!(windows) {
        "The Windows download of Picasso Suite includes both. If you got Picasso Suite another way, \
         either run `winget install ffmpeg` in a terminal and restart Picasso Suite, or download a \
         Windows build of FFmpeg (a .zip whose bin folder holds ffmpeg.exe and ffprobe.exe) and copy \
         those two .exe files into the folder that holds picasso-suite.exe."
    } else if cfg!(target_os = "macos") {
        "Install FFmpeg with `brew install ffmpeg` (https://brew.sh), then restart Picasso Suite."
    } else {
        "Install FFmpeg from your distribution (Debian/Ubuntu: `sudo apt install ffmpeg`; Fedora: \
         `sudo dnf install ffmpeg` from RPM Fusion), then restart Picasso Suite. The Flatpak includes it."
    };
    format!(
        "FFmpeg is missing: {missing}{exe} could not be started. Picasso Suite needs two FFmpeg \
         programs, ffmpeg{exe} and ffprobe{exe}. {how} (Advanced: PICASSO_FFMPEG and \
         PICASSO_FFPROBE can point at them.)"
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_missing_message_names_both_programs() {
        let text = missing_help("ffprobe");
        let exe = std::env::consts::EXE_SUFFIX;
        assert!(text.contains(&format!("ffprobe{exe} could not be started")));
        assert!(text.contains(&format!("ffmpeg{exe} and ffprobe{exe}")));
    }
}
