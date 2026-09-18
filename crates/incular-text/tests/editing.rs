use incular_text::{
    TextEditingController, TextEditingDelta, TextEditingValue, TextRange, TextSelection,
};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
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
fn reentrant_mutation_during_notify_terminates_without_panic() {
    // Listeners run after the controller releases its state borrow
    // (cloned out beforehand), so a listener that edits terminates
    // instead of tripping the borrow guard.
    let controller = TextEditingController::with_text("hi");
    let fired = Rc::new(Cell::new(0));
    let observed = fired.clone();
    let inner = controller.clone();
    let _token = controller.add_listener(move |value| {
        observed.set(observed.get() + 1);
        if !value.text.ends_with('!') {
            inner.set_text(format!("{}!", value.text));
        }
    });
    controller.set_text("go");
    assert_eq!(controller.text(), "go!");
    assert_eq!(fired.get(), 2);
}

#[test]
fn committed_preedit_preserves_composition_started_by_listener() {
    for commit in ["", "x"] {
        let controller = TextEditingController::new();
        controller.set_preedit("old", None);
        let editor = controller.clone();
        let fired = Rc::new(Cell::new(false));
        let observed = fired.clone();
        let token = controller.add_listener(move |_| {
            if !observed.replace(true) {
                assert_eq!(editor.preedit(), None);
                editor.set_preedit("new", None);
            }
        });
        controller.commit_preedit(commit);
        assert!(fired.get());
        assert_eq!(controller.text(), commit);
        assert_eq!(controller.preedit().as_deref(), Some("new"));
        assert!(controller.remove_listener(token));
    }
}

#[test]
fn restoration_during_open_preedit_clears_overlay_and_keeps_editing() {
    use incular_core::{RestorationKey, RestorationScope};
    use serde_json::json;

    #[derive(Default)]
    struct MemoryBackend(RefCell<BTreeMap<Vec<RestorationKey>, serde_json::Value>>);
    impl incular_core::RestorationBackend for MemoryBackend {
        fn read_value(&self, path: &[RestorationKey]) -> Option<serde_json::Value> {
            self.0.borrow().get(path).cloned()
        }
        fn write_value(&self, path: &[RestorationKey], value: serde_json::Value) {
            self.0.borrow_mut().insert(path.to_vec(), value);
        }
        fn remove_value(&self, path: &[RestorationKey]) {
            self.0.borrow_mut().remove(path);
        }
    }

    let scope = RestorationScope::root(Rc::new(MemoryBackend::default()));
    let key = RestorationKey::new("document").unwrap();
    let controller = TextEditingController::with_text("ab");
    controller.set_preedit("xy", None);
    assert_eq!(controller.preedit().as_deref(), Some("xy"));
    scope.set_json(
        &key,
        json!({"text": "restored", "selection": {"base": 8, "extent": 8}}),
    );
    controller.bind_restoration(scope, key);
    assert_eq!(controller.preedit(), None);
    assert_eq!(controller.text(), "restored");
    controller.insert("!");
    assert_eq!(controller.text(), "restored!");
}

#[test]
fn deletion_moves_by_codepoint_not_by_byte() {
    let controller = TextEditingController::with_text("a🙂");
    controller.delete_backward();
    assert_eq!(controller.text(), "a");
    controller.delete_backward();
    assert_eq!(controller.text(), "");
}
