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
    Column, ListView, RawScrollbar, Scrollable, Semantics, SingleChildScrollView, Viewport,
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
