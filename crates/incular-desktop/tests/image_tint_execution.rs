//! Real-GPU pixel evidence for `RawImage` tint.
//!
//! Each window paints a known source (white, black, colored, transparent,
//! or partially transparent) through the tint color-matrix layer over a
//! known opaque background, and the test asserts the composited center
//! pixel. This proves the constant-color tint executes on hardware rather
//! than only producing a neutral command.
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
use incular_widgets::{RawImage, Stack, Widget};

const WIN_W: f32 = 64.0;
const WIN_H: f32 = 64.0;

fn window_options(title: &str) -> WindowOptions {
    WindowOptions {
        initial_logical_size: Size::new(WIN_W, WIN_H),
        decorations: false,
        ..WindowOptions::new(title)
    }
}

fn solid_source(red: u8, green: u8, blue: u8, alpha: u8) -> ImageHandle {
    let pixels: Vec<u8> = (0..8 * 8).flat_map(|_| [red, green, blue, alpha]).collect();
    ImageHandle::from_rgba8(8, 8, pixels).expect("source handle")
}

fn tinted_window(source: (u8, u8, u8, u8), tint: Color, background: Color) -> Widget {
    let (red, green, blue, alpha) = source;
    let image = solid_source(red, green, blue, alpha);
    Stack::new([
        Widget::box_(Size::new(WIN_W, WIN_H), background),
        Widget::from(
            RawImage::new()
                .image(image)
                .width(WIN_W)
                .height(WIN_H)
                .color(tint),
        ),
    ])
    .into()
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

/// Expected composited pixel for a straight-RGBA tint result over an
/// opaque background, evaluated through the same transfer functions as
/// the renderer. Tolerance is justified below at the call site.
fn composite_over(tinted: [f32; 4], background: Color) -> [u8; 4] {
    let premultiplied = [
        tinted[0] * tinted[3],
        tinted[1] * tinted[3],
        tinted[2] * tinted[3],
        tinted[3],
    ];
    let destination = background.to_linear_rgba();
    let composed = [
        premultiplied[0] + destination[0] * (1. - premultiplied[3]),
        premultiplied[1] + destination[1] * (1. - premultiplied[3]),
        premultiplied[2] + destination[2] * (1. - premultiplied[3]),
        1.,
    ];
    let color = Color::from_linear_rgba(composed);
    [color.red, color.green, color.blue, color.alpha]
}

struct Case {
    name: &'static str,
    source: (u8, u8, u8, u8),
    tint: Color,
    background: Color,
    expected: [u8; 4],
    tolerance: u8,
}

fn cases() -> Vec<Case> {
    let red = Color::rgba(255, 0, 0, 255);
    let blue = Color::rgba(0, 0, 255, 255);
    let black = Color::rgba(0, 0, 0, 255);
    vec![
        Case {
            name: "opaque white flattens to red",
            source: (255, 255, 255, 255),
            tint: red,
            background: black,
            expected: [255, 0, 0, 255],
            tolerance: 1,
        },
        Case {
            name: "opaque black flattens to red (not modulate-black)",
            source: (0, 0, 0, 255),
            tint: red,
            background: blue,
            expected: [255, 0, 0, 255],
            tolerance: 1,
        },
        Case {
            name: "opaque green flattens to red",
            source: (0, 255, 0, 255),
            tint: red,
            background: blue,
            expected: [255, 0, 0, 255],
            tolerance: 1,
        },
        Case {
            name: "transparent source shows the background",
            source: (255, 255, 255, 0),
            tint: red,
            background: blue,
            expected: [0, 0, 255, 255],
            tolerance: 1,
        },
        Case {
            name: "half-transparent white composites over blue",
            source: (255, 255, 255, 128),
            tint: red,
            background: blue,
            // Alpha has no transfer function: the decoded straight alpha
            // is 128/255 and A' = 128/255 * 1.0. Composited over blue in
            // linear space this is the exact 50/50 blend the GPU produced.
            expected: composite_over([1., 0., 0., 128. / 255.], blue),
            // Tolerance covers one 8-bit quantization step in the capture
            // plus the linear/sRGB transfer round-trip; the effect path
            // does not dither.
            tolerance: 3,
        },
        Case {
            name: "half-transparent red tint composites over white",
            source: (255, 255, 255, 255),
            tint: Color::rgba(255, 0, 0, 128),
            background: Color::rgba(255, 255, 255, 255),
            expected: composite_over([1., 0., 0., 128. / 255.], Color::rgba(255, 255, 255, 255)),
            tolerance: 3,
        },
    ]
}

fn tint_execution() {
    let cases = cases();
    let mut application = Application::new(|_| Widget::box_(Size::new(WIN_W, WIN_H), Color::BLACK))
        .expect("bootstrap application");
    let bootstrap = application.primary_window();
    let mut windows = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        let source = case.source;
        let tint = case.tint;
        let background = case.background;
        let window = application
            .open_window_with(
                window_options(&format!("Incular image tint {index}")),
                move |_| tinted_window(source, tint, background),
            )
            .expect("open tint window");
        windows.push(window);
    }
    assert!(application.close_window(bootstrap));
    assert_eq!(application.active_window_ids().len(), cases.len());

    let simulation = application.simulation();
    let (sender, receiver) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || -> Result<Vec<[u8; 4]>, SimulationError> {
                let mut pixels = Vec::new();
                for window in &windows {
                    let sim = simulation.window(window);
                    sim.wait_for_frame()?;
                    sim.wait_for_frame()?;
                    pixels.push(center_pixel(&sim)?);
                    window.close().expect("close bridge remains active");
                }
                Ok(pixels)
            },
        ));
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
        .recv_timeout(Duration::from_secs(120))
        .expect("tint regression did not complete");
    worker.join().expect("tint worker did not panic");
    run_result.expect("desktop event loop failed");
    let pixels = report
        .expect("tint worker panicked")
        .map_err(|error| format!("tint simulation failed: {error:?}"))
        .expect("tint simulation failed");

    assert_eq!(pixels.len(), cases.len());
    for (case, pixel) in cases.iter().zip(pixels.iter()) {
        eprintln!("tint: {} -> {pixel:?}", case.name);
        for (channel, (got, want)) in pixel.iter().zip(case.expected.iter()).enumerate() {
            let distance = got.abs_diff(*want);
            assert!(
                distance <= case.tolerance,
                "{}: channel {channel} got {pixel:?}, want {:?} (tolerance {})",
                case.name,
                case.expected,
                case.tolerance
            );
        }
    }
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
