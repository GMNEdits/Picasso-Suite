//! The viewer's render cache (Roadmap Phase 3, "Frame cache LRU (RAM + disk)").
//!
//! Every final viewer picture is kept in memory, up to [`RAM_BUDGET`]. Pictures that were slow to
//! make (over [`SLOW`]: heavy grades, several layers, large video) are also written to disk on a
//! thread of their own, up to [`DISK_BUDGET`], so they come back after the app is restarted.
//! Cheap pictures stay in memory only, which spares the disk 100 MB a second of writes while
//! playing. Playback › Delete Render Cache empties both.
//!
//! A picture is found by what it depends on: a hash of the project and of its media files' sizes
//! and times, the timeline frame, the size and the proxy. Any edit changes the hash, so a cached
//! picture is never shown for a project it does not belong to.

use std::sync::Arc;

use picasso_core::{MediaSource, Project};
use picasso_render::{content_hash, Frame, FrameCache, FrameKey};

/// Memory for viewer pictures: about 500 pictures of 960 x 540.
const RAM_BUDGET: usize = 1 << 30;
/// Disk space for slow pictures.
#[cfg_attr(any(test, target_arch = "wasm32"), allow(dead_code))]
const DISK_BUDGET: u64 = 4 << 30;
/// A picture that took longer than this to make is also kept on disk.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(crate) const SLOW: std::time::Duration = std::time::Duration::from_millis(20);

pub(crate) struct RenderCache {
    ram: FrameCache,
    /// The content hash and what it was worked out for (epoch, revision, media generation).
    content: Option<((u64, u64, u64), u64)>,
    #[cfg(not(target_arch = "wasm32"))]
    disk: Option<disk::Disk>,
}

impl RenderCache {
    /// A cache in memory, and on disk in the user's cache folder (not in tests or the browser).
    pub(crate) fn new() -> Self {
        Self {
            ram: FrameCache::new(RAM_BUDGET),
            content: None,
            #[cfg(all(not(target_arch = "wasm32"), not(test)))]
            disk: disk::default_dir().and_then(|dir| disk::Disk::open(dir, DISK_BUDGET).ok()),
            #[cfg(all(not(target_arch = "wasm32"), test))]
            disk: None,
        }
    }

    /// A cache that keeps its disk pictures in `dir`.
    #[cfg(all(not(target_arch = "wasm32"), test))]
    pub(crate) fn with_disk(dir: std::path::PathBuf, budget: u64) -> Self {
        Self {
            ram: FrameCache::new(RAM_BUDGET),
            content: None,
            disk: disk::Disk::open(dir, budget).ok(),
        }
    }

    /// The hash of everything in `project` that changes a picture. `stamp` changes whenever the
    /// project or its media may have changed; the hash is only worked out again then.
    pub(crate) fn content(&mut self, stamp: (u64, u64, u64), project: &Project) -> u64 {
        if let Some((s, hash)) = self.content {
            if s == stamp {
                return hash;
            }
        }
        let json = project.to_json().unwrap_or_default();
        let mut parts: Vec<Vec<u8>> = vec![json.into_bytes()];
        for item in &project.media {
            let path = match &item.source {
                MediaSource::Image { path }
                | MediaSource::Video { path }
                | MediaSource::Audio { path } => path,
                _ => continue,
            };
            parts.push(file_stamp(path));
        }
        let slices: Vec<&[u8]> = parts.iter().map(Vec::as_slice).collect();
        let hash = content_hash(&slices);
        self.content = Some((stamp, hash));
        hash
    }

    /// The picture for `key`, from memory or else from disk.
    pub(crate) fn get(&mut self, key: &FrameKey) -> Option<Arc<Frame>> {
        if let Some(frame) = self.ram.get(key) {
            return Some(frame);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(frame) = self.disk.as_mut().and_then(|d| d.load(key)) {
            let frame = Arc::new(frame);
            self.ram.insert(*key, frame.clone());
            return Some(frame);
        }
        None
    }

    /// Keeps a final picture; a `slow` one is written to disk as well.
    pub(crate) fn put(&mut self, key: FrameKey, frame: Arc<Frame>, slow: bool) {
        #[cfg(not(target_arch = "wasm32"))]
        if slow {
            if let Some(d) = self.disk.as_mut() {
                d.store(key, frame.clone());
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = slow;
        self.ram.insert(key, frame);
    }

    /// Renders frame `key.frame` of `project`, or takes it from the cache, and keeps a final
    /// picture for next time. The viewer does the same in the background ([`crate::composer`]).
    #[cfg(test)]
    pub(crate) fn render(
        &mut self,
        project: &Project,
        media: &picasso_render::MediaCache,
        key: FrameKey,
    ) -> Option<Arc<Frame>> {
        if let Some(frame) = self.get(&key) {
            return Some(frame);
        }
        #[cfg(not(target_arch = "wasm32"))]
        let started = std::time::Instant::now();
        let (frame, exact) = picasso_render::render_frame_alpha(
            project, media, key.frame, key.width, key.height, key.alpha,
        )
        .ok()?;
        let frame = Arc::new(frame);
        if exact {
            #[cfg(not(target_arch = "wasm32"))]
            let slow = started.elapsed() >= SLOW;
            #[cfg(target_arch = "wasm32")]
            let slow = false;
            self.put(key, frame.clone(), slow);
        }
        Some(frame)
    }

    /// Forgets every picture, in memory and on disk. Returns how many files were deleted.
    pub(crate) fn clear(&mut self) -> Result<usize, String> {
        self.ram.clear();
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(d) = self.disk.as_mut() {
            return d.clear();
        }
        Ok(0)
    }

    /// How many pictures are in memory.
    #[cfg(test)]
    pub(crate) fn in_memory(&self) -> usize {
        self.ram.len()
    }
}

/// The size and modification time of a file, so replacing a file under the same name changes
/// the hash. A file that cannot be read gives nothing.
fn file_stamp(path: &str) -> Vec<u8> {
    #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
    let mut out = path.as_bytes().to_vec();
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(meta) = std::fs::metadata(path) {
        out.extend_from_slice(&meta.len().to_le_bytes());
        if let Ok(Ok(since)) = meta
            .modified()
            .map(|t| t.duration_since(std::time::UNIX_EPOCH))
        {
            out.extend_from_slice(&since.as_nanos().to_le_bytes());
        }
    }
    out
}

#[cfg(not(target_arch = "wasm32"))]
mod disk {
    use std::collections::HashSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
    use std::sync::Arc;

    use picasso_render::{Frame, FrameKey};

    /// The first bytes of a cached picture, then width and height (little-endian u32).
    const MAGIC: &[u8; 4] = b"PFC1";
    const HEADER: usize = 12;
    /// The extension of cached pictures; nothing else in the folder is ever deleted.
    const EXTENSION: &str = "pfc";
    /// The biggest picture read back (a 16K frame).
    const MAX_PIXELS: u64 = 16_384 * 16_384;
    /// Pictures waiting to be written; more are dropped rather than slowing the viewer down.
    const QUEUE: usize = 16;
    /// The folder is trimmed to its budget after this many writes.
    const PRUNE_EVERY: u32 = 64;

    /// Where the render cache lives: `PICASSO_CACHE_DIR`, or the platform's cache folder.
    #[cfg_attr(test, allow(dead_code))]
    pub(super) fn default_dir() -> Option<PathBuf> {
        if let Some(dir) = std::env::var_os("PICASSO_CACHE_DIR") {
            return Some(PathBuf::from(dir));
        }
        let base = if cfg!(windows) {
            std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
        } else if cfg!(target_os = "macos") {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library").join("Caches"))
        } else {
            std::env::var_os("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        }?;
        Some(base.join("picasso-suite").join("render-cache"))
    }

    fn file_name(key: &FrameKey) -> String {
        format!(
            "{:016x}-{}-{}x{}-{}-{}.{EXTENSION}",
            key.content,
            key.frame,
            key.width,
            key.height,
            key.reduce,
            if key.alpha { "a" } else { "o" }
        )
    }

    fn parse_name(name: &str) -> Option<FrameKey> {
        let stem = name.strip_suffix(&format!(".{EXTENSION}"))?;
        let mut parts = stem.split('-');
        let content = u64::from_str_radix(parts.next()?, 16).ok()?;
        let frame = parts.next()?.parse().ok()?;
        let (w, h) = parts.next()?.split_once('x')?;
        let reduce = parts.next()?.parse().ok()?;
        let alpha = match parts.next()? {
            "a" => true,
            "o" => false,
            _ => return None,
        };
        if parts.next().is_some() {
            return None;
        }
        Some(FrameKey {
            content,
            frame,
            width: w.parse().ok()?,
            height: h.parse().ok()?,
            reduce,
            alpha,
        })
    }

    fn encode(frame: &Frame) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(HEADER + frame.rgba.len());
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&frame.width.to_le_bytes());
        bytes.extend_from_slice(&frame.height.to_le_bytes());
        bytes.extend_from_slice(&frame.rgba);
        bytes
    }

    /// A picture from the bytes of a cache file, if they are whole and the size `key` says.
    pub(super) fn decode(bytes: Vec<u8>, key: &FrameKey) -> Option<Frame> {
        let head = bytes.get(..HEADER)?;
        if &head[..4] != MAGIC {
            return None;
        }
        let w = u32::from_le_bytes(head.get(4..8)?.try_into().ok()?);
        let h = u32::from_le_bytes(head.get(8..12)?.try_into().ok()?);
        let pixels = u64::from(w).checked_mul(u64::from(h))?;
        if (w, h) != (key.width, key.height) || pixels == 0 || pixels > MAX_PIXELS {
            return None;
        }
        let len = usize::try_from(pixels.checked_mul(4)?).ok()?;
        if bytes.len() != HEADER.checked_add(len)? {
            return None;
        }
        let mut rgba = bytes;
        rgba.drain(..HEADER);
        Some(Frame {
            width: w,
            height: h,
            rgba,
        })
    }

    pub(super) struct Disk {
        dir: PathBuf,
        /// The pictures in the folder (or on their way there).
        known: HashSet<FrameKey>,
        writer: SyncSender<(FrameKey, Arc<Frame>)>,
    }

    impl Disk {
        /// Opens (making it if needed) the cache folder, and starts its writer thread, which
        /// first trims the folder to `budget` bytes.
        pub(super) fn open(dir: PathBuf, budget: u64) -> Result<Self, String> {
            fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let known = fs::read_dir(&dir)
                .map_err(|e| e.to_string())?
                .flatten()
                .filter_map(|e| parse_name(&e.file_name().to_string_lossy()))
                .collect();
            let (writer, jobs) = sync_channel(QUEUE);
            let folder = dir.clone();
            std::thread::Builder::new()
                .name("render-cache".into())
                .spawn(move || write_loop(&folder, budget, &jobs))
                .map_err(|e| e.to_string())?;
            Ok(Self { dir, known, writer })
        }

        pub(super) fn load(&mut self, key: &FrameKey) -> Option<Frame> {
            if !self.known.contains(key) {
                return None;
            }
            let frame = fs::read(self.dir.join(file_name(key)))
                .ok()
                .and_then(|bytes| decode(bytes, key));
            if frame.is_none() {
                // Gone, half written or damaged: forget it.
                self.known.remove(key);
            }
            frame
        }

        pub(super) fn store(&mut self, key: FrameKey, frame: Arc<Frame>) {
            if self.known.contains(&key) {
                return;
            }
            match self.writer.try_send((key, frame)) {
                Ok(()) => {
                    self.known.insert(key);
                }
                // The writer is busy or gone; this picture stays in memory only.
                Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {}
            }
        }

        /// Deletes every cached picture in the folder.
        pub(super) fn clear(&mut self) -> Result<usize, String> {
            self.known.clear();
            let mut deleted = 0;
            for entry in fs::read_dir(&self.dir)
                .map_err(|e| e.to_string())?
                .flatten()
            {
                let path = entry.path();
                if is_ours(&path) && fs::remove_file(&path).is_ok() {
                    deleted += 1;
                }
            }
            Ok(deleted)
        }
    }

    /// A cached picture or a half-written one; nothing else in the folder is touched.
    fn is_ours(path: &Path) -> bool {
        path.extension()
            .is_some_and(|e| e == EXTENSION || e == "tmp")
    }

    fn write_loop(dir: &Path, budget: u64, jobs: &Receiver<(FrameKey, Arc<Frame>)>) {
        prune(dir, budget);
        let mut since_prune = 0;
        while let Ok((key, frame)) = jobs.recv() {
            let path = dir.join(file_name(&key));
            let partial = path.with_extension("tmp");
            // Written aside and renamed, so a reader never sees half a picture.
            if fs::write(&partial, encode(&frame)).is_ok() && fs::rename(&partial, &path).is_err() {
                let _ = fs::remove_file(&partial);
            }
            since_prune += 1;
            if since_prune >= PRUNE_EVERY {
                since_prune = 0;
                prune(dir, budget);
            }
        }
    }

    /// Deletes the oldest cached pictures until the folder holds at most `budget` bytes.
    pub(super) fn prune(dir: &Path, budget: u64) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
            .flatten()
            .filter(|e| is_ours(&e.path()))
            .filter_map(|e| {
                let meta = e.metadata().ok()?;
                Some((meta.modified().ok()?, meta.len(), e.path()))
            })
            .collect();
        let mut total: u64 = files.iter().map(|(_, len, _)| len).sum();
        files.sort();
        for (_, len, path) in files {
            if total <= budget {
                break;
            }
            if fs::remove_file(&path).is_ok() {
                total = total.saturating_sub(len);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn key() -> FrameKey {
            FrameKey {
                content: 0xabc,
                frame: 42,
                width: 3,
                height: 2,
                reduce: 2,
                alpha: false,
            }
        }

        #[test]
        fn names_round_trip_and_others_are_ignored() {
            assert_eq!(parse_name(&file_name(&key())), Some(key()));
            assert_eq!(parse_name("notes.txt"), None);
            assert_eq!(parse_name("zz-1-2x2-1.pfc"), None);
            assert_eq!(parse_name("1-2-3x4-1-9.pfc"), None);
        }

        #[test]
        fn damaged_or_wrong_size_files_are_refused() {
            let frame = Frame {
                width: 3,
                height: 2,
                rgba: vec![9; 24],
            };
            let good = encode(&frame);
            assert_eq!(decode(good.clone(), &key()), Some(frame));
            let mut short = good.clone();
            short.pop();
            assert_eq!(decode(short, &key()), None, "truncated");
            let mut magic = good.clone();
            magic[0] = b'X';
            assert_eq!(decode(magic, &key()), None, "not ours");
            let other = FrameKey { width: 2, ..key() };
            assert_eq!(decode(good, &other), None, "another size");
            assert_eq!(decode(Vec::new(), &key()), None);
            // A header that claims an enormous picture does not allocate it.
            let mut huge = MAGIC.to_vec();
            huge.extend_from_slice(&u32::MAX.to_le_bytes());
            huge.extend_from_slice(&u32::MAX.to_le_bytes());
            let big = FrameKey {
                width: u32::MAX,
                height: u32::MAX,
                ..key()
            };
            assert_eq!(decode(huge, &big), None);
        }

        #[test]
        fn prune_keeps_the_newest_within_the_budget_and_only_touches_ours() {
            let dir = std::env::temp_dir().join("picasso_prune_test");
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            for n in 0..4u64 {
                let k = FrameKey { frame: n, ..key() };
                fs::write(dir.join(file_name(&k)), vec![0u8; 100]).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            fs::write(dir.join("keep.txt"), vec![0u8; 1000]).unwrap();
            prune(&dir, 250);
            let left: Vec<String> = fs::read_dir(&dir)
                .unwrap()
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            assert!(left.contains(&"keep.txt".to_string()));
            assert_eq!(left.iter().filter(|n| n.ends_with(".pfc")).count(), 2);
            assert!(left.contains(&file_name(&FrameKey { frame: 3, ..key() })));
            let _ = fs::remove_dir_all(&dir);
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use picasso_core::{Command, Outcome};
    use picasso_render::MediaCache;

    fn project() -> Project {
        let mut p = Project::new("t");
        p.width = 64;
        p.height = 36;
        if let Ok(Outcome::Media(m)) = (Command::AddMedia {
            name: "c".into(),
            source: MediaSource::Solid { rgb: [10, 20, 30] },
            length: None,
            default_duration: 10,
        })
        .apply(&mut p)
        {
            let track = p.tracks[0].id;
            let _ = Command::AddClip {
                track,
                media: m,
                start: 0,
                duration: None,
                with_sound: false,
            }
            .apply(&mut p);
        }
        p
    }

    fn key(content: u64, frame: u64) -> FrameKey {
        FrameKey {
            content,
            frame,
            width: 64,
            height: 36,
            reduce: 1,
            alpha: false,
        }
    }

    #[test]
    fn the_content_hash_follows_edits_and_only_recomputes_on_a_new_stamp() {
        let mut cache = RenderCache::new();
        let mut p = project();
        let a = cache.content((0, 0, 0), &p);
        p.name = "renamed".into();
        assert_eq!(cache.content((0, 0, 0), &p), a, "same stamp, same answer");
        assert_ne!(cache.content((0, 1, 0), &p), a, "an edit changes it");
    }

    #[test]
    fn a_rendered_frame_is_kept_and_found_again() {
        let mut cache = RenderCache::new();
        let p = project();
        let media = MediaCache::new();
        let k = key(cache.content((0, 0, 0), &p), 3);
        let first = cache.render(&p, &media, k).unwrap();
        assert_eq!(cache.in_memory(), 1);
        let again = cache.render(&p, &media, k).unwrap();
        assert!(
            Arc::ptr_eq(&first, &again),
            "the second time comes from the cache"
        );
        assert_eq!(&first.rgba[..4], &[10, 20, 30, 255]);
        cache.clear().unwrap();
        assert_eq!(cache.in_memory(), 0);
    }

    #[test]
    fn slow_pictures_survive_a_restart_on_disk_and_delete_clears_them() {
        let dir = std::env::temp_dir().join("picasso_render_cache_test");
        let _ = std::fs::remove_dir_all(&dir);
        let frame = Arc::new(Frame {
            width: 64,
            height: 36,
            rgba: vec![7; 64 * 36 * 4],
        });
        {
            let mut cache = RenderCache::with_disk(dir.clone(), 1 << 30);
            cache.put(key(5, 1), frame.clone(), true);
            cache.put(key(5, 2), frame.clone(), false);
        }
        // The writer thread finishes on its own; wait for the file.
        let wanted = dir.join(disk_name(5, 1));
        for _ in 0..100 {
            if wanted.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let mut cache = RenderCache::with_disk(dir.clone(), 1 << 30);
        assert_eq!(
            cache.get(&key(5, 1)).as_deref(),
            Some(&*frame),
            "slow: on disk"
        );
        assert!(cache.get(&key(5, 2)).is_none(), "cheap: memory only");
        assert_eq!(cache.clear(), Ok(1));
        assert!(!wanted.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn disk_name(content: u64, frame: u64) -> String {
        format!("{content:016x}-{frame}-64x36-1.pfc")
    }
}
