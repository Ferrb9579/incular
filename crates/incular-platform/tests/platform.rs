use incular_core::{
    BACK_POINTER_BUTTON, Code, Color, FORWARD_POINTER_BUTTON, InputEvent, KeyboardEvent,
    KeyboardKey, NamedKey, Offset, PRIMARY_POINTER_BUTTON, PointerPhase, SECONDARY_POINTER_BUTTON,
    Size, TERTIARY_POINTER_BUTTON, additional_pointer_button_mask,
};
use incular_platform::*;

#[test]
fn window_id_generation_distinguishes_reused_slots() {
    let closed = WindowId::from_parts(7, 2);
    let replacement = WindowId::from_parts(7, 3);

    assert_ne!(closed, replacement);
    assert_eq!(replacement.index(), 7);
    assert_eq!(replacement.generation(), 3);
    assert_eq!(replacement.to_string(), "Window#8@3");
}

#[test]
fn window_options_use_the_shared_application_defaults() {
    let defaults = incular_config::ApplicationDefaults::DEFAULT;
    let options = WindowOptions::default();

    assert_eq!(options.title, defaults.window_title);
    assert_eq!(options.initial_logical_size, defaults.initial_window_size);
    assert_eq!(options.size_policy, defaults.window_size_policy);
    assert_eq!(options.resizable, defaults.resizable);
    assert_eq!(options.visible, defaults.visible);
    assert_eq!(options.decorations, defaults.decorations);
    assert_eq!(options.transparency_mode, defaults.transparency_mode);
    assert_eq!(options.background_color, defaults.background_color);
    assert_eq!(options.maximized, defaults.maximized);
    assert!(options.fullscreen.is_none());
    assert_eq!(options.window_level, WindowLevel::Normal);
    assert!(options.window_icon.is_none());
}

#[test]
fn content_sizing_is_explicit_and_independent_of_decorations() {
    let mut options = WindowOptions::new("Palette");
    options.decorations = false;
    assert_eq!(options.size_policy, WindowSizePolicy::Viewport);
    options.size_policy = WindowSizePolicy::Content;
    assert_eq!(options.size_policy, WindowSizePolicy::Content);
    assert!(!options.decorations);
}

#[test]
fn transparency_mode_and_background_color_are_independent() {
    let mut options = WindowOptions::default();
    assert_eq!(options.transparency_mode, TransparencyMode::Opaque);
    assert_eq!(options.background_color, Color::BLACK);
    options.transparency_mode = TransparencyMode::Transparent;
    options.background_color = Color::TRANSPARENT;
    assert_eq!(options.transparency_mode, TransparencyMode::Transparent);
    assert_eq!(options.background_color, Color::TRANSPARENT);
}

#[test]
fn window_options_validate_portable_size_constraints() {
    let mut options = WindowOptions::new("Inspector");
    options.initial_logical_size = Size::new(800.0, 600.0);
    options.minimum_logical_size = Some(Size::new(400.0, 300.0));
    options.maximum_logical_size = Some(Size::new(1_600.0, 1_200.0));
    assert_eq!(options.validate(), Ok(()));

    options.minimum_logical_size = Some(Size::new(1_800.0, 300.0));
    assert_eq!(
        options.validate(),
        Err(WindowOptionsError::MinimumExceedsMaximum)
    );

    options.minimum_logical_size = Some(Size::new(400.0, 300.0));
    options.initial_logical_size = Size::ZERO;
    assert_eq!(
        options.validate(),
        Err(WindowOptionsError::InvalidInitialSize)
    );
}

#[test]
fn initial_size_must_respect_declared_bounds() {
    let options = WindowOptions {
        initial_logical_size: Size::new(300.0, 300.0),
        minimum_logical_size: Some(Size::new(400.0, 200.0)),
        ..WindowOptions::default()
    };
    assert_eq!(
        options.validate(),
        Err(WindowOptionsError::InitialSizeBelowMinimum)
    );

    let options = WindowOptions {
        initial_logical_size: Size::new(300.0, 300.0),
        maximum_logical_size: Some(Size::new(200.0, 400.0)),
        ..WindowOptions::default()
    };
    assert_eq!(
        options.validate(),
        Err(WindowOptionsError::InitialSizeAboveMaximum)
    );
}

#[test]
fn command_preserves_target_and_operation_without_native_data() {
    let window_id = WindowId::from_parts(3, 1);
    let command = WindowCommand::new(
        window_id,
        WindowOperation::SetLogicalSize(Size::new(640.0, 480.0)),
    );

    assert_eq!(command.window_id, window_id);
    assert!(command.request_id.is_none());
    assert_eq!(
        command.operation,
        WindowOperation::SetLogicalSize(Size::new(640.0, 480.0))
    );
}

#[test]
fn capability_snapshots_distinguish_unknown_supported_and_unsupported() {
    let unknown = PlatformCapabilities::default();
    assert_eq!(
        unknown.window.set_title,
        CapabilitySupport::Unknown,
        "construction before a native backend attaches must not pretend unsupported"
    );

    let unsupported = PlatformCapabilities::unsupported();
    assert_eq!(
        unsupported.application_services.global_shortcuts,
        CapabilitySupport::Unsupported
    );
    assert!(!unsupported.transients.native_surface.is_supported());

    let mut discovered = unsupported;
    discovered.window.set_title = CapabilitySupport::Supported;
    discovered.data_transfer.clipboard_text = CapabilitySupport::Supported;
    assert!(discovered.window.set_title.is_supported());
    assert!(discovered.data_transfer.clipboard_text.is_supported());
}

#[test]
fn result_bearing_commands_carry_only_portable_request_identity() {
    let window_id = WindowId::from_parts(4, 2);
    let request_id = NativeRequestId::new(19);
    let command = WindowCommand::with_request(window_id, request_id, WindowOperation::RequestFocus);
    assert_eq!(command.window_id, window_id);
    assert_eq!(command.request_id, Some(request_id));
    assert_eq!(request_id.get(), 19);
}

#[test]
fn platform_operation_errors_have_stable_categories_not_native_types() {
    let error = PlatformOperationError::with_context(
        PlatformOperationErrorKind::RejectedByPlatform,
        "window manager declined request",
    );
    assert_eq!(error.kind(), PlatformOperationErrorKind::RejectedByPlatform);
    assert_eq!(error.context(), Some("window manager declined request"));
    assert!(error.to_string().contains("rejected by the platform"));
}

#[test]
fn native_mouse_buttons_map_to_distinct_portable_bits() {
    use winit::event::MouseButton;

    assert_eq!(
        mouse_button_mask(MouseButton::Left),
        Some(PRIMARY_POINTER_BUTTON)
    );
    assert_eq!(
        mouse_button_mask(MouseButton::Right),
        Some(SECONDARY_POINTER_BUTTON)
    );
    assert_eq!(
        mouse_button_mask(MouseButton::Middle),
        Some(TERTIARY_POINTER_BUTTON)
    );
    assert_eq!(
        mouse_button_mask(MouseButton::Back),
        Some(BACK_POINTER_BUTTON)
    );
    assert_eq!(
        mouse_button_mask(MouseButton::Forward),
        Some(FORWARD_POINTER_BUTTON)
    );
    assert_eq!(
        mouse_button_mask(MouseButton::Other(0)),
        additional_pointer_button_mask(0)
    );
    assert_eq!(mouse_button_mask(MouseButton::Other(26)), Some(1 << 31));
    assert_eq!(mouse_button_mask(MouseButton::Other(27)), None);
}

#[test]
fn logical_cursor_positions_reject_non_finite_native_inputs() {
    assert_eq!(
        LogicalWindowPosition::new(12.5, -3.0)
            .expect("finite position")
            .x(),
        12.5
    );
    assert!(LogicalWindowPosition::new(f64::NAN, 0.0).is_err());
    assert!(LogicalWindowPosition::new(0.0, f64::INFINITY).is_err());
}

#[test]
fn window_icons_validate_rgba_shape_before_the_native_boundary() {
    let icon = WindowIcon::from_rgba(vec![255; 4 * 3 * 2], 3, 2).expect("valid RGBA icon");
    assert_eq!(icon.width(), 3);
    assert_eq!(icon.height(), 2);
    assert_eq!(icon.rgba().len(), 24);

    assert_eq!(
        WindowIcon::from_rgba(Vec::new(), 0, 2),
        Err(WindowIconError::ZeroDimension)
    );
    assert_eq!(
        WindowIcon::from_rgba(vec![0; 7], 2, 1),
        Err(WindowIconError::InvalidByteCount {
            expected: 8,
            actual: 7,
        })
    );
}

#[test]
fn dynamic_logical_size_limits_reject_contradictory_constraints() {
    let limits = LogicalSizeLimits::new(Some(Size::new(200., 100.)), Some(Size::new(800., 600.)))
        .expect("valid limits");
    assert_eq!(limits.minimum(), Some(Size::new(200., 100.)));
    assert_eq!(limits.maximum(), Some(Size::new(800., 600.)));

    assert_eq!(
        LogicalSizeLimits::new(Some(Size::ZERO), None),
        Err(LogicalSizeLimitsError::InvalidMinimum)
    );
    assert_eq!(
        LogicalSizeLimits::new(Some(Size::new(900., 100.)), Some(Size::new(800., 600.)),),
        Err(LogicalSizeLimitsError::MinimumExceedsMaximum)
    );
}

#[test]
fn requested_and_observed_window_state_are_distinct_contracts() {
    let options = WindowOptions {
        visible: false,
        resizable: false,
        decorations: false,
        maximized: true,
        fullscreen: Some(Fullscreen::Borderless),
        window_level: WindowLevel::AlwaysOnTop,
        minimum_logical_size: Some(Size::new(200., 100.)),
        maximum_logical_size: Some(Size::new(900., 700.)),
        initial_logical_size: Size::new(400., 300.),
        ..WindowOptions::default()
    };
    options.validate().expect("valid options");
    let requested = options.requested_state().expect("valid requested state");
    assert!(!requested.visible);
    assert!(!requested.resizable);
    assert!(!requested.decorations);
    assert!(requested.maximized);
    assert_eq!(requested.fullscreen, Some(Fullscreen::Borderless));
    assert_eq!(requested.level, WindowLevel::AlwaysOnTop);

    let observed = WindowObservedState::default();
    assert_eq!(observed.visible, None);
    assert_eq!(observed.minimized, None);
    assert_eq!(observed.maximized, None);
    assert_eq!(observed.fullscreen, None);
}

#[test]
fn state_changed_events_do_not_masquerade_as_legacy_platform_events() {
    let window_id = WindowId::from_parts(2, 9);
    let observed = WindowObservedState {
        visible: Some(true),
        minimized: None,
        maximized: Some(false),
        fullscreen: Some(false),
        resizable: Some(true),
        decorations: Some(false),
        ..WindowObservedState::default()
    };
    let event = WindowEvent::state_changed(window_id, observed);
    assert_eq!(event.window_id, window_id);
    assert_eq!(event.kind, WindowEventKind::StateChanged(observed));
    assert!(event.platform_event().is_none());
}

#[test]
fn window_events_preserve_legacy_platform_events_with_window_identity() {
    let id = WindowId::from_parts(1, 0);
    let event = WindowEvent::platform(id, PlatformEvent::CloseRequested);
    assert_eq!(event.window_id, id);
    assert_eq!(event.platform_event(), Some(&PlatformEvent::CloseRequested));
    assert_eq!(
        WindowEvent::lifecycle(id, WindowLifecycle::Focused).kind,
        WindowEventKind::Lifecycle(WindowLifecycle::Focused)
    );
}

#[test]
fn dpi_round_trip_supports_fractional_scales() {
    let m = WindowMetrics::new(PhysicalSize::new(225, 150), 1.5);
    assert_eq!(m.logical_size(), Size::new(150., 100.));
    assert_eq!(
        m.physical_to_logical(Offset::new(75., 30.)),
        Offset::new(50., 20.)
    );
    assert_eq!(
        m.logical_to_physical(Offset::new(50., 20.)),
        Offset::new(75., 30.)
    );
}

#[test]
fn display_local_coordinates_are_explicit_and_mixed_dpi_safe() {
    let display = DisplaySnapshot {
        id: DisplayId::from_parts(4, 2),
        name: Some("HiDPI".into()),
        scale_factor: 1.5,
        physical_size: PhysicalSize::new(2_560, 1_440),
        physical_bounds: Some(PhysicalScreenRect::new(-2_560, 0, 2_560, 1_440)),
        logical_bounds: None,
        physical_work_area: None,
        logical_work_area: None,
        is_primary: false,
    };

    let local = LogicalDisplayPosition::new(100.0, 40.0);
    let physical = display
        .logical_to_physical_local(local)
        .expect("finite local coordinates");
    assert_eq!(physical, PhysicalDisplayPosition::new(150, 60));
    assert_eq!(display.physical_to_logical_local(physical), Some(local));
    assert_eq!(
        display.screen_position(physical),
        Some(PhysicalScreenPosition::new(-2_410, 60))
    );

    let wayland_style = DisplaySnapshot {
        physical_bounds: None,
        ..display
    };
    assert_eq!(
        wayland_style.logical_to_physical_local(local),
        Some(physical),
        "display-local DPI conversion does not require global coordinates"
    );
    assert_eq!(wayland_style.screen_position(physical), None);
}

#[test]
fn work_area_centering_excludes_reserved_desktop_space() {
    let display = DisplaySnapshot {
        id: DisplayId::from_parts(0, 0),
        name: None,
        scale_factor: 1.0,
        physical_size: PhysicalSize::new(1_920, 1_080),
        physical_bounds: Some(PhysicalScreenRect::new(0, 0, 1_920, 1_080)),
        logical_bounds: None,
        physical_work_area: Some(PhysicalScreenRect::new(0, 0, 1_920, 1_040)),
        logical_work_area: None,
        is_primary: true,
    };
    let outer = PhysicalSize::new(400, 200);

    assert_eq!(
        display
            .placement_rect(DisplayPlacementArea::WorkArea)
            .expect("work area")
            .centered_position(outer),
        PhysicalScreenPosition::new(760, 420)
    );
    assert_eq!(
        display
            .placement_rect(DisplayPlacementArea::FullBounds)
            .expect("bounds")
            .centered_position(outer),
        PhysicalScreenPosition::new(760, 440)
    );
}

#[test]
fn identified_pointer_conversion_preserves_contact_identity() {
    let metrics = WindowMetrics::new(PhysicalSize::new(200, 100), 2.0);
    let PlatformEvent::Input(InputEvent::PointerWithId {
        pointer,
        phase,
        position,
    }) = identified_pointer_event(
        42,
        PointerPhase::Down,
        winit::dpi::PhysicalPosition::new(50., 30.),
        metrics,
    )
    else {
        unreachable!()
    };
    assert_eq!(pointer, 42);
    assert_eq!(phase, PointerPhase::Down);
    assert_eq!(position, Offset::new(25., 15.));
}

#[test]
fn scroll_forms_use_natural_content_direction_and_fractional_pixels() {
    let metrics = WindowMetrics::new(PhysicalSize::new(200, 100), 2.0);
    let PlatformEvent::Input(InputEvent::Scroll { delta: line }) =
        wheel_event(winit::event::MouseScrollDelta::LineDelta(0., 1.), metrics)
    else {
        unreachable!()
    };
    let PlatformEvent::Input(InputEvent::Scroll { delta: pixel }) = wheel_event(
        winit::event::MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(0., 80.)),
        metrics,
    ) else {
        unreachable!()
    };
    assert_eq!(line.y, pixel.y);
    assert_eq!(line.y, -40.);
    let PlatformEvent::Input(InputEvent::Scroll { delta }) = wheel_event(
        winit::event::MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(0., 2.5)),
        metrics,
    ) else {
        unreachable!()
    };
    assert_eq!(delta.y, -1.25);
}

#[test]
fn command_control_text_is_not_a_text_commit() {
    let committed = PlatformEvent::Input(InputEvent::Text("é".into()));
    let command = PlatformEvent::TextInputAction(TextInputAction::Done);
    assert_ne!(committed, command);
    assert_eq!(
        committed,
        PlatformEvent::Input(InputEvent::Text("é".into()))
    );
}

#[test]
fn standardized_key_mapping_preserves_named_and_physical_values() {
    let key = KeyboardEvent::key_down(KeyboardKey::Named(NamedKey::Backspace), Code::Backspace);
    assert_eq!(key.code, Code::Backspace);
    assert_eq!(key.key, KeyboardKey::Named(NamedKey::Backspace));
}

#[test]
fn application_data_directory_requires_a_stable_identifier() {
    assert!(application_data_local_directory("").is_none());
    assert!(application_data_local_directory("  ").is_none());
}

#[test]
fn text_input_adapter_retains_ordered_commands() {
    let client = TextInputClientId::new(7);
    let command = TextInputCommand::Update {
        client,
        state: TextInputState {
            text: "hello".into(),
            selection_start: 5,
            selection_end: 5,
            composing: None,
        },
    };
    let mut adapter = MemoryTextInputAdapter::default();
    adapter.apply(&command);
    assert_eq!(adapter.commands(), &[command]);
    adapter.clear();
    assert!(adapter.commands().is_empty());
}
