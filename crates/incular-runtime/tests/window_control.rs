use incular_config::Constraints;
use incular_core::{Color, InputEvent, Offset, PointerPhase, Size, WindowResizeDirection};
use incular_platform::{
    Fullscreen, LogicalSizeLimits, NativeOperationCompletion, PlatformEvent, TextInputCommand,
    UserAttentionType, WindowCommand, WindowEvent, WindowObservedState, WindowOperation,
    WindowOptions,
};
use incular_runtime::{Application, NativeOperationCompletionStatus, NativeWindowCommand};
use incular_text::TextEditingController;
use incular_widgets::internal::{ActionId, action};
use incular_widgets::{Column, EditableText, Widget, WindowDragRegion, WindowResizeRegion};
use std::time::Instant;

fn options() -> WindowOptions {
    WindowOptions {
        title: "custom chrome".into(),
        initial_logical_size: Size::new(120., 40.),
        decorations: false,
        ..WindowOptions::default()
    }
}

fn application_with(root: Widget) -> Application {
    let retained = root.clone();
    let mut application =
        Application::new_with_options(options(), move |_| retained.clone()).expect("application");
    let id = application.primary_window();
    let _ = application.take_native_window_commands();
    application
        .run_window_frame_at(id, Constraints::tight(Size::new(120., 40.)), Instant::now())
        .expect("initial frame");
    let _ = application.take_native_window_commands();
    application
}

fn pointer_down(application: &mut Application, id: incular_platform::WindowId, point: Offset) {
    application.handle_window_event(WindowEvent::platform(
        id,
        PlatformEvent::Input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: point,
        }),
    ));
}

fn operations(application: &mut Application) -> Vec<WindowCommand> {
    application
        .take_native_window_commands()
        .into_iter()
        .filter_map(|command| match command {
            NativeWindowCommand::Operate(command) => Some(command),
            NativeWindowCommand::Create { .. } => None,
        })
        .collect()
}

#[test]
fn drag_region_emits_native_move_for_unhandled_primary_space() {
    let root = WindowDragRegion::new(Widget::box_(Size::new(120., 40.), Color::WHITE)).into();
    let mut application = application_with(root);
    let id = application.primary_window();

    pointer_down(&mut application, id, Offset::new(40., 20.));
    let commands = operations(&mut application);
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].window_id, id);
    assert_eq!(commands[0].operation, WindowOperation::BeginMoveDrag);
    assert!(
        commands[0].request_id.is_none(),
        "widget chrome has no result consumer; desktop records native failure in diagnostics"
    );
}

#[test]
fn interactive_child_wins_over_drag_region() {
    let button = action(Size::new(120., 40.), Color::WHITE, ActionId(7));
    let mut application = application_with(WindowDragRegion::new(button).into());
    let id = application.primary_window();

    pointer_down(&mut application, id, Offset::new(40., 20.));
    assert!(operations(&mut application).iter().all(|command| {
        !matches!(
            command.operation,
            WindowOperation::BeginMoveDrag | WindowOperation::BeginResizeDrag(_)
        )
    }));
}

#[test]
fn all_resize_regions_emit_the_exact_native_direction() {
    for direction in [
        WindowResizeDirection::East,
        WindowResizeDirection::North,
        WindowResizeDirection::NorthEast,
        WindowResizeDirection::NorthWest,
        WindowResizeDirection::South,
        WindowResizeDirection::SouthEast,
        WindowResizeDirection::SouthWest,
        WindowResizeDirection::West,
    ] {
        let root =
            WindowResizeRegion::new(direction, Widget::box_(Size::new(120., 40.), Color::WHITE))
                .into();
        let mut application = application_with(root);
        let id = application.primary_window();
        pointer_down(&mut application, id, Offset::new(20., 20.));
        let commands = operations(&mut application);
        assert!(
            commands.iter().any(|command| {
                command.operation == WindowOperation::BeginResizeDrag(direction)
            })
        );
    }
}

#[test]
fn dragging_titlebar_does_not_clear_an_existing_text_input_client() {
    let controller = TextEditingController::with_text("editable");
    let header: Widget =
        WindowDragRegion::new(Widget::box_(Size::new(120., 20.), Color::WHITE)).into();
    let editor: Widget = EditableText::new(controller)
        .size(Size::new(120., 20.))
        .into();
    let root: Widget = Column::new([header, editor]).into();
    let mut application = application_with(root);
    let id = application.primary_window();

    pointer_down(&mut application, id, Offset::new(30., 30.));
    let initial = application.take_window_text_input_commands(id);
    assert!(
        initial
            .iter()
            .any(|command| matches!(command, TextInputCommand::SetClient { .. }))
    );

    pointer_down(&mut application, id, Offset::new(30., 10.));
    let after_drag = application.take_window_text_input_commands(id);
    assert!(
        after_drag.iter().all(|command| !matches!(
            command,
            TextInputCommand::Clear { .. } | TextInputCommand::Hide { .. }
        )),
        "native titlebar drag must not blur the retained editor"
    );
    assert!(
        operations(&mut application)
            .iter()
            .any(|command| { command.operation == WindowOperation::BeginMoveDrag })
    );
}

#[test]
fn explicit_drag_requests_use_plan01_result_completion_contract() {
    let mut application = application_with(Widget::box_(Size::new(120., 40.), Color::WHITE));
    let id = application.primary_window();
    let handle = application.window_handle(id).expect("window handle");

    let mut request = handle.begin_move_drag().expect("enqueue drag request");
    let commands = operations(&mut application);
    assert!(commands.iter().any(|command| {
        command.operation == WindowOperation::BeginMoveDrag
            && command.request_id == Some(request.id())
    }));
    assert_eq!(
        application.complete_native_operation(NativeOperationCompletion::new(
            id,
            request.id(),
            Ok(()),
        )),
        NativeOperationCompletionStatus::Completed
    );
    assert_eq!(request.try_result(), Some(Ok(())));

    let direction = WindowResizeDirection::NorthWest;
    let mut resize = handle
        .begin_resize_drag(direction)
        .expect("enqueue resize request");
    let commands = operations(&mut application);
    assert!(commands.iter().any(|command| {
        command.operation == WindowOperation::BeginResizeDrag(direction)
            && command.request_id == Some(resize.id())
    }));
    assert_eq!(
        application.complete_native_operation(NativeOperationCompletion::new(
            id,
            resize.id(),
            Ok(()),
        )),
        NativeOperationCompletionStatus::Completed
    );
    assert_eq!(resize.try_result(), Some(Ok(())));
}

#[test]
fn requested_window_state_changes_before_observed_native_state() {
    let mut application = application_with(Widget::box_(Size::new(120., 40.), Color::WHITE));
    let id = application.primary_window();
    let handle = application.window_handle(id).expect("window handle");
    let limits = LogicalSizeLimits::new(Some(Size::new(100., 30.)), Some(Size::new(600., 400.)))
        .expect("limits");

    handle.set_maximized(true).expect("maximize");
    handle.set_resizable(false).expect("resizable");
    handle.set_decorations(true).expect("decorations");
    handle
        .set_fullscreen(Some(Fullscreen::Borderless))
        .expect("fullscreen");
    handle.set_logical_size_limits(limits).expect("size limits");
    handle
        .request_user_attention(UserAttentionType::Informational)
        .expect("attention");
    let commands = operations(&mut application);
    assert_eq!(commands.len(), 6);
    assert!(
        commands.iter().all(|command| command.window_id == id),
        "mutable window state must stay on the same generational Incular window identity"
    );

    let diagnostics = application.window_diagnostics(id).expect("diagnostics");
    assert!(diagnostics.requested_state.maximized);
    assert!(!diagnostics.requested_state.resizable);
    assert!(diagnostics.requested_state.decorations);
    assert_eq!(
        diagnostics.requested_state.fullscreen,
        Some(Fullscreen::Borderless)
    );
    assert_eq!(diagnostics.requested_state.size_limits, limits);
    assert_eq!(diagnostics.observed_state, WindowObservedState::default());

    let observed = WindowObservedState {
        visible: Some(true),
        minimized: Some(false),
        maximized: Some(false),
        fullscreen: Some(false),
        resizable: Some(true),
        decorations: Some(false),
    };
    application.handle_window_event(WindowEvent::state_changed(id, observed));
    let diagnostics = application.window_diagnostics(id).expect("diagnostics");
    assert_eq!(diagnostics.observed_state, observed);
    assert!(
        diagnostics.requested_state.maximized,
        "observed OS state must not overwrite the application's requested state"
    );
}
