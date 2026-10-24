//! Palette, fonts, widget style and design-grid scaling.
//!
//! The layout is designed on a 1872-point-wide grid; the interface is scaled
//! so the window is always that wide, keeping every proportion fixed. Colours
//! and row heights were measured from a reference capture of a professional
//! NLE at 1872×999.

use egui::{Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle, Visuals};
use std::sync::Arc;

pub const DESIGN_WIDTH: f32 = 1872.0;

// Measured row heights (points).
pub const MENU_H: f32 = 38.0;
pub const TOOLBAR_H: f32 = 34.0;
pub const HEADER_H: f32 = 32.0;
pub const PAGEBAR_H: f32 = 39.0;
pub const TRANSPORT_H: f32 = 40.0;
pub const TL_TOOLBAR_H: f32 = 34.0;
pub const DIVIDER_W: f32 = 2.0;

// Chrome.
pub const MENU_BG: Color32 = Color32::from_rgb(23, 25, 26);
pub const TOOLBAR_BG: Color32 = Color32::from_rgb(24, 24, 26);
pub const HEADER_BG: Color32 = Color32::from_rgb(34, 33, 38);
pub const PAGEBAR_BG: Color32 = Color32::from_rgb(23, 25, 26);
pub const TAB_SELECTED_BG: Color32 = Color32::from_rgb(3, 3, 3);
pub const DIVIDER: Color32 = Color32::from_rgb(11, 11, 15);
pub const SEPARATOR: Color32 = Color32::from_rgb(17, 17, 21);

// Panels.
pub const POOL_BG: Color32 = Color32::from_rgb(41, 40, 47);
pub const BINS_BG: Color32 = Color32::from_rgb(40, 40, 46);
pub const VIEWER_BG: Color32 = Color32::from_rgb(14, 14, 16);
pub const TRANSPORT_BG: Color32 = Color32::from_rgb(34, 33, 38);
pub const TIMELINE_BG: Color32 = Color32::from_rgb(30, 29, 34);
pub const TRACK_HEADER_BG: Color32 = Color32::from_rgb(36, 35, 41);
pub const TRACK_LANE_BG: Color32 = Color32::from_rgb(30, 29, 34);
pub const TRACK_LANE_ALT: Color32 = Color32::from_rgb(33, 32, 37);
pub const RULER_BG: Color32 = Color32::from_rgb(27, 26, 31);
pub const INSPECTOR_BG: Color32 = Color32::from_rgb(34, 33, 38);
pub const FIELD_BG: Color32 = Color32::from_rgb(24, 24, 28);

// Text and icons.
pub const MENU_TEXT: Color32 = Color32::from_rgb(140, 145, 146);
pub const TEXT: Color32 = Color32::from_rgb(214, 216, 220);
pub const TEXT_DIM: Color32 = Color32::from_rgb(128, 128, 134);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(86, 86, 92);
pub const ICON: Color32 = Color32::from_rgb(130, 130, 134);
pub const ICON_DIM: Color32 = Color32::from_rgb(64, 63, 68);
pub const ICON_HOVER: Color32 = Color32::from_rgb(196, 196, 200);
pub const ICON_ACTIVE: Color32 = Color32::from_rgb(232, 232, 236);
pub const EMPTY_TITLE: Color32 = Color32::from_rgb(71, 71, 82);
pub const EMPTY_SUB: Color32 = Color32::from_rgb(66, 70, 80);
pub const RULER_TEXT: Color32 = Color32::from_rgb(110, 110, 116);
pub const RULER_TICK: Color32 = Color32::from_rgb(66, 66, 70);
pub const TIMECODE: Color32 = Color32::from_rgb(235, 236, 238);

// Accents.
pub const ACCENT: Color32 = Color32::from_rgb(58, 138, 226);
pub const MARKER_BLUE: Color32 = Color32::from_rgb(0, 126, 230);
pub const PLAYHEAD: Color32 = Color32::from_rgb(231, 50, 48);
pub const SELECT_ORANGE: Color32 = Color32::from_rgb(232, 150, 56);
pub const TOOL_RED: Color32 = Color32::from_rgb(229, 64, 54);
pub const WARN: Color32 = Color32::from_rgb(232, 168, 56);
pub const ERROR: Color32 = Color32::from_rgb(235, 96, 96);
pub const OK: Color32 = Color32::from_rgb(110, 200, 120);

// Clips.
pub const VIDEO_CLIP: Color32 = Color32::from_rgb(46, 92, 133);
pub const VIDEO_CLIP_LABEL: Color32 = Color32::from_rgb(58, 118, 168);
pub const AUDIO_CLIP: Color32 = Color32::from_rgb(33, 107, 74);
pub const AUDIO_CLIP_LABEL: Color32 = Color32::from_rgb(30, 143, 97);
pub const AUDIO_WAVE: Color32 = Color32::from_rgb(118, 214, 164);
pub const CLIP_DISABLED: Color32 = Color32::from_rgb(64, 64, 70);

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

pub fn regular(size: f32) -> FontId {
    FontId::proportional(size)
}

pub fn install(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "noto-regular".into(),
        Arc::new(FontData::from_static(include_bytes!("../assets/fonts/NotoSans-Regular.ttf"))),
    );
    fonts.font_data.insert(
        "noto-semibold".into(),
        Arc::new(FontData::from_static(include_bytes!("../assets/fonts/NotoSans-SemiBold.ttf"))),
    );
    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
        family.insert(0, "noto-regular".into());
    }
    fonts.families.insert(FontFamily::Name("semibold".into()), vec!["noto-semibold".into(), "noto-regular".into()]);
    ctx.set_fonts(fonts);

    let mut v = Visuals::dark();
    v.panel_fill = HEADER_BG;
    v.window_fill = HEADER_BG;
    v.extreme_bg_color = FIELD_BG;
    v.faint_bg_color = POOL_BG;
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = Color32::from_rgb(40, 96, 160);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.window_stroke = Stroke::new(1.0, DIVIDER);
    v.menu_corner_radius = CornerRadius::same(4);
    let radius = CornerRadius::same(3);
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = radius;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, DIVIDER);
    v.widgets.inactive.bg_fill = Color32::from_rgb(52, 51, 58);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(46, 45, 52);
    v.widgets.hovered.bg_fill = Color32::from_rgb(66, 65, 73);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(60, 59, 67);
    v.widgets.active.bg_fill = Color32::from_rgb(40, 96, 160);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(40, 96, 160);
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.set_visuals_of(egui::Theme::Dark, v);

    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.text_styles = [
        (TextStyle::Small, regular(13.0)),
        (TextStyle::Body, regular(15.0)),
        (TextStyle::Button, regular(15.0)),
        (TextStyle::Heading, semibold(19.0)),
        (TextStyle::Monospace, FontId::monospace(14.0)),
    ]
    .into();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 4.0);
    style.spacing.interact_size.y = 26.0;
    ctx.set_style_of(egui::Theme::Dark, style);
}

/// Scales the interface so the window is [`DESIGN_WIDTH`] points wide.
pub fn fit_to_design_width(ctx: &Context) {
    let physical = ctx.content_rect().width() * ctx.pixels_per_point();
    let native = ctx.native_pixels_per_point().unwrap_or(1.0);
    if physical < 64.0 {
        return;
    }
    let zoom = physical / DESIGN_WIDTH / native;
    if (zoom - ctx.zoom_factor()).abs() > 0.002 {
        ctx.set_zoom_factor(zoom);
    }
}
