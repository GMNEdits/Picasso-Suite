//! The timeline: tracks of clips with a ruler and a playhead.
//!
//! One widget serves three places (see [`Mode`]): the Edit page's timeline, the Cut page's
//! detail timeline (the playhead stays in the middle and the clips move under it) and the
//! Cut page's overview strip (the whole timeline at once).
//!
//! Clips are moved and trimmed by dragging. While a drag is going on only a preview is drawn;
//! the command is run when the pointer is released, so a drag is one undo step and a refused edit
//! (an overlap) changes nothing.

use egui::{
    pos2, vec2, Align2, Color32, Context, CursorIcon, Pos2, Rect, Sense, Shape, Stroke, StrokeKind,
    Ui,
};
use picasso_core::{
    Clip, ClipId, Command, MediaId, MediaSource, Outcome, Project, TrackId, TrackKind,
};

use crate::app::StudioApp;
use crate::theme::{self, regular};

const EDGE_PX: f32 = 7.0;
const SNAP_PX: f32 = 8.0;
const MIN_ZOOM: f32 = 0.02;
const MAX_ZOOM: f32 = 60.0;
/// Pixels per frame of the detail timeline when it is first shown (240 px per second at 24 fps).
const DETAIL_ZOOM: f32 = 10.0;
/// The height of a sound strip under the picture on the Cut page's detail timeline.
const CUT_AUDIO_H: f32 = 19.5;
/// The space under the Cut page's detail tracks.
const CUT_BOTTOM_GAP: f32 = 24.5;
/// The blue of picture bars and sound strips on the Cut page.
const CUT_BLUE: Color32 = Color32::from_rgb(67, 119, 161);
/// The Cut page's timeline background, and the darker time before the timeline starts.
const CUT_BG: Color32 = Color32::from_rgb(41, 40, 47);
const CUT_BEFORE: Color32 = Color32::from_rgb(30, 29, 34);
/// The overview shows at least this many seconds, so a short timeline does not fill it.
const OVERVIEW_MIN_SECONDS: f32 = 43.08;
/// Space left of the first frame in the overview.
const OVERVIEW_PAD: f32 = 20.0;
/// The Edit page's column left of the tracks: the timecode over the track headers.
/// The track headers of the Edit page: 260 pixels of Resolve at 1080p.
pub(crate) const EDIT_HEADER_W: f32 = 260.0 * crate::chrome::R;
/// The Edit page's scroll bar under the tracks.
const SCROLLBAR_H: f32 = 14.0;
/// Pixels per frame of the Edit timeline when it is first shown (300 px per 8 s at 24 fps).
const EDIT_ZOOM: f32 = 1.46;
/// Pixels between two ruler labels, at least.
const LABEL_SPACING: f32 = 330.0;
/// Pixels between two small ticks, at least.
const TICK_SPACING: f32 = 8.5;

/// Where a timeline is shown, which sets how it looks and behaves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Mode {
    /// The Edit page: toolbar, track headers, free zoom and scroll.
    #[default]
    Edit,
    /// The Cut page's lower timeline: the playhead is fixed in the middle.
    Detail,
    /// The Cut page's upper strip: all of the timeline, clips as thin bars.
    Overview,
}

impl Mode {
    fn ruler_h(self) -> f32 {
        match self {
            Mode::Edit => 40.0 * crate::chrome::R,
            Mode::Detail => 37.0,
            Mode::Overview => 22.0,
        }
    }

    fn row_h(self) -> f32 {
        match self {
            Mode::Edit => 112.0 * crate::chrome::R,
            Mode::Detail => 76.0,
            Mode::Overview => 25.0,
        }
    }
}

/// What a drag on the timeline is doing.
#[derive(Clone, Debug, Default)]
enum Drag {
    #[default]
    None,
    /// Moving the playhead; `scroll` is where the view was when it started.
    Scrub {
        scroll: f32,
    },
    /// `track` is an index; `ok` says whether releasing here would work.
    Move {
        clip: ClipId,
        grab: i64,
        track: usize,
        start: u64,
        ok: bool,
    },
    TrimStart {
        clip: ClipId,
        to: u64,
        ok: bool,
    },
    TrimEnd {
        clip: ClipId,
        to: u64,
        ok: bool,
    },
    /// Alt + dragging the cut between two neighbouring clips: one gets shorter, the other longer.
    Roll {
        left: ClipId,
        right: ClipId,
        to: u64,
        ok: bool,
    },
}

pub(crate) struct TimelineView {
    mode: Mode,
    /// Pixels per frame.
    zoom: f32,
    /// The frame at the left edge.
    scroll: f32,
    snap: bool,
    drag: Drag,
    /// The recent answers of [`StudioApp::would_work`], for the project they were asked of: its
    /// `epoch` (a new one for each project opened) and revision.
    would: ((u64, u64), Vec<(Command, bool)>),
}

/// How many answers of `would_work` a drag remembers.
const WOULD_KEPT: usize = 32;

impl TimelineView {
    pub(crate) fn new(mode: Mode) -> Self {
        Self {
            mode,
            zoom: match mode {
                Mode::Detail => DETAIL_ZOOM,
                Mode::Edit => EDIT_ZOOM,
                Mode::Overview => 3.0,
            },
            scroll: 0.0,
            snap: true,
            drag: Drag::None,
            would: ((0, 0), Vec::new()),
        }
    }
}

impl TimelineView {
    pub(crate) fn snap(&self) -> bool {
        self.snap
    }

    pub(crate) fn set_snap(&mut self, on: bool) {
        self.snap = on;
    }

    /// The zoom as 0 (all the way out) to 1 (all the way in), for a slider.
    pub(crate) fn zoom_level(&self) -> f32 {
        (self.zoom.ln() - MIN_ZOOM.ln()) / (MAX_ZOOM.ln() - MIN_ZOOM.ln())
    }

    pub(crate) fn set_zoom_level(&mut self, level: f32) {
        let level = level.clamp(0.0, 1.0);
        self.zoom = (MIN_ZOOM.ln() + level * (MAX_ZOOM.ln() - MIN_ZOOM.ln()))
            .exp()
            .clamp(MIN_ZOOM, MAX_ZOOM);
    }

    /// Shows `frames` frames in `width` points, from the start.
    pub(crate) fn fit(&mut self, frames: u64, width: f32) {
        self.zoom = (width / (frames.max(1) as f32 * 1.1)).clamp(MIN_ZOOM, MAX_ZOOM);
        self.scroll = 0.0;
    }
}

impl Default for TimelineView {
    fn default() -> Self {
        Self::new(Mode::Edit)
    }
}

/// Which of the app's timeline views to show.
pub(crate) type Which = Mode;

/// Where things are on screen.
#[derive(Clone, Copy)]
struct Geo {
    /// Left edge of the clip area.
    left: f32,
    top: f32,
    zoom: f32,
    scroll: f32,
    tracks: usize,
    ruler_h: f32,
    row_h: f32,
    /// The track shown in each row, top to bottom: the video tracks from the highest down to V1,
    /// then the audio tracks from A1 down, as in the reference.
    order: [u8; MAX_ROWS],
    /// The row of each track (the inverse of `order`).
    row_of: [u8; MAX_ROWS],
    /// How many video tracks there are: the first audio row comes after them.
    videos: usize,
    /// The top of the first row (under the ruler, or lower on the Cut page's detail, whose tracks
    /// sit at the bottom as in Resolve).
    body_top: f32,
    /// The height of the audio rows: `row_h` except on the Cut page, where sound is a thin strip
    /// under the picture (and hidden in the overview).
    audio_h: f32,
}

/// A project has at most 64 tracks (picasso-core refuses more).
const MAX_ROWS: usize = 64;

/// The row of a track that is not shown.
const HIDDEN_ROW: u8 = 250;

/// The rows of `tracks` top to bottom (see [`Geo::order`]), the row of each track, and the
/// number of video tracks.
fn row_order(
    tracks: &[picasso_core::Track],
    audio_only: bool,
) -> ([u8; MAX_ROWS], [u8; MAX_ROWS], usize) {
    let video = tracks
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, t)| t.kind == TrackKind::Video && !audio_only);
    let audio = tracks
        .iter()
        .enumerate()
        .filter(|(_, t)| t.kind == TrackKind::Audio);
    let mut order = [0u8; MAX_ROWS];
    // A track without a row (a video track on the Audio page) is placed far below the rows,
    // where nothing is drawn or clicked.
    let mut row_of = [HIDDEN_ROW; MAX_ROWS];
    for (row, (index, _)) in video.chain(audio).take(MAX_ROWS).enumerate() {
        order[row] = index as u8;
        row_of[index] = row as u8;
    }
    let videos = if audio_only {
        0
    } else {
        tracks.iter().filter(|t| t.kind == TrackKind::Video).count()
    };
    (order, row_of, videos)
}

/// The box over a cut that has a transition: as wide as the transition, centred on the cut,
/// with a line across it.
fn paint_transition(painter: &egui::Painter, project: &Project, rect: Rect, id: ClipId) {
    let Some((track, clip)) = project.clip(id) else {
        return;
    };
    let Some(left) = track
        .clips
        .iter()
        .find(|c| c.end() == clip.start && c.id != id)
    else {
        return;
    };
    let Some(frames) = picasso_core::transition_frames(left, clip) else {
        return;
    };
    let per_frame = rect.width() / clip.duration.max(1) as f32;
    let width = (frames as f32 * per_frame).max(8.0);
    let body = Rect::from_min_max(
        pos2(rect.left() - width / 2.0, rect.top() + 3.0),
        pos2(rect.left() + width / 2.0, rect.top() + rect.height() * 0.62),
    );
    painter.rect_filled(body, 3.0, Color32::from_rgba_unmultiplied(70, 70, 80, 225));
    painter.rect_stroke(
        body,
        3.0,
        Stroke::new(1.0, Color32::from_gray(150)),
        egui::StrokeKind::Inside,
    );
    painter.line_segment(
        [
            body.left_bottom() + vec2(3.0, -3.0),
            body.right_top() + vec2(-3.0, 3.0),
        ],
        Stroke::new(1.0, Color32::from_gray(170)),
    );
}

impl Geo {
    fn x(&self, frame: f32) -> f32 {
        self.left + (frame - self.scroll) * self.zoom
    }

    fn frame(&self, x: f32) -> f32 {
        (x - self.left) / self.zoom + self.scroll
    }

    /// The top of the row of track `track` (an index into the project's tracks).
    /// The top of row `row` (counted from the top).
    fn top_of_row(&self, row: usize) -> f32 {
        let videos = row.min(self.videos);
        self.body_top + videos as f32 * self.row_h + (row - videos) as f32 * self.audio_h
    }

    /// The height of row `row`.
    fn height_of_row(&self, row: usize) -> f32 {
        if row < self.videos {
            self.row_h
        } else {
            self.audio_h
        }
    }

    fn row_top(&self, track: usize) -> f32 {
        let row = self.row_of.get(track).copied().unwrap_or(0);
        self.top_of_row(usize::from(row))
    }

    /// The height of the row of track `track`.
    fn track_h(&self, track: usize) -> f32 {
        let row = self.row_of.get(track).copied().unwrap_or(0);
        self.height_of_row(usize::from(row))
    }

    /// The track (an index into the project's tracks) whose row is at `y`.
    fn track_at(&self, y: f32) -> Option<usize> {
        (0..self.tracks.min(MAX_ROWS))
            .find(|&row| {
                let top = self.top_of_row(row);
                y >= top && y < top + self.height_of_row(row)
            })
            .map(|row| usize::from(self.order[row]))
    }

    /// The track whose row is at `y`, or else the nearest row. The Cut page's tracks sit at the
    /// bottom of the detail timeline, so a clip dropped on the empty space above them still goes
    /// onto a track, as in Resolve.
    fn nearest_track(&self, y: f32) -> Option<usize> {
        self.track_at(y).or_else(|| {
            let distance = |row: usize| {
                let top = self.top_of_row(row);
                (y - (top + self.height_of_row(row) / 2.0)).abs()
            };
            (0..self.tracks.min(MAX_ROWS))
                // Rows past the shown tracks (the Audio page hides the video tracks) are not rows.
                .filter(|&row| usize::from(self.row_of[usize::from(self.order[row])]) == row)
                .min_by(|&a, &b| distance(a).total_cmp(&distance(b)))
                .map(|row| usize::from(self.order[row]))
        })
    }

    /// The top of the first audio row, when there are video rows above it.
    fn audio_top(&self) -> Option<f32> {
        (self.videos > 0 && self.videos < self.tracks && self.audio_h == self.row_h)
            .then_some(self.top_of_row(self.videos))
    }

    fn clip_rect(&self, track: usize, start: u64, end: u64) -> Rect {
        let h = self.track_h(track);
        // On the Cut page's detail the picture and its sound touch, as one clip.
        let pad = if self.audio_h < self.row_h {
            0.0
        } else if h < 30.0 {
            3.0
        } else {
            4.0
        };
        let y = self.row_top(track) + pad;
        Rect::from_min_max(
            pos2(self.x(start as f32), y),
            pos2(self.x(end as f32), y + h - 2.0 * pad),
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Body,
    Start,
    End,
}

enum Hit {
    Ruler,
    Clip {
        clip: ClipId,
        track: usize,
        part: Part,
    },
    Empty,
}

impl StudioApp {
    fn view_mut(&mut self, which: Which) -> &mut TimelineView {
        match which {
            Mode::Edit => &mut self.timeline,
            Mode::Detail => &mut self.cut_detail,
            Mode::Overview => &mut self.cut_overview,
        }
    }

    /// Shows the timeline in the rest of `ui`; the Edit page's timeline has its column of
    /// track headers on the left.
    pub(crate) fn timeline_ui(&mut self, ui: &mut Ui, ctx: &Context, which: Which) {
        self.timeline_in(ui, ctx, which, which == Mode::Edit);
    }

    /// The Edit timeline without the column of track headers and with only the sound tracks,
    /// at `row_h` points a row (the Audio page has its own headers).
    pub(crate) fn timeline_bare(&mut self, ui: &mut Ui, ctx: &Context, row_h: f32) {
        self.audio_row_h = Some(row_h);
        self.timeline_in(ui, ctx, Mode::Edit, false);
        self.audio_row_h = None;
    }

    fn timeline_in(&mut self, ui: &mut Ui, ctx: &Context, which: Which, headers: bool) {
        let audio_only = self.audio_row_h.is_some();
        let mut view = std::mem::replace(self.view_mut(which), TimelineView::new(which));
        let mode = view.mode;
        let header_w = if headers { EDIT_HEADER_W } else { 0.0 };

        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, Sense::hover());
        // The Edit timeline keeps a scroll bar under its tracks.
        let avail = if mode == Mode::Edit {
            Rect::from_min_max(full.min, pos2(full.right(), full.bottom() - SCROLLBAR_H))
        } else {
            full
        };
        let body = Rect::from_min_max(pos2(avail.left() + header_w, avail.top()), avail.max);
        let resp = ui.interact(
            body,
            ui.id().with(("timeline_body", mode as u8)),
            Sense::click_and_drag(),
        );
        let fps = self.project().fps.nominal() as f32;

        // Zoom (Ctrl + scroll) around the pointer, pan with the scroll wheel.
        if resp.hovered() && mode != Mode::Overview {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));
            if (zoom - 1.0).abs() > f32::EPSILON {
                let anchor_x = if mode == Mode::Detail {
                    body.center().x
                } else {
                    resp.hover_pos().map_or(body.center().x, |p| p.x)
                };
                let anchor = (anchor_x - body.left()) / view.zoom + view.scroll;
                view.zoom = (view.zoom * zoom).clamp(MIN_ZOOM, MAX_ZOOM);
                view.scroll = anchor - (anchor_x - body.left()) / view.zoom;
            } else if scroll.x != 0.0 || scroll.y != 0.0 {
                let frames = (scroll.x + scroll.y) / view.zoom;
                if mode == Mode::Detail {
                    self.playing = false;
                    self.go_to((self.playhead as f32 - frames).max(0.0) as u64);
                } else {
                    view.scroll -= frames;
                }
            }
        }
        match mode {
            Mode::Edit => {
                // Keep the playhead in view while playing.
                if self.playing {
                    let visible = body.width() / view.zoom;
                    let at = self.playhead as f32;
                    if at > view.scroll + visible - 20.0 / view.zoom || at < view.scroll {
                        view.scroll = (at - visible * 0.15).max(0.0);
                    }
                }
                view.scroll = view.scroll.max(0.0);
            }
            Mode::Detail => {
                // The playhead stays in the middle, except while it is dragged by its ruler.
                if let Drag::Scrub { scroll } = view.drag {
                    view.scroll = scroll;
                } else {
                    view.scroll = self.playhead as f32 - body.width() / 2.0 / view.zoom;
                }
            }
            Mode::Overview => {
                let seconds =
                    (self.project().duration() as f32 / fps * 1.05).max(OVERVIEW_MIN_SECONDS);
                view.zoom = (body.width() - OVERVIEW_PAD) / (seconds * fps);
                view.scroll = -OVERVIEW_PAD / view.zoom;
            }
        }

        let tracks = if audio_only {
            self.project()
                .tracks
                .iter()
                .filter(|t| t.kind == TrackKind::Audio)
                .count()
        } else {
            self.project().tracks.len()
        };
        let (order, row_of, videos) = row_order(&self.project().tracks, audio_only);
        let row_h = if let Some(h) = self.audio_row_h {
            h
        } else if mode == Mode::Edit {
            // Resolve's rows: 112 pixels for a picture track, 72 for a sound track, smaller
            // when there are many.
            let want = 112.0 * crate::chrome::R;
            let audios = tracks.saturating_sub(videos.min(tracks)) as f32;
            let room = avail.height() - mode.ruler_h() - audios * 72.0 * crate::chrome::R;
            want.min(room / videos.max(1) as f32).max(36.0)
        } else {
            mode.row_h()
        };
        let geo = Geo {
            left: body.left(),
            top: avail.top(),
            zoom: view.zoom,
            scroll: view.scroll,
            tracks,
            ruler_h: mode.ruler_h(),
            row_h,
            order,
            row_of,
            videos,
            body_top: 0.0,
            audio_h: row_h,
        };
        let mut geo = geo;
        geo.body_top = geo.top + geo.ruler_h;
        match mode {
            // As Resolve's Cut page: the picture rows sit at the bottom with a thin sound strip
            // under each.
            Mode::Detail => {
                geo.audio_h = CUT_AUDIO_H;
                let audios = (tracks - videos.min(tracks)) as f32;
                // Picture rows as tall as Resolve's while they fit.
                let room =
                    avail.bottom() - CUT_BOTTOM_GAP - geo.top - geo.ruler_h - audios * geo.audio_h;
                geo.row_h = geo.row_h.min(room / videos.max(1) as f32).max(24.0);
                let rows_h = videos as f32 * geo.row_h + audios * geo.audio_h;
                geo.body_top =
                    (avail.bottom() - rows_h - CUT_BOTTOM_GAP).max(geo.top + geo.ruler_h);
            }
            // The overview shows the picture only.
            Mode::Overview => {
                geo.audio_h = 0.0;
                geo.body_top += 18.0;
            }
            Mode::Edit => {
                // Picture tracks stack up from V1, which sits 81 pixels under the ruler.
                geo.audio_h = (72.0 * crate::chrome::R).min(geo.row_h);
                let v1_top = geo.top + geo.ruler_h + 81.0 * crate::chrome::R;
                let up = videos.saturating_sub(1) as f32 * geo.row_h;
                geo.body_top = if videos == 0 {
                    geo.top + geo.ruler_h
                } else {
                    (v1_top - up).max(geo.top + geo.ruler_h)
                };
            }
        }
        // The Cut page labels the rows beside the timeline (V1, A1).
        if mode != Mode::Edit {
            let rows: Vec<(String, bool, f32, f32)> = (0..geo.tracks.min(MAX_ROWS))
                .filter_map(|row| {
                    let track = self.project().tracks.get(usize::from(geo.order[row]))?;
                    let h = geo.height_of_row(row);
                    (h > 0.0).then(|| {
                        (
                            track.name.clone(),
                            track.kind == TrackKind::Audio,
                            geo.top_of_row(row),
                            h,
                        )
                    })
                })
                .collect();
            if mode == Mode::Detail {
                self.cut_rows_detail = rows;
            } else {
                self.cut_rows_overview = rows;
            }
        }

        // Dragging.
        if resp.drag_started() {
            if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
                let alt = ui.input(|i| i.modifiers.alt);
                view.drag = self.begin_drag(&geo, p, mode, alt);
            }
        }
        if resp.dragged() {
            if let Some(p) = resp.interact_pointer_pos() {
                self.update_drag(&mut view, &geo, p);
            }
        }
        if resp.drag_stopped() {
            self.finish_drag(&mut view);
        }
        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                match self.hit(&geo, p, mode) {
                    Hit::Ruler => {
                        self.playing = false;
                        self.go_to(geo.frame(p.x).max(0.0) as u64);
                    }
                    Hit::Clip { clip, .. } => self.select_only(clip),
                    Hit::Empty => {
                        if mode != Mode::Overview {
                            self.clear_selection();
                        }
                    }
                }
            }
        }
        // Media dropped from the pool, on the overview too as in Resolve's Cut page.
        if let Some(media) = resp.dnd_release_payload::<MediaId>() {
            if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
                self.drop_media(&view, &geo, p, *media);
            }
        }

        // The cursor says what a press here would do.
        let hover = resp.hover_pos();
        match &view.drag {
            Drag::Move { .. } => ctx.set_cursor_icon(CursorIcon::Grabbing),
            Drag::TrimStart { .. } | Drag::TrimEnd { .. } | Drag::Roll { .. } => {
                ctx.set_cursor_icon(CursorIcon::ResizeHorizontal)
            }
            Drag::Scrub { .. } => {}
            Drag::None => {
                if let Some(p) = hover {
                    match self.hit(&geo, p, mode) {
                        Hit::Clip {
                            part: Part::Body, ..
                        } => ctx.set_cursor_icon(CursorIcon::Grab),
                        Hit::Clip { .. } => ctx.set_cursor_icon(CursorIcon::ResizeHorizontal),
                        _ => {}
                    }
                }
            }
        }

        let dropping = resp
            .dnd_hover_payload::<MediaId>()
            .is_some()
            .then_some(hover)
            .flatten();
        let thumbs = if mode != Mode::Overview {
            self.timeline_thumbs(ui.ctx())
        } else {
            std::collections::HashMap::new()
        };
        self.paint_timeline(ui, &view, &geo, avail, dropping, &thumbs);
        if headers {
            self.track_headers(ui, avail, &geo);
        }
        if mode == Mode::Edit {
            self.scroll_bar(ui, &mut view, full, body);
        }
        *self.view_mut(which) = view;
    }

    /// Thumbnails of the pictures on the video tracks, for the Edit timeline's clips.
    pub(crate) fn timeline_thumbs(
        &mut self,
        ctx: &Context,
    ) -> std::collections::HashMap<MediaId, egui::TextureId> {
        let media: Vec<MediaId> = self
            .project()
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Video)
            .flat_map(|t| t.clips.iter().map(|c| c.media))
            .collect();
        let mut out = std::collections::HashMap::new();
        for id in media {
            if out.contains_key(&id) {
                continue;
            }
            if let Some(t) = self.media_thumb(ctx, id) {
                out.insert(id, t);
            }
        }
        out
    }

    /// The bar under the Edit timeline: its thumb is the part of the timeline in view.
    fn scroll_bar(&mut self, ui: &mut Ui, view: &mut TimelineView, full: Rect, body: Rect) {
        let bar = Rect::from_min_max(
            pos2(full.left(), full.bottom() - SCROLLBAR_H),
            full.right_bottom(),
        );
        let painter = ui.painter_at(bar);
        painter.rect_filled(bar, 0.0, Color32::from_rgb(36, 36, 41));
        let track = Rect::from_min_max(pos2(body.left(), bar.top()), bar.max);
        let visible = body.width() / view.zoom;
        let total = (self.project().duration() as f32 * 1.2).max(view.scroll + visible);
        let x0 = track.left() + track.width() * (view.scroll / total).clamp(0.0, 1.0);
        let w = (track.width() * visible / total).clamp(24.0, track.width());
        let thumb = Rect::from_min_size(pos2(x0, bar.top() + 2.0), vec2(w, SCROLLBAR_H - 4.0));
        let response = ui
            .interact(
                track,
                ui.id().with("timeline_scroll"),
                Sense::click_and_drag(),
            )
            .on_hover_text("Drag to scroll the timeline");
        painter.rect_filled(
            thumb,
            5.0,
            if response.dragged() || response.hovered() {
                Color32::from_rgb(92, 92, 98)
            } else {
                Color32::from_rgb(72, 72, 78)
            },
        );
        if response.dragged() {
            view.scroll = (view.scroll + response.drag_delta().x / track.width() * total).max(0.0);
        } else if response.clicked() {
            if let Some(p) = response.interact_pointer_pos() {
                let at = (p.x - track.left()) / track.width() * total;
                view.scroll = (at - visible / 2.0).max(0.0);
            }
        }
    }

    fn hit(&self, geo: &Geo, p: Pos2, mode: Mode) -> Hit {
        if mode == Mode::Overview || p.y < geo.top + geo.ruler_h {
            return Hit::Ruler;
        }
        let Some(track) = geo.track_at(p.y) else {
            return Hit::Empty;
        };
        let Some(t) = self.project().tracks.get(track) else {
            return Hit::Empty;
        };
        for clip in &t.clips {
            let rect = geo.clip_rect(track, clip.start, clip.end());
            if p.x >= rect.left() && p.x <= rect.right() {
                let edge = EDGE_PX.min(rect.width() / 3.0);
                let part = if p.x - rect.left() < edge {
                    Part::Start
                } else if rect.right() - p.x < edge {
                    Part::End
                } else {
                    Part::Body
                };
                return Hit::Clip {
                    clip: clip.id,
                    track,
                    part,
                };
            }
        }
        Hit::Empty
    }

    fn begin_drag(&mut self, geo: &Geo, p: Pos2, mode: Mode, alt: bool) -> Drag {
        match self.hit(geo, p, mode) {
            Hit::Ruler => {
                self.playing = false;
                self.go_to(geo.frame(p.x).max(0.0) as u64);
                Drag::Scrub { scroll: geo.scroll }
            }
            Hit::Clip { clip, track, part } => {
                self.select_only(clip);
                let Some((t, c)) = self.project().clip(clip) else {
                    return Drag::None;
                };
                if t.locked {
                    self.fail("the track is locked");
                    return Drag::None;
                }
                let (start, end) = (c.start, c.end());
                // With Alt, an edge that touches another clip rolls the cut between them.
                if alt && part != Part::Body {
                    let neighbour = t.clips.iter().find(|n| {
                        if part == Part::Start {
                            n.end() == start
                        } else {
                            n.start == end
                        }
                    });
                    if let Some(n) = neighbour {
                        let (left, right) = if part == Part::Start {
                            (n.id, clip)
                        } else {
                            (clip, n.id)
                        };
                        return Drag::Roll {
                            left,
                            right,
                            to: if part == Part::Start { start } else { end },
                            ok: true,
                        };
                    }
                }
                match part {
                    Part::Body => Drag::Move {
                        clip,
                        grab: geo.frame(p.x) as i64 - start as i64,
                        track,
                        start,
                        ok: true,
                    },
                    Part::Start => Drag::TrimStart {
                        clip,
                        to: start,
                        ok: true,
                    },
                    Part::End => Drag::TrimEnd {
                        clip,
                        to: end,
                        ok: true,
                    },
                }
            }
            Hit::Empty => Drag::None,
        }
    }

    /// Whether `command` would work on the project as it is now. A drag asks the same question on
    /// every frame, and again for the places it has been over, so the recent answers are kept
    /// until the project changes; a copy of the whole project is only made for a new question.
    fn would_work(&self, view: &mut TimelineView, command: Command) -> bool {
        let project = (self.epoch, self.session.revision());
        if view.would.0 != project {
            view.would = (project, Vec::new());
        }
        if let Some((_, ok)) = view.would.1.iter().find(|(asked, _)| *asked == command) {
            return *ok;
        }
        let ok = command.clone().apply(&mut self.project().clone()).is_ok();
        if view.would.1.len() >= WOULD_KEPT {
            view.would.1.remove(0);
        }
        view.would.1.push((command, ok));
        ok
    }

    /// The edge or playhead nearest to `frame`, if one is within the snapping distance.
    fn snap_target(
        &self,
        view: &TimelineView,
        geo: &Geo,
        frame: i64,
        exclude: ClipId,
    ) -> Option<i64> {
        if !view.snap {
            return None;
        }
        let reach = (SNAP_PX / geo.zoom).ceil() as i64;
        let project = self.project();
        let edges = project
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .filter(|c| c.id != exclude)
            .flat_map(|c| [c.start as i64, c.end() as i64]);
        [0, self.playhead as i64]
            .into_iter()
            .chain(edges)
            .filter(|t| (t - frame).abs() <= reach)
            .min_by_key(|t| (t - frame).abs())
    }

    fn update_drag(&mut self, view: &mut TimelineView, geo: &Geo, p: Pos2) {
        let frame = geo.frame(p.x) as i64;
        match view.drag.clone() {
            Drag::None => {}
            Drag::Scrub { .. } => self.go_to(frame.max(0) as u64),
            Drag::Move {
                clip, grab, track, ..
            } => {
                let Some((_, c)) = self.project().clip(clip) else {
                    return;
                };
                let duration = c.duration as i64;
                let mut start = (frame - grab).max(0);
                let snap_start = self.snap_target(view, geo, start, clip).map(|t| t - start);
                let snap_end = self
                    .snap_target(view, geo, start + duration, clip)
                    .map(|t| t - (start + duration));
                if let Some(d) = [snap_start, snap_end]
                    .into_iter()
                    .flatten()
                    .min_by_key(|d| d.abs())
                {
                    start = (start + d).max(0);
                }
                // Stay on the same row when the pointer leaves the tracks of this kind (a
                // video clip stays on video tracks, a sound on audio tracks).
                let own_kind = self.project().tracks.get(track).map(|t| t.kind);
                let track = geo
                    .track_at(p.y)
                    .filter(|t| self.project().tracks.get(*t).map(|t| t.kind) == own_kind)
                    .unwrap_or(track);
                let ok = self.project().tracks.get(track).is_some_and(|t| !t.locked)
                    && self.would_work(
                        view,
                        Command::MoveClip {
                            clip,
                            track: self.project().tracks[track].id,
                            start: start as u64,
                        },
                    );
                view.drag = Drag::Move {
                    clip,
                    grab,
                    track,
                    start: start as u64,
                    ok,
                };
            }
            Drag::TrimStart { clip, .. } => {
                let to = self
                    .snap_target(view, geo, frame.max(0), clip)
                    .unwrap_or(frame.max(0)) as u64;
                view.drag = Drag::TrimStart {
                    clip,
                    to,
                    ok: self.would_work(view, Command::TrimStart { clip, start: to }),
                };
            }
            Drag::Roll { left, right, .. } => {
                let to = self
                    .snap_target(view, geo, frame.max(0), left)
                    .unwrap_or(frame.max(0)) as u64;
                view.drag = Drag::Roll {
                    left,
                    right,
                    to,
                    ok: self.would_work(view, Command::RollTrim { left, right, to }),
                };
            }
            Drag::TrimEnd { clip, .. } => {
                let to = self
                    .snap_target(view, geo, frame.max(0), clip)
                    .unwrap_or(frame.max(0)) as u64;
                view.drag = Drag::TrimEnd {
                    clip,
                    to,
                    ok: self.would_work(view, Command::TrimEnd { clip, end: to }),
                };
            }
        }
    }

    fn finish_drag(&mut self, view: &mut TimelineView) {
        let edit = match &view.drag {
            Drag::Roll { to, .. } | Drag::TrimStart { to, .. } | Drag::TrimEnd { to, .. } => {
                Some(*to)
            }
            _ => None,
        };
        let scrubbed = matches!(view.drag, Drag::Scrub { .. });
        self.finish_drag_inner(view);
        if scrubbed {
            self.dynamic_trim_apply();
        }
        if let Some(frame) = edit.filter(|_| self.extra.loop_trim) {
            self.loop_around(frame);
        }
    }

    fn finish_drag_inner(&mut self, view: &mut TimelineView) {
        if self.extra.position_lock && !matches!(view.drag, Drag::None | Drag::Scrub { .. }) {
            view.drag = Drag::None;
            self.fail("Position Lock is on (Timeline > Timeline Settings)");
            return;
        }
        if self.extra.trim_mode {
            match std::mem::take(&mut view.drag) {
                Drag::Move { clip, start, .. } => {
                    if let Some(old) = self.project().clip(clip).map(|(_, c)| c.start) {
                        self.slip_by(clip, start as i64 - old as i64);
                    }
                }
                Drag::TrimStart { clip, to, .. } => self.ripple_edit(clip, true, to),
                Drag::TrimEnd { clip, to, .. } => self.ripple_edit(clip, false, to),
                Drag::Roll {
                    left,
                    right,
                    to,
                    ok,
                } => {
                    if self.extra.asymmetric {
                        self.ripple_edit(left, false, to);
                    } else if ok {
                        self.run(Command::RollTrim { left, right, to });
                    }
                }
                Drag::None | Drag::Scrub { .. } => {}
            }
            return;
        }
        match std::mem::take(&mut view.drag) {
            Drag::None | Drag::Scrub { .. } => {}
            Drag::Move {
                clip,
                track,
                start,
                ok,
                ..
            } => {
                let Some(track_id) = self.project().tracks.get(track).map(|t| t.id) else {
                    return;
                };
                let unchanged = self
                    .project()
                    .clip(clip)
                    .is_some_and(|(t, c)| t.id == track_id && c.start == start);
                if unchanged {
                    return;
                }
                if ok {
                    self.run(Command::MoveClip {
                        clip,
                        track: track_id,
                        start,
                    });
                } else {
                    self.fail("the clip does not fit there");
                }
            }
            Drag::Roll {
                left,
                right,
                to,
                ok,
            } => {
                let cut = self.project().clip(right).map(|(_, c)| c.start);
                if self.extra.asymmetric && cut.is_some_and(|c| c != to) {
                    self.ripple_edit(left, false, to);
                } else if cut.is_some_and(|c| c != to) {
                    if ok {
                        self.run(Command::RollTrim { left, right, to });
                    } else {
                        self.fail("the cut cannot be rolled that far");
                    }
                }
            }
            Drag::TrimStart { clip, to, ok } => {
                if self
                    .project()
                    .clip(clip)
                    .is_some_and(|(_, c)| c.start != to)
                {
                    if ok {
                        self.run(Command::TrimStart { clip, start: to });
                    } else {
                        self.fail("the clip cannot be trimmed that far");
                    }
                }
            }
            Drag::TrimEnd { clip, to, ok } => {
                if self
                    .project()
                    .clip(clip)
                    .is_some_and(|(_, c)| c.end() != to)
                {
                    if ok {
                        self.run(Command::TrimEnd { clip, end: to });
                    } else {
                        self.fail("the clip cannot be trimmed that far");
                    }
                }
            }
        }
    }

    /// Media dragged from the pool and dropped on a row.
    fn drop_media(&mut self, view: &TimelineView, geo: &Geo, p: Pos2, media: MediaId) {
        let Some(track) = geo
            .nearest_track(p.y)
            .and_then(|i| self.project().tracks.get(i))
            .map(|t| t.id)
        else {
            self.fail("drop media on a track");
            return;
        };
        let start = geo.frame(p.x).max(0.0) as i64;
        let start = self
            .snap_target(view, geo, start, ClipId(0))
            .unwrap_or(start)
            .max(0) as u64;
        // A video dropped on a video track brings its sound along on an audio track; dropped on
        // an audio track, it is the sound alone.
        let on_video = self
            .project()
            .track(track)
            .is_some_and(|t| t.kind == TrackKind::Video);
        let with_sound = on_video && self.has_sound(media);
        if let Some(Outcome::Clip(id)) = self.run(Command::AddClip {
            track,
            media,
            start,
            duration: None,
            with_sound,
        }) {
            self.select_only(id);
        }
    }

    /// The column left of the tracks: the playhead's timecode over the ruler, then one header
    /// per track with its name and its lock and mute buttons. A right click adds tracks.
    fn track_headers(&mut self, ui: &mut Ui, avail: Rect, geo: &Geo) {
        let header_w = EDIT_HEADER_W;
        let column = Rect::from_min_size(avail.left_top(), vec2(header_w, avail.height()));
        let painter = ui.painter_at(column);
        painter.rect_filled(column, 0.0, Color32::from_rgb(40, 40, 45));
        let tracks: Vec<(usize, picasso_core::TrackId, TrackKind, String, bool, bool)> = self
            .project()
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| (i, t.id, t.kind, t.name.clone(), t.locked, t.muted))
            .collect();
        let clip_counts: Vec<usize> = self
            .project()
            .tracks
            .iter()
            .map(|t| t.clips.len())
            .collect();
        let mut counts = (0, 0);
        for (i, id, kind, name, locked, muted) in tracks {
            let video = kind == TrackKind::Video;
            let label = if video {
                counts.0 += 1;
                format!("V{}", counts.0)
            } else {
                counts.1 += 1;
                format!("A{}", counts.1)
            };
            let top = geo.row_top(i);
            let rect = Rect::from_min_size(pos2(avail.left(), top), vec2(header_w, geo.track_h(i)));
            painter.line_segment(
                [rect.left_bottom(), rect.right_bottom()],
                Stroke::new(1.0, Color32::from_rgb(9, 9, 9)),
            );
            // The coloured edge: blue for video, green for sound.
            painter.rect_filled(
                Rect::from_min_size(rect.min, vec2(3.0 * crate::chrome::R, geo.track_h(i) - 1.0)),
                0.0,
                if video {
                    Color32::from_rgb(64, 120, 196)
                } else {
                    Color32::from_rgb(64, 170, 104)
                },
            );
            // Positions are Resolve's pixels at 1080p from the header's left edge and the row's top.
            let r = crate::chrome::R;
            let row_y = top + 13.0 * r;
            let at = |x: f32| pos2(rect.left() + x * r, row_y);
            // The track's patch: the source and the destination, each its name in a red frame.
            for x in [31.0, 70.0] {
                let badge = Rect::from_center_size(at(x), vec2(25.0 * r, 16.0 * r));
                painter.rect_filled(badge, 2.0, Color32::from_rgb(62, 36, 36));
                painter.rect_stroke(
                    badge,
                    2.0,
                    Stroke::new(1.2, Color32::from_rgb(214, 70, 58)),
                    StrokeKind::Inside,
                );
                painter.text(
                    badge.center(),
                    Align2::CENTER_CENTER,
                    &label,
                    theme::semibold(10.5 * r),
                    theme::WHITE,
                );
            }
            // Lock, link and (for video) the film strip, or (for sound) solo and mute.
            let lock = Rect::from_center_size(at(105.0), vec2(20.0 * r, 20.0 * r));
            crate::icons::paint_scaled(
                &painter,
                lock.center(),
                crate::icons::Icon::PositionLock,
                if locked {
                    theme::WARN
                } else {
                    Color32::from_rgb(110, 110, 116)
                },
                0.52,
            );
            crate::icons::paint_scaled(
                &painter,
                at(131.0),
                crate::icons::Icon::TrimTool,
                Color32::from_rgb(170, 170, 176),
                0.5,
            );
            if video {
                crate::icons::paint_scaled(
                    &painter,
                    at(159.0),
                    crate::icons::Icon::FilmView,
                    Color32::from_rgb(170, 170, 176),
                    0.55,
                );
            }
            let mute = Rect::from_center_size(
                at(if video { 159.0 } else { 187.0 }),
                vec2(18.0 * r, 16.0 * r),
            );
            if !video {
                let solo = Rect::from_center_size(at(159.0), vec2(18.0 * r, 16.0 * r));
                painter.rect_filled(solo, 2.0, Color32::from_rgb(62, 62, 68));
                painter.text(
                    solo.center(),
                    Align2::CENTER_CENTER,
                    "S",
                    regular(9.5 * r),
                    Color32::from_rgb(140, 140, 146),
                );
                painter.rect_filled(
                    mute,
                    2.0,
                    if muted {
                        theme::WARN
                    } else {
                        Color32::from_rgb(62, 62, 68)
                    },
                );
                painter.text(
                    mute.center(),
                    Align2::CENTER_CENTER,
                    "M",
                    regular(9.5 * r),
                    if muted {
                        Color32::BLACK
                    } else {
                        Color32::from_rgb(140, 140, 146)
                    },
                );
                painter.text(
                    at(235.0),
                    Align2::CENTER_CENTER,
                    "2.0",
                    regular(10.0 * r),
                    Color32::from_rgb(140, 140, 146),
                );
            }
            let shown_name = if name == label {
                format!("{} {}", if video { "Video" } else { "Audio" }, &label[1..])
            } else {
                name
            };
            painter.text(
                pos2(rect.left() + 18.0 * r, top + 38.0 * r),
                Align2::LEFT_CENTER,
                &shown_name,
                theme::semibold(10.5 * r),
                theme::WHITE,
            );
            let n = clip_counts.get(i).copied().unwrap_or(0);
            painter.text(
                pos2(rect.left() + 18.0 * r, top + 53.0 * r),
                Align2::LEFT_CENTER,
                format!("{n} Clip{}", if n == 1 { "" } else { "s" }),
                regular(10.0 * r),
                Color32::from_rgb(150, 150, 156),
            );
            if ui
                .interact(lock, ui.id().with(("track_lock", id)), Sense::click())
                .on_hover_text("Lock the track")
                .clicked()
            {
                self.run(Command::SetTrackLocked {
                    track: id,
                    locked: !locked,
                });
            }
            let mute_hint = if video {
                "Turn the track off"
            } else {
                "Mute the track"
            };
            if ui
                .interact(mute, ui.id().with(("track_mute", id)), Sense::click())
                .on_hover_text(mute_hint)
                .clicked()
            {
                self.run(Command::SetTrackMuted {
                    track: id,
                    muted: !muted,
                });
            }
        }
        // The boundary between the video and the sound tracks.
        if geo.videos > 0 && geo.videos < geo.tracks {
            let y = geo.top_of_row(geo.videos);
            painter.line_segment(
                [pos2(column.left(), y), pos2(column.right(), y)],
                Stroke::new(1.5, Color32::from_rgb(58, 58, 58)),
            );
        }
        // The playhead's timecode over the ruler.
        let ruler = Rect::from_min_size(avail.left_top(), vec2(header_w, geo.ruler_h));
        painter.rect_filled(ruler, 0.0, Color32::from_rgb(40, 40, 45));
        painter.line_segment(
            [ruler.left_bottom(), ruler.right_bottom()],
            Stroke::new(1.0, Color32::from_rgb(30, 30, 34)),
        );
        painter.text(
            ruler.center() + vec2(0.0, 2.0),
            Align2::CENTER_CENTER,
            self.tc(self.playhead as i64),
            theme::semibold(20.0 * crate::chrome::R),
            theme::WHITE,
        );
        painter.line_segment(
            [column.right_top(), column.right_bottom()],
            Stroke::new(1.0, Color32::from_rgb(26, 26, 30)),
        );
        // A right click in the column adds tracks, or deletes the track under the pointer.
        let menu_id = ui.id().with("track_menu_track");
        let column_resp = ui.interact(column, ui.id().with("track_column"), Sense::click());
        if column_resp.secondary_clicked() {
            let under = column_resp
                .interact_pointer_pos()
                .and_then(|p| geo.track_at(p.y))
                .and_then(|i| self.project().tracks.get(i).map(|t| t.id));
            ui.data_mut(|d| d.insert_temp(menu_id, under));
        }
        column_resp.context_menu(|ui| {
            for (label, kind) in [
                ("Add Video Track", TrackKind::Video),
                ("Add Audio Track", TrackKind::Audio),
            ] {
                if ui.button(label).clicked() {
                    self.run(Command::AddTrack { kind });
                    ui.close();
                }
            }
            let under: Option<TrackId> = ui.data(|d| d.get_temp(menu_id)).flatten();
            if let Some(track) = under {
                let kind = self.project().track(track).map(|t| t.kind);
                let label = match kind {
                    Some(TrackKind::Audio) => "Delete Audio Track",
                    _ => "Delete Video Track",
                };
                let others = self
                    .project()
                    .tracks
                    .iter()
                    .filter(|t| Some(t.kind) == kind)
                    .count();
                ui.separator();
                if ui
                    .add_enabled(others > 1, egui::Button::new(label))
                    .clicked()
                {
                    self.run(Command::RemoveTrack { track });
                    ui.close();
                }
            }
        });
    }

    fn paint_timeline(
        &self,
        ui: &mut Ui,
        view: &TimelineView,
        geo: &Geo,
        avail: Rect,
        drop_at: Option<Pos2>,
        thumbs: &std::collections::HashMap<MediaId, egui::TextureId>,
    ) {
        let mode = view.mode;
        let area = Rect::from_min_max(pos2(geo.left, avail.top()), avail.max);
        let painter = ui.painter_at(area);
        let project = self.project();
        if mode == Mode::Edit {
            painter.rect_filled(area, 0.0, Color32::from_rgb(40, 40, 45));
        } else {
            painter.rect_filled(area, 0.0, CUT_BG);
            let start = geo.x(0.0);
            if start > area.left() {
                let before =
                    Rect::from_min_max(area.min, pos2(start.min(area.right()), area.bottom()));
                painter.rect_filled(before, 0.0, CUT_BEFORE);
                painter.line_segment(
                    [pos2(start, area.top()), pos2(start, area.bottom())],
                    Stroke::new(2.0, Color32::from_rgb(48, 47, 52)),
                );
            }
        }

        // Rows.
        for (i, track) in project.tracks.iter().enumerate() {
            let top = geo.row_top(i);
            if mode == Mode::Edit {
                let row =
                    Rect::from_min_max(pos2(area.left(), top), pos2(area.right(), top + geo.row_h));
                painter.rect_filled(row, 0.0, Color32::from_rgb(40, 40, 45));
                // A light line where the video tracks end and the sound tracks begin.
                let first_audio = geo.audio_top().is_some_and(|y| (y - top).abs() < 0.5);
                let (width, color) = if track.kind == TrackKind::Audio && first_audio {
                    (2.0, Color32::from_rgb(70, 70, 76))
                } else {
                    (1.0, Color32::from_rgb(30, 30, 34))
                };
                painter.line_segment(
                    [
                        pos2(row.left(), row.top() + 0.5),
                        pos2(row.right(), row.top() + 0.5),
                    ],
                    Stroke::new(width, color),
                );
            }
            for clip in &track.clips {
                let rect = geo.clip_rect(i, clip.start, clip.end());
                if rect.right() < area.left() || rect.left() > area.right() {
                    continue;
                }
                let dragged_away =
                    matches!(&view.drag, Drag::Move { clip: c, .. } if *c == clip.id);
                if mode == Mode::Overview {
                    // Resolve's overview: the picture as one thin blue bar, no sound.
                    if track.kind == TrackKind::Audio {
                        continue;
                    }
                    let color = clip_color(clip).unwrap_or(CUT_BLUE);
                    painter.rect_filled(rect, 0.0, color);
                    painter.line_segment(
                        [rect.left_top(), rect.right_top()],
                        Stroke::new(1.0, Color32::from_rgb(71, 109, 140)),
                    );
                    painter.line_segment(
                        [rect.left_bottom(), rect.right_bottom()],
                        Stroke::new(1.0, Color32::from_rgb(24, 40, 55)),
                    );
                } else if mode == Mode::Detail {
                    self.paint_cut_clip(
                        &painter,
                        project,
                        rect,
                        clip.id,
                        dragged_away || track.muted,
                        thumbs.get(&clip.media).copied(),
                    );
                    paint_transition(&painter, project, rect, clip.id);
                } else {
                    self.paint_clip(
                        &painter,
                        project,
                        rect,
                        clip.id,
                        dragged_away || track.muted,
                        (mode == Mode::Edit).then(|| thumbs.get(&clip.media).copied()),
                    );
                }
            }
        }

        // The preview of a drag.
        match &view.drag {
            Drag::Move {
                clip,
                track,
                start,
                ok,
                ..
            } => {
                if let Some((_, c)) = project.clip(*clip) {
                    ghost(
                        &painter,
                        geo.clip_rect(*track, *start, start + c.duration),
                        *ok,
                    );
                }
            }
            Drag::TrimStart { clip, to, ok } => {
                if let Some((t, c)) = project.clip(*clip) {
                    let ti = project
                        .tracks
                        .iter()
                        .position(|x| x.id == t.id)
                        .unwrap_or(0);
                    ghost(&painter, geo.clip_rect(ti, *to, c.end().max(*to + 1)), *ok);
                }
            }
            Drag::TrimEnd { clip, to, ok } => {
                if let Some((t, c)) = project.clip(*clip) {
                    let ti = project
                        .tracks
                        .iter()
                        .position(|x| x.id == t.id)
                        .unwrap_or(0);
                    ghost(
                        &painter,
                        geo.clip_rect(ti, c.start, (*to).max(c.start + 1)),
                        *ok,
                    );
                }
            }
            Drag::Roll {
                left,
                right,
                to,
                ok,
            } => {
                if let (Some((t, l)), Some((_, r))) = (project.clip(*left), project.clip(*right)) {
                    let ti = project
                        .tracks
                        .iter()
                        .position(|x| x.id == t.id)
                        .unwrap_or(0);
                    ghost(
                        &painter,
                        geo.clip_rect(ti, l.start, (*to).max(l.start + 1)),
                        *ok,
                    );
                    ghost(&painter, geo.clip_rect(ti, *to, r.end().max(*to + 1)), *ok);
                }
            }
            Drag::None | Drag::Scrub { .. } => {}
        }
        if let Some(p) = drop_at {
            if let Some(track) = geo.track_at(p.y) {
                let frame = geo.frame(p.x).max(0.0) as u64;
                ghost(
                    &painter,
                    geo.clip_rect(track, frame, frame + project.fps.nominal() * 2),
                    true,
                );
            }
        }

        self.paint_ruler(&painter, geo, area, mode);

        self.paint_marks(&painter, geo, area, mode);

        // The playhead.
        let x = geo.x(self.playhead as f32);
        if x >= area.left() - 12.0 && x <= area.right() + 12.0 {
            // Red on the Edit page, and on the Cut page once the timeline has clips (as Resolve);
            // a grey pin on an empty Cut page.
            let has_clips = project.tracks.iter().any(|t| !t.clips.is_empty());
            if mode != Mode::Edit && has_clips {
                let red = Color32::from_rgb(226, 79, 61);
                painter.line_segment(
                    [pos2(x, area.top() + 6.0), pos2(x, area.bottom())],
                    Stroke::new(2.0, red),
                );
                painter.add(Shape::convex_polygon(
                    vec![
                        pos2(x - 6.0, area.top() + 1.0),
                        pos2(x + 6.0, area.top() + 1.0),
                        pos2(x + 6.0, area.top() + 7.0),
                        pos2(x, area.top() + 13.0),
                        pos2(x - 6.0, area.top() + 7.0),
                    ],
                    red,
                    Stroke::NONE,
                ));
            } else if mode == Mode::Edit {
                let grey = Color32::from_rgb(226, 62, 50);
                painter.line_segment(
                    [pos2(x, area.top() + 8.0), pos2(x, area.bottom())],
                    Stroke::new(1.5, grey),
                );
                painter.add(Shape::convex_polygon(
                    vec![
                        pos2(x - 10.0, area.top() + 1.0),
                        pos2(x + 10.0, area.top() + 1.0),
                        pos2(x + 10.0, area.top() + 9.0),
                        pos2(x, area.top() + 18.0),
                        pos2(x - 10.0, area.top() + 9.0),
                    ],
                    grey,
                    Stroke::NONE,
                ));
            } else {
                // A grey line with a rounded pentagon on top, like a pin.
                painter.line_segment(
                    [pos2(x, area.top() + 8.0), pos2(x, area.bottom())],
                    Stroke::new(2.5, theme::PLAYHEAD_GRAY),
                );
                painter.add(Shape::convex_polygon(
                    vec![
                        pos2(x - 10.5, area.top() + 1.0),
                        pos2(x + 10.5, area.top() + 1.0),
                        pos2(x + 10.5, area.top() + 9.0),
                        pos2(x, area.top() + 18.0),
                        pos2(x - 10.5, area.top() + 9.0),
                    ],
                    theme::PLAYHEAD_GRAY,
                    Stroke::new(1.5, theme::PLAYHEAD_GRAY),
                ));
            }
        }
    }
    /// The In/Out range and the markers.
    fn paint_marks(&self, painter: &egui::Painter, geo: &Geo, area: Rect, mode: Mode) {
        let project = self.project();
        let ruler_bottom = area.top() + geo.ruler_h;
        if project.mark_in.is_some() || project.mark_out.is_some() {
            let x0 = project
                .mark_in
                .map_or(area.left(), |f| geo.x(f as f32))
                .max(area.left());
            let x1 = project
                .mark_out
                .map_or(area.right(), |f| geo.x(f as f32))
                .min(area.right());
            if x1 > x0 {
                painter.rect_filled(
                    Rect::from_min_max(pos2(x0, area.top()), pos2(x1, area.bottom())),
                    0.0,
                    Color32::from_white_alpha(9),
                );
            }
            for (frame, left) in [(project.mark_in, true), (project.mark_out, false)] {
                let Some(f) = frame else { continue };
                let x = geo.x(f as f32);
                if x >= area.left() && x <= area.right() {
                    let s = if left { 1.0 } else { -1.0 };
                    let stroke = Stroke::new(2.0, Color32::from_rgb(210, 210, 214));
                    painter.line_segment([pos2(x, area.top()), pos2(x, ruler_bottom)], stroke);
                    painter.line_segment(
                        [
                            pos2(x, area.top() + 1.0),
                            pos2(x + 7.0 * s, area.top() + 1.0),
                        ],
                        stroke,
                    );
                    painter.line_segment(
                        [
                            pos2(x, ruler_bottom - 1.0),
                            pos2(x + 7.0 * s, ruler_bottom - 1.0),
                        ],
                        stroke,
                    );
                }
            }
        }
        for marker in &project.markers {
            let x = geo.x(marker.frame as f32);
            if x < area.left() - 8.0 || x > area.right() + 8.0 {
                continue;
            }
            let color = theme::marker_color(marker.color);
            painter.line_segment(
                [pos2(x, ruler_bottom), pos2(x, area.bottom())],
                Stroke::new(1.0, color.gamma_multiply(0.45)),
            );
            painter.add(Shape::convex_polygon(
                vec![
                    pos2(x - 5.0, ruler_bottom - 12.0),
                    pos2(x + 5.0, ruler_bottom - 12.0),
                    pos2(x + 5.0, ruler_bottom - 4.0),
                    pos2(x, ruler_bottom + 1.0),
                    pos2(x - 5.0, ruler_bottom - 4.0),
                ],
                color,
                Stroke::NONE,
            ));
            if mode == Mode::Detail && !marker.name.is_empty() {
                painter.text(
                    pos2(x + 8.0, ruler_bottom + 10.0),
                    Align2::LEFT_CENTER,
                    &marker.name,
                    regular(13.0),
                    color,
                );
            }
        }
    }

    fn paint_ruler(&self, painter: &egui::Painter, geo: &Geo, area: Rect, mode: Mode) {
        let nominal = self.project().fps.nominal();
        let ruler = Rect::from_min_max(
            area.left_top(),
            pos2(area.right(), area.top() + geo.ruler_h),
        );
        if mode == Mode::Edit {
            painter.rect_filled(ruler, 0.0, Color32::from_rgb(40, 40, 45));
            painter.line_segment(
                [ruler.left_bottom(), ruler.right_bottom()],
                Stroke::new(1.0, Color32::from_rgb(30, 30, 34)),
            );
        }
        let strip = Rect::from_min_size(ruler.min + vec2(0.0, 2.0), vec2(ruler.width(), 8.0));
        if mode != Mode::Edit {
            painter.rect_filled(strip, 0.0, Color32::from_rgb(57, 56, 63));
            let start = geo.x(0.0);
            if start > strip.left() {
                painter.rect_filled(
                    Rect::from_min_max(strip.min, pos2(start.min(strip.right()), strip.bottom())),
                    0.0,
                    CUT_BEFORE,
                );
            }
            painter.line_segment(
                [
                    pos2(ruler.left(), ruler.bottom()),
                    pos2(ruler.right(), ruler.bottom()),
                ],
                Stroke::new(1.0, Color32::from_rgb(38, 38, 42)),
            );
        }
        let (tick_color, label_color, size) = match mode {
            Mode::Edit => (
                Color32::from_rgb(112, 112, 118),
                Color32::from_rgb(150, 150, 156),
                13.0,
            ),
            Mode::Detail | Mode::Overview => (
                Color32::from_rgb(70, 68, 76),
                Color32::from_rgb(118, 117, 122),
                10.0,
            ),
        };
        let stroke = Stroke::new(1.0, tick_color);
        let seconds = |s: u64| s * nominal;
        let label_steps = if mode == Mode::Edit {
            [1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 3600].map(seconds)
        } else {
            [1, 2, 5, 10, 30, 60, 120, 300, 600, 1800, 3600, 7200].map(seconds)
        };
        let min_label = if mode == Mode::Edit {
            280.0
        } else {
            LABEL_SPACING
        };
        let label_step = label_steps
            .into_iter()
            .find(|s| *s as f32 * geo.zoom >= min_label)
            .unwrap_or(seconds(3600));
        let min_tick = TICK_SPACING;
        let tick_candidates = [1, 2, 3, 6, 12, nominal, 2 * nominal, 5 * nominal];
        let tick_step = tick_candidates
            .into_iter()
            .filter(|s| *s > 0 && label_step % *s == 0)
            .find(|s| *s as f32 * geo.zoom >= min_tick)
            .unwrap_or(label_step);

        let left_frame = geo.frame(area.left()).floor() as i64;
        let first = left_frame.div_euclid(tick_step as i64) * tick_step as i64 - tick_step as i64;
        let mut frame = first;
        let (major, medium, minor) = if mode == Mode::Edit {
            (17.0, 10.0, 6.0)
        } else {
            // Inside the strip.
            (8.0, 5.5, 3.5)
        };
        while geo.x(frame as f32) < area.right() + 1.0 {
            let x = geo.x(frame as f32);
            if x >= area.left() - 1.0 {
                let on_label = frame.rem_euclid(label_step as i64) == 0;
                let on_half = label_step % 2 == 0 && frame.rem_euclid((label_step / 2) as i64) == 0;
                if on_label {
                    painter
                        .line_segment([pos2(x, ruler.top()), pos2(x, ruler.top() + major)], stroke);
                    let label_y = if mode == Mode::Edit {
                        ruler.top() + 28.0
                    } else {
                        strip.top() + 15.0
                    };
                    painter.text(
                        pos2(x + if mode == Mode::Edit { 8.0 } else { 6.0 }, label_y),
                        Align2::LEFT_CENTER,
                        self.tc(frame),
                        regular(size),
                        label_color,
                    );
                    if mode == Mode::Edit {
                        painter.line_segment(
                            [pos2(x, ruler.bottom()), pos2(x, area.bottom())],
                            Stroke::new(1.0, Color32::from_rgb(48, 48, 54)),
                        );
                    }
                    if mode == Mode::Detail {
                        painter.line_segment(
                            [pos2(x, strip.bottom()), pos2(x, area.bottom())],
                            Stroke::new(1.0, Color32::from_rgb(56, 55, 62)),
                        );
                    }
                } else if on_half {
                    painter.line_segment(
                        [pos2(x, ruler.top()), pos2(x, ruler.top() + medium)],
                        stroke,
                    );
                } else {
                    painter
                        .line_segment([pos2(x, ruler.top()), pos2(x, ruler.top() + minor)], stroke);
                }
            }
            frame += tick_step as i64;
        }
    }

    fn paint_clip(
        &self,
        painter: &egui::Painter,
        project: &Project,
        rect: Rect,
        id: ClipId,
        dim: bool,
        edit: Option<Option<egui::TextureId>>,
    ) {
        self.paint_clip_body(painter, project, rect, id, dim, edit);
        paint_transition(painter, project, rect, id);
    }

    fn paint_clip_body(
        &self,
        painter: &egui::Painter,
        project: &Project,
        rect: Rect,
        id: ClipId,
        dim: bool,
        edit: Option<Option<egui::TextureId>>,
    ) {
        let Some((track, clip)) = project.clip(id) else {
            return;
        };
        if let Some(thumb) = edit {
            self.paint_edit_clip(painter, project, rect, id, dim, thumb);
            return;
        }
        let base = if let Some(c) = clip_color(clip) {
            c
        } else if track.kind == TrackKind::Audio {
            theme::AUDIO_CLIP
        } else {
            theme::media_color(clip.media.0)
        };
        let dim = dim || !clip.enabled;
        let fill = if dim { base.gamma_multiply(0.45) } else { base };
        painter.rect_filled(rect, 4.0, fill);
        let strip = Rect::from_min_size(rect.min, vec2(rect.width(), 17.0));
        painter.rect_filled(strip, 4.0, Color32::from_black_alpha(70));
        if rect.width() > 24.0 {
            let clipped = painter.with_clip_rect(rect.shrink2(vec2(4.0, 0.0)));
            clipped.text(
                rect.left_top() + vec2(6.0, 1.0),
                Align2::LEFT_TOP,
                &clip.name,
                regular(13.0),
                Color32::WHITE,
            );
            if rect.height() > 36.0 {
                clipped.text(
                    rect.left_bottom() + vec2(6.0, -4.0),
                    Align2::LEFT_BOTTOM,
                    project.fps.timecode(clip.duration),
                    egui::FontId::monospace(12.0),
                    Color32::from_white_alpha(170),
                );
            }
        }
        // A linked clip shows a small chain after its name.
        if clip.link.is_some() && rect.width() > 40.0 {
            let c = rect.left_top() + vec2(rect.width().min(160.0) - 14.0, 9.0);
            let stroke = egui::Stroke::new(1.3, Color32::from_white_alpha(210));
            for dx in [-2.5, 2.5] {
                painter.rect_stroke(
                    Rect::from_center_size(c + vec2(dx, 0.0), vec2(7.0, 5.0)),
                    2.5,
                    stroke,
                    egui::StrokeKind::Middle,
                );
            }
        }
        // A picture whose sound has its own clip shows no waveform.
        if rect.height() > 36.0 && rect.width() > 8.0 && clip.sound {
            if let Some(MediaSource::Video { path } | MediaSource::Audio { path }) =
                project.media(clip.media).map(|m| &m.source)
            {
                if let Some(peaks) = self.sound.peaks(clip.media, path) {
                    paint_waveform(painter, rect, project, clip, &peaks, dim);
                }
            }
        }
        if clip.is_graded() {
            painter.circle_filled(rect.right_top() + vec2(-8.0, 8.0), 3.5, theme::ACCENT);
        }
        if clip.locked {
            paint_lock(painter, rect.right_bottom() + vec2(-10.0, -10.0));
        }
        paint_keys(painter, rect, clip);
        if (clip.speed - 1.0).abs() > 1e-4 && rect.width() > 60.0 {
            painter.text(
                rect.right_top() + vec2(-8.0, 2.0),
                Align2::RIGHT_TOP,
                format!("{:.0}%", clip.speed * 100.0),
                egui::FontId::monospace(12.0),
                Color32::from_white_alpha(200),
            );
        }
        if clip.opacity < 1.0 {
            painter.text(
                rect.right_bottom() + vec2(-5.0, -4.0),
                Align2::RIGHT_BOTTOM,
                format!("{:.0}%", clip.opacity * 100.0),
                egui::FontId::monospace(12.0),
                Color32::from_white_alpha(170),
            );
        }
        let selected = self.is_selected(id);
        let stroke = if selected {
            Stroke::new(2.0, Color32::WHITE)
        } else if track.locked {
            Stroke::new(1.0, theme::WARN)
        } else {
            Stroke::new(1.0, Color32::from_black_alpha(120))
        };
        painter.rect_stroke(rect, 4.0, stroke, StrokeKind::Inside);
    }
}

impl StudioApp {
    /// A clip on the Cut page's detail timeline, as Resolve draws it: the picture as a film
    /// strip with the clip's length in the corner, the sound as a blue strip with a white
    /// waveform right under it.
    fn paint_cut_clip(
        &self,
        painter: &egui::Painter,
        project: &Project,
        rect: Rect,
        id: ClipId,
        dim: bool,
        thumb: Option<egui::TextureId>,
    ) {
        let Some((track, clip)) = project.clip(id) else {
            return;
        };
        let dim = dim || !clip.enabled;
        let shade = |c: Color32| if dim { c.gamma_multiply(0.5) } else { c };
        let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
        let selected = self.is_selected(id);
        if track.kind == TrackKind::Audio {
            painter.rect_filled(rect, 0.0, shade(clip_color(clip).unwrap_or(CUT_BLUE)));
            painter.line_segment(
                [rect.left_top(), rect.right_top()],
                Stroke::new(1.0, Color32::from_rgb(14, 31, 48)),
            );
            if let Some(MediaSource::Video { path } | MediaSource::Audio { path }) =
                project.media(clip.media).map(|m| &m.source)
            {
                if let Some(peaks) = self.sound.peaks(clip.media, path) {
                    // The waveform fills the lower half of the strip.
                    let wave = Rect::from_min_max(
                        pos2(rect.left(), rect.center().y - 1.0),
                        rect.max - vec2(0.0, 1.0),
                    );
                    let area =
                        Rect::from_min_max(wave.min - vec2(0.0, 19.0), wave.max + vec2(0.0, 3.0));
                    paint_waveform_in(&painter, area, project, clip, &peaks, dim, true);
                }
            }
            // The clip's length, without the hours, in a dark box with a link mark.
            if rect.width() > 70.0 {
                let length = project.fps.timecode(clip.duration);
                let length = length.get(3..).unwrap_or(&length).to_string();
                let tag = Rect::from_min_size(
                    pos2(rect.left() + 1.5, rect.top() + 1.5),
                    vec2(58.0, rect.height() - 3.0),
                );
                painter.rect_filled(tag, 1.0, Color32::from_rgb(28, 44, 58));
                let c = pos2(tag.left() + 8.0, tag.center().y);
                let link = Stroke::new(1.0, Color32::from_white_alpha(200));
                for (dx, dy) in [(-1.6, 1.6), (1.6, -1.6)] {
                    painter.rect_stroke(
                        Rect::from_center_size(c + vec2(dx, dy), vec2(5.5, 4.0))
                            .rotate_bb(egui::emath::Rot2::from_angle(-0.785)),
                        1.6,
                        link,
                        StrokeKind::Middle,
                    );
                }
                painter.text(
                    pos2(tag.left() + 15.0, tag.center().y),
                    Align2::LEFT_CENTER,
                    length,
                    regular(9.5),
                    Color32::from_rgb(222, 232, 242),
                );
            }
        } else {
            painter.rect_filled(rect, 0.0, shade(Color32::from_rgb(20, 20, 20)));
            if let Some(texture) = thumb {
                let w = (rect.height() * 16.0 / 9.0).max(8.0);
                let mut x = rect.left();
                while x < rect.right() {
                    let tile = Rect::from_min_size(pos2(x, rect.top()), vec2(w, rect.height()));
                    painter.image(
                        texture,
                        tile,
                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        if dim {
                            Color32::from_gray(110)
                        } else {
                            Color32::WHITE
                        },
                    );
                    x += w;
                }
            }
            paint_keys(&painter, rect, clip);
        }
        if clip.locked {
            paint_lock(&painter, rect.right_center() + vec2(-12.0, 0.0));
        }
        // Resolve draws a picture clip with a blue line along its top; a selected clip gets an
        // orange outline.
        if selected {
            painter.rect_stroke(
                rect,
                0.0,
                Stroke::new(2.0, Color32::from_rgb(232, 120, 60)),
                StrokeKind::Inside,
            );
        } else if track.kind != TrackKind::Audio {
            painter.line_segment(
                [
                    rect.left_top() + vec2(0.0, 0.5),
                    rect.right_top() + vec2(0.0, 0.5),
                ],
                Stroke::new(1.5, Color32::from_rgb(74, 108, 136)),
            );
        }
    }

    /// A clip on the Edit page: video as a strip of pictures over a blue name bar, sound as a
    /// green waveform over a green name bar.
    pub(crate) fn paint_edit_clip(
        &self,
        painter: &egui::Painter,
        project: &Project,
        rect: Rect,
        id: ClipId,
        dim: bool,
        thumb: Option<egui::TextureId>,
    ) {
        let Some((track, clip)) = project.clip(id) else {
            return;
        };
        let dim = dim || !clip.enabled;
        let audio = track.kind == TrackKind::Audio;
        let (body, bar) = if audio {
            (
                Color32::from_rgb(58, 150, 98),
                Color32::from_rgb(52, 138, 90),
            )
        } else {
            (
                Color32::from_rgb(24, 24, 28),
                Color32::from_rgb(64, 120, 186),
            )
        };
        // A clip colour paints the name bar.
        let bar = clip_color(clip).unwrap_or(bar);
        let shade = |c: Color32| if dim { c.gamma_multiply(0.5) } else { c };
        let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
        painter.rect_filled(rect, 2.0, shade(body));
        let bar_h = (rect.height() * 0.22).clamp(14.0, 24.0);
        let pictures = Rect::from_min_max(rect.min, pos2(rect.right(), rect.bottom() - bar_h));
        if !audio {
            // The picture repeated along the clip, as a film strip.
            if let Some(texture) = thumb {
                let w = (pictures.height() * 16.0 / 9.0).max(8.0);
                let mut x = pictures.left();
                while x < pictures.right() {
                    let tile =
                        Rect::from_min_size(pos2(x, pictures.top()), vec2(w, pictures.height()));
                    painter.image(
                        texture,
                        tile,
                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        if dim {
                            Color32::from_gray(110)
                        } else {
                            Color32::WHITE
                        },
                    );
                    x += w;
                }
            }
        } else if clip.sound || track.kind == TrackKind::Audio {
            if let Some(MediaSource::Video { path } | MediaSource::Audio { path }) =
                project.media(clip.media).map(|m| &m.source)
            {
                if let Some(peaks) = self.sound.peaks(clip.media, path) {
                    let area = Rect::from_min_max(pictures.min - vec2(0.0, 19.0), pictures.max);
                    paint_waveform(&painter, area, project, clip, &peaks, dim);
                }
            }
        }
        // The name bar: a link mark, the file's name and the clip's length.
        let bar_rect = Rect::from_min_max(pos2(rect.left(), rect.bottom() - bar_h), rect.max);
        painter.rect_filled(bar_rect, 0.0, shade(bar));
        let name = project
            .media(clip.media)
            .map_or_else(|| clip.name.clone(), |m| m.name.clone());
        let mut x = rect.left() + 8.0;
        if clip.link.is_some() {
            let c = pos2(x + 6.0, bar_rect.center().y);
            for (dx, dy) in [(-2.0, 2.0), (2.0, -2.0)] {
                painter.rect_stroke(
                    Rect::from_center_size(c + vec2(dx, dy), vec2(7.0, 5.0))
                        .rotate_bb(egui::emath::Rot2::from_angle(-0.785)),
                    2.5,
                    Stroke::new(1.3, Color32::from_white_alpha(220)),
                    StrokeKind::Middle,
                );
            }
            x += 20.0;
        }
        let length = project.fps.timecode(clip.duration);
        let length = length.get(3..).unwrap_or(&length).to_string();
        painter.text(
            pos2(x, bar_rect.center().y),
            Align2::LEFT_CENTER,
            format!("{name}  {length}"),
            regular(14.0),
            Color32::from_white_alpha(if dim { 140 } else { 240 }),
        );
        if clip.locked {
            paint_lock(&painter, bar_rect.right_center() + vec2(-12.0, 0.0));
        }
        paint_keys(&painter, pictures, clip);
        let selected = self.is_selected(id);
        let stroke = if selected {
            Stroke::new(2.0, Color32::from_rgb(232, 120, 60))
        } else if track.locked {
            Stroke::new(1.0, theme::WARN)
        } else {
            Stroke::new(1.0, Color32::from_black_alpha(140))
        };
        painter.rect_stroke(rect, 2.0, stroke, StrokeKind::Inside);
    }
}

fn ghost(painter: &egui::Painter, rect: Rect, ok: bool) {
    let color = if ok { theme::ACCENT } else { theme::ERROR };
    painter.rect_filled(rect, 4.0, color.gamma_multiply(0.35));
    painter.rect_stroke(rect, 4.0, Stroke::new(2.0, color), StrokeKind::Inside);
}

/// Draws the sound of a clip as a mirrored waveform under its title. `peaks` holds one byte per
/// fiftieth of a second of the file.
/// The colour a clip was given (Mark > Set Clip Color).
fn clip_color(clip: &Clip) -> Option<Color32> {
    let (_, rgb) = picasso_core::CLIP_COLORS.get(usize::from(clip.color?))?;
    Some(Color32::from_rgb(rgb[0], rgb[1], rgb[2]))
}

/// Small diamonds along the bottom of a clip where it has keyframes.
pub(crate) fn paint_keys(painter: &egui::Painter, rect: Rect, clip: &Clip) {
    if clip.keyframes.is_empty() || rect.width() < 12.0 {
        return;
    }
    let per_frame = rect.width() / clip.duration.max(1) as f32;
    let y = rect.bottom() - 6.0;
    let mut drawn: Vec<u64> = Vec::new();
    for key in clip.keyframes.iter().flat_map(|t| t.keys.iter()) {
        if drawn.contains(&key.at) || key.at >= clip.duration {
            continue;
        }
        drawn.push(key.at);
        let c = egui::pos2(rect.left() + (key.at as f32 + 0.5) * per_frame, y);
        let r = 4.0;
        painter.add(egui::Shape::convex_polygon(
            vec![
                c + vec2(0.0, -r),
                c + vec2(r, 0.0),
                c + vec2(0.0, r),
                c + vec2(-r, 0.0),
            ],
            Color32::from_rgb(232, 120, 60),
            Stroke::new(1.0, Color32::from_black_alpha(160)),
        ));
        if drawn.len() > 500 {
            break;
        }
    }
}

/// A small padlock centred on `c`, on a locked clip.
pub(crate) fn paint_lock(painter: &egui::Painter, c: egui::Pos2) {
    let body = Rect::from_center_size(c + vec2(0.0, 2.0), vec2(9.0, 7.0));
    painter.rect_filled(body, 1.0, Color32::from_white_alpha(230));
    let shackle = Rect::from_center_size(c + vec2(0.0, -2.5), vec2(6.0, 7.0));
    painter.rect_stroke(
        shackle,
        3.0,
        Stroke::new(1.4, Color32::from_white_alpha(230)),
        StrokeKind::Middle,
    );
}

fn paint_waveform(
    painter: &egui::Painter,
    rect: Rect,
    project: &Project,
    clip: &Clip,
    peaks: &[u8],
    dim: bool,
) {
    paint_waveform_in(painter, rect, project, clip, peaks, dim, false);
}

/// The waveform of `clip` in `rect` (less a name bar at the top), centred, or rising from the
/// bottom with `from_bottom`.
fn paint_waveform_in(
    painter: &egui::Painter,
    rect: Rect,
    project: &Project,
    clip: &Clip,
    peaks: &[u8],
    dim: bool,
    from_bottom: bool,
) {
    let area = Rect::from_min_max(rect.min + vec2(0.0, 19.0), rect.max - vec2(0.0, 3.0));
    if area.height() < 6.0 {
        return;
    }
    let per_frame = f64::from(project.fps.den) / f64::from(project.fps.num.max(1));
    let first = clip.source_in as f64 * per_frame * 50.0; // in buckets
    let span = clip.duration as f64 * per_frame * 50.0;
    let columns = (rect.width() as usize).clamp(1, 4000);
    let (mid, half) = if from_bottom {
        (area.bottom(), area.height())
    } else {
        (area.center().y, area.height() * 0.5)
    };
    let color = if from_bottom {
        Color32::from_rgba_unmultiplied(224, 237, 253, if dim { 110 } else { 255 })
    } else {
        Color32::from_white_alpha(if dim { 70 } else { 150 })
    };
    let mut lines = Vec::with_capacity(columns);
    for col in 0..columns {
        let a = first + span * col as f64 / columns as f64;
        let b = (first + span * (col + 1) as f64 / columns as f64).max(a + 1.0);
        let (from, to) = (a as usize, (b.ceil() as usize).min(peaks.len()));
        let peak = peaks
            .get(from..to.max(from))
            .and_then(|s| s.iter().max())
            .copied()
            .unwrap_or(0);
        if peak == 0 {
            continue;
        }
        let x = rect.left() + rect.width() * (col as f32 + 0.5) / columns as f32;
        // A gentle curve so quiet sound is still visible.
        let h = (f32::from(peak) / 255.0)
            .powf(0.6)
            .mul_add(half, 0.0)
            .max(0.5);
        let bottom = if from_bottom { mid } else { mid + h };
        lines.push(egui::Shape::line_segment(
            [egui::pos2(x, mid - h), egui::pos2(x, bottom)],
            Stroke::new(1.0, color),
        ));
    }
    painter.extend(lines);
}

#[cfg(test)]
mod drop_tests {
    use super::*;

    /// One video row (V1) and one audio row (A1) at the bottom, as on the Cut page's detail.
    fn cut_detail() -> Geo {
        let mut order = [0u8; MAX_ROWS];
        let mut row_of = [HIDDEN_ROW; MAX_ROWS];
        order[0] = 0;
        order[1] = 1;
        row_of[0] = 0;
        row_of[1] = 1;
        Geo {
            left: 0.0,
            top: 0.0,
            zoom: 1.0,
            scroll: 0.0,
            tracks: 2,
            ruler_h: 20.0,
            row_h: 100.0,
            order,
            row_of,
            videos: 1,
            body_top: 500.0,
            audio_h: 30.0,
        }
    }

    #[test]
    fn a_drop_above_the_rows_goes_to_the_nearest_track() {
        let geo = cut_detail();
        assert_eq!(geo.track_at(200.0), None);
        assert_eq!(geo.nearest_track(200.0), Some(0));
        assert_eq!(geo.nearest_track(550.0), Some(0));
        assert_eq!(geo.nearest_track(615.0), Some(1));
        assert_eq!(geo.nearest_track(900.0), Some(1));
    }
}
