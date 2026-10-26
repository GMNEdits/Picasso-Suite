//! Opening DaVinci Resolve files (desktop only): projects (`.drp`), timelines (`.drt`), bins
//! (`.drb`) and XML exports, read by the `picasso-drp` crate.
//!
//! The new project takes the Resolve project's name, frame rate and picture size. Each media
//! file it uses is imported as if it had been dropped on the window, so videos and sounds are
//! measured and images decoded; files that are not on this computer are added as offline media,
//! so the project still lists them.

use picasso_core::{Command, MediaSource, Project};

use crate::app::StudioApp;

impl StudioApp {
    /// Opens a Resolve file read into `bytes`; `name` is its file name.
    pub(crate) fn open_resolve(&mut self, name: &str, bytes: &[u8]) {
        let drp = match picasso_drp::read(bytes) {
            Ok(drp) => drp,
            Err(e) => {
                self.fail(format!("{name}: {e}"));
                return;
            }
        };
        let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
        // The settings come from the file; the media is imported one by one below.
        let mut settings = picasso_drp::to_project(&drp, stem);
        settings.media.clear();
        let mut project = Project::new(settings.name.clone());
        project.fps = settings.fps;
        project.width = settings.width;
        project.height = settings.height;
        self.replace_session(project, None);
        self.manager.open = false;

        // Importing a file reads it, so only the first files are imported; the rest are listed
        // as offline media. Network paths and addresses are never touched (see `refuse_path`).
        const MAX_IMPORTED: usize = 200;
        let (mut found, mut missing) = (0, 0);
        for media in &drp.media {
            let local = picasso_decode::refuse_path(&media.path).is_none();
            if local && found < MAX_IMPORTED && std::path::Path::new(&media.path).is_file() {
                self.open_path(&media.path, false);
                found += 1;
            } else {
                // Kept as offline media, so the pool shows what the project needs.
                let source = match media.kind {
                    picasso_drp::MediaKind::Image => MediaSource::Image {
                        path: media.path.clone(),
                    },
                    picasso_drp::MediaKind::Video => MediaSource::Video {
                        path: media.path.clone(),
                    },
                    picasso_drp::MediaKind::Audio => MediaSource::Audio {
                        path: media.path.clone(),
                    },
                };
                let stem = media
                    .name
                    .rsplit_once('.')
                    .map_or(media.name.as_str(), |(s, _)| s)
                    .to_string();
                let default_duration = self.project().fps.nominal() * 5;
                self.run(Command::AddMedia {
                    name: stem,
                    source,
                    length: None,
                    default_duration,
                });
                missing += 1;
            }
        }
        self.session.mark_saved();
        self.info(match (found, missing) {
            (0, 0) => format!("Opened {name}: it lists no media files"),
            (f, 0) => format!("Opened {name}: {f} media file(s) in the media pool"),
            (f, m) => format!(
                "Opened {name}: {f} media file(s) in the media pool, {m} not found on this \
                 computer (shown offline)"
            ),
        });
    }
}
