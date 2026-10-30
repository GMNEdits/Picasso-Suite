//! Optimised media: small copies of video files that decode quickly, for smooth editing.
//!
//! A 4K file with few key frames takes seconds to show an exact picture. A copy at 1280×720
//! with a key frame every few pictures takes milliseconds, so the timeline and the viewer stay
//! smooth. The original is still what an export reads, and what sound is read from.
//!
//! The copies live in the data folder, not beside a project: one copy of a file serves every
//! project that uses it, and a project file never points at a copy. An index beside them says
//! which file each copy was made from, with that file's size and date, so a copy of a file that
//! has changed since is not used.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The longest side of an optimised copy, in pixels: a 4K and a 1080p file both become
/// 1280×720, and a file already smaller than that keeps its size.
pub(crate) const LONG_SIDE: u32 = 1280;

/// The index file, beside the copies.
const INDEX: &str = "index.txt";

/// What is known of one copy.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    /// The copy's file name in the folder.
    file: String,
    /// The size and date of the file it was made from: when either differs, the copy is stale.
    size: u64,
    modified: u64,
}

/// The copies that were made, by the file they were made from.
#[derive(Clone, Debug, Default)]
pub(crate) struct Store {
    /// Where the copies and the index are. `None` when there is no data folder (the browser).
    root: Option<PathBuf>,
    entries: HashMap<String, Entry>,
}

impl Store {
    /// The store in the data folder, as it is on disk.
    pub(crate) fn load() -> Store {
        let root = crate::project_manager::data_dir().map(|d| d.join("optimised"));
        Self::load_from(root)
    }

    /// The store kept under `root` (a test says where).
    pub(crate) fn load_from(root: Option<PathBuf>) -> Store {
        let mut store = Store {
            root,
            entries: HashMap::new(),
        };
        let Some(text) = store.index_path().and_then(|p| std::fs::read_to_string(p).ok()) else {
            return store;
        };
        for line in text.lines() {
            // path<TAB>file<TAB>size<TAB>modified
            let mut parts = line.split('\t');
            let (Some(source), Some(file), Some(size), Some(modified)) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            let (Ok(size), Ok(modified)) = (size.parse(), modified.parse()) else {
                continue;
            };
            if source.is_empty() || file.is_empty() {
                continue;
            }
            store.entries.insert(
                source.to_string(),
                Entry {
                    file: file.to_string(),
                    size,
                    modified,
                },
            );
        }
        store
    }

    fn index_path(&self) -> Option<PathBuf> {
        self.root.as_ref().map(|r| r.join(INDEX))
    }

    /// Where a copy of `source` would be kept: in the folder, named after the file and a hash of
    /// its whole path, so two files of one name in different folders do not share a copy.
    pub(crate) fn place_for(&self, source: &str) -> Option<PathBuf> {
        let root = self.root.as_ref()?;
        let stem: String = Path::new(source)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "media".into());
        let stem: String = stem
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .take(60)
            .collect();
        let stem = stem.trim_matches('_');
        let stem = if stem.is_empty() { "media" } else { stem };
        Some(root.join(format!("{stem}-{:08x}.mp4", hash(source))))
    }

    /// The copy to decode for `source`, when there is one and the file has not changed since.
    pub(crate) fn path_for(&self, source: &str) -> Option<PathBuf> {
        let entry = self.entries.get(source)?;
        let file = self.root.as_ref()?.join(&entry.file);
        if !file.is_file() {
            return None;
        }
        let (size, modified) = stamp(Path::new(source))?;
        (size == entry.size && modified == entry.modified).then_some(file)
    }

    /// Says that `file` is now the copy of `source`.
    pub(crate) fn record(&mut self, source: &str, file: &Path) {
        let Some((size, modified)) = stamp(Path::new(source)) else {
            return;
        };
        let Some(name) = file.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            return;
        };
        self.entries.insert(
            source.to_string(),
            Entry {
                file: name,
                size,
                modified,
            },
        );
    }

    /// Forgets the copy of `source`, deleting the file too.
    pub(crate) fn forget(&mut self, source: &str) {
        if let Some(entry) = self.entries.remove(source) {
            if let Some(root) = &self.root {
                let _ = std::fs::remove_file(root.join(entry.file));
            }
        }
    }

    /// Deletes every copy and the index.
    pub(crate) fn clear(&mut self) {
        let root = self.root.clone();
        for entry in self.entries.values() {
            if let Some(root) = &root {
                let _ = std::fs::remove_file(root.join(&entry.file));
            }
        }
        self.entries.clear();
        if let Some(index) = self.index_path() {
            let _ = std::fs::remove_file(index);
        }
    }

    /// Deletes the copies of files that are gone, and copies whose file has changed since.
    /// Returns how many went.
    pub(crate) fn prune(&mut self) -> usize {
        let stale: Vec<String> = self
            .entries
            .iter()
            .filter(|(source, entry)| match stamp(Path::new(source)) {
                Some((size, modified)) => size != entry.size || modified != entry.modified,
                // A file on a drive that is not plugged in is not gone: only a file that is
                // nowhere is dropped, and that is what `stamp` cannot tell apart. Kept.
                None => false,
            })
            .map(|(source, _)| source.clone())
            .collect();
        for source in &stale {
            self.forget(source);
        }
        stale.len()
    }

    /// Every copy, by the file it was made from.
    pub(crate) fn copies(&self) -> Vec<(String, PathBuf)> {
        let Some(root) = &self.root else {
            return Vec::new();
        };
        self.entries
            .iter()
            .map(|(source, entry)| (source.clone(), root.join(&entry.file)))
            .collect()
    }

    /// Writes the index, so the next start knows what is there. Written beside it first, so a
    /// crash while writing never leaves half an index.
    pub(crate) fn save(&self) {
        let (Some(root), Some(index)) = (self.root.as_ref(), self.index_path()) else {
            return;
        };
        if std::fs::create_dir_all(root).is_err() {
            return;
        }
        let mut text = String::new();
        let mut rows: Vec<_> = self.entries.iter().collect();
        rows.sort_by(|a, b| a.0.cmp(b.0));
        for (source, entry) in rows {
            text.push_str(&format!(
                "{source}\t{}\t{}\t{}\n",
                entry.file, entry.size, entry.modified
            ));
        }
        let tmp = index.with_extension("txt.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &index);
        }
    }
}

/// A file's size and the second it was last written.
fn stamp(path: &Path) -> Option<(u64, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Some((meta.len(), modified))
}

/// FNV-1a of the path's bytes: short, stable, and no dependency.
fn hash(text: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in text.as_bytes() {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// Makes the copy of `source` at `dest`: the file rendered again, smaller, with a key frame
/// every few pictures so a picture can be found without reading the ones before it.
///
/// It is the export pipeline on a project of one clip, so the copy has the file's own frame
/// rate and length and the frames line up with the original's: the same time asks for the same
/// picture. Runs on a thread of its own; `progress` gets the frames done and the total.
pub(crate) fn make(
    source: &Path,
    dest: &Path,
    progress: &mut dyn FnMut(u64, u64) -> bool,
) -> Result<(), String> {
    let info = picasso_decode::probe(source).map_err(|e| format!("{}: {e}", source.display()))?;
    let (num, den) = info.fps;
    if num == 0 || den == 0 {
        return Err(format!("{}: the file does not say its frame rate", source.display()));
    }
    let fps = picasso_core::Fps { num, den };
    let duration_us = info
        .duration_us
        .filter(|d| *d > 0)
        .ok_or_else(|| format!("{}: the file does not say how long it is", source.display()))?;
    let frames = (u128::try_from(duration_us).unwrap_or(0) * u128::from(num)
        / (1_000_000 * u128::from(den))) as u64;
    if frames == 0 {
        return Err(format!("{}: the file has no pictures", source.display()));
    }

    // A project of only this file, at the file's own size and frame rate.
    let mut project = picasso_core::Project::new("Optimised media");
    project.fps = fps;
    project.width = info.width.max(2);
    project.height = info.height.max(2);
    let mut session = picasso_core::Session::new(project);
    let name = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "media".into());
    let media = match session
        .dispatch(picasso_core::Command::AddMedia {
            name,
            source: picasso_core::MediaSource::Video {
                path: source.to_string_lossy().into_owned(),
            },
            length: Some(frames),
            default_duration: frames,
        })
        .map_err(|e| e.to_string())?
    {
        picasso_core::Outcome::Media(id) => id,
        other => return Err(format!("the file was not added: {other:?}")),
    };
    let track = match session
        .dispatch(picasso_core::Command::AddTrack {
            kind: picasso_core::TrackKind::Video,
        })
        .map_err(|e| e.to_string())?
    {
        picasso_core::Outcome::Track(id) => id,
        other => return Err(format!("the track was not added: {other:?}")),
    };
    session
        .dispatch(picasso_core::Command::AddClip {
            track,
            media,
            start: 0,
            duration: Some(frames),
            with_sound: false,
        })
        .map_err(|e| e.to_string())?;

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let cache = crate::video::new_cache(&egui::Context::default());
    let settings = picasso_export::Settings {
        preset: picasso_export::Preset::H264Web,
        path: dest.to_path_buf(),
        start: 0,
        end: frames,
        options: picasso_export::Options {
            // The sound stays with the original; a copy is only for the picture.
            audio: false,
            size: Some(copy_size((info.width, info.height))),
            fps: Some(fps),
            // A key frame often enough that a picture is found without reading back far, and no
            // pictures that refer to later ones, which is what makes scrubbing slow.
            keyframes: Some(8),
            frame_reordering: false,
            rate: picasso_export::Rate::Quality(picasso_export::Quality::Medium),
            ..picasso_export::Options::default()
        },
    };
    picasso_export::render(&session.project(), &cache, &settings, progress)
        .map_err(|e| format!("{}: {e}", dest.display()))
}

/// The size a copy of a `(width, height)` picture has: inside [`LONG_SIDE`] on its longest
/// side, the same shape, and never bigger than the file itself.
pub(crate) fn copy_size((width, height): (u32, u32)) -> (u32, u32) {
    let longest = width.max(height);
    if longest == 0 || longest <= LONG_SIDE {
        return (width.max(2), height.max(2));
    }
    let scale = f64::from(LONG_SIDE) / f64::from(longest);
    let w = ((f64::from(width) * scale).round() as u32).max(2) & !1;
    let h = ((f64::from(height) * scale).round() as u32).max(2) & !1;
    (w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("picasso-proxy-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn source(dir: &Path, name: &str) -> String {
        let path = dir.join(name);
        std::fs::write(&path, b"the original").unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn a_copy_is_found_by_the_file_it_was_made_from() {
        let dir = scratch("found");
        let store_dir = dir.join("store");
        let mut store = Store::load_from(Some(store_dir.clone()));
        let source = source(&dir, "A001.mov");
        let place = store.place_for(&source).unwrap();
        std::fs::create_dir_all(&store_dir).unwrap();
        std::fs::write(&place, b"the copy").unwrap();
        store.record(&source, &place);
        store.save();

        // A later start reads the index back.
        let again = Store::load_from(Some(store_dir));
        assert_eq!(again.path_for(&source), Some(place));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_files_of_one_name_in_different_folders_do_not_share_a_copy() {
        let dir = scratch("names");
        let store = Store::load_from(Some(dir.join("store")));
        std::fs::create_dir_all(dir.join("a")).unwrap();
        std::fs::create_dir_all(dir.join("b")).unwrap();
        let one = source(&dir.join("a"), "clip.mov");
        let two = source(&dir.join("b"), "clip.mov");
        assert_ne!(store.place_for(&one), store.place_for(&two));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_copy_of_a_file_that_changed_since_is_not_used() {
        let dir = scratch("stale");
        let store_dir = dir.join("store");
        let mut store = Store::load_from(Some(store_dir.clone()));
        let source = source(&dir, "A001.mov");
        let place = store.place_for(&source).unwrap();
        std::fs::create_dir_all(&store_dir).unwrap();
        std::fs::write(&place, b"the copy").unwrap();
        store.record(&source, &place);
        assert!(store.path_for(&source).is_some());

        // The file is written again: the copy no longer stands for it.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        std::fs::write(&source, b"the original, but longer").unwrap();
        assert_eq!(store.path_for(&source), None);
        assert_eq!(store.prune(), 1, "and pruning drops it");
        assert_eq!(store.path_for(&source), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_copy_whose_file_is_gone_is_not_used() {
        let dir = scratch("gone");
        let store_dir = dir.join("store");
        let mut store = Store::load_from(Some(store_dir.clone()));
        let source = source(&dir, "A001.mov");
        let place = store.place_for(&source).unwrap();
        std::fs::create_dir_all(&store_dir).unwrap();
        std::fs::write(&place, b"the copy").unwrap();
        store.record(&source, &place);
        std::fs::remove_file(&place).unwrap();
        assert_eq!(store.path_for(&source), None, "a copy that is not there is not used");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn forgetting_and_clearing_delete_the_files_too() {
        let dir = scratch("clear");
        let store_dir = dir.join("store");
        let mut store = Store::load_from(Some(store_dir.clone()));
        std::fs::create_dir_all(&store_dir).unwrap();
        let one = source(&dir, "one.mov");
        let two = source(&dir, "two.mov");
        let a = store.place_for(&one).unwrap();
        let b = store.place_for(&two).unwrap();
        std::fs::write(&a, b"a").unwrap();
        std::fs::write(&b, b"b").unwrap();
        store.record(&one, &a);
        store.record(&two, &b);

        store.forget(&one);
        assert!(!a.is_file(), "forgetting removes the copy");
        assert!(b.is_file(), "and only that one");

        store.clear();
        assert!(!b.is_file(), "clearing removes the rest");
        assert!(store.copies().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_copy_is_at_most_1280_on_its_longest_side_and_keeps_its_shape() {
        assert_eq!(copy_size((3840, 2160)), (1280, 720));
        assert_eq!(copy_size((1920, 1080)), (1280, 720));
        assert_eq!(copy_size((1080, 1920)), (720, 1280));
        assert_eq!(copy_size((1280, 720)), (1280, 720), "no bigger than the file");
        assert_eq!(copy_size((640, 360)), (640, 360));
        // An odd size stays even, which the encoders want.
        assert_eq!(copy_size((1919, 1079)), (1280, 720));
    }

    #[test]
    fn a_file_already_small_enough_is_kept_at_its_own_size() {
        assert_eq!(copy_size((720, 480)), (720, 480));
    }

    /// Two seconds of `size` at 25 fps; `None` when the `ffmpeg` program is not installed.
    fn video(dir: &Path, name: &str, size: &str) -> Option<PathBuf> {
        let path = dir.join(name);
        std::process::Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-f", "lavfi", "-i"])
            .arg(format!("testsrc=duration=2:size={size}:rate=25"))
            .args(["-pix_fmt", "yuv420p", "-g", "50"])
            .arg(&path)
            .status()
            .is_ok_and(|s| s.success())
            .then_some(path)
    }

    #[test]
    fn a_copy_is_the_same_length_and_frame_rate_at_a_smaller_size() {
        let dir = scratch("make");
        let Some(source) = video(&dir, "A001.mp4", "1920x1080") else {
            return; // no ffmpeg here
        };
        let dest = dir.join("copy.mp4");
        let mut frames = (0, 0);
        make(&source, &dest, &mut |done, total| {
            frames = (done, total);
            true
        })
        .unwrap();

        assert!(dest.is_file(), "the copy was written");
        let info = picasso_decode::probe(&dest).unwrap();
        assert_eq!((info.width, info.height), (1280, 720), "1280 on the long side");
        // The frame rate is the file's own. The container's average reads a hair over, because
        // the last picture is written without a length (an MP4 the export writes is one picture
        // short); the pictures themselves are where the original's are, which is what matters.
        let rate = f64::from(info.fps.0) / f64::from(info.fps.1.max(1));
        assert!((rate - 25.0).abs() < 0.6, "25 a second, not {rate}");
        let seconds = info.duration_us.unwrap() as f64 / 1e6;
        assert!((seconds - 2.0).abs() < 0.15, "two seconds, not {seconds}");
        assert_eq!(frames.1, 50, "50 pictures to write");
        assert_eq!(frames.0, 50, "and all of them written");

        // The same time is the same picture: one second in, the copy's picture starts at one
        // second, so a project that asks for a time gets the picture it asks for.
        let mut decoder = picasso_decode::VideoDecoder::open(&dest).unwrap();
        use picasso_decode::FrameProvider;
        let frame = decoder.frame_at(1_000_000).unwrap();
        assert!(
            (frame.pts_us - 1_000_000).abs() < 40_000,
            "one second in is one second in, not {}",
            frame.pts_us
        );
        assert_eq!((frame.width, frame.height), (1280, 720));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
