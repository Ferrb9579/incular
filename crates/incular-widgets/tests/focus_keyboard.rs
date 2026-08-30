//! Keyboard-listener behavior tests.

use incular_core::{Code, KeyboardEvent, KeyboardKey, NamedKey};
use incular_widgets::{FocusNode, KeyboardListener, SizedBox};
use std::cell::Cell;
use std::rc::Rc;

fn key_down(code: Code) -> KeyboardEvent {
    KeyboardEvent::key_down(KeyboardKey::Named(NamedKey::Unidentified), code)
}

#[test]
fn keyboard_listener_routes_down_repeat_and_up_events() {
    let down = Rc::new(Cell::new(0));
    let repeat = Rc::new(Cell::new(0));
    let up = Rc::new(Cell::new(0));
    let observed_down = down.clone();
    let observed_repeat = repeat.clone();
    let observed_up = up.clone();
    let listener = KeyboardListener::new(SizedBox::shrink())
        .on_key_down(move |_| observed_down.set(observed_down.get() + 1))
        .on_key_repeat(move |_| observed_repeat.set(observed_repeat.get() + 1))
        .on_key_up(move |_| observed_up.set(observed_up.get() + 1));

    assert!(listener.handle(key_down(Code::KeyA)));
    let mut repeated = key_down(Code::KeyA);
    repeated.repeat = true;
    assert!(listener.handle(repeated));
    assert!(listener.handle(KeyboardEvent::key_up(
        KeyboardKey::Named(NamedKey::Unidentified),
        Code::KeyA,
    )));
    assert_eq!(down.get(), 2);
    assert_eq!(repeat.get(), 1);
    assert_eq!(up.get(), 1);
}

#[test]
fn keyboard_listener_generic_handler_can_consume_event() {
    let phases = Rc::new(Cell::new(0));
    let observed = phases.clone();
    let listener = KeyboardListener::new(SizedBox::shrink()).on_key(move |_| {
        observed.set(observed.get() + 1);
        true
    });
    assert!(listener.handle(key_down(Code::Enter)));
    assert_eq!(phases.get(), 1);
}

#[test]
fn keyboard_listener_keeps_focus_and_semantics_configuration() {
    let node = FocusNode::new();
    let listener = KeyboardListener::new(SizedBox::shrink())
        .focus_node(node.clone())
        .autofocus(true)
        .include_semantics(false);
    assert_eq!(listener.configured_focus_node(), Some(node));
    assert!(listener.is_autofocus());
    assert!(!listener.includes_semantics());
}
