//! Semantic Increment/Decrement execution through real runtime dispatch.
//!
//! Advertisement (node action lists) and execution (dispatch results)
//! share one policy: explicit semantic callbacks execute independently,
//! while the keyboard fallback serves only listeners that opt into
//! semantics. These tests drive `dispatch_semantic_action` end to end.

use std::{cell::Cell, rc::Rc};

use incular_config::Constraints;
use incular_core::{Code, KeyboardEvent, KeyboardKey, NamedKey, Size};
use incular_runtime::Runtime;
use incular_controls::Button;
use incular_semantics::{SemanticAction, SemanticActionKind, SemanticRole};
use incular_widgets::{
    CallbackShortcuts, EditableText, KeyboardListener, Semantics, Stack, Text, Widget,
    internal::TextEditingController,
};

fn arrow_listener(calls: Rc<Cell<u32>>) -> impl Fn(KeyboardEvent) -> bool {
    move |event| {
        calls.set(calls.get() + 1);
        // Synthetic semantic keys carry an unidentified key name, like the
        // runtime fallback produces; match the physical code instead.
        matches!(event.code, Code::ArrowRight | Code::ArrowLeft)
    }
}

fn listener_node(calls: Rc<Cell<u32>>, include: bool) -> Widget {
    Semantics::new(
        KeyboardListener::new(Text::new("child"))
            .on_key(arrow_listener(calls))
            .include_semantics(include),
    )
    .role(SemanticRole::Group)
    .label("listener")
    .action(SemanticAction::Increment)
    .action(SemanticAction::Decrement)
    .into()
}

fn mount(include: bool, calls: Rc<Cell<u32>>) -> Runtime {
    let mut runtime = Runtime::new(
        Stack::new([
            listener_node(calls, include),
            Widget::from(Text::new("sibling")),
        ])
        .into(),
    )
    .expect("mount");
    runtime
        .run_frame(Constraints::tight(Size::new(200., 100.)))
        .expect("frame");
    runtime
}

/// Element and node ids for the listener wrapper in a freshly mounted tree.
fn listener_ids(
    runtime: &Runtime,
) -> (
    incular_widgets::internal::ElementId,
    incular_semantics::SemanticNodeId,
) {
    let tree = runtime.tree();
    let root = tree.root().expect("root");
    let kids: Vec<_> = tree.children(root).expect("children").to_vec();
    let node = tree
        .semantic_node_for_element(kids[0])
        .expect("listener node");
    (kids[0], node)
}

fn node_actions(
    runtime: &Runtime,
    node: incular_semantics::SemanticNodeId,
) -> Vec<SemanticActionKind> {
    runtime
        .tree()
        .semantics()
        .node(node)
        .expect("node")
        .actions
        .clone()
}

#[test]
fn semantic_increment_uses_keyboard_fallback_when_enabled() {
    let calls = Rc::new(Cell::new(0));
    let mut runtime = mount(true, calls.clone());
    let (_, node) = listener_ids(&runtime);
    let actions = node_actions(&runtime, node);
    assert!(actions.contains(&SemanticActionKind::Increment));
    assert!(actions.contains(&SemanticActionKind::Decrement));

    assert!(runtime.dispatch_semantic_action(node, SemanticAction::Increment));
    assert!(runtime.dispatch_semantic_action(node, SemanticAction::Decrement));
    assert_eq!(calls.get(), 2, "both actions route through key dispatch");
}

#[test]
fn semantic_increment_withdrawn_with_flag_disabled() {
    let calls = Rc::new(Cell::new(0));
    let mut runtime = mount(false, calls.clone());
    let (_, node) = listener_ids(&runtime);
    let actions = node_actions(&runtime, node);
    assert!(!actions.contains(&SemanticActionKind::Increment));
    assert!(!actions.contains(&SemanticActionKind::Decrement));

    // Execution must agree with advertisement: no explicit callback is
    // bound, and the opted-out listener may not serve the fallback.
    assert!(!runtime.dispatch_semantic_action(node, SemanticAction::Increment));
    assert!(!runtime.dispatch_semantic_action(node, SemanticAction::Decrement));
    assert_eq!(calls.get(), 0);
}

#[test]
fn explicit_increment_callback_executes_with_flag_disabled() {
    let keys = Rc::new(Cell::new(0));
    let explicit = Rc::new(Cell::new(0));
    let observed = explicit.clone();
    let mut runtime = Runtime::new(
        Stack::new([
            Semantics::new(
                KeyboardListener::new(Text::new("child"))
                    .on_key(arrow_listener(keys.clone()))
                    .include_semantics(false),
            )
            .role(SemanticRole::Group)
            .label("listener")
            .on_increase(move || observed.set(observed.get() + 1))
            .into(),
            Widget::from(Text::new("sibling")),
        ])
        .into(),
    )
    .expect("mount");
    runtime
        .run_frame(Constraints::tight(Size::new(200., 100.)))
        .expect("frame");
    let (_, node) = listener_ids(&runtime);
    // The bound callback advertises independently of the flag.
    assert!(node_actions(&runtime, node).contains(&SemanticActionKind::Increment));

    assert!(runtime.dispatch_semantic_action(node, SemanticAction::Increment));
    assert_eq!(
        explicit.get(),
        1,
        "explicit callback wins over the fallback"
    );
    assert_eq!(keys.get(), 0, "fallback never runs when explicit exists");
}

#[test]
fn callback_shortcuts_node_neither_advertises_nor_executes() {
    let mut runtime = Runtime::new(
        Stack::new([
            Widget::from(
                Semantics::new(CallbackShortcuts::new(Text::new("child")))
                    .role(SemanticRole::Group)
                    .label("shortcuts")
                    .action(SemanticAction::Increment),
            ),
            Widget::from(Text::new("sibling")),
        ])
        .into(),
    )
    .expect("mount");
    runtime
        .run_frame(Constraints::tight(Size::new(200., 100.)))
        .expect("frame");
    let (_, node) = listener_ids(&runtime);
    assert!(!node_actions(&runtime, node).contains(&SemanticActionKind::Increment));
    assert!(!runtime.dispatch_semantic_action(node, SemanticAction::Increment));
    assert!(!runtime.dispatch_semantic_action(node, SemanticAction::Decrement));
}

#[test]
fn child_actions_and_sibling_survive_flag_toggle() {
    let keys = Rc::new(Cell::new(0));
    let child_calls = Rc::new(Cell::new(0));
    let build = |include: bool, keys: Rc<Cell<u32>>| {
        let observed_child = child_calls.clone();
        Widget::from(
            Semantics::new(
                KeyboardListener::new(
                    Semantics::new(Text::new("child"))
                        .role(SemanticRole::Button)
                        .label("child button")
                        .on_tap(move || observed_child.set(observed_child.get() + 1)),
                )
                .on_key(arrow_listener(keys))
                .include_semantics(include),
            )
            .role(SemanticRole::Group)
            .label("listener")
            .action(SemanticAction::Increment),
        )
    };
    let mut runtime = Runtime::new(
        Stack::new([
            build(true, keys.clone()),
            Widget::from(Text::new("sibling")),
        ])
        .into(),
    )
    .expect("mount");
    runtime
        .run_frame(Constraints::tight(Size::new(200., 100.)))
        .expect("frame");
    let tree = runtime.tree();
    let root = tree.root().expect("root");
    let kids: Vec<_> = tree.children(root).expect("children").to_vec();
    let listener_el = kids[0];
    let child_el = tree.children(listener_el).expect("listener child")[0];
    let sibling_el = kids[1];
    let child_node = tree
        .semantic_node_for_element(child_el)
        .expect("child node");
    let sibling_node = tree
        .semantic_node_for_element(sibling_el)
        .expect("sibling node");

    // The child's own Activate action dispatches while the flag is on.
    assert!(runtime.dispatch_semantic_action(child_node, SemanticAction::Activate));
    assert_eq!(child_calls.get(), 1);

    // Toggle the flag: listener affordance flips, everything else holds.
    runtime
        .tree_mut()
        .update(
            root,
            Stack::new([
                build(false, keys.clone()),
                Widget::from(Text::new("sibling")),
            ])
            .into(),
        )
        .expect("toggle flag");
    runtime
        .run_frame(Constraints::tight(Size::new(200., 100.)))
        .expect("frame");
    let tree = runtime.tree();
    assert_eq!(
        tree.children(root).expect("children").to_vec(),
        vec![listener_el, sibling_el]
    );
    assert_eq!(
        tree.semantic_node_for_element(child_el),
        Some(child_node),
        "child node stable"
    );
    assert_eq!(
        tree.semantic_node_for_element(sibling_el),
        Some(sibling_node),
        "sibling node stable"
    );
    let listener_node = tree
        .semantic_node_for_element(listener_el)
        .expect("listener node");
    assert!(!node_actions(&runtime, listener_node).contains(&SemanticActionKind::Increment));
    assert!(runtime.dispatch_semantic_action(child_node, SemanticAction::Activate));
    assert_eq!(child_calls.get(), 2, "child execution intact");
    // Plain key routing never reads the flag.
    let arrow = KeyboardEvent::key_down(KeyboardKey::Named(NamedKey::ArrowRight), Code::ArrowRight);
    assert!(runtime.tree().dispatch_keyboard(Some(listener_el), arrow));
    assert_eq!(keys.get(), 1);
}

#[test]
fn semantic_activate_respects_button_enabled_gate() {
    let enabled_calls = Rc::new(Cell::new(0));
    let disabled_calls = Rc::new(Cell::new(0));
    let enabled_observed = enabled_calls.clone();
    let disabled_observed = disabled_calls.clone();
    let mut runtime = Runtime::new(
        Stack::new([
            Widget::from(
                Button::builder()
                    .child(Text::new("go"))
                    .enabled(true)
                    .on_click(move || enabled_observed.set(enabled_observed.get() + 1))
                    .build(),
            ),
            Widget::from(
                Button::builder()
                    .child(Text::new("stop"))
                    .enabled(false)
                    .on_click(move || disabled_observed.set(disabled_observed.get() + 1))
                    .build(),
            ),
        ])
        .into(),
    )
    .expect("mount");
    runtime
        .run_frame(Constraints::tight(Size::new(200., 100.)))
        .expect("frame");
    let tree = runtime.tree();
    let root = tree.root().expect("root");
    let kids: Vec<_> = tree.children(root).expect("children").to_vec();
    let enabled_node = tree
        .semantic_node_for_element(kids[0])
        .expect("enabled node");
    let disabled_node = tree
        .semantic_node_for_element(kids[1])
        .expect("disabled node");
    // Advertisement already agrees: only the enabled button offers Activate.
    assert!(node_actions(&runtime, enabled_node).contains(&SemanticActionKind::Activate));
    assert!(!node_actions(&runtime, disabled_node).contains(&SemanticActionKind::Activate));
    // Execution agrees: the enabled button fires exactly once per dispatch,
    // while the disabled button refuses without running its callback.
    assert!(runtime.dispatch_semantic_action(enabled_node, SemanticAction::Activate));
    assert_eq!(enabled_calls.get(), 1);
    assert!(runtime.dispatch_semantic_action(enabled_node, SemanticAction::Activate));
    assert_eq!(enabled_calls.get(), 2);
    assert!(!runtime.dispatch_semantic_action(disabled_node, SemanticAction::Activate));
    assert_eq!(disabled_calls.get(), 0);
}

#[test]
fn semantic_set_selection_disabled_field_applies_without_mutating() {
    let controller = TextEditingController::with_text("hello");
    let mut runtime = Runtime::new(
        Stack::new([
            Widget::from(
                EditableText::new(controller.clone())
                    .enabled(false)
                    .size(Size::new(180., 32.)),
            ),
            Widget::from(Text::new("sibling")),
        ])
        .into(),
    )
    .expect("mount");
    runtime
        .run_frame(Constraints::tight(Size::new(200., 100.)))
        .expect("frame");
    let tree = runtime.tree();
    let root = tree.root().expect("root");
    let field = tree.children(root).expect("children").to_vec()[0];
    let node = tree
        .semantic_node_for_element(field)
        .expect("field node");
    // SetSelection is advertised even for disabled fields, while SetText
    // requires editability — matching pointer and keyboard behavior, where
    // selection works but mutation is gated.
    assert!(node_actions(&runtime, node).contains(&SemanticActionKind::SetSelection));
    assert!(!node_actions(&runtime, node).contains(&SemanticActionKind::SetText));
    assert!(runtime.dispatch_semantic_action(
        node,
        SemanticAction::SetSelection {
            base: 1,
            extent: 3
        }
    ));
    assert_eq!(controller.selection().base, 1);
    assert_eq!(controller.selection().extent, 3);
    assert_eq!(controller.text(), "hello");
}

#[test]
fn semantic_action_after_unmount_resolves_false() {
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let mut runtime = Runtime::new(
        Stack::new([
            Semantics::new(Text::new("child"))
                .role(SemanticRole::Group)
                .label("tapped")
                .on_tap(move || observed.set(observed.get() + 1))
                .into(),
            Widget::from(Text::new("sibling")),
        ])
        .into(),
    )
    .expect("mount");
    runtime
        .run_frame(Constraints::tight(Size::new(200., 100.)))
        .expect("frame");
    let root = runtime.tree().root().expect("root");
    let node = runtime
        .tree()
        .semantic_node_for_element(
            runtime.tree().children(root).expect("children").to_vec()[0],
        )
        .expect("node");
    // Remove the subtree without running a frame: unmount drops the element
    // and its semantic id synchronously, so the retained node resolves to
    // nothing and the detached callback never runs.
    runtime
        .tree_mut()
        .update(
            root,
            Stack::new([Widget::from(Text::new("sibling"))]).into(),
        )
        .expect("unmount");
    assert!(!runtime.dispatch_semantic_action(node, SemanticAction::Activate));
    assert_eq!(calls.get(), 0);
}

#[test]
fn mounted_callback_and_flag_replacement() {
    let first = Rc::new(Cell::new(0));
    let second = Rc::new(Cell::new(0));
    let build = |calls: Rc<Cell<u32>>, include: bool| {
        Widget::from(
            Semantics::new(
                KeyboardListener::new(Text::new("child"))
                    .on_key(arrow_listener(calls))
                    .include_semantics(include),
            )
            .role(SemanticRole::Group)
            .label("listener")
            .action(SemanticAction::Increment),
        )
    };
    let mut runtime = Runtime::new(
        Stack::new([
            build(first.clone(), true),
            Widget::from(Text::new("sibling")),
        ])
        .into(),
    )
    .expect("mount");
    runtime
        .run_frame(Constraints::tight(Size::new(200., 100.)))
        .expect("frame");
    let root = runtime.tree().root().expect("root");
    let listener_el = runtime.tree().children(root).expect("children").to_vec()[0];
    let node = runtime
        .tree()
        .semantic_node_for_element(listener_el)
        .expect("listener node");
    assert!(runtime.dispatch_semantic_action(node, SemanticAction::Increment));
    assert_eq!(first.get(), 1);

    runtime
        .tree_mut()
        .update(
            root,
            Stack::new([
                build(second.clone(), false),
                Widget::from(Text::new("sibling")),
            ])
            .into(),
        )
        .expect("replace callback and flag");
    runtime
        .run_frame(Constraints::tight(Size::new(200., 100.)))
        .expect("frame");
    let arrow = KeyboardEvent::key_down(KeyboardKey::Named(NamedKey::ArrowRight), Code::ArrowRight);
    assert!(runtime.tree().dispatch_keyboard(Some(listener_el), arrow));
    assert_eq!(first.get(), 1, "old callback detached");
    assert_eq!(second.get(), 1, "new callback serves keys");
    // The replacement also withdrew the semantic fallback.
    assert!(!runtime.dispatch_semantic_action(node, SemanticAction::Increment));
    assert_eq!(second.get(), 1, "no fallback delivery after opt-out");
}
