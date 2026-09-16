use incular_runtime::{UndoHistoryController, UndoHistoryState};
use incular_text::{TextEditingController, TextSelection};

#[test]
fn undo_controller_undo_redo() {
    let editor = TextEditingController::with_text("one");
    let controller = UndoHistoryController::new(editor.clone());
    editor.set_text("two");
    editor.set_text("three");
    assert!(controller.can_undo());
    assert!(!controller.can_redo());
    assert!(controller.undo());
    assert_eq!(editor.text(), "two");
    assert!(controller.undo());
    assert_eq!(editor.text(), "one");
    assert!(!controller.undo());
    assert!(controller.redo());
    assert_eq!(editor.text(), "two");
    assert!(controller.redo());
    assert_eq!(editor.text(), "three");
    assert!(!controller.can_redo());
}

#[test]
fn undo_controller_uses_signal_state() {
    let editor = TextEditingController::new();
    let controller = UndoHistoryController::new(editor.clone());
    let state = controller.state_signal();
    assert_eq!(state.get(), UndoHistoryState::default());
    editor.set_text("changed");
    assert_eq!(
        state.get(),
        UndoHistoryState {
            can_undo: true,
            can_redo: false
        }
    );
    assert!(controller.undo());
    assert_eq!(
        state.get(),
        UndoHistoryState {
            can_undo: false,
            can_redo: true
        }
    );
    controller.clear();
    assert_eq!(state.get(), UndoHistoryState::default());
}

#[test]
fn preedit_is_transient_and_cancel_leaves_no_trace() {
    let editor = TextEditingController::with_text("ab");
    let controller = UndoHistoryController::new(editor.clone());
    editor.set_preedit("xy", None);
    assert_eq!(editor.preedit().as_deref(), Some("xy"));
    assert!(!controller.can_undo());
    assert!(!controller.can_redo());
    editor.clear_preedit();
    assert_eq!(editor.text(), "ab");
    assert!(!controller.can_undo());
    assert!(!controller.can_redo());
}

#[test]
fn committed_composition_is_a_single_undo_step() {
    let editor = TextEditingController::with_text("ab");
    let controller = UndoHistoryController::new(editor.clone());
    editor.set_selection(TextSelection::collapsed(2));
    editor.set_preedit("xy", None);
    editor.commit_preedit("xy");
    assert!(editor.text().contains("xy"));
    assert_eq!(editor.preedit(), None);
    assert!(controller.can_undo());
    assert!(controller.undo());
    assert_eq!(editor.text(), "ab");
    assert!(!controller.can_undo());
}

#[test]
fn new_edit_after_undo_clears_redo() {
    let editor = TextEditingController::with_text("a");
    let controller = UndoHistoryController::new(editor.clone());
    editor.set_text("b");
    assert!(controller.undo());
    assert_eq!(editor.text(), "a");
    assert!(controller.can_redo());
    editor.set_text("c");
    assert!(!controller.can_redo());
    assert!(controller.undo());
    assert_eq!(editor.text(), "a");
}

#[test]
fn reentrant_edit_during_notification_stays_consistent() {
    use std::{cell::Cell, rc::Rc};
    let editor = TextEditingController::with_text("a");
    let controller = UndoHistoryController::new(editor.clone());
    let fired = Rc::new(Cell::new(false));
    let observed = fired.clone();
    let editor_handle = editor.clone();
    let _subscription = editor.observe(move |_| {
        if !observed.get() {
            observed.set(true);
            editor_handle.set_text("reentrant");
        }
    });
    editor.set_text("first");
    // The reentrant edit wins; history tracks the editor value for value.
    assert_eq!(editor.text(), "reentrant");
    assert!(controller.can_undo());
    assert!(controller.undo());
    assert_eq!(editor.text(), "first");
    assert!(controller.undo());
    assert_eq!(editor.text(), "a");
    assert!(!controller.can_undo());
}

#[test]
fn selection_changes_do_not_create_undo_steps_and_capacity_is_bounded() {
    let editor = TextEditingController::with_text("one");
    let controller = UndoHistoryController::new(editor.clone());
    controller.set_max_entries(1);
    editor.set_selection(TextSelection::collapsed(0));
    editor.set_text("two");
    editor.set_text("three");
    assert_eq!(controller.max_entries(), 1);
    assert!(controller.undo());
    assert_eq!(editor.text(), "two");
    assert!(!controller.undo());
}
