use incular_config::{Brightness, Constraints, RuntimeEnvironment};
use incular_core::{Color, InputEvent, Offset, PointerPhase, Size};
use incular_platform::{
    PlatformEvent, PlatformLifecycle, WindowEvent, WindowLifecycle, WindowOptions,
};
use incular_runtime::{Application, ApplicationLifecycle, Runtime};
use incular_widgets::{
    Column, EditableText, LayoutBuilder, Widget, internal::TextEditingController,
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
