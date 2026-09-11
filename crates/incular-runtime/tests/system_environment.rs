use incular_config::{Brightness, Constraints, EdgeInsets, RuntimeEnvironment};
use incular_core::{Color, InputEvent, Offset, PointerPhase, Size};
use incular_platform::{
    PlatformEvent, PlatformLifecycle, WindowEvent, WindowId, WindowLifecycle, WindowOptions,
};
use incular_rendering::{DisplayList, PaintCommand};
use incular_runtime::{Application, ApplicationLifecycle, Runtime};
use incular_widgets::{
    Column, EditableText, LayoutBuilder, SafeArea, Widget, internal::TextEditingController,
};
use std::{cell::Cell, rc::Rc, time::Instant};

#[test]
fn retained_environment_invalidates_only_consumers_of_changed_fields() {
    let brightness_builds = Rc::new(Cell::new(0_u32));
    let reduced_motion_builds = Rc::new(Cell::new(0_u32));

    let brightness = brightness_builds.clone();
    let reduced_motion = reduced_motion_builds.clone();
    let root: Widget = Column::new(vec![
        Widget::from(LayoutBuilder::new(move |context, _| {
            brightness.set(brightness.get() + 1);
            let _ = context.brightness();
            Widget::box_(Size::new(10.0, 10.0), Color::WHITE)
        })),
        Widget::from(LayoutBuilder::new(move |context, _| {
            reduced_motion.set(reduced_motion.get() + 1);
            let _ = context.reduced_motion();
            Widget::box_(Size::new(10.0, 10.0), Color::WHITE)
        })),
    ])
    .into();

    let mut runtime = Runtime::new(root).expect("runtime");
    let constraints = Constraints::tight(Size::new(100.0, 100.0));
    runtime.run_frame(constraints).expect("initial frame");
    assert_eq!(brightness_builds.get(), 1);
    assert_eq!(reduced_motion_builds.get(), 1);

    assert!(runtime.set_environment(RuntimeEnvironment {
        brightness: Brightness::Dark,
        ..runtime.environment()
    }));
    assert!(runtime.frame_requested());
    runtime.run_frame(constraints).expect("brightness frame");
    assert_eq!(brightness_builds.get(), 2);
    assert_eq!(reduced_motion_builds.get(), 1);

    assert!(runtime.set_environment(RuntimeEnvironment {
        reduced_motion: true,
        ..runtime.environment()
    }));
    assert!(runtime.frame_requested());
    runtime.run_frame(constraints).expect("motion frame");
    assert_eq!(brightness_builds.get(), 2);
    assert_eq!(reduced_motion_builds.get(), 2);
}

#[test]
fn application_lifecycle_is_broadcast_and_inherited_by_new_windows() {
    let mut application =
        Application::new(|_| Widget::box_(Size::new(1.0, 1.0), Color::WHITE)).expect("application");
    let primary = application.primary_window();
    assert_eq!(
        application
            .window_diagnostics(primary)
            .expect("primary diagnostics")
            .application_lifecycle,
        ApplicationLifecycle::Starting
    );

    application.handle_application_lifecycle(PlatformLifecycle::Inactive);
    assert_eq!(
        application
            .window_diagnostics(primary)
            .expect("primary diagnostics")
            .application_lifecycle,
        ApplicationLifecycle::Inactive
    );

    let secondary = application
        .open_window(
            WindowOptions::new("Secondary"),
            Widget::box_(Size::new(1.0, 1.0), Color::WHITE),
        )
        .expect("secondary window")
        .id();
    assert_eq!(
        application
            .window_diagnostics(secondary)
            .expect("secondary diagnostics")
            .application_lifecycle,
        ApplicationLifecycle::Inactive,
        "windows created while inactive inherit application lifecycle"
    );

    application.handle_application_lifecycle(PlatformLifecycle::Resumed);
    for id in [primary, secondary] {
        assert_eq!(
            application
                .window_diagnostics(id)
                .expect("window diagnostics")
                .application_lifecycle,
            ApplicationLifecycle::Active
        );
    }
}

#[test]
fn native_window_unfocus_does_not_clear_logical_text_focus() {
    let controller = TextEditingController::new();
    let field = controller.clone();
    let mut application = Application::new(move |_| {
        EditableText::new(field.clone())
            .size(Size::new(120.0, 40.0))
            .into()
    })
    .expect("application");
    let id = application.primary_window();
    let constraints = Constraints::tight(Size::new(160.0, 80.0));
    application
        .run_window_frame_at(id, constraints, Instant::now())
        .expect("initial frame");

    application.handle_window_event(WindowEvent::platform(
        id,
        PlatformEvent::Input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(5.0, 5.0),
        }),
    ));
    application.handle_window_event(WindowEvent::platform(
        id,
        PlatformEvent::Input(InputEvent::Text("before".into())),
    ));
    assert_eq!(controller.text(), "before");

    application.handle_window_event(WindowEvent::lifecycle(id, WindowLifecycle::Unfocused));
    assert!(
        !application
            .window_diagnostics(id)
            .expect("window diagnostics")
            .native_focused
    );

    application.handle_window_event(WindowEvent::platform(
        id,
        PlatformEvent::Input(InputEvent::Text(" after".into())),
    ));
    assert_eq!(
        controller.text(),
        "before after",
        "native focus loss must not destroy retained logical text focus"
    );
}

#[test]
fn occlusion_invalidation_stays_queued_until_presentation_resumes() {
    let builds = Rc::new(Cell::new(0_u32));
    let observed = builds.clone();
    let root: Widget = LayoutBuilder::new(move |context, _| {
        observed.set(observed.get() + 1);
        let _ = context.window_occluded();
        Widget::box_(Size::new(10.0, 10.0), Color::WHITE)
    })
    .into();
    let mut runtime = Runtime::new(root).expect("runtime");
    let constraints = Constraints::tight(Size::new(100.0, 100.0));
    runtime.run_frame(constraints).expect("initial frame");
    assert_eq!(builds.get(), 1);

    assert!(runtime.set_environment(RuntimeEnvironment {
        window_occluded: true,
        ..runtime.environment()
    }));
    assert!(runtime.frame_requested());
    // A desktop host suppresses native redraw while occluded. Change the
    // environment again before a frame runs: retained invalidation must remain
    // queued rather than being consumed by the visibility transition.
    assert!(runtime.set_environment(RuntimeEnvironment {
        window_occluded: false,
        ..runtime.environment()
    }));
    assert!(runtime.frame_requested());
    assert_eq!(builds.get(), 1);

    runtime.run_frame(constraints).expect("resumed frame");
    assert_eq!(builds.get(), 2);
}

fn bottom_only(value: f32) -> EdgeInsets {
    EdgeInsets::only(0., 0., 0., value)
}

/// Full window snapshot: the environment event replaces rather than
/// merges, so every signal the frame reads is stated explicitly.
fn window_environment(
    safe_bottom: f32,
    padding_bottom: f32,
    occlusion_bottom: f32,
) -> RuntimeEnvironment {
    RuntimeEnvironment {
        viewport: Size::new(200., 200.),
        safe_insets: bottom_only(safe_bottom),
        view_padding: bottom_only(padding_bottom),
        view_insets: bottom_only(occlusion_bottom),
        ..RuntimeEnvironment::default()
    }
}

fn keyboard_hidden() -> RuntimeEnvironment {
    window_environment(20., 20., 0.)
}

fn keyboard_shown() -> RuntimeEnvironment {
    window_environment(0., 20., 180.)
}

fn safe_area_options() -> WindowOptions {
    WindowOptions {
        title: "safe area".into(),
        initial_logical_size: Size::new(200., 200.),
        ..WindowOptions::default()
    }
}

fn maintained_app(builds: &Rc<Cell<u32>>) -> Application {
    let builds = builds.clone();
    Application::new_with_options(safe_area_options(), move |context| {
        builds.set(builds.get() + 1);
        context.safe_area(
            SafeArea::new(Widget::box_(Size::new(50., 50.), Color::WHITE))
                .maintain_bottom_view_padding(true),
        )
    })
    .expect("application")
}

/// Painted box sizes per frame. Origins are not asserted: frame display
/// lists carry local rects with scroll/padding translations applied as
/// separate transforms, so sizes alone prove the resolved padding.
fn painted_sizes(list: &DisplayList) -> Vec<Size> {
    list.commands()
        .iter()
        .filter_map(|command| match command {
            PaintCommand::Rect { rect, .. } => Some(rect.size),
            _ => None,
        })
        .collect()
}

fn frame_with(
    application: &mut Application,
    id: WindowId,
    environment: RuntimeEnvironment,
) -> Vec<Size> {
    application.handle_window_event(WindowEvent::platform(
        id,
        PlatformEvent::Environment(environment),
    ));
    let (list, _) = application
        .run_window_frame_at(
            id,
            Constraints::tight(Size::new(200., 200.)),
            Instant::now(),
        )
        .expect("frame")
        .expect("presented");
    painted_sizes(&list)
}

#[test]
fn context_safe_area_maintains_through_keyboard_cycle() {
    let builds = Rc::new(Cell::new(0_u32));
    let mut application = maintained_app(&builds);
    let id = application.primary_window();
    assert_eq!(builds.get(), 1);

    // Hidden keyboard: the usable 20px margin pads the box to 180px tall.
    assert_eq!(
        frame_with(&mut application, id, keyboard_hidden()),
        vec![Size::new(200., 180.)]
    );
    assert_eq!(builds.get(), 2);
    // Shown keyboard: the usable margin collapses but the maintained edge
    // keeps the persistent 20px — never the 180px occlusion.
    assert_eq!(
        frame_with(&mut application, id, keyboard_shown()),
        vec![Size::new(200., 180.)]
    );
    assert_eq!(builds.get(), 3);
    // Hidden again: back to the usable margin, same geometry.
    assert_eq!(
        frame_with(&mut application, id, keyboard_hidden()),
        vec![Size::new(200., 180.)]
    );
    assert_eq!(builds.get(), 4);
}

#[test]
fn context_safe_area_mounts_while_occluded() {
    let builds = Rc::new(Cell::new(0_u32));
    let mut application = maintained_app(&builds);
    let id = application.primary_window();
    // No history is involved: the first frame under occlusion already
    // derives the maintained padding from the current snapshot.
    assert_eq!(
        frame_with(&mut application, id, keyboard_shown()),
        vec![Size::new(200., 180.)]
    );
    assert_eq!(builds.get(), 2);
}

#[test]
fn context_safe_area_tracks_only_read_dependencies() {
    let builds = Rc::new(Cell::new(0_u32));
    let mut application = maintained_app(&builds);
    let id = application.primary_window();
    assert_eq!(
        frame_with(&mut application, id, keyboard_hidden()),
        vec![Size::new(200., 180.)]
    );
    assert_eq!(builds.get(), 2);

    // Persistent margin 20 -> 30 rebuilds and re-pads to 170.
    assert_eq!(
        frame_with(&mut application, id, window_environment(20., 30., 0.)),
        vec![Size::new(200., 170.)]
    );
    assert_eq!(builds.get(), 3);
    // Occlusion alone rebuilds nothing and pads nothing: the helper never
    // reads transient insets.
    assert_eq!(
        frame_with(&mut application, id, window_environment(20., 30., 180.)),
        vec![Size::new(200., 170.)]
    );
    assert_eq!(builds.get(), 3);
    // Usable margin 20 -> 0 rebuilds; the maintained edge stays at 30.
    assert_eq!(
        frame_with(&mut application, id, window_environment(0., 30., 180.)),
        vec![Size::new(200., 170.)]
    );
    assert_eq!(builds.get(), 4);

    // A non-maintained consumer ignores the persistent margin entirely.
    let plain_builds = Rc::new(Cell::new(0_u32));
    let observed = plain_builds.clone();
    let mut plain = Application::new_with_options(safe_area_options(), move |context| {
        observed.set(observed.get() + 1);
        context.safe_area(SafeArea::new(Widget::box_(
            Size::new(50., 50.),
            Color::WHITE,
        )))
    })
    .expect("application");
    let id = plain.primary_window();
    assert_eq!(
        frame_with(&mut plain, id, keyboard_hidden()),
        vec![Size::new(200., 180.)]
    );
    assert_eq!(plain_builds.get(), 2);
    assert_eq!(
        frame_with(&mut plain, id, window_environment(20., 30., 0.)),
        vec![Size::new(200., 180.)]
    );
    assert_eq!(
        plain_builds.get(),
        2,
        "view padding alone must not rebuild a non-maintained consumer"
    );
}

#[test]
fn context_safe_area_disabled_and_minimum() {
    let mut application = Application::new_with_options(safe_area_options(), |context| {
        context.safe_area(
            SafeArea::new(Widget::box_(Size::new(50., 50.), Color::WHITE))
                .bottom(false)
                .minimum(EdgeInsets::all(5.))
                .maintain_bottom_view_padding(true),
        )
    })
    .expect("application");
    let id = application.primary_window();
    // Disabled bottom stays at the all-edges minimum: 200 - 5 - 5.
    assert_eq!(
        frame_with(&mut application, id, keyboard_shown()),
        vec![Size::new(190., 190.)]
    );
}

#[test]
fn context_safe_area_nested_accumulates() {
    // Each helper call resolves its own level from the ambient snapshot,
    // so nesting accumulates exactly like retained construction: callers
    // scope edges explicitly to avoid double-padding.
    let mut application = Application::new_with_options(safe_area_options(), |context| {
        context.safe_area(
            SafeArea::new(
                context.safe_area(
                    SafeArea::new(Widget::box_(Size::new(50., 50.), Color::WHITE))
                        .maintain_bottom_view_padding(true),
                ),
            )
            .maintain_bottom_view_padding(true),
        )
    })
    .expect("application");
    let id = application.primary_window();
    assert_eq!(
        frame_with(&mut application, id, keyboard_shown()),
        vec![Size::new(200., 160.)]
    );
}
