//! The decoded pictures of imported still images.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;

use image::ImageReader;
use picasso_core::MediaId;

use crate::compose::RenderError;

/// Images bigger than this on a side are refused.
const MAX_SIDE: u32 = 16_384;

/// A decoded image, 8 bits per channel, straight alpha.
#[derive(Debug)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Decoded {
    /// Decodes PNG or JPEG bytes. The format is found from the bytes, not from a file name.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, RenderError> {
        let reader = || {
            ImageReader::new(Cursor::new(bytes))
                .with_guessed_format()
                .map_err(|e| RenderError::Decode(e.to_string()))
        };
        // The size is checked from the header first, so a huge picture is refused before it is
        // decoded into memory.
        let (width, height) = reader()?
            .into_dimensions()
            .map_err(|e| RenderError::Decode(e.to_string()))?;
        if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
            return Err(RenderError::Decode("the image is too big".into()));
        }
        let image = reader()?
            .decode()
            .map_err(|e| RenderError::Decode(e.to_string()))?;
        Ok(Self {
            width,
            height,
            rgba: image.into_rgba8().into_raw(),
        })
    }

    /// The same picture made smaller, keeping its shape, so that it fits in `max_w` x `max_h`.
    /// A picture that already fits (or one whose pixels do not match its size) is kept as it is.
    pub fn fit_within(self, max_w: u32, max_h: u32) -> Self {
        if self.width <= max_w && self.height <= max_h || max_w == 0 || max_h == 0 {
            return self;
        }
        // Resampled straight from the pixels: copying a big picture first doubles its memory.
        let Some(buffer) = image::ImageBuffer::<image::Rgba<u8>, &[u8]>::from_raw(
            self.width,
            self.height,
            &self.rgba,
        ) else {
            return self;
        };
        let scale = (max_w as f32 / self.width as f32).min(max_h as f32 / self.height as f32);
        let w = ((self.width as f32 * scale).round() as u32).clamp(1, max_w);
        let h = ((self.height as f32 * scale).round() as u32).clamp(1, max_h);
        let small = image::imageops::resize(&buffer, w, h, image::imageops::FilterType::Triangle);
        Self {
            width: w,
            height: h,
            rgba: small.into_raw(),
        }
    }
}

/// What a video can show at one moment.
#[derive(Clone, Debug)]
pub enum VideoFrame {
    /// The exact picture asked for.
    Ready(Arc<Decoded>),
    /// A picture a little earlier or later than asked for, shown while the right one is still
    /// being decoded. A frame drawn with one is not kept in the frame cache.
    StandIn(Arc<Decoded>),
    /// Nothing decoded yet; the picture will come and `FrameSource::generation` will change.
    Pending,
    /// The file cannot be read.
    Offline,
}

/// Gives the pictures of video files (the desktop app decodes them with FFmpeg in background
/// threads; the browser has no implementation). The compositor asks it while it renders, so the
/// viewer, the media pool thumbnails and an export all see the same pictures.
pub trait FrameSource: Send + Sync + std::fmt::Debug {
    /// The picture of video `media`, read from `path`, shown `time_us` microseconds into the file,
    /// `reduce` times smaller on each side (1 is the whole picture; a proxy is 2, 4 or 8). With
    /// `wait` false it never blocks: it starts decoding and answers with what it has. With `wait`
    /// true it returns only when the exact picture is there (for exports and thumbnails).
    fn video_frame(
        &self,
        media: MediaId,
        path: &str,
        time_us: i64,
        reduce: u32,
        wait: bool,
    ) -> VideoFrame;

    /// Changes whenever a decoded picture arrives, so a picture that used a stand-in is drawn again.
    fn generation(&self) -> u64 {
        0
    }
}

/// How much smaller than the file the media pool's video thumbnails are decoded.
const THUMBNAIL_REDUCTION: u32 = 4;

/// Decoded images by the media item they belong to. Cloning is cheap: the pictures are shared.
#[derive(Clone, Debug, Default)]
pub struct MediaCache {
    /// Shared, so a copy of the cache (one for each picture the viewer's thread puts together,
    /// many a second while playing) costs one count, not a copy of every image's entry. Adding
    /// or removing one copies the map only while another copy still holds it.
    images: Arc<HashMap<MediaId, Arc<Decoded>>>,
    video: Option<Arc<dyn FrameSource>>,
    /// Decodes the media pool's thumbnails apart from `video`, whose one request per video the
    /// viewer replaces many times a second: a thumbnail's first frame would never be decoded.
    thumbnails: Option<Arc<dyn FrameSource>>,
    /// Decodes the Edit page's source viewer apart from `video`: one decoder serving both would
    /// seek back and forth between the source's frame and the playing timeline's, and each would
    /// lose its picture to the other (the source viewer blinked black while the timeline played).
    source: Option<Arc<dyn FrameSource>>,
    wait_for_video: bool,
    /// How much smaller video pictures are asked for (a proxy); 0 and 1 mean whole.
    reduction: u32,
}

impl MediaCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// A cache whose videos come from `source`.
    pub fn with_video_source(source: Arc<dyn FrameSource>) -> Self {
        Self {
            images: Arc::default(),
            video: Some(source),
            thumbnails: None,
            source: None,
            wait_for_video: false,
            reduction: 1,
        }
    }

    /// The same cache, with the source viewer's video pictures coming from `source`.
    pub fn with_source_viewer_source(self, source: Arc<dyn FrameSource>) -> Self {
        Self {
            source: Some(source),
            ..self
        }
    }

    /// The same cache, set up for the source viewer: its video pictures come from its own source
    /// when there is one.
    pub fn for_source_viewer(&self) -> Self {
        match &self.source {
            Some(source) => Self {
                video: Some(Arc::clone(source)),
                ..self.clone()
            },
            None => self.clone(),
        }
    }

    /// Changes when a picture for the source viewer arrives. Without a source of its own it is
    /// the viewer's ([`MediaCache::video_generation`]).
    pub fn source_generation(&self) -> u64 {
        match &self.source {
            Some(source) => source.generation(),
            None => self.video_generation(),
        }
    }

    /// The same cache, with the thumbnails' video pictures coming from `source`.
    pub fn with_thumbnail_source(self, source: Arc<dyn FrameSource>) -> Self {
        Self {
            thumbnails: Some(source),
            ..self
        }
    }

    /// The same cache, set up for the media pool's thumbnails. With a thumbnail source, video
    /// pictures come from it, `THUMBNAIL_REDUCTION` times smaller than the file (a tile is small,
    /// and a 4K picture per video would add up) and without waiting. Without one, the thumbnail
    /// waits for the real picture, as before.
    pub fn for_thumbnails(&self) -> Self {
        match &self.thumbnails {
            Some(source) => Self {
                video: Some(Arc::clone(source)),
                reduction: THUMBNAIL_REDUCTION,
                ..self.clone()
            },
            None => self.blocking(),
        }
    }

    /// Changes when a thumbnail's video picture arrives in the background. It stays 0 without a
    /// thumbnail source: those thumbnails are final when they are made, and the viewer's
    /// pictures arriving many times a second must not remake them.
    pub fn thumbnail_generation(&self) -> u64 {
        self.thumbnails.as_ref().map_or(0, |v| v.generation())
    }

    /// The same cache, but rendering with it waits for each exact video picture. Exports use it.
    pub fn blocking(&self) -> Self {
        Self {
            wait_for_video: true,
            reduction: 1,
            ..self.clone()
        }
    }

    /// The same cache, but video pictures come `reduce` times smaller (a proxy for playback).
    /// Exports and thumbnails use [`MediaCache::blocking`], which is always whole.
    pub fn with_reduction(&self, reduce: u32) -> Self {
        Self {
            reduction: reduce.clamp(1, 8),
            ..self.clone()
        }
    }

    /// The picture of a video at a time.
    pub fn video_frame(&self, media: MediaId, path: &str, time_us: i64) -> VideoFrame {
        match &self.video {
            Some(source) => source.video_frame(
                media,
                path,
                time_us,
                self.reduction.max(1),
                self.wait_for_video,
            ),
            None => VideoFrame::Offline,
        }
    }

    /// Changes when a video picture arrives in the background.
    pub fn video_generation(&self) -> u64 {
        self.video.as_ref().map_or(0, |v| v.generation())
    }

    pub fn insert(&mut self, media: MediaId, image: Decoded) {
        Arc::make_mut(&mut self.images).insert(media, Arc::new(image));
    }

    pub fn get(&self, media: MediaId) -> Option<&Arc<Decoded>> {
        self.images.get(&media)
    }

    pub fn contains(&self, media: MediaId) -> bool {
        self.images.contains_key(&media)
    }

    pub fn remove(&mut self, media: MediaId) {
        if self.images.contains_key(&media) {
            Arc::make_mut(&mut self.images).remove(&media);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn tiny_png() -> Vec<u8> {
        let frame = crate::Frame {
            width: 2,
            height: 1,
            rgba: vec![255, 0, 0, 255, 0, 0, 255, 255],
        };
        crate::encode_png(&frame).unwrap()
    }

    #[test]
    fn decodes_png_and_finds_the_format_from_the_bytes() {
        let d = Decoded::from_bytes(&tiny_png()).unwrap();
        assert_eq!((d.width, d.height), (2, 1));
        assert_eq!(d.rgba, vec![255, 0, 0, 255, 0, 0, 255, 255]);
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(Decoded::from_bytes(b"").is_err());
        assert!(Decoded::from_bytes(b"not an image at all").is_err());
        let mut truncated = tiny_png();
        truncated.truncate(20);
        assert!(Decoded::from_bytes(&truncated).is_err());
    }

    /// Answers every request with a pending picture and a generation of 7, noting the size asked.
    #[derive(Debug)]
    struct Asked(std::sync::Mutex<Vec<(u32, bool)>>);

    impl FrameSource for Asked {
        fn video_frame(&self, _: MediaId, _: &str, _: i64, reduce: u32, wait: bool) -> VideoFrame {
            self.0.lock().unwrap().push((reduce, wait));
            VideoFrame::Pending
        }

        fn generation(&self) -> u64 {
            7
        }
    }

    #[test]
    fn thumbnails_ask_small_pictures_from_their_own_source() {
        let (viewer, thumbs) = (
            Arc::new(Asked(Default::default())),
            Arc::new(Asked(Default::default())),
        );
        let cache =
            MediaCache::with_video_source(viewer.clone()).with_thumbnail_source(thumbs.clone());
        assert_eq!(cache.thumbnail_generation(), 7);
        cache.for_thumbnails().video_frame(MediaId(1), "a.mp4", 0);
        assert_eq!(*thumbs.0.lock().unwrap(), [(THUMBNAIL_REDUCTION, false)]);
        assert!(viewer.0.lock().unwrap().is_empty());
    }

    #[test]
    fn the_source_viewer_decodes_apart_from_the_timeline() {
        let (viewer, source) = (
            Arc::new(Asked(Default::default())),
            Arc::new(Asked(Default::default())),
        );
        let cache = MediaCache::with_video_source(viewer.clone())
            .with_source_viewer_source(source.clone())
            .with_reduction(2);
        // The timeline playing asks the viewer's decoder only.
        cache.video_frame(MediaId(1), "a.mp4", 5_000_000);
        // The source viewer asks its own, at the size it asks for, and never waits.
        cache
            .for_source_viewer()
            .video_frame(MediaId(1), "a.mp4", 0);
        assert_eq!(*viewer.0.lock().unwrap(), [(2, false)]);
        assert_eq!(*source.0.lock().unwrap(), [(2, false)]);
        // Its pictures arriving is what makes it draw again, not the timeline's.
        assert_eq!(cache.source_generation(), 7);
        // Without its own source it shares the viewer's, as before.
        let shared = MediaCache::with_video_source(viewer.clone());
        shared
            .for_source_viewer()
            .video_frame(MediaId(1), "a.mp4", 0);
        assert_eq!(viewer.0.lock().unwrap().len(), 2);
    }

    #[test]
    fn thumbnails_without_their_own_source_wait_for_the_real_picture() {
        let viewer = Arc::new(Asked(Default::default()));
        let cache = MediaCache::with_video_source(viewer.clone());
        // The viewer's pictures arriving must not remake the thumbnails.
        assert_eq!(cache.thumbnail_generation(), 0);
        cache.for_thumbnails().video_frame(MediaId(1), "a.mp4", 0);
        assert_eq!(*viewer.0.lock().unwrap(), [(1, true)]);
    }

    #[test]
    fn cache_stores_and_forgets() {
        let mut c = MediaCache::new();
        let id = MediaId(3);
        assert!(!c.contains(id));
        c.insert(id, Decoded::from_bytes(&tiny_png()).unwrap());
        assert!(c.contains(id) && c.get(id).is_some());
        let shared = c.clone();
        c.remove(id);
        assert!(!c.contains(id) && shared.contains(id));
    }

    #[test]
    fn big_pictures_are_made_to_fit_and_small_ones_kept() {
        let big = Decoded {
            width: 3840,
            height: 2160,
            rgba: vec![200; 3840 * 2160 * 4],
        };
        let small = big.fit_within(1280, 720);
        assert_eq!((small.width, small.height), (1280, 720));
        assert_eq!(small.rgba.len(), 1280 * 720 * 4);
        assert_eq!(small.rgba[0], 200);
        let tall = Decoded {
            width: 1000,
            height: 2000,
            rgba: vec![0; 1000 * 2000 * 4],
        }
        .fit_within(1280, 720);
        assert_eq!((tall.width, tall.height), (360, 720));
        let tiny = Decoded::from_bytes(&tiny_png())
            .unwrap()
            .fit_within(1280, 720);
        assert_eq!((tiny.width, tiny.height), (2, 1));
        // Pixels that do not match the size are left alone rather than read wrongly.
        let odd = Decoded {
            width: 4000,
            height: 4000,
            rgba: vec![0; 8],
        }
        .fit_within(1280, 720);
        assert_eq!(odd.width, 4000);
    }
}
