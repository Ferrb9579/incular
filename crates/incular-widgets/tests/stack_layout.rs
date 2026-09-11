//! Stack, Positioned, and IndexedStack layout contracts.
//!
//! Conflict policy (owned by `incular-layout`, pinned here): left
//! wins over right and top wins over bottom; a single edge bounds
//! measurement loosely while opposing edges derive the algorithm
//! size; the child keeps its measured size, so drawing may differ
//! from the algorithm size; non-finite edges are dropped as unset
//! and negative edges are meaningful offsets, not invalid dimensions.
//! Clip behavior is covered by the clip ledger and not re-tested here.

use incular_config::{Alignment, Constraints, StackFit};
use incular_core::{Color, Offset, Size};
use incular_widgets::{
    Focus, FocusNode, IndexedStack, Positioned, Stack, Text, UnconstrainedBox, Widget,
    internal::{ActionId, ElementId, WidgetTree, action},
};

fn layout_tight(tree: &mut WidgetTree, width: f32, height: f32) {
    tree.layout(Constraints::tight(Size::new(width, height)))
        .expect("layout");
}

fn bounds_of(tree: &WidgetTree, id: ElementId) -> (f32, f32, f32, f32) {
    let bounds = tree.element_bounds(id).expect("bounds");
    (
        bounds.origin.x,
        bounds.origin.y,
        bounds.size.width,
        bounds.size.height,
    )
}

fn stack_kids(tree: &WidgetTree, root: ElementId) -> Vec<ElementId> {
    tree.children(root).expect("stack children").to_vec()
}

fn white_box(width: f32, height: f32) -> Widget {
    Widget::box_(Size::new(width, height), Color::WHITE)
}

#[test]
fn positioned_horizontal_combinations_resolve_documented() {
    // A configured axis tightens the child so its measured width equals
    // the placement width. Opposing offsets derive the width and win
    // over an explicit width; a lone edge places against the resolved
    // (explicit or measured) width. Nonnegative right insets are exact.
    let cases: &[(&str, Positioned, (f32, f32))] = &[
        (
            "left_width",
            Positioned::new(white_box(10., 10.)).left(20.).width(30.),
            (20., 30.),
        ),
        (
            "right_width",
            Positioned::new(white_box(10., 10.)).right(20.).width(30.),
            (150., 30.),
        ),
        (
            "left_right",
            Positioned::new(white_box(10., 10.)).left(20.).right(30.),
            (20., 150.),
        ),
        (
            "left_right_beats_width",
            Positioned::new(white_box(10., 10.))
                .left(20.)
                .right(30.)
                .width(5.),
            (20., 150.),
        ),
        ("bare", Positioned::new(white_box(10., 10.)), (0., 10.)),
    ];
    for (name, positioned, (x, width)) in cases {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Stack::new([Widget::from(positioned.clone())]).into())
            .expect("mount");
        layout_tight(&mut tree, 200., 200.);
        let child = stack_kids(&tree, root)[0];
        let inner = tree.children(child).expect("positioned child")[0];
        let (actual_x, _, actual_width, _) = bounds_of(&tree, inner);
        assert_eq!((actual_x, actual_width), (*x, *width), "{name}");
        if *name == "right_width" {
            assert_eq!(200.0 - (actual_x + actual_width), 20.0, "{name}");
        }
    }
}

#[test]
fn positioned_vertical_combinations_resolve_documented() {
    let cases: &[(&str, Positioned, (f32, f32))] = &[
        (
            "top_height",
            Positioned::new(white_box(10., 10.)).top(20.).height(30.),
            (20., 30.),
        ),
        (
            "bottom_height",
            Positioned::new(white_box(10., 10.)).bottom(20.).height(30.),
            (150., 30.),
        ),
        (
            "top_bottom",
            Positioned::new(white_box(10., 10.)).top(20.).bottom(30.),
            (20., 150.),
        ),
        // Opposing edges derive the height and win over explicit height.
        (
            "top_bottom_beats_height",
            Positioned::new(white_box(10., 10.))
                .top(20.)
                .bottom(30.)
                .height(5.),
            (20., 150.),
        ),
        ("bare", Positioned::new(white_box(10., 10.)), (0., 10.)),
    ];
    for (name, positioned, (y, height)) in cases {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Stack::new([Widget::from(positioned.clone())]).into())
            .expect("mount");
        layout_tight(&mut tree, 200., 200.);
        let child = stack_kids(&tree, root)[0];
        let inner = tree.children(child).expect("positioned child")[0];
        let (_, actual_y, _, actual_height) = bounds_of(&tree, inner);
        assert_eq!((actual_y, actual_height), (*y, *height), "{name}");
        if *name == "bottom_height" {
            assert_eq!(200.0 - (actual_y + actual_height), 20.0, "{name}");
        }
    }
}

#[test]
fn positioned_negative_edges_are_meaningful_offsets() {
    // Negative offsets are kept verbatim (overflow), while negative
    // dimensions are invalid and dropped as unset.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Stack::new([
                Widget::from(Positioned::new(white_box(30., 10.)).left(-20.).width(30.)),
                Widget::from(Positioned::new(white_box(10., 10.)).right(20.).left(-5.)),
            ])
            .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 200., 200.);
    let kids = stack_kids(&tree, root);
    // Negative left is honored; the explicit width still sizes the child.
    let first = tree.children(kids[0]).expect("child")[0];
    assert_eq!(bounds_of(&tree, first), (-20., 0., 30., 10.));
    // Negative left wins over right: right is ignored when left is set.
    let second = tree.children(kids[1]).expect("child")[0];
    assert_eq!(bounds_of(&tree, second).0, -5.);

    // A negative width is invalid and dropped, so the child measures its
    // intrinsic size instead.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Stack::new([Widget::from(
                Positioned::new(white_box(25., 10.)).width(-5.),
            )])
            .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 200., 200.);
    let child = stack_kids(&tree, root)[0];
    let inner = tree.children(child).expect("child")[0];
    assert_eq!(bounds_of(&tree, inner).2, 25.);
}

#[test]
fn positioned_setter_order_last_wins_per_field() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Stack::new([Widget::from(
                Positioned::new(white_box(10., 10.)).left(10.).left(25.),
            )])
            .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 200., 200.);
    let child = stack_kids(&tree, root)[0];
    let inner = tree.children(child).expect("child")[0];
    assert_eq!(bounds_of(&tree, inner).0, 25.);
}

#[test]
fn positioned_builder_matches_fluent_construction() {
    let fluent = Positioned::new(white_box(10., 10.))
        .left(10.)
        .top(20.)
        .width(30.)
        .height(40.);
    let built = Positioned::builder()
        .left(10.)
        .top(20.)
        .width(30.)
        .height(40.)
        .child(white_box(10., 10.))
        .build();
    assert_eq!(Widget::from(fluent), Widget::from(built));
}

#[test]
fn stack_fit_loose_expand_passthrough() {
    // Loose: children keep intrinsic size, stack wraps the largest.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Stack::new([white_box(30., 20.), white_box(50., 10.)])
                .fit(StackFit::Loose)
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(bounds_of(&tree, root).2, 50.);
    assert_eq!(bounds_of(&tree, root).3, 20.);
    let kids = stack_kids(&tree, root);
    assert_eq!(bounds_of(&tree, kids[0]).2, 30.);

    // Expand: children stretch to the biggest constraints, stack fills.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Stack::new([white_box(30., 20.), white_box(50., 10.)])
                .fit(StackFit::Expand)
                .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 200., 200.);
    assert_eq!(bounds_of(&tree, root).2, 200.);
    let kids = stack_kids(&tree, root);
    assert_eq!(bounds_of(&tree, kids[0]).2, 200.);

    // Passthrough: incoming constraints reach children unchanged.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Stack::new([white_box(30., 20.)])
                .fit(StackFit::Passthrough)
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(bounds_of(&tree, root).2, 30.);
}

#[test]
fn stack_alignment_positions_asymmetric_children() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Stack::aligned(Alignment::CENTER, [white_box(40., 20.)]).into())
        .expect("mount");
    layout_tight(&mut tree, 200., 100.);
    // Tight constraints size the stack; center splits leftover space.
    let child = stack_kids(&tree, root)[0];
    assert_eq!(bounds_of(&tree, child).0, 80.);
    assert_eq!(bounds_of(&tree, child).1, 40.);
}

#[test]
fn stack_alignment_rebuilds_to_the_same_absolute_result() {
    // Alignment factors are absolute; rebuilding with a different
    // alignment moves the child deterministically, and re-applying the
    // original alignment restores the original position exactly.
    let build = |alignment| Widget::from(Stack::aligned(alignment, [white_box(40., 20.)]));
    let mut tree = WidgetTree::new();
    let root = tree.mount(build(Alignment::TOP_LEFT)).expect("mount");
    layout_tight(&mut tree, 200., 100.);
    let before = bounds_of(&tree, stack_kids(&tree, root)[0]);
    tree.update(root, build(Alignment::BOTTOM_RIGHT))
        .expect("realign");
    layout_tight(&mut tree, 200., 100.);
    let moved = bounds_of(&tree, stack_kids(&tree, root)[0]);
    assert!(moved.0 > before.0 && moved.1 > before.1);
    tree.update(root, build(Alignment::TOP_LEFT))
        .expect("realign back");
    layout_tight(&mut tree, 200., 100.);
    assert_eq!(bounds_of(&tree, stack_kids(&tree, root)[0]), before);
}

#[test]
fn indexed_stack_switches_active_child() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            IndexedStack::new([
                action(Size::new(60., 40.), Color::WHITE, ActionId(1)),
                action(Size::new(30., 20.), Color::WHITE, ActionId(2)),
            ])
            .index(0)
            .into(),
        )
        .expect("mount");
    // Loose constraints size the stack to the largest child; tight
    // constraints would fill instead via constrain.
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    // Size covers the largest child regardless of index.
    assert_eq!(bounds_of(&tree, root).2, 60.);
    let hit_action = |tree: &WidgetTree| {
        let hit = tree.hit_test(Offset::new(10., 10.)).expect("hit");
        tree.action_for_element(tree.element_for_render(hit).expect("element"))
    };
    assert_eq!(hit_action(&tree), Some(ActionId(1)));

    tree.update(
        root,
        IndexedStack::new([
            action(Size::new(60., 40.), Color::WHITE, ActionId(1)),
            action(Size::new(30., 20.), Color::WHITE, ActionId(2)),
        ])
        .index(1)
        .into(),
    )
    .expect("switch index");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(bounds_of(&tree, root).2, 60.);
    assert_eq!(hit_action(&tree), Some(ActionId(2)));

    // Only the active child contributes semantics.
    let mut tree = WidgetTree::new();
    tree.mount(
        IndexedStack::new([
            Widget::from(Text::new("first")),
            Widget::from(Text::new("second")),
        ])
        .index(1)
        .into(),
    )
    .expect("mount");
    layout_tight(&mut tree, 200., 200.);
    tree.update_semantics();
    let dump = tree.semantics_debug_dump();
    assert!(dump.contains("second"), "active semantics present:\n{dump}");
    assert!(
        !dump.contains("first"),
        "inactive semantics absent:\n{dump}"
    );
}

#[test]
fn indexed_stack_fit_policies_size_children() {
    // Loose: children keep intrinsic size; the stack wraps the largest.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            IndexedStack::new([white_box(30., 20.), white_box(50., 10.)])
                .fit(StackFit::Loose)
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(bounds_of(&tree, root).2, 50.);
    assert_eq!(bounds_of(&tree, root).3, 20.);
    let kids = stack_kids(&tree, root);
    assert_eq!(bounds_of(&tree, kids[0]).2, 30.);

    // Expand: children stretch to the biggest constraints, stack fills.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            IndexedStack::new([white_box(30., 20.), white_box(50., 10.)])
                .fit(StackFit::Expand)
                .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 200., 200.);
    assert_eq!(bounds_of(&tree, root).2, 200.);
    assert_eq!(bounds_of(&tree, stack_kids(&tree, root)[0]).2, 200.);

    // Passthrough: incoming constraints reach children unchanged.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            IndexedStack::new([white_box(30., 20.)])
                .fit(StackFit::Passthrough)
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(bounds_of(&tree, root).2, 30.);
}

#[test]
fn indexed_stack_fit_change_reflows_every_child() {
    let build = |fit| {
        IndexedStack::new([white_box(30., 20.), white_box(50., 10.)])
            .fit(fit)
            .index(1)
    };
    let mut tree = WidgetTree::new();
    let root = tree.mount(build(StackFit::Loose).into()).expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    let kids = stack_kids(&tree, root);
    let loose_active = bounds_of(&tree, kids[1]);
    assert_eq!(loose_active.2, 50.);

    // Switching to Expand re-measures the active child to the parent.
    tree.update(root, build(StackFit::Expand).into())
        .expect("switch fit");
    layout_tight(&mut tree, 200., 200.);
    let kids_after = stack_kids(&tree, root);
    assert_eq!(kids_after, kids, "retained identity is stable");
    assert_eq!(bounds_of(&tree, kids_after[1]).2, 200.);
}

#[test]
fn indexed_stack_clip_layers_follow_clip_behavior() {
    use incular_config::Clip;
    use incular_core::Rect;
    use incular_rendering::DisplayList;
    use incular_rendering::PaintCommand;

    let clip_rects = |list: &DisplayList| -> Vec<Rect> {
        let mut out = Vec::new();
        for command in list.commands() {
            if let PaintCommand::PushClip { rect } = command {
                out.push(*rect);
            }
        }
        out
    };

    let build = |clip| {
        IndexedStack::new([white_box(40., 40.), white_box(80., 80.)])
            .index(1)
            .clip_behavior(clip)
    };
    let mut tree = WidgetTree::new();
    let root = tree.mount(build(Clip::HardEdge).into()).expect("mount");
    layout_tight(&mut tree, 100., 100.);
    let clipped = clip_rects(&tree.paint());
    assert_eq!(clipped.len(), 1, "hard edge paints a clip");
    assert_eq!(clipped[0].size, Size::new(100., 100.));

    // Toggling to None rebuilds the layer binding without layout work;
    // the active child keeps its geometry and identity.
    let layouts = tree.diagnostics().layouts;
    tree.update(root, build(Clip::None).into())
        .expect("disable clip");
    layout_tight(&mut tree, 100., 100.);
    assert!(
        clip_rects(&tree.paint()).is_empty(),
        "Clip::None paints none"
    );
    assert_eq!(tree.diagnostics().layouts, layouts + 1);
    assert_eq!(bounds_of(&tree, stack_kids(&tree, root)[1]).2, 80.);
}

#[test]
fn indexed_stack_active_child_overflow_hits_within_parent() {
    // An unconstrained child wider than the stack overflows; hits inside
    // the parent reach it, hits beyond the parent do not descend.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            IndexedStack::new([UnconstrainedBox::new(
                action(Size::new(200., 40.), Color::WHITE, ActionId(3))
                    .accessibility_label("overflowing child"),
            )])
            .index(0)
            .clip_behavior(incular_config::Clip::None)
            .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 100., 100.);
    let wrapper = stack_kids(&tree, root)[0];
    let child = tree.children(wrapper).expect("unconstrained child")[0];
    assert_eq!(bounds_of(&tree, child).2, 200.);
    assert_eq!(tree.action_for_element(child), Some(ActionId(3)));
    assert!(tree.hit_test(Offset::new(90., 10.)).is_some());
    assert!(tree.hit_test(Offset::new(150., 10.)).is_none());

    // Clipping is raster-only: the overflow keeps its semantic bounds.
    tree.update_semantics();
    let node = tree
        .semantic_node_for_element(child)
        .expect("overflow semantics");
    assert!(tree.semantics().node(node).expect("node").bounds.size.width > 100.);
}

#[test]
fn indexed_stack_out_of_range_hides_gracefully() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            IndexedStack::new([white_box(60., 40.), white_box(30., 20.)])
                .index(9)
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    // Layout still measures every child; paint, traversal, and
    // semantics see none. Hit testing treats the stack like an empty
    // container: the stack element itself resolves, with no action.
    assert_eq!(bounds_of(&tree, root).2, 60.);
    let hit = tree.hit_test(Offset::new(10., 10.)).expect("stack hit");
    assert_eq!(tree.element_for_render(hit), Some(root));
    assert_eq!(
        tree.action_for_element(tree.element_for_render(hit).expect("element")),
        None
    );
    assert!(tree.focusable_elements().is_empty());
    tree.update_semantics();
    assert!(tree.semantics_debug_dump().is_empty());
}

#[test]
fn indexed_stack_inactive_focus_leaves_membership() {
    // Inactive children keep laid-out state but lose focus membership;
    // keyboard resolution follows membership while the node flag itself
    // is untouched (the tree never unfocuses nodes on its own).
    let node = FocusNode::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            IndexedStack::new([
                Widget::from(Focus::new(white_box(40., 40.)).node(node.clone())),
                white_box(40., 40.),
            ])
            .index(0)
            .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 200., 200.);
    assert_eq!(tree.focusable_elements().len(), 1);
    node.request_focus();
    tree.focused_keyboard_element().expect("focused");
    tree.update(
        root,
        IndexedStack::new([
            Widget::from(Focus::new(white_box(40., 40.)).node(node.clone())),
            white_box(40., 40.),
        ])
        .index(1)
        .into(),
    )
    .expect("switch away");
    layout_tight(&mut tree, 200., 200.);
    assert!(tree.focusable_elements().is_empty());
    assert_eq!(tree.focused_keyboard_element(), None);
    assert!(node.has_focus(), "node flag untouched by the switch");
}

#[test]
fn indexed_stack_focused_child_becoming_inactive_loses_delivery() {
    // The established contract is retention without eligibility: the
    // inactive child keeps its laid-out subtree and node flags, but focus
    // membership, keyboard resolution, and pointer delivery follow only
    // the selected child.
    let node = FocusNode::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            IndexedStack::new([
                Widget::from(Focus::new(white_box(40., 40.)).node(node.clone())),
                white_box(40., 40.),
            ])
            .index(0)
            .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 200., 200.);
    node.request_focus();
    let focused = tree.focused_keyboard_element().expect("focused");
    // Pointer delivery reaches the active child's subtree before the
    // switch (the hit lands on the Focus element's box child).
    let hit_before = tree
        .hit_test(Offset::new(10., 10.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(
        hit_before,
        Some(tree.children(focused).expect("focus child")[0])
    );

    tree.update(
        root,
        IndexedStack::new([
            Widget::from(Focus::new(white_box(40., 40.)).node(node.clone())),
            white_box(40., 40.),
        ])
        .index(1)
        .into(),
    )
    .expect("switch away");
    layout_tight(&mut tree, 200., 200.);
    // Membership and resolution leave; the node flag is untouched.
    assert!(tree.focusable_elements().is_empty());
    assert_eq!(tree.focused_keyboard_element(), None);
    assert!(node.has_focus(), "node flag untouched by the switch");
    // The hit now resolves to the newly selected child, not the
    // retained-but-inactive one.
    let kids = stack_kids(&tree, root);
    let hit_after = tree
        .hit_test(Offset::new(10., 10.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(hit_after, Some(kids[1]));
}

#[test]
fn indexed_stack_active_child_removal_and_keyed_reorder() {
    // Removing or reordering the active child moves selection delivery to
    // whatever the index now names; retained identities prove no child
    // subtree is rebuilt by the index change itself.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            IndexedStack::new([
                action(Size::new(60., 40.), Color::WHITE, ActionId(1)).with_key(1u64),
                action(Size::new(30., 20.), Color::WHITE, ActionId(2)).with_key(2u64),
            ])
            .index(0)
            .into(),
        )
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    let before = stack_kids(&tree, root);
    let hit_action = |tree: &WidgetTree| {
        let hit = tree.hit_test(Offset::new(10., 10.)).expect("hit");
        tree.action_for_element(tree.element_for_render(hit).expect("element"))
    };
    assert_eq!(hit_action(&tree), Some(ActionId(1)));

    // Keyed reorder: index 0 now names the previous second child, whose
    // identity is retained.
    tree.update(
        root,
        IndexedStack::new([
            action(Size::new(30., 20.), Color::WHITE, ActionId(2)).with_key(2u64),
            action(Size::new(60., 40.), Color::WHITE, ActionId(1)).with_key(1u64),
        ])
        .index(0)
        .into(),
    )
    .expect("reorder");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    let reordered = stack_kids(&tree, root);
    assert_eq!(reordered, vec![before[1], before[0]]);
    assert_eq!(hit_action(&tree), Some(ActionId(2)));

    // Removing the active child: delivery follows the index to the
    // survivor and the removed identity is gone.
    tree.update(
        root,
        IndexedStack::new([action(Size::new(60., 40.), Color::WHITE, ActionId(1)).with_key(1u64)])
            .index(1)
            .into(),
    )
    .expect("remove active child");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    let survivors = stack_kids(&tree, root);
    assert_eq!(survivors, vec![before[0]]);
    // Index 1 names nothing now: the stack itself resolves the hit.
    let hit = tree.hit_test(Offset::new(10., 10.)).expect("stack hit");
    assert_eq!(tree.element_for_render(hit), Some(root));
}

#[test]
fn indexed_stack_out_of_range_then_valid_restores_delivery() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            IndexedStack::new([
                action(Size::new(60., 40.), Color::WHITE, ActionId(1)),
                action(Size::new(30., 20.), Color::WHITE, ActionId(2)),
            ])
            .index(0)
            .into(),
        )
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    let kids = stack_kids(&tree, root);
    let hit_action = |tree: &WidgetTree| {
        let hit = tree.hit_test(Offset::new(10., 10.)).expect("hit");
        tree.action_for_element(tree.element_for_render(hit).expect("element"))
    };
    assert_eq!(hit_action(&tree), Some(ActionId(1)));

    // Out of range hides every child but measures them and keeps identity.
    tree.update(
        root,
        IndexedStack::new([
            action(Size::new(60., 40.), Color::WHITE, ActionId(1)),
            action(Size::new(30., 20.), Color::WHITE, ActionId(2)),
        ])
        .index(9)
        .into(),
    )
    .expect("index out of range");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(stack_kids(&tree, root), kids);
    assert!(tree.focusable_elements().is_empty());
    tree.update_semantics();
    assert!(tree.semantics_debug_dump().is_empty());

    // Back in range: the same retained child delivers again.
    tree.update(
        root,
        IndexedStack::new([
            action(Size::new(60., 40.), Color::WHITE, ActionId(1)),
            action(Size::new(30., 20.), Color::WHITE, ActionId(2)),
        ])
        .index(1)
        .into(),
    )
    .expect("index back in range");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(stack_kids(&tree, root), kids);
    assert_eq!(hit_action(&tree), Some(ActionId(2)));
}

#[test]
fn indexed_stack_inactive_child_size_change_applies_on_selection() {
    use incular_text::TextEditingController;
    use incular_widgets::EditableText;

    // An inactive text child keeps its controller live: edits apply to
    // the retained subtree while hidden, and the new size is visible as
    // soon as the index selects it again.
    let controller = TextEditingController::with_text("hi");
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            IndexedStack::new([
                white_box(60., 40.),
                Widget::from(EditableText::new(controller.clone()).multiline(true)),
            ])
            .index(1)
            .into(),
        )
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    let kids = stack_kids(&tree, root);
    let before = bounds_of(&tree, kids[1]).3;

    tree.update(
        root,
        IndexedStack::new([
            white_box(60., 40.),
            Widget::from(EditableText::new(controller.clone()).multiline(true)),
        ])
        .index(0)
        .into(),
    )
    .expect("hide editor");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    // Editing the hidden controller still mutates retained state.
    controller.set_text("hi\nthere\nagain\nmore");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(stack_kids(&tree, root), kids);

    tree.update(
        root,
        IndexedStack::new([
            white_box(60., 40.),
            Widget::from(EditableText::new(controller.clone()).multiline(true)),
        ])
        .index(1)
        .into(),
    )
    .expect("show editor");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(stack_kids(&tree, root), kids);
    assert!(
        bounds_of(&tree, kids[1]).3 > before,
        "hidden edits grew the retained editor"
    );
    assert_eq!(controller.text(), "hi\nthere\nagain\nmore");
}

#[test]
fn indexed_stack_inactive_child_keeps_subscriptions_live() {
    use incular_text::{TextAlign, TextStyle};
    use incular_widgets::{Column, SelectionAreaController};
    use std::{cell::Cell, rc::Rc};

    // Subscriptions belong to the retained controller, not to index
    // selection: a geometry listener on an inactive child's area still
    // fires when the selection changes programmatically while hidden.
    let controller = SelectionAreaController::new();
    let area_child = || {
        Widget::selection_area(
            controller.clone(),
            Column::new(vec![Widget::selectable_text_styled(
                "hidden text",
                TextStyle::default(),
                TextAlign::Start,
            )])
            .into(),
        )
    };
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            IndexedStack::new([white_box(60., 40.), area_child()])
                .index(1)
                .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 200., 200.);
    let notifications = Rc::new(Cell::new(0u32));
    let observed = notifications.clone();
    let _token = controller.add_geometry_listener(move |_| {
        observed.set(observed.get() + 1);
    });

    // Hide the area, then drive a selection on its retained child.
    tree.update(
        root,
        IndexedStack::new([white_box(60., 40.), area_child()])
            .index(0)
            .into(),
    )
    .expect("hide area");
    layout_tight(&mut tree, 200., 200.);
    let area = stack_kids(&tree, root)[1];
    let column = tree.children(area).expect("area child")[0];
    let label = tree.children(column).expect("column child")[0];
    let origin = tree.element_bounds(label).expect("bounds").origin;
    assert!(tree.selectable_text_set_selection(label, origin, false));
    assert!(tree.selectable_text_set_selection(label, origin + Offset::new(30., 0.), true));
    assert_eq!(
        notifications.get(),
        2,
        "hidden child's subscription still delivers (down and extend)"
    );
    assert_eq!(controller.selected_text(), "hidd");

    // Restoring the index keeps the same subscription delivering.
    tree.update(
        root,
        IndexedStack::new([white_box(60., 40.), area_child()])
            .index(1)
            .into(),
    )
    .expect("show area");
    layout_tight(&mut tree, 200., 200.);
    assert!(tree.selectable_text_set_selection(label, origin, false));
    assert_eq!(notifications.get(), 3);
}

#[test]
fn stack_builders_match_fluent_construction() {
    let fluent = Stack::new([white_box(10., 10.)])
        .alignment(Alignment::CENTER)
        .fit(StackFit::Expand);
    let built = Stack::builder()
        .children(vec![white_box(10., 10.)])
        .alignment(Alignment::CENTER)
        .fit(StackFit::Expand)
        .build();
    assert_eq!(Widget::from(fluent), Widget::from(built));

    let fluent_indexed = IndexedStack::new([white_box(10., 10.)]).index(1);
    let built_indexed = IndexedStack::builder()
        .children(vec![white_box(10., 10.)])
        .index(1)
        .build();
    assert_eq!(Widget::from(fluent_indexed), Widget::from(built_indexed));
}

#[test]
fn stack_keyed_reorder_and_removal() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Stack::new([
                white_box(30., 30.).with_key(1u64),
                Positioned::new(white_box(20., 20.))
                    .left(100.)
                    .top(50.)
                    .into(),
                white_box(10., 10.).with_key(3u64),
            ])
            .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 200., 200.);
    let before = stack_kids(&tree, root);
    assert_eq!(before.len(), 3);
    let positioned_id = before[1];

    // Reorder keyed children: identities permute, positions follow widgets.
    tree.update(
        root,
        Stack::new([
            white_box(10., 10.).with_key(3u64),
            Positioned::new(white_box(20., 20.))
                .left(100.)
                .top(50.)
                .into(),
            white_box(30., 30.).with_key(1u64),
        ])
        .into(),
    )
    .expect("reorder");
    layout_tight(&mut tree, 200., 200.);
    let after = stack_kids(&tree, root);
    assert_eq!(after, vec![before[2], positioned_id, before[0]]);
    assert_eq!(bounds_of(&tree, after[2]).0, 0.);

    // Removal recomputes from the survivors; survivors keep identity.
    tree.update(
        root,
        Stack::new([white_box(10., 10.).with_key(3u64)]).into(),
    )
    .expect("remove");
    layout_tight(&mut tree, 200., 200.);
    let remaining = stack_kids(&tree, root);
    assert_eq!(remaining, vec![before[2]]);
}

#[test]
fn stack_identical_reapplication_bails_out() {
    let mut tree = WidgetTree::new();
    let stack = Widget::from(Stack::aligned(Alignment::CENTER, [white_box(40., 20.)]));
    let root = tree.mount(stack.clone()).expect("mount");
    layout_tight(&mut tree, 200., 100.);
    let _ = tree.paint();
    let before = tree.diagnostics();
    tree.update(root, stack).expect("identical reapply");
    layout_tight(&mut tree, 200., 100.);
    let _ = tree.paint();
    let after = tree.diagnostics();
    assert!(after.identical_child_bailouts > before.identical_child_bailouts);
    assert_eq!(after.layouts, before.layouts);
    assert_eq!(after.paints, before.paints);
    assert_eq!(after.composites, before.composites);
}
