#[cfg(target_os = "macos")]
use incular_platform::{TrayItemPresentation, WindowIcon};
#[cfg(target_os = "macos")]
use incular_runtime::Application;
#[cfg(target_os = "macos")]
use incular_widgets::SizedBox;

#[cfg(target_os = "macos")]
fn native_application_shell_smoke() {
    let application = Application::new(|_| SizedBox::new().width(240.0).height(120.0).into())
        .expect("application");
    let shell = application.application_shell();
    let icon = WindowIcon::from_rgba(vec![0x33; 16 * 16 * 4], 16, 16).expect("valid status icon");
    let _tray = shell
        .create_tray_item(
            TrayItemPresentation::new()
                .title("Incular")
                .tooltip("Plan 12 native shell smoke")
                .icon(icon),
            &[],
        )
        .expect("queue native status-item creation");

    let handle = application
        .window_handle(application.primary_window())
        .expect("primary window handle");
    let simulation = application.simulation().window(&handle);
    let worker_handle = handle.clone();
    let worker = std::thread::spawn(move || {
        worker_handle.request_redraw().expect("request first frame");
        simulation.wait_for_frame().expect("native frame");
        worker_handle.close().expect("close native smoke window");
    });

    incular_macos::run_application(application).expect("desktop event loop");
    worker.join().expect("application-shell smoke worker");
    let completions = shell.take_completions();
    assert_eq!(
        completions.len(),
        1,
        "one native status-item create completion"
    );
    assert!(
        completions[0].result.is_ok(),
        "native status-item creation failed: {:?}",
        completions[0].result
    );
}

fn main() {
    #[cfg(target_os = "macos")]
    {
        if std::env::var_os("INCULAR_DESKTOP_LIVE_TESTS").is_none() {
            eprintln!(
                "application_shell: skipped live native regression; set INCULAR_DESKTOP_LIVE_TESTS=1 to run"
            );
            return;
        }
        native_application_shell_smoke();
    }
}
