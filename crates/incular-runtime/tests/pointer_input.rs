use incular_config::Constraints;
use incular_core::{
    InputEvent, Offset, PRIMARY_POINTER_BUTTON, PointerDeviceKind, PointerPhase,
    SECONDARY_POINTER_BUTTON, Size,
};
use incular_platform::{
    CursorGrabMode, NativeOperationCompletion, PlatformEvent, PlatformOperationError, WindowEvent,
    WindowOperation, WindowOptions,
};
use incular_runtime::{Application, NativeOperationCompletionStatus, NativeWindowCommand};
use incular_widgets::internal::ActionSurface;
use incular_widgets::{Color, Listener, MouseCursor, MouseRegion, Widget};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

fn app(root: Widget) -> Application {
    let retained = root.clone();
    let mut application = Application::new_with_options(
        WindowOptions {
            initial_logical_size: Size::new(100., 40.),
            ..WindowOptions::new("pointer test")
        },
        move |_| retained.clone(),
    )
    .expect("application");
    let id = application.primary_window();
    let _ = application.take_native_window_commands();
    application
        .run_window_frame_at(id, Constraints::tight(Size::new(100., 40.)), Instant::now())
        .expect("initial frame");
    let _ = application.take_native_window_commands();
    application
}

fn pointer(
    application: &mut Application,
    phase: PointerPhase,
    buttons: u32,
    button: Option<u32>,
    position: Offset,
) {
    let id = application.primary_window();
    application.handle_window_event(WindowEvent::platform(
        id,
        PlatformEvent::Input(InputEvent::PointerWithMetadata {
            pointer: 0,
            device: 11,
            kind: PointerDeviceKind::Mouse,
            buttons,
            button,
            phase,
            position,
        }),
    ));
}

#[test]
fn secondary_click_never_activates_an_ordinary_button() {
    let hits = Rc::new(Cell::new(0));
    let button = ActionSurface::new("ordinary")
        .size(Size::new(100., 40.))
        .on_press({
            let hits = hits.clone();
            move || hits.set(hits.get() + 1)
        });
    let mut application = app(button.into());
    let point = Offset::new(20., 20.);

    pointer(
        &mut application,
        PointerPhase::Down,
        SECONDARY_POINTER_BUTTON,
        Some(SECONDARY_POINTER_BUTTON),
        point,
    );
    pointer(
        &mut application,
        PointerPhase::Up,
        0,
        Some(SECONDARY_POINTER_BUTTON),
        point,
    );
    assert_eq!(hits.get(), 0);

    pointer(
        &mut application,
        PointerPhase::Down,
        PRIMARY_POINTER_BUTTON,
        Some(PRIMARY_POINTER_BUTTON),
        point,
    );
    pointer(
        &mut application,
        PointerPhase::Up,
        0,
        Some(PRIMARY_POINTER_BUTTON),
        point,
    );
    assert_eq!(hits.get(), 1);
}

#[test]
fn secondary_chord_does_not_restart_or_finish_primary_press() {
    let hits = Rc::new(Cell::new(0));
    let button = ActionSurface::new("ordinary")
        .size(Size::new(100., 40.))
        .on_press({
            let hits = hits.clone();
            move || hits.set(hits.get() + 1)
        });
    let mut application = app(button.into());
    let point = Offset::new(20., 20.);

    pointer(
        &mut application,
        PointerPhase::Down,
        PRIMARY_POINTER_BUTTON,
        Some(PRIMARY_POINTER_BUTTON),
        point,
    );
    pointer(
        &mut application,
        PointerPhase::Down,
        PRIMARY_POINTER_BUTTON | SECONDARY_POINTER_BUTTON,
        Some(SECONDARY_POINTER_BUTTON),
        point,
    );
    pointer(
        &mut application,
        PointerPhase::Move,
        PRIMARY_POINTER_BUTTON | SECONDARY_POINTER_BUTTON,
        None,
        Offset::new(30., 20.),
    );
    pointer(
        &mut application,
        PointerPhase::Up,
        PRIMARY_POINTER_BUTTON,
        Some(SECONDARY_POINTER_BUTTON),
        Offset::new(30., 20.),
    );
    assert_eq!(
        hits.get(),
        0,
        "secondary release cannot complete the primary press"
    );
    pointer(
        &mut application,
        PointerPhase::Up,
        0,
        Some(PRIMARY_POINTER_BUTTON),
        Offset::new(30., 20.),
    );
    assert_eq!(hits.get(), 1);
}

#[test]
fn raw_listener_receives_changed_secondary_button_and_full_chord() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let listener =
        Listener::new(Widget::box_(Size::new(100., 40.), Color::WHITE)).on_pointer_down({
            let seen = seen.clone();
            move |event| seen.borrow_mut().push((event.buttons, event.button))
        });
    let mut application = app(listener.into());

    pointer(
        &mut application,
        PointerPhase::Down,
        PRIMARY_POINTER_BUTTON,
        Some(PRIMARY_POINTER_BUTTON),
        Offset::new(10., 10.),
    );
    pointer(
        &mut application,
        PointerPhase::Down,
        PRIMARY_POINTER_BUTTON | SECONDARY_POINTER_BUTTON,
        Some(SECONDARY_POINTER_BUTTON),
        Offset::new(10., 10.),
    );
    assert_eq!(
        &*seen.borrow(),
        &[
            (PRIMARY_POINTER_BUTTON, Some(PRIMARY_POINTER_BUTTON)),
            (
                PRIMARY_POINTER_BUTTON | SECONDARY_POINTER_BUTTON,
                Some(SECONDARY_POINTER_BUTTON)
            ),
        ]
    );
}

#[test]
fn cursor_enter_move_exit_resolves_and_clears_effective_cursor() {
    let mut application = app(
        MouseRegion::new(Widget::box_(Size::new(100., 40.), Color::WHITE))
            .cursor(MouseCursor::Click)
            .into(),
    );
    let id = application.primary_window();
    assert_eq!(
        application.window_mouse_cursor(id),
        Some(MouseCursor::Basic)
    );

    pointer(&mut application, PointerPhase::Enter, 0, None, Offset::ZERO);
    assert_eq!(
        application.window_mouse_cursor(id),
        Some(MouseCursor::Basic)
    );
    pointer(
        &mut application,
        PointerPhase::Move,
        0,
        None,
        Offset::new(10., 10.),
    );
    assert_eq!(
        application.window_mouse_cursor(id),
        Some(MouseCursor::Click)
    );
    pointer(
        &mut application,
        PointerPhase::Exit,
        0,
        None,
        Offset::new(10., 10.),
    );
    assert_eq!(
        application.window_mouse_cursor(id),
        Some(MouseCursor::Basic)
    );
}

#[test]
fn cursor_grab_uses_result_bearing_native_operation_contract() {
    let mut application = app(Widget::box_(Size::new(100., 40.), Color::WHITE));
    let id = application.primary_window();
    let handle = application.window_handle(id).expect("window handle");
    let mut request = handle
        .set_cursor_grab(CursorGrabMode::Locked)
        .expect("enqueue cursor lock");
    let command = application
        .take_native_window_commands()
        .into_iter()
        .find_map(|command| match command {
            NativeWindowCommand::Operate(command)
                if command.operation == WindowOperation::SetCursorGrab(CursorGrabMode::Locked) =>
            {
                Some(command)
            }
            _ => None,
        })
        .expect("cursor lock native command");
    assert_eq!(command.request_id, Some(request.id()));
    assert_eq!(
        application.complete_native_operation(NativeOperationCompletion::new(
            id,
            request.id(),
            Err(PlatformOperationError::unsupported()),
        )),
        NativeOperationCompletionStatus::Completed
    );
    assert_eq!(
        request
            .try_result()
            .map(|result| result.map_err(|error| error.kind())),
        Some(Err(
            incular_platform::PlatformOperationErrorKind::Unsupported
        ))
    );
}
