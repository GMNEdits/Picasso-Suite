//! `picasso-suite --graphics-check`: which graphics cards and interfaces (Vulkan, DirectX 12,
//! Metal, OpenGL) this computer offers, and whether each one can draw. Every adapter is asked for a
//! device, draws a colour into a small picture, and the picture is read back and compared. The
//! report is printed and saved as `picasso-graphics-report.txt` in the current folder (a Windows
//! program started from a window has no console to print to), so that people with graphics cards
//! the project cannot try itself can send it in (see `.github/ISSUE_TEMPLATE/graphics-report.yml`).

use std::fmt::Write as _;
use std::panic::{catch_unwind, AssertUnwindSafe};

use eframe::wgpu;

const REPORT_FILE: &str = "picasso-graphics-report.txt";
/// The picture is this many pixels on each side: a row is 256 bytes, which copies need.
const SIDE: u32 = 64;
/// What the picture is cleared to, and the bytes that come back for it (blue is 0.75 * 255 = 191.25).
const CLEAR: wgpu::Color = wgpu::Color {
    r: 0.25,
    g: 0.5,
    b: 0.75,
    a: 1.0,
};
const EXPECTED: [u8; 4] = [64, 128, 191, 255];

/// Runs the check, prints the report and saves it. Returns whether at least one adapter drew the
/// test picture, which the process exit code tells.
pub(crate) fn run() -> bool {
    let (report, ok) = report();
    print!("{report}");
    if let Err(e) = std::fs::write(REPORT_FILE, &report) {
        eprintln!("could not save {REPORT_FILE}: {e}");
    } else {
        println!("Saved as {REPORT_FILE}");
    }
    ok
}

fn report() -> (String, bool) {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Picasso Suite {} graphics check ({} {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    for name in ["XDG_SESSION_TYPE", "WGPU_BACKEND", "WGPU_ADAPTER_NAME"] {
        if let Ok(value) = std::env::var(name) {
            let _ = writeln!(out, "{name}={value}");
        }
    }
    let _ = writeln!(
        out,
        "Wayland display: {}, X11 display: {}",
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            "yes"
        } else {
            "no"
        },
        if std::env::var_os("DISPLAY").is_some() {
            "yes"
        } else {
            "no"
        },
    );

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));
    if adapters.is_empty() {
        let _ = writeln!(
            out,
            "\nNo graphics adapter was found. On Linux, Vulkan needs a driver (mesa-vulkan-drivers, \
             or the vendor's own) and its loader (libvulkan1)."
        );
        return (out, false);
    }

    let mut drawing: Vec<(String, bool)> = Vec::new();
    for (i, adapter) in adapters.iter().enumerate() {
        let info = adapter.get_info();
        let software = info.device_type == wgpu::DeviceType::Cpu;
        let _ = writeln!(
            out,
            "\nAdapter {}: {} ({:?}, {:?})",
            i + 1,
            info.name,
            info.backend,
            info.device_type
        );
        let _ = writeln!(
            out,
            "  vendor {:#06x}, device {:#06x}, driver {} {}",
            info.vendor,
            info.device,
            info.driver.trim(),
            info.driver_info.trim()
        );
        let limits = adapter.limits();
        let _ = writeln!(
            out,
            "  largest picture {} pixels, {} features",
            limits.max_texture_dimension_2d,
            adapter.features().iter().count()
        );
        // A driver that fails badly may panic inside wgpu; that is a result too.
        let drew = catch_unwind(AssertUnwindSafe(|| draw_test_picture(adapter)))
            .unwrap_or_else(|_| Err("the driver made the program panic".into()));
        match drew {
            Ok(()) => {
                let _ = writeln!(out, "  draws: yes");
                drawing.push((format!("{} ({:?})", info.name, info.backend), software));
            }
            Err(why) => {
                let _ = writeln!(out, "  draws: NO ({why})");
            }
        }
    }

    // Decoding video on the card is FFmpeg's, not wgpu's: a card can draw and still decode on the
    // processor, when its VA-API driver is missing.
    match picasso_decode::hw::check() {
        Ok(kind) => {
            let _ = writeln!(out, "\nVideo decoding on the card: yes, with {kind}");
        }
        Err(why) => {
            let _ = writeln!(out, "\nVideo decoding on the card: no, on the processor");
            for line in why {
                let _ = writeln!(out, "  {line}");
            }
        }
    }

    let _ = writeln!(out);
    let cards: Vec<&str> = drawing
        .iter()
        .filter(|(_, software)| !software)
        .map(|(name, _)| name.as_str())
        .collect();
    let ok = !drawing.is_empty();
    if let Some(first) = cards.first() {
        let _ = writeln!(out, "Result: OK, {first} draws.");
    } else if ok {
        let _ = writeln!(
            out,
            "Result: only a software renderer draws; the editor would run, but slowly. \
             Install the graphics driver of the card."
        );
    } else {
        let _ = writeln!(out, "Result: FAILED, no adapter drew the test picture.");
    }
    (out, ok)
}

/// Clears a small picture on `adapter` and reads it back.
fn draw_test_picture(adapter: &wgpu::Adapter) -> Result<(), String> {
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .map_err(|e| format!("no device: {e}"))?;
    let size = wgpu::Extent3d {
        width: SIDE,
        height: SIDE,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(SIDE * SIDE * 4),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    drop(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(CLEAR),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    }));
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(SIDE * 4),
                rows_per_image: None,
            },
        },
        size,
    );
    queue.submit([encoder.finish()]);

    let (sender, receiver) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = sender.send(r);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| format!("the card did not finish: {e}"))?;
    receiver
        .recv()
        .map_err(|_| "no answer from the card".to_string())?
        .map_err(|e| format!("the picture could not be read back: {e}"))?;

    let data = buffer
        .slice(..)
        .get_mapped_range()
        .map_err(|e| format!("the picture could not be read back: {e}"))?;
    let wrong = data
        .chunks_exact(4)
        .filter(|px| px.iter().zip(EXPECTED).any(|(a, b)| a.abs_diff(b) > 2))
        .count();
    if wrong == 0 {
        Ok(())
    } else {
        Err(format!(
            "{wrong} of {} pixels came back with the wrong colour, the first {:?}",
            SIDE * SIDE,
            &data[..4]
        ))
    }
}
