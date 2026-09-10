//! Wrap and Table layout contracts.
//!
//! Wrap flows children into runs bounded by the main-axis maximum, with
//! `spacing` inside a run and `run_spacing` between runs; alignment
//! distributes leftover main space and the vertical/text directions
//! reverse visual order. Table places row-major cells in max-content
//! columns with max-content row heights; `columns` floors at one and
//! ragged tails fill left to right. Both delegate to `incular-layout`.

use incular_config::{
    Axis, Constraints, TextDirection, VerticalDirection, WrapAlignment, WrapCrossAlignment,
};
use incular_core::{Color, Offset, Size};
use incular_semantics::SemanticRole;
use incular_widgets::{
    Semantics, Table, Widget, Wrap,
    internal::{ActionId, ElementId, WidgetTree, action},
};

fn box_(w: f32, h: f32) -> Widget {
    Widget::box_(Size::new(w, h), Color::WHITE)
}

fn keyed(w: f32, h: f32, key: u64) -> Widget {
    Widget::box_(Size::new(w, h), Color::WHITE).with_key(key)
}

fn bounds(tree: &WidgetTree, id: ElementId) -> (f32, f32, f32, f32) {
    let b = tree.element_bounds(id).expect("bounds");
    (b.origin.x, b.origin.y, b.size.width, b.size.height)
}

fn children_of(tree: &WidgetTree, root: ElementId) -> Vec<ElementId> {
    tree.children(root).expect("children").to_vec()
}

#[test]
fn wrap_flows_into_runs_with_spacing() {
    // 60px children in a 130px main axis: two per run, spacing 5.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Wrap::new([box_(60., 20.), box_(60., 20.), box_(60., 20.)])
                .spacing(5.)
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(130., 200.)))
        .expect("layout");
    assert_eq!(bounds(&tree, root), (0., 0., 125., 40.));
    let kids = children_of(&tree, root);
    assert_eq!(bounds(&tree, kids[0]), (0., 0., 60., 20.));
    assert_eq!(bounds(&tree, kids[1]), (65., 0., 60., 20.));
    assert_eq!(bounds(&tree, kids[2]), (0., 20., 60., 20.));
}

#[test]
fn wrap_run_spacing_separates_runs() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Wrap::new([box_(60., 20.), box_(60., 20.), box_(60., 20.)])
                .spacing(5.)
                .run_spacing(7.)
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(130., 200.)))
        .expect("layout");
    // Run two starts 20 + 7 lower.
    assert_eq!(bounds(&tree, children_of(&tree, root)[2]).1, 27.);
}

#[test]
fn wrap_unequal_children_size_cross_to_the_largest() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Wrap::new([box_(30., 10.), box_(40., 30.)]).into())
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    // One run whose cross extent is the tallest child.
    assert_eq!(bounds(&tree, root).3, 30.);
}

#[test]
fn wrap_main_alignment_distributes_leftover() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Wrap::new([box_(30., 10.), box_(30., 10.)])
                .alignment(WrapAlignment::SpaceBetween)
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(100., 50.)))
        .expect("layout");
    let kids = children_of(&tree, root);
    assert_eq!(bounds(&tree, kids[0]).0, 0.);
    assert_eq!(bounds(&tree, kids[1]).0, 70.);
}

#[test]
fn wrap_cross_alignment_places_unequal_heights() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Wrap::new([box_(30., 10.), box_(30., 30.)])
                .cross_axis_alignment(WrapCrossAlignment::End)
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(100., 50.)))
        .expect("layout");
    let kids = children_of(&tree, root);
    // The shorter child drops to the run's cross end.
    assert_eq!(bounds(&tree, kids[0]).1, 20.);
    assert_eq!(bounds(&tree, kids[1]).1, 0.);
}

#[test]
fn wrap_run_alignment_distributes_extra_cross_space() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Wrap::new([box_(30., 10.)])
                .run_alignment(WrapAlignment::Center)
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    // The single run centers in the 100px cross extent.
    assert_eq!(bounds(&tree, children_of(&tree, root)[0]).1, 45.);
}

#[test]
fn wrap_text_direction_reverses_visual_order() {
    let build = |direction| {
        Widget::from(Wrap::new([box_(30., 10.), box_(40., 10.)]).text_direction(direction))
    };
    let mut tree = WidgetTree::new();
    let root = tree.mount(build(TextDirection::Ltr)).expect("mount");
    tree.layout(Constraints::tight(Size::new(100., 50.)))
        .expect("layout");
    let kids = children_of(&tree, root);
    assert_eq!(bounds(&tree, kids[0]).0, 0.);
    assert_eq!(bounds(&tree, kids[1]).0, 30.);

    tree.update(root, build(TextDirection::Rtl))
        .expect("flip direction");
    tree.layout(Constraints::tight(Size::new(100., 50.)))
        .expect("layout");
    let kids = children_of(&tree, root);
    assert_eq!(bounds(&tree, kids[0]).0, 40.);
    assert_eq!(bounds(&tree, kids[1]).0, 0.);
}

#[test]
fn wrap_vertical_direction_reverses_runs() {
    // Vertical axis with Up reverses run order.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Wrap::new([box_(30., 40.), box_(30., 40.)])
                .direction(Axis::Vertical)
                .vertical_direction(VerticalDirection::Up)
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(50., 100.)))
        .expect("layout");
    let kids = children_of(&tree, root);
    assert_eq!(bounds(&tree, kids[0]).1, 40.);
    assert_eq!(bounds(&tree, kids[1]).1, 0.);
}

#[test]
fn wrap_unbounded_main_axis_keeps_one_run() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Wrap::new([box_(60., 20.), box_(60., 20.)])
                .spacing(5.)
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::unbounded()).expect("layout");
    // No wrapping without a finite main bound.
    assert_eq!(bounds(&tree, root), (0., 0., 125., 20.));
}

#[test]
fn wrap_keyed_reorder_permutes_identities() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Wrap::new([keyed(30., 10., 1), keyed(40., 10., 2), keyed(50., 10., 3)]).into())
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(200., 50.)))
        .expect("layout");
    let before = children_of(&tree, root);
    tree.update(
        root,
        Wrap::new([keyed(50., 10., 3), keyed(30., 10., 1), keyed(40., 10., 2)]).into(),
    )
    .expect("reorder");
    tree.layout(Constraints::tight(Size::new(200., 50.)))
        .expect("layout");
    let after = children_of(&tree, root);
    assert_eq!(after, vec![before[2], before[0], before[1]]);
    // Geometry follows the new registration order.
    assert_eq!(bounds(&tree, after[0]).0, 0.);
    assert_eq!(bounds(&tree, after[1]).0, 50.);
    assert_eq!(bounds(&tree, after[2]).0, 80.);
}

#[test]
fn wrap_builder_defaults_match_default_and_lowering() {
    assert_eq!(Wrap::builder().build(), Wrap::default());
    let built = Wrap::builder()
        .children(vec![box_(10., 10.)])
        .direction(Axis::Vertical)
        .spacing(3.)
        .run_spacing(4.)
        .build();
    let fluent = Wrap::new([box_(10., 10.)])
        .direction(Axis::Vertical)
        .spacing(3.)
        .run_spacing(4.);
    assert_eq!(built, fluent);
}

#[test]
fn table_max_content_columns_and_rows() {
    // Column widths are max-content; row heights are max-content.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Table::new(2, [box_(30., 10.), box_(50., 20.), box_(40., 15.)]).into())
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(300., 300.)))
        .expect("layout");
    // Column 0 is max(30, 40) = 40 from the two rows; column 1 is 50.
    assert_eq!(bounds(&tree, root), (0., 0., 90., 35.));
    let kids = children_of(&tree, root);
    assert_eq!(bounds(&tree, kids[0]), (0., 0., 30., 10.));
    assert_eq!(bounds(&tree, kids[1]), (40., 0., 50., 20.));
    assert_eq!(bounds(&tree, kids[2]), (0., 20., 40., 15.));
}

#[test]
fn table_ragged_tail_fills_left_to_right() {
    // Five cells in two columns: three rows, the last with one cell.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Table::new(
                2,
                [
                    box_(30., 10.),
                    box_(50., 20.),
                    box_(40., 15.),
                    box_(10., 10.),
                    box_(20., 5.),
                ],
            )
            .into(),
        )
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(300., 300.)))
        .expect("layout");
    let kids = children_of(&tree, root);
    assert_eq!(kids.len(), 5);
    assert_eq!(bounds(&tree, kids[4]), (0., 35., 20., 5.));
}

#[test]
fn table_columns_change_reconfigures_rows() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Table::new(2, [box_(30., 10.), box_(50., 20.)]).into())
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(300., 300.)))
        .expect("layout");
    assert_eq!(bounds(&tree, root), (0., 0., 80., 20.));

    tree.update(root, Table::new(1, [box_(30., 10.), box_(50., 20.)]).into())
        .expect("reconfigure columns");
    tree.layout(Constraints::loose(Size::new(300., 300.)))
        .expect("layout");
    // One column: two rows, width is the max cell.
    assert_eq!(bounds(&tree, root), (0., 0., 50., 30.));
    let kids = children_of(&tree, root);
    assert_eq!(bounds(&tree, kids[1]), (0., 10., 50., 20.));
}

#[test]
fn table_columns_floor_at_one_and_cells_replace() {
    // Zero columns floors to one.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Table::new(0, [box_(10., 10.), box_(20., 20.)]).into())
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(300., 300.)))
        .expect("layout");
    assert_eq!(bounds(&tree, root), (0., 0., 20., 30.));

    // Replacing cells keeps retained identities and reflows.
    let before = children_of(&tree, root);
    tree.update(
        root,
        Table::new(1, [box_(10., 10.), box_(20., 20.), box_(5., 5.)]).into(),
    )
    .expect("replace cells");
    tree.layout(Constraints::loose(Size::new(300., 300.)))
        .expect("layout");
    let after = children_of(&tree, root);
    assert_eq!(after.len(), 3);
    assert_eq!(after[0], before[0]);
    assert_eq!(after[1], before[1]);
    assert_eq!(bounds(&tree, after[2]), (0., 30., 5., 5.));
}

#[test]
fn table_empty_has_zero_size() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Table::new(3, Vec::<Widget>::new()).into())
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(300., 300.)))
        .expect("layout");
    assert_eq!(bounds(&tree, root), (0., 0., 0., 0.));
}

#[test]
fn wrap_and_table_hit_and_semantics_follow_layout() {
    // Wrap: a hit lands on the cell occupying the point.
    let mut tree = WidgetTree::new();
    tree.mount(
        Wrap::new([
            action(Size::new(60., 20.), Color::WHITE, ActionId(1)),
            action(Size::new(60., 20.), Color::WHITE, ActionId(2)),
            action(Size::new(60., 20.), Color::WHITE, ActionId(3)),
        ])
        .spacing(5.)
        .into(),
    )
    .expect("mount");
    tree.layout(Constraints::loose(Size::new(130., 200.)))
        .expect("layout");
    let hit_action = |tree: &WidgetTree, point: Offset| {
        let hit = tree.hit_test(point).expect("hit");
        tree.action_for_element(tree.element_for_render(hit).expect("element"))
    };
    assert_eq!(hit_action(&tree, Offset::new(5., 5.)), Some(ActionId(1)));
    assert_eq!(hit_action(&tree, Offset::new(70., 5.)), Some(ActionId(2)));
    assert_eq!(hit_action(&tree, Offset::new(5., 30.)), Some(ActionId(3)));

    // Table: semantic bounds follow cell geometry, including ragged rows.
    // Plain boxes emit no node, so wrap them in a semantic group.
    let cell = |w: f32, h: f32, label: &str| {
        Widget::from(
            Semantics::new(box_(w, h))
                .role(SemanticRole::Group)
                .label(label.to_owned()),
        )
    };
    let mut tree = WidgetTree::new();
    tree.mount(
        Table::new(
            2,
            [
                cell(30., 10., "a"),
                cell(50., 20., "b"),
                cell(40., 15., "c"),
            ],
        )
        .into(),
    )
    .expect("mount");
    tree.layout(Constraints::loose(Size::new(300., 300.)))
        .expect("layout");
    tree.update_semantics();
    let kids = children_of(&tree, tree.root().expect("root"));
    let third = tree.semantic_node_for_element(kids[2]).expect("node");
    let bounds = tree.semantics().node(third).expect("node").bounds;
    assert_eq!(bounds.origin, Offset::new(0., 20.));
    assert_eq!(bounds.size, Size::new(40., 15.));
}
