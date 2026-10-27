//! The source viewer of the Edit page: the media chosen in the pool, played and scrubbed on its
//! own (silently), with Match Frame between it and the timeline.

use egui::{ColorImage, Context, TextureHandle, TextureId, TextureOptions};
use picasso_core::{MediaId, TrackKind};

use crate::app::StudioApp;

/// The longest side of the source viewer's picture.
const SIDE: u32 = 640;

#[derive(Default)]
pub(crate) struct SourceViewer {
    /// The media shown, and the frame of it.
    pub media: Option<MediaId>,
    pub frame: u64,
    pub playing: bool,
    pub looping: bool,
    /// Frames to move, carried between repaints.
    accum: f32,
    texture: Option<TextureHandle>,
    /// What the picture was made from: media, frame, picture generation.
    key: Option<(MediaId, u64, u64)>,
}

impl StudioApp {
    /// How many frames the source has (a still or generator: its default length).
    pub(crate) fn source_length(&self) -> u64 {
        self.source
            .media
            .and_then(|m| self.project().media(m))
            .map_or(0, |m| m.length.unwrap_or(m.default_duration))
            .max(1)
    }

    /// Follows the pool's choice; plays on while playing.
    pub(crate) fn tick_source(&mut self, ctx: &Context) {
        if self.selected_media != self.source.media {
            self.source.media = self.selected_media;
            self.source.frame = 0;
            self.source.playing = false;
        }
        if !self.source.playing {
            return;
        }
        let fps = self.project().fps;
        let rate = fps.num as f32 / fps.den.max(1) as f32;
        self.source.accum += ctx.input(|i| i.stable_dt).min(0.1) * rate;
        let whole = self.source.accum.floor();
        if whole >= 1.0 {
            self.source.accum -= whole;
            let length = self.source_length();
            let next = self.source.frame + whole as u64;
            if next >= length {
                if self.source.looping {
                    self.source.frame = 0;
                } else {
                    self.source.frame = length - 1;
                    self.source.playing = false;
                }
            } else {
                self.source.frame = next;
            }
        }
        ctx.request_repaint();
    }

    /// Moves the source to `frame`, stopping it.
    pub(crate) fn source_go_to(&mut self, frame: u64) {
        self.source.playing = false;
        self.source.frame = frame.min(self.source_length() - 1);
    }

    pub(crate) fn source_toggle_play(&mut self) {
        if self.source.media.is_none() {
            self.fail("choose a clip in the media pool first");
            return;
        }
        if !self.source.playing && self.source.frame + 1 >= self.source_length() {
            self.source.frame = 0;
        }
        self.source.playing = !self.source.playing;
        self.source.accum = 0.0;
    }

    /// The source viewer's picture, made again when the frame or the media changed, or when a
    /// picture of its own arrives. Its videos are decoded apart from the timeline's
    /// ([`picasso_render::MediaCache::for_source_viewer`]), so a playing timeline neither takes
    /// its picture away nor makes it draw again on every frame.
    pub(crate) fn source_texture(&mut self, ctx: &Context) -> Option<TextureId> {
        let media = self.source.media?;
        let key = (media, self.source.frame, self.cache.source_generation());
        if self.source.key == Some(key) {
            return self.source.texture.as_ref().map(TextureHandle::id);
        }
        let item = self.project().media(media)?.clone();
        let (pw, ph) = (self.project().width.max(1), self.project().height.max(1));
        let scale = SIDE as f32 / pw.max(ph) as f32;
        let (w, h) = (
            ((pw as f32 * scale) as u32).max(16),
            ((ph as f32 * scale) as u32).max(16),
        );
        let frame = picasso_render::render_media_frame(
            self.project(),
            &item,
            &self.cache.for_source_viewer(),
            self.source.frame,
            w,
            h,
        )
        .ok()?;
        let image = ColorImage::from_rgba_unmultiplied(
            [frame.width as usize, frame.height as usize],
            &frame.rgba,
        );
        match &mut self.source.texture {
            Some(t) => t.set(image, TextureOptions::LINEAR),
            None => {
                self.source.texture =
                    Some(ctx.load_texture("source_viewer", image, TextureOptions::LINEAR));
            }
        }
        self.source.key = Some(key);
        self.source.texture.as_ref().map(TextureHandle::id)
    }

    /// Match Frame from the timeline: the source viewer shows the media of the clip at the
    /// playhead, at the frame the timeline shows.
    pub(crate) fn match_frame_to_source(&mut self) {
        let at = self.playhead;
        let project = self.project();
        let clip = self
            .current_clip()
            .and_then(|id| project.clip(id))
            .map(|(_, c)| c)
            .or_else(|| {
                project
                    .tracks
                    .iter()
                    .filter(|t| t.kind == TrackKind::Audio)
                    .find_map(|t| picasso_core::Project::clip_at(t, at))
            })
            .map(|c| (c.media, c.source_frame_at(at)));
        let Some((media, frame)) = clip else {
            self.fail("there is no clip at the playhead");
            return;
        };
        self.selected_media = Some(media);
        self.source.media = Some(media);
        self.source_go_to(frame);
        self.page = crate::app::Page::Edit;
    }

    /// Match Frame from the source: the playhead goes to where the timeline shows the source's
    /// frame, in the first clip of that media that holds it.
    pub(crate) fn match_frame_to_timeline(&mut self) {
        let Some(media) = self.source.media else {
            self.fail("choose a clip in the media pool first");
            return;
        };
        let wanted = self.source.frame;
        let found = self
            .project()
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .filter(|c| c.media == media && !c.freeze && !c.reverse)
            .find_map(|c| {
                let span = c.source_span();
                (wanted >= c.source_in && wanted < c.source_in + span).then(|| {
                    // From the source frame back to the timeline frame, at the clip's speed.
                    let along = ((wanted - c.source_in) as f64 / f64::from(c.speed.max(0.01)))
                        .floor() as u64;
                    (c.start + along.min(c.duration.saturating_sub(1)), c.id)
                })
            });
        match found {
            Some((frame, clip)) => {
                self.go_to(frame);
                self.select_only(clip);
            }
            None => self.fail("that frame of the clip is not on the timeline"),
        }
    }
}
