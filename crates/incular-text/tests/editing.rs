use incular_text::{
    TextEditingController, TextEditingDelta, TextEditingValue, TextRange, TextSelection,
};
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn selection_and_replacement_preserve_utf8_boundaries() {
    let value = TextEditingValue::new("a🙂b").with_selection(TextSelection::new(1, 5));
    assert_eq!(value.selection, TextSelection::new(1, 5));
    let range = value.selection.range();
    let replaced = value.replace(range, "x");
    assert_eq!(replaced.text, "axb");
    assert_eq!(replaced.selection, TextSelection::collapsed(2));
}

#[test]
fn controller_notifies_listeners_and_applies_deltas() {
    let controller = TextEditingController::with_text("hello");
    let calls = Rc::new(Cell::new(0));
    let calls_for_listener = calls.clone();
    let token = controller.add_listener(move |_| {
        calls_for_listener.set(calls_for_listener.get() + 1);
    });
    controller.set_selection(TextSelection::new(0, 5));
    controller.apply_delta(&TextEditingDelta::replace(TextRange::new(0, 5), "world"));
    assert_eq!(controller.text(), "world");
    assert_eq!(calls.get(), 2);
    assert!(controller.remove_listener(token));
}

#[test]
fn deletion_moves_by_codepoint_not_by_byte() {
    let controller = TextEditingController::with_text("a🙂");
    controller.delete_backward();
    assert_eq!(controller.text(), "a");
    controller.delete_backward();
    assert_eq!(controller.text(), "");
}
