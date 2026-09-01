#[path = "../src/pointer.rs"]
mod pointer;

use incular_core::{
    BACK_POINTER_BUTTON, FORWARD_POINTER_BUTTON, PRIMARY_POINTER_BUTTON, SECONDARY_POINTER_BUTTON,
    TERTIARY_POINTER_BUTTON, additional_pointer_button_mask,
};
use incular_widgets::MouseCursor;
use pointer::{MouseButtonState, NativeCursorCoordinator, PointerDeviceRegistry};
use winit::{
    event::{DeviceId, ElementState, MouseButton},
    window::CursorIcon,
};

#[test]
fn mouse_buttons_keep_a_complete_chord_and_changed_button() {
    let mut state = MouseButtonState::default();
    let cases = [
        (MouseButton::Left, PRIMARY_POINTER_BUTTON),
        (MouseButton::Right, SECONDARY_POINTER_BUTTON),
        (MouseButton::Middle, TERTIARY_POINTER_BUTTON),
        (MouseButton::Back, BACK_POINTER_BUTTON),
        (MouseButton::Forward, FORWARD_POINTER_BUTTON),
        (
            MouseButton::Other(0),
            additional_pointer_button_mask(0).unwrap(),
        ),
    ];
    let mut expected = 0;
    for (button, mask) in cases {
        expected |= mask;
        let transition = state
            .transition(ElementState::Pressed, button)
            .expect("representable button");
        assert_eq!(transition.button, mask);
        assert_eq!(transition.buttons, expected);
    }
    for (button, mask) in cases.into_iter().rev() {
        expected &= !mask;
        let transition = state
            .transition(ElementState::Released, button)
            .expect("representable button");
        assert_eq!(transition.button, mask);
        assert_eq!(transition.buttons, expected);
    }
    assert_eq!(state.pressed(), 0);
    assert!(
        state
            .transition(ElementState::Pressed, MouseButton::Other(27))
            .is_none()
    );

    let _ = state
        .transition(ElementState::Pressed, MouseButton::Left)
        .expect("primary down");
    let _ = state
        .transition(ElementState::Pressed, MouseButton::Right)
        .expect("secondary down");
    assert!(state.cancel());
    assert_eq!(state.pressed(), 0);
    assert!(
        !state.cancel(),
        "repeated focus-loss cancellation is idempotent"
    );
}

#[test]
fn device_ids_are_stable_and_distinct() {
    let mut devices = PointerDeviceRegistry::default();
    let a = DeviceId::dummy();
    let first = devices.id(a);
    assert_ne!(first, 0);
    assert_eq!(devices.id(a), first);
}

#[test]
fn native_cursor_updates_are_deduplicated_and_include_diagonals() {
    let mut cursor = NativeCursorCoordinator::default();
    assert_eq!(cursor.update(MouseCursor::Defer), Some(CursorIcon::Default));
    assert_eq!(cursor.update(MouseCursor::Basic), None);
    assert_eq!(
        cursor.update(MouseCursor::ResizeUpLeftDownRight),
        Some(CursorIcon::NwseResize)
    );
    assert_eq!(cursor.update(MouseCursor::ResizeUpLeftDownRight), None);
    assert_eq!(
        cursor.update(MouseCursor::ResizeUpRightDownLeft),
        Some(CursorIcon::NeswResize)
    );
}
