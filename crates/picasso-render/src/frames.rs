//! Rendered frames kept in memory (Roadmap Phase 3, "Frame cache LRU"), so playing a stretch again,
//! looping or stepping back shows pictures without decoding and compositing them again.
//!
//! The cache holds whole viewer pictures by what they depend on: the content of the project, the
//! timeline frame, the picture size and the proxy reduction. It holds up to a byte budget and
//! forgets the picture used longest ago first. It does no I/O; a disk tier lives in the app.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use crate::compose::Frame;

/// What a rendered picture depends on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FrameKey {
    /// A hash of everything in the project that changes a picture (see [`content_hash`]).
    pub content: u64,
    /// The timeline frame.
    pub frame: u64,
    pub width: u32,
    pub height: u32,
    /// How many times smaller video was decoded (1 is whole; a proxy is 2, 4 or 8).
    pub reduce: u32,
    /// Whether what no clip covers is left transparent (for the checkerboard background)
    /// instead of black.
    pub alpha: bool,
}

/// Rendered pictures in memory, the least recently used forgotten first.
#[derive(Debug)]
pub struct FrameCache {
    budget: usize,
    bytes: usize,
    clock: u64,
    entries: HashMap<FrameKey, (Arc<Frame>, u64)>,
    /// Keys by when they were last used, oldest first.
    order: BTreeMap<u64, FrameKey>,
}

impl FrameCache {
    /// A cache that holds up to `budget` bytes of pictures.
    pub fn new(budget: usize) -> Self {
        Self {
            budget,
            bytes: 0,
            clock: 0,
            entries: HashMap::new(),
            order: BTreeMap::new(),
        }
    }

    fn tick(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }

    /// The picture for `key`, if it is kept; it becomes the most recently used.
    pub fn get(&mut self, key: &FrameKey) -> Option<Arc<Frame>> {
        let now = self.tick();
        let (frame, used) = self.entries.get_mut(key)?;
        self.order.remove(used);
        *used = now;
        self.order.insert(now, *key);
        Some(frame.clone())
    }

    /// Keeps `frame` for `key`. A picture bigger than the whole budget is not kept.
    pub fn insert(&mut self, key: FrameKey, frame: Arc<Frame>) {
        let size = frame.rgba.len();
        if size > self.budget {
            return;
        }
        if let Some((old, used)) = self.entries.remove(&key) {
            self.order.remove(&used);
            self.bytes -= old.rgba.len();
        }
        while self.bytes + size > self.budget {
            let Some((_, oldest)) = self.order.pop_first() else {
                break;
            };
            if let Some((old, _)) = self.entries.remove(&oldest) {
                self.bytes -= old.rgba.len();
            }
        }
        let now = self.tick();
        self.order.insert(now, key);
        self.entries.insert(key, (frame, now));
        self.bytes += size;
    }

    /// Forgets every picture.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.bytes = 0;
    }

    /// How many bytes of pictures are kept.
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// How many pictures are kept.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// A 64-bit FNV-1a hash, the same on every platform and Rust version, so keys written to disk
/// stay valid between runs.
pub fn content_hash(parts: &[&[u8]]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for b in *part {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        // A separator, so ["ab", "c"] and ["a", "bc"] differ.
        h ^= 0xff;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(frame: u64) -> FrameKey {
        FrameKey {
            content: 1,
            frame,
            width: 2,
            height: 2,
            reduce: 1,
            alpha: false,
        }
    }

    fn pic(v: u8) -> Arc<Frame> {
        Arc::new(Frame {
            width: 2,
            height: 2,
            rgba: vec![v; 16],
        })
    }

    #[test]
    fn keeps_and_returns_pictures() {
        let mut c = FrameCache::new(1000);
        assert!(c.get(&key(1)).is_none());
        c.insert(key(1), pic(7));
        assert_eq!(c.get(&key(1)).map(|f| f.rgba[0]), Some(7));
        assert_eq!((c.len(), c.bytes()), (1, 16));
    }

    #[test]
    fn forgets_the_least_recently_used_first() {
        // Room for three 16-byte pictures.
        let mut c = FrameCache::new(48);
        c.insert(key(1), pic(1));
        c.insert(key(2), pic(2));
        c.insert(key(3), pic(3));
        // Using 1 makes 2 the oldest.
        assert!(c.get(&key(1)).is_some());
        c.insert(key(4), pic(4));
        assert!(c.get(&key(2)).is_none(), "the oldest went");
        assert!(c.get(&key(1)).is_some() && c.get(&key(3)).is_some());
        assert!(c.get(&key(4)).is_some());
        assert!(c.bytes() <= 48);
    }

    #[test]
    fn replacing_a_key_does_not_count_it_twice() {
        let mut c = FrameCache::new(1000);
        c.insert(key(1), pic(1));
        c.insert(key(1), pic(2));
        assert_eq!((c.len(), c.bytes()), (1, 16));
        assert_eq!(c.get(&key(1)).map(|f| f.rgba[0]), Some(2));
    }

    #[test]
    fn a_picture_bigger_than_the_budget_is_not_kept() {
        let mut c = FrameCache::new(8);
        c.insert(key(1), pic(1));
        assert!(c.is_empty());
    }

    #[test]
    fn clear_forgets_everything() {
        let mut c = FrameCache::new(1000);
        c.insert(key(1), pic(1));
        c.clear();
        assert!(c.is_empty() && c.bytes() == 0 && c.get(&key(1)).is_none());
    }

    #[test]
    fn the_content_hash_is_stable_and_separates_parts() {
        assert_eq!(content_hash(&[b"abc"]), content_hash(&[b"abc"]));
        assert_ne!(content_hash(&[b"ab", b"c"]), content_hash(&[b"a", b"bc"]));
        assert_ne!(content_hash(&[b"abc"]), content_hash(&[b"abd"]));
        // Pinned, so a change of algorithm is noticed (it would orphan the disk cache).
        assert_eq!(content_hash(&[]), 0xcbf2_9ce4_8422_2325);
    }
}
