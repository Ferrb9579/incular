//! Stack, Positioned, and IndexedStack layout contracts.
//!
//! Conflict policy (established in `incular-layout`, pinned here):
//! left wins over right, top wins over bottom; left+right derive
//! width (explicit width ignored) and top+bottom derive height;
//! negative or non-finite edges are dropped as unset, never clamped.
//! Clip behavior is covered by the clip ledger and not re-tested here.

use incular_config::{Alignment, Constraints, StackFit, TextDirection};
use incular_core::{Color, Offset, Size};
use incular_widgets::{
    Focus, FocusNode, IndexedStack, Positioned, Stack, Text, Widget,
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
    // Positions follow left, else right-anchored math (which uses the
    // explicit or derived width), else the origin. Measurement takes
    // the explicit width over the derived one as a loose bound, and
    // fixed children shrink to it but never grow: the render keeps
    // measured size, which right/bottom anchoring math does not reuse.
    let cases: &[(&str, Positioned, (f32, f32))] = &[
        (
            "left_width",
            Positioned::new(white_box(10., 10.)).left(20.).width(30.),
            (20., 10.),
        ),
        (
            "right_width",
            Positioned::new(white_box(10., 10.)).right(20.).width(30.),
            (150., 10.),
        ),
        (
            "left_right",
            Positioned::new(white_box(10., 10.)).left(20.).right(30.),
            (20., 10.),
        ),
        (
            "left_right_width_bounds_measure",
            Positioned::new(white_box(10., 10.))
                .left(20.)
                .right(30.)
                .width(5.),
            (20., 5.),
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
    }
}

#[test]
fn positioned_vertical_combinations_resolve_documented() {
    let cases: &[(&str, Positioned, (f32, f32))] = &[
        (
            "top_height",
            Positioned::new(white_box(10., 10.)).top(20.).height(30.),
            (20., 10.),
        ),
        (
            "bottom_height",
            Positioned::new(white_box(10., 10.)).bottom(20.).height(30.),
            (150., 10.),
        ),
        (
            "top_bottom",
            Positioned::new(white_box(10., 10.)).top(20.).bottom(30.),
            (20., 10.),
        ),
        // top wins placement while explicit height still bounds
        // measurement even though top+bottom derive the algorithm size.
        (
            "top_wins",
            Positioned::new(white_box(10., 10.))
                .top(20.)
                .bottom(30.)
                .height(5.),
            (20., 5.),
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
    }
}

#[test]
fn positioned_negative_edges_drop_to_unset() {
    // Negative edges are not clamped to zero: they read as absent, so
    // layout falls through to the opposite edge or the origin.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Stack::new([
                Widget::from(Positioned::new(white_box(10., 10.)).left(-20.).width(30.)),
                Widget::from(Positioned::new(white_box(10., 10.)).right(20.).left(-5.)),
            ])
            .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 200., 200.);
    let kids = stack_kids(&tree, root);
    // Negative left with explicit width: width still only bounds the
    // fixed child, origin falls to 0.
    let first = tree.children(kids[0]).expect("child")[0];
    assert_eq!(bounds_of(&tree, first), (0., 0., 10., 10.));
    // Negative left with valid right: right anchors against the
    // measured width.
    let second = tree.children(kids[1]).expect("child")[0];
    assert_eq!(bounds_of(&tree, second), (170., 0., 10., 10.));
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
fn stack_text_direction_change_keeps_absolute_layout() {
    // Stack alignment factors are absolute: text direction is retained
    // but never consulted, so flipping it re-resolves identically.
    let build = |direction| {
        Widget::from(
            Stack::aligned(Alignment::CENTER, [white_box(40., 20.)]).text_direction(direction),
        )
    };
    let mut tree = WidgetTree::new();
    let root = tree.mount(build(TextDirection::Ltr)).expect("mount");
    layout_tight(&mut tree, 200., 100.);
    let before = bounds_of(&tree, stack_kids(&tree, root)[0]);
    tree.update(root, build(TextDirection::Rtl))
        .expect("flip direction");
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
