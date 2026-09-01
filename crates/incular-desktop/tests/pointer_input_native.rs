use incular_core::{Color, Size};
use incular_desktop::run_application;
use incular_platform::{CapabilitySupport, CursorGrabMode, LogicalWindowPosition, WindowOptions};
use incular_runtime::{Application, Simulation, SimulationError};
use incular_widgets::Widget;
use std::{sync::mpsc, time::Duration};

fn frame(
    simulation: &Simulation,
    handle: &incular_runtime::WindowHandle,
) -> Result<(), SimulationError> {
    handle
        .request_redraw()
        .expect("native pointer test command bridge remains active");
    simulation.wait_for_frame()
}

fn native_pointer_control_smoke() {
    let application = Application::new_with_options(
        WindowOptions {
            initial_logical_size: Size::new(320., 180.),
            ..WindowOptions::new("Incular native pointer smoke")
        },
        |_| Widget::box_(Size::new(320., 180.), Color::BLACK),
    )
    .expect("application");
    let id = application.primary_window();
    let handle = application.window_handle(id).expect("primary handle");
    let simulation = application.simulation().window(&handle);
    let worker_handle = handle.clone();
    let (sender, receiver) = mpsc::sync_channel(1);

    let worker = std::thread::spawn(move || {
        let result = (|| -> Result<(), SimulationError> {
            frame(&simulation, &worker_handle)?;
            let capabilities = worker_handle.capabilities().advanced_input;
            assert_eq!(capabilities.pointer_metadata, CapabilitySupport::Supported);
            assert_eq!(capabilities.cursor_icons, CapabilitySupport::Supported);
            assert_eq!(capabilities.cursor_visibility, CapabilitySupport::Supported);

            pollster::block_on(
                worker_handle
                    .set_cursor_visible(false)
                    .expect("enqueue cursor hide"),
            )
            .expect("hide native cursor");
            pollster::block_on(
                worker_handle
                    .set_cursor_visible(true)
                    .expect("enqueue cursor show"),
            )
            .expect("restore native cursor");

            if capabilities.cursor_position.is_supported() {
                let position = LogicalWindowPosition::new(40., 30.).expect("finite position");
                pollster::block_on(
                    worker_handle
                        .set_cursor_position(position)
                        .expect("enqueue cursor warp"),
                )
                .expect("warp native cursor inside window");
            }

            if capabilities.cursor_confine.is_supported() {
                pollster::block_on(
                    worker_handle
                        .set_cursor_grab(CursorGrabMode::Confined)
                        .expect("enqueue cursor confinement"),
                )
                .expect("confine native cursor");
                pollster::block_on(
                    worker_handle
                        .set_cursor_grab(CursorGrabMode::None)
                        .expect("enqueue cursor release"),
                )
                .expect("release native cursor confinement");
            }

            if capabilities.cursor_lock.is_supported() {
                pollster::block_on(
                    worker_handle
                        .set_cursor_grab(CursorGrabMode::Locked)
                        .expect("enqueue cursor lock"),
                )
                .expect("lock native cursor");
                pollster::block_on(
                    worker_handle
                        .set_cursor_grab(CursorGrabMode::None)
                        .expect("enqueue cursor unlock"),
                )
                .expect("unlock native cursor");
            }

            assert_eq!(worker_handle.id(), id);
            Ok(())
        })();
        let _ = worker_handle.set_cursor_visible(true);
        let _ = worker_handle.set_cursor_grab(CursorGrabMode::None);
        let _ = worker_handle.close();
        sender.send(result).expect("send native pointer result");
    });

    let run_result = run_application(application);
    receiver
        .recv_timeout(Duration::from_secs(30))
        .expect("native pointer smoke timed out")
        .expect("native pointer smoke failed");
    worker.join().expect("native pointer worker panicked");
    run_result.expect("desktop event loop failed");
}

fn main() {
    if std::env::var_os("INCULAR_DESKTOP_LIVE_TESTS").is_none() {
        eprintln!(
            "pointer_input_native: skipped live native regression; set INCULAR_DESKTOP_LIVE_TESTS=1 to run"
        );
        return;
    }
    native_pointer_control_smoke();
}
