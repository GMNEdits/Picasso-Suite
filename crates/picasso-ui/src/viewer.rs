//! The viewer: the picture at the playhead.

use egui::{pos2, vec2, Color32, ColorImage, Context, Rect, TextureHandle, TextureOptions, Ui};
use picasso_core::Project;

use crate::actions::Overlays;
use std::sync::Arc;

use crate::render_cache::RenderCache;
use picasso_render::{Frame, FrameKey, MediaCache};

/// The longest side of the picture the viewer renders; exports render at the project's size.
const PREVIEW_SIDE: u32 = 960;

/// What the rendered picture depends on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ViewerKey {
    pub epoch: u64,
    pub revision: u64,
    pub media_generation: u64,
    /// Changes when a video picture arrives from the decoder.
    pub video: u64,
    pub frame: u64,
    pub width: u32,
    pub height: u32,
    /// Whether empty parts are rendered transparent (the checkerboard background).
    pub alpha: bool,
    /// The content of the highlighted copy of the project, when the key is shown.
    pub highlight: u64,
}

/// The size of the preview of a project: its shape, at most [`PREVIEW_SIDE`] on the long side.
pub(crate) fn preview_size(project: &Project) -> (u32, u32) {
    let (w, h) = (project.width.max(1), project.height.max(1));
    let long = w.max(h);
    if long <= PREVIEW_SIDE {
        return (w, h);
    }
    let scale = PREVIEW_SIDE as f32 / long as f32;
    (
        ((w as f32 * scale).round() as u32).max(1),
        ((h as f32 * scale).round() as u32).max(1),
    )
}

#[derive(Default)]
pub(crate) struct Viewer {
    texture: Option<TextureHandle>,
    key: Option<ViewerKey>,
    /// The picture that was last rendered, for the scopes.
    pub frame: Option<Arc<Frame>>,
    /// Counts the pictures rendered, so a scope knows when to measure again.
    pub generation: u64,
}

impl Viewer {
    /// Renders the picture again when what it depends on changed, or takes it from the render
    /// cache.
    pub fn ensure(
        &mut self,
        ctx: &Context,
        project: &Project,
        cache: &MediaCache,
        key: ViewerKey,
        frames: &mut RenderCache,
        frame_key: FrameKey,
    ) {
        if self.key.as_ref() == Some(&key) {
            return;
        }
        let Some(frame) = frames.render(project, cache, frame_key) else {
            return;
        };
        let image = ColorImage::from_rgba_unmultiplied(
            [frame.width as usize, frame.height as usize],
            &frame.rgba,
        );
        match &mut self.texture {
            Some(texture) => texture.set(image, TextureOptions::LINEAR),
            None => self.texture = Some(ctx.load_texture("viewer", image, TextureOptions::LINEAR)),
        }
        self.frame = Some(frame);
        self.generation += 1;
        self.key = Some(key);
    }

    /// Draws the picture as large as fits in `area`, and returns where it is.
    pub fn paint(
        &self,
        ui: &mut Ui,
        area: Rect,
        aspect: f32,
        background: Color32,
        picture: bool,
        overlays: Overlays,
    ) -> Rect {
        let painter = ui.painter_at(area);
        painter.rect_filled(area, 0.0, background);
        let inner = area.shrink(8.0);
        if inner.width() < 4.0 || inner.height() < 4.0 {
            return inner;
        }
        let fit = if inner.width() / inner.height() > aspect {
            vec2(inner.height() * aspect, inner.height())
        } else {
            vec2(inner.width(), inner.width() / aspect)
        };
        let rect = Rect::from_center_size(inner.center(), fit);
        if !picture {
            return rect;
        }
        if overlays.checkerboard {
            draw_checkerboard(&painter, rect);
        } else {
            painter.rect_filled(rect, 0.0, Color32::BLACK);
        }
        if let Some(texture) = &self.texture {
            painter.image(
                texture.id(),
                rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        draw_overlays(&painter, rect, overlays);
        rect
    }
}

/// Grey squares behind a picture with transparent parts.
fn draw_checkerboard(painter: &egui::Painter, rect: Rect) {
    const SQUARE: f32 = 12.0;
    let (light, dark) = (Color32::from_gray(102), Color32::from_gray(64));
    painter.rect_filled(rect, 0.0, dark);
    let (cols, rows) = (
        (rect.width() / SQUARE).ceil() as usize,
        (rect.height() / SQUARE).ceil() as usize,
    );
    for row in 0..rows {
        for col in (row % 2..cols).step_by(2) {
            let min = rect.min + vec2(col as f32 * SQUARE, row as f32 * SQUARE);
            let square = Rect::from_min_size(min, vec2(SQUARE, SQUARE)).intersect(rect);
            painter.rect_filled(square, 0.0, light);
        }
    }
}

/// The guides over the picture: the safe areas, the centre and the thirds.
fn draw_overlays(painter: &egui::Painter, rect: Rect, overlays: Overlays) {
    let stroke = egui::Stroke::new(1.0, Color32::from_white_alpha(150));
    if overlays.safe_area {
        for share in [0.93, 0.9] {
            let r = Rect::from_center_size(rect.center(), rect.size() * share);
            painter.rect_stroke(r, 0.0, stroke, egui::StrokeKind::Inside);
        }
    }
    if overlays.crosshair {
        let c = rect.center();
        painter.line_segment([pos2(c.x - 14.0, c.y), pos2(c.x + 14.0, c.y)], stroke);
        painter.line_segment([pos2(c.x, c.y - 14.0), pos2(c.x, c.y + 14.0)], stroke);
    }
    if overlays.grid {
        for n in 1..3 {
            let f = n as f32 / 3.0;
            let x = rect.left() + rect.width() * f;
            let y = rect.top() + rect.height() * f;
            painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], stroke);
            painter.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], stroke);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_keeps_the_shape_and_caps_the_long_side() {
        let mut p = Project::new("t");
        p.width = 3840;
        p.height = 2160;
        assert_eq!(preview_size(&p), (960, 540));
        p.width = 1080;
        p.height = 1920;
        assert_eq!(preview_size(&p), (540, 960));
        p.width = 640;
        p.height = 360;
        assert_eq!(preview_size(&p), (640, 360));
        p.width = 16_000;
        p.height = 1;
        assert_eq!(preview_size(&p), (960, 1));
    }
}
