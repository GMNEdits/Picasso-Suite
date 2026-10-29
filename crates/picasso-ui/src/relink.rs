//! Relinking media whose files moved: finding them again in a folder and pointing the project
//! at them.
//!
//! A file is matched by its name, and only when exactly one file under the chosen folder has
//! it: relinking to the wrong take is worse than not relinking, so a name that appears twice is
//! reported rather than guessed. Every match is one undo step, and the picture the decoders
//! hold is dropped so the new file is what plays.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use egui::Context;
use picasso_core::{Command, MediaId, MediaSource};

use crate::app::StudioApp;
use crate::platform::{self, Incoming};

/// The most files looked at under the chosen folder, and how deep it is walked.
const MAX_FILES: usize = 200_000;
const MAX_DEPTH: usize = 8;

/// A media item whose file is not where the project says it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Missing {
    pub media: MediaId,
    /// The item's name in the pool, for the report.
    pub name: String,
    /// The path the project remembers.
    pub path: String,
    /// Whether it is sound, so the file found is read the same way.
    pub audio: bool,
}

/// A file found for a media item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FoundFile {
    pub media: MediaId,
    pub path: String,
    pub audio: bool,
}

/// What a search of a folder found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Search {
    pub found: Vec<FoundFile>,
    /// Names no file under the folder has.
    pub absent: Vec<String>,
    /// Names more than one file has: not guessed.
    pub ambiguous: Vec<String>,
}

impl StudioApp {
    /// Asks for a folder to look for the media whose files are gone.
    pub(crate) fn relink_missing(&mut self, ctx: &Context) {
        let missing = self.missing_media();
        if missing.is_empty() {
            self.info("Every media file is where the project says it is");
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = ctx;
            self.fail(format!(
                "relinking needs the desktop app ({} file(s) moved)",
                missing.len()
            ));
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.info(format!(
                "Looking for {} file(s) that moved: choose the folder to search",
                missing.len()
            ));
            platform::pick_relink_folder(self.inbox.clone(), ctx.clone());
        }
    }

    /// The media whose file is not on this computer.
    pub(crate) fn missing_media(&self) -> Vec<Missing> {
        self.project()
            .media
            .iter()
            .filter_map(|m| {
                let path = match &m.source {
                    MediaSource::Video { path }
                    | MediaSource::Audio { path }
                    | MediaSource::Image { path } => path,
                    _ => return None,
                };
                if path.trim().is_empty() || Path::new(path).exists() {
                    return None;
                }
                Some(Missing {
                    media: m.id,
                    name: m.name.clone(),
                    path: path.clone(),
                    audio: matches!(m.source, MediaSource::Audio { .. }),
                })
            })
            .collect()
    }

    /// A folder was chosen: looks through it in the background.
    pub(crate) fn search_for_media(&mut self, folder: String) {
        let missing = self.missing_media();
        if missing.is_empty() {
            return;
        }
        let (inbox, ctx) = (self.inbox.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let search = find(&missing, Path::new(&folder));
            // What each file says about itself, so the pool shows the new file, not the old one.
            let facts: Vec<(MediaId, Option<crate::projectfile::MediaMetadata>)> = search
                .found
                .iter()
                .map(|f| (f.media, crate::app::probe_file(&f.path, f.audio).ok()))
                .collect();
            inbox.push(
                &ctx,
                Incoming::Relinked {
                    found: search.found,
                    absent: search.absent,
                    ambiguous: search.ambiguous,
                    facts,
                    folder,
                },
            );
        });
    }

    /// The search came back: points the media at what was found, as one undo step.
    pub(crate) fn apply_relink(
        &mut self,
        found: Vec<FoundFile>,
        absent: Vec<String>,
        ambiguous: Vec<String>,
        facts: Vec<(MediaId, Option<crate::projectfile::MediaMetadata>)>,
        folder: String,
    ) {
        if !found.is_empty() {
            let commands: Vec<Command> = found
                .iter()
                .map(|f| Command::RelinkMedia {
                    media: f.media,
                    path: f.path.clone(),
                })
                .collect();
            if let Err(e) = self.session.dispatch_all(commands) {
                self.fail(format!("could not relink: {e}"));
                return;
            }
            // What the decoders hold is the old file.
            for f in &found {
                self.cache.remove(f.media);
                self.media_facts.remove(&f.media);
            }
            for (media, read) in facts {
                if let Some(read) = read {
                    self.media_facts.insert(media, read);
                }
            }
            self.media_generation += 1;
        }
        let mut note = if found.is_empty() {
            format!("Nothing was relinked from {folder}")
        } else {
            format!("Relinked {} file(s) from {folder}", found.len())
        };
        if !ambiguous.is_empty() {
            note.push_str(&format!(
                "; {} name(s) are in more than one file and were not guessed",
                ambiguous.len()
            ));
        }
        if !absent.is_empty() {
            note.push_str(&format!("; {} not found", absent.len()));
        }
        if found.is_empty() {
            self.fail(note);
        } else {
            self.info(note);
        }
    }
}

/// Looks for each missing file under `folder`, by name.
pub(crate) fn find(missing: &[Missing], folder: &Path) -> Search {
    let by_name = index(folder);
    let mut search = Search::default();
    // A name two media items share cannot be told apart by it: one file with that name would
    // stand for both takes, so neither is relinked.
    let mut shared: HashMap<String, usize> = HashMap::new();
    for item in missing {
        if let Some(key) = name_key(&item.path) {
            *shared.entry(key).or_default() += 1;
        }
    }
    for item in missing {
        let Some(key) = name_key(&item.path) else {
            search.absent.push(item.name.clone());
            continue;
        };
        if shared.get(&key).copied().unwrap_or(0) > 1 {
            search.ambiguous.push(item.name.clone());
            continue;
        }
        let Some(candidates) = by_name.get(&key) else {
            search.absent.push(item.name.clone());
            continue;
        };
        // One file with that name, and of the kind the media is.
        let mut usable = candidates
            .iter()
            .filter(|p| fits(item.audio, p))
            .cloned()
            .collect::<Vec<_>>();
        usable.sort();
        match usable.len() {
            1 => search.found.push(FoundFile {
                media: item.media,
                path: usable[0].to_string_lossy().into_owned(),
                audio: item.audio,
            }),
            0 => search.absent.push(item.name.clone()),
            _ => search.ambiguous.push(item.name.clone()),
        }
    }
    search
}

/// The lowercase file name of a path, which is what a relink matches on.
fn name_key(path: &str) -> Option<String> {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
}

/// Every file under `folder`, by lowercase file name.
fn index(folder: &Path) -> HashMap<String, Vec<PathBuf>> {
    let mut by_name: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let mut stack = vec![(folder.to_path_buf(), 0usize)];
    let mut seen = 0usize;
    while let Some((dir, depth)) = stack.pop() {
        if depth > MAX_DEPTH || seen >= MAX_FILES {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if seen >= MAX_FILES {
                break;
            }
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_dir() {
                stack.push((entry.path(), depth + 1));
            } else if kind.is_file() {
                seen += 1;
                if let Some(name) = entry.file_name().to_str() {
                    by_name
                        .entry(name.to_lowercase())
                        .or_default()
                        .push(entry.path());
                }
            }
        }
    }
    by_name
}

/// Whether `path` can stand in for a media item: the same kind of file.
fn fits(audio: bool, path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let is_video = platform::is_video_name(&name);
    let is_audio = platform::is_audio_name(&name);
    if audio {
        is_audio
    } else if is_video {
        true
    } else {
        // A picture, or a file this program has no list for.
        !is_audio
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("picasso-relink-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn missing(id: u64, path: &str) -> Missing {
        Missing {
            media: MediaId(id),
            name: format!("shot {id}"),
            path: path.to_string(),
            audio: false,
        }
    }

    #[test]
    fn a_file_with_the_same_name_in_another_folder_is_found() {
        let dir = scratch("found");
        std::fs::create_dir_all(dir.join("day 2/cards")).unwrap();
        std::fs::write(dir.join("day 2/cards/A001.mov"), b"x").unwrap();
        let search = find(&[missing(1, "/old/cards/A001.mov")], &dir);
        assert_eq!(
            search.found,
            vec![FoundFile {
                media: MediaId(1),
                path: dir
                    .join("day 2/cards/A001.mov")
                    .to_string_lossy()
                    .into_owned(),
                audio: false,
            }]
        );
        assert!(search.absent.is_empty() && search.ambiguous.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_name_in_more_than_one_file_is_not_guessed() {
        let dir = scratch("ambiguous");
        std::fs::create_dir_all(dir.join("a")).unwrap();
        std::fs::create_dir_all(dir.join("b")).unwrap();
        std::fs::write(dir.join("a/A001.mov"), b"x").unwrap();
        std::fs::write(dir.join("b/A001.mov"), b"y").unwrap();
        let search = find(&[missing(1, "/old/A001.mov")], &dir);
        assert!(search.found.is_empty(), "two takes are not one");
        assert_eq!(search.ambiguous, vec!["shot 1".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_media_items_with_one_name_are_not_both_pointed_at_one_file() {
        let dir = scratch("shared");
        std::fs::write(dir.join("A001.mov"), b"x").unwrap();
        // Two takes, both called A001.mov: one file cannot stand for both.
        let search = find(
            &[missing(1, "/card1/A001.mov"), missing(2, "/card2/A001.mov")],
            &dir,
        );
        assert!(search.found.is_empty(), "one file cannot stand for two takes");
        assert_eq!(search.ambiguous.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_of_another_kind_does_not_take_a_media_items_place() {
        let dir = scratch("kind");
        std::fs::write(dir.join("A001.mov"), b"x").unwrap();
        // The media is sound; a video of the same name is not its file.
        let mut sound = missing(1, "/old/A001.wav");
        sound.audio = true;
        let search = find(&[sound], &dir);
        assert!(search.found.is_empty());
        assert_eq!(search.absent, vec!["shot 1".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nothing_found_is_reported_by_name() {
        let dir = scratch("none");
        let search = find(
            &[missing(1, "/old/A001.mov"), missing(2, "/old/A002.mov")],
            &dir,
        );
        assert!(search.found.is_empty());
        assert_eq!(search.absent.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn relinking_points_the_project_at_the_file_and_can_be_undone() {
        use picasso_core::Outcome;

        let dir = scratch("apply");
        let folder = dir.join("moved");
        std::fs::create_dir_all(&folder).unwrap();
        let file = folder.join("A001.mov");
        std::fs::write(&file, b"x").unwrap();

        let ctx = egui::Context::default();
        let mut app = StudioApp::new(&ctx);
        // A media item whose file is not where the project says it is.
        let media = match app.run(Command::AddMedia {
            name: "A001".into(),
            source: MediaSource::Video {
                path: "/gone/cards/A001.mov".into(),
            },
            length: Some(100),
            default_duration: 50,
        }) {
            Some(Outcome::Media(id)) => id,
            _ => panic!("the media was not added"),
        };
        assert_eq!(app.missing_media().len(), 1, "the file is gone");

        let search = find(&app.missing_media(), &folder);
        assert_eq!(search.found.len(), 1, "the file is found again");
        app.apply_relink(
            search.found,
            search.absent,
            search.ambiguous,
            Vec::new(),
            folder.to_string_lossy().into_owned(),
        );

        let path = match &app.project().media(media).unwrap().source {
            MediaSource::Video { path } => path.clone(),
            _ => panic!("the media changed kind"),
        };
        assert_eq!(path, file.to_string_lossy(), "the project points at it");
        assert!(app.missing_media().is_empty(), "nothing is missing now");

        // One undo step puts the old path back.
        assert!(app.session.undo());
        let old = match &app.project().media(media).unwrap().source {
            MediaSource::Video { path } => path.clone(),
            _ => panic!("the media changed kind"),
        };
        assert_eq!(old, "/gone/cards/A001.mov");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
