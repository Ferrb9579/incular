//! Row, Column, Flex, Expanded, and Flexible layout contracts.
//!
//! Flex factors floor at one everywhere (zero included); spacing is
//! nonnegative and shared before flex allocation; unbounded main axes
//! measure flex children intrinsically. Public Row/Column APIs stay
//! separate while sharing the retained Flex representation.

use incular_config::{
    Constraints, CrossAxisAlignment, MainAxisAlignment, MainAxisSize, TextDirection,
    VerticalDirection,
};
use incular_core::{Color, Offset, Size};
use incular_text::TextStyle;
use incular_widgets::{
    Column, Expanded, Flex, Flexible, Row, Spacer, Text, Widget,
    internal::{ElementId, WidgetTree},
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

fn row_kids(tree: &WidgetTree, root: ElementId) -> Vec<ElementId> {
    tree.children(root).expect("row children").to_vec()
}

fn white_box(width: f32, height: f32) -> Widget {
    Widget::box_(Size::new(width, height), Color::WHITE)
}

fn text_sized(text: &str, size: f32) -> Widget {
    Widget::from(Text::new(text).style(TextStyle {
        size,
        ..Default::default()
    }))
}

#[test]
fn finite_and_unbounded_main_axis_flex() {
    // Tight: the expanded child fills leftover space.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Row::new([
                white_box(30., 20.),
                Expanded::new(white_box(10., 10.)).into(),
            ])
            .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 200., 100.);
    let kids = row_kids(&tree, root);
    assert_eq!(bounds_of(&tree, kids[1]).2, 170.);

    // Unbounded: flex factors do not size children; everything measures
    // intrinsically and the row wraps content.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Row::new([
                white_box(30., 20.),
                Expanded::new(white_box(10., 10.)).into(),
            ])
            .into(),
        )
        .expect("mount");
    tree.layout(Constraints::new(0., f32::INFINITY, 0., 100.))
        .expect("layout");
    assert_eq!(bounds_of(&tree, root).2, 40.);
    let kids = row_kids(&tree, root);
    assert_eq!(bounds_of(&tree, kids[1]).2, 10.);
}

#[test]
fn tight_and_loose_flex_children() {
    // Fixed 10, Expanded flex 1, Flexible-loose flex 1 in 200: shares
    // are 95 each, but only the tight child is forced there while the
    // loose child keeps its intrinsic 10.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Row::new([
                white_box(10., 10.),
                Widget::from(Expanded::new(white_box(10., 10.))),
                Widget::from(Flexible::new(white_box(10., 10.))),
            ])
            .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 200., 100.);
    let kids = row_kids(&tree, root);
    assert_eq!(bounds_of(&tree, kids[0]).0, 0.);
    assert_eq!(bounds_of(&tree, kids[1]), (10., 45., 95., 10.));
    assert_eq!(bounds_of(&tree, kids[2]), (105., 45., 10., 10.));
}

#[test]
fn zero_flex_factor_behaves_as_one() {
    // Zero is floored to one at every entry point: constructor, setter,
    // conversion, and spacer agree. Tight children prove equal shares;
    // without flooring the total would be zero and nothing would fill.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Row::new([
                Widget::from(Expanded::flex_factor(0, white_box(10., 10.))),
                Widget::from(Expanded::new(white_box(10., 10.)).flex(0)),
                Widget::from(Spacer::flex_factor(0)),
            ])
            .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 300., 100.);
    let kids = row_kids(&tree, root);
    assert_eq!(kids.len(), 3);
    for kid in &kids {
        assert_eq!(bounds_of(&tree, *kid).2, 100.);
    }
    // Loose children keep intrinsic size under the same shares.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Row::new([Widget::from(Flexible::flex_factor(0, white_box(10., 10.)))]).into())
        .expect("mount");
    layout_tight(&mut tree, 300., 100.);
    let kids = row_kids(&tree, root);
    assert_eq!(bounds_of(&tree, kids[0]).2, 10.);
}

#[test]
fn spacing_combines_with_distributing_alignment() {
    // Three 20px children, spacing 10, SpaceBetween in 200: gaps carry
    // both the fixed spacing and the distributed remainder.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Row::new([
                white_box(20., 10.),
                white_box(20., 10.),
                white_box(20., 10.),
            ])
            .spacing(10.)
            .main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 200., 100.);
    let kids = row_kids(&tree, root);
    assert_eq!(bounds_of(&tree, kids[0]).0, 0.);
    assert_eq!(bounds_of(&tree, kids[1]).0, 90.);
    assert_eq!(bounds_of(&tree, kids[2]).0, 180.);

    // Center shares one leading block with spacing inside the content.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Row::new([white_box(20., 10.), white_box(20., 10.)])
                .spacing(10.)
                .main_axis_alignment(MainAxisAlignment::Center)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 200., 100.);
    let kids = row_kids(&tree, root);
    assert_eq!(bounds_of(&tree, kids[0]), (75., 45., 20., 10.));
    assert_eq!(bounds_of(&tree, kids[1]).0, 105.);
}

#[test]
fn rtl_reverses_row_visual_order() {
    let build = |direction| {
        Widget::from(Row::new([white_box(30., 10.), white_box(50., 10.)]).text_direction(direction))
    };
    let mut tree = WidgetTree::new();
    let root = tree.mount(build(TextDirection::Ltr)).expect("mount");
    layout_tight(&mut tree, 200., 100.);
    let kids = row_kids(&tree, root);
    assert_eq!(bounds_of(&tree, kids[0]).0, 0.);
    assert_eq!(bounds_of(&tree, kids[1]).0, 30.);

    // Reversal flips child order, not packing edges: alignment still
    // packs from the leading edge, so the row stays left-packed with
    // the last child first.
    tree.update(root, build(TextDirection::Rtl))
        .expect("flip direction");
    layout_tight(&mut tree, 200., 100.);
    let kids = row_kids(&tree, root);
    assert_eq!(bounds_of(&tree, kids[0]).0, 50.);
    assert_eq!(bounds_of(&tree, kids[1]).0, 0.);
}

#[test]
fn vertical_direction_reverses_column() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Column::new([white_box(10., 30.), white_box(10., 50.)])
                .vertical_direction(VerticalDirection::Up)
                .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 100., 200.);
    let kids = row_kids(&tree, root);
    assert_eq!(bounds_of(&tree, kids[0]).1, 50.);
    assert_eq!(bounds_of(&tree, kids[1]).1, 0.);
}

#[test]
fn mounted_flex_factor_change_reallocates() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Row::new([
                Widget::from(Expanded::flex_factor(1, white_box(10., 10.))),
                Widget::from(Expanded::flex_factor(1, white_box(10., 10.))),
            ])
            .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 200., 100.);
    let kids = row_kids(&tree, root);
    assert_eq!(bounds_of(&tree, kids[0]).2, 100.);

    tree.update(
        root,
        Row::new([
            Widget::from(Expanded::flex_factor(1, white_box(10., 10.))),
            Widget::from(Expanded::flex_factor(3, white_box(10., 10.))),
        ])
        .into(),
    )
    .expect("change factors");
    layout_tight(&mut tree, 200., 100.);
    let kids = row_kids(&tree, root);
    assert_eq!(bounds_of(&tree, kids[0]).2, 50.);
    assert_eq!(bounds_of(&tree, kids[1]), (50., 45., 150., 10.));

    // A fresh vertical mount at the same factors agrees exactly.
    let mut fresh = WidgetTree::new();
    fresh
        .mount(
            Flex::vertical([
                Widget::from(Expanded::flex_factor(1, white_box(10., 10.))),
                Widget::from(Expanded::flex_factor(3, white_box(10., 10.))),
            ])
            .into(),
        )
        .expect("mount");
    layout_tight(&mut fresh, 200., 200.);
    let fresh_root = fresh.root().expect("root");
    let fresh_kids = row_kids(&fresh, fresh_root);
    assert_eq!(bounds_of(&fresh, fresh_kids[1]).3, 150.);
}

#[test]
fn baseline_alignment_resolves_text_baselines() {
    // Two texts at different sizes: baseline alignment pushes the
    // smaller one down so baselines coincide instead of tops.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Row::new([text_sized("Ag", 16.), text_sized("Ag", 32.)])
                .cross_axis_alignment(CrossAxisAlignment::Baseline)
                .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 200., 100.);
    let kids = row_kids(&tree, root);
    let (_, small_y, _, _) = bounds_of(&tree, kids[0]);
    let (_, big_y, _, _) = bounds_of(&tree, kids[1]);
    assert_eq!(big_y, 0.);
    assert!(
        small_y > 5.,
        "smaller text must drop to the shared baseline, got {small_y}"
    );

    // Start alignment keeps both tops at zero for contrast.
    let mut tree = WidgetTree::new();
    tree.mount(
        Row::new([text_sized("Ag", 16.), text_sized("Ag", 32.)])
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .into(),
    )
    .expect("mount");
    layout_tight(&mut tree, 200., 100.);
    let root = tree.root().expect("root");
    let kids = row_kids(&tree, root);
    assert_eq!(bounds_of(&tree, kids[0]).1, 0.);
    assert_eq!(bounds_of(&tree, kids[1]).1, 0.);
}

#[test]
fn overflow_hit_and_semantics_follow_layout() {
    // Two 80px boxes in a 100px row: the second overflows. Hits inside
    // the parent reach it; hits past the parent bounds cannot, because
    // ancestors reject outside points before descending.
    let mut tree = WidgetTree::new();
    tree.mount(Row::new([white_box(80., 40.), white_box(80., 40.)]).into())
        .expect("mount");
    layout_tight(&mut tree, 100., 100.);
    let root = tree.root().expect("root");
    let kids = row_kids(&tree, root);
    assert_eq!(bounds_of(&tree, kids[1]).0, 80.);
    assert!(tree.hit_test(Offset::new(90., 10.)).is_some());
    assert!(tree.hit_test(Offset::new(150., 10.)).is_none());

    // Semantics follow laid-out bounds, which may exceed the parent:
    // an unwrapped overflowing label keeps a node past the edge.
    let mut tree = WidgetTree::new();
    tree.mount(
        Row::new([Widget::from(
            Text::new("overflowing label text here").soft_wrap(false),
        )])
        .into(),
    )
    .expect("mount");
    layout_tight(&mut tree, 100., 100.);
    tree.update_semantics();
    let dump = tree.semantics_debug_dump();
    assert!(dump.contains("overflowing"), "label present:\n{dump}");
    let root = tree.root().expect("root");
    let label = row_kids(&tree, root)[0];
    let node = tree.semantic_node_for_element(label).expect("node");
    let bounds = tree.semantics().node(node).expect("node").bounds;
    assert!(
        bounds.origin.x + bounds.size.width > 100.,
        "semantic bounds exceed the parent: {bounds:?}"
    );
}

#[test]
fn main_axis_size_min_wraps_content() {
    // Min sizes to content, but only where constraints allow: loose
    // maxima wrap while tight minima still fill.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Row::new([white_box(30., 10.), white_box(50., 10.)])
                .main_axis_size(MainAxisSize::Min)
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 100.)))
        .expect("layout");
    assert_eq!(bounds_of(&tree, root).2, 80.);
}

#[test]
fn row_column_share_the_flex_representation() {
    // Public Row/Column APIs stay separate, but both lower to the same
    // retained kind with identical defaults: no divergent copies.
    let row = Widget::from(Row::new([white_box(10., 10.)]));
    let flex = Widget::from(Flex::horizontal([white_box(10., 10.)]));
    assert_eq!(row, flex);
    let column = Widget::from(Column::new([white_box(10., 10.)]));
    let flex_vertical = Widget::from(Flex::vertical([white_box(10., 10.)]));
    assert_eq!(column, flex_vertical);
}

#[test]
fn flex_identical_reapplication_bails_out() {
    let mut tree = WidgetTree::new();
    let row = Widget::from(Row::new([white_box(30., 10.)]));
    let root = tree.mount(row.clone()).expect("mount");
    layout_tight(&mut tree, 200., 100.);
    let _ = tree.paint();
    let before = tree.diagnostics();
    tree.update(root, row).expect("identical reapply");
    layout_tight(&mut tree, 200., 100.);
    let _ = tree.paint();
    let after = tree.diagnostics();
    assert!(after.identical_child_bailouts > before.identical_child_bailouts);
    assert_eq!(after.layouts, before.layouts);
    assert_eq!(after.paints, before.paints);
    assert_eq!(after.composites, before.composites);
}

#[test]
fn flex_builder_matches_fluent_construction() {
    let fluent = Row::new([white_box(10., 10.)])
        .main_axis_alignment(MainAxisAlignment::Center)
        .spacing(8.);
    let built = Row::builder()
        .children(vec![white_box(10., 10.)])
        .main_axis_alignment(MainAxisAlignment::Center)
        .spacing(8.)
        .build();
    assert_eq!(Widget::from(fluent), Widget::from(built));
}
