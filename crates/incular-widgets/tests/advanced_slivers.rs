#![allow(clippy::float_cmp)]

use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

use incular_config::Axis;
use incular_core::{Color, Size};
use incular_scroll::SliverConstraints;
use incular_widgets::{
    AnimatedGrid, AnimatedItemPhase, AnimatedList, AnimatedListController, AutomaticKeepAlive,
    KeepAlive, KeepAliveHandle, KeepAliveNotification, KeepAliveRegistry, SliverAnimatedGrid,
    TreeRowAnimation, TreeSliver, TreeSliverIndentation, TreeSliverNode,
};
use incular_widgets::{Sliver, SliverGridDelegate, Widget};

fn constraints() -> SliverConstraints {
    SliverConstraints::new(
        Axis::Vertical,
        false,
        0.0,
        0.0,
        0.0,
        200.0,
        100.0,
        200.0,
        400.0,
        0.0,
    )
}

fn fixed_child(width: f32, height: f32) -> Widget {
    Widget::box_(Size::new(width, height), Color::WHITE)
}

#[test]
fn tree_sliver_flattens_active_hierarchy_with_stable_semantics_and_indent() {
    let leaf = TreeSliverNode::new("leaf");
    let child = TreeSliverNode::new("child")
        .with_children([leaf])
        .expanded(true);
    let root = TreeSliverNode::new("root")
        .with_children([child])
        .expanded(false);
    let root_id = root.id();
    let child_id = root.children()[0].id();
    let now = Instant::now();
    let tree = TreeSliver::new(vec![root], |_node, _animation: TreeRowAnimation| {
        fixed_child(100.0, 40.0)
    })
    .row_extent(40.0)
    .indentation(TreeSliverIndentation::Fixed(15.0))
    .duration(Duration::from_millis(100));
    let controller = tree.controller();
    let mut render = tree.create_render_sliver(&Default::default(), Axis::Vertical, false);

    let initial = render.perform_layout(constraints());
    assert_eq!(controller.active_count(), 1);
    assert_eq!(initial.children.len(), 1);
    assert_eq!(initial.children[0].id.0, root_id.0);
    assert_eq!(initial.children[0].semantic_index, Some(0));

    assert!(controller.expand_node_at(root_id, now));
    assert_eq!(controller.active_count(), 3);
    controller.tick(now + Duration::from_millis(50));
    let halfway = render.perform_layout(constraints());
    let child_layout = halfway
        .children
        .iter()
        .find(|child| child.id.0 == child_id.0)
        .expect("expanded child is retained during the transition");
    assert_eq!(child_layout.semantic_index, Some(1));
    assert_eq!(child_layout.cross_offset, 15.0);
    assert_eq!(child_layout.offset, 20.0);
    assert_eq!(halfway.geometry.scroll_extent, 100.0);

    controller.tick(now + Duration::from_millis(100));
    let expanded = render.perform_layout(constraints());
    assert_eq!(expanded.geometry.scroll_extent, 120.0);
    assert_eq!(expanded.children[1].offset, 40.0);
    assert_eq!(expanded.children[2].semantic_index, Some(2));

    assert!(controller.collapse_node_at(root_id, now + Duration::from_millis(100)));
    controller.tick(now + Duration::from_millis(150));
    let collapsing = render.perform_layout(constraints());
    assert_eq!(collapsing.children.len(), 3);
    assert_eq!(collapsing.geometry.scroll_extent, 100.0);
    controller.tick(now + Duration::from_millis(200));
    let collapsed = render.perform_layout(constraints());
    assert_eq!(collapsed.children.len(), 1);
    assert_eq!(collapsed.geometry.scroll_extent, 40.0);
}

#[test]
fn animated_list_keeps_physical_slots_until_removal_completes_and_reuses_ids() {
    let controller = AnimatedListController::with_duration(3, Duration::from_millis(100));
    let list = AnimatedList::animated(3, |_index, _item| fixed_child(100.0, 20.0))
        .controller(controller.clone());
    let mut render = list.create_render_sliver(&Default::default(), Axis::Vertical, false);
    let now = Instant::now();
    let first = render.perform_layout(constraints());
    for child in &first.children {
        assert!(render.set_child_extent(child.id, 20.0));
    }
    let measured = render.perform_layout(constraints());
    let stable_ids = measured
        .children
        .iter()
        .map(|child| child.id.0)
        .collect::<Vec<_>>();
    assert_eq!(measured.geometry.scroll_extent, 60.0);

    let inserted_id = controller.insert_at(1, now);
    assert_eq!(controller.item_count(), 4);
    assert_eq!(controller.physical_item_count(), 4);
    assert_eq!(controller.entries()[1].id, inserted_id);
    assert_eq!(controller.entries()[1].phase, AnimatedItemPhase::Incoming);
    let at_start = render.perform_layout(constraints());
    assert_eq!(at_start.geometry.scroll_extent, 60.0);
    assert_eq!(at_start.children[2].id.0, stable_ids[1]);

    controller.tick(now + Duration::from_millis(50));
    let halfway = render.perform_layout(constraints());
    assert_eq!(halfway.geometry.scroll_extent, 70.0);
    assert_eq!(halfway.children[2].offset, 30.0);

    let removed_builder_calls = Rc::new(RefCell::new(0usize));
    let calls = removed_builder_calls.clone();
    let removed_id = controller
        .remove_at_with_builder(2, now + Duration::from_millis(50), move |_item| {
            *calls.borrow_mut() += 1;
            fixed_child(100.0, 20.0)
        })
        .expect("a live logical item can be removed");
    assert_eq!(controller.item_count(), 3);
    assert_eq!(controller.physical_item_count(), 4);
    assert!(
        controller
            .entries()
            .iter()
            .any(|item| item.id == removed_id && item.is_outgoing())
    );
    let removing = render.perform_layout(constraints());
    assert_eq!(removing.children.len(), 4);
    assert!(*removed_builder_calls.borrow() > 0);

    controller.tick(now + Duration::from_millis(150));
    let finished = render.perform_layout(constraints());
    assert_eq!(controller.item_count(), 3);
    assert_eq!(controller.physical_item_count(), 3);
    assert!(
        !finished
            .children
            .iter()
            .any(|child| child.id.0 == removed_id)
    );
    assert_eq!(finished.children[1].id.0, inserted_id);
}

#[test]
fn animated_grid_and_sliver_animated_grid_use_tile_geometry_for_each_stable_item() {
    let delegate = SliverGridDelegate::fixed_cross_axis_count(2)
        .cross_axis_spacing(5.0)
        .main_axis_spacing(7.0)
        .main_axis_extent(30.0);
    let grid = AnimatedGrid::animated(3, delegate.clone(), |_index, _item| fixed_child(40.0, 30.0));
    let mut render = grid.create_render_sliver(&Default::default(), Axis::Vertical, false);
    let layout = render.perform_layout(constraints());
    assert_eq!(layout.children.len(), 3);
    assert_eq!(layout.geometry.scroll_extent, 67.0);
    assert_eq!(layout.children[0].offset, 0.0);
    assert_eq!(layout.children[1].offset, 0.0);
    assert_eq!(layout.children[1].cross_offset, 52.5);
    assert_eq!(layout.children[2].offset, 37.0);

    let sliver = SliverAnimatedGrid::animated(4, delegate, |_index, item| {
        assert!(item.value >= 0.0 && item.value <= 1.0);
        fixed_child(40.0, 30.0)
    });
    let _ = sliver.delegate();
    let mut sliver_render = sliver.create_render_sliver(&Default::default(), Axis::Vertical, false);
    let sliver_layout = sliver_render.perform_layout(constraints());
    assert_eq!(sliver_layout.children.len(), 4);
    assert_eq!(sliver_layout.geometry.scroll_extent, 67.0);
}

#[test]
fn keep_alive_notifications_bubble_and_release_only_their_own_retention() {
    let registry = KeepAliveRegistry::new();
    let first = KeepAliveHandle::new();
    let second = KeepAliveHandle::new();
    assert!(!registry.dispatch(KeepAliveNotification::new(first.clone())));
    assert!(!registry.dispatch(KeepAliveNotification::new(second.clone())));
    assert_eq!(registry.active_count(), 2);
    assert!(registry.keeping_alive());

    first.release();
    first.release();
    assert_eq!(registry.active_count(), 1);
    assert!(registry.keeping_alive());
    second.release();
    assert_eq!(registry.active_count(), 0);
    assert!(!registry.keeping_alive());
    assert!(first.is_released());
    assert!(second.is_released());

    let automatic = AutomaticKeepAlive::with_registry(fixed_child(100.0, 20.0), registry.clone());
    let third = KeepAliveHandle::new();
    assert!(!automatic.keeping_alive());
    assert!(!automatic.on_notification(KeepAliveNotification::new(third.clone())));
    assert!(automatic.keeping_alive());
    third.release();
    assert!(!automatic.keeping_alive());

    let keep_alive = KeepAlive::new(true, fixed_child(100.0, 20.0));
    assert!(keep_alive.keep_alive());
    assert!(keep_alive.request().keep_alive);
}
