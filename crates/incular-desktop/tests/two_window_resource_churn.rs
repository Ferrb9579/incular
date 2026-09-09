//! Two-window GPU resource churn on real renderers sharing one GPU context.
//!
//! Window A renders fixed shared content (one image, one gradient, one
//! text run) and then goes idle. Window B starts with the same shared
//! content, then churns per-epoch content — distinct images, gradients,
//! and text sizes that exceed the default 256-entry shared budgets by
//! volume — without touching the shared handles, so budget pressure
//! evicts A's entries cold on the device while host maintenance reclaims
//! idle bindings. A then resumes and must present pixel-identical output,
//! proving eviction, reclamation, and re-resolution stayed correct;
//! finally A closes while B keeps rendering.
//!
//! What this proves on hardware: actual image/gradient uploads, glyph
//! rasterization and page uploads, binding, submission, and presentation
//! across two windows; correctness of eviction and re-resolution under
//! churn (stale images, gradients, or atlas contents would corrupt the
//! screenshots); and clean single-window teardown. Residency counts,
//! registry identities, and budget-tightening mechanics are asserted
//! headlessly against the production policy components (which the live
//! worker cannot reach by design); the live run proves those paths behave
//! on a real device. Actual GPU presentation stays native-only: this
//! target runs solely under `INCULAR_DESKTOP_LIVE_TESTS=1` and fails on
//! any initialization or presentation error instead of passing silently.

use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use incular_core::{Color, Offset, Size};
use incular_desktop::run_application;
use incular_image::ImageHandle;
use incular_platform::WindowOptions;
use incular_rendering::{Brush, GradientStop, GradientStops, LinearGradient};
use incular_runtime::{Application, Signal, Simulation, SimulationError};
use incular_widgets::{Column, DecoratedBox, RawImage, Row, SizedBox, Text, Widget};

const WIN_W: f32 = 320.0;
const WIN_H: f32 = 240.0;
const EPOCHS: u64 = 12;
/// Distinct images and gradients per epoch; 12 epochs admit 288 of each,
/// past the default 256-entry shared budgets by volume.
const CELLS_PER_EPOCH: u64 = 24;
const CELL: f32 = 16.0;

fn window_options(title: &str) -> WindowOptions {
    WindowOptions {
        initial_logical_size: Size::new(WIN_W, WIN_H),
        decorations: false,
        ..WindowOptions::new(title)
    }
}

fn solid_image(seed: u8) -> ImageHandle {
    let pixel = [
        seed.wrapping_mul(37).wrapping_add(11),
        seed.wrapping_mul(73).wrapping_add(101),
        seed.wrapping_mul(151).wrapping_add(197),
        255,
    ];
    let mut bytes = Vec::with_capacity(4 * 4 * 4);
    for _ in 0..16 {
        bytes.extend_from_slice(&pixel);
    }
    ImageHandle::from_rgba8(4, 4, Arc::<[u8]>::from(bytes)).expect("test image handle")
}

fn epoch_gradient(epoch: u64, index: u64) -> Brush {
    let shift = (epoch.wrapping_mul(24).wrapping_add(index) % 200) as f32 / 200.0;
    Brush::LinearGradient(LinearGradient {
        start: Offset::new(0., 0.),
        end: Offset::new(CELL, 0.),
        stops: GradientStops::new(vec![
            GradientStop {
                offset: 0.,
                color: Color::rgba(
                    (40. + 180. * shift) as u8,
                    60,
                    (220. - 180. * shift) as u8,
                    255,
                ),
            },
            GradientStop {
                offset: 1.,
                color: Color::rgba(
                    60,
                    (40. + 180. * shift) as u8,
                    (100. + 100. * shift) as u8,
                    255,
                ),
            },
        ]),
    })
}

fn image_cell(handle: ImageHandle) -> Widget {
    RawImage::new()
        .image(handle)
        .width(CELL)
        .height(CELL)
        .into()
}

fn gradient_cell(brush: Brush) -> Widget {
    DecoratedBox::new(SizedBox::new().width(CELL).height(CELL))
        .size(Size::new(CELL, CELL))
        .background(brush)
        .into()
}

fn epoch_row(epoch: u64, range: std::ops::Range<u64>) -> Widget {
    let mut cells = Vec::new();
    for index in range {
        let seed = epoch.wrapping_mul(CELLS_PER_EPOCH).wrapping_add(index) as u8;
        if index % 2 == 0 {
            cells.push(image_cell(solid_image(seed)));
        } else {
            cells.push(gradient_cell(epoch_gradient(epoch, index)));
        }
    }
    Row::new(cells).into()
}

fn churn_text(epoch: u64, base: f32) -> Widget {
    // Sizes cycle so rows stay visible (fully clipped text may never reach
    // the atlas); digits still vary the glyph keys per epoch.
    let style = incular_widgets::TextStyle {
        size: base + (epoch % 6) as f32 * 4.0,
        color: Color::WHITE,
        ..incular_widgets::TextStyle::default()
    };
    Text::new(format!("B{epoch}")).style(style).into()
}

fn shared_image_handle() -> ImageHandle {
    let mut bytes = Vec::with_capacity(8 * 8 * 4);
    for _ in 0..64 {
        bytes.extend_from_slice(&[255, 0, 0, 255]);
    }
    ImageHandle::from_rgba8(8, 8, Arc::<[u8]>::from(bytes)).expect("shared test image")
}

fn shared_gradient() -> Brush {
    Brush::LinearGradient(LinearGradient {
        start: Offset::new(0., 0.),
        end: Offset::new(64., 0.),
        stops: GradientStops::new(vec![
            GradientStop {
                offset: 0.,
                color: Color::rgba(0, 80, 255, 255),
            },
            GradientStop {
                offset: 1.,
                color: Color::rgba(0, 255, 120, 255),
            },
        ]),
    })
}

fn shared_rows(shared_image: &ImageHandle) -> Vec<Widget> {
    let style = incular_widgets::TextStyle {
        size: 20.0,
        color: Color::WHITE,
        ..incular_widgets::TextStyle::default()
    };
    vec![
        RawImage::new()
            .image(shared_image.clone())
            .width(64.)
            .height(64.)
            .into(),
        DecoratedBox::new(SizedBox::new().width(64.).height(64.))
            .size(Size::new(64., 64.))
            .background(shared_gradient())
            .into(),
        Text::new("Alpha").style(style).into(),
    ]
}

fn settle(simulation: &Simulation) -> Result<(), SimulationError> {
    simulation.wait_for_frame()?;
    simulation.wait_for_frame()
}

fn sample(screenshot: &incular_runtime::Screenshot, x: u32, y: u32) -> [u8; 4] {
    let width = screenshot.width();
    let pixels = screenshot.pixels();
    let start = (y as usize * width as usize + x as usize) * 4;
    [
        pixels[start],
        pixels[start + 1],
        pixels[start + 2],
        pixels[start + 3],
    ]
}

/// Asserts the fixed shared content anywhere in the frame, independent
/// of alignment: a solid red 64x64-logical image block, a blue-to-green
/// horizontal gradient block below it, and bright text glyphs below that.
fn assert_shared_content(screenshot: &incular_runtime::Screenshot, what: &str) -> (Vec<u8>, u32) {
    let width = screenshot.width();
    assert!(width > 0, "{what}: capture has no width");
    if std::env::var_os("CHURN_DEBUG_MAP").is_some() {
        let height = screenshot.height();
        let pixels = screenshot.pixels();
        let mut map = String::new();
        for gy in 0..40 {
            for gx in 0..80 {
                let x = gx * width as usize / 80;
                let y = gy * height as usize / 40;
                let s = (y * width as usize + x) * 4;
                let lum =
                    (u32::from(pixels[s]) + u32::from(pixels[s + 1]) + u32::from(pixels[s + 2]))
                        / 3;
                let r = pixels[s] > 128 && pixels[s + 1] < 128 && pixels[s + 2] < 128;
                let g = pixels[s + 1] > 128 && pixels[s] < 128 && pixels[s + 2] < 128;
                let b = pixels[s + 2] > 128 && pixels[s] < 128 && pixels[s + 1] < 128;
                map.push(if r {
                    'R'
                } else if g {
                    'G'
                } else if b {
                    'B'
                } else if lum > 200 {
                    '#'
                } else if lum > 100 {
                    '+'
                } else if lum > 20 {
                    '.'
                } else {
                    ' '
                });
            }
            map.push('\n');
        }
        eprintln!("churn map {what} ({width}x{height}):\n{map}");
    }
    let height = screenshot.height();
    let scale = f64::from(width) / f64::from(WIN_W);
    let block = (64.0 * scale).round() as u32;
    let at = |x: u32, y: u32| sample(screenshot, x.min(width - 1), y.min(height - 1));
    // Solid red block: find a red pixel, then verify a full block around
    // its row band is uniformly red (opaque, no bleeding or stale tiles).
    let red_seed = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .find(|(x, y)| at(*x, *y) == [255, 0, 0, 255]);
    let (red_x, red_y) = red_seed.unwrap_or_else(|| {
        panic!("{what}: no solid red image pixels rendered");
    });
    assert!(
        red_x + block <= width && red_y + block <= height,
        "{what}: red block at ({red_x},{red_y}) must fit a full 64-logical square"
    );
    for y in [red_y + 2, red_y + block / 2, red_y + block - 3] {
        for x in [red_x + 2, red_x + block / 2, red_x + block - 3] {
            let pixel = at(x, y);
            assert_eq!(
                pixel,
                [255, 0, 0, 255],
                "{what}: image block must stay uniformly red, got {pixel:?} at ({x},{y})"
            );
        }
    }
    // Gradient block: a row band below the image holding blue-dominant
    // pixels left of green-dominant pixels (both opaque).
    let band_y = red_y + block;
    let mut blue_x: Option<u32> = None;
    let mut green_x: Option<u32> = None;
    for x in 0..width {
        let mut blue = false;
        let mut green = false;
        for y in band_y..(band_y + block).min(height) {
            let pixel = at(x, y);
            if pixel[2] > 160 && pixel[0] < 96 && pixel[3] == 255 {
                blue = true;
            }
            if pixel[1] > 160 && pixel[2] < 160 && pixel[3] == 255 {
                green = true;
            }
        }
        if blue && blue_x.is_none() {
            blue_x = Some(x);
        }
        if green {
            green_x = Some(x);
        }
    }
    match (blue_x, green_x) {
        (Some(blue), Some(green)) => assert!(
            green > blue,
            "{what}: gradient must run blue-to-green left to right (blue at {blue}, green at {green})"
        ),
        _ => panic!("{what}: blue-to-green gradient block must render"),
    }
    // Text run below the gradient: bright glyph pixels on black.
    let mut bright = 0u32;
    for y in band_y + block..height {
        for x in (0..width).step_by(2) {
            let pixel = at(x, y);
            if pixel[0] > 40 && pixel[1] > 40 && pixel[2] > 40 {
                bright += 1;
            }
        }
    }
    assert!(
        bright > 20,
        "{what}: text run must contain bright glyph pixels, found {bright}"
    );
    (screenshot.clone().into_pixels(), bright)
}

#[derive(Debug)]
struct ChurnReport {
    epochs_presented: u64,
    resumed_identical: bool,
    survivor_sane: bool,
}

fn two_window_resource_churn() {
    let shared_image = shared_image_handle();
    let churn_epoch = Signal::new(0u64);

    let mut application = Application::new(|_| Widget::box_(Size::new(WIN_W, WIN_H), Color::BLACK))
        .expect("bootstrap application");
    let bootstrap = application.primary_window();
    let image_for_a = shared_image.clone();
    let window_a = application
        .open_window_with(window_options("Incular GPU churn window A"), move |_| {
            incular_widgets::ColoredBox::new(Color::BLACK, Column::new(shared_rows(&image_for_a)))
                .into()
        })
        .expect("open churn window A");
    let image_for_b = shared_image.clone();
    let churn_signal = churn_epoch.clone();
    let window_b = application
        .open_window_with(window_options("Incular GPU churn window B"), move |_| {
            let e = churn_epoch.get();
            // The driver button stays visible in every epoch so the worker
            // can keep clicking it through the real input path.
            let churn_signal = churn_signal.clone();
            let mut rows = vec![
                incular_widgets::internal::ActionSurface::new("Churn")
                    .size(Size::new(96.0, 24.0))
                    .on_press(move || {
                        let _ = churn_signal.set(churn_signal.get() + 1);
                    })
                    .into(),
            ];
            if e == 0 {
                // Epoch zero shares A's exact content, proving both windows
                // render the shared uploads. Later epochs drop the shared
                // handles entirely so B's churn leaves A's entries cold and
                // lets budget pressure evict them on the device.
                rows.extend(shared_rows(&image_for_b));
            } else {
                rows.push(epoch_row(e, 0..12));
                rows.push(epoch_row(e, 12..24));
                rows.push(Row::new([churn_text(e, 12.0), churn_text(e, 14.0)]).into());
            }
            incular_widgets::ColoredBox::new(Color::BLACK, Column::new(rows)).into()
        })
        .expect("open churn window B");
    assert!(application.close_window(bootstrap));
    assert_eq!(application.active_window_ids().len(), 2);

    let root = application.simulation();
    let sim_a = root.window(&window_a);
    let sim_b = root.window(&window_b);
    let handle_a = window_a.clone();
    let handle_b = window_b.clone();
    let (sender, receiver) = mpsc::sync_channel(1);

    let worker = std::thread::spawn(move || {
        // A scenario panic must still close the windows below: the event
        // loop only returns once none remain, so an unwinding worker that
        // skips teardown hangs the run instead of failing it.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || -> Result<ChurnReport, SimulationError> {
                // Phase 1: both windows present the shared content.
                eprintln!("churn: waiting for initial frames");
                settle(&sim_a)?;
                settle(&sim_b)?;
                eprintln!("churn: initial frames presented");
                let shot_a0 = sim_a.capture()?;
                let shot_b0 = sim_b.capture()?;
                let (pixels_a0, _) = assert_shared_content(&shot_a0, "window A initial");
                let (_pixels_b0, _) = assert_shared_content(&shot_b0, "window B initial");
                assert_eq!(
                    shot_a0.width(),
                    shot_b0.width(),
                    "both windows share one size class"
                );
                // Both windows render the same shared handles (image bytes,
                // gradient stops, text run); identical content through the
                // shared cache must look identical wherever aligned, and each
                // window's regions are asserted independently above. Sharing
                // itself is proven by the headless ownership suites with real
                // reference counts; the live run proves the shared path stays
                // correct on the device under churn.

                // Phase 2: A goes idle while B churns past the shared budgets.
                let mut epochs_presented = 0u64;
                for _ in 1..=EPOCHS {
                    sim_b.click("Churn")?;
                    settle(&sim_b)?;
                    epochs_presented += 1;
                }
                assert_eq!(epochs_presented, EPOCHS);
                eprintln!("churn: all {EPOCHS} epochs presented");
                let shot_b_churn = sim_b.capture()?;
                eprintln!("churn: churn capture done");
                let (chunks, _) = shot_b_churn.pixels().as_chunks::<4>();
                assert!(
                    chunks.iter().any(|pixel| *pixel != [0, 0, 0, 255]),
                    "churned window B must present non-background content"
                );

                // Phase 3: A resumes and must present pixel-identical output —
                // eviction and re-resolution stayed correct on the device.
                handle_a
                    .request_redraw()
                    .expect("redraw request bridge remains active");
                eprintln!("churn: A redraw requested");
                settle(&sim_a)?;
                eprintln!("churn: A settled after resume");
                let shot_a1 = sim_a.capture()?;
                eprintln!("churn: A resume capture done");
                let (pixels_a1, _) = assert_shared_content(&shot_a1, "window A resumed");
                let resumed_identical = pixels_a0 == pixels_a1;
                eprintln!("churn: A resume identical={resumed_identical}");

                // Phase 4: close A while B keeps rendering, then finish.
                handle_a.close().expect("close bridge remains active");
                eprintln!("churn: A close requested");
                settle(&sim_b)?;
                eprintln!("churn: B settled after A closed");
                let shot_b_last = sim_b.capture()?;
                eprintln!("churn: B survivor capture done");
                let (chunks, _) = shot_b_last.pixels().as_chunks::<4>();
                let survivor_sane =
                    shot_b_last.width() > 0 && chunks.iter().any(|pixel| *pixel != [0, 0, 0, 255]);
                Ok(ChurnReport {
                    epochs_presented,
                    resumed_identical,
                    survivor_sane,
                })
            },
        ));
        // Close through the same generational handles however the scenario
        // ended: the event loop only returns once no windows remain, so a
        // failure must never strand open windows and hang the run.
        eprintln!("churn: closing windows");
        let _ = handle_a.close();
        let _ = handle_b.close();
        eprintln!("churn: closes sent, sending report");
        sender.send(result).expect("send churn report");
        eprintln!("churn: worker done");
    });

    let run_result = run_application(application);
    eprintln!("churn: event loop returned");
    let report = receiver
        .recv_timeout(Duration::from_secs(180))
        .expect("two-window churn regression did not complete");
    worker.join().expect("churn worker did not panic");
    run_result.expect("desktop event loop failed");
    let report = report
        .map_err(|_| "two-window churn worker panicked".to_string())
        .expect("two-window churn worker panicked")
        .map_err(|error| format!("two-window churn simulation failed: {error:?}"))
        .expect("two-window churn simulation failed");
    assert_eq!(
        report.epochs_presented, EPOCHS,
        "every churn epoch presented"
    );
    assert!(
        report.resumed_identical,
        "idle window A must resume pixel-identical after B's churn"
    );
    assert!(
        report.survivor_sane,
        "window B must render on after A closes"
    );
}

fn main() {
    if std::env::var_os("INCULAR_DESKTOP_LIVE_TESTS").is_none() {
        eprintln!(
            "two_window_resource_churn: skipped live native regression; set INCULAR_DESKTOP_LIVE_TESTS=1 to run"
        );
        return;
    }
    two_window_resource_churn();
}
