//! Viewport-configuration and ordinary-list-scrolling coverage.
//!
//! Initial scrolling ledger group: `Scrollable`, `Viewport`,
//! `CustomScrollView`, `ListView`, `SliverList`, and the `restored`
//! `ScrollController` attachment. Grids, pages, animated lists,
//! two-dimensional views, and physics internals belong to later groups.
//! Offset ownership stays with `ScrollController` (`incular-scroll`);
//! retained layout stays in Widgets.

mod common;

use common::*;
use incular_config::{Axis, Constraints, EdgeInsets};
use incular_core::{Color, Offset, Size};
use incular_scroll::{ScrollController, ScrollPhysics};
use incular_semantics::SemanticRole;
use incular_widgets::internal::TreeError;
use incular_widgets::{
    Column, DraggableScrollableSheet, ListView, ListWheelScrollView, ListWheelViewport,
    RawScrollbar, Scrollable, Semantics, SingleChildScrollView, TwoDimensionalChildDelegate,
    TwoDimensionalScrollable, TwoDimensionalViewport, Viewport, WheelChildDelegate,
};
use std::time::Instant;

fn rows(count: usize) -> Vec<Widget> {
    (0..count)
        .map(|_| Widget::box_(Size::new(80., 40.), Color::WHITE))
        .collect()
}

fn slivers(count: usize) -> Vec<Box<dyn Sliver>> {
    vec![Box::new(SliverList::builder(count, |_| {
        Widget::box_(Size::new(80., 40.), Color::WHITE)
    })) as Box<dyn Sliver>]
}

fn subtree_geometry(
    tree: &WidgetTree,
    root: incular_widgets::internal::ElementId,
) -> Vec<(Offset, Size)> {
    let mut geometry = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let rect = tree.element_bounds(id).expect("element has bounds");
        geometry.push((rect.origin, rect.size));
        if let Some(kids) = tree.children(id) {
            stack.extend(kids.iter().rev());
        }
    }
    geometry
}

/// Extracts the reported y origin of each labeled group node, in dump
/// order. (The viewport contributes its own unlabeled node, which is
/// covered by the line-count stability check instead.)
fn semantic_ys(dump: &str) -> Vec<f32> {
    dump.lines()
        .filter(|line| line.contains("Group#"))
        .map(|line| {
            let bounds = line
                .split("bounds=(")
                .nth(1)
                .expect("semantic line carries bounds");
            bounds
                .split(',')
                .nth(1)
                .expect("bounds carry y")
                .parse()
                .expect("numeric y")
        })
        .collect()
}

fn mount_tight(
    tree: &mut WidgetTree,
    widget: Widget,
    w: f32,
    h: f32,
) -> incular_widgets::internal::ElementId {
    let root = tree.mount(widget).expect("mount");
    tree.layout(Constraints::tight(Size::new(w, h)))
        .expect("layout");
    root
}

#[test]
fn viewport_builder_defaults_match_new() {
    let mut via_builder = WidgetTree::new();
    let builder_root = mount_tight(
        &mut via_builder,
        Viewport::builder().slivers(slivers(3)).build().into(),
        200.,
        200.,
    );
    let mut via_ctor = WidgetTree::new();
    let ctor_root = mount_tight(&mut via_ctor, Viewport::new(slivers(3)).into(), 200., 200.);
    assert_eq!(
        subtree_geometry(&via_builder, builder_root),
        subtree_geometry(&via_ctor, ctor_root)
    );
}

#[test]
fn custom_scroll_view_builder_defaults_match_new() {
    let mut via_builder = WidgetTree::new();
    let builder_root = mount_tight(
        &mut via_builder,
        CustomScrollView::builder()
            .slivers(slivers(3))
            .build()
            .into(),
        200.,
        200.,
    );
    let mut via_ctor = WidgetTree::new();
    let ctor_root = mount_tight(
        &mut via_ctor,
        CustomScrollView::new(slivers(3)).into(),
        200.,
        200.,
    );
    assert_eq!(
        subtree_geometry(&via_builder, builder_root),
        subtree_geometry(&via_ctor, ctor_root)
    );
}

#[test]
fn list_view_builder_defaults_match_new() {
    // ListView carries an Rc strategy/callbacks so it has no structural
    // equality; parity is proven geometrically.
    let mut via_builder = WidgetTree::new();
    let builder_root = mount_tight(
        &mut via_builder,
        ListView::typed_builder().children(rows(3)).build().into(),
        200.,
        200.,
    );
    let mut via_ctor = WidgetTree::new();
    let ctor_root = mount_tight(&mut via_ctor, ListView::new(rows(3)).into(), 200., 200.);
    assert_eq!(
        subtree_geometry(&via_builder, builder_root),
        subtree_geometry(&via_ctor, ctor_root)
    );
}

#[test]
fn scrollable_builds_viewport_from_builder() {
    // The builder receives the resolved controller: jumping it scrolls the
    // built viewport, whether the controller is supplied or defaulted.
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        Scrollable::new(|controller: &ScrollController| {
            Viewport::new(slivers(5)).controller(controller.clone())
        })
        .controller(controller.clone())
        .into(),
        100.,
        100.,
    );
    let origin_of = |tree: &WidgetTree, kid: incular_widgets::internal::ElementId| {
        tree.render_origin(tree.render_id(kid).expect("render"))
    };
    let kids = tree.children(root).expect("items").to_vec();
    assert_eq!(origin_of(&tree, kids[0]), Offset::new(0., 0.));
    // Variable-extent estimates make the initial range approximate, but it
    // always covers at least the first measured item.
    assert!(controller.max_offset() >= 40.);
    assert!(controller.jump_to(40.));
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    // The scrolled-out item releases while the retained second item moves
    // by exactly the offset: the builder, controller, and viewport agree.
    assert_eq!(controller.offset(), 40.);
    assert!(!tree.element_exists(kids[0]));
    assert!(tree.element_exists(kids[1]));
    assert_eq!(origin_of(&tree, kids[1]), Offset::new(0., 0.));
}

#[test]
fn viewport_axis_direction_lays_out_slivers() {
    for (axis, first_size, second_origin) in [
        (Axis::Vertical, Size::new(200., 40.), Offset::new(0., 40.)),
        (Axis::Horizontal, Size::new(80., 200.), Offset::new(80., 0.)),
    ] {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        let root = mount_tight(
            &mut tree,
            Viewport::new(slivers(3))
                .axis_direction(axis)
                .controller(controller.clone())
                .into(),
            200.,
            200.,
        );
        let kids = tree.children(root).expect("items").to_vec();
        assert_eq!(kids.len(), 3);
        let first = tree.element_bounds(kids[0]).expect("bounds");
        let second = tree.element_bounds(kids[1]).expect("bounds");
        assert_eq!(first.size, first_size);
        assert_eq!(second.origin, second_origin);
        // The attached controller drives the viewport it was given.
        let max = controller.max_offset();
        assert!(max >= 0.);
        if max > 0. {
            assert!(controller.jump_to(max));
            tree.layout(Constraints::tight(Size::new(200., 200.)))
                .expect("layout");
            let shifted = tree.element_bounds(kids[0]).expect("bounds");
            let delta = if axis == Axis::Vertical {
                Offset::new(0., -max)
            } else {
                Offset::new(-max, 0.)
            };
            assert_eq!(shifted.origin, first.origin + delta);
        }
    }
}

#[test]
fn list_view_axis_change_reflows_children() {
    let mut tree = WidgetTree::new();
    let root = mount_tight(&mut tree, ListView::new(rows(3)).into(), 200., 200.);
    let vertical: Vec<(Offset, Size)> = tree
        .children(root)
        .expect("items")
        .iter()
        .map(|kid| {
            let rect = tree.element_bounds(*kid).expect("bounds");
            (rect.origin, rect.size)
        })
        .collect();
    assert_eq!(
        vertical,
        vec![
            (Offset::new(0., 0.), Size::new(200., 40.)),
            (Offset::new(0., 40.), Size::new(200., 40.)),
            (Offset::new(0., 80.), Size::new(200., 40.)),
        ]
    );
    tree.update(
        root,
        ListView::new(rows(3))
            .scroll_direction(Axis::Horizontal)
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let horizontal: Vec<(Offset, Size)> = tree
        .children(root)
        .expect("items")
        .iter()
        .map(|kid| {
            let rect = tree.element_bounds(*kid).expect("bounds");
            (rect.origin, rect.size)
        })
        .collect();
    assert_eq!(
        horizontal,
        vec![
            (Offset::new(0., 0.), Size::new(80., 200.)),
            (Offset::new(80., 0.), Size::new(80., 200.)),
            (Offset::new(160., 0.), Size::new(80., 200.)),
        ]
    );
}

#[test]
fn list_view_reverse_change_moves_first_child_to_trailing_edge() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        ListView::builder(5, |_| Widget::box_(Size::new(80., 40.), Color::WHITE))
            .item_extent(40.)
            .controller(controller.clone())
            .into(),
        100.,
        100.,
    );
    let kids = tree.children(root).expect("items").to_vec();
    assert_eq!(kids.len(), 5);
    assert_eq!(
        tree.element_bounds(kids[0]).expect("bounds").origin,
        Offset::new(0., 0.)
    );
    tree.update(
        root,
        ListView::builder(5, |_| Widget::box_(Size::new(80., 40.), Color::WHITE))
            .item_extent(40.)
            .reverse(true)
            .controller(controller.clone())
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    // Logical offset zero maps to the physical end for reverse views.
    assert_eq!(
        tree.element_bounds(kids[0]).expect("bounds").origin,
        Offset::new(0., -100.)
    );
    assert!(controller.jump_to(100.));
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert_eq!(
        tree.element_bounds(kids[0]).expect("bounds").origin,
        Offset::new(0., 0.)
    );
}

#[test]
fn list_view_padding_insets_content() {
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        ListView::new(rows(2)).padding(EdgeInsets::all(10.)).into(),
        200.,
        200.,
    );
    let kids = tree.children(root).expect("items").to_vec();
    assert_eq!(kids.len(), 2);
    assert_eq!(
        tree.element_bounds(kids[0]).expect("bounds").origin,
        Offset::new(10., 10.)
    );
    assert_eq!(
        tree.element_bounds(kids[1]).expect("bounds").origin,
        Offset::new(10., 50.)
    );
}

#[test]
fn list_view_shrink_wrap_transition_resizes_to_content() {
    let mut tree = WidgetTree::new();
    let root = tree.mount(ListView::new(rows(3)).into()).expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 300.)))
        .expect("layout");
    let before = tree.element_bounds(root).expect("bounds").size;
    assert_eq!(before, Size::new(200., 300.));
    let kids_before = tree.children(root).expect("items").to_vec();
    tree.update(root, ListView::new(rows(3)).shrink_wrap(true).into())
        .expect("update");
    tree.layout(Constraints::loose(Size::new(200., 300.)))
        .expect("layout");
    let after = tree.element_bounds(root).expect("bounds").size;
    assert_eq!(after, Size::new(200., 120.));
    // The transition re-resolves size without rebuilding the items.
    assert_eq!(tree.children(root).expect("items").to_vec(), kids_before);
}

#[test]
fn list_view_controller_replacement_isolates_old_controller() {
    let first = ScrollController::new();
    let second = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        ListView::new(rows(3)).controller(first.clone()).into(),
        100.,
        100.,
    );
    assert_eq!(first.max_offset(), 20.);
    assert!(first.jump_to(20.));
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    let kids = tree.children(root).expect("items").to_vec();
    let driven: Vec<Offset> = kids
        .iter()
        .map(|kid| tree.element_bounds(*kid).expect("bounds").origin)
        .collect();

    // Swap attachment: the new controller starts at zero and drives the
    // viewport; the old one keeps its own offset record in isolation.
    tree.update(
        root,
        ListView::new(rows(3)).controller(second.clone()).into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert_eq!(second.offset(), 0.);
    assert!(second.jump_to(20.));
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert!(first.jump_to(0.));
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert_eq!(first.offset(), 0.);
    assert_eq!(second.offset(), 20.);
    // The viewport still shows the attached controller's position.
    let shown: Vec<Offset> = tree
        .children(root)
        .expect("items")
        .iter()
        .map(|kid| tree.element_bounds(*kid).expect("bounds").origin)
        .collect();
    assert_eq!(shown, driven);
    assert_eq!(tree.children(root).expect("items").to_vec(), kids);
}

#[test]
fn list_view_physics_replacement_changes_input() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        ListView::new(rows(5))
            .physics(ScrollPhysics::clamping())
            .controller(controller.clone())
            .into(),
        100.,
        100.,
    );
    assert!(tree.scroll_at(Offset::new(50., 50.), Offset::new(0., 20.)));
    assert_eq!(controller.offset(), 20.);
    // A never-scrollable replacement rejects gesture input while the
    // programmatic position stays put.
    tree.update(
        root,
        ListView::new(rows(5))
            .physics(ScrollPhysics::clamping().never_scrollable())
            .controller(controller.clone())
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert!(!tree.scroll_at(Offset::new(50., 50.), Offset::new(0., 20.)));
    assert_eq!(controller.offset(), 20.);
    // Restoring a scrolling physics re-enables input on the same viewport.
    tree.update(
        root,
        ListView::new(rows(5))
            .physics(ScrollPhysics::clamping())
            .controller(controller.clone())
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert!(tree.scroll_at(Offset::new(50., 50.), Offset::new(0., 20.)));
    assert_eq!(controller.offset(), 40.);
}

#[test]
fn list_view_cache_extent_widens_materialized_window() {
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        ListView::builder(100, |_| Widget::box_(Size::new(80., 40.), Color::WHITE))
            .cache_extent(0.)
            .into(),
        100.,
        100.,
    );
    assert_eq!(tree.children(root).expect("items").len(), 3);
    tree.update(
        root,
        ListView::builder(100, |_| Widget::box_(Size::new(80., 40.), Color::WHITE))
            .cache_extent(500.)
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert_eq!(tree.children(root).expect("items").len(), 16);
    let unmounted = tree.diagnostics().items_unmounted;
    tree.update(
        root,
        ListView::builder(100, |_| Widget::box_(Size::new(80., 40.), Color::WHITE))
            .cache_extent(0.)
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert_eq!(tree.children(root).expect("items").len(), 3);
    assert!(tree.diagnostics().items_unmounted > unmounted);
}

#[test]
fn custom_scroll_view_cache_extent_changes_materialized_window() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        CustomScrollView::new(slivers(100))
            .controller(controller.clone())
            .cache_extent(0.)
            .into(),
        100.,
        100.,
    );
    let diagnostics = tree.sliver_viewport_diagnostics().expect("diagnostics");
    assert_eq!(diagnostics.materialized_item_count, 3);
    tree.update(
        root,
        CustomScrollView::new(slivers(100))
            .controller(controller.clone())
            .cache_extent(500.)
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    let diagnostics = tree.sliver_viewport_diagnostics().expect("diagnostics");
    assert_eq!(diagnostics.materialized_item_count, 16);
    assert!(diagnostics.materialized_range.contains(&15));
    // Shrinking the window releases the extra children.
    let unmounted = tree.diagnostics().items_unmounted;
    tree.update(
        root,
        CustomScrollView::new(slivers(100))
            .controller(controller.clone())
            .cache_extent(0.)
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    let diagnostics = tree.sliver_viewport_diagnostics().expect("diagnostics");
    assert_eq!(diagnostics.materialized_item_count, 3);
    assert!(tree.diagnostics().items_unmounted > unmounted);
}

#[test]
fn custom_scroll_view_horizontal_lays_columns_and_physics_gates_input() {
    // Horizontal lays columns; never-scrollable physics rejects gesture
    // input while programmatic jumps still apply.
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        CustomScrollView::new(slivers(3))
            .scroll_direction(Axis::Horizontal)
            .physics(ScrollPhysics::clamping().never_scrollable())
            .controller(controller.clone())
            .into(),
        200.,
        200.,
    );
    let kids = tree.children(root).expect("items").to_vec();
    assert_eq!(kids.len(), 3);
    assert_eq!(
        tree.element_bounds(kids[1]).expect("bounds").origin,
        Offset::new(80., 0.)
    );
    assert!(!tree.scroll_at(Offset::new(100., 100.), Offset::new(20., 0.)));
    assert_eq!(controller.offset(), 0.);
    assert!(controller.jump_to(40.));
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(
        tree.render_origin(tree.render_id(kids[0]).expect("render")),
        Offset::new(-40., 0.)
    );
}

#[test]
fn custom_scroll_view_reverse_anchors_trailing_edge() {
    // Reverse anchoring is observable only with overflowing content: five
    // 40px rows in a 100px viewport start the first child at -100.
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        CustomScrollView::new(slivers(5)).reverse(true).into(),
        100.,
        100.,
    );
    let kids = tree.children(root).expect("items").to_vec();
    assert_eq!(kids.len(), 5);
    assert_eq!(
        tree.element_bounds(kids[0]).expect("bounds").origin,
        Offset::new(0., -100.)
    );
    assert_eq!(
        tree.element_bounds(kids[4]).expect("bounds").origin,
        Offset::new(0., 60.)
    );
}

#[test]
fn custom_scroll_view_viewport_resize_retains_children_and_offset() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        CustomScrollView::new(slivers(5))
            .controller(controller.clone())
            .into(),
        100.,
        100.,
    );
    assert_eq!(controller.max_offset(), 100.);
    assert!(controller.jump_to(60.));
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    let kids = tree.children(root).expect("items").to_vec();
    // Shrinking the viewport keeps every retained child, the offset, and
    // widens the scroll range instead of dropping state.
    tree.layout(Constraints::tight(Size::new(100., 80.)))
        .expect("layout");
    assert_eq!(controller.offset(), 60.);
    assert_eq!(controller.max_offset(), 120.);
    for kid in &kids {
        assert!(tree.element_exists(*kid));
    }
    assert_eq!(
        tree.element_bounds(kids[0]).expect("bounds").origin,
        Offset::new(0., -60.)
    );
}

#[test]
fn list_view_separated_interleaves_items_and_separators() {
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        ListView::separated(
            3,
            |_| Widget::box_(Size::new(80., 40.), Color::WHITE),
            |_| Widget::box_(Size::new(80., 10.), Color::WHITE),
        )
        .into(),
        200.,
        300.,
    );
    let kids = tree.children(root).expect("items").to_vec();
    assert_eq!(kids.len(), 5);
    let geometry: Vec<(Offset, Size)> = kids
        .iter()
        .map(|kid| {
            let rect = tree.element_bounds(*kid).expect("bounds");
            (rect.origin, rect.size)
        })
        .collect();
    assert_eq!(
        geometry,
        vec![
            (Offset::new(0., 0.), Size::new(200., 40.)),
            (Offset::new(0., 40.), Size::new(200., 10.)),
            (Offset::new(0., 50.), Size::new(200., 40.)),
            (Offset::new(0., 90.), Size::new(200., 10.)),
            (Offset::new(0., 100.), Size::new(200., 40.)),
        ]
    );
}

#[test]
fn list_view_prototype_item_measures_children() {
    // Items intrinsically 20px tall measure at the 40px prototype instead.
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        ListView::builder(3, |_| Widget::box_(Size::new(80., 20.), Color::WHITE))
            .prototype_item(Widget::box_(Size::new(80., 40.), Color::WHITE))
            .into(),
        200.,
        300.,
    );
    let kids = tree.children(root).expect("items").to_vec();
    assert_eq!(kids.len(), 3);
    for (index, kid) in kids.iter().enumerate() {
        let rect = tree.element_bounds(*kid).expect("bounds");
        assert_eq!(rect.origin, Offset::new(0., index as f32 * 40.));
        assert_eq!(rect.size, Size::new(200., 40.));
    }
}

#[test]
fn list_view_item_extent_builder_makes_metrics_exact() {
    // Alternating 30/50 extents total 4000 over 100 items. The builder
    // seeds the retained extent index, so the scroll range is exact (4000
    // minus the 100px viewport); default estimates overshoot it.
    let exact_controller = ScrollController::new();
    let mut exact = WidgetTree::new();
    mount_tight(
        &mut exact,
        ListView::builder(100, |i| {
            Widget::box_(
                Size::new(80., if i % 2 == 0 { 30. } else { 50. }),
                Color::WHITE,
            )
        })
        .item_extent_builder(|i| if i % 2 == 0 { 30. } else { 50. })
        .controller(exact_controller.clone())
        .into(),
        100.,
        100.,
    );
    assert_eq!(exact_controller.max_offset(), 3900.);
    let mut estimated = WidgetTree::new();
    let estimated_controller = ScrollController::new();
    mount_tight(
        &mut estimated,
        ListView::builder(100, |i| {
            Widget::box_(
                Size::new(80., if i % 2 == 0 { 30. } else { 50. }),
                Color::WHITE,
            )
        })
        .controller(estimated_controller.clone())
        .into(),
        100.,
        100.,
    );
    assert_ne!(
        estimated_controller.max_offset(),
        3900.,
        "default estimates must not already be exact"
    );
}

#[test]
fn scroll_paint_hit_semantics_agree_without_repaint() {
    static LABELS: [&str; 3] = ["alpha", "beta", "gamma"];
    let labels = &LABELS;
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        ListView::builder(3, |index| {
            Semantics::new(Widget::box_(Size::new(80., 40.), Color::WHITE))
                .role(SemanticRole::Group)
                .label(labels[index])
        })
        .controller(controller.clone())
        .into(),
        100.,
        100.,
    );
    tree.update_compositor(Instant::now())
        .expect("compositor update");
    // Scroll translation rides a paint transform, so origins accumulate it
    // like the shared `rect_origins` helper.
    let paint_before: Vec<Offset> = rect_origins(&tree.paint())
        .into_iter()
        .filter(|origin| origin.x < 80.)
        .collect();
    tree.update_semantics();
    let dump_before = tree.semantics_debug_dump();
    for label in labels {
        assert!(
            dump_before.contains(label),
            "missing {label}:\n{dump_before}"
        );
    }
    let lines_before = dump_before.lines().count();
    let elements: Vec<incular_widgets::internal::ElementId> =
        tree.children(root).expect("items").to_vec();
    assert_eq!(elements.len(), 3);
    let hit = |tree: &WidgetTree, point: Offset| {
        tree.hit_test(point)
            .and_then(|render| tree.element_for_render(render))
    };
    // Before scrolling, (40, 30) lands on the first box and (40, 70) on
    // the second.
    assert_eq!(hit(&tree, Offset::new(40., 30.)), Some(elements[0]));
    assert_eq!(hit(&tree, Offset::new(40., 70.)), Some(elements[1]));

    // Scroll half an item: paint shifts by exactly the offset, hits follow
    // the same elements, and semantics keep every node.
    let diagnostics_before = tree.diagnostics();
    assert!(controller.jump_to(20.));
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    tree.update_compositor(Instant::now())
        .expect("compositor update");
    let paint_after: Vec<Offset> = rect_origins(&tree.paint())
        .into_iter()
        .filter(|origin| origin.x < 80.)
        .collect();
    assert_eq!(
        paint_after,
        paint_before
            .iter()
            .map(|origin| *origin + Offset::new(0., -20.))
            .collect::<Vec<_>>()
    );
    assert_eq!(hit(&tree, Offset::new(40., 10.)), Some(elements[0]));
    assert_eq!(hit(&tree, Offset::new(40., 50.)), Some(elements[1]));
    tree.update_semantics();
    let dump_after = tree.semantics_debug_dump();
    assert_eq!(dump_after.lines().count(), lines_before);
    // The viewport node itself reports the scrolled position.
    assert!(
        dump_before.contains("value=Some(\"0/20\")"),
        ":\n{dump_before}"
    );
    assert!(
        dump_after.contains("value=Some(\"20/20\")"),
        ":\n{dump_after}"
    );
    // Every node survives with its label, and each reported y shifts by
    // exactly the scrolled offset.
    assert_eq!(semantic_ys(&dump_before), vec![0., 40., 80.]);
    assert_eq!(semantic_ys(&dump_after), vec![-20., 20., 60.]);
    for label in labels {
        assert!(dump_after.contains(label), "missing {label}:\n{dump_after}");
    }
    // Retained pictures replay instead of rebuilding items.
    let diagnostics_after = tree.diagnostics();
    assert!(diagnostics_after.display_lists_reused > diagnostics_before.display_lists_reused);
    assert_eq!(
        diagnostics_after.items_built,
        diagnostics_before.items_built
    );
    assert_eq!(
        diagnostics_after.items_mounted,
        diagnostics_before.items_mounted
    );
}

#[test]
fn duplicate_viewport_attachment_is_rejected() {
    // W5.1 contract: one live viewport per ordinary controller. The
    // second viewport fails at layout before overwriting anything — with
    // both viewport identities — while the first keeps its geometry and
    // stays programmatically drivable.
    let controller = ScrollController::new();
    // V1: 100px viewport over 300px content (own max 200). V2: 150px
    // viewport over 500px content (own max 350).
    let scrolled1: Widget =
        SingleChildScrollView::new(Widget::box_(Size::new(200., 300.), Color::WHITE))
            .controller(controller.clone())
            .into();
    let v1: Widget = SizedBox::from_dimensions(Some(200.), Some(100.), Some(scrolled1)).into();
    let scrolled2: Widget =
        SingleChildScrollView::new(Widget::box_(Size::new(200., 500.), Color::BLACK))
            .controller(controller.clone())
            .into();
    let v2: Widget = SizedBox::from_dimensions(Some(200.), Some(150.), Some(scrolled2)).into();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Column::new(vec![v1, v2]).into())
        .expect("mount defers attachment");
    let error = tree
        .layout(Constraints::tight(Size::new(200., 300.)))
        .unwrap_err();
    // Both viewport identities, with the first viewport owning.
    let kids = tree.children(root).expect("viewports").to_vec();
    assert_eq!(kids.len(), 2);
    let viewport_of =
        |sized: incular_widgets::internal::ElementId| tree.children(sized).expect("viewport")[0];
    match error {
        TreeError::DuplicateScrollAttachment {
            owner_tree,
            owner,
            attempted,
        } => {
            assert_eq!(owner_tree, tree.tree_id());
            assert_eq!(owner, Some(viewport_of(kids[0])));
            assert_eq!(attempted, viewport_of(kids[1]));
        }
        other => panic!("unexpected failure: {other:?}"),
    }
    // The first viewport laid out first and keeps its geometry exactly;
    // the rejected second wrote nothing.
    assert_eq!(controller.content_extent(), 300.);
    assert_eq!(controller.viewport_extent(), 100.);
    assert_eq!(controller.max_offset(), 200.);
    // Programmatic use of the shared handle still works (clones are not
    // attachments).
    assert!(controller.jump_to(200.));
    assert_eq!(controller.offset(), 200.);
}

#[test]
fn unmount_ends_open_activity() {
    // App-driven begin, then viewport detach: End fires on unmount, and
    // the next begin starts fresh — no stuck flag swallowing Starts.
    let controller = ScrollController::new();
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let _subscription = controller.add_listener({
        let events = events.clone();
        move |notification| {
            events.borrow_mut().push(notification.kind);
            false
        }
    });
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    assert!(controller.begin_activity());
    tree.update(root, Column::new(Vec::<Widget>::new()).into())
        .expect("unmount");
    tree.layout(Constraints::tight(Size::new(200., 100.)))
        .expect("layout");
    assert_eq!(
        events.borrow().as_slice(),
        &[
            incular_scroll::ScrollNotificationType::Metrics,
            incular_scroll::ScrollNotificationType::Start,
            incular_scroll::ScrollNotificationType::End,
        ]
    );
    // Fresh start afterwards: the flag did not linger.
    assert!(controller.begin_activity());
    assert!(controller.end_activity());
    assert_eq!(events.borrow().len(), 5);
}

#[test]
fn reentrant_begin_during_detach_end_survives() {
    // Release runs before callbacks: a listener observing End sees no
    // owner (`metric_owner` already cleared) and may begin a new
    // activity on the spot — the trailing cleanup cancels nothing new.
    // Exact sequence: Start, End, then the nested Start.
    let controller = ScrollController::new();
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let saw_free = std::rc::Rc::new(std::cell::Cell::new(false));
    let events_for_listener = events.clone();
    let controller_for_listener = controller.clone();
    let saw_free_for_listener = saw_free.clone();
    let _subscription = controller.add_listener(move |notification| {
        use incular_scroll::ScrollNotificationType::End;
        events_for_listener.borrow_mut().push(notification.kind);
        if notification.kind == End {
            saw_free_for_listener.set(controller_for_listener.metric_owner().is_none());
            assert!(controller_for_listener.begin_activity());
        }
        false
    });
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    assert!(controller.begin_activity());
    tree.update(root, Column::new(Vec::<Widget>::new()).into())
        .expect("unmount");
    tree.layout(Constraints::tight(Size::new(200., 100.)))
        .expect("layout");
    use incular_scroll::ScrollNotificationType::{End, Start};
    let framed: Vec<incular_scroll::ScrollNotificationType> = events
        .borrow()
        .iter()
        .copied()
        .filter(|kind| matches!(kind, Start | End))
        .collect();
    assert_eq!(framed.as_slice(), &[Start, End, Start]);
    assert!(saw_free.get(), "ownership released before callbacks");
    assert!(
        !controller.begin_activity(),
        "new activity survives cleanup"
    );
    assert!(controller.end_activity());
}

#[test]
fn detaching_end_listener_can_reattach_and_begin_fresh() {
    // Ownership commits before End dispatches: a listener observes a
    // free controller, attaches it, and begins a fresh activity — and
    // that activity survives the rest of detach.
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let tree_id = tree.tree_id();
    let reattached = std::rc::Rc::new(std::cell::RefCell::new(None));
    let reattached_for_listener = reattached.clone();
    let controller_for_listener = controller.clone();
    let _subscription = controller.add_listener(move |notification| {
        if notification.kind == incular_scroll::ScrollNotificationType::End
            && reattached_for_listener.borrow().is_none()
        {
            let handle = controller_for_listener
                .try_attach(incular_scroll::MetricOwner::of_tree(tree_id))
                .expect("ownership released before End");
            assert!(controller_for_listener.begin_activity());
            *reattached_for_listener.borrow_mut() = Some(handle);
        }
        false
    });
    let root = mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    assert!(controller.begin_activity());
    tree.update(root, Column::new(Vec::<Widget>::new()).into())
        .expect("unmount");
    tree.layout(Constraints::tight(Size::new(200., 100.)))
        .expect("layout");
    let handle = reattached
        .borrow_mut()
        .take()
        .expect("listener reattached during End");
    assert_eq!(controller.metric_owner(), Some(tree_id));
    assert_eq!(controller.attachment_id(), Some(handle.id()));
    assert!(
        !controller.begin_activity(),
        "listener-started activity survives detach"
    );
    // Unsubscribe before cleanup: ending the fresh activity would
    // otherwise re-fire the listener, which is exactly what the guard
    // above no longer suppresses once the slot is taken.
    drop(_subscription);
    assert!(handle.release());
    assert!(controller.end_activity());
    assert_eq!(controller.metric_owner(), None);
}

#[test]
fn unwinding_through_detach_releases_remaining_leases_silently() {
    // A listener panicking during the first viewport's End aborts the
    // update midway — yet nothing leaks and nothing double-frees.
    // Ownership already committed before the callback, so the first
    // controller is free despite the unwind; the never-reached second
    // viewport stays mounted and owned with its activity open; dropping
    // the tree then tears its lease down silently.
    let first = ScrollController::new();
    let second = ScrollController::new();
    let first_ends = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let second_ends = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let counted_first = first_ends.clone();
    let counted_second = second_ends.clone();
    let _first_subscription = first.add_listener(move |notification| {
        if notification.kind == incular_scroll::ScrollNotificationType::End {
            counted_first.set(counted_first.get() + 1);
            panic!("listener unwinds through detach");
        }
        false
    });
    let _second_subscription = second.add_listener(move |notification| {
        if notification.kind == incular_scroll::ScrollNotificationType::End {
            counted_second.set(counted_second.get() + 1);
        }
        false
    });
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        Column::new(vec![
            sized_viewport(first.clone(), 200., 100., 300.),
            sized_viewport(second.clone(), 200., 100., 300.),
        ])
        .into(),
        200.,
        300.,
    );
    assert!(first.begin_activity());
    assert!(second.begin_activity());
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        tree.update(root, Column::new(Vec::<Widget>::new()).into())
            .expect("unmount");
    }));
    assert!(outcome.is_err(), "listener panic propagates");
    // The first release committed before its End dispatched: free
    // despite the unwind, with exactly one End delivered.
    assert_eq!(
        (first.metric_owner(), first.attachment_id()),
        (None, None),
        "first state after unwind"
    );
    assert_eq!(first_ends.get(), 1, "first End dispatched before unwinding");
    // The aborted update never reached the second viewport: still
    // mounted and owned, activity open, undisturbed.
    assert_eq!(second.metric_owner(), Some(tree.tree_id()));
    assert!(second.attachment_id().is_some());
    assert!(
        !second.begin_activity(),
        "unreached viewport keeps its activity"
    );
    assert_eq!(second_ends.get(), 0);
    // Tree teardown releases the remaining lease silently. Drop the
    // subscriptions first: the panicking listener has served its
    // purpose, and final cleanup must not re-fire it.
    drop(_first_subscription);
    drop(_second_subscription);
    drop(tree);
    assert_eq!(second.metric_owner(), None);
    assert_eq!(second_ends.get(), 0, "remaining lease tears down silently");
    assert!(second.begin_activity());
    assert!(second.end_activity());
    assert!(first.begin_activity());
    assert!(first.end_activity());
}

#[test]
fn authority_boundary_lifecycle_end_to_end() {
    // The complete authority boundary through public paths only:
    // headless publication succeeds while free; an ordinary viewport
    // attaches; headless and second-viewport writes are rejected without
    // mutation; replacement succeeds transactionally; a wheel attaches
    // after release and publishes through its lease; explicit detach
    // notifies while implicit teardown stays silent; the retained
    // controller remounts successfully.
    let owned = ScrollController::new();
    let next = ScrollController::new();
    let owned_ends = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let next_ends = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let counted_owned = owned_ends.clone();
    let counted_next = next_ends.clone();
    let _owned_subscription = owned.add_listener(move |notification| {
        if notification.kind == incular_scroll::ScrollNotificationType::End {
            counted_owned.set(counted_owned.get() + 1);
        }
        false
    });
    let _next_subscription = next.add_listener(move |notification| {
        if notification.kind == incular_scroll::ScrollNotificationType::End {
            counted_next.set(counted_next.get() + 1);
        }
        false
    });
    let wheel = |controller: ScrollController| -> Widget {
        let view: Widget = ListWheelScrollView::new(
            controller,
            20.0,
            WheelChildDelegate::children(
                (0..8)
                    .map(|index| {
                        Widget::box_(Size::new(80., 20.), Color::WHITE).with_key(index as u64)
                    })
                    .collect(),
            ),
        )
        .into();
        SizedBox::from_dimensions(Some(200.), Some(100.), Some(view)).into()
    };
    // 1. Unattached headless publication succeeds while free.
    owned
        .update_extents_with_physics(300., 100., ScrollPhysics::clamping())
        .expect("free controller accepts headless publication");
    assert_eq!(owned.max_offset(), 200.);
    // 2. An ordinary viewport attaches.
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(owned.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    let attachment = owned.attachment_id().expect("viewport owns");
    assert_eq!(owned.metric_owner(), Some(tree.tree_id()));
    // 3a. Headless publication is rejected without mutation.
    let revision = owned.revision();
    let error = owned
        .update_extents_with_physics(900., 50., ScrollPhysics::clamping())
        .unwrap_err();
    assert_eq!(error.owner_tree(), Some(tree.tree_id()));
    assert_eq!(owned.content_extent(), 300.);
    assert_eq!(owned.viewport_extent(), 100.);
    assert_eq!(owned.max_offset(), 200.);
    assert_eq!(owned.offset(), 0.);
    assert_eq!(owned.revision(), revision);
    assert_eq!(owned.attachment_id(), Some(attachment));
    // 3b. A second viewport is rejected without mutation.
    tree.update(
        root,
        Column::new(vec![
            sized_viewport(owned.clone(), 200., 100., 300.),
            sized_viewport(owned.clone(), 200., 100., 300.),
        ])
        .into(),
    )
    .expect("update");
    let error = tree
        .layout(Constraints::tight(Size::new(200., 300.)))
        .unwrap_err();
    let kids = tree.children(root).expect("viewports").to_vec();
    let viewport_of =
        |sized: incular_widgets::internal::ElementId| tree.children(sized).expect("viewport")[0];
    match error {
        TreeError::DuplicateScrollAttachment {
            owner_tree,
            owner,
            attempted,
        } => {
            assert_eq!(owner_tree, tree.tree_id());
            assert_eq!(owner, Some(viewport_of(kids[0])));
            assert_eq!(attempted, viewport_of(kids[1]));
        }
        other => panic!("unexpected failure: {other:?}"),
    }
    assert_eq!(owned.content_extent(), 300.);
    assert_eq!(owned.max_offset(), 200.);
    assert_eq!(owned.attachment_id(), Some(attachment));
    // 4. Replacement succeeds transactionally: the new controller takes
    // over with its geometry while the old tenure ends and goes free.
    assert!(owned.begin_activity());
    tree.update(
        root,
        Column::new(vec![sized_viewport(next.clone(), 200., 100., 500.)]).into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 100.)))
        .expect("layout");
    assert_eq!(next.content_extent(), 500.);
    assert_eq!(next.max_offset(), 400.);
    assert_eq!(next.metric_owner(), Some(tree.tree_id()));
    assert_eq!(owned.metric_owner(), None);
    assert_eq!(owned_ends.get(), 1, "old tenure ended by the transfer");
    assert_eq!(owned.content_extent(), 300.);
    assert_eq!(owned.max_offset(), 200.);
    // 5. A wheel attaches after release and publishes through its lease.
    tree.update(
        root,
        Column::new(vec![
            sized_viewport(next.clone(), 200., 100., 500.),
            wheel(owned.clone()),
        ])
        .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 250.)))
        .expect("layout");
    assert_eq!(owned.metric_owner(), Some(tree.tree_id()));
    assert_eq!(owned.max_offset(), 140.);
    let wheel_attachment = owned.attachment_id().expect("wheel owns");
    assert_ne!(wheel_attachment, attachment);
    // 6a. Explicit detach notifies: open an activity, unmount the wheel.
    assert!(owned.begin_activity());
    tree.update(
        root,
        Column::new(vec![sized_viewport(next.clone(), 200., 100., 500.)]).into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 100.)))
        .expect("layout");
    assert_eq!(owned.metric_owner(), None);
    assert_eq!(owned_ends.get(), 2, "explicit detach ends the open tenure");
    // 6b. Implicit teardown stays silent: drop the tree holding the next
    // controller with its activity open.
    assert!(next.begin_activity());
    drop(tree);
    assert_eq!(next.metric_owner(), None);
    assert_eq!(next_ends.get(), 0, "teardown emits no End");
    assert!(next.begin_activity());
    assert!(next.end_activity());
    // 7. The retained controller remounts successfully.
    let mut fresh = WidgetTree::new();
    mount_tight(
        &mut fresh,
        sized_viewport(next.clone(), 200., 100., 500.),
        200.,
        100.,
    );
    assert_eq!(next.content_extent(), 500.);
    assert_eq!(next.max_offset(), 400.);
    assert!(next.jump_to(100.));
}

#[test]
fn no_public_route_bypasses_attachment_authority() {
    // Attach an ordinary viewport, then attempt publication through every
    // legacy/model route: both controller entries, both wheel aliases,
    // the 2D pair helper and layout, the sheet inner write, and a second
    // viewport. Every route fails without changing geometry, offset,
    // revision, ownership, or notifications; the owner then publishes
    // normally through its lease.
    let owned = ScrollController::new();
    let free = ScrollController::new();
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let _subscription = owned.add_listener({
        let events = events.clone();
        move |notification| {
            events.borrow_mut().push(notification.kind);
            false
        }
    });
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(owned.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    assert!(owned.jump_to(30.));
    let attachment = owned.attachment_id().expect("viewport owns");
    let revision = owned.revision();
    events.borrow_mut().clear();
    let wheel_children: Vec<Widget> = (0..8)
        .map(|index| Widget::box_(Size::new(80., 20.), Color::WHITE).with_key(index as u64))
        .collect();
    // Routes 1-2: controller entries.
    assert!(owned.update_extents(900., 50.).is_err());
    assert!(
        owned
            .update_extents_with_physics(900., 50., ScrollPhysics::clamping())
            .is_err()
    );
    // Routes 3-4: wheel aliases over the owned controller.
    let mut wheel = ListWheelViewport::new(
        owned.clone(),
        20.0,
        WheelChildDelegate::children(wheel_children),
    );
    assert!(wheel.layout(Size::new(100., 100.)).is_err());
    assert!(
        wheel
            .layout_with_measure(Size::new(100., 100.), |_, constraints| constraints
                .biggest())
            .is_err()
    );
    // Route 5: 2D pair helper spanning the owned axis.
    let scrollable = TwoDimensionalScrollable::new(owned.clone(), free.clone());
    assert!(scrollable.update_extents(1., 1., 1., 1.).is_err());
    // Route 6: 2D viewport layout over the owned axis.
    let mut grid = TwoDimensionalViewport::new(
        TwoDimensionalChildDelegate::new(10, 12, Some),
        owned.clone(),
        free.clone(),
        20.,
        30.,
    );
    assert!(grid.layout(Size::new(90., 60.)).is_err());
    // Route 7: sheet inner write against an owned inner controller. The
    // sheet builds its own inner handle, so ownership here is taken
    // directly — the refused call travels the same checked path an
    // attached inner list would enforce.
    let (state, _) =
        DraggableScrollableSheet::new(|_| Widget::box_(Size::new(80., 20.), Color::WHITE)).mount();
    let inner = state.inner_controller();
    let _inner_lease = inner
        .try_attach(incular_scroll::MetricOwner::of_tree(5))
        .expect("free inner attaches");
    assert!(state.set_inner_extents(500., 100.).is_err());
    // Route 8: a second viewport.
    let mut probe = WidgetTree::new();
    probe
        .mount(sized_viewport(owned.clone(), 200., 100., 300.))
        .expect("mount defers attachment");
    assert!(
        probe
            .layout(Constraints::tight(Size::new(200., 100.)))
            .is_err()
    );
    // Nothing moved anywhere: owned record intact, free axis untouched,
    // no notification emitted by any attempt.
    assert_eq!(owned.content_extent(), 300.);
    assert_eq!(owned.viewport_extent(), 100.);
    assert_eq!(owned.max_offset(), 200.);
    assert_eq!(owned.offset(), 30.);
    assert_eq!(owned.revision(), revision);
    assert_eq!(owned.attachment_id(), Some(attachment));
    assert_eq!(owned.metric_owner(), Some(tree.tree_id()));
    assert_eq!(free.max_offset(), 0.);
    assert_eq!(free.metric_owner(), None);
    assert!(events.borrow().is_empty(), "no route notifies");
    // The owner publishes normally afterward through its lease.
    tree.update(
        root,
        Column::new(vec![sized_viewport(owned.clone(), 200., 100., 320.)]).into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 100.)))
        .expect("layout");
    assert_eq!(owned.content_extent(), 320.);
    assert_eq!(owned.max_offset(), 220.);
    assert_eq!(owned.attachment_id(), Some(attachment));
}

#[test]
fn wheel_sample_completion_preserves_reentrant_activity() {
    // Through the real wheel adapter: a newer activity started inside a
    // sample's notification survives the old sample's completion — the
    // sample's token went stale at takeover, so its finish stays silent
    // instead of ending the newer bracket.
    use incular_scroll::ScrollNotificationType::{End, Start, Update, UserScroll};
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let controller_for_listener = controller.clone();
    let taken_handle = std::rc::Rc::new(std::cell::RefCell::new(None));
    let taken_for_listener = taken_handle.clone();
    let _subscription = controller.add_listener({
        let events = events.clone();
        move |notification| {
            events.borrow_mut().push(notification.kind);
            if notification.kind == UserScroll && taken_for_listener.borrow().is_none() {
                // Take over mid-sample; the sample's own token goes stale.
                *taken_for_listener.borrow_mut() = Some(
                    controller_for_listener
                        .start_owned_activity(incular_scroll::ActivityOrigin::Wheel),
                );
            }
            false
        }
    });
    assert!(tree.scroll_at(Offset::new(50., 50.), Offset::new(0., 20.)));
    assert_eq!(
        events.borrow().as_slice(),
        &[Start, UserScroll, Update],
        "sample completion stays silent after takeover"
    );
    let taken = taken_handle
        .borrow_mut()
        .take()
        .expect("listener retained its takeover");
    assert!(!controller.begin_activity(), "newer bracket still open");
    assert!(taken.finish());
    assert_eq!(
        events.borrow().as_slice(),
        &[Start, UserScroll, Update, End]
    );
}

fn touch(
    tree: &mut WidgetTree,
    pointer: u64,
    point: Offset,
    phase: incular_core::PointerPhase,
    millis: u64,
) {
    use std::time::{Duration, Instant};
    // Deterministic clock: tests advance a fixed origin instead of
    // sampling the wall clock per event.
    static ORIGIN: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let base = *ORIGIN.get_or_init(Instant::now);
    tree.dispatch_gesture(incular_gestures::PointerEvent {
        pointer,
        position: point,
        phase,
        time: base + Duration::from_millis(millis),
    });
}

fn fling_touch(
    tree: &mut WidgetTree,
    base: std::time::Instant,
    pointer: u64,
    point: Offset,
    phase: incular_core::PointerPhase,
    millis: u64,
) {
    // Same dispatch with a caller-owned clock, so pump times share the
    // exact origin the release velocity was measured against.
    tree.dispatch_gesture(incular_gestures::PointerEvent {
        pointer,
        position: point,
        phase,
        time: base + std::time::Duration::from_millis(millis),
    });
}

#[test]
fn touch_down_without_accepted_drag_brackets_nothing() {
    // Down followed by release with no slop never accepts: no bracket,
    // no notifications, no offset change.
    use incular_core::PointerPhase::{Down, Up};
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    assert!(controller.jump_to(100.));
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let _subscription = controller.add_listener({
        let events = events.clone();
        move |notification| {
            events.borrow_mut().push(notification.kind);
            false
        }
    });
    touch(&mut tree, 1, Offset::new(50., 50.), Down, 0);
    touch(&mut tree, 1, Offset::new(50., 50.), Up, 10);
    assert!(events.borrow().is_empty());
    assert_eq!(controller.offset(), 100.);
}

#[test]
fn accepted_touch_drag_brackets_moves() {
    // An accepted touch drag opens one bracket at acceptance: the
    // accepting move already drives inside it, later moves publish
    // plain Updates, and release closes exactly once. A second drag
    // afterwards brackets fresh — no orphaned entries.
    use incular_core::PointerPhase::{Down, Move, Up};
    use incular_scroll::ScrollNotificationType::{End, Start, Update};
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    assert!(controller.jump_to(100.));
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let _subscription = controller.add_listener({
        let events = events.clone();
        move |notification| {
            events.borrow_mut().push(notification.kind);
            false
        }
    });
    touch(&mut tree, 1, Offset::new(50., 30.), Down, 0);
    touch(&mut tree, 1, Offset::new(50., 52.), Move, 10);
    touch(&mut tree, 1, Offset::new(50., 74.), Move, 20);
    touch(&mut tree, 1, Offset::new(50., 96.), Move, 30);
    touch(&mut tree, 1, Offset::new(50., 96.), Up, 40);
    assert_eq!(controller.offset(), 34.);
    assert_eq!(
        events.borrow().as_slice(),
        &[Start, Update, Update, Update, End]
    );
    touch(&mut tree, 2, Offset::new(50., 30.), Down, 50);
    touch(&mut tree, 2, Offset::new(50., 52.), Move, 60);
    touch(&mut tree, 2, Offset::new(50., 52.), Up, 70);
    assert_eq!(
        events.borrow().as_slice(),
        &[Start, Update, Update, Update, End, Start, Update, End]
    );
}

#[test]
fn touch_drag_cancel_closes_bracket() {
    // Cancellation ends the open bracket exactly like release.
    use incular_core::PointerPhase::{Cancel, Down, Move};
    use incular_scroll::ScrollNotificationType::{End, Start, Update};
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    assert!(controller.jump_to(100.));
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let _subscription = controller.add_listener({
        let events = events.clone();
        move |notification| {
            events.borrow_mut().push(notification.kind);
            false
        }
    });
    touch(&mut tree, 1, Offset::new(50., 30.), Down, 0);
    touch(&mut tree, 1, Offset::new(50., 52.), Move, 10);
    touch(&mut tree, 1, Offset::new(50., 52.), Cancel, 20);
    assert_eq!(events.borrow().as_slice(), &[Start, Update, End]);
}

#[test]
fn touch_drag_replacement_rebrackets_stale_token() {
    // Controller replacement mid-drag ends the open bracket through the
    // tenure path; the next accepted move re-brackets the still-live
    // gesture on the old controller, and release closes that — the
    // replacement never gains activity from this stream.
    use incular_core::PointerPhase::{Down, Move, Up};
    use incular_scroll::ScrollNotificationType::{End, Start, Update};
    let controller = ScrollController::new();
    let replacement = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    assert!(controller.jump_to(100.));
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let _subscription = controller.add_listener({
        let events = events.clone();
        move |notification| {
            events.borrow_mut().push(notification.kind);
            false
        }
    });
    touch(&mut tree, 1, Offset::new(50., 30.), Down, 0);
    touch(&mut tree, 1, Offset::new(50., 52.), Move, 10);
    tree.update(
        root,
        Column::new(vec![sized_viewport(replacement.clone(), 200., 100., 300.)]).into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 100.)))
        .expect("layout");
    touch(&mut tree, 1, Offset::new(50., 74.), Move, 20);
    touch(&mut tree, 1, Offset::new(50., 74.), Up, 30);
    assert_eq!(
        events.borrow().as_slice(),
        &[Start, Update, End, Start, Update, End]
    );
    assert_eq!(replacement.offset(), 0.);
    assert!(replacement.begin_activity());
    assert!(replacement.end_activity());
}

#[test]
fn touch_drag_unmount_ends_bracket() {
    // Unmounting mid-drag ends the open bracket; the dead stream's
    // release afterwards is silent and panic-free.
    use incular_core::PointerPhase::{Down, Move, Up};
    use incular_scroll::ScrollNotificationType::{End, Start, Update};
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    assert!(controller.jump_to(100.));
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let _subscription = controller.add_listener({
        let events = events.clone();
        move |notification| {
            events.borrow_mut().push(notification.kind);
            false
        }
    });
    touch(&mut tree, 1, Offset::new(50., 30.), Down, 0);
    touch(&mut tree, 1, Offset::new(50., 52.), Move, 10);
    tree.update(root, Column::new(Vec::<Widget>::new()).into())
        .expect("unmount");
    tree.layout(Constraints::tight(Size::new(200., 100.)))
        .expect("layout");
    assert_eq!(events.borrow().as_slice(), &[Start, Update, End]);
    assert_eq!(controller.metric_owner(), None);
    touch(&mut tree, 1, Offset::new(50., 74.), Up, 20);
    assert_eq!(
        events.borrow().as_slice(),
        &[Start, Update, End],
        "dead-stream release stays silent"
    );
}

#[test]
fn touch_drag_close_aborts_silently() {
    // Dropping the tree mid-drag aborts the bracket silently; the
    // retained controller drives fresh afterward.
    use incular_core::PointerPhase::{Down, Move};
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    assert!(controller.jump_to(100.));
    let ends = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let counted = ends.clone();
    let _subscription = controller.add_listener(move |notification| {
        if notification.kind == incular_scroll::ScrollNotificationType::End {
            counted.set(counted.get() + 1);
        }
        false
    });
    touch(&mut tree, 1, Offset::new(50., 30.), Down, 0);
    touch(&mut tree, 1, Offset::new(50., 52.), Move, 10);
    drop(tree);
    assert_eq!(ends.get(), 0);
    assert_eq!(controller.metric_owner(), None);
    assert!(controller.begin_activity());
    assert!(controller.end_activity());
}

#[test]
fn touch_drag_takeover_survives_release() {
    // A newer activity started mid-drag retires the drag's token: moves
    // keep driving, release stays silent, and the newer bracket stays
    // open.
    use incular_core::PointerPhase::{Down, Move, Up};
    use incular_scroll::ScrollNotificationType::{End, Start, Update};
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    assert!(controller.jump_to(100.));
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let controller_for_listener = controller.clone();
    let taken_handle = std::rc::Rc::new(std::cell::RefCell::new(None));
    let taken_for_listener = taken_handle.clone();
    let _subscription = controller.add_listener({
        let events = events.clone();
        move |notification| {
            events.borrow_mut().push(notification.kind);
            if notification.kind == Update && taken_for_listener.borrow().is_none() {
                *taken_for_listener.borrow_mut() = Some(
                    controller_for_listener
                        .start_owned_activity(incular_scroll::ActivityOrigin::Drag),
                );
            }
            false
        }
    });
    touch(&mut tree, 1, Offset::new(50., 30.), Down, 0);
    touch(&mut tree, 1, Offset::new(50., 52.), Move, 10);
    touch(&mut tree, 1, Offset::new(50., 74.), Move, 20);
    touch(&mut tree, 1, Offset::new(50., 74.), Up, 30);
    assert_eq!(
        events.borrow().as_slice(),
        &[Start, Update, Update],
        "stale release adds no End"
    );
    let taken = taken_handle
        .borrow_mut()
        .take()
        .expect("listener retained its takeover");
    assert!(!controller.begin_activity(), "newer bracket still open");
    assert!(taken.finish());
    assert_eq!(events.borrow().as_slice(), &[Start, Update, Update, End]);
}

#[test]
fn touch_fling_moves_then_settles() {
    // A fast release transfers the live bracket into a ballistic tenure
    // with no End/Start churn; pumped frames move the offset in the
    // fling direction until decay settles, closing with exactly one End.
    // All clocks are explicit, so every offset below is deterministic.
    use incular_core::PointerPhase::{Down, Move, Up};
    use incular_scroll::ScrollNotificationType::{End, Start, Update};
    use std::time::{Duration, Instant};
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let _subscription = controller.add_listener({
        let events = events.clone();
        move |notification| {
            events.borrow_mut().push(notification.kind);
            false
        }
    });
    let base = Instant::now();
    fling_touch(&mut tree, base, 7, Offset::new(50., 70.), Down, 0);
    fling_touch(&mut tree, base, 7, Offset::new(50., 50.), Move, 10);
    fling_touch(&mut tree, base, 7, Offset::new(50., 30.), Move, 20);
    assert_eq!(controller.offset(), 40.);
    fling_touch(&mut tree, base, 7, Offset::new(50., 10.), Up, 30);
    // Transferred, not finished: still open, nothing emitted at release.
    assert_eq!(events.borrow().as_slice(), &[Start, Update, Update]);
    assert!(!controller.begin_activity());
    // First pumped frame moves deterministically: 2,000 px/s decaying
    // over 16 ms advances ~31 px from 40.
    assert!(tree.pump_scroll_flings(base + Duration::from_millis(30), false));
    assert!(tree.pump_scroll_flings(base + Duration::from_millis(46), false));
    let after_first = controller.offset();
    assert!(
        (after_first - 71.).abs() < 0.5,
        "deterministic first step, got {after_first}"
    );
    // Keep pumping: strictly increasing until the clamp, then idle with
    // exactly one End closing the transferred bracket.
    let mut frames = 1;
    while tree.pump_scroll_flings(base + Duration::from_millis(46 + 16 * frames), false) {
        frames += 1;
        assert!(frames < 600, "fling settles");
        assert!(
            controller.offset() <= controller.max_offset(),
            "never past the edge"
        );
    }
    assert_eq!(controller.offset(), 200.);
    let kinds = events.borrow();
    assert_eq!(&kinds[..3], &[Start, Update, Update]);
    assert_eq!(kinds[kinds.len() - 1], End, "one close at settle");
    assert_eq!(kinds.iter().filter(|kind| **kind == Start).count(), 1);
    assert_eq!(kinds.iter().filter(|kind| **kind == End).count(), 1);
    drop(kinds);
    assert!(!tree.pump_scroll_flings(base + Duration::from_millis(10_000), false));
}

#[test]
fn slow_release_settles_without_fling() {
    // Below the fling threshold the release finishes normally: End at
    // release, no driver retained, later pumps idle and motionless.
    use incular_core::PointerPhase::{Down, Move, Up};
    use incular_scroll::ScrollNotificationType::{End, Start, Update};
    use std::time::Instant;
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let _subscription = controller.add_listener({
        let events = events.clone();
        move |notification| {
            events.borrow_mut().push(notification.kind);
            false
        }
    });
    let base = Instant::now();
    fling_touch(&mut tree, base, 7, Offset::new(50., 70.), Down, 0);
    fling_touch(&mut tree, base, 7, Offset::new(50., 50.), Move, 10);
    fling_touch(&mut tree, base, 7, Offset::new(50., 30.), Move, 1_010);
    // 20 px over 1,000 ms: far below threshold.
    fling_touch(&mut tree, base, 7, Offset::new(50., 30.), Up, 2_010);
    assert_eq!(events.borrow().as_slice(), &[Start, Update, Update, End]);
    assert!(!tree.pump_scroll_flings(base + std::time::Duration::from_millis(2_010), false));
    assert_eq!(controller.offset(), 40.);
}

#[test]
fn fling_interrupted_by_new_drag_continues() {
    // A new drag taking over mid-fling keeps motion continuous under the
    // new bracket: the stale driver drops silently at the next pump, and
    // release closes only the new tenure.
    use incular_core::PointerPhase::{Down, Move, Up};
    use incular_scroll::ScrollNotificationType::{End, Start, Update};
    use std::time::{Duration, Instant};
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let _subscription = controller.add_listener({
        let events = events.clone();
        move |notification| {
            events.borrow_mut().push(notification.kind);
            false
        }
    });
    let base = Instant::now();
    fling_touch(&mut tree, base, 7, Offset::new(50., 70.), Down, 0);
    fling_touch(&mut tree, base, 7, Offset::new(50., 50.), Move, 10);
    fling_touch(&mut tree, base, 7, Offset::new(50., 30.), Move, 20);
    fling_touch(&mut tree, base, 7, Offset::new(50., 10.), Up, 30);
    assert!(tree.pump_scroll_flings(base + Duration::from_millis(46), false));
    let flung = controller.offset();
    assert!(flung > 40.);
    // A fresh pointer grabs mid-fling and keeps driving upward.
    fling_touch(&mut tree, base, 8, Offset::new(50., 70.), Down, 50);
    fling_touch(&mut tree, base, 8, Offset::new(50., 50.), Move, 60);
    assert!(
        controller.offset() > flung,
        "motion continues under takeover"
    );
    assert!(!tree.pump_scroll_flings(base + Duration::from_millis(70), false));
    fling_touch(&mut tree, base, 8, Offset::new(50., 50.), Up, 80);
    assert_eq!(
        events.borrow().as_slice(),
        &[Start, Update, Update, Update, Update, End]
    );
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|kind| **kind == Start)
            .count(),
        1
    );
    assert_eq!(
        events.borrow().iter().filter(|kind| **kind == End).count(),
        1
    );
}

#[test]
fn fling_into_bounds_clamps_and_settles() {
    // A fling aimed past the edge never overshoots: the bound holds,
    // and one End closes.
    use incular_core::PointerPhase::{Down, Move, Up};
    use incular_scroll::ScrollNotificationType::End;
    use std::time::{Duration, Instant};
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    assert!(controller.jump_to(190.));
    let ends = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let counted = ends.clone();
    let _subscription = controller.add_listener(move |notification| {
        if notification.kind == End {
            counted.set(counted.get() + 1);
        }
        false
    });
    let base = Instant::now();
    fling_touch(&mut tree, base, 7, Offset::new(50., 70.), Down, 0);
    fling_touch(&mut tree, base, 7, Offset::new(50., 50.), Move, 10);
    fling_touch(&mut tree, base, 7, Offset::new(50., 30.), Move, 20);
    fling_touch(&mut tree, base, 7, Offset::new(50., 10.), Up, 30);
    let mut frames = 0;
    while tree.pump_scroll_flings(base + Duration::from_millis(30 + 16 * frames), false) {
        frames += 1;
        assert!(frames < 600, "clamped fling settles");
        assert_eq!(controller.offset(), 200.);
    }
    assert_eq!(ends.get(), 1);
}

#[test]
fn outward_fling_at_hard_boundary_stops_promptly() {
    // Pinned at the edge with the release aimed further outward, no
    // movement is possible: the first pump closes the tenure instead
    // of scheduling useless frames until the velocity decays. One
    // Start, one End, zero travel.
    use incular_core::PointerPhase::{Down, Move, Up};
    use incular_scroll::ScrollNotificationType::{End, Start};
    use std::time::{Duration, Instant};
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    assert!(controller.jump_to(200.));
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let _subscription = controller.add_listener({
        let events = events.clone();
        move |notification| {
            events.borrow_mut().push(notification.kind);
            false
        }
    });
    let base = Instant::now();
    // Upward finger motion at the maximum offset: clamped in place, but
    // the release still carries outward (positive) velocity.
    fling_touch(&mut tree, base, 7, Offset::new(50., 70.), Down, 0);
    fling_touch(&mut tree, base, 7, Offset::new(50., 48.), Move, 10);
    fling_touch(&mut tree, base, 7, Offset::new(50., 40.), Up, 20);
    assert_eq!(controller.offset(), 200.);
    assert!(
        !tree.pump_scroll_flings(base + Duration::from_millis(36), false),
        "no movement possible, no further frames"
    );
    assert_eq!(controller.offset(), 200.);
    assert_eq!(events.borrow().as_slice(), &[Start, End]);
}

#[test]
fn programmatic_jump_during_fling_ends_it() {
    // An external position change takes visual control: the next pump
    // sees the mismatch, closes the tenure with End, and never writes
    // over the jumped position.
    use incular_core::PointerPhase::{Down, Move, Up};
    use incular_scroll::ScrollNotificationType::End;
    use std::time::{Duration, Instant};
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    let ends = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let counted = ends.clone();
    let _subscription = controller.add_listener(move |notification| {
        if notification.kind == End {
            counted.set(counted.get() + 1);
        }
        false
    });
    let base = Instant::now();
    fling_touch(&mut tree, base, 7, Offset::new(50., 70.), Down, 0);
    fling_touch(&mut tree, base, 7, Offset::new(50., 50.), Move, 10);
    fling_touch(&mut tree, base, 7, Offset::new(50., 30.), Move, 20);
    fling_touch(&mut tree, base, 7, Offset::new(50., 10.), Up, 30);
    assert!(tree.pump_scroll_flings(base + Duration::from_millis(46), false));
    assert!(controller.jump_to(150.));
    assert!(!tree.pump_scroll_flings(base + Duration::from_millis(62), false));
    assert_eq!(ends.get(), 1);
    assert_eq!(controller.offset(), 150.);
    assert!(!tree.pump_scroll_flings(base + Duration::from_millis(78), false));
}

#[test]
fn fling_update_listener_jump_away_never_resumes() {
    // Interference *during* the fling step's own `Update` callbacks —
    // not a between-frames mismatch — must not become the driver's new
    // baseline: the driver committed ~71, the listener jumped to 150,
    // and the tenure closes without writing over 150 or resuming.
    use incular_core::PointerPhase::{Down, Move, Up};
    use incular_scroll::ScrollNotificationType::{End, Start, Update};
    use std::time::{Duration, Instant};
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let armed = std::rc::Rc::new(std::cell::Cell::new(false));
    let jumped = std::rc::Rc::new(std::cell::Cell::new(false));
    let _subscription = controller.add_listener({
        let events = events.clone();
        let armed = armed.clone();
        let jumped = jumped.clone();
        let controller = controller.clone();
        move |notification| {
            events.borrow_mut().push(notification.kind);
            if notification.kind == Update && armed.get() {
                armed.set(false);
                jumped.set(controller.jump_to(150.));
            }
            false
        }
    });
    let base = Instant::now();
    fling_touch(&mut tree, base, 7, Offset::new(50., 70.), Down, 0);
    fling_touch(&mut tree, base, 7, Offset::new(50., 50.), Move, 10);
    fling_touch(&mut tree, base, 7, Offset::new(50., 30.), Move, 20);
    assert_eq!(controller.offset(), 40.);
    fling_touch(&mut tree, base, 7, Offset::new(50., 10.), Up, 30);
    armed.set(true);
    assert!(!tree.pump_scroll_flings(base + Duration::from_millis(46), false));
    assert!(jumped.get(), "listener jump ran inside the step");
    assert_eq!(controller.offset(), 150.);
    assert_eq!(
        events.borrow().as_slice(),
        &[Start, Update, Update, Update, Update, End],
        "driver step, listener jump, then close — no resume"
    );
    assert!(!tree.pump_scroll_flings(base + Duration::from_millis(62), false));
    assert_eq!(controller.offset(), 150., "stale driver writes nothing");
    assert_eq!(
        events.borrow().iter().filter(|kind| **kind == End).count(),
        1
    );
}

#[test]
fn fling_update_listener_same_value_jump_continues() {
    // A listener jumping to the already-current position is a no-op
    // command (same revision, no notification): the tenure survives it
    // and settles normally with one Start and one End.
    use incular_core::PointerPhase::{Down, Move, Up};
    use incular_scroll::ScrollNotificationType::{End, Start, Update};
    use std::time::{Duration, Instant};
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let armed = std::rc::Rc::new(std::cell::Cell::new(false));
    let noop = std::rc::Rc::new(std::cell::Cell::new(true));
    let _subscription = controller.add_listener({
        let events = events.clone();
        let armed = armed.clone();
        let noop = noop.clone();
        let controller = controller.clone();
        move |notification| {
            events.borrow_mut().push(notification.kind);
            if notification.kind == Update && armed.get() {
                armed.set(false);
                noop.set(controller.jump_to(controller.offset()));
            }
            false
        }
    });
    let base = Instant::now();
    fling_touch(&mut tree, base, 7, Offset::new(50., 70.), Down, 0);
    fling_touch(&mut tree, base, 7, Offset::new(50., 50.), Move, 10);
    fling_touch(&mut tree, base, 7, Offset::new(50., 30.), Move, 20);
    fling_touch(&mut tree, base, 7, Offset::new(50., 10.), Up, 30);
    armed.set(true);
    assert!(tree.pump_scroll_flings(base + Duration::from_millis(46), false));
    assert!(!noop.get(), "same-value jump is a no-op command");
    let mut frames = 1;
    while tree.pump_scroll_flings(base + Duration::from_millis(46 + 16 * frames), false) {
        frames += 1;
        assert!(frames < 600, "fling settles");
    }
    assert_eq!(controller.offset(), controller.max_offset());
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|kind| **kind == Start)
            .count(),
        1
    );
    assert_eq!(
        events.borrow().iter().filter(|kind| **kind == End).count(),
        1
    );
}

#[test]
fn fling_update_listener_takeover_leaves_new_activity_intact() {
    // A listener starting another activity mid-drive takes ownership:
    // the fling driver goes stale and drops silently at the same pump
    // — no End for the old tenure, no touch of the new one — and the
    // retained token still closes its bracket exactly once.
    use incular_core::PointerPhase::{Down, Move, Up};
    use incular_scroll::{
        ActivityOrigin,
        ScrollNotificationType::{End, Start, Update},
    };
    use std::time::{Duration, Instant};
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let armed = std::rc::Rc::new(std::cell::Cell::new(false));
    let taken = std::rc::Rc::new(std::cell::RefCell::new(None));
    let _subscription = controller.add_listener({
        let events = events.clone();
        let armed = armed.clone();
        let taken = taken.clone();
        let controller = controller.clone();
        move |notification| {
            events.borrow_mut().push(notification.kind);
            if notification.kind == Update && armed.get() {
                armed.set(false);
                *taken.borrow_mut() = Some(controller.start_owned_activity(ActivityOrigin::Drag));
            }
            false
        }
    });
    let base = Instant::now();
    fling_touch(&mut tree, base, 7, Offset::new(50., 70.), Down, 0);
    fling_touch(&mut tree, base, 7, Offset::new(50., 50.), Move, 10);
    fling_touch(&mut tree, base, 7, Offset::new(50., 30.), Move, 20);
    fling_touch(&mut tree, base, 7, Offset::new(50., 10.), Up, 30);
    armed.set(true);
    assert!(
        !tree.pump_scroll_flings(base + Duration::from_millis(46), false),
        "stale driver retains nothing"
    );
    assert_eq!(
        events.borrow().as_slice(),
        &[Start, Update, Update, Update],
        "no End from the stale driver"
    );
    assert!(!controller.begin_activity(), "newer bracket still open");
    let owned = taken.borrow_mut().take().expect("listener took over");
    assert!(owned.is_current());
    assert!(owned.finish());
    assert_eq!(
        events.borrow().as_slice(),
        &[Start, Update, Update, Update, End]
    );
}

#[test]
fn unmount_mid_fling_cancels_silently() {
    // Tearing the viewport down mid-fling ends the tenure through the
    // normal detach; the orphaned driver drops silently at the next
    // pump with no second End.
    use incular_core::PointerPhase::{Down, Move, Up};
    use incular_scroll::ScrollNotificationType::End;
    use std::time::{Duration, Instant};
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    let ends = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let counted = ends.clone();
    let _subscription = controller.add_listener(move |notification| {
        if notification.kind == End {
            counted.set(counted.get() + 1);
        }
        false
    });
    let base = Instant::now();
    fling_touch(&mut tree, base, 7, Offset::new(50., 70.), Down, 0);
    fling_touch(&mut tree, base, 7, Offset::new(50., 50.), Move, 10);
    fling_touch(&mut tree, base, 7, Offset::new(50., 30.), Move, 20);
    fling_touch(&mut tree, base, 7, Offset::new(50., 10.), Up, 30);
    tree.update(root, Column::new(Vec::<Widget>::new()).into())
        .expect("unmount");
    tree.layout(Constraints::tight(Size::new(200., 100.)))
        .expect("layout");
    assert_eq!(ends.get(), 1);
    assert!(!tree.pump_scroll_flings(base + Duration::from_millis(46), false));
    assert_eq!(ends.get(), 1, "orphaned driver stays silent");
}

#[test]
fn reduced_motion_settles_without_motion() {
    // Reduced motion finishes the transferred tenure in place: one End,
    // zero travel, pump idle immediately.
    use incular_core::PointerPhase::{Down, Move, Up};
    use incular_scroll::ScrollNotificationType::End;
    use std::time::{Duration, Instant};
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    let ends = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let counted = ends.clone();
    let _subscription = controller.add_listener(move |notification| {
        if notification.kind == End {
            counted.set(counted.get() + 1);
        }
        false
    });
    let base = Instant::now();
    fling_touch(&mut tree, base, 7, Offset::new(50., 70.), Down, 0);
    fling_touch(&mut tree, base, 7, Offset::new(50., 50.), Move, 10);
    fling_touch(&mut tree, base, 7, Offset::new(50., 30.), Move, 20);
    fling_touch(&mut tree, base, 7, Offset::new(50., 10.), Up, 30);
    assert_eq!(controller.offset(), 40.);
    assert!(!tree.pump_scroll_flings(base + Duration::from_millis(46), true));
    assert_eq!(ends.get(), 1);
    assert_eq!(controller.offset(), 40.);
}

#[test]
fn dropping_tree_with_open_activity_stays_silent_and_reusable() {
    // Teardown policy: dropping the tree releases ownership and clears
    // the open activity with no End notification — then the retained
    // controller remounts and drives normally.
    let controller = ScrollController::new();
    let ends = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let counted = ends.clone();
    let _subscription = controller.add_listener(move |notification| {
        if notification.kind == incular_scroll::ScrollNotificationType::End {
            counted.set(counted.get() + 1);
        }
        false
    });
    {
        let mut tree = WidgetTree::new();
        mount_tight(
            &mut tree,
            sized_viewport(controller.clone(), 200., 100., 300.),
            200.,
            100.,
        );
        assert!(controller.begin_activity());
    }
    assert_eq!(ends.get(), 0, "teardown emits no End");
    assert_eq!(controller.metric_owner(), None);
    assert!(controller.begin_activity());
    assert!(controller.end_activity());
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        sized_viewport(controller.clone(), 200., 150., 500.),
        200.,
        150.,
    );
    assert_eq!(controller.max_offset(), 350.);
    assert!(controller.jump_to(100.));
}

#[test]
fn sliver_unmount_ends_open_activity() {
    // Same through a sliver viewport render kind.
    let controller = ScrollController::new();
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let _subscription = controller.add_listener({
        let events = events.clone();
        move |notification| {
            events.borrow_mut().push(notification.kind);
            false
        }
    });
    let mut tree = WidgetTree::new();
    let slivers: Widget = CustomScrollView::new(slivers(3))
        .controller(controller.clone())
        .into();
    let root = mount_tight(&mut tree, Column::new(vec![slivers]).into(), 200., 200.);
    assert!(controller.begin_activity());
    tree.update(root, Column::new(Vec::<Widget>::new()).into())
        .expect("unmount");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let kinds: Vec<_> = events
        .borrow()
        .iter()
        .map(|kind| format!("{kind:?}"))
        .collect();
    assert!(
        kinds.ends_with(&["Start".to_owned(), "End".to_owned()]),
        "unmount ends the open activity: {kinds:?}"
    );
    assert!(controller.begin_activity());
    assert!(controller.end_activity());
}

#[test]
fn cross_tree_duplicate_attachment_is_rejected() {
    // Two trees, one controller, different geometry: the second tree's
    // viewport fails naming the owning tree (arena indices may overlap
    // across trees, so the tree id carries identity). The legitimate
    // owner's metrics, offset, and activity stay exactly intact, and the
    // rejected tree recovers with its own controller.
    let controller = ScrollController::new();
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let _subscription = controller.add_listener({
        let events = events.clone();
        move |notification| {
            events.borrow_mut().push(notification.kind);
            false
        }
    });
    let mut tree1 = WidgetTree::new();
    let mut tree2 = WidgetTree::new();
    assert_ne!(tree1.tree_id(), tree2.tree_id());
    mount_tight(
        &mut tree1,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    // Mount defers attachment: only layout claims.
    let root2 = tree2
        .mount(Column::new(vec![sized_viewport(controller.clone(), 200., 150., 500.)]).into())
        .expect("mount defers attachment");
    assert!(controller.jump_to(150.));
    assert!(controller.begin_activity());
    // Re-layout: tree1 converges (same pair reclaims); tree2 fails naming
    // tree1 as the owner with no element (foreign elements are opaque).
    tree1
        .layout(Constraints::tight(Size::new(200., 100.)))
        .expect("owner re-layout converges");
    let error = tree2
        .layout(Constraints::tight(Size::new(200., 150.)))
        .unwrap_err();
    let kids = tree2.children(root2).expect("viewports").to_vec();
    let attempted = tree2.children(kids[0]).expect("viewport")[0];
    match error {
        TreeError::DuplicateScrollAttachment {
            owner_tree,
            owner,
            attempted: got,
        } => {
            assert_eq!(owner_tree, tree1.tree_id());
            assert_eq!(owner, None);
            assert_eq!(got, attempted);
        }
        other => panic!("unexpected failure: {other:?}"),
    }
    // Legitimate owner untouched: metrics, offset, and activity.
    assert_eq!(controller.content_extent(), 300.);
    assert_eq!(controller.viewport_extent(), 100.);
    assert_eq!(controller.max_offset(), 200.);
    assert_eq!(controller.offset(), 150.);
    assert!(!controller.begin_activity(), "still active, not disturbed");
    assert!(controller.end_activity());
    let framed: Vec<incular_scroll::ScrollNotificationType> = events
        .borrow()
        .iter()
        .copied()
        .filter(|kind| {
            matches!(
                kind,
                incular_scroll::ScrollNotificationType::Start
                    | incular_scroll::ScrollNotificationType::End
            )
        })
        .collect();
    assert_eq!(
        framed.as_slice(),
        &[
            incular_scroll::ScrollNotificationType::Start,
            incular_scroll::ScrollNotificationType::End,
        ],
        "exactly one balanced activity around the rejection"
    );
    // The rejected tree recovers with its own controller.
    let other = ScrollController::new();
    tree2
        .update(
            root2,
            Column::new(vec![sized_viewport(other.clone(), 200., 150., 500.)]).into(),
        )
        .expect("update");
    tree2
        .layout(Constraints::tight(Size::new(200., 150.)))
        .expect("layout");
    assert_eq!(other.max_offset(), 350.);
}

#[test]
fn replacement_detaches_the_old_tenure_activity() {
    // Viewport-tenure policy: an open activity belongs to the viewport
    // driving the controller. Replacing the controller ends the old
    // tenure with `End` — after the new attachment commits — instead of
    // stranding its flag to brick the handle's next tenure. The incoming
    // tenure starts fresh, and the replaced-away handle reattaches
    // cleanly elsewhere with its metrics intact.
    let old = ScrollController::new();
    let new = ScrollController::new();
    let log_old = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let log_new = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut subscriptions = Vec::new();
    for (controller, log) in [(&old, &log_old), (&new, &log_new)] {
        let log = log.clone();
        subscriptions.push(controller.add_listener(move |notification| {
            log.borrow_mut().push(notification.kind);
            false
        }));
    }
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(old.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    assert_eq!(old.max_offset(), 200.);
    assert!(old.begin_activity());
    tree.update(
        root,
        Column::new(vec![sized_viewport(new.clone(), 200., 100., 500.)]).into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 100.)))
        .expect("layout");
    assert_eq!(new.content_extent(), 500.);
    assert_eq!(new.max_offset(), 400.);
    // Ownership moved; the old handle is free with metrics intact.
    assert_eq!(old.metric_owner(), None);
    assert_eq!(new.metric_owner(), Some(tree.tree_id()));
    assert_eq!(old.content_extent(), 300.);
    assert_eq!(old.max_offset(), 200.);
    // The transfer closed the old tenure: its only End is accounted for
    // here, so a further end finds nothing and a fresh begin succeeds.
    assert!(!old.end_activity());
    assert!(old.begin_activity());
    assert!(old.end_activity());
    // The new tenure starts fresh.
    assert!(new.begin_activity());
    use incular_scroll::ScrollNotificationType::{End, Start};
    let framed =
        |log: &std::rc::Rc<std::cell::RefCell<Vec<incular_scroll::ScrollNotificationType>>>| {
            log.borrow()
                .iter()
                .copied()
                .filter(|kind| matches!(kind, Start | End))
                .collect::<Vec<_>>()
        };
    assert_eq!(framed(&log_old).as_slice(), &[Start, End, Start, End]);
    assert_eq!(framed(&log_new).as_slice(), &[Start]);
    assert!(new.end_activity());
    drop(subscriptions);
    // The replaced-away handle reattaches cleanly elsewhere.
    let mut abroad = WidgetTree::new();
    mount_tight(
        &mut abroad,
        sized_viewport(old.clone(), 200., 100., 300.),
        200.,
        100.,
    );
    assert_eq!(old.max_offset(), 200.);
    assert!(old.begin_activity());
    assert!(old.end_activity());
    // Repeated unchanged layouts stay clean and stable.
    tree.layout(Constraints::tight(Size::new(200., 100.)))
        .expect("layout");
    tree.layout(Constraints::tight(Size::new(200., 100.)))
        .expect("layout");
    assert_eq!(new.max_offset(), 400.);
    assert_eq!(new.offset(), 0.);
}

#[test]
fn reentrant_attach_during_replacement_end_survives() {
    // During the transfer's End on the old controller, a listener
    // attaches it and begins a fresh tenure — no trailing cleanup
    // cancels what the listener starts.
    let old = ScrollController::new();
    let new = ScrollController::new();
    let mut tree = WidgetTree::new();
    let tree_id = tree.tree_id();
    let reattached = std::rc::Rc::new(std::cell::RefCell::new(None));
    let reattached_for_listener = reattached.clone();
    let old_for_listener = old.clone();
    let _subscription = old.add_listener(move |notification| {
        if notification.kind == incular_scroll::ScrollNotificationType::End
            && reattached_for_listener.borrow().is_none()
        {
            let handle = old_for_listener
                .try_attach(incular_scroll::MetricOwner::of_tree(tree_id))
                .expect("tenure released before End");
            assert!(old_for_listener.begin_activity());
            *reattached_for_listener.borrow_mut() = Some(handle);
        }
        false
    });
    let root = mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(old.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    assert!(old.begin_activity());
    tree.update(
        root,
        Column::new(vec![sized_viewport(new.clone(), 200., 100., 500.)]).into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 100.)))
        .expect("layout");
    // The listener's tenure on the old controller is live and open;
    // the new controller drives the viewport with no activity yet.
    let handle = reattached
        .borrow_mut()
        .take()
        .expect("listener attached during transfer End");
    assert_eq!(old.metric_owner(), Some(tree_id));
    assert_eq!(old.attachment_id(), Some(handle.id()));
    assert!(!old.begin_activity());
    assert_eq!(new.metric_owner(), Some(tree_id));
    assert!(new.begin_activity());
    assert!(new.end_activity());
    drop(_subscription);
    assert!(handle.release());
    assert!(old.end_activity());
    assert_eq!(old.metric_owner(), None);
}

#[test]
fn failed_replacement_preserves_owner_against_local_conflict() {
    // Acquire-before-release, local shape:
    // 1. Viewport A owns X with an active activity; viewport B owns Y.
    // 2. Replace A's controller with Y. 3. The layout fails. 4. A still
    // owns X: attachment identity, owner, activity, and metrics all
    // unchanged — offsets alone would not prove ownership. 5. A third
    // tree attempting X is rejected by the live owner. 6. Repair resumes
    // normal operation, and same-controller updates disturb nothing.
    let owned = ScrollController::new();
    let rival = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        Column::new(vec![
            sized_viewport(owned.clone(), 200., 100., 300.),
            sized_viewport(rival.clone(), 200., 100., 500.),
        ])
        .into(),
        200.,
        300.,
    );
    assert!(owned.jump_to(30.));
    assert!(owned.begin_activity());
    let attachment = owned.attachment_id().expect("A owns X");
    assert_eq!(owned.metric_owner(), Some(tree.tree_id()));
    tree.update(
        root,
        Column::new(vec![
            sized_viewport(rival.clone(), 200., 100., 300.),
            sized_viewport(rival.clone(), 200., 100., 500.),
        ])
        .into(),
    )
    .expect("update");
    let error = tree
        .layout(Constraints::tight(Size::new(200., 300.)))
        .unwrap_err();
    let kids = tree.children(root).expect("viewports").to_vec();
    let viewport_of =
        |sized: incular_widgets::internal::ElementId| tree.children(sized).expect("viewport")[0];
    match error {
        TreeError::DuplicateScrollAttachment {
            owner_tree,
            owner,
            attempted,
        } => {
            assert_eq!(owner_tree, tree.tree_id());
            assert_eq!(owner, Some(viewport_of(kids[1])));
            assert_eq!(attempted, viewport_of(kids[0]));
        }
        other => panic!("unexpected failure: {other:?}"),
    }
    assert_eq!(owned.attachment_id(), Some(attachment));
    assert_eq!(owned.metric_owner(), Some(tree.tree_id()));
    assert!(!owned.begin_activity());
    assert_eq!(owned.offset(), 30.);
    assert_eq!(owned.content_extent(), 300.);
    assert_eq!(owned.max_offset(), 200.);
    let mut probe = WidgetTree::new();
    probe
        .mount(sized_viewport(owned.clone(), 200., 100., 300.))
        .expect("mount defers attachment");
    match probe
        .layout(Constraints::tight(Size::new(200., 100.)))
        .unwrap_err()
    {
        TreeError::DuplicateScrollAttachment {
            owner_tree,
            owner,
            attempted: _,
        } => {
            assert_eq!(owner_tree, tree.tree_id());
            assert_eq!(owner, None);
        }
        other => panic!("unexpected failure: {other:?}"),
    }
    tree.update(
        root,
        Column::new(vec![
            sized_viewport(owned.clone(), 200., 100., 300.),
            sized_viewport(rival.clone(), 200., 100., 500.),
        ])
        .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 300.)))
        .expect("layout");
    assert_eq!(owned.attachment_id(), Some(attachment));
    assert_eq!(owned.max_offset(), 200.);
    assert!(owned.end_activity());
    assert!(owned.begin_activity());
    tree.update(
        root,
        Column::new(vec![
            sized_viewport(owned.clone(), 200., 100., 300.),
            sized_viewport(rival.clone(), 200., 100., 500.),
        ])
        .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 300.)))
        .expect("layout");
    assert_eq!(owned.attachment_id(), Some(attachment));
    assert!(!owned.begin_activity());
    assert!(owned.jump_to(60.));
    assert_eq!(owned.offset(), 60.);
    assert!(owned.end_activity());
}

#[test]
fn failed_replacement_preserves_owner_against_foreign_conflict() {
    // Acquire-before-release, foreign shape: Y is owned abroad, so the
    // failure names the foreign tree with no element. X stays fully
    // owned here, a third tree is still rejected, and the failure
    // persists on relayout until the widget changes back — then normal
    // operation resumes.
    let owned = ScrollController::new();
    let abroad = ScrollController::new();
    let mut foreign = WidgetTree::new();
    mount_tight(
        &mut foreign,
        sized_viewport(abroad.clone(), 200., 100., 500.),
        200.,
        100.,
    );
    let mut tree = WidgetTree::new();
    assert_ne!(foreign.tree_id(), tree.tree_id());
    let root = mount_tight(
        &mut tree,
        sized_viewport(owned.clone(), 200., 100., 300.),
        200.,
        100.,
    );
    assert!(owned.jump_to(30.));
    assert!(owned.begin_activity());
    let attachment = owned.attachment_id().expect("owns X");
    tree.update(root, sized_viewport(abroad.clone(), 200., 100., 300.))
        .expect("update");
    let error = tree
        .layout(Constraints::tight(Size::new(200., 100.)))
        .unwrap_err();
    match error {
        TreeError::DuplicateScrollAttachment {
            owner_tree,
            owner,
            attempted: _,
        } => {
            assert_eq!(owner_tree, foreign.tree_id());
            assert_eq!(owner, None);
        }
        other => panic!("unexpected failure: {other:?}"),
    }
    assert_eq!(owned.attachment_id(), Some(attachment));
    assert_eq!(owned.metric_owner(), Some(tree.tree_id()));
    assert!(!owned.begin_activity());
    assert_eq!(owned.offset(), 30.);
    assert_eq!(owned.max_offset(), 200.);
    let mut probe = WidgetTree::new();
    probe
        .mount(sized_viewport(owned.clone(), 200., 100., 300.))
        .expect("mount defers attachment");
    match probe
        .layout(Constraints::tight(Size::new(200., 100.)))
        .unwrap_err()
    {
        TreeError::DuplicateScrollAttachment {
            owner_tree,
            owner,
            attempted: _,
        } => {
            assert_eq!(owner_tree, tree.tree_id());
            assert_eq!(owner, None);
        }
        other => panic!("unexpected failure: {other:?}"),
    }
    tree.layout(Constraints::tight(Size::new(200., 100.)))
        .expect_err("still owned abroad");
    tree.update(root, sized_viewport(owned.clone(), 200., 100., 300.))
        .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 100.)))
        .expect("layout");
    assert_eq!(owned.attachment_id(), Some(attachment));
    assert_eq!(owned.max_offset(), 200.);
    assert!(owned.end_activity());
    assert!(owned.begin_activity());
    assert!(owned.jump_to(60.));
    assert_eq!(owned.offset(), 60.);
    assert!(owned.end_activity());
}

#[test]
fn rejected_viewport_unmount_keeps_owner_activity() {
    // Only an owned lease triggers detach behavior: unmounting the
    // rejected second viewport must not end the owner's open activity,
    // disturb its metrics, or poison later layouts.
    let controller = ScrollController::new();
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let _subscription = controller.add_listener({
        let events = events.clone();
        move |notification| {
            events.borrow_mut().push(notification.kind);
            false
        }
    });
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Column::new(vec![
                sized_viewport(controller.clone(), 200., 100., 300.),
                sized_viewport(controller.clone(), 200., 150., 500.),
            ])
            .into(),
        )
        .expect("mount defers attachment");
    assert!(controller.begin_activity());
    tree.layout(Constraints::tight(Size::new(200., 300.)))
        .expect_err("second viewport rejected");
    // Unmount the rejected viewport: the owner's activity survives.
    tree.update(
        root,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 300.)))
        .expect("layout");
    assert!(!controller.begin_activity(), "owner activity untouched");
    assert_eq!(controller.max_offset(), 200.);
    assert!(controller.end_activity());
    use incular_scroll::ScrollNotificationType::{End, Start};
    let framed: Vec<incular_scroll::ScrollNotificationType> = events
        .borrow()
        .iter()
        .copied()
        .filter(|kind| matches!(kind, Start | End))
        .collect();
    assert_eq!(framed.as_slice(), &[Start, End]);
}

fn sized_viewport(
    controller: ScrollController,
    width: f32,
    height: f32,
    content_height: f32,
) -> Widget {
    let scrolled: Widget =
        SingleChildScrollView::new(Widget::box_(Size::new(width, content_height), Color::WHITE))
            .controller(controller)
            .into();
    SizedBox::from_dimensions(Some(width), Some(height), Some(scrolled)).into()
}

#[test]
fn replaced_controller_frees_the_old_handle() {
    // Replacement swaps the driver: the viewport follows the new
    // controller's geometry, and the old handle attaches cleanly
    // elsewhere — no record lingers.
    let old = ScrollController::new();
    let new = ScrollController::new();
    let mut tree = WidgetTree::new();
    // Stable wrappers (reconciliation matches element kinds): only the
    // controllers and contents change across updates.
    let root = mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(old.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    assert_eq!(old.max_offset(), 200.);
    tree.update(
        root,
        Column::new(vec![sized_viewport(new.clone(), 200., 100., 500.)]).into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 100.)))
        .expect("layout");
    assert_eq!(new.max_offset(), 400.);
    // The old handle is free: a second viewport takes it without error.
    tree.update(
        root,
        Column::new(vec![
            sized_viewport(new.clone(), 200., 100., 500.),
            sized_viewport(old.clone(), 200., 100., 300.),
        ])
        .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 300.)))
        .expect("layout");
    assert_eq!(new.max_offset(), 400.);
    assert_eq!(old.max_offset(), 200.);
}

#[test]
fn unmount_releases_attachment_for_remount() {
    // Deterministic release: unmounting drops the claim, so the same
    // controller remounts elsewhere with the new geometry.
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    assert_eq!(controller.max_offset(), 200.);
    tree.update(root, Column::new(Vec::<Widget>::new()).into())
        .expect("unmount");
    tree.layout(Constraints::tight(Size::new(200., 100.)))
        .expect("layout");
    tree.update(
        root,
        Column::new(vec![sized_viewport(controller.clone(), 200., 150., 500.)]).into(),
    )
    .expect("remount");
    tree.layout(Constraints::tight(Size::new(200., 150.)))
        .expect("layout");
    assert_eq!(controller.content_extent(), 500.);
    assert_eq!(controller.viewport_extent(), 150.);
    assert_eq!(controller.max_offset(), 350.);
}

#[test]
fn failed_update_preserves_attachment() {
    // A rejected update changes nothing, including the attachment map: a
    // duplicate-key failure elsewhere leaves the viewport driving, and a
    // valid retry works without spurious conflicts.
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        Column::new(vec![
            sized_viewport(controller.clone(), 200., 100., 300.),
            Widget::box_(Size::new(200., 40.), Color::WHITE),
        ])
        .into(),
        200.,
        300.,
    );
    assert_eq!(controller.max_offset(), 200.);
    tree.update(
        root,
        Column::new(vec![
            sized_viewport(controller.clone(), 200., 100., 300.),
            Widget::box_(Size::new(200., 40.), Color::WHITE).with_key(7_u64),
            Widget::box_(Size::new(200., 40.), Color::BLACK).with_key(7_u64),
        ])
        .into(),
    )
    .expect_err("duplicate keys fail before mutation");
    tree.layout(Constraints::tight(Size::new(200., 300.)))
        .expect("layout still clean");
    assert_eq!(controller.max_offset(), 200.);
    assert!(controller.jump_to(200.));
    tree.layout(Constraints::tight(Size::new(200., 300.)))
        .expect("retry works");
    assert_eq!(controller.offset(), 200.);
}

#[test]
fn scrollbar_and_app_clones_are_not_attachments() {
    // Read-only coordination never claims: a headless scrollbar sharing
    // the controller plus programmatic jumps compose with a mounted
    // viewport without conflict.
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        sized_viewport(controller.clone(), 200., 100., 300.),
        200.,
        100.,
    );
    let alias = controller.clone();
    assert!(alias.jump_to(50.));
    let bar = RawScrollbar::new(controller.clone());
    let geometry = bar.geometry(Size::new(120., 100.));
    assert!(geometry.thumb.size.height > 0.);
    tree.layout(Constraints::tight(Size::new(200., 100.)))
        .expect("layout");
    assert_eq!(controller.offset(), 50.);
    assert_eq!(alias.offset(), 50.);
}

#[test]
fn deferred_jump_applies_on_first_attached_layout() {
    // Programmatic use before attachment: the deferred request survives
    // until the first layout establishes bounds, then applies.
    let controller = ScrollController::new();
    assert!(controller.deferred_jump_to(80.));
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        sized_viewport(controller.clone(), 200., 100., 300.),
        200.,
        100.,
    );
    assert_eq!(controller.offset(), 80.);
}

#[test]
fn steady_layout_reuses_its_lease() {
    // The stored lease is the attachment: repeated layouts neither
    // re-acquire nor disturb ownership — the attachment identity stays
    // put while geometry keeps refreshing. The viewport is built from a
    // clone, proving clones configure the same attachment.
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        sized_viewport(controller.clone(), 200., 100., 300.),
        200.,
        100.,
    );
    let first = controller.attachment_id().expect("attached");
    assert_eq!(controller.metric_owner(), Some(tree.tree_id()));
    tree.layout(Constraints::tight(Size::new(200., 100.)))
        .expect("second layout");
    assert_eq!(controller.attachment_id(), Some(first));
    assert_eq!(controller.metric_owner(), Some(tree.tree_id()));
    assert_eq!(controller.max_offset(), 200.);
    // A clone observes the same live attachment without claiming.
    assert_eq!(controller.clone().attachment_id(), Some(first));
}

#[test]
fn unattached_publication_rejected_while_viewport_owns() {
    // The checked headless path refuses an owned controller with full
    // preservation, then succeeds once the viewport detaches: ownership
    // gates publication, detachment restores headless access.
    let controller = ScrollController::new();
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let _subscription = controller.add_listener({
        let events = events.clone();
        move |notification| {
            events.borrow_mut().push(notification.kind);
            false
        }
    });
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        Column::new(vec![sized_viewport(controller.clone(), 200., 100., 300.)]).into(),
        200.,
        100.,
    );
    assert!(controller.jump_to(30.));
    let attachment = controller.attachment_id().expect("viewport owns");
    let revision = controller.revision();
    events.borrow_mut().clear();
    let error = controller
        .update_extents_with_physics(900., 50., ScrollPhysics::clamping())
        .unwrap_err();
    assert_eq!(error.owner_tree(), Some(tree.tree_id()));
    assert_eq!(controller.content_extent(), 300.);
    assert_eq!(controller.viewport_extent(), 100.);
    assert_eq!(controller.max_offset(), 200.);
    assert_eq!(controller.offset(), 30.);
    assert_eq!(controller.revision(), revision);
    assert_eq!(controller.attachment_id(), Some(attachment));
    assert!(events.borrow().is_empty(), "rejection emits nothing");
    tree.update(root, Column::new(Vec::<Widget>::new()).into())
        .expect("unmount");
    tree.layout(Constraints::tight(Size::new(200., 100.)))
        .expect("layout");
    controller
        .update_extents_with_physics(900., 50., ScrollPhysics::clamping())
        .expect("freed controller accepts headless publication");
    assert_eq!(controller.max_offset(), 850.);
}

#[test]
fn dropped_tree_releases_attachments() {
    // Window-teardown shape: dropping the whole tree releases every
    // claim, so the app-owned controller mounts cleanly in a fresh tree.
    let controller = ScrollController::new();
    {
        let mut tree = WidgetTree::new();
        mount_tight(
            &mut tree,
            sized_viewport(controller.clone(), 200., 100., 300.),
            200.,
            100.,
        );
        assert_eq!(controller.max_offset(), 200.);
    }
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        sized_viewport(controller.clone(), 200., 150., 500.),
        200.,
        150.,
    );
    assert_eq!(controller.content_extent(), 500.);
    assert_eq!(controller.viewport_extent(), 150.);
    assert_eq!(controller.max_offset(), 350.);
}
