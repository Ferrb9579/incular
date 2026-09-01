use incular_core::{Color, Size, WindowResizeDirection};
use incular_desktop::run_application;
use incular_platform::{
    CapabilitySupport, LogicalSizeLimits, UserAttentionType, WindowLevel, WindowOptions,
};
use incular_runtime::{Application, Simulation, SimulationError};
use incular_widgets::Widget;
use std::sync::mpsc;
use std::time::Duration;

fn frame(
    simulation: &Simulation,
    handle: &incular_runtime::WindowHandle,
) -> Result<(), SimulationError> {
    handle
        .request_redraw()
        .expect("native window command bridge remains active");
    simulation.wait_for_frame()
}

fn native_window_control_smoke() {
    let options = WindowOptions {
        initial_logical_size: Size::new(320., 180.),
        decorations: true,
        ..WindowOptions::new("Incular native window control smoke")
    };
    let application = Application::new_with_options(options, |_| {
        Widget::box_(Size::new(320., 180.), Color::BLACK)
    })
    .expect("application");
    let id = application.primary_window();
    let handle = application
        .window_handle(id)
        .expect("primary window handle");
    let simulation = application.simulation().window(&handle);
    let worker_handle = handle.clone();
    let (sender, receiver) = mpsc::sync_channel(1);

    let worker = std::thread::spawn(move || {
        let result = (|| -> Result<(), SimulationError> {
            frame(&simulation, &worker_handle)?;
            let capabilities = worker_handle.capabilities();
            assert_eq!(
                capabilities.window.set_resizable,
                CapabilitySupport::Supported
            );
            assert_eq!(
                capabilities.window.set_decorations,
                CapabilitySupport::Supported
            );

            worker_handle
                .set_resizable(false)
                .expect("set resizable false");
            worker_handle
                .set_decorations(false)
                .expect("remove native decorations");
            worker_handle
                .set_logical_size_limits(
                    LogicalSizeLimits::new(
                        Some(Size::new(240., 120.)),
                        Some(Size::new(640., 480.)),
                    )
                    .expect("valid native size limits"),
                )
                .expect("set native size limits");
            if capabilities.window.set_window_level.is_supported() {
                worker_handle
                    .set_window_level(WindowLevel::AlwaysOnTop)
                    .expect("raise native window level");
            }
            if capabilities.window.request_user_attention.is_supported() {
                worker_handle
                    .request_user_attention(UserAttentionType::Informational)
                    .expect("request user attention");
                worker_handle
                    .cancel_user_attention()
                    .expect("cancel user attention");
            }
            frame(&simulation, &worker_handle)?;

            worker_handle
                .set_resizable(true)
                .expect("restore resizable");
            worker_handle
                .set_decorations(true)
                .expect("restore decorations");
            worker_handle
                .set_logical_size_limits(LogicalSizeLimits::default())
                .expect("clear native size limits");
            if capabilities.window.set_window_level.is_supported() {
                worker_handle
                    .set_window_level(WindowLevel::Normal)
                    .expect("restore native window level");
            }
            frame(&simulation, &worker_handle)?;

            if capabilities.window.begin_move_drag.is_supported() {
                pollster::block_on(worker_handle.begin_move_drag().expect("enqueue move drag"))
                    .expect("native move drag initiation");
            }
            if capabilities.window.begin_resize_drag.is_supported() {
                pollster::block_on(
                    worker_handle
                        .begin_resize_drag(WindowResizeDirection::SouthEast)
                        .expect("enqueue resize drag"),
                )
                .expect("native resize drag initiation");
            }
            Ok(())
        })();
        let _ = worker_handle.close();
        sender.send(result).expect("send native smoke result");
    });

    let run_result = run_application(application);
    receiver
        .recv_timeout(Duration::from_secs(30))
        .expect("native window-control smoke timed out")
        .expect("native window-control smoke failed");
    worker.join().expect("native control worker panicked");
    run_result.expect("desktop event loop failed");
}

fn main() {
    if std::env::var_os("INCULAR_DESKTOP_LIVE_TESTS").is_none() {
        eprintln!(
            "window_control: skipped live native regression; set INCULAR_DESKTOP_LIVE_TESTS=1 to run"
        );
        return;
    }
    native_window_control_smoke();
}
