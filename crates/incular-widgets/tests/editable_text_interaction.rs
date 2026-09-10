//! EditableText interaction transitions at the retained-tree level.
//!
//! Covers mounted flag transitions, controller replacement (old edits
//! must not leak into the new presentation), submit callback
//! replacement, hint updates while focused, selection-only versus
//! content revisions, and caret point mapping. Genuine input dispatch
//! lives in the runtime gating tests; these pin the tree contract that
//! dispatch relies on.

mod common;

use common::*;
use incular_config::Constraints;
use incular_core::{Offset, Size};
use incular_text::{TextEditingController, TextSelection};
use incular_widgets::internal::*;
use incular_widgets::{TextInputActionHint, TextInputTypeHint};
use std::time::Instant;

fn mount_field(
    controller: TextEditingController,
    build: impl FnOnce(EditableText) -> EditableText,
) -> (WidgetTree, ElementId) {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::from(build(EditableText::new(controller))))
        .expect("mount field");
    (tree, root)
}

fn layout_tight(tree: &mut WidgetTree) {
    tree.layout(Constraints::tight(Size::new(180., 32.)))
        .expect("layout");
}

#[test]
fn enabled_and_read_only_transitions_gate_queries() {
    let controller = TextEditingController::with_text("hi");
    let (mut tree, root) =
        mount_field(controller.clone(), |field| field.size(Size::new(180., 32.)));
    layout_tight(&mut tree);
    assert!(tree.text_field_is_editable(root));
    assert!(!tree.text_field_is_read_only(root));

    tree.update(
        root,
        Widget::from(
            EditableText::new(controller.clone())
                .enabled(false)
                .size(Size::new(180., 32.)),
        ),
    )
    .expect("update");
    layout_tight(&mut tree);
    assert!(!tree.text_field_is_editable(root));
    assert!(!tree.text_field_is_read_only(root));

    tree.update(
        root,
        Widget::from(
            EditableText::new(controller.clone())
                .enabled(true)
                .read_only(true)
                .size(Size::new(180., 32.)),
        ),
    )
    .expect("update");
    layout_tight(&mut tree);
    assert!(!tree.text_field_is_editable(root));
    assert!(tree.text_field_is_read_only(root));

    tree.update(
        root,
        Widget::from(EditableText::new(controller).size(Size::new(180., 32.))),
    )
    .expect("update");
    layout_tight(&mut tree);
    assert!(tree.text_field_is_editable(root));
}

#[test]
fn controller_replacement_detaches_old_presentation() {
    let old = TextEditingController::with_text("alpha");
    let new = TextEditingController::with_text("beta");
    let (mut tree, root) = mount_field(old.clone(), |field| field.size(Size::new(180., 32.)));
    layout_tight(&mut tree);
    let _ = tree.paint();
    assert!(
        tree.text_controller(root)
            .is_some_and(|current| current == old)
    );

    // Swapping controllers swaps the retained presentation wholesale:
    // element identity stays, the controller does not.
    tree.update(
        root,
        Widget::from(EditableText::new(new.clone()).size(Size::new(180., 32.))),
    )
    .expect("update");
    layout_tight(&mut tree);
    assert!(tree.is_text_field(root));
    assert!(
        tree.text_controller(root)
            .is_some_and(|current| current == new)
    );

    // Later edits to the detached controller never reach the tree: the
    // render kind holds no reference to it, so revision polling cannot
    // observe it either.
    old.set_text("alpha-CHANGED");
    layout_tight(&mut tree);
    assert_eq!(
        tree.text_controller(root)
            .map(|controller| controller.text()),
        Some("beta".to_owned())
    );
    assert_eq!(
        glyph_count(&tree.paint()),
        4,
        "paint must still show the four glyphs of beta"
    );

    // The mounted controller stays live: its own edits still present.
    new.set_text("beta!");
    layout_tight(&mut tree);
    assert_eq!(glyph_count(&tree.paint()), 5);
}

#[test]
fn submit_callback_replacement_takes_effect() {
    use std::{cell::Cell, rc::Rc};
    let controller = TextEditingController::with_text("go");
    let first = Rc::new(Cell::new(0));
    let observed_first = first.clone();
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field
            .on_submit(move |_| observed_first.set(observed_first.get() + 1))
            .size(Size::new(180., 32.))
    });
    layout_tight(&mut tree);
    assert!(tree.submit_text_field(root));
    assert_eq!(first.get(), 1);

    let second = Rc::new(Cell::new(0));
    let observed_second = second.clone();
    tree.update(
        root,
        Widget::from(
            EditableText::new(controller)
                .on_submit(move |_| observed_second.set(observed_second.get() + 1))
                .size(Size::new(180., 32.)),
        ),
    )
    .expect("update");
    layout_tight(&mut tree);
    assert!(tree.submit_text_field(root));
    assert_eq!(second.get(), 1);
    assert_eq!(first.get(), 1, "replaced callback must stay silent");
}

#[test]
fn input_hints_update_while_focused() {
    let controller = TextEditingController::with_text("hi");
    let (mut tree, root) =
        mount_field(controller.clone(), |field| field.size(Size::new(180., 32.)));
    layout_tight(&mut tree);
    tree.set_focused(root, true, Instant::now())
        .expect("focus field");
    let snapshot = tree
        .text_field_input_snapshot(root)
        .expect("input snapshot");
    assert_eq!(snapshot.input_type, TextInputTypeHint::Text);
    assert_eq!(snapshot.input_action, TextInputActionHint::Unspecified);

    tree.update(
        root,
        Widget::from(
            EditableText::new(controller)
                .input_type(TextInputTypeHint::Number)
                .input_action(TextInputActionHint::Done)
                .size(Size::new(180., 32.)),
        ),
    )
    .expect("update");
    layout_tight(&mut tree);
    let snapshot = tree
        .text_field_input_snapshot(root)
        .expect("input snapshot");
    assert_eq!(snapshot.input_type, TextInputTypeHint::Number);
    assert_eq!(snapshot.input_action, TextInputActionHint::Done);

    // Focus survives the hint update: focus state lives in the render
    // node, not the replaced widget description.
    tree.update_semantics();
    let focused = tree
        .semantic_node_for_element(root)
        .and_then(|id| tree.semantics().node(id))
        .expect("node")
        .state
        .focused;
    assert!(focused);
}

#[test]
fn selection_change_keeps_content_revision() {
    let controller = TextEditingController::with_text("hello");
    let (mut tree, _) = mount_field(controller.clone(), |field| field.size(Size::new(180., 32.)));
    layout_tight(&mut tree);
    let (content_before, visual_before) = controller.revisions();
    controller.set_selection(TextSelection { base: 1, extent: 4 });
    let (content_after, visual_after) = controller.revisions();
    assert_eq!(content_after, content_before);
    assert!(visual_after > visual_before);
    // Measurement still observes the selection-only change.
    layout_tight(&mut tree);
    let _ = tree.paint();
    controller.set_text("hello!");
    let (content_text, _) = controller.revisions();
    assert!(content_text > content_after);
}

#[test]
fn caret_point_mapping_preserves_anchor_on_extend() {
    let controller = TextEditingController::with_text("abcdef\nxy\n123456");
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field.multiline(true).size(Size::new(120., 100.))
    });
    tree.layout(Constraints::tight(Size::new(120., 100.)))
        .expect("layout");
    // A point past the text end clamps to a char boundary at the end.
    assert!(tree.text_field_set_caret(root, Offset::new(500., 200.), false, Instant::now()));
    let end = controller.value().selection.extent;
    assert!(controller.text().is_char_boundary(end));
    assert!(end <= controller.text().len());
    // Extending from a collapsed caret keeps the anchor side fixed.
    controller.set_selection(TextSelection::collapsed(2));
    assert!(tree.text_field_set_caret(root, Offset::new(500., 200.), true, Instant::now()));
    let selection = controller.value().selection;
    assert_eq!(selection.base, 2);
    assert!(selection.extent >= selection.base);
    assert!(controller.text().is_char_boundary(selection.extent));
}
