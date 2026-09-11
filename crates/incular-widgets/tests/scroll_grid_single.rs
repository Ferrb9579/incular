//! Grid, single-child, and animated-list scrolling coverage.
//!
//! Second scrolling ledger group: `SingleChildScrollView`, `GridView` (with
//! `SliverGridDelegate`), `AnimatedList`, `AnimatedGrid`, and the animated
//! collection controller. Viewport configuration and ordinary lists are
//! covered by the first group. Offset ownership stays with the
//! controllers; retained layout stays in Widgets.

mod common;

use common::*;
use incular_config::{Axis, Constraints, EdgeInsets};
use incular_core::{Color, Offset, Size};
use incular_rendering::PaintCommand;
use incular_scroll::ScrollPhysics;
use incular_widgets::{
    AnimatedGrid, AnimatedList, AnimatedListController, GridView, SingleChildScrollView,
    SliverGridDelegate,
};
use std::time::{Duration, Instant};

fn grid_boxes(count: usize) -> Vec<Widget> {
    (0..count)
        .map(|_| Widget::box_(Size::new(80., 40.), Color::WHITE))
        .collect()
}

fn child_origins(tree: &WidgetTree, id: incular_widgets::internal::ElementId) -> Vec<Offset> {
    tree.children(id)
        .expect("items")
        .iter()
        .map(|kid| tree.element_bounds(*kid).expect("bounds").origin)
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

#[test]
fn single_child_scroll_view_builder_defaults_match_new() {
    let mut via_builder = WidgetTree::new();
    let builder_root = mount_tight(
        &mut via_builder,
        SingleChildScrollView::builder()
            .child(Widget::box_(Size::new(80., 300.), Color::WHITE))
            .build()
            .into(),
        100.,
        100.,
    );
    let mut via_ctor = WidgetTree::new();
    let ctor_root = mount_tight(
        &mut via_ctor,
        SingleChildScrollView::new(Widget::box_(Size::new(80., 300.), Color::WHITE)).into(),
        100.,
        100.,
    );
    assert_eq!(
        subtree_geometry(&via_builder, builder_root),
        subtree_geometry(&via_ctor, ctor_root)
    );
}

#[test]
fn grid_view_builder_defaults_match_new() {
    let mut via_builder = WidgetTree::new();
    let builder_root = mount_tight(
        &mut via_builder,
        GridView::typed_builder()
            .children(grid_boxes(4))
            .grid_delegate(SliverGridDelegate::fixed_cross_axis_count(2))
            .build()
            .into(),
        200.,
        200.,
    );
    let mut via_ctor = WidgetTree::new();
    let ctor_root = mount_tight(
        &mut via_ctor,
        GridView::count(2, grid_boxes(4)).into(),
        200.,
        200.,
    );
    // The generated builder with an explicit 2-count delegate lays out
    // exactly like the matching constructor.
    assert_eq!(
        subtree_geometry(&via_builder, builder_root),
        subtree_geometry(&via_ctor, ctor_root)
    );
}

#[test]
fn single_child_scroll_view_axis_change_reflows() {
    let controller = ScrollController::new();
    let rows = || {
        incular_widgets::Row::new(vec![
            Widget::box_(Size::new(40., 30.), Color::WHITE),
            Widget::box_(Size::new(40., 30.), Color::WHITE),
            Widget::box_(Size::new(40., 30.), Color::WHITE),
        ])
    };
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        SingleChildScrollView::new(rows())
            .controller(controller.clone())
            .into(),
        100.,
        40.,
    );
    // Vertical: the 120px-wide row overflows nowhere vertically.
    assert_eq!(controller.max_offset(), 0.);
    tree.update(
        root,
        SingleChildScrollView::new(rows())
            .scroll_direction(Axis::Horizontal)
            .controller(controller.clone())
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 40.)))
        .expect("layout");
    // Horizontal: the same row scrolls along x.
    assert_eq!(controller.max_offset(), 20.);
    assert!(controller.jump_to(20.));
    tree.layout(Constraints::tight(Size::new(100., 40.)))
        .expect("layout");
    assert_eq!(controller.offset(), 20.);
}

#[test]
fn single_child_scroll_view_reverse_and_padding_anchor_end() {
    // 300px child plus 10px padding in a 100px viewport scrolls 220px;
    // reverse anchors the content end to the viewport end.
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        SingleChildScrollView::new(Widget::box_(Size::new(80., 300.), Color::WHITE))
            .reverse(true)
            .padding(EdgeInsets::all(10.))
            .controller(controller.clone())
            .into(),
        100.,
        100.,
    );
    assert_eq!(controller.max_offset(), 220.);
    let kids = tree.children(root).expect("items").to_vec();
    assert_eq!(kids.len(), 1);
    assert_eq!(
        tree.element_bounds(kids[0]).expect("bounds").origin,
        Offset::new(0., -220.)
    );
}

#[test]
fn single_child_scroll_view_controller_replacement_isolates_old() {
    let first = ScrollController::new();
    let second = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        SingleChildScrollView::new(Widget::box_(Size::new(80., 300.), Color::WHITE))
            .controller(first.clone())
            .into(),
        100.,
        100.,
    );
    assert!(first.jump_to(100.));
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    tree.update(
        root,
        SingleChildScrollView::new(Widget::box_(Size::new(80., 300.), Color::WHITE))
            .controller(second.clone())
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert_eq!(second.offset(), 0.);
    assert!(second.jump_to(100.));
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    // The detached handle keeps its own record without moving the view.
    assert!(first.jump_to(0.));
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert_eq!(first.offset(), 0.);
    assert_eq!(second.offset(), 100.);
    let kids = tree.children(root).expect("items").to_vec();
    assert_eq!(
        tree.element_bounds(kids[0]).expect("bounds").origin,
        Offset::new(0., -100.)
    );
}

#[test]
fn single_child_scroll_view_physics_replacement_changes_input() {
    let controller = ScrollController::new();
    let child = || Widget::box_(Size::new(80., 300.), Color::WHITE);
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        SingleChildScrollView::new(child())
            .physics(ScrollPhysics::clamping())
            .controller(controller.clone())
            .into(),
        100.,
        100.,
    );
    assert!(tree.scroll_at(Offset::new(50., 50.), Offset::new(0., 20.)));
    assert_eq!(controller.offset(), 20.);
    tree.update(
        root,
        SingleChildScrollView::new(child())
            .physics(ScrollPhysics::clamping().never_scrollable())
            .controller(controller.clone())
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert!(!tree.scroll_at(Offset::new(50., 50.), Offset::new(0., 20.)));
    assert_eq!(controller.offset(), 20.);
}

#[test]
fn grid_view_count_lays_rows_and_cells() {
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        GridView::count(2, grid_boxes(4)).into(),
        200.,
        200.,
    );
    let rows = tree.children(root).expect("rows").to_vec();
    assert_eq!(rows.len(), 2);
    // Two columns of aspect 1 in 200px: 100px cells in 100px rows.
    assert_eq!(
        tree.element_bounds(rows[0]).expect("bounds").size,
        Size::new(200., 100.)
    );
    let cells = tree.children(rows[0]).expect("cells").to_vec();
    assert_eq!(cells.len(), 2);
    assert_eq!(
        tree.element_bounds(cells[0]).expect("bounds").origin,
        Offset::new(0., 0.)
    );
    assert_eq!(
        tree.element_bounds(cells[1]).expect("bounds").origin,
        Offset::new(100., 0.)
    );
    assert_eq!(
        tree.element_bounds(rows[1]).expect("bounds").origin,
        Offset::new(0., 100.)
    );
}

#[test]
fn grid_view_constructors_agree() {
    // count, max-extent (resolving to 2 columns in 200px), and builder
    // with the same delegate lay out identically.
    let mut by_count = WidgetTree::new();
    let count_root = mount_tight(
        &mut by_count,
        GridView::count(2, grid_boxes(4)).into(),
        200.,
        200.,
    );
    let mut by_extent = WidgetTree::new();
    let extent_root = mount_tight(
        &mut by_extent,
        GridView::extent(90., grid_boxes(4)).into(),
        200.,
        200.,
    );
    let mut by_builder = WidgetTree::new();
    let builder_root = mount_tight(
        &mut by_builder,
        GridView::builder(4, SliverGridDelegate::fixed_cross_axis_count(2), |_| {
            Widget::box_(Size::new(80., 40.), Color::WHITE)
        })
        .into(),
        200.,
        200.,
    );
    assert_eq!(
        subtree_geometry(&by_count, count_root),
        subtree_geometry(&by_extent, extent_root)
    );
    assert_eq!(
        subtree_geometry(&by_count, count_root),
        subtree_geometry(&by_builder, builder_root)
    );
}

#[test]
fn grid_view_delegate_change_reflows_columns() {
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        GridView::count(2, grid_boxes(4)).into(),
        200.,
        200.,
    );
    let row_height = |tree: &WidgetTree| {
        tree.element_bounds(tree.children(root).expect("rows")[0])
            .expect("bounds")
            .size
            .height
    };
    assert_eq!(row_height(&tree), 100.);
    // Three columns of aspect 1 in 200px: 200/3 rows.
    tree.update(root, GridView::count(3, grid_boxes(4)).into())
        .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert!((row_height(&tree) - 200. / 3.).abs() < 0.01);
    // A fixed 60px main extent overrides the aspect-derived rows.
    tree.update(
        root,
        GridView::count(2, grid_boxes(4))
            .grid_delegate(SliverGridDelegate::fixed_cross_axis_count(2).main_axis_extent(60.))
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(row_height(&tree), 60.);
}

#[test]
fn grid_view_axis_reverse_and_shrink_wrap() {
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        GridView::count(2, grid_boxes(4)).into(),
        200.,
        200.,
    );
    // Horizontal: the same 2-count forms 100x200 columns.
    tree.update(
        root,
        GridView::count(2, grid_boxes(4))
            .scroll_direction(Axis::Horizontal)
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(
        child_origins(&tree, root),
        vec![Offset::new(0., 0.), Offset::new(100., 0.)]
    );
    // Reverse with overflowing content anchors the trailing edge: five
    // items in two columns are three 100px rows in a 200px viewport.
    tree.update(root, GridView::count(2, grid_boxes(5)).reverse(true).into())
        .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(child_origins(&tree, root)[0], Offset::new(0., -100.));
    // Shrink-wrap sizes to the 200px content in loose bounds.
    let mut loose = WidgetTree::new();
    let shrink_root = loose
        .mount(GridView::count(2, grid_boxes(4)).shrink_wrap(true).into())
        .expect("mount");
    loose
        .layout(Constraints::loose(Size::new(200., 300.)))
        .expect("layout");
    assert_eq!(
        loose.element_bounds(shrink_root).expect("bounds").size,
        Size::new(200., 200.)
    );
}

#[test]
fn grid_view_controller_and_physics_replacement() {
    let first = ScrollController::new();
    let second = ScrollController::new();
    let controller = first.clone();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        GridView::builder(100, SliverGridDelegate::fixed_cross_axis_count(2), |_| {
            Widget::box_(Size::new(80., 40.), Color::WHITE)
        })
        .controller(controller)
        .physics(ScrollPhysics::clamping())
        .into(),
        100.,
        100.,
    );
    // Gesture input scrolls under clamping physics.
    assert!(tree.scroll_at(Offset::new(50., 50.), Offset::new(0., 20.)));
    assert_eq!(first.offset(), 20.);
    // Swapping the descriptor swaps attachment: the new handle drives the
    // viewport while the old record stays isolated.
    tree.update(
        root,
        GridView::builder(100, SliverGridDelegate::fixed_cross_axis_count(2), |_| {
            Widget::box_(Size::new(80., 40.), Color::WHITE)
        })
        .controller(second.clone())
        .physics(ScrollPhysics::clamping().never_scrollable())
        .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert!(!tree.scroll_at(Offset::new(50., 50.), Offset::new(0., 20.)));
    assert!(second.jump_to(40.));
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert!(first.jump_to(0.));
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert_eq!(first.offset(), 0.);
    assert_eq!(second.offset(), 40.);
}

#[test]
fn grid_view_padding_and_cache_lifetime() {
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        GridView::count(2, grid_boxes(4))
            .padding(EdgeInsets::all(10.))
            .cache_extent(0.)
            .into(),
        200.,
        200.,
    );
    // Padding offsets the first row; zero cache keeps the two rows.
    assert_eq!(child_origins(&tree, root)[0], Offset::new(10., 10.));
    // Widening the cache on a large grid materializes more rows, and
    // narrowing it releases them with unmount accounting.
    let mut large = WidgetTree::new();
    let big = mount_tight(
        &mut large,
        GridView::builder(100, SliverGridDelegate::fixed_cross_axis_count(2), |_| {
            Widget::box_(Size::new(80., 40.), Color::WHITE)
        })
        .cache_extent(0.)
        .into(),
        100.,
        100.,
    );
    assert_eq!(large.children(big).expect("rows").len(), 2);
    large
        .update(
            big,
            GridView::builder(100, SliverGridDelegate::fixed_cross_axis_count(2), |_| {
                Widget::box_(Size::new(80., 40.), Color::WHITE)
            })
            .cache_extent(500.)
            .into(),
        )
        .expect("update");
    large
        .layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert_eq!(large.children(big).expect("rows").len(), 12);
    let unmounted = large.diagnostics().items_unmounted;
    large
        .update(
            big,
            GridView::builder(100, SliverGridDelegate::fixed_cross_axis_count(2), |_| {
                Widget::box_(Size::new(80., 40.), Color::WHITE)
            })
            .cache_extent(0.)
            .into(),
        )
        .expect("update");
    large
        .layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert_eq!(large.children(big).expect("rows").len(), 2);
    assert!(large.diagnostics().items_unmounted > unmounted);
}

#[test]
fn scroll_viewports_always_clip() {
    // Viewport clipping is unconditional: with no per-view override left,
    // both viewports still emit exactly one viewport clip.
    for widget in [
        GridView::count(2, grid_boxes(4)).into(),
        SingleChildScrollView::new(Widget::box_(Size::new(80., 300.), Color::WHITE)).into(),
    ] {
        let mut tree = WidgetTree::new();
        tree.mount(widget).expect("mount");
        tree.layout(Constraints::tight(Size::new(100., 100.)))
            .expect("layout");
        let clips = tree
            .paint()
            .commands()
            .iter()
            .filter(|command| matches!(command, PaintCommand::PushClip { .. }))
            .count();
        assert_eq!(clips, 1);
    }
}

#[test]
fn animated_list_mounts_items_at_default_extent() {
    // The animated collection lays resting items out at its 48px default
    // item extent.
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        AnimatedList::new(3, |_| Widget::box_(Size::new(80., 40.), Color::WHITE)).into(),
        200.,
        200.,
    );
    assert_eq!(
        child_origins(&tree, root),
        vec![
            Offset::new(0., 0.),
            Offset::new(0., 48.),
            Offset::new(0., 96.)
        ]
    );
}

#[test]
fn animated_list_insert_and_remove_update_items() {
    let controller = AnimatedListController::new(2);
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        AnimatedList::new(2, |_| Widget::box_(Size::new(80., 40.), Color::WHITE))
            .controller(controller.clone())
            .into(),
        200.,
        200.,
    );
    assert_eq!(tree.children(root).expect("items").len(), 2);
    // An insert grows the structure; the incoming item starts at zero
    // extent until its animation ticks.
    controller.insert(1, Instant::now());
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(controller.item_count(), 3);
    let kids = tree.children(root).expect("items").to_vec();
    assert_eq!(kids.len(), 3);
    assert_eq!(
        tree.element_bounds(kids[1]).expect("bounds").size.height,
        0.
    );
    // A remove keeps the outgoing item mounted while dropping the count.
    assert!(controller.remove(0, Instant::now()).is_some());
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(controller.item_count(), 2);
    assert_eq!(tree.children(root).expect("items").len(), 3);
}

#[test]
fn animated_list_controller_replacement_isolates_old() {
    let first = AnimatedListController::new(2);
    let second = AnimatedListController::new(3);
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        AnimatedList::new(2, |_| Widget::box_(Size::new(80., 40.), Color::WHITE))
            .controller(first.clone())
            .into(),
        200.,
        200.,
    );
    assert_eq!(tree.children(root).expect("items").len(), 2);
    tree.update(
        root,
        AnimatedList::new(2, |_| Widget::box_(Size::new(80., 40.), Color::WHITE))
            .controller(second.clone())
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    // The viewport follows the attached controller's three items; edits
    // through the detached handle no longer reach it.
    assert_eq!(tree.children(root).expect("items").len(), 3);
    first.insert(0, Instant::now());
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(first.item_count(), 3);
    assert_eq!(second.item_count(), 3);
    assert_eq!(tree.children(root).expect("items").len(), 3);
}

#[test]
fn animated_list_constructors_and_builders_agree() {
    // new, animated, and the animation/removed-item builders all
    // materialize the same three resting items.
    let animated_items = |index: usize, _: incular_widgets::AnimatedItem| {
        Widget::box_(Size::new(80., 40.), Color::WHITE).with_key(index as u64)
    };
    for widget in [
        AnimatedList::new(3, |_| Widget::box_(Size::new(80., 40.), Color::WHITE)).into(),
        AnimatedList::animated(3, |_, _| Widget::box_(Size::new(80., 40.), Color::WHITE)).into(),
        AnimatedList::with_animation_builder(3, animated_items).into(),
        AnimatedList::new(3, |_| Widget::box_(Size::new(80., 40.), Color::WHITE))
            .animation_builder(animated_items)
            .removed_item_builder(|_| Widget::box_(Size::new(80., 40.), Color::WHITE))
            .into(),
    ] {
        let mut tree = WidgetTree::new();
        let root = mount_tight(&mut tree, widget, 200., 200.);
        assert_eq!(tree.children(root).expect("items").len(), 3);
        assert_eq!(child_origins(&tree, root)[0], Offset::new(0., 0.));
    }
}

#[test]
fn animated_list_direction_reverse_cache_and_physics() {
    // Horizontal lays 48px columns; reverse with overflowing content
    // anchors the trailing edge; the never-scrollable policy gates input
    // (the scroll position itself stays internal to the animated list, so
    // only acceptance is asserted here).
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        AnimatedList::new(3, |_| Widget::box_(Size::new(80., 40.), Color::WHITE))
            .scroll_direction(Axis::Horizontal)
            .physics(ScrollPhysics::clamping().never_scrollable())
            .into(),
        200.,
        200.,
    );
    assert_eq!(
        child_origins(&tree, root),
        vec![
            Offset::new(0., 0.),
            Offset::new(48., 0.),
            Offset::new(96., 0.)
        ]
    );
    assert!(!tree.scroll_at(Offset::new(100., 100.), Offset::new(20., 0.)));

    let mut reverse = WidgetTree::new();
    let reverse_root = mount_tight(
        &mut reverse,
        AnimatedList::new(5, |_| Widget::box_(Size::new(80., 40.), Color::WHITE))
            .reverse(true)
            .into(),
        200.,
        200.,
    );
    // Five 48px items are 240px in a 200px viewport: trailing anchor.
    assert_eq!(
        child_origins(&reverse, reverse_root)[0],
        Offset::new(0., -40.)
    );

    let mut cached = WidgetTree::new();
    let cache_root = mount_tight(
        &mut cached,
        AnimatedList::new(100, |_| Widget::box_(Size::new(80., 40.), Color::WHITE))
            .controller(AnimatedListController::new(100))
            .cache_extent(0.)
            .into(),
        100.,
        100.,
    );
    assert_eq!(cached.children(cache_root).expect("items").len(), 5);
    cached
        .update(
            cache_root,
            AnimatedList::new(100, |_| Widget::box_(Size::new(80., 40.), Color::WHITE))
                .controller(AnimatedListController::new(100))
                .cache_extent(500.)
                .into(),
        )
        .expect("update");
    cached
        .layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert_eq!(cached.children(cache_root).expect("items").len(), 15);
}

#[test]
fn animated_grid_direction_reverse_cache_and_physics() {
    // Horizontal fills column-major; reverse with overflowing content
    // anchors the trailing edge; the never-scrollable policy gates input.
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        AnimatedGrid::new(4, SliverGridDelegate::fixed_cross_axis_count(2), |_| {
            Widget::box_(Size::new(80., 40.), Color::WHITE)
        })
        .scroll_direction(Axis::Horizontal)
        .physics(ScrollPhysics::clamping().never_scrollable())
        .into(),
        200.,
        200.,
    );
    assert_eq!(
        child_origins(&tree, root),
        vec![
            Offset::new(0., 0.),
            Offset::new(0., 100.),
            Offset::new(100., 0.),
            Offset::new(100., 100.)
        ]
    );
    assert!(!tree.scroll_at(Offset::new(100., 100.), Offset::new(20., 0.)));

    let mut reverse = WidgetTree::new();
    let reverse_root = mount_tight(
        &mut reverse,
        AnimatedGrid::new(6, SliverGridDelegate::fixed_cross_axis_count(2), |_| {
            Widget::box_(Size::new(80., 40.), Color::WHITE)
        })
        .reverse(true)
        .into(),
        200.,
        200.,
    );
    // Six cells are three 100px rows in a 200px viewport: trailing anchor.
    assert_eq!(
        child_origins(&reverse, reverse_root)[0],
        Offset::new(0., -100.)
    );

    let mut cached = WidgetTree::new();
    let cache_root = mount_tight(
        &mut cached,
        AnimatedGrid::new(100, SliverGridDelegate::fixed_cross_axis_count(2), |_| {
            Widget::box_(Size::new(80., 40.), Color::WHITE)
        })
        .controller(AnimatedListController::new(100))
        .cache_extent(0.)
        .into(),
        100.,
        100.,
    );
    let flat = cached.children(cache_root).expect("items").len();
    cached
        .update(
            cache_root,
            AnimatedGrid::new(100, SliverGridDelegate::fixed_cross_axis_count(2), |_| {
                Widget::box_(Size::new(80., 40.), Color::WHITE)
            })
            .controller(AnimatedListController::new(100))
            .cache_extent(500.)
            .into(),
        )
        .expect("update");
    cached
        .layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert!(cached.children(cache_root).expect("items").len() > flat);
}

#[test]
fn animated_collection_controller_tracks_structure() {
    let controller = AnimatedListController::new(2);
    assert_eq!(controller.item_count(), 2);
    let timed = AnimatedListController::with_duration(4, Duration::from_millis(300));
    assert_eq!(timed.item_count(), 4);
    assert_eq!(timed.duration(), Duration::from_millis(300));
    controller.insert(0, Instant::now());
    assert_eq!(controller.item_count(), 3);
    assert!(controller.remove(0, Instant::now()).is_some());
    assert_eq!(controller.item_count(), 2);
}

#[test]
fn animated_grid_mounts_cells_and_delegate_change() {
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        AnimatedGrid::new(4, SliverGridDelegate::fixed_cross_axis_count(2), |_| {
            Widget::box_(Size::new(80., 40.), Color::WHITE)
        })
        .into(),
        200.,
        200.,
    );
    // Unlike GridView rows, the animated grid exposes cells directly.
    assert_eq!(
        child_origins(&tree, root),
        vec![
            Offset::new(0., 0.),
            Offset::new(100., 0.),
            Offset::new(0., 100.),
            Offset::new(100., 100.)
        ]
    );
    tree.update(
        root,
        AnimatedGrid::new(4, SliverGridDelegate::fixed_cross_axis_count(3), |_| {
            Widget::box_(Size::new(80., 40.), Color::WHITE)
        })
        .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let cells = tree.children(root).expect("items").to_vec();
    assert_eq!(cells.len(), 4);
    assert_eq!(
        tree.element_bounds(cells[1]).expect("bounds").origin,
        Offset::new(200. / 3., 0.)
    );
}

#[test]
fn animated_grid_constructors_and_replacement() {
    let first = incular_widgets::AnimatedGridController::new(2);
    let second = incular_widgets::AnimatedGridController::new(4);
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        AnimatedGrid::new(2, SliverGridDelegate::fixed_cross_axis_count(2), |_| {
            Widget::box_(Size::new(80., 40.), Color::WHITE)
        })
        .controller(first.clone())
        .into(),
        200.,
        200.,
    );
    assert_eq!(tree.children(root).expect("items").len(), 2);
    for widget in [
        AnimatedGrid::animated(2, SliverGridDelegate::fixed_cross_axis_count(2), |_, _| {
            Widget::box_(Size::new(80., 40.), Color::WHITE)
        })
        .into(),
        AnimatedGrid::with_animation_builder(
            2,
            SliverGridDelegate::fixed_cross_axis_count(2),
            |_, _| Widget::box_(Size::new(80., 40.), Color::WHITE),
        )
        .into(),
        AnimatedGrid::new(2, SliverGridDelegate::fixed_cross_axis_count(2), |_| {
            Widget::box_(Size::new(80., 40.), Color::WHITE)
        })
        .animation_builder(|_, _| Widget::box_(Size::new(80., 40.), Color::WHITE))
        .removed_item_builder(|_| Widget::box_(Size::new(80., 40.), Color::WHITE))
        .into(),
    ] {
        let mut other = WidgetTree::new();
        let other_root = mount_tight(&mut other, widget, 200., 200.);
        assert_eq!(other.children(other_root).expect("items").len(), 2);
    }
    // Controller replacement swaps the attached item source.
    tree.update(
        root,
        AnimatedGrid::new(2, SliverGridDelegate::fixed_cross_axis_count(2), |_| {
            Widget::box_(Size::new(80., 40.), Color::WHITE)
        })
        .controller(second.clone())
        .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(tree.children(root).expect("items").len(), 4);
}
