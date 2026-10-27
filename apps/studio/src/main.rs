//! Picasso Suite: the same app as a desktop program (Windows, Linux, macOS) and as WebAssembly.

// Release builds on Windows start without a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_arch = "wasm32"))]
mod graphics_check;

use picasso_ui::StudioApp;

/// The window: `picasso-ui` draws, eframe runs it.
struct Window {
    app: StudioApp,
    /// `PICASSO_SCREENSHOT=<file.png>`: the app saves a picture of its own window and quits.
    #[cfg(not(target_arch = "wasm32"))]
    shot: Option<(std::path::PathBuf, std::time::Instant, bool)>,
    /// `PICASSO_PERF=1`: frame times in ms, printed after 150 frames, then the window closes.
    #[cfg(not(target_arch = "wasm32"))]
    perf: Option<Vec<f64>>,
}

impl Window {
    fn new(app: StudioApp) -> Self {
        Self {
            app,
            #[cfg(not(target_arch = "wasm32"))]
            shot: std::env::var_os("PICASSO_SCREENSHOT")
                .map(|p| (p.into(), std::time::Instant::now(), false)),
            #[cfg(not(target_arch = "wasm32"))]
            perf: std::env::var_os("PICASSO_PERF").map(|_| Vec::new()),
        }
    }

    /// Asks for the picture after a few frames, saves it when it arrives, then closes the window.
    #[cfg(not(target_arch = "wasm32"))]
    fn take_screenshot(&mut self, ctx: &eframe::egui::Context) {
        use eframe::egui::{Event, UserData, ViewportCommand};
        let Some((path, started, requested)) = &mut self.shot else {
            return;
        };
        ctx.request_repaint();
        // A moment for background work (decoding, waveforms) to finish first.
        if !*requested && started.elapsed() > std::time::Duration::from_secs(2) {
            *requested = true;
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(UserData::default()));
        }
        let image = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            let [w, h] = image.size;
            let saved = image::RgbaImage::from_raw(
                w as u32,
                h as u32,
                image.pixels.iter().flat_map(|c| c.to_array()).collect(),
            )
            .map(|img| img.save(&*path));
            if !matches!(saved, Some(Ok(()))) {
                eprintln!("could not save the screenshot to {}", path.display());
            }
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }
}

impl eframe::App for Window {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        #[cfg(not(target_arch = "wasm32"))]
        let started = std::time::Instant::now();
        self.app.ui(ui);
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(times) = &mut self.perf {
                times.push(started.elapsed().as_secs_f64() * 1000.0);
                ui.ctx().request_repaint();
                if times.len() == 150 {
                    let mut t = times.split_off(30);
                    t.sort_by(|a, b| a.total_cmp(b));
                    let avg = t.iter().sum::<f64>() / t.len() as f64;
                    println!(
                        "frames={} avg={avg:.2}ms p50={:.2}ms p95={:.2}ms max={:.2}ms",
                        t.len(),
                        t[t.len() / 2],
                        t[t.len() * 95 / 100],
                        t[t.len() - 1]
                    );
                    ui.ctx()
                        .send_viewport_cmd(eframe::egui::ViewportCommand::Close);
                }
            }
            self.take_screenshot(&ui.ctx().clone());
        }
    }
}

/// The window's first size in points; `PICASSO_WINDOW_SIZE=1872x999` overrides it (for screenshots, with `PICASSO_SCREENSHOT=<file.png>`).
#[cfg(not(target_arch = "wasm32"))]
fn window_size() -> [f32; 2] {
    std::env::var("PICASSO_WINDOW_SIZE")
        .ok()
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            Some([w.parse().ok()?, h.parse().ok()?])
        })
        .unwrap_or([1500.0, 800.0])
}

/// The window and taskbar picture; a plain window picture is used if it cannot be decoded.
#[cfg(not(target_arch = "wasm32"))]
fn app_icon() -> eframe::egui::IconData {
    let bytes = include_bytes!("../../../assets/icon.png");
    match image::load_from_memory(bytes) {
        Ok(img) => {
            let rgba = img.into_rgba8();
            let (width, height) = rgba.dimensions();
            eframe::egui::IconData {
                rgba: rgba.into_raw(),
                width,
                height,
            }
        }
        Err(_) => eframe::egui::IconData::default(),
    }
}

/// How the window is drawn: with DirectX 12 on Windows, the system's own graphics interface
/// (wgpu would otherwise pick Vulkan there). Linux keeps Vulkan and macOS Metal. Setting
/// `WGPU_BACKEND` still chooses another one.
#[cfg(not(target_arch = "wasm32"))]
fn wgpu_options() -> eframe::egui_wgpu::WgpuConfiguration {
    let mut options = eframe::egui_wgpu::WgpuConfiguration::default();
    if cfg!(windows) && eframe::wgpu::Backends::from_env().is_none() {
        if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_setup {
            setup.instance_descriptor.backends = eframe::wgpu::Backends::DX12;
        }
    }
    options
}

/// The name people know a graphics interface by.
#[cfg(not(target_arch = "wasm32"))]
fn api_name(backend: eframe::wgpu::Backend) -> String {
    match backend {
        eframe::wgpu::Backend::Dx12 => "DirectX 12".to_string(),
        eframe::wgpu::Backend::Vulkan => "Vulkan".to_string(),
        eframe::wgpu::Backend::Metal => "Metal".to_string(),
        eframe::wgpu::Backend::Gl => "OpenGL".to_string(),
        other => format!("{other:?}"),
    }
}

/// On a Wayland desktop, opens the window through XWayland when it is there: winit 0.30 does
/// not take files dropped on a Wayland window, so the media pool could not receive them.
/// `PICASSO_WAYLAND=1` keeps the native Wayland window. winit is told, rather than
/// `WAYLAND_DISPLAY` removed, so the programs started from here (the browser, the installer)
/// still find the Wayland desktop.
#[cfg(target_os = "linux")]
fn prefer_x11_for_drops(mut options: eframe::NativeOptions) -> eframe::NativeOptions {
    use winit::platform::x11::EventLoopBuilderExtX11 as _;

    if std::env::var_os("PICASSO_WAYLAND").is_none()
        && std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var_os("DISPLAY").is_some()
    {
        options.event_loop_builder = Some(Box::new(|builder| {
            builder.with_x11();
        }));
    }
    options
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    if std::env::args().any(|a| a == "--menu-status") {
        println!("{}", picasso_ui::menu_status_markdown());
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Picasso Suite")
            // Wayland's app id and X11's WM_CLASS: the Linux desktop entry has this name, so the
            // dock shows the icon and groups the window with its launcher.
            .with_app_id("io.github.fabox_os.PicassoSuite")
            .with_inner_size(window_size())
            .with_min_inner_size([936.0, 500.0])
            .with_decorations(false)
            .with_drag_and_drop(true)
            .with_icon(app_icon()),
        wgpu_options: wgpu_options(),
        ..Default::default()
    };
    #[cfg(target_os = "linux")]
    let options = prefer_x11_for_drops(options);
    eframe::run_native(
        "Picasso Suite",
        options,
        Box::new(|cc| {
            let mut app = StudioApp::new(&cc.egui_ctx);
            // Which graphics card draws the window: a software renderer makes playback slow.
            if let Some(render_state) = &cc.wgpu_render_state {
                let info = render_state.adapter.get_info();
                let api = api_name(info.backend);
                let software = info.device_type == eframe::wgpu::DeviceType::Cpu;
                println!("Graphics: {} ({api}, {:?})", info.name, info.device_type);
                app.set_graphics(&info.name, &api, software);
            }
            // `picasso-suite [--place] [--page=audio] <file>...` opens projects and imports media.
            let args: Vec<String> = std::env::args().skip(1).collect();
            let place = args.iter().any(|a| a == "--place");
            for path in args.iter().filter(|a| !a.starts_with("--")) {
                app.open_path(path, place);
            }
            if let Some(page) = args.iter().find_map(|a| a.strip_prefix("--page=")) {
                app.show_page(page);
            }
            // Files or a page on the command line go straight to the editor.
            if args.iter().any(|a| a == "--projects") {
                app.show_project_manager();
            } else if !args.is_empty() {
                app.hide_project_manager();
            }
            Ok(Box::new(Window::new(app)))
        }),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    // Messages from the renderer and the app go to the browser's console.
    eframe::WebLogger::init(log::LevelFilter::Info).ok();

    wasm_bindgen_futures::spawn_local(async {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        let canvas = document
            .get_element_by_id("picasso_canvas")
            .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok());
        let Some(canvas) = canvas else { return };
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(Window::new(StudioApp::new(&cc.egui_ctx))))),
            )
            .await;
        // Replace the loading text with what went wrong, if anything did.
        if let Some(loading) = document.get_element_by_id("loading") {
            match started {
                Ok(()) => loading.remove(),
                Err(_) => loading
                    .set_inner_html("Picasso Suite could not start: this browser needs WebGL2."),
            }
        }
    });
}
