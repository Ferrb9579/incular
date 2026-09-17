use incular_config::Constraints;
use incular_core::Size;
use incular_material::TextField;
use incular_runtime::{Runtime, UndoHistoryController};
use incular_text::{TextEditingController, TextEditingValue, TextRange};
use incular_widgets::{LengthLimitingTextInputFormatter, MaxLengthEnforcement, Stack, Text, TextInputFormatter, Widget};
use std::{cell::{Cell, RefCell}, panic::{AssertUnwindSafe, catch_unwind}, rc::Rc};

#[test]
fn rebuild_replaces_callbacks_and_unmount_releases_formatter() {
    let controller = TextEditingController::new();
    let calls = Rc::new(Cell::new(0));
    let build = |limit, calls: Rc<Cell<usize>>| -> Widget {
        TextField::new(controller.clone()).max_length(Some(limit))
            .on_changed(move |_| calls.set(calls.get() + 1)).into()
    };
    let retained_descriptor = build(1, calls.clone());
    let mut runtime = Runtime::new(Stack::new([retained_descriptor.clone()]).into()).unwrap();
    let constraints = Constraints::tight(Size::new(240., 80.));
    runtime.run_frame(constraints).unwrap();
    let root = runtime.tree().root().unwrap();
    for _ in 0..4 {
        runtime.tree_mut().update(root, Stack::new([build(1, calls.clone())]).into()).unwrap();
        runtime.run_frame(constraints).unwrap();
    }
    controller.set_text("abc");
    assert_eq!(controller.text(), "a");
    assert_eq!(calls.get(), 1);
    let replacement_calls = Rc::new(Cell::new(0));
    runtime.tree_mut().update(root, Stack::new([build(2, replacement_calls.clone())]).into()).unwrap();
    runtime.run_frame(constraints).unwrap();
    controller.set_text("xyz");
    assert_eq!(controller.text(), "xy");
    assert_eq!(calls.get(), 1);
    assert_eq!(replacement_calls.get(), 1);
    runtime.tree_mut().update(root, Stack::new([Widget::from(Text::new("gone"))]).into()).unwrap();
    controller.set_text("unmounted");
    assert_eq!(controller.text(), "unmounted");
    assert_eq!(replacement_calls.get(), 1);
    drop(retained_descriptor);
}

struct RecordingFormatter {
    old: Rc<RefCell<Vec<String>>>,
    panic_once: Rc<Cell<bool>>,
}

impl TextInputFormatter for RecordingFormatter {
    fn format_edit_update(&self, old: &TextEditingValue, next: &TextEditingValue) -> TextEditingValue {
        assert!(!self.panic_once.replace(false), "formatter panic");
        self.old.borrow_mut().push(old.text.clone());
        LengthLimitingTextInputFormatter::new(2).format_edit_update(old, next)
    }
}

#[test]
fn formatter_history_uses_accepted_values_and_recovers_from_panic() {
    let controller = TextEditingController::new();
    let history = UndoHistoryController::new(controller.clone());
    let old = Rc::new(RefCell::new(Vec::new()));
    let panic_once = Rc::new(Cell::new(true));
    let mut runtime = Runtime::new(TextField::new(controller.clone()).input_formatter(
        RecordingFormatter { old: old.clone(), panic_once: panic_once.clone() }
    ).into()).unwrap();
    runtime.run_frame(Constraints::tight(Size::new(240., 80.))).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| controller.set_text("bad"))).is_err());
    assert_eq!(controller.text(), "");
    controller.set_text("abcd");
    controller.set_text("xyz");
    assert_eq!(&*old.borrow(), &[String::new(), String::from("ab")]);
    assert_eq!(controller.text(), "xy");
    assert!(history.undo());
    assert_eq!(controller.text(), "ab");
    assert!(history.undo());
    assert_eq!(controller.text(), "");
    assert!(!history.undo());
    runtime.shutdown();
    controller.set_text("disposed");
    assert_eq!(controller.text(), "disposed");
}

#[test]
fn changed_callback_nested_edit_is_formatted_once_per_effective_value() {
    let controller = TextEditingController::new();
    let nested = controller.clone();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let observed = calls.clone();
    let mut runtime = Runtime::new(TextField::new(controller.clone()).max_length(Some(2))
        .on_changed(move |value| {
            observed.borrow_mut().push(value.to_owned());
            if value == "ab" {
                nested.set_text("xyz");
            }
        }).into()).unwrap();
    runtime.run_frame(Constraints::tight(Size::new(240., 80.))).unwrap();
    controller.set_text("abcd");
    assert_eq!(controller.text(), "xy");
    assert_eq!(&*calls.borrow(), &["ab", "xy"]);
    drop(runtime);
    controller.set_text("after drop");
    assert_eq!(&*calls.borrow(), &["ab", "xy"]);
}

#[test]
fn material_max_length_uses_grapheme_safe_formatter() {
    let controller = TextEditingController::new();
    let widget: Widget = TextField::new(controller.clone())
        .max_length(Some(1))
        .into();
    let mut runtime = Runtime::new(widget).unwrap();
    runtime.run_frame(Constraints::tight(Size::new(240., 80.))).unwrap();
    controller.set_text("👩‍👩‍👧‍👦x");
    assert_eq!(controller.text(), "👩‍👩‍👧‍👦");
    assert_eq!(controller.value().selection.extent, controller.text().len());
}

#[test]
fn material_formatter_defers_until_controller_composition_commits() {
    let controller = TextEditingController::new();
    let widget: Widget = TextField::new(controller.clone())
        .input_formatter(LengthLimitingTextInputFormatter::with_enforcement(
            1,
            MaxLengthEnforcement::TruncateAfterCompositionEnds,
        ))
        .into();
    let mut runtime = Runtime::new(widget).unwrap();
    runtime.run_frame(Constraints::tight(Size::new(240., 80.))).unwrap();
    controller.set_value(TextEditingValue::new("éx").with_composing(Some(TextRange::new(0, 3))));
    assert_eq!(controller.text(), "éx");
    controller.set_value(controller.value().with_composing(None));
    assert_eq!(controller.text(), "é");
    assert!(controller.value().composing.is_none());
    assert_eq!(controller.value().selection.extent, 2);
}
