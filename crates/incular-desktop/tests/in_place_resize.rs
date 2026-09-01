use incular_config::WindowSizePolicy;
use incular_core::{Color, Size};
use incular_desktop::run_application;
use incular_platform::WindowOptions;
use incular_runtime::{Application, Simulation, SimulationError};
use incular_widgets::internal::ActionSurface;
use incular_widgets::{SizedBox, Widget};
use std::sync::mpsc;
use std::time::Duration;

const WIDTH: f32 = 184.0;
const COMPACT_HEIGHT: f32 = 54.0;
const EXPANDED_HEIGHT: f32 = 154.0;

#[derive(Debug)]
struct ResizeCaptureReport {
    initial: (u32, u32),
    expanded: (u32, u32),
    compact: (u32, u32),
}

fn window_options(size_policy: WindowSizePolicy) -> WindowOptions {
    WindowOptions {
        initial_logical_size: Size::new(WIDTH, COMPACT_HEIGHT),
        size_policy,
        decorations: false,
        ..WindowOptions::new("Incular in-place resize regression")
    }
}

fn settle(simulation: &Simulation) -> Result<(), SimulationError> {
    // A programmatic resize can be acknowledged synchronously by the native
    // backend or asynchronously by a later Resized event. Waiting for several
    // presented frames makes this regression valid for both contracts without
    // sleeping or guessing backend timing.
    for _ in 0..3 {
        simulation.wait_for_frame()?;
    }
    Ok(())
}

fn dimensions(simulation: &Simulation) -> Result<(u32, u32), SimulationError> {
    let capture = simulation.capture()?;
    Ok((capture.width(), capture.height()))
}

fn expected_height(initial_width: u32, logical_height: f32) -> u32 {
    let scale_factor = f64::from(initial_width) / f64::from(WIDTH);
    (f64::from(logical_height) * scale_factor).round() as u32
}

fn assert_report(report: &ResizeCaptureReport) {
    assert_eq!(report.initial.0, report.expanded.0);
    assert_eq!(report.initial.0, report.compact.0);
    assert_eq!(
        report.initial.1,
        expected_height(report.initial.0, COMPACT_HEIGHT)
    );
    assert_eq!(
        report.expanded.1,
        expected_height(report.initial.0, EXPANDED_HEIGHT),
        "post-resize capture must use the expanded physical surface"
    );
    assert_eq!(
        report.compact.1,
        expected_height(report.initial.0, COMPACT_HEIGHT),
        "shrinking in place must restore the compact physical surface"
    );
}

fn programmatic_and_content_resizes_update_capture_surfaces_in_place() {
    let expanded = incular_runtime::Signal::new(false);
    let observed = expanded.clone();
    let mut application =
        Application::new(|_| SizedBox::new().into()).expect("bootstrap application");
    let bootstrap = application.primary_window();
    let explicit = application
        .open_window_with(window_options(WindowSizePolicy::Viewport), |_| {
            Widget::box_(Size::new(WIDTH, COMPACT_HEIGHT), Color::TRANSPARENT)
        })
        .expect("open explicit resize target");
    let content = application
        .open_window_with(window_options(WindowSizePolicy::Content), move |_| {
            let is_expanded = observed.get();
            let state = observed.clone();
            let button = ActionSurface::new(if is_expanded { "Shrink" } else { "Expand" })
                .size(Size::new(96.0, 40.0))
                .on_press(move || {
                    let _ = state.set(!state.get());
                });
            SizedBox::new()
                .width(WIDTH)
                .height(if is_expanded {
                    EXPANDED_HEIGHT
                } else {
                    COMPACT_HEIGHT
                })
                .child(button)
                .into()
        })
        .expect("open content-sized resize target");
    assert!(application.close_window(bootstrap));
    let active = application.active_window_ids();
    assert_eq!(active.len(), 2);
    assert!(active.contains(&explicit.id()));
    assert!(active.contains(&content.id()));

    let root_simulation = application.simulation();
    let explicit_simulation = root_simulation.window(&explicit);
    let content_simulation = root_simulation.window(&content);
    assert_eq!(explicit_simulation.window_id(), explicit.id());
    assert_eq!(content_simulation.window_id(), content.id());

    let explicit_handle = explicit.clone();
    let content_handle = content.clone();
    let (sender, receiver) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        let result = (|| -> Result<(ResizeCaptureReport, ResizeCaptureReport), SimulationError> {
            settle(&explicit_simulation)?;
            let initial = dimensions(&explicit_simulation)?;
            assert!(
                explicit_handle
                    .request_logical_size(Size::new(WIDTH, EXPANDED_HEIGHT))
                    .is_ok()
            );
            settle(&explicit_simulation)?;
            let expanded = dimensions(&explicit_simulation)?;
            assert_eq!(explicit_simulation.window_id(), explicit_handle.id());
            assert!(
                explicit_handle
                    .request_logical_size(Size::new(WIDTH, COMPACT_HEIGHT))
                    .is_ok()
            );
            settle(&explicit_simulation)?;
            let compact = dimensions(&explicit_simulation)?;
            assert_eq!(explicit_simulation.window_id(), explicit_handle.id());
            let explicit_report = ResizeCaptureReport {
                initial,
                expanded,
                compact,
            };

            settle(&content_simulation)?;
            let initial = dimensions(&content_simulation)?;
            content_simulation.click("Expand")?;
            settle(&content_simulation)?;
            let expanded = dimensions(&content_simulation)?;
            assert_eq!(content_simulation.window_id(), content_handle.id());
            content_simulation.click("Shrink")?;
            settle(&content_simulation)?;
            let compact = dimensions(&content_simulation)?;
            assert_eq!(content_simulation.window_id(), content_handle.id());
            let content_report = ResizeCaptureReport {
                initial,
                expanded,
                compact,
            };
            Ok((explicit_report, content_report))
        })();

        // Close through the same generational handles. No resize path is
        // allowed to replace either native window or its Incular identity.
        let _ = explicit_handle.close();
        let _ = content_handle.close();
        sender.send(result).expect("send resize reports");
    });

    let run_result = run_application(application);
    let (explicit_report, content_report) = receiver
        .recv_timeout(Duration::from_secs(30))
        .expect("desktop resize regression did not complete")
        .expect("desktop resize simulation failed");
    worker.join().expect("resize worker did not panic");
    run_result.expect("desktop event loop failed");
    assert_report(&explicit_report);
    assert_report(&content_report);
}

fn main() {
    if std::env::var_os("INCULAR_DESKTOP_LIVE_TESTS").is_none() {
        eprintln!(
            "in_place_resize: skipped live native regression; set INCULAR_DESKTOP_LIVE_TESTS=1 to run"
        );
        return;
    }
    programmatic_and_content_resizes_update_capture_surfaces_in_place();
}
