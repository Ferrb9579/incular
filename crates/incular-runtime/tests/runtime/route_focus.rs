use super::*;
use incular_navigation::{Navigator, Page, PageKey, RouteId};
use incular_widgets::{
    Focus, FocusNode,
    internal::{ElementId, Key},
};

fn focus_box(node: FocusNode) -> Widget {
    Focus::new(Widget::box_(Size::new(40., 40.), Color::WHITE))
        .node(node)
        .into()
}

fn plain_box() -> Widget {
    Widget::box_(Size::new(40., 40.), Color::WHITE)
}

fn frame(runtime: &mut Runtime) {
    runtime
        .run_frame(Constraints::tight(Size::new(200., 200.)))
        .expect("frame");
}

fn tab(runtime: &mut Runtime) {
    let _ = runtime.handle_input(InputEvent::Key(key_down(Code::Tab)));
}

fn route_page(name: &str) -> Page {
    Page::new(name, plain_box())
}

fn push_route(navigator: &Navigator, name: &str) -> RouteId {
    navigator.push_page(route_page(name));
    navigator.current().expect("pushed route").id
}

/// Mounts two focusable boxes, focuses the first through autofocus, and
/// reaches the second with one Tab. Returns the discovered element ids so
/// the oracle maps real mounted identities, never names or indices.
fn two_focus_tree() -> (
    FocusNode,
    FocusNode,
    Widget,
    impl Fn(ElementId, ElementId, RouteId, RouteId) -> RouteFocusState,
) {
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    let tree = Widget::from(incular_widgets::Column::new(vec![
        Focus::new(Widget::box_(Size::new(40., 40.), Color::WHITE))
            .node(node_a.clone())
            .autofocus(true)
            .into(),
        focus_box(node_b.clone()),
    ]));
    let make_state = move |ea: ElementId, eb: ElementId, ra: RouteId, rb: RouteId| {
        RouteFocusState::new(move |_, id| {
            if id == ea {
                Some(ra)
            } else if id == eb {
                Some(rb)
            } else {
                None
            }
        })
    };
    (node_a, node_b, tree, make_state)
}

#[test]
fn push_pop_saves_and_restores_route_focus() {
    let (node_a, node_b, tree, make_state) = two_focus_tree();
    let mut runtime = Runtime::new(tree).unwrap();
    frame(&mut runtime);
    let navigator = Navigator::new();
    let ra = push_route(&navigator, "a");
    let ea = runtime.focused_element().expect("autofocus lands on A");
    assert!(node_a.has_focus());
    tab(&mut runtime);
    let eb = runtime.focused_element().expect("tab reaches B");
    assert!(node_b.has_focus());
    assert_ne!(ea, eb);
    // Refocus A while route A is active, then deactivate it.
    tab(&mut runtime);
    assert_eq!(runtime.focused_element(), Some(ea));
    let rb = push_route(&navigator, "b");
    let mut focus = make_state(ea, eb, ra, rb);
    focus.save_focused(&runtime, ra);
    // While B is active, focus moves to B's element.
    tab(&mut runtime);
    assert_eq!(runtime.focused_element(), Some(eb));
    // Popping back revalidates and restores A's element.
    navigator.pop();
    focus.forget(rb);
    focus.restore_saved(&mut runtime, &navigator, ra);
    assert_eq!(runtime.focused_element(), Some(ea));
    assert!(node_a.has_focus());
    assert!(!node_b.has_focus());
}

#[test]
fn keyed_reorder_preserves_focus_records() {
    let (node_a, _node_b, tree, make_state) = two_focus_tree();
    let mut runtime = Runtime::new(tree).unwrap();
    frame(&mut runtime);
    let key = |name: &str| PageKey::new(name).unwrap();
    let navigator = Navigator::new();
    navigator
        .set_pages([
            Page::new("b", plain_box()).key(key("b")),
            Page::new("a", plain_box()).key(key("a")),
        ])
        .unwrap();
    let rb = navigator.routes()[0].id;
    let ra = navigator.routes()[1].id;
    let ea = runtime.focused_element().expect("autofocus lands on A");
    tab(&mut runtime);
    let eb = runtime.focused_element().expect("tab reaches B");
    tab(&mut runtime);
    assert_eq!(runtime.focused_element(), Some(ea));
    let mut focus = make_state(ea, eb, ra, rb);
    // Reorder deactivates A (top moves to B) without removing it: the
    // record survives keyed reconciliation and restores after reordering
    // back, while the route that never saved restores nothing.
    navigator
        .set_pages([
            Page::new("a", plain_box()).key(key("a")),
            Page::new("b", plain_box()).key(key("b")),
        ])
        .unwrap();
    focus.save_focused(&runtime, ra);
    focus.restore_saved(&mut runtime, &navigator, rb);
    assert_eq!(runtime.focused_element(), Some(ea));
    tab(&mut runtime);
    assert_eq!(runtime.focused_element(), Some(eb));
    navigator
        .set_pages([
            Page::new("b", plain_box()).key(key("b")),
            Page::new("a", plain_box()).key(key("a")),
        ])
        .unwrap();
    focus.save_focused(&runtime, rb);
    focus.restore_saved(&mut runtime, &navigator, ra);
    assert_eq!(runtime.focused_element(), Some(ea));
    assert!(node_a.has_focus());
}

#[test]
fn permanent_removal_forgets_and_never_restores() {
    let (node_a, node_b, tree, make_state) = two_focus_tree();
    let mut runtime = Runtime::new(tree).unwrap();
    frame(&mut runtime);
    let navigator = Navigator::new();
    let ra = push_route(&navigator, "a");
    let rb = push_route(&navigator, "b");
    let ea = runtime.focused_element().expect("autofocus lands on A");
    tab(&mut runtime);
    let eb = runtime.focused_element().expect("tab reaches B");
    let mut focus = make_state(ea, eb, ra, rb);
    // A deactivates with focus on B's element, then is removed permanently:
    // forgetting drops the record, so a later restore is a no-op even
    // though B's element is still focused and valid.
    tab(&mut runtime);
    assert_eq!(runtime.focused_element(), Some(ea));
    focus.save_focused(&runtime, ra);
    navigator.pop();
    navigator.pop();
    focus.forget(rb);
    focus.forget(ra);
    focus.retain_mounted(&navigator);
    tab(&mut runtime);
    assert_eq!(runtime.focused_element(), Some(eb));
    focus.restore_saved(&mut runtime, &navigator, ra);
    assert_eq!(runtime.focused_element(), Some(eb));
    assert!(node_b.has_focus());
    let _ = node_a;
}

#[test]
fn hidden_target_falls_back_to_clear() {
    // Keyed children keep B's element identity across the structural
    // rebuild that unmounts A: the oracle keeps working while the saved
    // target dies.
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    let show_a = Signal::new(true);
    let show_a_for_builder = show_a.clone();
    let node_a_for_builder = node_a.clone();
    let node_b_for_builder = node_b.clone();
    let tree = Widget::from(incular_widgets::Column::new(vec![
        Widget::from(
            Focus::new(Widget::box_(Size::new(40., 40.), Color::WHITE))
                .node(node_a.clone())
                .autofocus(true),
        )
        .with_key(1_u64),
        focus_box(node_b.clone()).with_key(2_u64),
    ]));
    let mut runtime = Runtime::new(tree).unwrap();
    frame(&mut runtime);
    let root = runtime.tree().root().expect("root");
    runtime
        .register_builder(root, move || {
            let mut children = Vec::new();
            if show_a_for_builder.get() {
                children.push(
                    Widget::from(
                        Focus::new(Widget::box_(Size::new(40., 40.), Color::WHITE))
                            .node(node_a_for_builder.clone()),
                    )
                    .with_key(1_u64),
                );
            }
            children.push(focus_box(node_b_for_builder.clone()).with_key(2_u64));
            incular_widgets::Column::new(children).into()
        })
        .expect("builder registers");
    let ea = runtime.focused_element().expect("autofocus lands on A");
    tab(&mut runtime);
    let eb = runtime.focused_element().expect("tab reaches B");
    tab(&mut runtime);
    assert_eq!(runtime.focused_element(), Some(ea));
    let navigator = Navigator::new();
    let ra = push_route(&navigator, "a");
    let rb = push_route(&navigator, "b");
    let mut focus = RouteFocusState::new(move |_, id| {
        if id == ea {
            Some(ra)
        } else if id == eb {
            Some(rb)
        } else {
            None
        }
    });
    focus.save_focused(&runtime, ra);
    show_a.set(false);
    frame(&mut runtime);
    assert!(!runtime.tree().element_exists(ea));
    // Focus B first so the fallback has something visible to do: the dead
    // target must clear focus, never resurrect it.
    tab(&mut runtime);
    assert_eq!(runtime.focused_element(), Some(eb));
    navigator.pop();
    focus.forget(rb);
    focus.restore_saved(&mut runtime, &navigator, ra);
    assert_eq!(runtime.focused_element(), None);
    assert!(!node_b.has_focus());
}

#[test]
fn stale_arena_generation_never_restores() {
    let node_b = FocusNode::new();
    let node_old = FocusNode::new();
    let node_new = FocusNode::new();
    let show_new = Signal::new(false);
    let show_new_for_builder = show_new.clone();
    let node_old_for_builder = node_old.clone();
    let node_new_for_builder = node_new.clone();
    let node_b_for_builder = node_b.clone();
    let tree = Widget::from(incular_widgets::Column::new(vec![
        focus_box(node_old.clone()).with_key(1_u64),
        focus_box(node_b.clone()).with_key(2_u64),
    ]));
    let mut runtime = Runtime::new(tree).unwrap();
    frame(&mut runtime);
    let root = runtime.tree().root().expect("root");
    runtime
        .register_builder(root, move || {
            let slot = if show_new_for_builder.get() {
                focus_box(node_new_for_builder.clone()).with_key(3_u64)
            } else {
                focus_box(node_old_for_builder.clone()).with_key(1_u64)
            };
            incular_widgets::Column::new(vec![
                slot,
                focus_box(node_b_for_builder.clone()).with_key(2_u64),
            ])
            .into()
        })
        .expect("builder registers");
    frame(&mut runtime);
    for _ in 0..3 {
        if node_old.has_focus() {
            break;
        }
        tab(&mut runtime);
    }
    let stale = runtime.focused_element().expect("old slot focused");
    assert!(node_old.has_focus());
    let navigator = Navigator::new();
    let ra = push_route(&navigator, "a");
    let rb = push_route(&navigator, "b");
    let eb = {
        tab(&mut runtime);
        runtime.focused_element().expect("tab reaches B")
    };
    tab(&mut runtime);
    assert_eq!(runtime.focused_element(), Some(stale));
    let mut focus = RouteFocusState::new(move |_, id| {
        if id == stale {
            Some(ra)
        } else if id == eb {
            Some(rb)
        } else {
            None
        }
    });
    focus.save_focused(&runtime, ra);
    // Replace the slot content: the arena may reuse the index, but the
    // generation differs, so the saved id stays dead while the new element
    // is live and focusable.
    show_new.set(true);
    frame(&mut runtime);
    assert!(!runtime.tree().element_exists(stale));
    for _ in 0..2 {
        if node_new.has_focus() {
            break;
        }
        tab(&mut runtime);
    }
    assert!(node_new.has_focus());
    let live_replacement = runtime.focused_element().expect("new slot focused");
    assert_ne!(live_replacement, stale);
    navigator.pop();
    focus.forget(rb);
    focus.restore_saved(&mut runtime, &navigator, ra);
    // The live replacement is focusable but attributable to no route: the
    // stale id is never restored, and the unrelated current focus is left
    // alone rather than cleared for a dead record.
    assert_eq!(runtime.focused_element(), Some(live_replacement));
    assert!(node_new.has_focus());
}

#[test]
fn disabled_target_falls_back_while_mounted() {
    let (node_a, node_b, tree, make_state) = two_focus_tree();
    let mut runtime = Runtime::new(tree).unwrap();
    frame(&mut runtime);
    let navigator = Navigator::new();
    let ra = push_route(&navigator, "a");
    let rb = push_route(&navigator, "b");
    let ea = runtime.focused_element().expect("autofocus lands on A");
    tab(&mut runtime);
    let eb = runtime.focused_element().expect("tab reaches B");
    tab(&mut runtime);
    assert_eq!(runtime.focused_element(), Some(ea));
    let mut focus = make_state(ea, eb, ra, rb);
    focus.save_focused(&runtime, ra);
    // Disabling keeps the element mounted but removes it from the
    // focusable set: the restore must clear rather than refocus it.
    node_a.set_can_request_focus(false);
    tab(&mut runtime);
    assert_eq!(runtime.focused_element(), Some(eb));
    navigator.pop();
    focus.forget(rb);
    focus.restore_saved(&mut runtime, &navigator, ra);
    assert!(runtime.tree().element_exists(ea));
    assert_eq!(runtime.focused_element(), None);
    assert!(!node_a.has_focus());
    assert!(!node_b.has_focus());
}

#[test]
fn foreign_target_clears_instead_of_leaking() {
    // Deliberately no autofocus anywhere: the orphan rule alone must clear
    // the removed route's lingering focus.
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    let tree = Widget::from(incular_widgets::Column::new(vec![
        focus_box(node_a.clone()),
        focus_box(node_b.clone()),
    ]));
    let mut runtime = Runtime::new(tree).unwrap();
    frame(&mut runtime);
    let navigator = Navigator::new();
    let ra = push_route(&navigator, "a");
    let rb = push_route(&navigator, "b");
    for _ in 0..3 {
        if node_a.has_focus() {
            break;
        }
        tab(&mut runtime);
    }
    let ea = runtime.focused_element().expect("A focused");
    tab(&mut runtime);
    let eb = runtime.focused_element().expect("tab reaches B");
    let mut focus = RouteFocusState::new(move |_, id| {
        if id == ea {
            Some(ra)
        } else if id == eb {
            Some(rb)
        } else {
            None
        }
    });
    // Route A deactivates while focus sits on B's element: the snapshot is
    // kept, but reactivation must not hand B's focus to A.
    focus.save_focused(&runtime, ra);
    navigator.pop();
    focus.forget(rb);
    focus.restore_saved(&mut runtime, &navigator, ra);
    assert_eq!(runtime.focused_element(), None);
    assert!(!node_a.has_focus());
    assert!(!node_b.has_focus());
}

#[test]
fn nested_navigators_keep_independent_focus_records() {
    let (node_a, node_b, tree, make_state) = two_focus_tree();
    let mut runtime = Runtime::new(tree).unwrap();
    frame(&mut runtime);
    let outer = Navigator::new();
    let inner = Navigator::new();
    let oa = push_route(&outer, "outer-a");
    let ob = push_route(&outer, "outer-b");
    let ia = push_route(&inner, "inner-a");
    // Route ids are navigator-scoped: both navigators issue the same first
    // id, so each state must serve exactly its own navigator.
    assert_eq!(oa, ia);
    let ea = runtime.focused_element().expect("autofocus lands on A");
    tab(&mut runtime);
    let eb = runtime.focused_element().expect("tab reaches B");
    let mut outer_focus = make_state(ea, eb, oa, ob);
    let mut inner_focus = RouteFocusState::new(move |_, id| if id == eb { Some(ia) } else { None });
    tab(&mut runtime);
    assert_eq!(runtime.focused_element(), Some(ea));
    outer_focus.save_focused(&runtime, oa);
    tab(&mut runtime);
    assert_eq!(runtime.focused_element(), Some(eb));
    inner_focus.save_focused(&runtime, ia);
    // Interleaved restores stay with their own navigator despite the id
    // collision: outer returns to A, inner returns to B.
    outer_focus.restore_saved(&mut runtime, &outer, oa);
    assert_eq!(runtime.focused_element(), Some(ea));
    assert!(node_a.has_focus());
    inner_focus.restore_saved(&mut runtime, &inner, ia);
    assert_eq!(runtime.focused_element(), Some(eb));
    assert!(node_b.has_focus());
}

#[test]
fn state_disposal_forgets_records() {
    let (node_a, node_b, tree, make_state) = two_focus_tree();
    let mut runtime = Runtime::new(tree).unwrap();
    frame(&mut runtime);
    let navigator = Navigator::new();
    let ra = push_route(&navigator, "a");
    let rb = push_route(&navigator, "b");
    let ea = runtime.focused_element().expect("autofocus lands on A");
    tab(&mut runtime);
    let eb = runtime.focused_element().expect("tab reaches B");
    let mut focus = make_state(ea, eb, ra, rb);
    tab(&mut runtime);
    focus.save_focused(&runtime, ra);
    // Teardown drops the records with the state: a fresh state has no
    // history, so restoring is a no-op that leaves focus untouched.
    drop(focus);
    tab(&mut runtime);
    assert_eq!(runtime.focused_element(), Some(eb));
    let mut fresh = make_state(ea, eb, ra, rb);
    fresh.restore_saved(&mut runtime, &navigator, ra);
    assert_eq!(runtime.focused_element(), Some(eb));
    assert!(node_b.has_focus());
    let _ = node_a;
}

#[test]
fn save_rejects_foreign_focus_and_restore_preserves_unrelated() {
    let (node_a, node_b, tree, make_state) = two_focus_tree();
    let mut runtime = Runtime::new(tree).unwrap();
    frame(&mut runtime);
    let navigator = Navigator::new();
    let ra = push_route(&navigator, "a");
    let rb = push_route(&navigator, "b");
    let ea = runtime.focused_element().expect("autofocus lands on A");
    tab(&mut runtime);
    let eb = runtime.focused_element().expect("tab reaches B");
    let mut focus = make_state(ea, eb, ra, rb);
    // Route A deactivates while focus sits on B's element: the mismatch
    // records no information, so restoring A leaves B's focus untouched.
    focus.save_focused(&runtime, ra);
    tab(&mut runtime);
    assert_eq!(runtime.focused_element(), Some(ea));
    focus.restore_saved(&mut runtime, &navigator, ra);
    assert_eq!(runtime.focused_element(), Some(ea));
    assert!(node_a.has_focus());
    // The contrast: saving while focus is route-owned records it, and a
    // later restore hands it back.
    tab(&mut runtime);
    assert_eq!(runtime.focused_element(), Some(eb));
    focus.save_focused(&runtime, rb);
    tab(&mut runtime);
    assert_eq!(runtime.focused_element(), Some(ea));
    focus.restore_saved(&mut runtime, &navigator, rb);
    assert_eq!(runtime.focused_element(), Some(eb));
    assert!(node_b.has_focus());
}

#[test]
fn hidden_but_mounted_target_restores() {
    // Visibility with state retention keeps its subtree mounted and focus
    // eligible by framework design: hiding is not detachment, so a saved
    // hidden target restores rather than falling back.
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    let visible = Signal::new(true);
    let visible_for_builder = visible.clone();
    let node_a_for_builder = node_a.clone();
    let node_b_for_builder = node_b.clone();
    let tree = Widget::from(incular_widgets::Column::new(vec![
        focus_box(node_a.clone()).with_key(1_u64),
        focus_box(node_b.clone()).with_key(2_u64),
    ]));
    let mut runtime = Runtime::new(tree).unwrap();
    frame(&mut runtime);
    let root = runtime.tree().root().expect("root");
    runtime
        .register_builder(root, move || {
            incular_widgets::Column::new(vec![
                incular_widgets::Visibility::new(focus_box(node_a_for_builder.clone()))
                    .visible(visible_for_builder.get())
                    .maintain_state(true)
                    .into(),
                focus_box(node_b_for_builder.clone()),
            ])
            .into()
        })
        .expect("builder registers");
    frame(&mut runtime);
    for _ in 0..3 {
        if node_a.has_focus() {
            break;
        }
        tab(&mut runtime);
    }
    let ea = runtime.focused_element().expect("A focused");
    tab(&mut runtime);
    let eb = runtime.focused_element().expect("tab reaches B");
    tab(&mut runtime);
    assert_eq!(runtime.focused_element(), Some(ea));
    let navigator = Navigator::new();
    let ra = push_route(&navigator, "a");
    let rb = push_route(&navigator, "b");
    let mut focus = RouteFocusState::new(move |_, id| {
        if id == ea {
            Some(ra)
        } else if id == eb {
            Some(rb)
        } else {
            None
        }
    });
    focus.save_focused(&runtime, ra);
    visible.set(false);
    frame(&mut runtime);
    assert!(runtime.tree().element_exists(ea));
    navigator.pop();
    focus.forget(rb);
    focus.restore_saved(&mut runtime, &navigator, ra);
    assert_eq!(runtime.focused_element(), Some(ea));
    assert!(node_a.has_focus());
}

#[test]
fn inactive_indexed_stack_target_leaves_valid_focus() {
    // An IndexedStack child outside the index loses focus membership while
    // staying mounted: the saved record becomes ineligible, and the
    // fallback must leave the other route's valid focus alone.
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    let index = Signal::new(0_usize);
    let index_for_builder = index.clone();
    let node_a_for_builder = node_a.clone();
    let node_b_for_builder = node_b.clone();
    let tree = incular_widgets::IndexedStack::new(vec![
        focus_box(node_a.clone()).with_key(1_u64),
        focus_box(node_b.clone()).with_key(2_u64),
    ])
    .index(0)
    .into();
    let mut runtime = Runtime::new(tree).unwrap();
    frame(&mut runtime);
    let root = runtime.tree().root().expect("root");
    runtime
        .register_builder(root, move || {
            incular_widgets::IndexedStack::new(vec![
                focus_box(node_a_for_builder.clone()).with_key(1_u64),
                focus_box(node_b_for_builder.clone()).with_key(2_u64),
            ])
            .index(index_for_builder.get())
            .into()
        })
        .expect("builder registers");
    frame(&mut runtime);
    for _ in 0..3 {
        if node_a.has_focus() {
            break;
        }
        tab(&mut runtime);
    }
    let ea = runtime.focused_element().expect("A focused");
    // B is mounted but inactive, so it is queryable yet unfocusable: read
    // its identity from its key rather than by focusing it.
    let eb = runtime
        .tree()
        .element_with_key(&Key::from(2_u64))
        .expect("keyed B element");
    assert_ne!(ea, eb);
    let navigator = Navigator::new();
    let ra = push_route(&navigator, "a");
    let rb = push_route(&navigator, "b");
    let mut focus = RouteFocusState::new(move |_, id| {
        if id == ea {
            Some(ra)
        } else if id == eb {
            Some(rb)
        } else {
            None
        }
    });
    focus.save_focused(&runtime, ra);
    index.set(1);
    frame(&mut runtime);
    assert!(runtime.tree().element_exists(ea));
    assert!(!runtime.tree().focusable_elements().contains(&ea));
    for _ in 0..3 {
        if node_b.has_focus() {
            break;
        }
        tab(&mut runtime);
    }
    assert_eq!(runtime.focused_element(), Some(eb));
    // A's record is ineligible, but B's focus is owned by route B, which is
    // still mounted: the fallback leaves it alone instead of clearing
    // unrelated focus for the dead record.
    focus.restore_saved(&mut runtime, &navigator, ra);
    assert_eq!(runtime.focused_element(), Some(eb));
    assert!(node_b.has_focus());
}

#[test]
fn nested_invalid_record_preserves_outer_focus() {
    let (node_a, node_b, tree, make_state) = two_focus_tree();
    let mut runtime = Runtime::new(tree).unwrap();
    frame(&mut runtime);
    let outer = Navigator::new();
    let inner = Navigator::new();
    let oa = push_route(&outer, "outer-a");
    let ob = push_route(&outer, "outer-b");
    let ia = push_route(&inner, "inner-a");
    let ea = runtime.focused_element().expect("autofocus lands on A");
    tab(&mut runtime);
    let eb = runtime.focused_element().expect("tab reaches B");
    let mut outer_focus = make_state(ea, eb, oa, ob);
    let mut inner_focus = RouteFocusState::new(move |_, id| if id == eb { Some(ia) } else { None });
    // The inner route saves B's element, which is then disabled: the
    // record is invalid, and the outer route's live focus must survive
    // the inner restore.
    tab(&mut runtime);
    assert_eq!(runtime.focused_element(), Some(ea));
    outer_focus.save_focused(&runtime, oa);
    tab(&mut runtime);
    assert_eq!(runtime.focused_element(), Some(eb));
    inner_focus.save_focused(&runtime, ia);
    node_b.set_can_request_focus(false);
    for _ in 0..3 {
        if node_a.has_focus() {
            break;
        }
        tab(&mut runtime);
    }
    assert_eq!(runtime.focused_element(), Some(ea));
    inner_focus.restore_saved(&mut runtime, &inner, ia);
    assert_eq!(runtime.focused_element(), Some(ea));
    assert!(node_a.has_focus());
}

#[test]
fn restore_without_record_leaves_focus_untouched() {
    let (_node_a, node_b, tree, make_state) = two_focus_tree();
    let mut runtime = Runtime::new(tree).unwrap();
    frame(&mut runtime);
    let navigator = Navigator::new();
    let ra = push_route(&navigator, "a");
    let rb = push_route(&navigator, "b");
    let ea = runtime.focused_element().expect("autofocus lands on A");
    tab(&mut runtime);
    let eb = runtime.focused_element().expect("tab reaches B");
    let mut focus = make_state(ea, eb, ra, rb);
    // A route that never saved must not steal focus on activation.
    focus.restore_saved(&mut runtime, &navigator, rb);
    assert_eq!(runtime.focused_element(), Some(eb));
    assert!(node_b.has_focus());
}
