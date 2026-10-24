//! The timeline widget. It never mutates the project itself: it returns
//! [`Action`]s that the app applies with undo history.

use crate::icons::{self, Icon};
use crate::theme::*;
use crate::workers::WAVE_PER_SECOND;
use egui::{Align2, Color32, CursorIcon, Rect, Sense, Stroke, StrokeKind, TextureHandle, pos2, vec2};
use picasso_core::time::format_timecode;
use picasso_core::{Edge, Id, Project, TrackKind};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub const HEADER_W: f32 = 190.0;
const RULER_H: f32 = 30.0;
const VIDEO_H: f32 = 64.0;
const AUDIO_H: f32 = 52.0;
const EDGE_GRAB: f32 = 7.0;
const SNAP_PX: f32 = 8.0;
const LABEL_H: f32 = 17.0;

/// Payload for dragging a media pool item onto the timeline.
#[derive(Clone, Copy)]
pub struct MediaDrag(pub Id);

pub enum Action {
    Seek(i64),
    Select { id: Id, additive: bool },
    ClearSelection,
    Move { id: Id, delta: i64, track: usize },
    Trim { id: Id, edge: Edge, delta: i64 },
    Blade { id: Id, frame: i64 },
    DropMedia { media: Id, frame: i64, track: usize },
    ToggleTrackEnabled(usize),
    ToggleTrackLocked(usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Blade,
}

#[derive(Clone, Copy, PartialEq)]
enum DragMode {
    Move,
    Trim(Edge),
}

struct Drag {
    clip: Id,
    mode: DragMode,
    grab_frame: i64,
    delta: i64,
    track: usize,
}

pub struct TimelineState {
    pub px_per_frame: f32,
    pub scroll: f32,
    pub tool: Tool,
    pub snapping: bool,
    drag: Option<Drag>,
    scrubbing: bool,
}

impl Default for TimelineState {
    fn default() -> Self {
        Self { px_per_frame: 4.0, scroll: 0.0, tool: Tool::Select, snapping: true, drag: None, scrubbing: false }
    }
}

/// What the timeline draws besides the project.
pub struct Overviews<'a> {
    pub thumbs: &'a HashMap<Id, TextureHandle>,
    pub waves: &'a HashMap<Id, Arc<Vec<f32>>>,
    pub marks: (Option<i64>, Option<i64>),
}

struct Row {
    track: usize,
    rect: Rect,
}

impl TimelineState {
    fn frame_to_x(&self, left: f32, frame: f64) -> f32 {
        left + (frame as f32 - self.scroll) * self.px_per_frame
    }

    fn x_to_frame(&self, left: f32, x: f32) -> f64 {
        ((x - left) / self.px_per_frame + self.scroll) as f64
    }

    pub fn zoom_to_fit(&mut self, frames: i64, width: f32) {
        let usable = (width - HEADER_W - 60.0).max(100.0);
        self.px_per_frame = (usable / frames.max(25) as f32).clamp(0.02, 40.0);
        self.scroll = 0.0;
    }

    pub fn zoom_by(&mut self, factor: f32) {
        self.px_per_frame = (self.px_per_frame * factor).clamp(0.02, 40.0);
    }

    /// Keeps the playhead visible during playback.
    pub fn follow(&mut self, frame: i64, width: f32) {
        let visible = (width - HEADER_W) / self.px_per_frame;
        let f = frame as f32;
        if f < self.scroll || f > self.scroll + visible * 0.95 {
            self.scroll = (f - visible * 0.1).max(0.0);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut egui::Ui,
    area: Rect,
    state: &mut TimelineState,
    project: &Project,
    selection: &HashSet<Id>,
    playhead: i64,
    overviews: &Overviews,
) -> Vec<Action> {
    let mut actions = Vec::new();
    let fps = project.settings.fps;
    let tc0 = project.timeline.start_timecode;
    let area_resp = ui.interact(area, ui.scope_id().with("timeline-area"), Sense::click_and_drag());
    let painter = ui.painter_at(area);
    painter.rect_filled(area, 0.0, TIMELINE_BG);

    let left = area.left() + HEADER_W;
    let ruler = Rect::from_min_max(pos2(left, area.top()), pos2(area.right(), area.top() + RULER_H));
    let header_col = Rect::from_min_max(area.left_top(), pos2(left - 1.0, area.bottom()));
    painter.rect_filled(header_col, 0.0, TRACK_HEADER_BG);
    painter.line_segment([pos2(left - 1.0, area.top()), pos2(left - 1.0, area.bottom())], Stroke::new(2.0, DIVIDER));

    // Big timecode above the track headers.
    painter.text(
        pos2(area.left() + HEADER_W / 2.0, area.top() + RULER_H / 2.0 + 2.0),
        Align2::CENTER_CENTER,
        format_timecode(playhead + tc0, fps),
        semibold(21.0),
        TIMECODE,
    );

    // Rows: video tracks top-down from the highest, then audio tracks.
    let mut rows = Vec::new();
    let mut y = ruler.bottom() + 4.0;
    let tracks = &project.timeline.tracks;
    let mut order: Vec<usize> = (0..tracks.len()).filter(|&i| tracks[i].kind == TrackKind::Video).rev().collect();
    let first_audio = order.len();
    order.extend((0..tracks.len()).filter(|&i| tracks[i].kind == TrackKind::Audio));
    for (n, ti) in order.into_iter().enumerate() {
        if n == first_audio && n > 0 {
            y += 4.0;
        }
        let h = if tracks[ti].kind == TrackKind::Video { VIDEO_H } else { AUDIO_H };
        rows.push(Row { track: ti, rect: Rect::from_min_size(pos2(area.left(), y), vec2(area.width(), h)) });
        y += h + 1.0;
    }

    // Scroll and zoom with the wheel (Ctrl/Cmd zooms around the pointer).
    if area_resp.hovered() {
        let (scroll_delta, zoom, pointer) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.pointer.hover_pos()));
        if zoom != 1.0 {
            let anchor = pointer.map_or(left, |p| p.x.max(left));
            let anchor_frame = state.x_to_frame(left, anchor) as f32;
            state.zoom_by(zoom);
            state.scroll = (anchor_frame - (anchor - left) / state.px_per_frame).max(0.0);
        } else {
            let d = if scroll_delta.x != 0.0 { scroll_delta.x } else { scroll_delta.y };
            state.scroll = (state.scroll - d / state.px_per_frame).max(0.0);
        }
    }

    // Ruler.
    painter.rect_filled(ruler, 0.0, RULER_BG);
    let nominal = fps.nominal().max(1);
    let label_steps = [1, 2, 5, 10, nominal, nominal * 2, nominal * 5, nominal * 10, nominal * 30, nominal * 60, nominal * 300, nominal * 600];
    let step = label_steps.iter().copied().find(|s| *s as f32 * state.px_per_frame >= 110.0).unwrap_or(nominal * 3600);
    let minor = (step / 10).max(1);
    let lane_clip = Rect::from_min_max(pos2(left, area.top()), area.max);
    let lp = painter.with_clip_rect(lane_clip);
    let mut f = (state.scroll as i64 / minor) * minor;
    while state.frame_to_x(left, f as f64) < area.right() {
        let x = state.frame_to_x(left, f as f64);
        let major = f % step == 0;
        lp.line_segment([pos2(x, ruler.bottom() - if major { 10.0 } else { 4.0 }), pos2(x, ruler.bottom())], Stroke::new(1.0, RULER_TICK));
        if major {
            lp.text(pos2(x + 9.0, ruler.top() + 3.0), Align2::LEFT_TOP, format_timecode(f + tc0, fps), regular(11.0), RULER_TEXT);
        }
        f += minor;
    }
    // In/out range shading on the ruler.
    let (mark_in, mark_out) = overviews.marks;
    if mark_in.is_some() || mark_out.is_some() {
        let x0 = state.frame_to_x(left, mark_in.unwrap_or(0) as f64);
        let x1 = mark_out.map_or(area.right(), |o| state.frame_to_x(left, o as f64));
        lp.rect_filled(Rect::from_min_max(pos2(x0, ruler.bottom() - 6.0), pos2(x1, ruler.bottom())), 0.0, Color32::from_rgba_unmultiplied(160, 160, 170, 90));
    }

    // Track headers and lanes.
    let mut counters: HashMap<TrackKind, usize> = HashMap::new();
    for row in &rows {
        let t = &tracks[row.track];
        let n = {
            let c = counters.entry(t.kind).or_insert(0);
            *c += 1;
            *c
        };
        let header = Rect::from_min_max(row.rect.min, pos2(left - 2.0, row.rect.bottom()));
        painter.rect_filled(header, 0.0, TRACK_HEADER_BG);
        painter.line_segment([header.left_bottom(), pos2(area.right(), header.bottom())], Stroke::new(1.0, SEPARATOR));
        let badge_text = format!("{}{}", if t.kind == TrackKind::Video { "V" } else { "A" }, n);
        let badge = Rect::from_min_size(pos2(header.left() + 8.0, header.top() + 7.0), vec2(30.0, 20.0));
        painter.rect_filled(badge, 3.0, Color32::from_rgb(58, 57, 64));
        painter.text(badge.center(), Align2::CENTER_CENTER, &badge_text, semibold(12.0), TEXT);
        let display = if t.name.starts_with(['V', 'A']) && t.name.len() <= 3 {
            format!("{} {}", if t.kind == TrackKind::Video { "Video" } else { "Audio" }, n)
        } else {
            t.name.clone()
        };
        painter.text(pos2(header.left() + 10.0, badge.bottom() + 8.0), Align2::LEFT_TOP, display, regular(13.0), TEXT);
        let count = t.clips.len();
        painter.text(
            pos2(header.left() + 10.0, badge.bottom() + 25.0),
            Align2::LEFT_TOP,
            format!("{count} Clip{}", if count == 1 { "" } else { "s" }),
            regular(11.0),
            TEXT_DIM,
        );
        let icon = |i: usize| Rect::from_center_size(pos2(header.left() + 58.0 + i as f32 * 24.0, badge.center().y), vec2(14.0, 14.0));
        if icons::button(ui, icon(0), Icon::Lock, t.locked, true, "Lock track").clicked() {
            actions.push(Action::ToggleTrackLocked(row.track));
        }
        icons::button(ui, icon(1), Icon::AutoSelect, true, false, "Auto track selector");
        match t.kind {
            TrackKind::Video => {
                if icons::button(ui, icon(2), Icon::Eye, t.enabled, true, "Enable / disable track").clicked() {
                    actions.push(Action::ToggleTrackEnabled(row.track));
                }
            }
            TrackKind::Audio => {
                let solo = icon(2);
                let mute = icon(3);
                painter.text(solo.center(), Align2::CENTER_CENTER, "S", semibold(11.0), ICON_DIM);
                let mresp = ui.interact(mute.expand(3.0), ui.scope_id().with(("mute", row.track)), Sense::click()).on_hover_text("Mute track");
                if !t.enabled {
                    painter.rect_filled(mute.expand(2.0), 2.0, Color32::from_rgb(200, 60, 50));
                }
                painter.text(mute.center(), Align2::CENTER_CENTER, "M", semibold(11.0), if t.enabled { ICON } else { Color32::WHITE });
                if mresp.clicked() {
                    actions.push(Action::ToggleTrackEnabled(row.track));
                }
                painter.text(pos2(header.right() - 10.0, badge.center().y), Align2::RIGHT_CENTER, "2.0", regular(11.0), TEXT_DIM);
            }
        }
        let lane = Rect::from_min_max(pos2(left, row.rect.top()), row.rect.max);
        painter.rect_filled(lane, 0.0, if n % 2 == 0 { TRACK_LANE_ALT } else { TRACK_LANE_BG });
    }

    let pointer = ui.ctx().pointer_interact_pos();
    let pointer_frame = pointer.map(|p| state.x_to_frame(left, p.x).round() as i64);
    let row_at = |y: f32| rows.iter().find(|r| y >= r.rect.top() && y <= r.rect.bottom());

    let dragged_group: HashSet<Id> =
        state.drag.as_ref().map(|d| project.linked_group(d.clip).into_iter().collect()).unwrap_or_default();
    let mut snaps = vec![0, playhead];
    snaps.extend(mark_in);
    snaps.extend(mark_out);
    for c in tracks.iter().flat_map(|t| &t.clips).filter(|c| !dragged_group.contains(&c.id)) {
        snaps.extend([c.start, c.end()]);
    }
    let snapping = state.snapping;
    let ppf = state.px_per_frame;
    let snap = |frame: i64| -> Option<i64> {
        if !snapping {
            return None;
        }
        snaps.iter().copied().filter(|s| ((s - frame) as f32 * ppf).abs() <= SNAP_PX).min_by_key(|s| (s - frame).abs())
    };

    // Clips.
    let mut clip_hit = false;
    for row in &rows {
        let t = &tracks[row.track];
        for clip in &t.clips {
            let x0 = state.frame_to_x(left, clip.start as f64);
            let x1 = state.frame_to_x(left, clip.end() as f64);
            if x1 < left || x0 > area.right() {
                continue;
            }
            let mut rect = Rect::from_min_max(pos2(x0, row.rect.top() + 1.0), pos2(x1.max(x0 + 2.0), row.rect.bottom() - 1.0));
            let mut ghost = false;
            if let Some(d) = &state.drag
                && dragged_group.contains(&clip.id) {
                    ghost = true;
                    match d.mode {
                        DragMode::Move => rect = rect.translate(vec2(d.delta as f32 * ppf, 0.0)),
                        DragMode::Trim(Edge::Start) => rect.min.x += d.delta as f32 * ppf,
                        DragMode::Trim(Edge::End) => rect.max.x += d.delta as f32 * ppf,
                    }
                    if d.clip == clip.id && d.mode == DragMode::Move && d.track != row.track
                        && let Some(target) = rows.iter().find(|r| r.track == d.track) {
                            rect = rect.translate(vec2(0.0, target.rect.top() - row.rect.top()));
                        }
                }
            let off = !clip.enabled || !t.enabled;
            let (body, label) = match t.kind {
                _ if off => (CLIP_DISABLED, Color32::from_rgb(84, 84, 90)),
                TrackKind::Video => (VIDEO_CLIP, VIDEO_CLIP_LABEL),
                TrackKind::Audio => (AUDIO_CLIP, AUDIO_CLIP_LABEL),
            };
            let cp = lp.with_clip_rect(rect.intersect(lane_clip));
            let alpha = if ghost { 0.75 } else { 1.0 };
            cp.rect_filled(rect, 2.0, body.gamma_multiply(alpha));
            let label_rect = match t.kind {
                TrackKind::Video => Rect::from_min_max(pos2(rect.left(), rect.bottom() - LABEL_H), rect.max),
                TrackKind::Audio => Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + LABEL_H)),
            };
            match t.kind {
                TrackKind::Video => {
                    // Poster frames along the clip, like a filmstrip.
                    if let Some(tex) = overviews.thumbs.get(&clip.media_id) {
                        let pic_h = rect.height() - LABEL_H;
                        let size = tex.size_vec2();
                        let pic_w = (pic_h * size.x / size.y.max(1.0)).max(4.0);
                        let tint = Color32::WHITE.gamma_multiply(if off { 0.35 } else { alpha });
                        let mut x = rect.left().max(left - pic_w);
                        while x < rect.right().min(area.right()) {
                            let r = Rect::from_min_size(pos2(x, rect.top()), vec2(pic_w, pic_h));
                            cp.image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
                            x += pic_w;
                        }
                    }
                }
                TrackKind::Audio => {
                    if let Some(wave) = overviews.waves.get(&clip.media_id) {
                        let body_r = Rect::from_min_max(pos2(rect.left(), rect.top() + LABEL_H), rect.max);
                        let mid = body_r.center().y;
                        let half = body_r.height() / 2.0 - 2.0;
                        let color = if off { Color32::from_gray(120) } else { AUDIO_WAVE };
                        let per_px = (WAVE_PER_SECOND as f64 / fps.as_f64() / ppf as f64).ceil().max(1.0) as usize;
                        let mut x = body_r.left().max(left);
                        while x < body_r.right().min(area.right()) {
                            let frame = state.x_to_frame(left, x);
                            let secs = (clip.source_in as f64 + frame - clip.start as f64) / fps.as_f64();
                            let i = (secs.max(0.0) * WAVE_PER_SECOND as f64) as usize;
                            let peak = wave.get(i..(i + per_px).min(wave.len())).map_or(0.0, |s| s.iter().fold(0f32, |m, v| m.max(*v)));
                            let h = (peak * clip.volume).min(1.0) * half;
                            if h > 0.3 {
                                cp.line_segment([pos2(x, mid - h), pos2(x, mid + h)], Stroke::new(1.0, color));
                            }
                            x += 1.0;
                        }
                    }
                }
            }
            cp.rect_filled(label_rect, 0.0, label.gamma_multiply(alpha));
            let mut tx = label_rect.left().max(left) + 5.0;
            if clip.link.is_some() {
                icons::paint(&cp, Rect::from_min_size(pos2(tx, label_rect.center().y - 6.0), vec2(12.0, 12.0)), Icon::Chain, Color32::WHITE);
                tx += 15.0;
            }
            cp.text(pos2(tx, label_rect.center().y), Align2::LEFT_CENTER, &clip.name, regular(11.5), Color32::WHITE);
            let selected = selection.contains(&clip.id);
            cp.rect_stroke(
                rect,
                2.0,
                Stroke::new(if selected { 2.0 } else { 1.0 }, if selected { SELECT_ORANGE } else { Color32::from_black_alpha(150) }),
                StrokeKind::Inside,
            );

            // Interaction (on the clip's real position, not the ghost).
            let hit = Rect::from_min_max(pos2(x0.max(left), row.rect.top() + 1.0), pos2(x1.min(area.right()), row.rect.bottom() - 1.0));
            if hit.width() < 1.0 {
                continue;
            }
            let resp = ui.interact(hit, ui.scope_id().with(("clip", clip.id)), Sense::click_and_drag());
            let near = |px: f32| pointer.is_some_and(|p| (p.x - px).abs() <= EDGE_GRAB);
            let edge = if state.tool != Tool::Select || hit.width() <= EDGE_GRAB * 3.0 {
                None
            } else if near(x0) {
                Some(Edge::Start)
            } else if near(x1) {
                Some(Edge::End)
            } else {
                None
            };
            let resp = if t.locked {
                resp
            } else if state.tool == Tool::Blade {
                resp.on_hover_cursor(CursorIcon::Crosshair)
            } else if edge.is_some() {
                resp.on_hover_cursor(CursorIcon::ResizeHorizontal)
            } else {
                resp
            };
            if resp.contains_pointer() {
                clip_hit = true;
                if state.tool == Tool::Blade && !t.locked
                    && let Some(pf) = pointer_frame {
                        let x = state.frame_to_x(left, snap(pf).unwrap_or(pf) as f64);
                        lp.line_segment([pos2(x, row.rect.top()), pos2(x, row.rect.bottom())], Stroke::new(1.0, Color32::WHITE));
                    }
            }
            if resp.clicked() {
                if state.tool == Tool::Blade && !t.locked {
                    if let Some(pf) = pointer_frame {
                        actions.push(Action::Blade { id: clip.id, frame: snap(pf).unwrap_or(pf) });
                    }
                } else {
                    let additive = ui.input(|i| i.modifiers.command || i.modifiers.shift);
                    actions.push(Action::Select { id: clip.id, additive });
                }
            }
            if resp.drag_started() && !t.locked && state.tool == Tool::Select {
                if !selection.contains(&clip.id) {
                    actions.push(Action::Select { id: clip.id, additive: false });
                }
                state.drag = Some(Drag {
                    clip: clip.id,
                    mode: edge.map_or(DragMode::Move, DragMode::Trim),
                    grab_frame: pointer_frame.unwrap_or(clip.start),
                    delta: 0,
                    track: row.track,
                });
            }
        }
    }

    // Update / finish a drag.
    if let Some(d) = state.drag.as_mut() {
        if let (Some(pf), Some(clip)) = (pointer_frame, project.timeline.clip(d.clip)) {
            let mut delta = pf - d.grab_frame;
            let edge_frames: Vec<i64> = match d.mode {
                DragMode::Move => vec![clip.start, clip.end()],
                DragMode::Trim(Edge::Start) => vec![clip.start],
                DragMode::Trim(Edge::End) => vec![clip.end()],
            };
            if !ui.input(|i| i.modifiers.alt)
                && let Some((e, s)) = edge_frames.iter().filter_map(|e| snap(e + delta).map(|s| (*e, s))).min_by_key(|(e, s)| (e + delta - s).abs()) {
                    delta = s - e;
                }
            d.delta = delta;
            if d.mode == DragMode::Move {
                let kind = tracks[project.timeline.find_clip(d.clip).map_or(0, |(t, _)| t)].kind;
                if let Some(r) = pointer.and_then(|p| row_at(p.y)).filter(|r| tracks[r.track].kind == kind) {
                    d.track = r.track;
                }
            }
        }
        if ui.input(|i| i.pointer.any_released()) {
            let d = state.drag.take().expect("checked above");
            match d.mode {
                DragMode::Move => {
                    let src = project.timeline.find_clip(d.clip).map(|(t, _)| t);
                    if d.delta != 0 || src != Some(d.track) {
                        actions.push(Action::Move { id: d.clip, delta: d.delta, track: d.track });
                    }
                }
                DragMode::Trim(edge) if d.delta != 0 => actions.push(Action::Trim { id: d.clip, edge, delta: d.delta }),
                DragMode::Trim(_) => {}
            }
        }
    }

    // Ruler scrubbing and empty-space clicks.
    let in_ruler = pointer.is_some_and(|p| ruler.contains(p));
    if area_resp.drag_started() && in_ruler {
        state.scrubbing = true;
    }
    if state.scrubbing {
        if let Some(pf) = pointer_frame {
            actions.push(Action::Seek(pf.max(0)));
        }
        if !area_resp.dragged() {
            state.scrubbing = false;
        }
    }
    if area_resp.clicked() && !clip_hit {
        if let Some(pf) = pointer_frame.filter(|_| pointer.is_some_and(|p| p.x >= left)) {
            actions.push(Action::Seek(pf.max(0)));
        }
        if !in_ruler {
            actions.push(Action::ClearSelection);
        }
    }

    // Media dropped from the pool.
    if area_resp.dnd_hover_payload::<MediaDrag>().is_some()
        && let (Some(p), Some(pf)) = (pointer, pointer_frame)
            && let Some(r) = row_at(p.y) {
                let x = state.frame_to_x(left, snap(pf).unwrap_or(pf) as f64);
                lp.rect_filled(Rect::from_min_size(pos2(x - 1.0, r.rect.top()), vec2(3.0, r.rect.height())), 0.0, SELECT_ORANGE);
            }
    if let Some(payload) = area_resp.dnd_release_payload::<MediaDrag>()
        && let (Some(p), Some(pf)) = (pointer, pointer_frame) {
            let track = row_at(p.y).map_or(usize::MAX, |r| r.track);
            actions.push(Action::DropMedia { media: payload.0, frame: snap(pf).unwrap_or(pf).max(0), track });
        }

    // Playhead: a red line with a flag on the ruler.
    let px = state.frame_to_x(left, playhead as f64);
    if px >= left && px <= area.right() {
        lp.line_segment([pos2(px, ruler.top()), pos2(px, area.bottom())], Stroke::new(1.5, PLAYHEAD));
        lp.add(egui::Shape::convex_polygon(
            vec![pos2(px - 7.0, ruler.top()), pos2(px + 7.0, ruler.top()), pos2(px + 7.0, ruler.top() + 8.0), pos2(px, ruler.top() + 15.0), pos2(px - 7.0, ruler.top() + 8.0)],
            PLAYHEAD,
            Stroke::NONE,
        ));
    }
    actions
}
