//! Unsupported retained effects fail loudly on real renderer execution.
//!
//! A window renders a shader mask, then a backdrop filter, then plain
//! content across three signal-driven phases. The first two phases must
//! fail each attempt with the backend's typed error and produce no
//! presentation; the third must present, proving the failure is scoped
//! to the attempt and later demand renders corrected content.
//!
//! What this proves on hardware, through production acquisition,
//! lowering, presentation, and waiter paths: the `UnsupportedShaderMask`
//! / `UnsupportedBackdropFilter` errors travel from real lowering
//! through the desktop frame-error branch to typed waiter failures, and
//! capture waiters settle as unavailable rather than hanging. Pixel
//! behavior of the effects themselves stays unresolved: there is no
//! supported rendering to screenshot.
//!
//! Actual GPU presentation stays native-only: this target runs solely
//! when `INCULAR_DESKTOP_LIVE_TESTS` is exactly `1` and fails on any
//! initialization error instead of passing silently.

use std::sync::mpsc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use incular_core::{Color, Rect, Size};
use incular_desktop::run_application;
use incular_platform::WindowOptions;
use incular_rendering::Brush;
use incular_runtime::{Application, Signal, Simulation, SimulationError};
use incular_widgets::{BackdropFilter, Column, ShaderMask, Widget};

const WIN_W: f32 = 160.0;
const WIN_H: f32 = 120.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Mask,
    Backdrop,
    Plain,
}

fn window_options(title: &str) -> WindowOptions {
    WindowOptions {
        initial_logical_size: Size::new(WIN_W, WIN_H),
        decorations: false,
        ..WindowOptions::new(title)
    }
}

/// Asserts one frame attempt failed with the backend's typed error and
/// that no presentation was reported for it: the frame waiter and a
/// follow-up capture waiter must both fail typed.
fn expect_unsupported(sim: &Simulation, fragment: &str) -> Result<(), SimulationError> {
    match sim.wait_for_frame() {
        Err(SimulationError::FrameFailed(message)) => assert!(
            message.contains(fragment),
            "frame must fail with the backend message, got {message:?}"
        ),
        other => panic!("frame attempt must fail typed, got {other:?}"),
    }
    match sim.capture() {
        Err(SimulationError::CaptureUnavailable(_)) => Ok(()),
        other => panic!("capture must fail without a presentation, got {other:?}"),
    }
}

fn unsupported_effect_execution() {
    let phase = Signal::new(Phase::Mask);

    let mut application = Application::new(|_| Widget::box_(Size::new(WIN_W, WIN_H), Color::BLACK))
        .expect("bootstrap application");
    let bootstrap = application.primary_window();
    let phase_for_window = phase.clone();
    let window = application
        .open_window_with(window_options("Incular unsupported effect"), move |_| {
            let content: Widget = match phase_for_window.get() {
                Phase::Mask => ShaderMask::new(
                    |_: Rect| Brush::Solid(Color::WHITE),
                    Widget::box_(Size::new(64., 64.), Color::WHITE),
                )
                .into(),
                Phase::Backdrop => {
                    BackdropFilter::blur(8., Widget::box_(Size::new(64., 64.), Color::WHITE)).into()
                }
                Phase::Plain => Widget::box_(Size::new(64., 64.), Color::WHITE),
            };
            let advance = phase_for_window.clone();
            Column::new([
                content,
                incular_widgets::internal::ActionSurface::new("Next")
                    .size(Size::new(96., 24.))
                    .on_press(move || {
                        let _ = advance.set(match advance.get() {
                            Phase::Mask => Phase::Backdrop,
                            Phase::Backdrop => Phase::Plain,
                            Phase::Plain => Phase::Plain,
                        });
                    })
                    .into(),
            ])
            .into()
        })
        .expect("open effect window");
    // Close the bootstrap window explicitly: the event loop returns once
    // no windows remain, so the test window must be the only one left.
    // Leaving bootstrap open would strand the loop here forever.
    assert!(application.close_window(bootstrap));
    assert_eq!(application.active_window_ids().len(), 1);

    let root = application.simulation();
    let sim = root.window(&window);
    let handle = window.clone();
    let (sender, receiver) = mpsc::sync_channel(1);

    let worker = std::thread::spawn(move || {
        // A scenario panic must still close the window below: the event
        // loop only returns once none remain, so an unwinding worker that
        // skips teardown hangs the run instead of failing it.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || -> Result<(), SimulationError> {
                // Phase 1: the mask fails typed with no presentation.
                eprintln!("effect: waiting for mask frame");
                expect_unsupported(&sim, "shader masks are not executed")?;
                eprintln!("effect: mask frame failed typed");
                sim.click("Next")?;
                // Phase 2: the backdrop filter fails typed the same way.
                eprintln!("effect: waiting for backdrop frame");
                expect_unsupported(&sim, "backdrop filters are not executed")?;
                eprintln!("effect: backdrop frame failed typed");
                sim.click("Next")?;
                // Phase 3: corrected content presents; the failure did not
                // wedge the window, and later demand renders normally.
                eprintln!("effect: waiting for plain frame");
                sim.wait_for_frame()?;
                let shot = sim.capture()?;
                assert!(
                    shot.width() > 0 && shot.height() > 0,
                    "plain content must present with dimensions"
                );
                eprintln!("effect: plain frame presented");
                handle.close().expect("close bridge remains active");
                Ok(())
            },
        ));
        // Close however the scenario ended so a failure cannot strand the
        // window and hang the run.
        eprintln!("effect: closing window");
        let _ = handle.close();
        eprintln!("effect: worker done");
        sender.send(result).expect("send effect report");
    });

    // Watchdog while the native loop runs: fail nonzero instead of
    // hanging forever if anything above wedges the event loop. Bounds
    // are tight for three small phases so a wedge surfaces in minutes,
    // not as an indefinite black window.
    let settled = Arc::new(AtomicBool::new(false));
    let watchdog_settled = settled.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(180));
        if !watchdog_settled.load(Ordering::SeqCst) {
            eprintln!("unsupported_effect_execution: watchdog expired before completion");
            std::process::exit(2);
        }
    });

    let run_result = run_application(application);
    eprintln!("effect: event loop returned");
    let report = receiver
        .recv_timeout(Duration::from_secs(90))
        .expect("unsupported effect regression did not complete");
    worker.join().expect("effect worker did not panic");
    run_result.expect("desktop event loop failed");
    report
        .map_err(|_| "unsupported effect worker panicked".to_string())
        .expect("unsupported effect worker panicked")
        .map_err(|error| format!("unsupported effect simulation failed: {error:?}"))
        .expect("unsupported effect simulation failed");
    settled.store(true, Ordering::SeqCst);
}

fn main() {
    if std::env::var("INCULAR_DESKTOP_LIVE_TESTS").as_deref() != Ok("1") {
        eprintln!(
            "unsupported_effect_execution: skipped live native regression; \
             set INCULAR_DESKTOP_LIVE_TESTS=1 to run"
        );
        return;
    }
    unsupported_effect_execution();
}
