use incular_core::{Code, KeyState, KeyboardEvent, KeyboardKey, Modifiers, Offset, Size};
use incular_runtime::{Application, Screenshot, Simulation, SimulationError};
use incular_widgets::internal::{ActionSurface, TextEditingController};
use incular_widgets::{EditableText, FocusNode, KeyboardListener, Text};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

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
