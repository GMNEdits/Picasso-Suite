//! The Deliver page's clips: the strip of the timeline's video clips under the viewer (number,
//! track, picture and codec), and the tracks of its timeline with their headers.

use egui::{pos2, vec2, Align2, Color32, Context, Rect, Sense, Stroke, Ui};
use picasso_core::{ClipId, MediaId, MediaSource, TrackKind};

use crate::app::StudioApp;
use crate::chrome::Screen;
use crate::icons::{paint, Icon};
#[cfg(not(target_arch = "wasm32"))]
use crate::platform::Incoming;
use crate::theme::{self, regular, semibold};

const TEXT: Color32 = Color32::from_rgb(244, 240, 236);
const DIM: Color32 = Color32::from_rgb(150, 150, 156);
const LINE: Color32 = Color32::from_rgb(14, 14, 18);
const RED: Color32 = Color32::from_rgb(226, 60, 50);

/// The size of a picture in the strip, and the room each clip takes.
const THUMB: egui::Vec2 = vec2(145.0, 80.0);
const STEP: f32 = 162.0;

/// A video clip of the strip: its number is its place in time.
pub(crate) struct StripClip {
    pub(crate) id: ClipId,
    pub(crate) media: MediaId,
    pub(crate) track: String,
    pub(crate) start: u64,
    pub(crate) end: u64,
}

impl StudioApp {
    /// The video clips of the timeline in time order, as Deliver shows them.
    pub(crate) fn strip_clips(&self) -> Vec<StripClip> {
        let project = self.project();
        let mut clips: Vec<StripClip> = project
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Video)
            .flat_map(|t| {
                t.clips.iter().map(|c| StripClip {
                    id: c.id,
                    media: c.media,
                    track: t.name.clone(),
                    start: c.start,
                    end: c.end(),
                })
            })
            .collect();
        clips.sort_by_key(|c| (c.start, c.end));
        clips
    }

    /// The codec of a media item as editors write it ("H.264 High L4.0"); for a picture made
    /// here, what it is. Read once and remembered; a video's is read from its file in the
    /// background (empty until then), since opening files one after another held up the first
    /// frame of a page showing many clips.
    pub(crate) fn codec_label(&mut self, media: MediaId) -> String {
        if let Some(label) = self.codec_labels.get(&media) {
            return label.clone();
        }
        let label = match self.project().media(media).map(|m| m.source.clone()) {
            Some(MediaSource::Video { path }) => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.codec_wanted.push((media, path));
                    String::new()
                }
                #[cfg(target_arch = "wasm32")]
                {
                    let _ = path;
                    "Video".to_string()
                }
            }
            Some(MediaSource::Image { path }) => std::path::Path::new(&path)
                .extension()
                .map_or("Still".into(), |e| e.to_string_lossy().to_uppercase()),
            Some(MediaSource::Audio { .. }) => "Audio".into(),
            Some(MediaSource::Title(_)) => "Text".into(),
            Some(MediaSource::Compound { .. }) => "Compound Clip".into(),
            Some(_) => "Generator".into(),
            None => String::new(),
        };
        self.codec_labels.insert(media, label.clone());
        label
    }

    /// Reads the codecs `codec_label` asked for, on a thread of their own; they arrive through
    /// the inbox.
    pub(crate) fn read_wanted_codecs(&mut self) {
        if self.codec_wanted.is_empty() {
            return;
        }
        let wanted = std::mem::take(&mut self.codec_wanted);
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (inbox, ctx) = (self.inbox.clone(), self.ctx.clone());
            std::thread::spawn(move || {
                for (media, path) in wanted {
                    let label = picasso_decode::probe(std::path::Path::new(&path))
                        .map(|i| i.codec_label())
                        .unwrap_or_else(|_| "Media offline".into());
                    inbox.push(&ctx, Incoming::CodecLabel { media, label });
                }
            });
        }
        #[cfg(target_arch = "wasm32")]
        drop(wanted);
    }

    /// The strip under the viewer: each video clip with its number and track over its picture,
    /// the one at the playhead outlined in red, and its codec under it. A click goes to it.
    pub(crate) fn clip_strip(&mut self, ui: &mut Ui, ctx: &Context, s: &Screen, rect: Rect) {
        let clips = self.strip_clips();
        let painter = ui.painter_at(rect);
        let at = self.playhead;
        // The clip seen at the playhead: the topmost video clip there.
        let current = self
            .project()
            .tracks
            .iter()
            .rev()
            .filter(|t| t.kind == TrackKind::Video && !t.muted)
            .find_map(|t| picasso_core::Project::clip_at(t, at))
            .map(|c| c.id);
        let mut go = None;
        // A shorter band shows smaller pictures, so the codec under them still fits.
        let fit = ((rect.height() - 10.0 - 28.0 - 30.0) / THUMB.y).clamp(0.5, 1.0);
        let (thumb, step) = (THUMB * fit, STEP - THUMB.x * (1.0 - fit));
        for (n, clip) in clips.iter().enumerate() {
            let x = rect.left() + 12.0 + step * n as f32;
            if x > rect.right() {
                break;
            }
            let top = rect.top() + 10.0;
            // The number in a box, the track on the right.
            let number = format!("{:02}", n + 1);
            let label = Rect::from_min_size(pos2(x, top), vec2(34.0, 24.0));
            painter.rect_filled(label, 2.0, Color32::from_rgb(20, 20, 24));
            painter.rect_stroke(
                label,
                2.0,
                Stroke::new(1.0, Color32::from_rgb(70, 70, 76)),
                egui::StrokeKind::Inside,
            );
            painter.text(
                label.center(),
                Align2::CENTER_CENTER,
                &number,
                semibold(16.0),
                TEXT,
            );
            painter.text(
                pos2(x + thumb.x - 4.0, label.center().y),
                Align2::RIGHT_CENTER,
                &clip.track,
                regular(16.0),
                DIM,
            );
            let pic = Rect::from_min_size(pos2(x, label.bottom() + 4.0), thumb);
            painter.rect_filled(pic, 3.0, Color32::from_rgb(12, 12, 14));
            if let Some(tex) = self.media_thumb(ctx, clip.media) {
                painter.image(
                    tex,
                    pic.shrink(2.0),
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            let (stroke, width) = if current == Some(clip.id) {
                (RED, 2.5)
            } else {
                (Color32::from_rgb(54, 54, 60), 1.0)
            };
            painter.rect_stroke(
                pic,
                3.0,
                Stroke::new(width, stroke),
                egui::StrokeKind::Inside,
            );
            let codec = self.codec_label(clip.media);
            painter.text(
                pos2(x, pic.bottom() + 16.0),
                Align2::LEFT_CENTER,
                codec,
                regular(17.0),
                TEXT,
            );
            let hit = Rect::from_min_max(label.min, pic.max);
            if ui
                .interact(hit, ui.id().with(("strip", clip.id.0)), Sense::click())
                .on_hover_text(format!(
                    "{}  {}",
                    self.tc(clip.start as i64),
                    self.tc(clip.end as i64)
                ))
                .clicked()
            {
                go = Some((clip.id, clip.start));
            }
        }
        if let Some((id, start)) = go {
            self.playing = false;
            self.go_to(start);
            self.select_only(id);
        }
        let _ = s;
    }

    /// The tracks of the Deliver timeline: a header for each (its name, and on/off for video,
    /// solo and mute for sound) and its clips, video tracks from the highest down, then sound.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn deliver_tracks(
        &mut self,
        ui: &mut Ui,
        ctx: &Context,
        header: Rect,
        area: Rect,
        first: f32,
        per_frame: f32,
        top: f32,
    ) {
        let thumbs = self.timeline_thumbs(ctx);
        // Borrowed, not copied: copying the whole project on every frame was the cost of this page.
        let project = self.project();
        let videos: Vec<usize> = (0..project.tracks.len())
            .rev()
            .filter(|i| project.tracks[*i].kind == TrackKind::Video)
            .collect();
        let audios: Vec<usize> = (0..project.tracks.len())
            .filter(|i| project.tracks[*i].kind == TrackKind::Audio)
            .collect();
        let rows: Vec<usize> = videos.into_iter().chain(audios).collect();
        // Tall rows like the reference while they fit.
        let room = (area.bottom() - 16.0 - top).max(40.0);
        let wanted: f32 = rows
            .iter()
            .map(|i| {
                if project.tracks[*i].kind == TrackKind::Video {
                    75.0
                } else {
                    62.0
                }
            })
            .sum();
        let scale = (room / wanted.max(1.0)).clamp(0.45, 1.0);
        let clipped = ui.painter().with_clip_rect(area);
        let heads = ui.painter().with_clip_rect(Rect::from_min_max(
            header.min,
            pos2(header.right(), area.bottom()),
        ));
        let x_of = |frame: f32| area.left() + (frame - first) * per_frame;
        let mut y = top;
        let mut toggles: Vec<(usize, u8)> = Vec::new();
        for (n, i) in rows.iter().enumerate() {
            let track = &project.tracks[*i];
            let video = track.kind == TrackKind::Video;
            let h = if video { 75.0 } else { 62.0 } * scale;
            // As much smaller as the dividers made the headers narrower than their usual 380.
            let z = (header.width() / 380.0).min(1.0);
            if y > area.bottom() - 16.0 {
                break;
            }
            let row = Rect::from_min_max(pos2(header.left(), y), pos2(area.right(), y + h));
            // The header.
            let head = Rect::from_min_max(row.min, pos2(header.right(), row.bottom()));
            heads.rect_filled(head, 0.0, Color32::from_rgb(30, 30, 34));
            heads.rect_filled(
                Rect::from_min_size(head.min + vec2(2.0, 0.0), vec2(3.0, h - 1.0)),
                0.0,
                if video {
                    Color32::from_rgb(64, 120, 186)
                } else {
                    Color32::from_rgb(58, 150, 98)
                },
            );
            heads.text(
                pos2(head.left() + 47.0 * z, head.center().y),
                Align2::CENTER_CENTER,
                &track.name,
                semibold(17.0 * z),
                TEXT,
            );
            heads.vline(
                head.left() + 75.0 * z,
                head.top()..=head.bottom(),
                Stroke::new(1.0, LINE),
            );
            if video {
                let on = !track.muted;
                let r = Rect::from_center_size(
                    pos2(head.left() + 98.0 * z, head.center().y),
                    vec2(30.0 * z, 24.0 * z),
                );
                paint(
                    &heads,
                    r.center(),
                    Icon::FilmView,
                    if on {
                        theme::ICON
                    } else {
                        Color32::from_rgb(80, 80, 86)
                    },
                );
                if ui
                    .interact(r, ui.id().with(("dl_vid", n)), Sense::click())
                    .on_hover_text(if on {
                        "Turn the track off"
                    } else {
                        "Turn the track on"
                    })
                    .clicked()
                {
                    toggles.push((*i, 0));
                }
            } else {
                let soloed = self.monitor.solo.contains(&track.id);
                for (k, (letter, lit, hint)) in [
                    ("S", soloed, "Solo: hear only soloed tracks"),
                    ("M", track.muted, "Mute"),
                ]
                .into_iter()
                .enumerate()
                {
                    let r = Rect::from_center_size(
                        pos2(head.left() + (98.0 + 40.0 * k as f32) * z, head.center().y),
                        vec2(22.0 * z, 22.0 * z),
                    );
                    let fill = match (letter, lit) {
                        ("S", true) => Color32::from_rgb(214, 170, 40),
                        ("M", true) => Color32::from_rgb(200, 70, 56),
                        _ => Color32::from_rgb(64, 64, 70),
                    };
                    heads.rect_filled(r, 3.0, fill);
                    heads.text(
                        r.center(),
                        Align2::CENTER_CENTER,
                        letter,
                        regular(13.0 * z),
                        if lit { Color32::BLACK } else { DIM },
                    );
                    if ui
                        .interact(r, ui.id().with(("dl_sm", n, k)), Sense::click())
                        .on_hover_text(hint)
                        .clicked()
                    {
                        toggles.push((*i, 1 + k as u8));
                    }
                }
                heads.text(
                    pos2(head.right() - 18.0 * z, head.center().y),
                    Align2::RIGHT_CENTER,
                    "2.0",
                    regular(15.0 * z),
                    DIM,
                );
            }
            // The clips.
            for c in &track.clips {
                let rect = Rect::from_min_max(
                    pos2(x_of(c.start as f32), row.top() + 2.0),
                    pos2(x_of(c.end() as f32), row.bottom() - 2.0),
                );
                if rect.right() < area.left() || rect.left() > area.right() {
                    continue;
                }
                let thumb = thumbs.get(&c.media).copied();
                self.paint_edit_clip(&clipped, project, rect, c.id, track.muted, thumb);
            }
            clipped.hline(
                area.left()..=area.right(),
                row.bottom() + 1.0,
                Stroke::new(1.0, Color32::from_rgb(40, 40, 46)),
            );
            y += h + 4.0;
        }
        for (i, what) in toggles {
            let Some(track) = self.project().tracks.get(i).map(|t| (t.id, t.muted)) else {
                continue;
            };
            match what {
                1 => {
                    let id = track.0;
                    self.set_monitor(|m| {
                        if !m.solo.remove(&id) {
                            m.solo.insert(id);
                        }
                    });
                }
                _ => {
                    self.run(picasso_core::Command::SetTrackMuted {
                        track: track.0,
                        muted: !track.1,
                    });
                }
            }
        }
    }
}
