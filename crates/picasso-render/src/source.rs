//! A pool of video decoders keyed by (media, output size), with LRU eviction.

use crate::compositor::FrameSource;
use picasso_core::{Id, MediaItem};
use picasso_media::{RgbaImage, VideoDecoder};
use std::collections::HashMap;
use std::sync::Arc;

type Key = (Id, u32, u32);

pub struct DecoderPool {
    decoders: Vec<(Key, VideoDecoder, u64)>,
    tick: u64,
    max_decoders: usize,
    failed: HashMap<Id, String>,
}

impl DecoderPool {
    pub fn new(max_decoders: usize) -> Self {
        Self { decoders: Vec::new(), tick: 0, max_decoders: max_decoders.max(1), failed: HashMap::new() }
    }

    /// Media that failed to decode, with the error. Cleared by [`Self::reset`].
    pub fn failures(&self) -> &HashMap<Id, String> {
        &self.failed
    }

    /// Closes all decoders and forgets failures (e.g. after relinking media).
    pub fn reset(&mut self) {
        self.decoders.clear();
        self.failed.clear();
    }
}

impl FrameSource for DecoderPool {
    fn video_frame(&mut self, media: &MediaItem, seconds: f64, width: u32, height: u32) -> Option<Arc<RgbaImage>> {
        if self.failed.contains_key(&media.id) {
            return None;
        }
        self.tick += 1;
        let key = (media.id, width, height);
        let idx = match self.decoders.iter().position(|(k, ..)| *k == key) {
            Some(i) => i,
            None => {
                if self.decoders.len() >= self.max_decoders {
                    let lru = self.decoders.iter().enumerate().min_by_key(|(_, d)| d.2).map(|(i, _)| i).unwrap();
                    self.decoders.swap_remove(lru);
                }
                match VideoDecoder::new(media, width, height) {
                    Ok(d) => self.decoders.push((key, d, self.tick)),
                    Err(e) => {
                        self.failed.insert(media.id, e.to_string());
                        return None;
                    }
                }
                self.decoders.len() - 1
            }
        };
        let entry = &mut self.decoders[idx];
        entry.2 = self.tick;
        match entry.1.frame_at(seconds) {
            Ok(frame) => frame,
            Err(e) => {
                log::warn!("decode failed for {}: {e}", media.name);
                self.failed.insert(media.id, e.to_string());
                self.decoders.swap_remove(idx);
                None
            }
        }
    }
}
