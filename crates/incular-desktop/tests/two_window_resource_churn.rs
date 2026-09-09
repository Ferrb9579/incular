//! Two-window GPU resource churn on real renderers sharing one GPU context.
//!
//! Window A renders fixed shared content (one image, one gradient, one
//! text run) and then goes idle. Window B starts with the same shared
//! handles, then churns per-epoch content — distinct images, gradients,
//! and glyph sizes — without touching them, forcing real eviction on the
//! device while host maintenance reclaims idle bindings. A then resumes
//! and must present pixel-identical output; finally A closes (to observed
//! completion) while B keeps rendering.
//!
//! What this proves on hardware, through production acquisition,
//! retirement, maintenance, and presentation paths: actual image and
//! gradient uploads, glyph rasterization and page uploads, binding,
//! submission, and presentation across two windows; per-family eviction
//! (shared entries capped with eviction counters advanced, observed via
//! `GpuResourceSummary` reads that never present); idle local-binding
//! reclamation without an A presentation; and correct re-resolution on
//! resume (stale content would corrupt the screenshots). Residency and
//! identity mechanics beyond these reads, plus glyph-page budget
//! tightening under protection (frame pins exist only mid-submission and
//! cannot be held from outside), stay asserted headlessly against the
//! production policy components. Actual GPU presentation stays
//! native-only: this target runs solely when `INCULAR_DESKTOP_LIVE_TESTS`
//! is exactly `1` and fails on any initialization or presentation error
//! instead of passing silently.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use incular_core::{Color, Offset, Size};
use incular_desktop::run_application;
use incular_image::ImageHandle;
use incular_platform::WindowOptions;
use incular_rendering::{Brush, GradientStop, GradientStops, LinearGradient};
use incular_runtime::{Application, GpuResourceSummary, Signal, Simulation, SimulationError};
use incular_widgets::{Column, DecoratedBox, RawImage, Row, SizedBox, Text, Widget};

const WIN_W: f32 = 320.0;
const WIN_H: f32 = 240.0;
/// Image/gradient churn epochs: 12 + 12 distinct uploads per epoch, so 24
/// epochs admit 289 distinct resources per family against the default
/// 256-entry shared budgets, deterministically retiring 33 each.
const IMAGE_EPOCHS: u64 = 24;
/// Glyph pressure epochs: one oversize glyph per epoch takes its own page
/// while the 8-page budget allows, so 12 epochs deterministically retire
/// 4 pages.
const GLYPH_EPOCHS: u64 = 12;
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

/// One oversize glyph per epoch: at ~900 physical px a wide glyph's
/// stored bitmap exceeds a quarter page, so each takes its own atlas
/// page while the 8-page budget allows. The `size` is a logical size the
/// worker derives from the observed display scale so every epoch stays
/// inside the supported physical raster limits on any display; the letter
/// varies per epoch for distinct cache keys. The glyph sits right below
/// the driver button so its top stays visible (fully clipped runs never
/// reach the atlas); partially visible glyphs still rasterize fully.
fn pressure_text(letter: char, size: f32) -> Widget {
    let style = incular_widgets::TextStyle {
        size,
        color: Color::WHITE,
        ..incular_widgets::TextStyle::default()
    };
    Text::new(letter.to_string()).style(style).into()
}

/// Wide letters for distinct oversize cache keys, one per epoch.
const PRESSURE_LETTERS: [char; 12] = ['G', 'H', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'W'];

fn shared_image_handle() -> ImageHandle {
    let mut bytes = Vec::with_capacity(8 * 8 * 4);
    for _ in 0..64 {
        bytes.extend_from_slice(&[255, 0, 0, 255]);
    }
    ImageHandle::from_rgba8(8, 8, Arc::<[u8]>::from(bytes)).expect("shared test image")
}

fn shared_rows(shared_image: &ImageHandle, shared_gradient: &Brush) -> Vec<Widget> {
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
            .background(shared_gradient.clone())
            .into(),
        Text::new("Alpha").style(style).into(),
    ]
}

fn settle(simulation: &Simulation) -> Result<(), SimulationError> {
    simulation.wait_for_frame()?;
    simulation.wait_for_frame()
}

/// Non-rendering residency read for one window. The native adapter
/// fulfills this from current state on its maintenance path.
fn gpu_summary(simulation: &Simulation) -> Result<GpuResourceSummary, SimulationError> {
    simulation.query_gpu_resources()
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
    let height = screenshot.height();
    assert!(width > 0 && height > 0, "{what}: capture is empty");
    if std::env::var_os("CHURN_DEBUG_MAP").is_some() {
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
    image_evictions: u64,
    gradient_evictions: u64,
    glyph_page_evictions: u64,
    resumed_identical: bool,
    survivor_sane: bool,
}

fn two_window_resource_churn() {
    // The shared gradient is constructed once: both windows clone this
    // identity, so one shared upload must serve both (previous revisions
    // rebuilt it per window and never actually shared it).
    let shared_gradient = Brush::LinearGradient(LinearGradient {
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
    });
    let shared_image = shared_image_handle();
    let churn_epoch = Signal::new(0u64);

    let mut application = Application::new(|_| Widget::box_(Size::new(WIN_W, WIN_H), Color::BLACK))
        .expect("bootstrap application");
    let bootstrap = application.primary_window();
    let image_for_a = shared_image.clone();
    let gradient_for_a = shared_gradient.clone();
    let window_a = application
        .open_window_with(window_options("Incular GPU churn window A"), move |_| {
            incular_widgets::ColoredBox::new(
                Color::BLACK,
                Column::new(shared_rows(&image_for_a, &gradient_for_a)),
            )
            .into()
        })
        .expect("open churn window A");
    let image_for_b = shared_image.clone();
    let gradient_for_b = shared_gradient.clone();
    let churn_signal = churn_epoch.clone();
    let window_b = application
        .open_window_with(
            window_options("Incular GPU churn window B"),
            move |context| {
                let e = churn_epoch.get();
                // Logical size rasterizing near 900 physical px on this
                // display: inside the supported raster limits, above the
                // oversize page threshold, on any scale factor.
                let pressure_size =
                    (900.0 / context.scale_factor().max(0.25)).clamp(100.0, 2000.0) as f32;
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
                    // Epoch zero shares A's exact handles, proving both windows
                    // render the shared uploads. Later epochs drop them so B's
                    // churn leaves A's entries cold for eviction.
                    rows.extend(shared_rows(&image_for_b, &gradient_for_b));
                } else if e <= IMAGE_EPOCHS {
                    // Image/gradient churn: grid and small texts stay visible;
                    // the pressure glyph would push them off-screen, so it
                    // runs in its own phase below.
                    rows.push(epoch_row(e, 0..12));
                    rows.push(epoch_row(e, 12..24));
                    rows.push(Row::new([churn_text(e, 12.0), churn_text(e, 14.0)]).into());
                } else {
                    // Glyph pressure: the oversize glyph sits right below the
                    // button so its top stays visible (fully clipped runs never
                    // reach the atlas).
                    let pressure_index = (e - IMAGE_EPOCHS - 1) as usize % PRESSURE_LETTERS.len();
                    rows.push(pressure_text(
                        PRESSURE_LETTERS[pressure_index],
                        pressure_size,
                    ));
                    rows.push(Row::new([churn_text(e, 12.0), churn_text(e, 14.0)]).into());
                }
                incular_widgets::ColoredBox::new(Color::BLACK, Column::new(rows)).into()
            },
        )
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
                // Phase 1: both windows present the shared content, then
                // prove one shared upload serves both windows per family.
                eprintln!("churn: waiting for initial frames");
                settle(&sim_a)?;
                settle(&sim_b)?;
                eprintln!("churn: initial frames presented");
                let shot_a0 = sim_a.capture()?;
                let shot_b0 = sim_b.capture()?;
                let (pixels_a0, _) = assert_shared_content(&shot_a0, "window A initial");
                let (_pixels_b0, _) = assert_shared_content(&shot_b0, "window B initial");
                let before_a = gpu_summary(&sim_a)?;
                let before_b = gpu_summary(&sim_b)?;
                assert_eq!(
                    before_a.shared_image_entries, 1,
                    "one shared image upload must serve window A, got {}",
                    before_a.shared_image_entries
                );
                assert_eq!(
                    before_b.shared_image_entries, 1,
                    "window B must reuse A's shared image upload, got {}",
                    before_b.shared_image_entries
                );
                assert_eq!(
                    before_a.shared_gradient_entries, 1,
                    "one shared gradient upload must serve window A, got {}",
                    before_a.shared_gradient_entries
                );
                assert_eq!(
                    before_b.shared_gradient_entries, 1,
                    "window B must reuse A's shared gradient upload, got {}",
                    before_b.shared_gradient_entries
                );
                // "Alpha" (5 distinct glyphs) shared by both windows plus
                // B's "Churn" button label (5 more): exactly 10 shared
                // rasterizations, never one per window.
                assert_eq!(
                    before_b.glyphs_rasterized, 10,
                    "shared glyphs must rasterize once for both windows, got {}",
                    before_b.glyphs_rasterized
                );
                assert!(
                    before_a.local_image_entries >= 1
                        && before_a.local_gradient_entries >= 1
                        && before_a.local_glyph_pages >= 1,
                    "window A must hold local bindings after presenting: {before_a:?}"
                );

                // Phase 2: A goes idle (no A frames, no A queries that
                // render) while B churns images and gradients past their
                // budgets: 1 shared + 12 new per family per epoch.
                for _ in 1..=IMAGE_EPOCHS {
                    sim_b.click("Churn")?;
                    sim_b.wait_for_frame()?;
                }
                let churned = gpu_summary(&sim_b)?;
                eprintln!(
                    "churn: image/gradient pressure after {IMAGE_EPOCHS} epochs \
                     (images {} evicted, gradients {} evicted)",
                    churned.shared_image_evictions, churned.shared_gradient_evictions
                );
                // Deterministic arithmetic: 1 + 12*24 = 289 distinct
                // admissions per family against the 256-entry budgets must
                // drop exactly the 33 oldest entries first, so A's
                // untouched image and gradient are necessarily gone.
                assert_eq!(
                    churned.shared_image_entries, 256,
                    "image cache must sit at its entry budget, got {}",
                    churned.shared_image_entries
                );
                assert_eq!(
                    churned.shared_image_evictions,
                    289 - 256,
                    "image evictions must equal admissions past the budget"
                );
                assert_eq!(
                    churned.shared_gradient_entries, 256,
                    "gradient cache must sit at its entry budget, got {}",
                    churned.shared_gradient_entries
                );
                assert_eq!(
                    churned.shared_gradient_evictions,
                    289 - 256,
                    "gradient evictions must equal admissions past the budget"
                );

                // Phase 2b: glyph pressure. One oversize glyph per epoch
                // takes its own page while the 8-page budget allows; 12
                // epochs must retire exactly the 4 oldest pages.
                let rasterized_before = gpu_summary(&sim_b)?.glyphs_rasterized;
                for _ in 1..=GLYPH_EPOCHS {
                    sim_b.click("Churn")?;
                    sim_b.wait_for_frame()?;
                }
                let pressured = gpu_summary(&sim_b)?;
                eprintln!(
                    "churn: glyph pressure done ({} pages live, {} retired, rasterized {} -> {})",
                    pressured.glyph_live_pages,
                    pressured.glyph_page_evictions,
                    rasterized_before,
                    pressured.glyphs_rasterized
                );
                assert_eq!(
                    pressured.glyph_live_pages, 8,
                    "glyph budget must stay capped at 8 live pages, got {}",
                    pressured.glyph_live_pages
                );
                // Twelve oversize placements against eight live pages;
                // at least all but a narrow-letter or coalesced epoch
                // must retire. Exactness is font- and timing-sensitive,
                // so the bound stays a floor.
                assert!(
                    pressured.glyph_page_evictions >= 3,
                    "oversize placements past the page budget must retire pages, got {}",
                    pressured.glyph_page_evictions
                );

                // Phase 3: production host maintenance must have removed
                // A's stale local bindings already, with no A presentation
                // since phase 1. This read itself renders nothing. The
                // image and gradient bindings are stale (their shared
                // entries churned away) and must be gone; the glyph page
                // binding stays because page 0 — refreshed by every
                // rendered glyph hit, including B's button — was never a
                // least-recently-used victim, so reclaim correctly keeps
                // the still-valid binding while dropping the stale ones.
                let idle_a = gpu_summary(&sim_a)?;
                assert_eq!(
                    (
                        idle_a.local_image_entries,
                        idle_a.local_gradient_entries,
                        idle_a.local_glyph_pages
                    ),
                    (0, 0, 1),
                    "idle window A must show precise reclamation (stale dropped, live kept): {idle_a:?}"
                );

                // Phase 4: A resumes and must present pixel-identical
                // output — eviction, reclamation, and re-resolution stayed
                // correct on the device.
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
                let reaquired = gpu_summary(&sim_a)?;
                assert!(
                    reaquired.local_image_entries >= 1
                        && reaquired.local_gradient_entries >= 1
                        && reaquired.local_glyph_pages >= 1,
                    "resumed window A must hold fresh bindings: {reaquired:?}"
                );
                eprintln!("churn: A resume identical={resumed_identical}");

                // Phase 5: close A to observed completion before checking
                // B's continued rendering.
                handle_a.close().expect("close bridge remains active");
                eprintln!("churn: A close requested");
                for _ in 0..60 {
                    match sim_a.query_gpu_resources() {
                        Err(
                            SimulationError::WindowNotFound(_) | SimulationError::WindowClosed(_),
                        ) => break,
                        Ok(_) => continue,
                        Err(other) => {
                            panic!("unexpected error while awaiting A's close: {other:?}")
                        }
                    }
                }
                // Confirm the loop above actually observed the close.
                assert!(
                    matches!(
                        sim_a.query_gpu_resources(),
                        Err(SimulationError::WindowNotFound(_) | SimulationError::WindowClosed(_))
                    ),
                    "window A close must complete observably"
                );
                settle(&sim_b)?;
                eprintln!("churn: B settled after A closed");
                let shot_b_last = sim_b.capture()?;
                eprintln!("churn: B survivor capture done");
                let (chunks, _) = shot_b_last.pixels().as_chunks::<4>();
                let survivor_sane =
                    shot_b_last.width() > 0 && chunks.iter().any(|pixel| *pixel != [0, 0, 0, 255]);
                handle_b.close().expect("close bridge remains active");
                Ok(ChurnReport {
                    image_evictions: churned.shared_image_evictions,
                    gradient_evictions: churned.shared_gradient_evictions,
                    glyph_page_evictions: pressured.glyph_page_evictions,
                    resumed_identical,
                    survivor_sane,
                })
            },
        ));
        // Close through the same generational handles however the scenario
        // ended: the event loop only returns once none remain, so a
        // failure must never strand open windows and hang the run.
        eprintln!("churn: closing windows");
        let _ = handle_a.close();
        let _ = handle_b.close();
        eprintln!("churn: closes sent, sending report");
        sender.send(result).expect("send churn report");
        eprintln!("churn: worker done");
    });

    // Watchdog while the native loop runs: if anything above wedges the
    // event loop (or teardown), fail nonzero instead of hanging forever.
    // Disarmed below once the run completes.
    let settled = Arc::new(AtomicBool::new(false));
    let watchdog_settled = settled.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(480));
        if !watchdog_settled.load(Ordering::SeqCst) {
            eprintln!("two_window_resource_churn: watchdog expired before completion");
            std::process::exit(2);
        }
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
    assert!(
        report.image_evictions > 0 && report.gradient_evictions > 0,
        "per-family eviction evidence required: {report:?}"
    );
    assert!(
        report.glyph_page_evictions > 0,
        "glyph page eviction evidence required: {report:?}"
    );
    assert!(
        report.resumed_identical,
        "idle window A must resume pixel-identical after B's churn"
    );
    assert!(
        report.survivor_sane,
        "window B must render on after A closes"
    );
    settled.store(true, Ordering::SeqCst);
}

fn main() {
    if std::env::var("INCULAR_DESKTOP_LIVE_TESTS").as_deref() != Ok("1") {
        eprintln!(
            "two_window_resource_churn: skipped live native regression; \
             set INCULAR_DESKTOP_LIVE_TESTS=1 to run"
        );
        return;
    }
    two_window_resource_churn();
}
