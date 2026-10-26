//! Help > Check for Updates: asks GitHub for the latest release and installs it the way this copy
//! was installed.
//!
//! - Windows (the release zip, with FFmpeg and its licence beside the program): the new zip is unpacked
//!   over the folder. A running program cannot be overwritten but can be renamed, so each file in
//!   use is renamed to `*.old` first; the next start deletes those.
//! - macOS (`Picasso Suite.app`): the new app replaces the old one, which is kept as
//!   `Picasso Suite.app.old` until the next start.
//! - Flatpak: the new bundle is installed by the host's `flatpak`, through `flatpak-spawn`.
//! - Debian/Ubuntu and Fedora/openSUSE (`/usr/bin`): the new package is installed with `pkexec`,
//!   which asks for the password.
//! - Anything else (a build from source): the release page is opened instead.
//!
//! The network work goes through the system's `curl`, so the program carries no HTTP or TLS code
//! of its own. Packages are only downloaded from this repository's release files.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use egui::Context;

/// The GitHub repository the releases come from.
const REPO: &str = "fabox-os/Picasso-Suite";
/// The release page, opened when this copy cannot update itself.
pub(crate) const RELEASES_PAGE: &str = "https://github.com/fabox-os/Picasso-Suite/releases/latest";
/// Release files are only downloaded from here.
const DOWNLOAD_PREFIX: &str = "https://github.com/fabox-os/Picasso-Suite/releases/download/";
/// The longest release notes shown in the window.
const MAX_NOTES: usize = 6000;

/// How this copy of the program was installed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Install {
    /// The Windows release zip, unpacked in this folder.
    WindowsZip(PathBuf),
    /// The macOS app at this path.
    MacApp(PathBuf),
    Flatpak,
    Deb,
    Rpm,
    /// Built from source or copied by hand: nothing to update in place.
    Unmanaged,
}

impl Install {
    /// How this copy was installed, from where the program runs.
    pub(crate) fn detect() -> Self {
        if std::env::var_os("FLATPAK_ID").is_some() || Path::new("/.flatpak-info").exists() {
            return Self::Flatpak;
        }
        let Ok(exe) = std::env::current_exe() else {
            return Self::Unmanaged;
        };
        if cfg!(windows) {
            // The release zip carries FFmpeg and its licence beside the program.
            return match exe.parent() {
                Some(dir) if has_ffmpeg_beside(dir) => Self::WindowsZip(dir.into()),
                _ => Self::Unmanaged,
            };
        }
        if cfg!(target_os = "macos") {
            return exe
                .ancestors()
                .find(|p| p.extension().is_some_and(|e| e == "app"))
                .map_or(Self::Unmanaged, |app| Self::MacApp(app.into()));
        }
        if exe.starts_with("/usr") {
            if Path::new("/usr/bin/dpkg").exists() {
                return Self::Deb;
            }
            if Path::new("/usr/bin/rpm").exists() {
                return Self::Rpm;
            }
        }
        Self::Unmanaged
    }

    /// Whether the new version can be installed from inside the program.
    pub(crate) fn can_update(&self) -> bool {
        *self != Self::Unmanaged
    }

    /// Whether a release file is this kind of package.
    fn wants(&self, file: &str) -> bool {
        match self {
            Self::WindowsZip(_) => file.ends_with("-windows.zip"),
            Self::MacApp(_) => {
                file.ends_with("-macos-universal.zip")
                    || (cfg!(target_arch = "aarch64") && file.ends_with("-macos-arm64.zip"))
            }
            Self::Flatpak => file.ends_with(".flatpak"),
            Self::Deb => file.ends_with("amd64.deb"),
            Self::Rpm => file.ends_with("x86_64.rpm"),
            Self::Unmanaged => false,
        }
    }
}

/// Whether `dir` is an unpacked release zip: those carry FFmpeg's licence beside the program. A
/// build from source has FFmpeg's DLLs copied beside it too (see `apps/studio/build.rs`), but not
/// the licence, so it is never overwritten by an update.
fn has_ffmpeg_beside(dir: &Path) -> bool {
    dir.join("FFMPEG-LICENSE.txt").is_file()
}

/// A release on GitHub.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Release {
    pub tag: String,
    pub notes: String,
    /// The package for this kind of install: its file name and download address.
    pub package: Option<(String, String)>,
}

/// This build's version: the release tag it was built from when the build says so
/// (`PICASSO_RELEASE_TAG`), else the version in `Cargo.toml`.
pub(crate) fn current_version() -> String {
    option_env!("PICASSO_RELEASE_TAG")
        .map(|t| t.trim_start_matches('v').to_string())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}

/// `v0.2.0-hotfix` as its numbers and the words after them: `([0, 2, 0], "hotfix")`.
fn parse_version(tag: &str) -> ([u64; 3], String) {
    let tag = tag.trim().trim_start_matches(['v', 'V']);
    let (numbers, suffix) = tag.split_once('-').unwrap_or((tag, ""));
    let mut out = [0u64; 3];
    for (slot, part) in out.iter_mut().zip(numbers.split('.')) {
        *slot = part.trim().parse().unwrap_or(0);
    }
    (out, suffix.to_ascii_lowercase())
}

/// Whether release `latest` comes after version `current`. A hotfix of a version (`0.2.0-hotfix`)
/// comes after the version itself (`0.2.0`), as this project tags them.
pub(crate) fn is_newer(latest: &str, current: &str) -> bool {
    let (a, a_suffix) = parse_version(latest);
    let (b, b_suffix) = parse_version(current);
    if a != b {
        return a > b;
    }
    match (a_suffix.is_empty(), b_suffix.is_empty()) {
        (false, true) => true,
        (false, false) => a_suffix > b_suffix,
        _ => false,
    }
}

/// Reads GitHub's answer about the latest release; the package is the one for `install`.
fn parse_release(json: &str, install: &Install) -> Result<Release, String> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("GitHub's answer could not be read: {e}"))?;
    let tag = v["tag_name"]
        .as_str()
        .ok_or("GitHub did not name a release")?
        .to_string();
    let mut notes = v["body"].as_str().unwrap_or("").replace("\r\n", "\n");
    if notes.len() > MAX_NOTES {
        let mut cut = MAX_NOTES;
        while !notes.is_char_boundary(cut) {
            cut -= 1;
        }
        notes.truncate(cut);
        notes.push_str("\n...");
    }
    let package = v["assets"].as_array().and_then(|assets| {
        assets.iter().find_map(|a| {
            let name = a["name"].as_str()?;
            let url = a["browser_download_url"].as_str()?;
            (install.wants(name) && url.starts_with(DOWNLOAD_PREFIX))
                .then(|| (name.to_string(), url.to_string()))
        })
    });
    Ok(Release {
        tag,
        notes,
        package,
    })
}

/// A program run with no console window popping up on Windows.
fn quiet(program: impl AsRef<std::ffi::OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Runs `cmd` and says what went wrong in words when it fails.
fn run(mut cmd: Command, what: &str) -> Result<Vec<u8>, String> {
    let out = cmd
        .output()
        .map_err(|e| format!("{what}: could not start it ({e})"))?;
    if out.status.success() {
        return Ok(out.stdout);
    }
    let err = String::from_utf8_lossy(&out.stderr);
    let err = err.trim();
    Err(if err.is_empty() {
        format!("{what} failed ({})", out.status)
    } else {
        format!("{what} failed: {err}")
    })
}

/// Asks GitHub for the latest release.
fn fetch_latest(install: &Install) -> Result<Release, String> {
    let mut cmd = quiet("curl");
    cmd.args(["-fsSL", "--max-time", "20"])
        .args(["-H", "Accept: application/vnd.github+json"])
        .args(["-H", "User-Agent: picasso-suite-updater"])
        .arg(format!("https://api.github.com/repos/{REPO}/releases/latest"));
    let body = run(cmd, "Asking GitHub for the latest release")?;
    parse_release(&String::from_utf8_lossy(&body), install)
}

/// Downloads `url` (a file of this repository's releases) to `dest`.
fn download(url: &str, dest: &Path) -> Result<(), String> {
    if !url.starts_with(DOWNLOAD_PREFIX) {
        return Err("the package is not on this project's release page".into());
    }
    let mut cmd = quiet("curl");
    cmd.args(["-fsSL", "--max-time", "900", "-o"])
        .arg(dest)
        .arg(url);
    run(cmd, "Downloading the new version").map(|_| ())
}

/// An empty folder for this update's files.
fn work_dir(tag: &str) -> Result<PathBuf, String> {
    let safe: String = tag
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .collect();
    let dir = std::env::temp_dir().join(format!("picasso-suite-update-{safe}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
}

/// The folder (at most two levels down) that holds `file`.
fn folder_with(root: &Path, file: &str) -> Option<PathBuf> {
    if root.join(file).exists() {
        return Some(root.into());
    }
    let entries = std::fs::read_dir(root).ok()?;
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .find_map(|p| folder_with(&p, file))
}

/// What happened, for the window.
pub(crate) struct Installed {
    pub message: String,
    /// Whether starting the program again shows the new version.
    pub restart: bool,
}

/// Downloads `release`'s package and installs it over this copy.
fn download_and_install(
    install: &Install,
    release: &Release,
    step: &dyn Fn(&str),
) -> Result<Installed, String> {
    let (file, url) = release
        .package
        .clone()
        .ok_or("this release has no package for this kind of install")?;
    let work = work_dir(&release.tag)?;
    // Flatpak and the Linux packages are installed by programs outside this one, which must be
    // able to read the file: inside a Flatpak the cache folder is shared with the host.
    let package = match install {
        Install::Flatpak => {
            let cache = std::env::var_os("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            let _ = std::fs::create_dir_all(&cache);
            cache.join(&file)
        }
        _ => work.join(&file),
    };
    step(&format!("Downloading {file}..."));
    download(&url, &package)?;
    step("Installing...");
    let done = match install {
        Install::WindowsZip(dir) => install_windows(&package, &work, dir)?,
        Install::MacApp(app) => install_mac(&package, &work, app)?,
        Install::Flatpak => {
            let mut cmd = quiet("flatpak-spawn");
            cmd.args(["--host", "flatpak", "install", "--noninteractive", "-y", "--reinstall"])
                .arg(&package);
            run(cmd, "Installing the Flatpak").map_err(|e| {
                format!(
                    "{e}\n\nInstall it from a terminal instead:\nflatpak install --reinstall {}",
                    package.display()
                )
            })?;
            Installed {
                message: format!("Picasso Suite {} is installed. Close Picasso Suite and open it again.", release.tag),
                restart: false,
            }
        }
        Install::Deb | Install::Rpm => install_linux_package(install, &package, &release.tag)?,
        Install::Unmanaged => return Err("this copy cannot update itself".into()),
    };
    if !matches!(install, Install::Flatpak | Install::Deb | Install::Rpm) {
        let _ = std::fs::remove_dir_all(&work);
    }
    Ok(done)
}

/// Unpacks the Windows zip and puts its files in `dir`, renaming the files in use first.
fn install_windows(zip: &Path, work: &Path, dir: &Path) -> Result<Installed, String> {
    let unpacked = work.join("unpacked");
    std::fs::create_dir_all(&unpacked).map_err(|e| e.to_string())?;
    // Windows 10 and later carry bsdtar, which reads zip files. It is named by its full path: a
    // GNU tar (from Git, say) earlier on the PATH cannot.
    let tar = std::env::var_os("SystemRoot")
        .map(|root| PathBuf::from(root).join("System32").join("tar.exe"))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("tar"));
    let mut cmd = quiet(tar);
    cmd.arg("-xf").arg(zip).arg("-C").arg(&unpacked);
    run(cmd, "Unpacking the new version")?;
    let source = folder_with(&unpacked, "picasso-suite.exe")
        .ok_or("the new version's zip has no picasso-suite.exe")?;
    let entries = std::fs::read_dir(&source).map_err(|e| e.to_string())?;
    // What was done, so a failure half way can put the old version back: (new file, the old
    // file renamed out of its way, if there was one).
    let mut done: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();
    let undo = |done: &[(PathBuf, Option<PathBuf>)]| {
        for (to, old) in done.iter().rev() {
            let _ = std::fs::remove_file(to);
            if let Some(old) = old {
                let _ = std::fs::rename(old, to);
            }
        }
    };
    for entry in entries.flatten() {
        let from = entry.path();
        if !from.is_file() {
            continue;
        }
        let to = dir.join(entry.file_name());
        let mut old = None;
        if to.exists() {
            let mut name = to.as_os_str().to_owned();
            name.push(".old");
            let renamed = PathBuf::from(name);
            let _ = std::fs::remove_file(&renamed);
            if let Err(e) = std::fs::rename(&to, &renamed) {
                undo(&done);
                return Err(format!(
                    "{} cannot be replaced ({e}). If Picasso Suite is in Program Files, move its \
                     folder somewhere you can write to, or update it by hand.",
                    to.display()
                ));
            }
            old = Some(renamed);
        }
        let copied = std::fs::copy(&from, &to);
        done.push((to.clone(), old));
        if let Err(e) = copied {
            undo(&done);
            return Err(format!(
                "{}: {e}. The version you had is kept.",
                to.display()
            ));
        }
    }
    Ok(Installed {
        message: "The new version is installed. Restart Picasso Suite to use it.".into(),
        restart: true,
    })
}

/// Unpacks the macOS zip and puts the new app where `app` is.
fn install_mac(zip: &Path, work: &Path, app: &Path) -> Result<Installed, String> {
    let unpacked = work.join("unpacked");
    let mut cmd = quiet("ditto");
    cmd.args(["-x", "-k"]).arg(zip).arg(&unpacked);
    run(cmd, "Unpacking the new version")?;
    let new_app = std::fs::read_dir(&unpacked)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .ok_or("the new version's zip has no app")?;
    let old = old_app_path(app);
    let _ = std::fs::remove_dir_all(&old);
    std::fs::rename(app, &old).map_err(|e| format!("{} cannot be replaced: {e}", app.display()))?;
    let mut cmd = quiet("ditto");
    cmd.arg(&new_app).arg(app);
    if let Err(e) = run(cmd, "Copying the new app") {
        // Put the old app back so the program still starts.
        let _ = std::fs::remove_dir_all(app);
        let _ = std::fs::rename(&old, app);
        return Err(e);
    }
    Ok(Installed {
        message: "The new version is installed. Restart Picasso Suite to use it.".into(),
        restart: true,
    })
}

fn old_app_path(app: &Path) -> PathBuf {
    let mut old = app.as_os_str().to_owned();
    old.push(".old");
    PathBuf::from(old)
}

/// Installs a `.deb` or `.rpm` with the system's package manager, asking for the password.
fn install_linux_package(install: &Install, package: &Path, tag: &str) -> Result<Installed, String> {
    let manual = match install {
        Install::Deb => format!("sudo apt install {}", package.display()),
        _ => format!("sudo dnf install {}", package.display()),
    };
    let mut cmd = quiet("pkexec");
    match install {
        Install::Deb => {
            cmd.args(["apt-get", "install", "-y"]).arg(package);
        }
        _ if Path::new("/usr/bin/dnf").exists() => {
            cmd.args(["dnf", "install", "-y"]).arg(package);
        }
        _ => {
            cmd.args(["zypper", "--non-interactive", "install"]).arg(package);
        }
    }
    run(cmd, "Installing the package")
        .map_err(|e| format!("{e}\n\nInstall it from a terminal instead:\n{manual}"))?;
    Ok(Installed {
        message: format!("Picasso Suite {tag} is installed. Restart Picasso Suite to use it."),
        restart: true,
    })
}

/// Deletes what the last update left behind: `*.old` files beside the program on Windows, the old
/// app on macOS. Run at start.
pub(crate) fn clean_up() {
    match Install::detect() {
        Install::WindowsZip(dir) => clean_up_dir(&dir),
        Install::MacApp(app) => {
            let _ = std::fs::remove_dir_all(old_app_path(&app));
        }
        _ => {}
    }
}

/// Deletes the `*.old` files an update left in `dir`.
fn clean_up_dir(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().ends_with(".old") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

/// Opens `url` in the web browser.
pub(crate) fn open_in_browser(url: &str) {
    let mut cmd = if cfg!(windows) {
        let mut c = quiet("rundll32");
        c.arg("url.dll,FileProtocolHandler");
        c
    } else if cfg!(target_os = "macos") {
        quiet("open")
    } else {
        quiet("xdg-open")
    };
    let _ = cmd.arg(url).spawn();
}

/// Starts this program again (the new version, once installed).
fn relaunch(install: &Install) {
    match install {
        Install::MacApp(app) => {
            let _ = quiet("open").arg("-n").arg(app).spawn();
        }
        _ => {
            if let Ok(exe) = std::env::current_exe() {
                let _ = quiet(exe).spawn();
            }
        }
    }
}

/// Where the check or the update is.
#[derive(Clone, Debug, Default)]
enum Phase {
    #[default]
    Idle,
    Checking,
    UpToDate(String),
    Available(Release),
    Working(String),
    Done {
        message: String,
        restart: bool,
    },
    Failed(String),
}

/// The Check for Updates window and the work behind it.
#[derive(Default)]
pub(crate) struct Updater {
    pub open: bool,
    phase: Arc<Mutex<Phase>>,
    install: Option<Install>,
}

impl Updater {
    fn phase(&self) -> Phase {
        self.phase.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn install(&mut self) -> Install {
        self.install.get_or_insert_with(Install::detect).clone()
    }

    /// Opens the window and asks GitHub for the latest release.
    pub(crate) fn check(&mut self, ctx: &Context) {
        self.open = true;
        if matches!(self.phase(), Phase::Checking | Phase::Working(_)) {
            return;
        }
        let install = self.install();
        let shared = self.phase.clone();
        let ctx = ctx.clone();
        *shared.lock().unwrap_or_else(|e| e.into_inner()) = Phase::Checking;
        std::thread::spawn(move || {
            let current = current_version();
            let phase = match fetch_latest(&install) {
                Ok(r) if is_newer(&r.tag, &current) => Phase::Available(r),
                Ok(r) => Phase::UpToDate(r.tag),
                Err(e) => Phase::Failed(e),
            };
            *shared.lock().unwrap_or_else(|e| e.into_inner()) = phase;
            ctx.request_repaint();
        });
    }

    fn update(&mut self, ctx: &Context, release: Release) {
        let install = self.install();
        let shared = self.phase.clone();
        let ctx = ctx.clone();
        *shared.lock().unwrap_or_else(|e| e.into_inner()) = Phase::Working("Starting...".into());
        std::thread::spawn(move || {
            let step = |s: &str| {
                *shared.lock().unwrap_or_else(|e| e.into_inner()) = Phase::Working(s.into());
                ctx.request_repaint();
            };
            let phase = match download_and_install(&install, &release, &step) {
                Ok(done) => Phase::Done {
                    message: done.message,
                    restart: done.restart,
                },
                Err(e) => Phase::Failed(e),
            };
            *shared.lock().unwrap_or_else(|e| e.into_inner()) = phase;
            ctx.request_repaint();
        });
    }

    /// Draws the window while it is open.
    pub(crate) fn window(&mut self, ctx: &Context) {
        if !self.open {
            return;
        }
        let phase = self.phase();
        let install = self.install();
        let current = current_version();
        let mut open = true;
        let mut wanted: Option<Release> = None;
        let mut check_again = false;
        let mut restart = false;
        egui::Window::new("Check for Updates")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.set_max_width(460.0);
                match &phase {
                    Phase::Idle | Phase::Checking => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Checking GitHub for a newer version...");
                        });
                    }
                    Phase::UpToDate(latest) => {
                        ui.label(format!("Picasso Suite {current} is up to date."));
                        ui.weak(format!("The latest release is {latest}."));
                    }
                    Phase::Available(release) => {
                        ui.heading(format!("Picasso Suite {} is available", release.tag));
                        ui.label(format!("You have {current}."));
                        if !release.notes.trim().is_empty() {
                            ui.add_space(4.0);
                            egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                                ui.label(&release.notes);
                            });
                        }
                        ui.add_space(6.0);
                        if !install.can_update() {
                            ui.label(
                                "This copy was not installed from a release package, so it \
                                 cannot update itself. Download the new version from the \
                                 release page.",
                            );
                        } else if release.package.is_none() {
                            ui.label("This release has no package for this kind of install.");
                        }
                        ui.horizontal(|ui| {
                            let can = install.can_update() && release.package.is_some();
                            if can && ui.button("Update now").clicked() {
                                wanted = Some(release.clone());
                            }
                            if ui.button("Release page").clicked() {
                                open_in_browser(RELEASES_PAGE);
                            }
                        });
                    }
                    Phase::Working(step) => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(step);
                        });
                    }
                    Phase::Done { message, restart: r } => {
                        ui.label(message);
                        if *r {
                            ui.weak("Save your project first: restarting closes it.");
                            if ui.button("Restart now").clicked() {
                                restart = true;
                            }
                        }
                    }
                    Phase::Failed(e) => {
                        ui.label(egui::RichText::new("The update did not work.").strong());
                        ui.label(e);
                        ui.horizontal(|ui| {
                            if ui.button("Try again").clicked() {
                                check_again = true;
                            }
                            if ui.button("Release page").clicked() {
                                open_in_browser(RELEASES_PAGE);
                            }
                        });
                    }
                }
            });
        if let Some(release) = wanted {
            self.update(ctx, release);
        }
        if check_again {
            *self.phase.lock().unwrap_or_else(|e| e.into_inner()) = Phase::Idle;
            self.check(ctx);
        }
        if restart {
            relaunch(&install);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        // Closing the window while it works lets the work finish in the background.
        self.open = open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hotfix_comes_after_its_version() {
        assert!(is_newer("v0.2.0-hotfix", "0.2.0"));
        assert!(is_newer("v0.3.0", "0.2.0-hotfix"));
        assert!(is_newer("v0.2.1", "0.2.0"));
        assert!(is_newer("v1.0.0", "0.9.9"));
        assert!(!is_newer("v0.2.0", "0.2.0"));
        assert!(!is_newer("v0.2.0-hotfix", "0.2.0-hotfix"));
        assert!(!is_newer("v0.2.0", "0.2.0-hotfix"));
        assert!(!is_newer("v0.1.0-hotfix", "0.2.0"));
        assert!(is_newer("v0.2.0-hotfix2", "0.2.0-hotfix"));
    }

    fn answer() -> String {
        let asset = |name: &str| {
            format!(
                r#"{{"name":"{name}","browser_download_url":"{DOWNLOAD_PREFIX}v0.2.0-hotfix/{name}"}}"#
            )
        };
        format!(
            r#"{{"tag_name":"v0.2.0-hotfix","body":"New:\r\n- Preferences","assets":[{},{},{},{},{}]}}"#,
            asset("picasso-suite-0.2.0-hotfix-amd64.deb"),
            asset("picasso-suite-0.2.0-hotfix-macos-universal.zip"),
            asset("picasso-suite-0.2.0-hotfix-windows.zip"),
            asset("picasso-suite-0.2.0-hotfix-x86_64.rpm"),
            asset("picasso-suite-0.2.0-hotfix.flatpak"),
        )
    }

    #[test]
    fn each_install_gets_its_own_package() {
        let pick = |install: Install| {
            parse_release(&answer(), &install)
                .ok()
                .and_then(|r| r.package)
                .map(|(name, _)| name)
        };
        assert_eq!(
            pick(Install::WindowsZip(PathBuf::from("x"))).as_deref(),
            Some("picasso-suite-0.2.0-hotfix-windows.zip")
        );
        assert_eq!(
            pick(Install::MacApp(PathBuf::from("x"))).as_deref(),
            Some("picasso-suite-0.2.0-hotfix-macos-universal.zip")
        );
        assert_eq!(pick(Install::Flatpak).as_deref(), Some("picasso-suite-0.2.0-hotfix.flatpak"));
        assert_eq!(pick(Install::Deb).as_deref(), Some("picasso-suite-0.2.0-hotfix-amd64.deb"));
        assert_eq!(pick(Install::Rpm).as_deref(), Some("picasso-suite-0.2.0-hotfix-x86_64.rpm"));
        assert_eq!(pick(Install::Unmanaged), None);
    }

    #[test]
    fn the_answer_gives_the_tag_and_notes() {
        let r = parse_release(&answer(), &Install::Unmanaged);
        assert_eq!(r.as_ref().map(|r| r.tag.as_str()), Ok("v0.2.0-hotfix"));
        assert_eq!(r.map(|r| r.notes), Ok("New:\n- Preferences".to_string()));
    }

    #[test]
    fn files_from_elsewhere_are_not_downloaded() {
        let json = r#"{"tag_name":"v9.0.0","assets":[{"name":"picasso-suite-9.0.0-windows.zip","browser_download_url":"https://example.com/evil.zip"}]}"#;
        let r = parse_release(json, &Install::WindowsZip(PathBuf::from("x")));
        assert_eq!(r.map(|r| r.package), Ok(None));
        assert!(download("https://example.com/evil.zip", Path::new("x")).is_err());
    }

    #[test]
    fn a_broken_answer_is_an_error() {
        assert!(parse_release("not json", &Install::Unmanaged).is_err());
        assert!(parse_release("{}", &Install::Unmanaged).is_err());
    }

    /// Asks the real GitHub (run with `--ignored`; needs the network and `curl`).
    #[test]
    #[ignore]
    fn the_latest_release_can_be_read() {
        let r = fetch_latest(&Install::WindowsZip(PathBuf::from("x")));
        let r = r.map_err(|e| e.to_string());
        assert!(r.as_ref().is_ok_and(|r| r.tag.starts_with('v')), "{r:?}");
        assert!(r.is_ok_and(|r| r.package.is_some()));
    }
}

#[cfg(all(test, windows))]
mod windows_install_tests {
    use super::*;

    /// Downloads the real latest Windows zip and installs it over a fake older copy in a temporary
    /// folder (run with `--ignored`; needs the network).
    #[test]
    #[ignore]
    fn the_windows_zip_installs_over_an_older_copy() {
        let dir = std::env::temp_dir().join("picasso-update-install-test");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(std::fs::create_dir_all(&dir).is_ok());
        for (name, text) in [("picasso-suite.exe", "old"), ("ffmpeg.exe", "old"), ("README.md", "old")] {
            assert!(std::fs::write(dir.join(name), text).is_ok());
        }
        let install = Install::WindowsZip(dir.clone());
        let release = fetch_latest(&install);
        assert!(release.is_ok(), "{release:?}");
        let Ok(release) = release else { return };
        let done = download_and_install(&install, &release, &|_| {});
        assert!(done.as_ref().is_ok_and(|d| d.restart), "{:?}", done.err());
        let size = |name: &str| std::fs::metadata(dir.join(name)).map(|m| m.len()).unwrap_or(0);
        assert!(size("picasso-suite.exe") > 1_000_000);
        assert!(size("ffmpeg.exe") > 1_000_000);
        assert_eq!(std::fs::read_to_string(dir.join("picasso-suite.exe.old")).ok().as_deref(), Some("old"));
        clean_up_dir(&dir);
        assert!(!dir.join("picasso-suite.exe.old").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
