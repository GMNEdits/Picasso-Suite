//! Colours, fonts and the look of widgets.
//!
//! The palette was measured from the reference screen of the Cut page, so the chrome matches
//! it. Sizes are in points at a window width of 1872 points (see `app.rs`: the interface is
//! scaled so the window is always that wide).

use std::sync::Arc;

use egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke,
    TextStyle, Visuals,
};

// ----- the chrome -----
pub const MENU_BG: Color32 = Color32::from_rgb(23, 25, 26);
pub const TOOLBAR1_BG: Color32 = Color32::from_rgb(24, 24, 26);
pub const TOOLBAR2_BG: Color32 = Color32::from_rgb(34, 33, 38);
pub const POOL_BG: Color32 = Color32::from_rgb(41, 40, 47);
pub const POOL_HEADER_BG: Color32 = Color32::from_rgb(34, 33, 38);
pub const VIEWER_BG: Color32 = Color32::from_rgb(26, 26, 26);
pub const METER_BG: Color32 = Color32::from_rgb(34, 33, 38);
pub const TRANSPORT_BG: Color32 = Color32::from_rgb(34, 33, 38);
pub const RAIL_BG: Color32 = Color32::from_rgb(34, 33, 38);
pub const TIMELINE_BG: Color32 = Color32::from_rgb(30, 29, 34);
pub const PAGEBAR_BG: Color32 = Color32::from_rgb(23, 25, 26);
pub const WINDOW_BORDER: Color32 = Color32::BLACK;
/// The dark lines between panels.
pub const DIVIDER: Color32 = Color32::from_rgb(11, 11, 15);
pub const SEPARATOR_DARK: Color32 = Color32::from_rgb(9, 10, 12);
pub const SEPARATOR_SOFT: Color32 = Color32::from_rgb(17, 17, 21);

pub const ICON: Color32 = Color32::from_rgb(130, 130, 134);
pub const ICON_DIM: Color32 = Color32::from_rgb(64, 63, 68);
pub const ICON_HOVER: Color32 = Color32::from_rgb(190, 190, 194);
pub const ICON_BRIGHT: Color32 = Color32::from_rgb(206, 205, 208);
pub const TRANSPORT_ICON: Color32 = Color32::from_rgb(135, 135, 136);
pub const MENU_TEXT: Color32 = Color32::from_rgb(140, 145, 146);
pub const WHITE: Color32 = Color32::from_rgb(255, 255, 255);
pub const EMPTY_TITLE: Color32 = Color32::from_rgb(71, 71, 82);
pub const EMPTY_SUB: Color32 = Color32::from_rgb(66, 70, 80);
pub const RULER_LABEL_OVERVIEW: Color32 = Color32::from_rgb(146, 145, 148);
pub const RULER_TICK: Color32 = Color32::from_rgb(66, 66, 70);
pub const PLAYHEAD_GRAY: Color32 = Color32::from_rgb(147, 147, 147);
pub const MARKER_BLUE: Color32 = Color32::from_rgb(0, 126, 230);
pub const TIMECODE_BOX_BG: Color32 = Color32::from_rgb(14, 14, 16);
pub const TIMECODE_TEXT: Color32 = Color32::from_rgb(235, 253, 254);

// ----- general widgets (the pages that are not the Cut page) -----
pub const PANEL: Color32 = Color32::from_rgb(34, 33, 38);
pub const PANEL_DEEP: Color32 = Color32::from_rgb(26, 26, 26);
pub const LINE: Color32 = Color32::from_rgb(58, 57, 63);
pub const TEXT: Color32 = Color32::from_rgb(214, 216, 220);
pub const TEXT_DIM: Color32 = Color32::from_rgb(128, 128, 134);
pub const ACCENT: Color32 = Color32::from_rgb(58, 138, 226);
pub const WARN: Color32 = Color32::from_rgb(232, 168, 56);
/// A soloed track's button, as Resolve lights it.
pub const SOLO: Color32 = Color32::from_rgb(214, 170, 40);
/// The active timeline tool (the selection arrow), as in DaVinci Resolve.
pub const TOOL_RED: Color32 = Color32::from_rgb(230, 75, 61);
pub const ERROR: Color32 = Color32::from_rgb(235, 96, 96);

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

pub fn regular(size: f32) -> FontId {
    FontId::proportional(size)
}

/// The fonts for a language: Noto Sans for the Latin, Greek and Cyrillic scripts, and behind it the
/// Chinese, Japanese, Korean, Arabic, Hindi, Bengali and Thai ones (the characters those languages need: `assets/fonts`), so a
/// language name or a file name in any of them is drawn. The Han characters of the Chinese and the
/// Japanese font differ in shape: the one of the language in use comes first.
pub fn fonts_for(language: crate::i18n::Language) -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let mut add = |name: &str, bytes: &'static [u8]| {
        fonts
            .font_data
            .insert(name.into(), Arc::new(FontData::from_static(bytes)));
    };
    add("noto-regular", include_bytes!("../assets/fonts/NotoSans-Regular.ttf"));
    add("noto-semibold", include_bytes!("../assets/fonts/NotoSans-SemiBold.ttf"));
    add("noto-sc", include_bytes!("../assets/fonts/NotoSansSC-Regular.subset.ttf"));
    add("noto-jp", include_bytes!("../assets/fonts/NotoSansJP-Regular.subset.ttf"));
    add("noto-arabic", include_bytes!("../assets/fonts/NotoSansArabic-Regular.subset.ttf"));
    add("noto-deva", include_bytes!("../assets/fonts/NotoSansDevanagari-Regular.subset.ttf"));
    add("noto-kr", include_bytes!("../assets/fonts/NotoSansKR-Regular.subset.ttf"));
    add("noto-bn", include_bytes!("../assets/fonts/NotoSansBengali-Regular.subset.ttf"));
    add("noto-th", include_bytes!("../assets/fonts/NotoSansThai-Regular.subset.ttf"));
    let han: [&str; 2] = if crate::i18n::japanese_fonts(language) {
        ["noto-jp", "noto-sc"]
    } else {
        ["noto-sc", "noto-jp"]
    };
    let behind = [han[0], han[1], "noto-kr", "noto-arabic", "noto-deva", "noto-bn", "noto-th"];
    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
        family.insert(0, "noto-regular".into());
        for (i, name) in behind.iter().enumerate() {
            family.insert(1 + i, (*name).into());
        }
    }
    let mut semibold: Vec<String> = vec!["noto-semibold".into(), "noto-regular".into()];
    semibold.extend(behind.iter().map(|n| (*n).to_string()));
    fonts
        .families
        .insert(FontFamily::Name("semibold".into()), semibold);
    fonts
}

/// Installs the fonts, the dark theme and the type scale.
pub fn install(ctx: &Context) {
    let language = crate::i18n::current();
    ctx.set_fonts(fonts_for(language));
    crate::i18n::fonts_installed(language);

    let mut v = Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.extreme_bg_color = PANEL_DEEP;
    v.faint_bg_color = POOL_BG;
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = Color32::from_rgb(40, 96, 160);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.window_stroke = Stroke::new(1.0, DIVIDER);
    let radius = CornerRadius::same(3);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
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
        (TextStyle::Small, regular(14.0)),
        (TextStyle::Body, regular(17.0)),
        (TextStyle::Button, regular(17.0)),
        (TextStyle::Heading, semibold(21.0)),
        (TextStyle::Monospace, FontId::monospace(16.0)),
    ]
    .into();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 4.0);
    style.spacing.interact_size.y = 28.0;
    ctx.set_style_of(egui::Theme::Dark, style);
}

/// A colour for a media item, the same every time.
pub fn media_color(id: u64) -> Color32 {
    let hue = (id as f32 * 0.618_034).fract();
    egui::ecolor::Hsva::new(hue, 0.42, 0.52, 1.0).into()
}

/// The eight marker colours: blue, cyan, green, yellow, red, pink, purple, fawn.
/// The colour of a clip on an audio track.
pub const AUDIO_CLIP: Color32 = Color32::from_rgb(52, 118, 110);

pub fn marker_color(index: u8) -> Color32 {
    [
        Color32::from_rgb(52, 120, 236),
        Color32::from_rgb(40, 190, 210),
        Color32::from_rgb(70, 190, 90),
        Color32::from_rgb(232, 200, 50),
        Color32::from_rgb(226, 72, 64),
        Color32::from_rgb(236, 110, 170),
        Color32::from_rgb(150, 100, 220),
        Color32::from_rgb(190, 150, 110),
    ][usize::from(index) % 8]
}
