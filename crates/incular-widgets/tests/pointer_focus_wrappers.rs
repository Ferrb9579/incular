//! Pointer-blocking and focus-wrapper behavior: IgnorePointer,
//! AbsorbPointer, Focus, FocusScope, and mounted KeyboardListener paths.
//!
//! Hit routing is the whole contract of the pointer wrappers, so these
//! tests assert hit-test resolution over overlapping siblings (unit
//! dispatch follows hit results). Focus tests pin traversal membership,
//! node attach/detach, and callback replacement through real rebuilds.

use std::{cell::Cell, rc::Rc};

use incular_config::Constraints;
use incular_core::{Code, Color, KeyboardEvent, KeyboardKey, Modifiers, NamedKey, Offset, Size};
use incular_semantics::{SemanticAction, SemanticActionKind, SemanticRole};
use incular_widgets::{
    AbsorbPointer, ActionResult, Actions, Focus, FocusNode, FocusScope, FocusTraversalOrder,
    IgnorePointer, KeyboardListener, LogicalShortcutKey, Positioned, Semantics, Shortcuts,
    SizedBox, Stack, Text, Widget,
    internal::{ActionId, ElementId, WidgetTree, action},
};

fn key_down(code: Code) -> KeyboardEvent {
    KeyboardEvent::key_down(KeyboardKey::Named(NamedKey::Unidentified), code)
}

fn layout_loose(tree: &mut WidgetTree) {
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
}

fn hit_action(tree: &WidgetTree, point: Offset) -> Option<ActionId> {
    let hit = tree.hit_test(point)?;
    let element = tree.element_for_render(hit)?;
    tree.action_for_element(element)
}

#[test]
fn ignore_pointer_passes_through_to_sibling_behind() {
    let mut tree = WidgetTree::new();
    tree.mount(
        Stack::new([
            action(Size::new(80., 40.), Color::WHITE, ActionId(7)),
            IgnorePointer::new(Widget::box_(Size::new(80., 40.), Color::WHITE)).into(),
        ])
        .into(),
    )
    .expect("mount");
    layout_loose(&mut tree);
    assert_eq!(
        hit_action(&tree, Offset::new(20., 20.)),
        Some(ActionId(7)),
        "an ignoring wrapper must not intercept the sibling behind it"
    );
}

#[test]
fn absorb_pointer_blocks_sibling_behind() {
    let mut tree = WidgetTree::new();
    tree.mount(
        Stack::new([
            action(Size::new(80., 40.), Color::WHITE, ActionId(7)),
            AbsorbPointer::new(Widget::box_(Size::new(80., 40.), Color::WHITE)).into(),
        ])
        .into(),
    )
    .expect("mount");
    layout_loose(&mut tree);
    let hit = tree
        .hit_test(Offset::new(20., 20.))
        .expect("absorbing wrapper is itself a hit");
    assert_eq!(
        tree.action_for_element(tree.element_for_render(hit).expect("element")),
        None,
        "an absorbing wrapper stops the hit instead of passing it behind"
    );
}

#[test]
fn disabled_pointer_flags_restore_child_hits() {
    for (name, wrapper) in [
        (
            "ignore",
            Widget::from(
                IgnorePointer::new(action(Size::new(80., 40.), Color::WHITE, ActionId(7)))
                    .ignoring(false),
            ),
        ),
        (
            "absorb",
            Widget::from(
                AbsorbPointer::new(action(Size::new(80., 40.), Color::WHITE, ActionId(7)))
                    .absorbing(false),
            ),
        ),
    ] {
        let mut tree = WidgetTree::new();
        tree.mount(Stack::new([wrapper]).into()).expect("mount");
        layout_loose(&mut tree);
        assert_eq!(
            hit_action(&tree, Offset::new(20., 20.)),
            Some(ActionId(7)),
            "{name} with its flag off must hit the wrapped child"
        );
    }
}

#[test]
fn mounted_pointer_flag_change_applies_without_layout() {
    // Hit paths read the live element configuration, so flipping either
    // flag needs a rebuild but no re-layout and no render invalidation.
    let sifatida = |ignoring: bool| {
        Widget::from(
            IgnorePointer::new(Widget::box_(Size::new(80., 40.), Color::WHITE)).ignoring(ignoring),
        )
    };
    let absorb = |absorbing: bool| {
        Widget::from(
            AbsorbPointer::new(Widget::box_(Size::new(80., 40.), Color::WHITE))
                .absorbing(absorbing),
        )
    };
    for (name, enabled, disabled) in [
        ("ignore", sifatida(true), sifatida(false)),
        ("absorb", absorb(true), absorb(false)),
    ] {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(
                Stack::new([
                    action(Size::new(80., 40.), Color::WHITE, ActionId(7)),
                    enabled,
                ])
                .into(),
            )
            .expect("mount");
        layout_loose(&mut tree);
        let stack_kids: Vec<_> = tree.children(root).expect("stack children").to_vec();
        let front_box = tree.children(stack_kids[1]).expect("wrapper child")[0];
        let point = Offset::new(20., 20.);
        if name == "ignore" {
            assert_eq!(hit_action(&tree, point), Some(ActionId(7)));
        } else {
            let before = tree.hit_test(point).expect("absorb hit");
            assert_eq!(tree.element_for_render(before), Some(stack_kids[1]));
        }

        tree.update(stack_kids[1], disabled).expect("disable flag");
        let after = tree.hit_test(point).expect("{name} hit after flag change");
        assert_eq!(
            tree.element_for_render(after),
            Some(front_box),
            "{name} with its flag off hits the wrapped child with no re-layout"
        );
    }
}

#[test]
fn pointer_wrappers_keep_subtree_semantics() {
    for (name, wrapper) in [
        (
            "ignore",
            Widget::from(IgnorePointer::new(Widget::from(Text::new("front label")))),
        ),
        (
            "absorb",
            Widget::from(AbsorbPointer::new(Widget::from(Text::new("front label")))),
        ),
    ] {
        let mut tree = WidgetTree::new();
        tree.mount(Stack::new([wrapper]).into()).expect("mount");
        layout_loose(&mut tree);
        tree.update_semantics();
        assert!(
            tree.semantics_debug_dump().contains("front label"),
            "{name} must not remove its subtree from semantics"
        );
    }
}

#[test]
fn focus_without_node_or_autofocus_adds_no_focus_target() {
    let mut tree = WidgetTree::new();
    tree.mount(Widget::box_(Size::new(40., 40.), Color::WHITE))
        .expect("mount plain box");
    let plain_targets = tree.focusable_elements().len();

    let mut tree = WidgetTree::new();
    tree.mount(Widget::from(Focus::new(Widget::box_(
        Size::new(40., 40.),
        Color::WHITE,
    ))))
    .expect("mount focus wrapper");
    layout_loose(&mut tree);
    // A default Focus passes its child through: no node, no target, and the
    // child itself stays hittable.
    assert_eq!(tree.focusable_elements().len(), plain_targets);
    assert!(tree.hit_test(Offset::new(20., 20.)).is_some());
}

#[test]
fn focus_can_request_focus_false_excludes_from_traversal() {
    let node = FocusNode::new();
    let mut tree = WidgetTree::new();
    tree.mount(
        Focus::new(Widget::box_(Size::new(40., 40.), Color::WHITE))
            .node(node.clone())
            .autofocus(true)
            .can_request_focus(false)
            .into(),
    )
    .expect("mount");
    layout_loose(&mut tree);
    assert!(!node.can_request_focus());
    assert!(tree.focusable_elements().is_empty());
    assert_eq!(tree.autofocus_element(), None);
}

#[test]
fn focus_node_replacement_detaches_old_node() {
    let old = FocusNode::new();
    let new = FocusNode::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Focus::new(Widget::box_(Size::new(40., 40.), Color::WHITE))
                .node(old.clone())
                .into(),
        )
        .expect("mount");
    layout_loose(&mut tree);
    old.request_focus();
    assert_eq!(tree.focused_keyboard_element(), Some(root));

    tree.update(
        root,
        Focus::new(Widget::box_(Size::new(40., 40.), Color::WHITE))
            .node(new.clone())
            .into(),
    )
    .expect("replace focus node");
    // The old node's local focus flag is untouched; tree resolution follows
    // only the currently attached node.
    assert!(old.has_focus());
    assert_eq!(tree.focused_keyboard_element(), None);
    new.request_focus();
    assert_eq!(tree.focused_keyboard_element(), Some(root));
}

#[test]
fn removing_focused_child_clears_keyboard_resolution() {
    let node = FocusNode::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Stack::new([Widget::from(
                Focus::new(Widget::box_(Size::new(40., 40.), Color::WHITE)).node(node.clone()),
            )])
            .into(),
        )
        .expect("mount");
    layout_loose(&mut tree);
    let child = tree.children(root).expect("stack child")[0];
    node.request_focus();
    assert_eq!(tree.focused_keyboard_element(), Some(child));

    tree.update(
        root,
        Stack::new([Widget::box_(Size::new(10., 10.), Color::WHITE)]).into(),
    )
    .expect("remove focused child");
    assert_eq!(tree.focused_keyboard_element(), None);
    // Mirroring focus onto a stale id is a harmless no-op.
    tree.set_keyboard_focus(child, false);
}

#[test]
fn focus_scope_autofocus_selects_first_descendant() {
    let node = FocusNode::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            FocusScope::new(
                Focus::new(Widget::box_(Size::new(40., 40.), Color::WHITE)).node(node.clone()),
            )
            .autofocus(true)
            .into(),
        )
        .expect("mount");
    layout_loose(&mut tree);
    // The tree advertises the descendant; only the runtime mirrors focus
    // onto the node, so the flag stays clear at tree level.
    assert_eq!(tree.autofocus_element(), Some(root));
    assert!(!node.has_focus());
}

#[test]
fn focus_scope_policies_order_descendants() {
    let upper = FocusNode::new();
    let lower = FocusNode::new();
    let build = || {
        Widget::from(Stack::new([
            Widget::from(
                Positioned::new(Widget::from(
                    Focus::new(Widget::box_(Size::new(40., 20.), Color::WHITE)).node(lower.clone()),
                ))
                .top(50.),
            ),
            Widget::from(
                Positioned::new(Widget::from(
                    Focus::new(Widget::box_(Size::new(40., 20.), Color::WHITE)).node(upper.clone()),
                ))
                .top(0.),
            ),
        ]))
    };
    let members = |tree: &WidgetTree, root: ElementId| {
        let positioned = tree.children(root).expect("scope children");
        assert_eq!(positioned.len(), 2);
        let first = tree.children(positioned[0]).expect("first focus")[0];
        let second = tree.children(positioned[1]).expect("second focus")[0];
        (first, second)
    };

    // Widget order follows registration: lower child first.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(FocusScope::new(build()).widget_order().into())
        .expect("mount");
    layout_loose(&mut tree);
    let (first, second) = members(&tree, root);
    assert_eq!(tree.focusable_elements(), vec![first, second]);

    // Reading order sorts by top edge: the upper child leads despite
    // registering second.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(FocusScope::new(build()).reading_order().into())
        .expect("mount");
    layout_loose(&mut tree);
    let (first, second) = members(&tree, root);
    assert_eq!(tree.focusable_elements(), vec![second, first]);

    // Explicit numeric order beats both position and registration.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            FocusScope::new(Stack::new([
                Widget::from(
                    Positioned::new(
                        FocusTraversalOrder::new(Widget::from(
                            Focus::new(Widget::box_(Size::new(40., 20.), Color::WHITE))
                                .node(lower.clone()),
                        ))
                        .order(2.),
                    )
                    .top(0.),
                ),
                Widget::from(
                    Positioned::new(
                        FocusTraversalOrder::new(Widget::from(
                            Focus::new(Widget::box_(Size::new(40., 20.), Color::WHITE))
                                .node(upper.clone()),
                        ))
                        .order(1.),
                    )
                    .top(50.),
                ),
            ]))
            .ordered()
            .into(),
        )
        .expect("mount");
    layout_loose(&mut tree);
    let (first, second) = members(&tree, root);
    assert_eq!(tree.focusable_elements(), vec![second, first]);
}

#[test]
fn keyboard_listener_callback_replacement() {
    let first = Rc::new(Cell::new(0));
    let second = Rc::new(Cell::new(0));
    let observed_first = first.clone();
    let observed_second = second.clone();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            KeyboardListener::new(Widget::box_(Size::new(40., 40.), Color::WHITE))
                .on_key(move |_| {
                    observed_first.set(observed_first.get() + 1);
                    false
                })
                .into(),
        )
        .expect("mount");
    assert!(!tree.dispatch_keyboard(Some(root), key_down(Code::KeyA)));
    assert_eq!(first.get(), 1);

    tree.update(
        root,
        KeyboardListener::new(Widget::box_(Size::new(40., 40.), Color::WHITE))
            .on_key(move |_| {
                observed_second.set(observed_second.get() + 1);
                false
            })
            .into(),
    )
    .expect("replace callback");
    assert!(!tree.dispatch_keyboard(Some(root), key_down(Code::KeyA)));
    assert_eq!(first.get(), 1, "the old callback must stay detached");
    assert_eq!(second.get(), 1);
}

#[test]
fn keyboard_listener_identical_reapplication_bails_out() {
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let listener = Widget::from(
        KeyboardListener::new(Widget::box_(Size::new(40., 40.), Color::WHITE)).on_key(move |_| {
            observed.set(observed.get() + 1);
            false
        }),
    );
    let mut tree = WidgetTree::new();
    let root = tree.mount(listener.clone()).expect("mount");
    layout_loose(&mut tree);
    let _ = tree.paint();
    let before = tree.diagnostics();
    tree.update(root, listener).expect("identical reapply");
    layout_loose(&mut tree);
    let _ = tree.paint();
    let after = tree.diagnostics();
    assert!(after.identical_child_bailouts > before.identical_child_bailouts);
    assert_eq!(after.layouts, before.layouts);
    assert_eq!(after.paints, before.paints);
    assert!(!tree.dispatch_keyboard(Some(root), key_down(Code::KeyA)));
    assert_eq!(calls.get(), 1);
}

#[test]
fn keyboard_listener_on_key_event_alias_routes() {
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let listener = KeyboardListener::new(SizedBox::shrink()).on_key_event(move |_| {
        observed.set(observed.get() + 1);
        true
    });
    assert!(listener.handle(key_down(Code::Enter)));
    assert_eq!(calls.get(), 1);
}

#[test]
fn keyboard_listener_autofocus_needs_a_node() {
    let mut tree = WidgetTree::new();
    tree.mount(
        KeyboardListener::new(Widget::box_(Size::new(40., 40.), Color::WHITE))
            .autofocus(true)
            .into(),
    )
    .expect("mount");
    layout_loose(&mut tree);
    assert_eq!(
        tree.autofocus_element(),
        None,
        "autofocus without a focus node advertises nothing"
    );

    let node = FocusNode::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            KeyboardListener::new(Widget::box_(Size::new(40., 40.), Color::WHITE))
                .focus_node(node)
                .autofocus(true)
                .into(),
        )
        .expect("mount");
    layout_loose(&mut tree);
    assert_eq!(tree.autofocus_element(), Some(root));
}

#[test]
fn keyboard_listener_include_semantics_governs_listener_affordance() {
    let calls = Rc::new(Cell::new(0));
    let build = |include: bool| {
        let observed = calls.clone();
        Widget::from(
            Semantics::new(
                KeyboardListener::new(Widget::from(Text::new("child label")))
                    .on_key(move |_| {
                        observed.set(observed.get() + 1);
                        false
                    })
                    .include_semantics(include),
            )
            .role(SemanticRole::Group)
            .label("listener")
            .action(SemanticAction::Increment)
            .action(SemanticAction::Decrement),
        )
    };
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Stack::new([build(true), Widget::from(Text::new("sibling"))]).into())
        .expect("mount");
    let refresh = |tree: &mut WidgetTree| {
        layout_loose(tree);
        let _ = tree.paint();
        tree.update_semantics();
    };
    refresh(&mut tree);
    let stack_kids: Vec<_> = tree.children(root).expect("stack children").to_vec();
    let listener_text = tree.children(stack_kids[0]).expect("listener child")[0];
    let sibling_text = stack_kids[1];
    let listener_semantic = tree
        .semantic_node_for_element(stack_kids[0])
        .expect("listener node");
    let child_semantic = tree
        .semantic_node_for_element(listener_text)
        .expect("child node");
    let sibling_semantic = tree
        .semantic_node_for_element(sibling_text)
        .expect("sibling node");
    let actions = |tree: &WidgetTree| {
        tree.semantics()
            .node(listener_semantic)
            .expect("listener node")
            .actions
            .clone()
    };
    assert!(actions(&tree).contains(&SemanticActionKind::Increment));
    assert!(actions(&tree).contains(&SemanticActionKind::Decrement));

    // With the flag off the listener node keeps its label but withdraws
    // exactly its own keyboard affordance: child and sibling nodes,
    // retained identity, and key dispatch are untouched, and the
    // render-neutral toggle schedules no layout or picture work.
    let before = tree.diagnostics();
    tree.update(
        root,
        Stack::new([build(false), Widget::from(Text::new("sibling"))]).into(),
    )
    .expect("disable include_semantics");
    refresh(&mut tree);
    let after = tree.diagnostics();
    assert_eq!(after.layouts, before.layouts);
    assert_eq!(after.paints, before.paints);
    assert_eq!(after.composites, before.composites);
    assert_eq!(
        tree.children(root).expect("stack children").to_vec(),
        stack_kids
    );
    assert!(!actions(&tree).contains(&SemanticActionKind::Increment));
    assert!(!actions(&tree).contains(&SemanticActionKind::Decrement));
    assert_eq!(
        tree.semantics()
            .node(listener_semantic)
            .expect("listener node")
            .label
            .as_deref(),
        Some("listener")
    );
    assert_eq!(
        tree.semantic_node_for_element(listener_text),
        Some(child_semantic)
    );
    assert_eq!(
        tree.semantic_node_for_element(sibling_text),
        Some(sibling_semantic)
    );
    assert!(!tree.dispatch_keyboard(Some(stack_kids[0]), key_down(Code::KeyA)));
    assert_eq!(calls.get(), 1);

    // Re-enabling restores the affordance on the same retained elements.
    tree.update(
        root,
        Stack::new([build(true), Widget::from(Text::new("sibling"))]).into(),
    )
    .expect("re-enable include_semantics");
    refresh(&mut tree);
    assert!(actions(&tree).contains(&SemanticActionKind::Increment));
    assert!(actions(&tree).contains(&SemanticActionKind::Decrement));
    assert_eq!(
        tree.children(root).expect("stack children").to_vec(),
        stack_kids
    );
    assert_eq!(
        tree.semantic_node_for_element(listener_text),
        Some(child_semantic)
    );
    assert_eq!(
        tree.semantic_node_for_element(sibling_text),
        Some(sibling_semantic)
    );
    assert!(!tree.dispatch_keyboard(Some(stack_kids[0]), key_down(Code::KeyA)));
    assert_eq!(calls.get(), 2);
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum TestCommand {
    Submit,
}

#[test]
fn keyboard_listener_with_shortcuts_dispatches_typed_intents() {
    use std::cell::RefCell;
    let mut shortcuts = Shortcuts::<TestCommand>::typed();
    shortcuts.bind_logical(
        LogicalShortcutKey::new(KeyboardKey::Character("s".into()), Modifiers::CONTROL),
        TestCommand::Submit,
    );
    let received = Rc::new(RefCell::new(Vec::new()));
    let observed = received.clone();
    let mut actions = Actions::<TestCommand>::typed();
    actions.register_handler(TestCommand::Submit, move |intent| {
        observed.borrow_mut().push(*intent.command());
        ActionResult::Handled
    });

    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            KeyboardListener::new(Widget::box_(Size::new(40., 40.), Color::WHITE))
                .with_shortcuts(Rc::new(shortcuts), Rc::new(actions))
                .into(),
        )
        .expect("mount");
    let press = |code: Code, key: KeyboardKey| {
        let mut event = KeyboardEvent::key_down(key, code);
        event.modifiers = Modifiers::CONTROL;
        event
    };
    assert!(!tree.dispatch_keyboard(
        Some(root),
        press(Code::KeyN, KeyboardKey::Character("n".into())),
    ));
    assert!(received.borrow().is_empty());
    assert!(tree.dispatch_keyboard(
        Some(root),
        press(Code::KeyS, KeyboardKey::Character("s".into())),
    ));
    assert_eq!(&*received.borrow(), &[TestCommand::Submit]);
}

#[test]
fn keyboard_listener_node_alias_matches_focus_node_builder() {
    let node = FocusNode::new();
    let via_alias = KeyboardListener::new(SizedBox::shrink()).node(node.clone());
    let via_named = KeyboardListener::new(SizedBox::shrink()).focus_node(node.clone());
    assert_eq!(via_alias.configured_focus_node(), Some(node.clone()));
    assert_eq!(via_named.configured_focus_node(), Some(node));
}

#[test]
fn sibling_focus_and_semantics_survive_neighbor_rebuild() {
    let left_node = FocusNode::new();
    let right_node = FocusNode::new();
    let left = || Widget::from(Focus::new(Widget::from(Text::new("left"))).node(left_node.clone()));
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Stack::new([
                left(),
                Widget::from(
                    Focus::new(Widget::from(Text::new("right v1"))).node(right_node.clone()),
                ),
            ])
            .into(),
        )
        .expect("mount");
    layout_loose(&mut tree);
    tree.update_semantics();
    left_node.request_focus();
    let focused = tree.focused_keyboard_element().expect("left focused");
    let siblings: Vec<_> = tree.children(root).expect("stack children").to_vec();
    assert_eq!(siblings.len(), 2);
    // The Focus wrapper itself owns no semantic node; its labeled child does.
    let left_text = tree.children(siblings[0]).expect("left text")[0];
    let right_text = tree.children(siblings[1]).expect("right text")[0];
    let left_semantic = tree
        .semantic_node_for_element(left_text)
        .expect("left semantics");
    let right_semantic = tree
        .semantic_node_for_element(right_text)
        .expect("right semantics");
    assert_eq!(
        tree.semantics()
            .node(right_semantic)
            .expect("right node")
            .label
            .as_deref(),
        Some("right v1")
    );

    tree.update(
        root,
        Stack::new([
            left(),
            Widget::from(Focus::new(Widget::from(Text::new("right v2"))).node(right_node.clone())),
        ])
        .into(),
    )
    .expect("rebuild neighbor");
    layout_loose(&mut tree);
    tree.update_semantics();
    assert_eq!(
        tree.focused_keyboard_element(),
        Some(focused),
        "rebuilding the right subtree must not move left focus"
    );
    assert_eq!(
        tree.semantic_node_for_element(left_text),
        Some(left_semantic),
        "left semantics must stay stable"
    );
    // Sibling roots share no semantic parent, so the debug dump follows one
    // root; assert through node handles instead.
    let siblings_after: Vec<_> = tree.children(root).expect("stack children").to_vec();
    assert_eq!(siblings_after[0], siblings[0]);
    assert_eq!(siblings_after[1], siblings[1]);
    let right_text_after = tree.children(siblings_after[1]).expect("right text")[0];
    assert_eq!(right_text_after, right_text);
    assert_eq!(
        tree.semantics()
            .node(
                tree.semantic_node_for_element(right_text_after)
                    .expect("right semantics")
            )
            .expect("right node")
            .label
            .as_deref(),
        Some("right v2")
    );
}
