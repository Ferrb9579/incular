use incular_core::{
    Code, Color, InputEvent, KeyboardEvent, KeyboardKey, NamedKey, Offset, PointerPhase, Size,
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
    assert_eq!(
        command.operation,
        WindowOperation::SetLogicalSize(Size::new(640.0, 480.0))
    );
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
