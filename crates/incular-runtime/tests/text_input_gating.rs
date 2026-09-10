//! Text input gating through real dispatch: keyboard, text, IME, and
//! input actions against enabled, read-only, and multiline transitions.
//!
//! Forbidden edits must leave the buffer untouched while supported
//! selection and focus behavior follows the documented contract.

use incular_config::Constraints;
use incular_core::{
    Code, ImeEvent, InputEvent, KeyboardEvent, KeyboardKey, Modifiers, NamedKey, Offset, Size,
};
use incular_platform::TextInputAction;
use incular_runtime::Runtime;
use incular_widgets::{EditableText, Widget, internal::TextEditingController};

fn key_down(code: Code) -> KeyboardEvent {
    KeyboardEvent::key_down(KeyboardKey::Named(NamedKey::Unidentified), code)
}

fn mount_focused(
    controller: TextEditingController,
    build: impl FnOnce(EditableText) -> EditableText,
) -> Runtime {
    let mut runtime = Runtime::new(Widget::from(build(EditableText::new(controller)))).unwrap();
    let constraints = Constraints::tight(Size::new(180., 100.));
    runtime.run_frame(constraints).unwrap();
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: incular_core::PointerPhase::Down,
        position: Offset::new(5., 5.),
    });
    assert!(
        runtime.focused_element().is_some(),
        "pointer down must focus the field"
    );
    runtime
}

#[test]
fn disabled_field_rejects_edits_and_submit() {
    use std::{cell::Cell, rc::Rc};
    let controller = TextEditingController::with_text("abc");
    let submitted = Rc::new(Cell::new(0));
    let observed = submitted.clone();
    let mut runtime = mount_focused(controller.clone(), |field| {
        field
            .enabled(false)
            .on_submit(move |_| observed.set(observed.get() + 1))
            .size(Size::new(180., 32.))
    });
    let before = runtime.editing_diagnostics();
    let _ = runtime.handle_input(InputEvent::Key(key_down(Code::Backspace)));
    let _ = runtime.handle_input(InputEvent::Text("x".into()));
    let _ = runtime.handle_input(InputEvent::Key(key_down(Code::Enter)));
    assert_eq!(controller.text(), "abc");
    assert_eq!(submitted.get(), 0);
    assert_eq!(
        runtime.editing_diagnostics().backspace_commands,
        before.backspace_commands
    );
    assert_eq!(
        runtime.editing_diagnostics().text_commits,
        before.text_commits
    );
}

#[test]
fn read_only_field_selects_without_mutating() {
    let controller = TextEditingController::with_text("hello");
    let mut runtime = mount_focused(controller.clone(), |field| {
        field.read_only(true).size(Size::new(180., 32.))
    });
    // Pointer focus resets the caret; select afterwards so the test
    // observes selection, not the click point.
    controller.set_selection(incular_widgets::internal::TextSelection::collapsed(5));
    // Arrow keys still move the caret in a read-only field.
    let _ = runtime.handle_input(InputEvent::Key(key_down(Code::ArrowLeft)));
    assert_eq!(controller.selection().extent, 4);
    // Mutation paths stay closed.
    let _ = runtime.handle_input(InputEvent::Text("x".into()));
    let _ = runtime.handle_input(InputEvent::Key(key_down(Code::Backspace)));
    assert_eq!(controller.text(), "hello");
    assert_eq!(controller.selection().extent, 4);
}

#[test]
fn single_line_enter_submits_while_multiline_inserts_newline() {
    use std::{cell::Cell, rc::Rc};
    let submitted = Rc::new(Cell::new(0));
    let observed = submitted.clone();
    let controller = TextEditingController::with_text("hi");
    let mut runtime = mount_focused(controller.clone(), |field| {
        field
            .on_submit(move |_| observed.set(observed.get() + 1))
            .size(Size::new(180., 32.))
    });
    let _ = runtime.handle_input(InputEvent::Key(key_down(Code::Enter)));
    assert_eq!(submitted.get(), 1);
    assert_eq!(controller.text(), "hi");

    let multi = TextEditingController::with_text("hi");
    let mut runtime = mount_focused(multi.clone(), |field| {
        field.multiline(true).size(Size::new(180., 64.))
    });
    multi.set_selection(incular_widgets::internal::TextSelection::collapsed(2));
    let _ = runtime.handle_input(InputEvent::Key(key_down(Code::Enter)));
    assert_eq!(multi.text(), "hi\n");
}

#[test]
fn input_action_done_submits_and_newline_inserts() {
    use std::{cell::Cell, rc::Rc};
    let submitted = Rc::new(Cell::new(0));
    let observed = submitted.clone();
    let controller = TextEditingController::with_text("hi");
    let mut runtime = mount_focused(controller.clone(), |field| {
        field
            .on_submit(move |_| observed.set(observed.get() + 1))
            .size(Size::new(180., 32.))
    });
    assert!(runtime.handle_text_input_action(TextInputAction::Done));
    assert_eq!(submitted.get(), 1);
    assert!(!runtime.handle_text_input_action(TextInputAction::Unspecified));

    let multi = TextEditingController::with_text("hi");
    let mut runtime = mount_focused(multi.clone(), |field| {
        field.multiline(true).size(Size::new(180., 64.))
    });
    multi.set_selection(incular_widgets::internal::TextSelection::collapsed(2));
    assert!(runtime.handle_text_input_action(TextInputAction::Newline));
    assert_eq!(multi.text(), "hi\n");
}

#[test]
fn ime_preedit_and_commit_follow_editability() {
    let controller = TextEditingController::with_text("ab");
    let mut runtime = mount_focused(controller.clone(), |field| field.size(Size::new(180., 32.)));
    let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::Preedit {
        text: "xy".into(),
        selection: None,
    }));
    assert_eq!(controller.preedit().as_deref(), Some("xy"));
    let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::Commit("xy".into())));
    assert!(controller.text().contains("xy"));
    assert_eq!(controller.preedit(), None);

    let frozen = TextEditingController::with_text("ab");
    let mut runtime = mount_focused(frozen.clone(), |field| {
        field.enabled(false).size(Size::new(180., 32.))
    });
    let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::Preedit {
        text: "xy".into(),
        selection: None,
    }));
    assert_eq!(frozen.preedit(), None);
    let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::Commit("xy".into())));
    assert_eq!(frozen.text(), "ab");
}

#[test]
fn cut_and_paste_follow_editability() {
    let controller = TextEditingController::with_text("hello");
    let mut runtime = mount_focused(controller.clone(), |field| {
        field.enabled(false).size(Size::new(180., 32.))
    });
    controller.set_selection(incular_widgets::internal::TextSelection { base: 0, extent: 5 });
    let mut cut = key_down(Code::KeyX);
    cut.modifiers = Modifiers::CONTROL;
    let _ = runtime.handle_input(InputEvent::Key(cut));
    assert_eq!(controller.text(), "hello");
    let mut paste = key_down(Code::KeyV);
    paste.modifiers = Modifiers::CONTROL;
    let _ = runtime.handle_input(InputEvent::Key(paste));
    assert_eq!(controller.text(), "hello");
}
