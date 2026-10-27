//! The sizes of the panels the user can drag the dividers between, and the divider itself.
//!
//! Sizes are kept in Resolve's 1080p pixels (columns) and thousandths (how two panels share a
//! height), so they read the same at every window size. They are saved with the preferences.
//!
//! The Edit and Cut pages move their own columns and rows. The other pages are drawn through a
//! `Screen` with a `Warp`: each divider moves its line and stretches what lies between it and the
//! boundaries on both sides, so a page only says where its dividers are (`Split`).

use std::collections::BTreeMap;

use egui::{CursorIcon, Id, Pos2, Rect, Sense, Ui};

use crate::app::StudioApp;
use crate::chrome::Screen;
use crate::theme;

/// Edit page: where the bins, the media pool and the source viewer end, in Resolve's pixels.
const EDIT_COLUMNS: [u16; 3] = [147, 457, 1186];
/// Edit page: the right edge of the page in Resolve's pixels.
const EDIT_RIGHT: u16 = 1915;
/// Edit page: the narrowest the bins, the media pool and each viewer may get.
const EDIT_MIN: [u16; 4] = [110, 230, 450, 450];
/// Cut page: where the media pool ends and the level meter starts.
const CUT_POOL: u16 = 960;
const CUT_METER: u16 = 1850;
/// Cut page: the narrowest the media pool and the viewer may get.
const CUT_MIN: [u16; 2] = [400, 450];
/// How much of a shared height a panel may take, in thousandths.
const SHARE: (u16, u16) = (250, 750);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Panels {
    /// Edit page: where the bins, the media pool and the source viewer end.
    pub edit_columns: [u16; 3],
    /// Edit page: the viewers' share of the height they and the tracks divide.
    pub edit_viewers: u16,
    /// Cut page: where the media pool ends.
    pub cut_pool: u16,
    /// Cut page: the media pool and viewer's share of the height they and the detail timeline
    /// divide.
    pub cut_upper: u16,
    /// The other pages: how far each divider was dragged from its usual place, in points, by
    /// the divider's name.
    pub moved: BTreeMap<String, i16>,
}

impl Default for Panels {
    fn default() -> Self {
        Self {
            edit_columns: EDIT_COLUMNS,
            // The reference's viewers are 311 pixels high and its tracks 345.
            edit_viewers: 474,
            cut_pool: CUT_POOL,
            // The reference's pool band is 378 pixels high and its detail timeline 350.
            cut_upper: 519,
            moved: BTreeMap::new(),
        }
    }
}

impl Panels {
    /// Moves the Edit page's divider `i` (0: bins | pool, 1: pool | source viewer, 2: source
    /// viewer | timeline viewer) to `x`, as far as the panels on both sides allow.
    pub(crate) fn set_edit_column(&mut self, i: usize, x: f32) {
        let c = self.edit_columns;
        let left = (if i == 0 { 0 } else { c[i - 1] }) + EDIT_MIN[i];
        let right = (if i == 2 { EDIT_RIGHT } else { c[i + 1] }).saturating_sub(EDIT_MIN[i + 1]);
        self.edit_columns[i] = clamp(x, left, right);
    }

    pub(crate) fn set_cut_pool(&mut self, x: f32) {
        self.cut_pool = clamp(x, CUT_MIN[0], CUT_METER - CUT_MIN[1]);
    }

    /// Sets a share from the part of the height the first panel takes (0 to 1).
    pub(crate) fn share(part: f32) -> u16 {
        clamp(part * 1000.0, SHARE.0, SHARE.1)
    }

    /// The same sizes with every value in its range, for sizes read from a file.
    pub(crate) fn checked(mut self) -> Self {
        let c = self.edit_columns.map(u32::from);
        let min = EDIT_MIN.map(u32::from);
        let ordered = c[0] >= min[0]
            && c[1] >= c[0] + min[1]
            && c[2] >= c[1] + min[2]
            && u32::from(EDIT_RIGHT) >= c[2] + min[3];
        if !ordered {
            self.edit_columns = EDIT_COLUMNS;
        }
        self.edit_viewers = self.edit_viewers.clamp(SHARE.0, SHARE.1);
        self.cut_pool = self.cut_pool.clamp(CUT_MIN[0], CUT_METER - CUT_MIN[1]);
        self.cut_upper = self.cut_upper.clamp(SHARE.0, SHARE.1);
        self
    }
}

/// A divider of a page drawn through a warped `Screen`, in points from the screen's origin.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Split {
    pub id: &'static str,
    /// A horizontal line, dragged up and down; otherwise a vertical one, dragged sideways.
    pub across: bool,
    /// Where the line usually is.
    pub at: f32,
    /// The boundaries before and after it: what lies between them stretches as it moves.
    pub a: f32,
    pub b: f32,
    /// The stretch of the other axis the line runs along, where the stretching applies.
    pub lo: f32,
    pub hi: f32,
    /// How far past the line the rows or columns that keep their size go (a transport row
    /// under a viewer): they move with the line.
    pub keep: f32,
}

impl Split {
    /// The same divider with `len` points after its line moving with it unstretched.
    pub(crate) fn keeping(self, len: f32) -> Self {
        Split { keep: len, ..self }
    }

    /// A vertical divider from `top` to `bottom` (where the page draws them, in window points),
    /// between the columns starting at `left` and ending at `right`.
    pub(crate) fn column(
        id: &'static str,
        s: &Screen,
        top: Pos2,
        bottom: Pos2,
        left: f32,
        right: f32,
    ) -> Self {
        Split {
            id,
            across: false,
            at: top.x - s.o.x,
            a: left - s.o.x,
            b: right - s.o.x,
            lo: top.y - s.o.y,
            hi: bottom.y - s.o.y,
            keep: 0.0,
        }
    }

    /// A horizontal divider from `left` to `right`, between the rows starting at `top` and
    /// ending at `bottom`.
    pub(crate) fn row(
        id: &'static str,
        s: &Screen,
        left: Pos2,
        right: Pos2,
        top: f32,
        bottom: f32,
    ) -> Self {
        Split {
            id,
            across: true,
            at: left.y - s.o.y,
            a: top - s.o.y,
            b: bottom - s.o.y,
            lo: left.x - s.o.x,
            hi: right.x - s.o.x,
            keep: 0.0,
        }
    }

    /// Where the line is when dragged `moved` points: each panel beside it keeps at least two
    /// fifths of its usual size.
    fn place(&self, moved: f32) -> f32 {
        let lo = self.a + (self.at - self.a) * 0.4;
        let hi = self.b - self.keep - (self.b - self.keep - self.at) * 0.4;
        (self.at + moved).clamp(lo, hi.max(lo))
    }
}

/// Stretches a page's points between its dividers' boundaries so that the dividers land where
/// they were dragged. Nothing outside the boundaries moves. The lines apply one after another,
/// so a divider inside another's panels (the two viewers left of an inspector) moves with it.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Warp {
    lines: [(Split, f32); 8],
    count: usize,
}

/// `v` stretched so that `from` lands on `to`, between `a` and `b`; the `keep` points after the
/// line move with it unstretched.
fn stretch(v: f32, a: f32, from: f32, to: f32, b: f32, keep: f32) -> f32 {
    if v <= from {
        a + (v - a) * (to - a) / (from - a).max(0.01)
    } else if v <= from + keep {
        v + (to - from)
    } else {
        to + keep + (v - from - keep) * (b - to - keep) / (b - from - keep).max(0.01)
    }
}

impl Split {
    /// Whether the line stretches a point whose other coordinate (unmoved) is `other`.
    fn covers(&self, along: f32, other: f32) -> bool {
        (self.lo..=self.hi).contains(&other) && (self.a..=self.b).contains(&along)
    }
}

impl Warp {
    /// Moves the line of `split` to `to`; lines past the eighth stay where they are.
    fn push(&mut self, split: Split, to: f32) {
        if self.count < self.lines.len() {
            self.lines[self.count] = (split, to);
            self.count += 1;
        }
    }

    /// Where a point (from the screen's origin) is drawn.
    pub(crate) fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        self.apply_within(x, y, (x, y))
    }

    /// Where a corner of a rectangle is drawn: whether a line stretches it is decided by the
    /// rectangle's `middle`, so a panel that starts on the row where a divider ends (the album
    /// under the Photo page's pool) is not stretched at one edge and left at the other.
    pub(crate) fn apply_within(&self, x: f32, y: f32, middle: (f32, f32)) -> (f32, f32) {
        let (mut wx, mut wy) = (x, y);
        for (split, to) in &self.lines[..self.count] {
            let (along, other) = if split.across {
                (&mut wy, middle.0)
            } else {
                (&mut wx, middle.1)
            };
            if split.covers(*along, other) {
                *along = stretch(*along, split.a, split.at, *to, split.b, split.keep);
            }
        }
        (wx, wy)
    }

    /// Where line `i` has to be for the lines after it to draw it at `v`.
    fn before_later(&self, i: usize, mut v: f32) -> f32 {
        let (line, _) = self.lines[i];
        let other = (line.lo + line.hi) / 2.0;
        for (split, to) in self.lines[i + 1..self.count].iter().rev() {
            if split.across == line.across && split.covers(v, other) {
                v = stretch(v, split.a, *to, split.at, split.b, split.keep);
            }
        }
        v
    }
}

impl StudioApp {
    /// `s` with the page's dividers where they were dragged.
    pub(crate) fn warped(&self, s: &Screen, splits: &[Split]) -> Screen {
        let mut warp = Warp::default();
        for split in splits {
            let moved = self.extra.prefs.panels.moved.get(split.id).copied();
            warp.push(*split, split.place(f32::from(moved.unwrap_or(0))));
        }
        Screen {
            o: s.o,
            h: s.h,
            warp,
        }
    }

    /// The dividers of a page drawn through `warped` (the screen `warped` gave). Call it after
    /// the page, so they are above what it draws. They move on the next frame.
    pub(crate) fn dividers(&mut self, ui: &mut Ui, warped: &Screen, splits: &[Split]) {
        let mut save = false;
        for (i, split) in splits.iter().enumerate() {
            let (start, end) = if split.across {
                (warped.p(split.lo, split.at), warped.p(split.hi, split.at))
            } else {
                (warped.p(split.at, split.lo), warped.p(split.at, split.hi))
            };
            let moved = &mut self.extra.prefs.panels.moved;
            match divider(ui, split.id, Rect::from_two_pos(start, end), split.across) {
                Divider::Moved(at) => {
                    let to = if split.across {
                        at.y - warped.o.y
                    } else {
                        at.x - warped.o.x
                    };
                    let to = if i < warped.warp.count {
                        warped.warp.before_later(i, to)
                    } else {
                        to
                    };
                    let by = split.place(to - split.at) - split.at;
                    moved.insert(split.id.to_string(), by.round() as i16);
                }
                Divider::Reset => {
                    moved.remove(split.id);
                    save = true;
                }
                Divider::Released => save = true,
                Divider::Still => {}
            }
        }
        if save {
            self.extra.prefs.save();
        }
    }
}

fn clamp(v: f32, lo: u16, hi: u16) -> u16 {
    if v.is_finite() {
        v.round().clamp(f32::from(lo), f32::from(hi.max(lo))) as u16
    } else {
        lo
    }
}

/// What happened to a divider this frame.
pub(crate) enum Divider {
    Still,
    /// Dragged; the pointer is here.
    Moved(egui::Pos2),
    /// Let go after a drag: time to save the sizes.
    Released,
    /// Double-clicked: back to the usual size.
    Reset,
}

/// A divider between two panels along `line` (a thin rectangle on the line between them), which
/// can be dragged. It lights up in the accent colour while the pointer is on it, as in most
/// editors. Call it after the panels, so it is above what they draw.
pub(crate) fn divider(ui: &mut Ui, id: &str, line: Rect, across: bool) -> Divider {
    // Easier to catch than the one-point line it draws.
    let grab = if across {
        line.expand2(egui::vec2(0.0, 3.0))
    } else {
        line.expand2(egui::vec2(3.0, 0.0))
    };
    let response = ui.interact(
        grab,
        Id::new(("panel divider", id)),
        Sense::click_and_drag(),
    );
    let active = response.hovered() || response.dragged();
    if active {
        ui.ctx().set_cursor_icon(if across {
            CursorIcon::ResizeVertical
        } else {
            CursorIcon::ResizeHorizontal
        });
        let lit = if across {
            Rect::from_center_size(line.center(), egui::vec2(line.width(), 2.0))
        } else {
            Rect::from_center_size(line.center(), egui::vec2(2.0, line.height()))
        };
        ui.painter().rect_filled(lit, 0.0, theme::ACCENT);
    }
    if response.double_clicked() {
        Divider::Reset
    } else if response.drag_stopped() {
        Divider::Released
    } else if response.dragged() {
        response
            .interact_pointer_pos()
            .map_or(Divider::Still, Divider::Moved)
    } else {
        Divider::Still
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_divider_stops_where_the_panel_beside_it_is_as_narrow_as_it_may_be() {
        let mut p = Panels::default();
        p.set_edit_column(1, 0.0);
        assert_eq!(p.edit_columns[1], 147 + 230);
        p.set_edit_column(1, 5000.0);
        assert_eq!(p.edit_columns[1], 1186 - 450);
        p.set_edit_column(2, 1700.0);
        assert_eq!(p.edit_columns[2], 1915 - 450);
        p.set_edit_column(0, 120.5);
        assert_eq!(p.edit_columns[0], 121);
        p.set_cut_pool(f32::NAN);
        assert_eq!(p.cut_pool, 400);
        assert_eq!(Panels::share(0.9), 750);
        assert_eq!(Panels::share(0.5), 500);
    }

    #[test]
    fn sizes_out_of_range_are_put_back_in_it() {
        let p = Panels {
            edit_columns: [900, 100, 2000],
            edit_viewers: 5,
            cut_pool: 1900,
            cut_upper: 999,
            moved: BTreeMap::new(),
        }
        .checked();
        assert_eq!(p.edit_columns, EDIT_COLUMNS);
        assert_eq!(p.edit_viewers, 250);
        assert_eq!(p.cut_pool, 1400);
        assert_eq!(p.cut_upper, 750);
        assert_eq!(Panels::default().checked(), Panels::default());
    }

    #[test]
    fn a_warp_moves_the_line_and_stretches_only_between_its_boundaries() {
        let split = Split {
            id: "test",
            across: false,
            at: 100.0,
            a: 0.0,
            b: 300.0,
            lo: 10.0,
            hi: 50.0,
            keep: 0.0,
        };
        let mut warp = Warp::default();
        warp.push(split, split.place(50.0));
        assert_eq!(warp.apply(100.0, 20.0), (150.0, 20.0));
        assert_eq!(warp.apply(50.0, 20.0), (75.0, 20.0));
        assert_eq!(warp.apply(200.0, 20.0), (225.0, 20.0));
        // The boundaries stay, and so does everything above or below the line's rows.
        assert_eq!(warp.apply(0.0, 20.0), (0.0, 20.0));
        assert_eq!(warp.apply(300.0, 20.0), (300.0, 20.0));
        assert_eq!(warp.apply(100.0, 60.0), (100.0, 60.0));
        // A panel keeps two fifths of its size.
        assert_eq!(split.place(-1000.0), 40.0);
        assert_eq!(split.place(1000.0), 220.0);

        // A second line around the first stretches it too, and is undone to find where the
        // first has to be for the pointer.
        let outer = Split {
            id: "outer",
            at: 300.0,
            b: 400.0,
            ..split
        };
        warp.push(outer, 200.0);
        assert_eq!(warp.apply(100.0, 20.0), (100.0, 20.0));
        assert_eq!(warp.before_later(0, 100.0), 150.0);

        // Rows that keep their size move with the line.
        let kept = split.keeping(20.0);
        assert_eq!(stretch(110.0, 0.0, 100.0, 150.0, 300.0, 20.0), 160.0);
        assert_eq!(stretch(160.0, 0.0, 150.0, 100.0, 300.0, 20.0), 110.0);
        assert_eq!(kept.place(1000.0), 300.0 - 20.0 - 180.0 * 0.4);
    }
}
