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
fn ime_focus_move_cancels_composition_and_commit_targets_focused_field() {
    let first = TextEditingController::with_text("aa");
    let second = TextEditingController::with_text("bb");
    let mut runtime = Runtime::new(Widget::from(incular_widgets::Column::new(vec![
        Widget::from(EditableText::new(first.clone()).size(Size::new(180., 32.))),
        Widget::from(EditableText::new(second.clone()).size(Size::new(180., 32.))),
    ])))
    .unwrap();
    let constraints = Constraints::tight(Size::new(180., 100.));
    runtime.run_frame(constraints).unwrap();
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: incular_core::PointerPhase::Down,
        position: Offset::new(5., 5.),
    });
    let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::Preedit {
        text: "xy".into(),
        selection: None,
    }));
    assert_eq!(first.preedit().as_deref(), Some("xy"));
    // Moving focus retires the composition's client: the orphaned
    // preedit clears on its own editor instead of lingering, and the
    // newly focused field holds none.
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: incular_core::PointerPhase::Down,
        position: Offset::new(5., 40.),
    });
    assert_eq!(first.preedit(), None);
    assert_eq!(second.preedit(), None);
    // The following commit belongs to no open composition, so it
    // applies to the focused field directly; the first field is
    // untouched.
    let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::Commit("zz".into())));
    assert_eq!(first.text(), "aa");
    assert!(second.text().contains("zz"));
}

#[test]
fn ime_commit_without_preedit_applies_to_focused_field() {
    let controller = TextEditingController::with_text("ab");
    let mut runtime = mount_focused(controller.clone(), |field| field.size(Size::new(180., 32.)));
    let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::Commit("xy".into())));
    assert!(controller.text().contains("xy"));
    assert_eq!(controller.preedit(), None);
}

#[test]
fn ime_end_cancels_preedit_without_text_change() {
    let controller = TextEditingController::with_text("ab");
    let mut runtime = mount_focused(controller.clone(), |field| field.size(Size::new(180., 32.)));
    let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::Preedit {
        text: "xy".into(),
        selection: None,
    }));
    assert_eq!(controller.preedit().as_deref(), Some("xy"));
    let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::End));
    assert_eq!(controller.preedit(), None);
    assert_eq!(controller.text(), "ab");
}

#[test]
fn ime_controller_replacement_cancels_composition() {
    let first = TextEditingController::with_text("ab");
    let replacement = TextEditingController::new();
    let mut runtime = mount_focused(first.clone(), |field| field.size(Size::new(180., 32.)));
    let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::Preedit {
        text: "xy".into(),
        selection: None,
    }));
    assert_eq!(first.preedit().as_deref(), Some("xy"));
    let field = runtime.focused_element().expect("field stays focused");
    runtime
        .tree_mut()
        .update(
            field,
            Widget::from(EditableText::new(replacement.clone()).size(Size::new(180., 32.))),
        )
        .unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(180., 100.)))
        .unwrap();
    assert_eq!(first.preedit(), None);
    assert_eq!(first.text(), "ab");
    assert_eq!(replacement.text(), "");
}

#[test]
fn retained_editability_changes_cancel_composition_without_native_events() {
    for read_only in [false, true] {
        let controller = TextEditingController::with_text("ab");
        let mut runtime = mount_focused(controller.clone(), |field| field.size(Size::new(180., 32.)));
        let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::Preedit {
            text: "xy".into(),
            selection: None,
        }));
        let field = runtime.focused_element().unwrap();
        runtime.tree_mut().update(
            field,
            EditableText::new(controller.clone())
                .enabled(read_only)
                .read_only(read_only)
                .size(Size::new(180., 32.))
                .into(),
        ).unwrap();
        runtime.run_frame(Constraints::tight(Size::new(180., 100.))).unwrap();
        assert_eq!(controller.preedit(), None);
        assert_eq!(controller.text(), "ab");
    }
}

#[test]
fn ownerless_end_preserves_unrelated_preedit() {
    let controller = TextEditingController::with_text("ab");
    let mut runtime = mount_focused(controller.clone(), |field| field.size(Size::new(180., 32.)));
    controller.set_preedit("application", None);
    let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::End));
    assert_eq!(controller.preedit().as_deref(), Some("application"));
    assert_eq!(controller.text(), "ab");
}

#[test]
fn shutdown_during_composition_cancels_and_clears_focus() {
    let controller = TextEditingController::with_text("ab");
    let mut runtime = mount_focused(controller.clone(), |field| field.size(Size::new(180., 32.)));
    let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::Preedit {
        text: "xy".into(),
        selection: None,
    }));
    assert_eq!(controller.preedit().as_deref(), Some("xy"));
    // Window close retires focus through the single focus owner, which
    // cancels the open composition on its own editor: no commit lands,
    // no stale preedit survives, and no focus remains.
    runtime.shutdown();
    assert_eq!(controller.preedit(), None);
    assert_eq!(controller.text(), "ab");
    assert_eq!(runtime.focused_element(), None);
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
