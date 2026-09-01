#[cfg(target_os = "windows")]
use incular_platform::{CapabilitySupport, DisplayPlacementArea};
#[cfg(target_os = "windows")]
use incular_runtime::{Application, Simulation, SimulationError};
#[cfg(target_os = "windows")]
use incular_widgets::SizedBox;
#[cfg(target_os = "windows")]
use std::{sync::mpsc, time::Duration};

#[cfg(target_os = "windows")]
fn frame(
    simulation: &Simulation,
    handle: &incular_runtime::WindowHandle,
) -> Result<(), SimulationError> {
    handle.request_redraw().expect("window command bridge");
    simulation.wait_for_frame()
}

#[cfg(target_os = "windows")]
fn native_display_placement_smoke() {
    let application = Application::new(|_| SizedBox::new().width(320.0).height(180.0).into())
        .expect("application");
    let id = application.primary_window();
    let handle = application.window_handle(id).expect("window handle");
    let simulation = application.simulation().window(&handle);
    let worker_handle = handle.clone();
    let (sender, receiver) = mpsc::sync_channel(1);

    let worker = std::thread::spawn(move || {
        let result = (|| -> Result<(), SimulationError> {
            frame(&simulation, &worker_handle)?;
            let capabilities = worker_handle.capabilities();
            assert_eq!(
                capabilities.display.enumerate_displays,
                CapabilitySupport::Supported
            );
            assert_eq!(
                capabilities.display.query_current_display,
                CapabilitySupport::Supported
            );
            assert_eq!(
                capabilities.display.display_bounds,
                CapabilitySupport::Supported
            );
            assert_eq!(capabilities.display.work_area, CapabilitySupport::Supported);
            assert_eq!(
                capabilities.display.query_window_position,
                CapabilitySupport::Supported
            );
            assert_eq!(
                capabilities.display.set_window_position,
                CapabilitySupport::Supported
            );

            let current = worker_handle
                .current_display()
                .expect("Win32 current monitor should resolve to a published DisplayId");
            assert!(current.physical_bounds.is_some());
            assert!(current.physical_work_area.is_some());
            assert!(
                worker_handle
                    .displays()
                    .iter()
                    .any(|display| display.id == current.id)
            );
            let observed = worker_handle.observed_state();
            assert!(observed.outer_position.is_some());
            assert!(observed.outer_size.is_some());
            assert_eq!(observed.current_display, Some(current.id));

            let target = worker_handle
                .center_on_display(&current, DisplayPlacementArea::WorkArea)
                .expect("center on Win32 work area");
            let mut reached = false;
            for _ in 0..12 {
                frame(&simulation, &worker_handle)?;
                if worker_handle.observed_state().outer_position == Some(target) {
                    reached = true;
                    break;
                }
            }
            assert!(
                reached,
                "native window never reported the requested centered position"
            );
            assert_eq!(worker_handle.id(), id);
            Ok(())
        })();
        let _ = worker_handle.close();
        sender.send(result).expect("send native display result");
    });

    let run_result = incular_windows::run_application(application);
    receiver
        .recv_timeout(Duration::from_secs(30))
        .expect("native display-placement smoke timed out")
        .expect("native display-placement smoke failed");
    worker.join().expect("native display worker panicked");
    run_result.expect("desktop event loop failed");
}

fn main() {
    #[cfg(target_os = "windows")]
    {
        if std::env::var_os("INCULAR_DESKTOP_LIVE_TESTS").is_none() {
            eprintln!(
                "display_placement: skipped live native regression; set INCULAR_DESKTOP_LIVE_TESTS=1 to run"
            );
            return;
        }
        native_display_placement_smoke();
    }
}
