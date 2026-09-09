use incular_config::Constraints;
use incular_core::{Code, Color, KeyState, KeyboardEvent, KeyboardKey, Modifiers, Offset, Size};
use incular_platform::WindowId;
use incular_runtime::{
    Application, GpuResourceSummary, Screenshot, Signal, Simulation, SimulationError,
};
use incular_widgets::internal::{ActionSurface, TextEditingController};
use incular_widgets::{
    EditableText, FocusNode, GestureDetector, KeyboardListener, OverlayPortal, Positioned, Text,
    TransientRole, Widget,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

fn drive<T, F>(application: &mut Application, simulation: Simulation, command: F) -> T
where
    T: Send + 'static,
    F: FnOnce(Simulation) -> T + Send + 'static,
{
    let worker = std::thread::spawn(move || command(simulation));
    for _ in 0..2_000 {
        application.process_simulation_requests();
        if worker.is_finished() {
            return worker.join().expect("simulation worker did not panic");
        }
        std::thread::park_timeout(Duration::from_millis(1));
    }
    panic!("simulation request was not serviced");
}

#[test]
fn semantic_click_uses_the_normal_pointer_dispatch_pipeline() {
    let hits = Rc::new(Cell::new(0));
    let mut application = Application::new({
        let hits = hits.clone();
        move |_| {
            ActionSurface::new("Press me")
                .size(Size::new(120., 40.))
                .on_press({
                    let hits = hits.clone();
                    move || hits.set(hits.get() + 1)
                })
                .into()
        }
    })
    .expect("application should build");
    let window = application.primary_window();
    let simulation = application.simulation();
    let result = drive(&mut application, simulation, |simulation| {
        simulation.click("Press me")
    });

    assert_eq!(result, Ok(()));
    assert_eq!(hits.get(), 1);
    assert_eq!(
        application.window_diagnostics(window).unwrap().input_events,
        3
    );
}

#[test]
fn reactive_transient_closes_after_simulated_anchor_activation() {
    let open = Signal::new(true);
    let observed = open.clone();
    let mut application = Application::new(move |_| {
        let toggle = observed.clone();
        let anchor: Widget =
            GestureDetector::new(Widget::box_(Size::new(100.0, 100.0), Color::BLACK))
                .on_tap(move || {
                    let _ = toggle.set(!toggle.get());
                })
                .into();
        let popup: Widget = Positioned::new(Widget::box_(Size::new(80.0, 120.0), Color::WHITE))
            .left(10.0)
            .top(80.0)
            .width(80.0)
            .height(120.0)
            .into();
        OverlayPortal::new(anchor)
            .overlay_child(popup)
            .role(TransientRole::Menu)
            .show(observed.get())
            .into()
    })
    .expect("application should build");
    let window = application.primary_window();
    application
        .run_window_frame_at(
            window,
            Constraints::tight(Size::new(100.0, 100.0)),
            std::time::Instant::now(),
        )
        .expect("initial transient frame");
    assert_eq!(application.transient_surfaces(window).len(), 1);

    let simulation = application.simulation();
    let result = drive(&mut application, simulation, |simulation| {
        simulation.click_at(Offset::new(10.0, 10.0))
    });
    assert_eq!(result, Ok(()));
    assert!(!open.get());

    application
        .run_window_frame_at(
            window,
            Constraints::tight(Size::new(100.0, 100.0)),
            std::time::Instant::now(),
        )
        .expect("closed transient frame");
    assert!(application.transient_surfaces(window).is_empty());
}

#[test]
fn keyboard_commands_preserve_modifier_state_and_use_runtime_keyboard_dispatch() {
    let events = Rc::new(RefCell::new(Vec::<KeyboardEvent>::new()));
    let mut application = Application::new({
        let events = events.clone();
        move |_| {
            let observed_down = events.clone();
            let observed_up = events.clone();
            KeyboardListener::new(Text::new("keyboard target"))
                .focus_node(FocusNode::new())
                .autofocus(true)
                .on_key_down(move |event| observed_down.borrow_mut().push(event))
                .on_key_up(move |event| observed_up.borrow_mut().push(event))
                .into()
        }
    })
    .expect("application should build");

    let simulation = application.simulation();
    let result = drive(&mut application, simulation, |simulation| {
        simulation.key_down(Code::ControlLeft)?;
        simulation.key_down(Code::KeyS)?;
        simulation.key_down(Code::KeyS)?;
        simulation.key_up(Code::KeyS)?;
        simulation.key_up(Code::ControlLeft)
    });
    assert_eq!(result, Ok(()));

    let events = events.borrow();
    assert_eq!(events.len(), 5);
    assert_eq!(events[0].code, Code::ControlLeft);
    assert_eq!(events[0].state, KeyState::Down);
    assert_eq!(events[1].code, Code::KeyS);
    assert_eq!(events[1].state, KeyState::Down);
    assert_eq!(events[1].key, KeyboardKey::Character("s".into()));
    assert!(events[1].modifiers.contains(Modifiers::CONTROL));
    assert_eq!(events[2].code, Code::KeyS);
    assert_eq!(events[2].state, KeyState::Down);
    assert!(events[2].repeat);
    assert!(events[2].modifiers.contains(Modifiers::CONTROL));
    assert_eq!(events[3].code, Code::KeyS);
    assert_eq!(events[3].state, KeyState::Up);
    assert!(events[3].modifiers.contains(Modifiers::CONTROL));
    assert_eq!(events[4].code, Code::ControlLeft);
    assert_eq!(events[4].state, KeyState::Up);
}

#[test]
fn committed_text_reaches_a_focused_editor() {
    let controller = TextEditingController::new();
    let mut application = Application::new({
        let controller = controller.clone();
        move |_| {
            EditableText::new(controller.clone())
                .size(Size::new(160., 40.))
                .into()
        }
    })
    .expect("application should build");

    let simulation = application.simulation();
    let result = drive(&mut application, simulation, |simulation| {
        simulation.click_at(Offset::new(8., 8.))?;
        simulation.type_text("hello from simulation")
    });
    assert_eq!(result, Ok(()));
    assert_eq!(controller.text(), "hello from simulation");
}

#[test]
fn frame_wait_and_capture_complete_only_when_the_adapter_reports_presentation() {
    let mut application =
        Application::new(|_| Text::new("frame").into()).expect("application should build");
    let window = application.primary_window();
    let simulation = application.simulation();
    let frame_worker = std::thread::spawn(move || simulation.wait_for_frame());

    for _ in 0..2_000 {
        application.process_simulation_requests();
        if application.simulation_frame_pending(window) {
            break;
        }
        std::thread::park_timeout(Duration::from_millis(1));
    }
    assert!(application.simulation_frame_pending(window));
    assert!(!frame_worker.is_finished());
    application.complete_simulation_frame(window, false, None);
    assert!(!frame_worker.is_finished());
    application.complete_simulation_frame(window, true, None);
    assert_eq!(frame_worker.join().unwrap(), Ok(()));

    let simulation = application.simulation();
    let capture_worker = std::thread::spawn(move || simulation.capture());
    let screenshot = Screenshot::from_rgba8(2, 1, vec![1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
    for _ in 0..2_000 {
        application.process_simulation_requests();
        if application.simulation_capture_pending(window) {
            break;
        }
        std::thread::park_timeout(Duration::from_millis(1));
    }
    assert!(application.simulation_capture_pending(window));
    assert!(!capture_worker.is_finished());
    application.complete_simulation_frame(window, true, Some(Ok(screenshot.clone())));
    assert_eq!(capture_worker.join().unwrap(), Ok(screenshot));
}

#[test]
fn screenshot_validation_rejects_invalid_pixel_buffers() {
    let screenshot = Screenshot::from_rgba8(2, 1, vec![0; 8]).unwrap();
    assert_eq!(screenshot.width(), 2);
    assert_eq!(screenshot.height(), 1);
    assert_eq!(screenshot.pixels(), &[0; 8]);
    assert!(matches!(
        Screenshot::from_rgba8(2, 1, vec![0; 7]),
        Err(SimulationError::InvalidInput(_))
    ));
    assert!(matches!(
        Screenshot::from_rgba8(0, 1, Vec::new()),
        Err(SimulationError::InvalidInput(_))
    ));
}

fn gpu_summary_fixture() -> GpuResourceSummary {
    GpuResourceSummary {
        shared_image_entries: 7,
        shared_image_evictions: 3,
        glyph_live_pages: 2,
        glyphs_rasterized: 41,
        local_image_entries: 1,
        ..GpuResourceSummary::default()
    }
}

/// Pumps simulation requests until `settled` reports done, then returns
/// the worker result. Fails loudly instead of hanging the test. The
/// worker handle stays owned here because settling closures only borrow
/// it to poll completion.
fn drive_query<F>(
    application: &mut Application,
    worker: std::thread::JoinHandle<Result<GpuResourceSummary, SimulationError>>,
    mut settled: F,
) -> Result<GpuResourceSummary, SimulationError>
where
    F: FnMut(&mut Application),
{
    let mut worker = Some(worker);
    for _ in 0..2_000 {
        application.process_simulation_requests();
        settled(application);
        if worker.as_ref().is_some_and(|worker| worker.is_finished()) {
            return worker
                .take()
                .expect("finished worker")
                .join()
                .expect("simulation worker did not panic");
        }
        std::thread::park_timeout(Duration::from_millis(1));
    }
    panic!("simulation request was not serviced");
}

fn query_in_background(
    simulation: Simulation,
) -> std::thread::JoinHandle<Result<GpuResourceSummary, SimulationError>> {
    std::thread::spawn(move || simulation.query_gpu_resources())
}

/// A resource query completes with the adapter's summary and never asks
/// the window for a frame: observation stays free of presentation. A
/// headless frame first consumes the launch invalidation so the flag
/// below measures only what the query path itself requests.
#[test]
fn gpu_resource_query_completes_without_requesting_a_frame() {
    let mut application =
        Application::new(|_| Text::new("frame").into()).expect("application should build");
    let window = application.primary_window();
    application
        .run_window_frame_at(
            window,
            Constraints::tight(Size::new(80., 60.)),
            Instant::now(),
        )
        .expect("headless frame runs");
    assert!(
        !application.frame_requested(window),
        "the headless frame must consume the launch invalidation"
    );
    let worker = query_in_background(application.simulation());
    let expected = gpu_summary_fixture();
    let result = drive_query(&mut application, worker, |application| {
        for id in application.take_gpu_resource_queries() {
            assert_eq!(id, window);
            assert!(
                !application.frame_requested(window),
                "a resource query must not request a frame"
            );
            application.complete_gpu_resource_query(id, expected);
        }
    });
    assert_eq!(result, Ok(expected));
    assert!(!application.frame_requested(window));
}

/// A query pending when its window closes fails promptly with
/// `WindowClosed`: no maintenance pass is needed to release it.
#[test]
fn gpu_resource_query_pending_when_window_closes() {
    let mut application =
        Application::new(|_| Text::new("frame").into()).expect("application should build");
    let window = application.primary_window();
    let worker = query_in_background(application.simulation());
    let mut closed = false;
    let result = drive_query(&mut application, worker, |application| {
        if !closed && application.take_gpu_resource_queries().contains(&window) {
            assert!(application.close_window(window));
            closed = true;
        }
    });
    assert!(closed, "the query must land before the close");
    assert_eq!(result, Err(SimulationError::WindowClosed(window)));
}

/// A query pending across application shutdown fails with `WindowClosed`
/// through the existing shutdown cleanup, with no desktop maintenance
/// running at all.
#[test]
fn gpu_resource_query_pending_across_shutdown() {
    let mut application =
        Application::new(|_| Text::new("frame").into()).expect("application should build");
    let window = application.primary_window();
    let worker = query_in_background(application.simulation());
    // Land the request first so the waiter genuinely exists pre-shutdown.
    for _ in 0..2_000 {
        application.process_simulation_requests();
        if application.take_gpu_resource_queries().contains(&window) {
            break;
        }
        std::thread::park_timeout(Duration::from_millis(1));
    }
    application.shutdown();
    let result = worker.join().expect("simulation worker did not panic");
    assert_eq!(result, Err(SimulationError::WindowClosed(window)));
}

/// Late completion after closure reports nothing: the waiter is already
/// gone, and the replacement window's own queries are unaffected —
/// generational ids keep the two apart.
#[test]
fn late_gpu_resource_completion_after_closure_reports_nothing() {
    let mut application =
        Application::new(|_| Text::new("frame").into()).expect("application should build");
    let window = application.primary_window();
    let worker = query_in_background(application.simulation());
    let mut observed_close = false;
    let result = drive_query(&mut application, worker, |application| {
        if !observed_close && application.take_gpu_resource_queries().contains(&window) {
            assert!(application.close_window(window));
            observed_close = true;
        }
    });
    assert_eq!(result, Err(SimulationError::WindowClosed(window)));

    // A completion arriving after the close finds no waiter: no success is
    // reported and nothing panics.
    application.complete_gpu_resource_query(window, gpu_summary_fixture());

    // A replacement window (possibly the same slot, never the same id)
    // queries and completes cleanly.
    let replacement: WindowId = application
        .open_window(Default::default(), Text::new("replacement").into())
        .expect("replacement window")
        .id();
    assert_ne!(replacement, window);
    let worker = query_in_background(application.simulation().for_window(replacement));
    let expected = gpu_summary_fixture();
    let result = drive_query(&mut application, worker, |application| {
        for id in application.take_gpu_resource_queries() {
            application.complete_gpu_resource_query(id, expected);
        }
    });
    assert_eq!(result, Ok(expected));
}

/// A live window without fulfillment keeps its query pending across
/// takes: pending (renderer still arriving) is distinct from terminal
/// (close/shutdown fails the waiter), and repeated takes never drop or
/// duplicate it.
#[test]
fn live_window_keeps_query_pending_across_takes() {
    let mut application =
        Application::new(|_| Text::new("frame").into()).expect("application should build");
    let window = application.primary_window();
    application
        .run_window_frame_at(
            window,
            Constraints::tight(Size::new(80., 60.)),
            Instant::now(),
        )
        .expect("headless frame runs");
    let worker = query_in_background(application.simulation());
    let expected = gpu_summary_fixture();
    let mut takes = 0;
    let result = drive_query(&mut application, worker, |application| {
        let pending = application.take_gpu_resource_queries();
        if pending.contains(&window) {
            takes += 1;
            assert!(
                !application.frame_requested(window),
                "repeated takes must not request frames either"
            );
            if takes >= 3 {
                application.complete_gpu_resource_query(window, expected);
            }
        }
    });
    assert!(takes >= 3, "the waiter must survive repeated takes");
    assert_eq!(result, Ok(expected));
}
