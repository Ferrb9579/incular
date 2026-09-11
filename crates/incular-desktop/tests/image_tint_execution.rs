//! Real-GPU pixel evidence for `RawImage` tint.
//!
//! A window paints a fully-opaque white source image tinted red through
//! the retained color-matrix layer. The captured center pixel must be
//! that tint color, proving the tint reaches WGPU execution rather than
//! only producing a neutral command.
//!
//! Actual GPU presentation stays native-only: this target runs solely when
//! `INCULAR_DESKTOP_LIVE_TESTS` is exactly `1` and fails on any
//! initialization error instead of passing silently.

use std::sync::mpsc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use incular_core::{Color, Size};
use incular_desktop::run_application;
use incular_image::ImageHandle;
use incular_platform::WindowOptions;
use incular_runtime::{Application, Simulation, SimulationError};
use incular_widgets::{RawImage, Widget};

const WIN_W: f32 = 64.0;
const WIN_H: f32 = 64.0;

fn window_options(title: &str) -> WindowOptions {
    WindowOptions {
        initial_logical_size: Size::new(WIN_W, WIN_H),
        decorations: false,
        ..WindowOptions::new(title)
    }
}

/// An opaque white 8x8 source. Tinting it red must yield a solid red block.
fn white_source() -> ImageHandle {
    ImageHandle::from_rgba8(8, 8, vec![255u8; 8 * 8 * 4]).expect("white handle")
}

fn tinted_root() -> Widget {
    Widget::from(
        RawImage::new()
            .image(white_source())
            .width(WIN_W)
            .height(WIN_H)
            .color(Color::rgba(255, 0, 0, 255)),
    )
}

fn center_pixel(simulation: &Simulation) -> Result<[u8; 4], SimulationError> {
    let shot = simulation.capture()?;
    let width = shot.width();
    let height = shot.height();
    let x = width / 2;
    let y = height / 2;
    let start = (y as usize * width as usize + x as usize) * 4;
    let pixels = shot.pixels();
    Ok([
        pixels[start],
        pixels[start + 1],
        pixels[start + 2],
        pixels[start + 3],
    ])
}

fn tint_execution() {
    let mut application = Application::new(|_| Widget::box_(Size::new(WIN_W, WIN_H), Color::BLACK))
        .expect("bootstrap application");
    let bootstrap = application.primary_window();
    let window = application
        .open_window_with(window_options("Incular image tint"), move |_| tinted_root())
        .expect("open tint window");
    assert!(application.close_window(bootstrap));
    assert_eq!(application.active_window_ids().len(), 1);

    let simulation = application.simulation();
    let sim = simulation.window(&window);
    let handle = window.clone();
    let (sender, receiver) = mpsc::sync_channel(1);

    let worker = std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || -> Result<[u8; 4], SimulationError> {
                sim.wait_for_frame()?;
                sim.wait_for_frame()?;
                let pixel = center_pixel(&sim)?;
                handle.close().expect("close bridge remains active");
                Ok(pixel)
            },
        ));
        let _ = handle.close();
        sender.send(result).expect("send tint report");
    });

    let settled = Arc::new(AtomicBool::new(false));
    let watchdog_settled = settled.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(180));
        if !watchdog_settled.load(Ordering::SeqCst) {
            eprintln!("image_tint_execution: watchdog expired before completion");
            std::process::exit(2);
        }
    });

    let run_result = run_application(application);
    let report = receiver
        .recv_timeout(Duration::from_secs(90))
        .expect("tint regression did not complete");
    worker.join().expect("tint worker did not panic");
    run_result.expect("desktop event loop failed");
    let pixel = report
        .expect("tint worker panicked")
        .map_err(|error| format!("tint simulation failed: {error:?}"))
        .expect("tint simulation failed");

    // Opaque white tinted red: every channel is exact (red kept, green and
    // blue zero, alpha preserved).
    eprintln!("tint: captured center pixel {pixel:?}");
    assert!(pixel[0] > 200, "tinted red channel, got {pixel:?}");
    assert!(pixel[1] < 40, "green must be suppressed, got {pixel:?}");
    assert!(pixel[2] < 40, "blue must be suppressed, got {pixel:?}");
    assert_eq!(pixel[3], 255, "alpha preserved, got {pixel:?}");
    settled.store(true, Ordering::SeqCst);
}

fn main() {
    if std::env::var("INCULAR_DESKTOP_LIVE_TESTS").as_deref() != Ok("1") {
        eprintln!(
            "image_tint_execution: skipped live native regression; \
             set INCULAR_DESKTOP_LIVE_TESTS=1 to run"
        );
        return;
    }
    tint_execution();
}
