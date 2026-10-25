#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod chrome;
mod commands;
mod cut_page;
mod deliver_page;
mod edit_page;
mod icons;
mod keymap;
mod theme;
mod timeline_ui;
mod workers;

fn main() -> eframe::Result {
    env_logger::init();
    let open = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(app::APP_NAME)
        .with_app_id("picasso-suite")
        .with_decorations(false)
        .with_inner_size([1600.0, 900.0])
        .with_min_inner_size([960.0, 560.0]);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")) {
        viewport = viewport.with_icon(icon);
    }
    if let Some((w, h)) = std::env::var("PICASSO_WINDOW_SIZE").ok().and_then(|s| {
        let (w, h) = s.split_once('x')?;
        Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?))
    }) {
        viewport = viewport.with_inner_size([w, h]);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(app::APP_NAME, options, Box::new(move |cc| Ok(Box::new(app::PicassoApp::new(cc, open)))))
}
