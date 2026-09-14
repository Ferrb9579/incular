use incular_config::{Constraints, RuntimeEnvironment};
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
use incular_widgets::{
    Color, ListView, Listener, MouseCursor, MouseRegion, ScrollController, Widget,
};
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
            sample: incular_core::PointerSampleMetadata::default(),
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

#[test]
fn touch_fling_pumps_through_production_frames() {
    // End to end through the production frame path: a fast touch release
    // transfers into a fling tenure, and explicit frame clocks pump it
    // from the drag-end offset to the edge, closing with exactly one
    // End. Wall-clock input times make exact offsets approximate, so
    // this asserts direction, settling, and bracket accounting — exact
    // steps are pinned deterministically at the tree level.
    use std::time::Duration;
    let controller = ScrollController::new();
    let rows: Vec<Widget> = (0..5)
        .map(|_| Widget::box_(Size::new(100., 40.), Color::WHITE))
        .collect();
    let mut application = app(ListView::new(rows).controller(controller.clone()).into());
    let id = application.primary_window();
    let constraints = Constraints::tight(Size::new(100., 40.));
    let ends = Rc::new(Cell::new(0usize));
    let counted = ends.clone();
    let _subscription = controller.add_listener(move |notification| {
        if notification.kind == incular_scroll::ScrollNotificationType::End {
            counted.set(counted.get() + 1);
        }
        false
    });
    let point = Offset::new(50., 30.);
    pointer(
        &mut application,
        PointerPhase::Down,
        PRIMARY_POINTER_BUTTON,
        Some(PRIMARY_POINTER_BUTTON),
        point,
    );
    pointer(
        &mut application,
        PointerPhase::Move,
        PRIMARY_POINTER_BUTTON,
        None,
        Offset::new(50., 20.),
    );
    pointer(
        &mut application,
        PointerPhase::Move,
        PRIMARY_POINTER_BUTTON,
        None,
        Offset::new(50., 10.),
    );
    // Guarantee nonzero release velocity: the release must both move and
    // postdate the previous sample on the wall clock the release details
    // read.
    std::thread::sleep(Duration::from_millis(3));
    pointer(
        &mut application,
        PointerPhase::Up,
        0,
        Some(PRIMARY_POINTER_BUTTON),
        Offset::new(50., 5.),
    );
    let released = controller.offset();
    assert!(released > 0., "drag moved before release");
    // Demand-driven, never a fixed loop: an unconditional fixed loop
    // would keep pumping after lost scheduling and hide the defect, so
    // this loop runs only while the host reports demand (bounded
    // against a stuck scheduler). Every demanded frame must advance
    // the fling until the edge, and settlement must stop demand.
    assert!(application.frame_requested(id), "release schedules frames");
    let mut now = Instant::now();
    let mut frames = 0usize;
    let mut previous = released;
    while application.frame_requested(id) {
        assert!(frames < 600, "fling settles");
        now += Duration::from_millis(16);
        application
            .run_window_frame_at(id, constraints, now)
            .expect("frame pumps fling");
        frames += 1;
        let current = controller.offset();
        assert!(current >= previous, "demanded frames never regress");
        assert!(
            current > previous || current == controller.max_offset(),
            "demanded frames advance until the edge"
        );
        previous = current;
    }
    assert!(frames > 0, "the fling owned the clock");
    assert_eq!(controller.offset(), controller.max_offset());
    assert_eq!(ends.get(), 1, "one bracket from press to settle");
    assert!(controller.begin_activity());
    assert!(controller.end_activity());
}

#[test]
fn touch_fling_interruption_stops_demand() {
    // A fresh drag mid-fling drives under the old tenure until the
    // next pump sees the mismatch and closes it; the slow release
    // keeps no driver, so demand stops after the drain.
    use std::time::Duration;
    let controller = ScrollController::new();
    let rows: Vec<Widget> = (0..5)
        .map(|_| Widget::box_(Size::new(100., 40.), Color::WHITE))
        .collect();
    let mut application = app(ListView::new(rows).controller(controller.clone()).into());
    let id = application.primary_window();
    let constraints = Constraints::tight(Size::new(100., 40.));
    let ends = Rc::new(Cell::new(0usize));
    let counted = ends.clone();
    let _subscription = controller.add_listener(move |notification| {
        if notification.kind == incular_scroll::ScrollNotificationType::End {
            counted.set(counted.get() + 1);
        }
        false
    });
    pointer(
        &mut application,
        PointerPhase::Down,
        PRIMARY_POINTER_BUTTON,
        Some(PRIMARY_POINTER_BUTTON),
        Offset::new(50., 30.),
    );
    pointer(
        &mut application,
        PointerPhase::Move,
        PRIMARY_POINTER_BUTTON,
        None,
        Offset::new(50., 20.),
    );
    pointer(
        &mut application,
        PointerPhase::Move,
        PRIMARY_POINTER_BUTTON,
        None,
        Offset::new(50., 10.),
    );
    std::thread::sleep(Duration::from_millis(3));
    pointer(
        &mut application,
        PointerPhase::Up,
        0,
        Some(PRIMARY_POINTER_BUTTON),
        Offset::new(50., 5.),
    );
    let released = controller.offset();
    assert!(released > 0., "drag moved before release");
    let mut now = Instant::now() + Duration::from_millis(16);
    application
        .run_window_frame_at(id, constraints, now)
        .expect("first fling frame");
    let flung = controller.offset();
    assert!(flung > released, "fling advanced before takeover");
    // A fresh pointer grabs mid-fling and keeps driving upward; it
    // never opens its own bracket while the fling owns the tenure.
    pointer(
        &mut application,
        PointerPhase::Down,
        PRIMARY_POINTER_BUTTON,
        Some(PRIMARY_POINTER_BUTTON),
        Offset::new(50., 30.),
    );
    pointer(
        &mut application,
        PointerPhase::Move,
        PRIMARY_POINTER_BUTTON,
        None,
        Offset::new(50., 20.),
    );
    pointer(
        &mut application,
        PointerPhase::Move,
        PRIMARY_POINTER_BUTTON,
        None,
        Offset::new(50., 10.),
    );
    std::thread::sleep(Duration::from_millis(3));
    pointer(
        &mut application,
        PointerPhase::Up,
        0,
        Some(PRIMARY_POINTER_BUTTON),
        Offset::new(50., 10.),
    );
    let taken = controller.offset();
    assert!(taken > flung, "motion continues under takeover");
    let mut frames = 0usize;
    while application.frame_requested(id) {
        assert!(frames < 30, "takeover drains promptly");
        now += Duration::from_millis(16);
        application
            .run_window_frame_at(id, constraints, now)
            .expect("drain");
        frames += 1;
    }
    assert_eq!(controller.offset(), taken, "no driver resumes");
    assert_eq!(ends.get(), 1, "only the taken-over tenure closes");
    assert!(!application.frame_requested(id));
    assert!(controller.begin_activity());
    assert!(controller.end_activity());
}

#[test]
fn touch_fling_reduced_motion_settles_without_demand() {
    // Reduced motion settles the transferred tenure in place on the
    // next frame: zero travel, one End, and no retained driver — so no
    // further demand.
    use std::time::Duration;
    let controller = ScrollController::new();
    let rows: Vec<Widget> = (0..5)
        .map(|_| Widget::box_(Size::new(100., 40.), Color::WHITE))
        .collect();
    let mut application = app(ListView::new(rows).controller(controller.clone()).into());
    let id = application.primary_window();
    let constraints = Constraints::tight(Size::new(100., 40.));
    let ends = Rc::new(Cell::new(0usize));
    let counted = ends.clone();
    let _subscription = controller.add_listener(move |notification| {
        if notification.kind == incular_scroll::ScrollNotificationType::End {
            counted.set(counted.get() + 1);
        }
        false
    });
    pointer(
        &mut application,
        PointerPhase::Down,
        PRIMARY_POINTER_BUTTON,
        Some(PRIMARY_POINTER_BUTTON),
        Offset::new(50., 30.),
    );
    pointer(
        &mut application,
        PointerPhase::Move,
        PRIMARY_POINTER_BUTTON,
        None,
        Offset::new(50., 20.),
    );
    pointer(
        &mut application,
        PointerPhase::Move,
        PRIMARY_POINTER_BUTTON,
        None,
        Offset::new(50., 10.),
    );
    std::thread::sleep(Duration::from_millis(3));
    pointer(
        &mut application,
        PointerPhase::Up,
        0,
        Some(PRIMARY_POINTER_BUTTON),
        Offset::new(50., 5.),
    );
    let released = controller.offset();
    assert!(released > 0., "drag moved before release");
    // Flip only the motion preference, preserving the window's live
    // metric-derived fields so no unrelated rebuild churn follows.
    application.set_window_environment(
        id,
        RuntimeEnvironment {
            viewport: Size::new(100., 40.),
            physical_width: 100,
            physical_height: 40,
            scale_factor: 1.0,
            reduced_motion: true,
            ..RuntimeEnvironment::default()
        },
    );
    let now = Instant::now() + Duration::from_millis(16);
    application
        .run_window_frame_at(id, constraints, now)
        .expect("reduced-motion frame");
    assert_eq!(controller.offset(), released, "settles in place");
    assert_eq!(ends.get(), 1);
    assert!(
        !application.frame_requested(id),
        "no driver retained, no further demand"
    );
}
