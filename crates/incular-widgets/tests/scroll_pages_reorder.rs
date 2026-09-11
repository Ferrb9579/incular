//! PageView and sliver-only animated/reorderable list coverage.
//!
//! Third scrolling group: `PageView`, `SliverAnimatedList` with
//! `SliverAnimatedListController`, `SliverAnimatedGrid` with
//! `SliverAnimatedGridController`, and `SliverReorderableList` with
//! `SliverReorderController`. Offset ownership stays with the scroll
//! controllers and structure ownership with the collection controllers;
//! retained layout stays in Widgets. Drag-path reorder callbacks are
//! covered by the existing `reorderable` suite and referenced, not
//! duplicated. No second scroll engine or attachment model is added:
//! one controller across unrelated viewports stays a W5 gap.

mod common;

use common::*;
use incular_config::{Axis, Constraints};
use incular_core::{Color, Offset, Size};
use incular_scroll::ScrollPhysics;
use incular_semantics::SemanticRole;
use incular_widgets::{
    PageView, Semantics, SliverAnimatedGrid, SliverAnimatedGridController, SliverAnimatedList,
    SliverAnimatedListController, SliverGridDelegate, SliverReorderController,
    SliverReorderableList,
};
use std::time::{Duration, Instant};

fn pages() -> Vec<Widget> {
    vec![
        Widget::box_(Size::new(20., 20.), Color::WHITE),
        Widget::box_(Size::new(20., 20.), Color::BLACK),
    ]
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

fn descriptions(tree: &mut WidgetTree) -> Vec<String> {
    tree.update_semantics();
    tree.semantics()
        .iter()
        .filter_map(|(_, node)| node.description.clone())
        .collect()
}

#[test]
fn page_view_builder_defaults_match_new() {
    let mut via_builder = WidgetTree::new();
    let builder_root = mount_tight(
        &mut via_builder,
        PageView::typed_builder().children(pages()).build().into(),
        100.,
        40.,
    );
    let mut via_ctor = WidgetTree::new();
    let ctor_root = mount_tight(&mut via_ctor, PageView::new(pages()).into(), 100., 40.);
    assert_eq!(
        subtree_geometry(&via_builder, builder_root),
        subtree_geometry(&via_ctor, ctor_root)
    );
}

#[test]
fn page_view_pages_fill_viewport() {
    let mut tree = WidgetTree::new();
    let root = mount_tight(&mut tree, PageView::new(pages()).into(), 100., 40.);
    let kids = tree.children(root).expect("pages").to_vec();
    assert_eq!(kids.len(), 2);
    assert_eq!(
        tree.element_bounds(kids[0]).expect("bounds").size,
        Size::new(100., 40.)
    );
    assert_eq!(
        tree.element_bounds(kids[1]).expect("bounds").origin,
        Offset::new(100., 0.)
    );
}

#[test]
fn page_view_viewport_fraction_sizes_pages() {
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        PageView::new(pages()).viewport_fraction(0.5).into(),
        100.,
        40.,
    );
    let kids = tree.children(root).expect("pages").to_vec();
    assert_eq!(
        tree.element_bounds(kids[0]).expect("bounds").size,
        Size::new(50., 40.)
    );
    assert_eq!(
        tree.element_bounds(kids[1]).expect("bounds").origin,
        Offset::new(50., 0.)
    );
    // Degenerate fractions floor at 0.01 rather than collapsing.
    assert_eq!(
        PageView::new(pages())
            .viewport_fraction(0.)
            .viewport_fraction_value(),
        0.01
    );
}

#[test]
fn page_view_builder_matches_children() {
    let mut via_builder = WidgetTree::new();
    let builder_root = mount_tight(
        &mut via_builder,
        PageView::builder(2, |_| Widget::box_(Size::new(20., 20.), Color::WHITE)).into(),
        100.,
        40.,
    );
    assert_eq!(via_builder.children(builder_root).expect("pages").len(), 2);
}

#[test]
fn page_view_controller_replacement_isolates_old() {
    let first = ScrollController::new();
    let second = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        PageView::new(pages()).controller(first.clone()).into(),
        100.,
        40.,
    );
    assert_eq!(first.max_offset(), 100.);
    assert!(first.jump_to(100.));
    tree.layout(Constraints::tight(Size::new(100., 40.)))
        .expect("layout");
    let kids = tree.children(root).expect("pages").to_vec();

    tree.update(
        root,
        PageView::new(pages()).controller(second.clone()).into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 40.)))
        .expect("layout");
    assert_eq!(second.offset(), 0.);
    assert!(second.jump_to(100.));
    tree.layout(Constraints::tight(Size::new(100., 40.)))
        .expect("layout");
    assert!(first.jump_to(0.));
    tree.layout(Constraints::tight(Size::new(100., 40.)))
        .expect("layout");
    assert_eq!(first.offset(), 0.);
    assert_eq!(second.offset(), 100.);
    assert_eq!(tree.children(root).expect("pages").to_vec(), kids);
}

#[test]
fn page_view_reverse_change_anchors_trailing_edge() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        PageView::new(pages()).controller(controller.clone()).into(),
        200.,
        40.,
    );
    let kids = tree.children(root).expect("pages").to_vec();
    assert_eq!(
        tree.element_bounds(kids[0]).expect("bounds").origin,
        Offset::new(0., 0.)
    );
    tree.update(
        root,
        PageView::new(pages())
            .reverse(true)
            .controller(controller.clone())
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 40.)))
        .expect("layout");
    // Two 200px pages in a 200px viewport: trailing anchor at -200.
    assert_eq!(
        tree.element_bounds(kids[0]).expect("bounds").origin,
        Offset::new(-200., 0.)
    );
    assert!(controller.jump_to(200.));
    tree.layout(Constraints::tight(Size::new(200., 40.)))
        .expect("layout");
    assert_eq!(
        tree.element_bounds(kids[0]).expect("bounds").origin,
        Offset::new(0., 0.)
    );
}

#[test]
fn page_view_axis_change_reflows() {
    let mut tree = WidgetTree::new();
    let root = mount_tight(&mut tree, PageView::new(pages()).into(), 100., 40.);
    let kids = tree.children(root).expect("pages").to_vec();
    assert_eq!(
        tree.element_bounds(kids[1]).expect("bounds").origin,
        Offset::new(100., 0.)
    );
    tree.update(
        root,
        PageView::new(pages())
            .scroll_direction(Axis::Vertical)
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 40.)))
        .expect("layout");
    assert_eq!(
        tree.element_bounds(kids[1]).expect("bounds").origin,
        Offset::new(0., 40.)
    );
}

#[test]
fn page_view_resize_retains_offset_and_pages() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        PageView::new(pages()).controller(controller.clone()).into(),
        100.,
        40.,
    );
    assert!(controller.jump_to(100.));
    tree.layout(Constraints::tight(Size::new(200., 40.)))
        .expect("layout");
    // The offset survives; the wider viewport widens the range and pages.
    assert_eq!(controller.offset(), 100.);
    assert_eq!(controller.max_offset(), 200.);
    let kids = tree.children(root).expect("pages").to_vec();
    for kid in &kids {
        assert!(tree.element_exists(*kid));
    }
    assert_eq!(
        tree.element_bounds(kids[0]).expect("bounds").size,
        Size::new(200., 40.)
    );
}

#[test]
fn page_view_snapping_settles_and_releases() {
    // Snapping pages settle a mid-page offset to the boundary while a
    // non-snapping view keeps it; the snap policy rides the viewport.
    let snapping = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        PageView::new([
            Widget::box_(Size::new(20., 20.), Color::WHITE),
            Widget::box_(Size::new(20., 20.), Color::BLACK),
            Widget::box_(Size::new(20., 20.), Color::WHITE),
        ])
        .controller(snapping.clone())
        .into(),
        100.,
        40.,
    );
    assert!(snapping.jump_to(30.));
    assert!(snapping.settle_physics(ScrollPhysics::clamping().page(), 0.));
    assert_eq!(snapping.offset(), 0.);
    let render = tree.render_id(root).expect("render");
    assert!(matches!(
        tree.render_object_kind(render).expect("kind"),
        incular_widgets::internal::RenderKind::SliverViewport { .. }
    ));

    let plain = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        PageView::new(pages())
            .page_snapping(false)
            .controller(plain.clone())
            .into(),
        100.,
        40.,
    );
    assert!(plain.jump_to(30.));
    assert!(!plain.settle_physics(ScrollPhysics::clamping(), 0.));
    assert_eq!(plain.offset(), 30.);
}

#[test]
fn page_view_page_identity_survives_updates() {
    let mut tree = WidgetTree::new();
    let root = mount_tight(&mut tree, PageView::new(pages()).into(), 100., 40.);
    let kids = tree.children(root).expect("pages").to_vec();
    tree.update(root, PageView::new(pages()).viewport_fraction(0.5).into())
        .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 40.)))
        .expect("layout");
    // Same pages resized, not rebuilt.
    assert_eq!(tree.children(root).expect("pages").to_vec(), kids);
}

#[test]
fn page_view_unmount_releases_pages() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(incular_widgets::Container::with_child(PageView::new(pages())).into())
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(100., 40.)))
        .expect("layout");
    let before = tree.diagnostics().elements_removed;
    tree.update(
        root,
        incular_widgets::Container::with_child(Widget::box_(Size::new(100., 40.), Color::WHITE))
            .into(),
    )
    .expect("unmount");
    tree.layout(Constraints::tight(Size::new(100., 40.)))
        .expect("layout");
    assert!(tree.diagnostics().elements_removed > before);
}

#[test]
fn sliver_animated_list_controller_tracks_structure() {
    let controller = SliverAnimatedListController::new(3);
    assert_eq!(controller.item_count(), 3);
    let timed = SliverAnimatedListController::with_duration(2, Duration::from_millis(300));
    assert_eq!(timed.item_count(), 2);
    assert_eq!(timed.duration(), Duration::from_millis(300));
    let start_revision = controller.revision();
    assert!(controller.insert(1));
    assert!(controller.is_animating());
    assert_eq!(controller.item_count(), 4);
    assert!(controller.revision() > start_revision);
    assert!(controller.tick(Instant::now() + Duration::from_secs(60)));
    assert!(!controller.is_animating());
    assert!(controller.remove(0));
    assert_eq!(controller.item_count(), 4);
    assert!(controller.tick(Instant::now() + Duration::from_secs(120)));
    assert_eq!(controller.item_count(), 3);
}

#[test]
fn sliver_animated_list_insert_tick_remove_releases() {
    let controller = SliverAnimatedListController::new(3);
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            CustomScrollView::new(vec![Box::new(
                SliverAnimatedList::new(3, |index| {
                    Semantics::new(Widget::box_(Size::new(80., 40.), Color::WHITE))
                        .role(SemanticRole::Group)
                        .label(["item-zero", "item-one", "item-two"][index])
                })
                .controller(controller.clone()),
            ) as Box<dyn Sliver>])
            .into(),
        )
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(100., 200.)))
        .expect("layout");
    assert_eq!(tree.children(root).expect("items").len(), 3);

    let start = Instant::now();
    assert!(controller.insert_at(1, start));
    tree.layout(Constraints::tight(Size::new(100., 200.)))
        .expect("layout");
    // The incoming item mounts and animates; the count grows.
    assert_eq!(controller.item_count(), 4);
    assert_eq!(tree.children(root).expect("items").len(), 4);
    assert!(controller.tick(start + Duration::from_secs(60)));
    tree.layout(Constraints::tight(Size::new(100., 200.)))
        .expect("layout");
    assert!(!controller.is_animating());

    // Removing keeps the outgoing item mounted through its exit lifetime.
    let removed = tree.children(root).expect("items")[0];
    assert!(controller.remove_at(0, start + Duration::from_secs(60)));
    tree.layout(Constraints::tight(Size::new(100., 200.)))
        .expect("layout");
    assert_eq!(controller.item_count(), 4);
    assert!(tree.element_exists(removed));
    // Past the exit lifetime the item releases everywhere: no element,
    // no semantic node, and no hit target.
    assert!(controller.tick(start + Duration::from_secs(120)));
    tree.layout(Constraints::tight(Size::new(100., 200.)))
        .expect("layout");
    assert_eq!(controller.item_count(), 3);
    assert_eq!(tree.children(root).expect("items").len(), 3);
    assert!(!tree.element_exists(removed));
    assert!(
        !descriptions(&mut tree)
            .iter()
            .any(|description| description.contains("item-zero"))
    );
    let hit = tree
        .hit_test(Offset::new(40., 180.))
        .and_then(|render| tree.element_for_render(render));
    assert_ne!(hit, Some(removed));
}

#[test]
fn sliver_animated_list_controller_replacement_isolates_old() {
    let first = SliverAnimatedListController::new(2);
    let second = SliverAnimatedListController::new(3);
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            CustomScrollView::new(vec![Box::new(
                SliverAnimatedList::new(2, |_| Widget::box_(Size::new(80., 40.), Color::WHITE))
                    .controller(first.clone()),
            ) as Box<dyn Sliver>])
            .into(),
        )
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(100., 200.)))
        .expect("layout");
    assert_eq!(tree.children(root).expect("items").len(), 2);
    tree.update(
        root,
        CustomScrollView::new(vec![Box::new(
            SliverAnimatedList::new(2, |_| Widget::box_(Size::new(80., 40.), Color::WHITE))
                .controller(second.clone()),
        ) as Box<dyn Sliver>])
        .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 200.)))
        .expect("layout");
    // The viewport follows the attached controller; edits through the
    // detached handle never reach it.
    assert_eq!(tree.children(root).expect("items").len(), 3);
    first.insert(0);
    tree.layout(Constraints::tight(Size::new(100., 200.)))
        .expect("layout");
    assert_eq!(first.item_count(), 3);
    assert_eq!(second.item_count(), 3);
    assert_eq!(tree.children(root).expect("items").len(), 3);
}

#[test]
fn sliver_animated_grid_insert_tick_and_replacement() {
    let controller = SliverAnimatedGridController::new(4);
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            CustomScrollView::new(vec![Box::new(
                SliverAnimatedGrid::new(4, SliverGridDelegate::fixed_cross_axis_count(2), |_| {
                    Widget::box_(Size::new(80., 40.), Color::WHITE)
                })
                .controller(controller.clone()),
            ) as Box<dyn Sliver>])
            .into(),
        )
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(tree.children(root).expect("items").len(), 4);
    let start = Instant::now();
    controller.insert(0, start);
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(tree.children(root).expect("items").len(), 5);
    controller.tick(start + Duration::from_secs(60));
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert!(!controller.is_animating());

    let replacement = SliverAnimatedGridController::new(2);
    tree.update(
        root,
        CustomScrollView::new(vec![Box::new(
            SliverAnimatedGrid::new(4, SliverGridDelegate::fixed_cross_axis_count(2), |_| {
                Widget::box_(Size::new(80., 40.), Color::WHITE)
            })
            .controller(replacement.clone()),
        ) as Box<dyn Sliver>])
        .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(tree.children(root).expect("items").len(), 2);
}

#[test]
fn sliver_reorder_controller_tracks_order() {
    let controller = SliverReorderController::new(4);
    assert_eq!(controller.order(), vec![0, 1, 2, 3]);
    assert_eq!(controller.item_at(2), Some(2));
    assert_eq!(controller.position_of(2), Some(2));
    let start_revision = controller.revision();
    assert!(controller.reorder(0, 2));
    assert_eq!(controller.order(), vec![1, 0, 2, 3]);
    assert_eq!(controller.item_at(0), Some(1));
    assert_eq!(controller.position_of(0), Some(1));
    assert!(controller.revision() > start_revision);
    assert!(controller.move_item(3, 0));
    assert_eq!(controller.order(), vec![3, 1, 0, 2]);
    controller.set_item_count(2);
    assert_eq!(controller.order(), vec![3, 1]);
}

#[test]
fn sliver_reorderable_list_reorder_permutes_identity() {
    let controller = SliverReorderController::new(4);
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            CustomScrollView::new(vec![Box::new(
                SliverReorderableList::new(4, |item| {
                    Widget::box_(Size::new(80., 40.), Color::rgba(item as u8 * 60, 0, 0, 255))
                        .with_key(item as u64)
                })
                .controller(controller.clone()),
            ) as Box<dyn Sliver>])
            .into(),
        )
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(100., 200.)))
        .expect("layout");
    let before = tree.children(root).expect("items").to_vec();
    assert_eq!(before.len(), 4);

    // Reordering permutes retained identities in place: the same element
    // set moves slots, so logical item identity — not position — owns
    // each element.
    assert!(controller.reorder(0, 2));
    tree.layout(Constraints::tight(Size::new(100., 200.)))
        .expect("layout");
    let after = tree.children(root).expect("items").to_vec();
    assert_eq!(after.len(), 4);
    let mut sorted_before = before.clone();
    let mut sorted_after = after.clone();
    sorted_before.sort();
    sorted_after.sort();
    assert_eq!(sorted_before, sorted_after);
    assert_eq!(after[0], before[1]);
    assert_eq!(after[1], before[0]);
    // Drag-path callbacks stay covered by the existing reorderable suite.
}

#[test]
fn sliver_reorderable_list_extents_and_replacement() {
    // Fixed, per-index, and prototype extents all resolve; the last setter
    // wins and the others clear.
    for list in [
        SliverReorderableList::new(3, |_| Widget::box_(Size::new(80., 40.), Color::WHITE))
            .item_extent(50.),
        SliverReorderableList::new(3, |_| Widget::box_(Size::new(80., 40.), Color::WHITE))
            .item_extent_builder(|index| 40. + index as f32 * 10.),
        SliverReorderableList::new(3, |_| Widget::box_(Size::new(80., 40.), Color::WHITE))
            .prototype_item(Widget::box_(Size::new(80., 60.), Color::WHITE)),
    ] {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(CustomScrollView::new(vec![Box::new(list) as Box<dyn Sliver>]).into())
            .expect("mount");
        tree.layout(Constraints::tight(Size::new(100., 300.)))
            .expect("layout");
        assert_eq!(tree.children(root).expect("items").len(), 3);
    }
    // Controller replacement swaps the attached order source.
    let first = SliverReorderController::new(3);
    let second = SliverReorderController::new(2);
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            CustomScrollView::new(vec![Box::new(
                SliverReorderableList::new(3, |_| Widget::box_(Size::new(80., 40.), Color::WHITE))
                    .controller(first.clone()),
            ) as Box<dyn Sliver>])
            .into(),
        )
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(100., 300.)))
        .expect("layout");
    assert_eq!(tree.children(root).expect("items").len(), 3);
    tree.update(
        root,
        CustomScrollView::new(vec![Box::new(
            SliverReorderableList::new(3, |_| Widget::box_(Size::new(80., 40.), Color::WHITE))
                .controller(second.clone()),
        ) as Box<dyn Sliver>])
        .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 300.)))
        .expect("layout");
    assert_eq!(tree.children(root).expect("items").len(), 2);
    first.set_item_count(5);
    tree.layout(Constraints::tight(Size::new(100., 300.)))
        .expect("layout");
    assert_eq!(tree.children(root).expect("items").len(), 2);
}
