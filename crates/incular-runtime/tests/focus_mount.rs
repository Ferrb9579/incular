//! Focus autofocus through real runtime mount: the tree advertises the
//! autofocus element, and mount mirrors focus onto the attached node.

use incular_config::Constraints;
use incular_core::{Color, Size};
use incular_runtime::Runtime;
use incular_widgets::{Focus, FocusNode, FocusScope, Widget};

#[test]
fn inactive_retained_editor_releases_focus_capture_and_native_client() {
    use incular_core::{ImeEvent, InputEvent, Offset, PointerPhase};
    use incular_platform::TextInputCommand;
    use incular_widgets::{EditableText, IndexedStack, internal::TextEditingController};

    let editor = TextEditingController::with_text("first");
    let other = TextEditingController::with_text("second");
    let build = |index| Widget::from(IndexedStack::new([
        Widget::from(EditableText::new(editor.clone()).size(Size::new(180., 32.))),
        Widget::from(EditableText::new(other.clone()).size(Size::new(180., 32.))),
    ]).index(index));
    let mut runtime = Runtime::new(build(0)).unwrap();
    let constraints = Constraints::tight(Size::new(180., 100.));
    runtime.run_frame(constraints).unwrap();
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: Offset::new(5., 5.),
    });
    let focused = runtime.focused_element().unwrap();
    assert_eq!(runtime.focus_diagnostics().text_pointer_capture, Some(focused));
    let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::Preedit {
        text: "pending".into(),
        selection: None,
    }));
    let _ = runtime.take_text_input_commands();
    let root = runtime.tree().root().unwrap();
    runtime.tree_mut().update(root, build(1)).unwrap();
    runtime.run_frame(constraints).unwrap();
    assert!(runtime.tree().element_exists(focused));
    assert_eq!(runtime.focused_element(), None);
    assert_eq!(runtime.focus_diagnostics().text_pointer_capture, None);
    assert_eq!(editor.preedit(), None);
    let commands = runtime.take_text_input_commands();
    assert!(commands.iter().any(|command| matches!(command, TextInputCommand::Clear { .. })));
    let selection = editor.selection();
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Move,
        position: Offset::new(170., 5.),
    });
    let _ = runtime.handle_input(InputEvent::Text("late".into()));
    assert_eq!(editor.selection(), selection);
    assert_eq!(editor.text(), "first");
    assert_eq!(other.text(), "second");
}

#[test]
fn stale_selection_capture_is_pruned_without_clearing_another_focus() {
    use incular_core::{InputEvent, Offset, PointerPhase};
    use incular_semantics::SemanticAction;
    use incular_widgets::{Column, EditableText, internal::TextEditingController};

    let first = TextEditingController::with_text("first");
    let second = TextEditingController::with_text("second");
    let build = |enabled| Widget::from(Column::new([
        Widget::from(EditableText::new(first.clone()).enabled(enabled).size(Size::new(180., 32.))),
        Widget::from(EditableText::new(second.clone()).size(Size::new(180., 32.))),
    ]));
    let mut runtime = Runtime::new(build(true)).unwrap();
    let constraints = Constraints::tight(Size::new(180., 100.));
    runtime.run_frame(constraints).unwrap();
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: Offset::new(5., 5.),
    });
    let root = runtime.tree().root().unwrap();
    let children = runtime.tree().children(root).unwrap().to_vec();
    let node = runtime.tree().semantic_node_for_element(children[1]).unwrap();
    assert!(runtime.dispatch_semantic_action(node, SemanticAction::Focus));
    assert_eq!(runtime.focus_diagnostics().text_pointer_capture, Some(children[0]));
    runtime.tree_mut().update(root, build(false)).unwrap();
    runtime.run_frame(constraints).unwrap();
    assert_eq!(runtime.focused_element(), Some(children[1]));
    assert_eq!(runtime.focus_diagnostics().text_pointer_capture, None);
    let selection = first.selection();
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Move,
        position: Offset::new(170., 5.),
    });
    assert_eq!(first.selection(), selection);
}

#[test]
fn focus_autofocus_mount_requests_node() {
    let node = FocusNode::new();
    let mut runtime = Runtime::new(
        Focus::new(Widget::box_(Size::new(40., 40.), Color::WHITE))
            .node(node.clone())
            .autofocus(true)
            .into(),
    )
    .expect("mount");
    runtime
        .run_frame(Constraints::tight(Size::new(100., 100.)))
        .expect("frame");
    assert!(node.has_focus());
    assert!(runtime.focused_element().is_some());
}

#[test]
fn focus_scope_autofocus_mount_focuses_descendant() {
    let node = FocusNode::new();
    let mut runtime = Runtime::new(
        FocusScope::new(
            Focus::new(Widget::box_(Size::new(40., 40.), Color::WHITE)).node(node.clone()),
        )
        .autofocus(true)
        .into(),
    )
    .expect("mount");
    runtime
        .run_frame(Constraints::tight(Size::new(100., 100.)))
        .expect("frame");
    assert!(
        node.has_focus(),
        "scope autofocus must resolve to the first focusable descendant"
    );
}
