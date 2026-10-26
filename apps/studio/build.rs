//! On Windows the FFmpeg DLLs must sit next to the program. Copy them there when `FFMPEG_DIR`
//! (the folder with `bin`, `include` and `lib`) is set.

use std::path::PathBuf;

fn main() {
    embed_icon();
    println!("cargo:rerun-if-env-changed=FFMPEG_DIR");
    let (Some(ffmpeg), Some(out)) = (std::env::var_os("FFMPEG_DIR"), std::env::var_os("OUT_DIR"))
    else {
        return;
    };
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // OUT_DIR is target/<profile>/build/<crate>-<hash>/out; the program is in target/<profile>.
    let out = PathBuf::from(out);
    let Some(profile_dir) = out.ancestors().nth(3) else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(PathBuf::from(ffmpeg).join("bin")) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "dll") {
            if let Some(name) = path.file_name() {
                let _ = std::fs::copy(&path, profile_dir.join(name));
            }
        }
    }
}

/// Puts `assets/icon.ico` into the Windows program; a failure only leaves the default icon.
#[cfg(windows)]
fn embed_icon() {
    println!("cargo:rerun-if-changed=../../assets/icon.ico");
    // The build script runs on the build machine; only a Windows program gets the icon.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/icon.ico");
    if let Err(e) = res.compile() {
        println!("cargo:warning=could not embed the icon: {e}");
    }
}

#[cfg(not(windows))]
fn embed_icon() {}
