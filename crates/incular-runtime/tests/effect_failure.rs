//! Host handling for failed frames: when the backend reports an
//! unsupported effect, the desktop error branch settles waiters through
//! [`Application::fail_simulation_frame`] — the exact production call.
//! Pending frame and capture waiters receive typed errors (never
//! presentation success), with no retry state involved: errors bypass
//! the outcome/retry machine entirely.

use incular_runtime::{Application, SimulationError};
use incular_widgets::Text;
use std::time::Duration;

#[test]
fn unsupported_effect_failure_settles_waiters_without_success() {
    let mut application =
        Application::new(|_| Text::new("frame").into()).expect("application should build");
    let window = application.primary_window();
    let frame_worker = {
        let simulation = application.simulation();
        std::thread::spawn(move || simulation.wait_for_frame())
    };
    let capture_worker = {
        let simulation = application.simulation();
        std::thread::spawn(move || simulation.capture())
    };
    for _ in 0..2_000 {
        application.process_simulation_requests();
        if application.simulation_frame_pending(window)
            && application.simulation_capture_pending(window)
        {
            break;
        }
        std::thread::park_timeout(Duration::from_millis(1));
    }
    assert!(
        application.simulation_frame_pending(window),
        "frame waiter must be registered"
    );
    assert!(
        application.simulation_capture_pending(window),
        "capture waiter must be registered"
    );
    // The exact production call the desktop frame-error branch makes with
    // the renderer's message: waiters settle synchronously, no pump needed.
    application.fail_simulation_frame(
        window,
        "shader masks are not executed by this backend; remove the mask or await backend support",
    );
    let frame = frame_worker.join().expect("frame worker did not panic");
    match frame {
        Err(SimulationError::FrameFailed(message)) => assert!(
            message.contains("shader masks are not executed"),
            "waiter carries the backend message, got {message:?}"
        ),
        other => panic!("frame waiter must fail typed, got {other:?}"),
    }
    let capture = capture_worker.join().expect("capture worker did not panic");
    assert!(
        matches!(capture, Err(SimulationError::CaptureUnavailable(_))),
        "capture waiter must fail typed, got {capture:?}"
    );
    assert!(
        !application.simulation_frame_pending(window)
            && !application.simulation_capture_pending(window),
        "settled waiters must not linger"
    );
}
