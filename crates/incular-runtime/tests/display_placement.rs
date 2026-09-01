use incular_core::{Color, Size};
use incular_platform::{
    CapabilitySupport, DisplayId, DisplayPlacementArea, DisplaySnapshot, PhysicalDisplayPosition,
    PhysicalScreenPosition, PhysicalScreenRect, PhysicalSize, PlatformCapabilities, PlatformEvent,
    WindowEvent, WindowMetrics, WindowObservedState, WindowOperation, WindowOptions,
};
use incular_runtime::{Application, NativeWindowCommand, WindowPlacementError};
use incular_widgets::Widget;

fn display(
    id: DisplayId,
    scale_factor: f64,
    bounds: Option<PhysicalScreenRect>,
    work_area: Option<PhysicalScreenRect>,
    primary: bool,
) -> DisplaySnapshot {
    let physical_size = bounds.map_or(PhysicalSize::new(1_920, 1_080), |rect| rect.size);
    DisplaySnapshot {
        id,
        name: Some(format!("display-{}", id.index())),
        scale_factor,
        physical_size,
        physical_bounds: bounds,
        logical_bounds: None,
        physical_work_area: work_area,
        logical_work_area: None,
        is_primary: primary,
    }
}

fn app() -> Application {
    let mut application = Application::new_with_options(
        WindowOptions {
            initial_logical_size: Size::new(200., 100.),
            ..WindowOptions::new("display placement")
        },
        |_| Widget::box_(Size::new(200., 100.), Color::BLACK),
    )
    .expect("application");
    let _ = application.take_native_window_commands();
    application
}

fn placement_capabilities() -> PlatformCapabilities {
    let mut capabilities = PlatformCapabilities::unsupported();
    capabilities.display.enumerate_displays = CapabilitySupport::Supported;
    capabilities.display.query_current_display = CapabilitySupport::Supported;
    capabilities.display.display_bounds = CapabilitySupport::Supported;
    capabilities.display.work_area = CapabilitySupport::Supported;
    capabilities.display.query_window_position = CapabilitySupport::Supported;
    capabilities.display.set_window_position = CapabilitySupport::Supported;
    capabilities
}

fn operations(application: &mut Application) -> Vec<WindowOperation> {
    application
        .take_native_window_commands()
        .into_iter()
        .filter_map(|command| match command {
            NativeWindowCommand::Operate(command) => Some(command.operation),
            NativeWindowCommand::Create { .. } => None,
        })
        .collect()
}

#[test]
fn display_catalog_is_visible_through_window_handles_and_centers_in_work_area() {
    let mut application = app();
    application.set_platform_capabilities(placement_capabilities());
    let id = application.primary_window();
    let handle = application.window_handle(id).expect("window handle");
    let display_id = DisplayId::from_parts(3, 1);
    let snapshot = display(
        display_id,
        1.25,
        Some(PhysicalScreenRect::new(1_920, 0, 2_560, 1_440)),
        Some(PhysicalScreenRect::new(1_920, 0, 2_560, 1_400)),
        true,
    );
    application.publish_displays([snapshot.clone()], Some(display_id));
    application.handle_window_event(WindowEvent::state_changed(
        id,
        WindowObservedState {
            outer_position: Some(PhysicalScreenPosition::new(2_000, 100)),
            outer_size: Some(PhysicalSize::new(500, 250)),
            current_display: Some(display_id),
            ..WindowObservedState::default()
        },
    ));

    assert_eq!(handle.displays(), vec![snapshot.clone()]);
    assert_eq!(handle.primary_display(), Some(snapshot.clone()));
    assert_eq!(handle.current_display(), Some(snapshot.clone()));
    let target = handle
        .center_on_display(&snapshot, DisplayPlacementArea::WorkArea)
        .expect("center on work area");
    assert_eq!(target, PhysicalScreenPosition::new(2_950, 575));
    assert_eq!(
        operations(&mut application),
        [WindowOperation::SetOuterPosition(target)]
    );
}

#[test]
fn display_removal_invalidates_old_generation_and_window_association() {
    let mut application = app();
    let id = application.primary_window();
    let handle = application.window_handle(id).expect("window handle");
    let old_id = DisplayId::from_parts(1, 4);
    let replacement_id = DisplayId::from_parts(1, 5);
    let old = display(
        old_id,
        1.0,
        Some(PhysicalScreenRect::new(0, 0, 1_920, 1_080)),
        None,
        true,
    );
    application.publish_displays([old], Some(old_id));
    application.handle_window_event(WindowEvent::state_changed(
        id,
        WindowObservedState {
            current_display: Some(old_id),
            ..WindowObservedState::default()
        },
    ));
    assert_eq!(handle.current_display().map(|value| value.id), Some(old_id));

    let replacement = display(
        replacement_id,
        1.0,
        Some(PhysicalScreenRect::new(0, 0, 1_920, 1_080)),
        None,
        true,
    );
    application.publish_displays([replacement.clone()], Some(replacement_id));
    assert!(handle.display(old_id).is_none());
    assert!(handle.current_display().is_none());
    application.handle_window_event(WindowEvent::state_changed(
        id,
        WindowObservedState {
            current_display: Some(replacement_id),
            ..WindowObservedState::default()
        },
    ));
    assert_eq!(handle.current_display(), Some(replacement));
}

#[test]
fn unsupported_global_placement_never_enqueues_a_position_command() {
    let mut application = app();
    application.set_platform_capabilities(PlatformCapabilities::unsupported());
    let id = application.primary_window();
    let handle = application.window_handle(id).expect("window handle");
    let display_id = DisplayId::from_parts(0, 0);
    let snapshot = display(display_id, 2.0, None, None, true);
    application.publish_displays([snapshot.clone()], Some(display_id));
    application.handle_window_event(WindowEvent::state_changed(
        id,
        WindowObservedState {
            outer_size: Some(PhysicalSize::new(400, 200)),
            current_display: Some(display_id),
            ..WindowObservedState::default()
        },
    ));

    assert_eq!(
        handle.center_on_display(&snapshot, DisplayPlacementArea::FullBounds),
        Err(WindowPlacementError::Unsupported)
    );
    assert_eq!(
        handle.move_relative_to_display(&snapshot, PhysicalDisplayPosition::new(10, 20)),
        Err(WindowPlacementError::Unsupported)
    );
    assert!(operations(&mut application).is_empty());
}

#[test]
fn moving_between_mixed_dpi_displays_updates_scale_without_replacing_window() {
    let mut application = app();
    let id = application.primary_window();
    let handle = application.window_handle(id).expect("window handle");
    let first_id = DisplayId::from_parts(0, 0);
    let second_id = DisplayId::from_parts(1, 0);
    application.publish_displays(
        [
            display(
                first_id,
                1.0,
                Some(PhysicalScreenRect::new(0, 0, 1_920, 1_080)),
                None,
                true,
            ),
            display(
                second_id,
                2.0,
                Some(PhysicalScreenRect::new(1_920, 0, 3_840, 2_160)),
                None,
                false,
            ),
        ],
        Some(first_id),
    );
    application.handle_window_event(WindowEvent::state_changed(
        id,
        WindowObservedState {
            current_display: Some(first_id),
            ..WindowObservedState::default()
        },
    ));
    application.handle_window_event(WindowEvent::platform(
        id,
        PlatformEvent::Metrics(WindowMetrics::new(PhysicalSize::new(200, 100), 1.0)),
    ));
    let generation = application
        .window_diagnostics(id)
        .expect("window")
        .surface_generation;

    application.handle_window_event(WindowEvent::platform(
        id,
        PlatformEvent::Metrics(WindowMetrics::new(PhysicalSize::new(400, 200), 2.0)),
    ));
    application.handle_window_event(WindowEvent::state_changed(
        id,
        WindowObservedState {
            current_display: Some(second_id),
            ..WindowObservedState::default()
        },
    ));

    assert_eq!(handle.id(), id);
    assert_eq!(
        handle.current_display().map(|value| value.id),
        Some(second_id)
    );
    let diagnostics = application.window_diagnostics(id).expect("same window");
    assert_eq!(diagnostics.scale_factor, 2.0);
    assert_eq!(diagnostics.logical_size, Size::new(200., 100.));
    assert!(diagnostics.surface_generation > generation);
}
