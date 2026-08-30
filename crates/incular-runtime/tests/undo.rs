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
