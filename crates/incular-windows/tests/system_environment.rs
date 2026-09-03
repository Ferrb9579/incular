#[cfg(target_os = "windows")]
use incular_config::RuntimeEnvironment;
#[cfg(target_os = "windows")]
use incular_platform::{PlatformEvent, WindowEvent};
#[cfg(target_os = "windows")]
use incular_runtime::{Application, Simulation, SimulationError};
#[cfg(target_os = "windows")]
use incular_widgets::SizedBox;
#[cfg(target_os = "windows")]
use std::{
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
        mpsc,
    },
    time::Duration,
};
#[cfg(target_os = "windows")]
use windows::UI::ViewManagement::UISettings;

#[cfg(target_os = "windows")]
fn frame(
    simulation: &Simulation,
    handle: &incular_runtime::WindowHandle,
) -> Result<(), SimulationError> {
    handle.request_redraw().expect("window command bridge");
    simulation.wait_for_frame()
}

#[cfg(target_os = "windows")]
fn native_text_scale_smoke() {
    let expected = UISettings::new()
        .expect("UISettings must activate on the native UI thread")
        .TextScaleFactor()
        .expect("Windows text scale must be readable") as f32;
    assert!((1.0..=2.25).contains(&expected));

    let observed = Arc::new(AtomicU32::new(f32::NAN.to_bits()));
    let observed_for_build = Arc::clone(&observed);
    let mut application = Application::new(move |context| {
        observed_for_build.store(context.text_scale().to_bits(), Ordering::SeqCst);
        SizedBox::new().width(240.0).height(120.0).into()
    })
    .expect("application");
    let id = application.primary_window();

    // Prove the native adapter replaces an existing runtime value instead of
    // merely leaving the framework default in place. 0.5 is valid to the
    // generic runtime but outside Windows' documented Text Size range.
    application.handle_window_event(WindowEvent::platform(
        id,
        PlatformEvent::Environment(RuntimeEnvironment {
            text_scale: 0.5,
            ..RuntimeEnvironment::default()
        }),
    ));
    assert_eq!(f32::from_bits(observed.load(Ordering::SeqCst)), 0.5);

    let handle = application.window_handle(id).expect("window handle");
    let simulation = application.simulation().window(&handle);
    let worker_handle = handle.clone();
    let worker_observed = Arc::clone(&observed);
    let (sender, receiver) = mpsc::sync_channel(1);

    let worker = std::thread::spawn(move || {
        let result = (|| -> Result<(), SimulationError> {
            for _ in 0..12 {
                frame(&simulation, &worker_handle)?;
                if f32::from_bits(worker_observed.load(Ordering::SeqCst)) == expected {
                    return Ok(());
                }
            }
            panic!(
                "native UISettings text scale never replaced seeded runtime value: expected {expected}, observed {}",
                f32::from_bits(worker_observed.load(Ordering::SeqCst))
            );
        })();
        let _ = worker_handle.close();
        sender.send(result).expect("send native environment result");
    });

    let run_result = incular_windows::run_application(application);
    receiver
        .recv_timeout(Duration::from_secs(30))
        .expect("native system-environment smoke timed out")
        .expect("native system-environment smoke failed");
    worker.join().expect("native environment worker panicked");
    run_result.expect("desktop event loop failed");
}

fn main() {
    #[cfg(target_os = "windows")]
    {
        if std::env::var_os("INCULAR_DESKTOP_LIVE_TESTS").is_none() {
            eprintln!(
                "system_environment: skipped live native regression; set INCULAR_DESKTOP_LIVE_TESTS=1 to run"
            );
            return;
        }
        native_text_scale_smoke();
    }
}
