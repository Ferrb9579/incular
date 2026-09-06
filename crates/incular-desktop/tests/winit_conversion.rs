use incular_core::{
    BACK_POINTER_BUTTON, FORWARD_POINTER_BUTTON, InputEvent, Offset, PRIMARY_POINTER_BUTTON,
    PointerPhase, SECONDARY_POINTER_BUTTON, TERTIARY_POINTER_BUTTON,
    additional_pointer_button_mask,
};
use incular_platform::*;

use incular_desktop::winit_adapter::*;

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
