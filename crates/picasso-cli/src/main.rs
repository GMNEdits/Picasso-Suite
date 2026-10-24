//! `picasso` — headless Picasso Suite tools: probe media, build projects,
//! import DaVinci Resolve `.drp` projects and render timelines.

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use picasso_core::{Project, ProjectSettings, Rational};
use picasso_media::encoders::{self, Quality};
use picasso_render::{ExportProgress, ExportSettings};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

#[derive(Parser)]
#[command(name = "picasso", version, about = "Picasso Suite command-line tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show what Picasso Suite reads from a media file.
    Probe { file: PathBuf },
    /// List export presets usable with the installed FFmpeg.
    Presets,
    /// Create a project by appending media files to the timeline.
    New {
        output: PathBuf,
        #[arg(required = true)]
        media: Vec<PathBuf>,
        /// Frame rate (e.g. 25, 23.976, 30000/1001). Defaults to the first video's rate.
        #[arg(long)]
        fps: Option<String>,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
    },
    /// Convert a DaVinci Resolve .drp project to a Picasso Suite project.
    ImportDrp {
        drp: PathBuf,
        output: PathBuf,
        /// Extra folders to search for media that moved (searched recursively).
        #[arg(long = "search")]
        search: Vec<PathBuf>,
        /// Path prefix rewrite, e.g. --map "D:\Footage=/mnt/footage". Repeatable.
        #[arg(long = "map")]
        maps: Vec<String>,
    },
    /// Export a project's timeline to OpenTimelineIO.
    Otio { project: PathBuf, output: PathBuf },
    /// Render a project (.pcsp or .drp) to a video file.
    Render {
        project: PathBuf,
        output: PathBuf,
        /// Preset id, see `picasso presets`.
        #[arg(long, default_value = "h264")]
        preset: String,
        #[arg(long, default_value = "high", value_parser = ["best", "high", "medium", "small"])]
        quality: String,
    },
}

fn load_any(path: &Path) -> Result<Project> {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("drp")) {
        let r = picasso_interop::drp::import(path, &picasso_interop::relink::Relinker::for_project_file(path))?;
        eprintln!("{}", r.summary());
        Ok(r.project)
    } else {
        Ok(picasso_core::io::load(path)?)
    }
}

fn main() -> Result<()> {
    env_logger_init();
    let cli = Cli::parse();
    match cli.command {
        Command::Probe { file } => {
            let item = picasso_media::probe(&file, Rational::new(24, 1))?;
            println!("{}", serde_json::to_string_pretty(&item)?);
        }
        Command::Presets => {
            let enc = encoders::available_encoders()?;
            for p in encoders::available_presets(&enc) {
                println!("{:<20} {:<36} .{}", p.id, p.label, p.extensions.join(" ."));
            }
        }
        Command::New { output, media, fps, width, height } => {
            let mut items = Vec::new();
            for m in &media {
                items.push(picasso_media::probe(m, Rational::new(24, 1)).with_context(|| format!("probing {}", m.display()))?);
            }
            let first_video = items.iter().find_map(|i| i.info.video.clone().filter(|v| !v.is_still));
            let mut settings = ProjectSettings::default();
            if let Some(v) = &first_video {
                settings.width = v.width & !1;
                settings.height = v.height & !1;
                settings.fps = v.fps;
            }
            if let Some(f) = fps {
                settings.fps = Rational::parse(&f).context("invalid --fps")?;
            }
            settings.width = width.unwrap_or(settings.width);
            settings.height = height.unwrap_or(settings.height);
            let name = output.file_stem().map_or("Untitled".into(), |s| s.to_string_lossy().into_owned());
            let mut project = Project::new(&name, settings);
            for item in items {
                let id = item.id;
                project.media.push(item);
                project.append_media(id).map_err(|e| anyhow::anyhow!("{e}"))?;
            }
            picasso_core::io::save(&project, &output)?;
            println!("wrote {} ({} frames)", output.display(), project.timeline.duration());
        }
        Command::ImportDrp { drp, output, search, maps } => {
            let mut relinker = picasso_interop::relink::Relinker::for_project_file(&drp);
            relinker.roots.extend(search);
            for m in maps {
                let (from, to) = m.split_once('=').context("--map expects FROM=TO")?;
                relinker.mappings.push(picasso_interop::relink::PathMapping { from: from.into(), to: to.into() });
            }
            let r = picasso_interop::drp::import(&drp, &relinker)?;
            println!("{}", r.summary());
            for o in &r.offline {
                println!("  offline: {o}");
            }
            picasso_core::io::save(&r.project, &output)?;
            println!("wrote {}", output.display());
        }
        Command::Otio { project, output } => {
            let p = load_any(&project)?;
            picasso_interop::otio::write(&p, &output)?;
            println!("wrote {}", output.display());
        }
        Command::Render { project, output, preset, quality } => {
            let p = load_any(&project)?;
            let Some(preset) = encoders::preset(&preset) else { bail!("unknown preset {preset}; see `picasso presets`") };
            let quality = match quality.as_str() {
                "best" => Quality::Best,
                "medium" => Quality::Medium,
                "small" => Quality::Small,
                _ => Quality::High,
            };
            let missing = picasso_render::export::missing_media(&p);
            for m in &missing {
                eprintln!("warning: media offline, renders black/silent: {}", m.path.display());
            }
            let settings = ExportSettings { preset, quality, output: output.clone(), range: None };
            let progress = std::sync::Arc::new(ExportProgress::default());
            let reporter = {
                let progress = progress.clone();
                std::thread::spawn(move || {
                    while !progress.cancel.load(Ordering::Relaxed) {
                        eprint!("\r{:5.1}%", progress.fraction() * 100.0);
                        std::thread::sleep(std::time::Duration::from_millis(250));
                    }
                })
            };
            let t0 = std::time::Instant::now();
            let result = picasso_render::export(&p, &settings, &progress);
            progress.cancel.store(true, Ordering::Relaxed);
            let _ = reporter.join();
            result?;
            eprintln!("\rrendered {} in {:.1}s", output.display(), t0.elapsed().as_secs_f64());
        }
    }
    Ok(())
}

fn env_logger_init() {
    if std::env::var_os("RUST_LOG").is_some() {
        eprintln!("(logging via RUST_LOG is available in the GUI build)");
    }
}
