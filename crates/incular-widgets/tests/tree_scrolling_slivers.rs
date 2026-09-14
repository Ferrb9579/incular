//! Scrolling and sliver tests.

mod common;

use common::*;
use incular_config::{Axis, Constraints};
use incular_core::{Color, Offset, Size};
use incular_scroll::ScrollPhysics;
use incular_semantics::SemanticRole;
use incular_widgets::internal::*;
use serde_json::json;
use std::{cell::Cell, rc::Rc, time::Instant};

#[test]
fn scroll_positions_are_logical_clamped_and_clip_the_viewport() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    tree.mount(
        incular_widgets::SingleChildScrollView::new(incular_widgets::Column::new(
            (0..5)
                .map(|_| Widget::box_(Size::new(80., 40.), Color::WHITE))
                .collect::<Vec<_>>(),
        ))
        .controller(controller.clone())
        .into(),
    )
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    tree.update_compositor(Instant::now())
        .expect("compositor update");
    assert_eq!(controller.max_offset(), 100.);
    assert_eq!(
        rect_origins(&tree.paint())
            .into_iter()
            .filter(|origin| origin.x < 80.)
            .collect::<Vec<_>>(),
        vec![
            Offset::new(0., 0.),
            Offset::new(0., 40.),
            Offset::new(0., 80.),
        ]
    );
    assert!(controller.jump_to(50.));
    tree.update_compositor(Instant::now())
        .expect("compositor update");
    assert_eq!(
        rect_origins(&tree.paint())
            .into_iter()
            .filter(|origin| origin.x < 80.)
            .collect::<Vec<_>>(),
        vec![
            Offset::new(0., -10.),
            Offset::new(0., 30.),
            Offset::new(0., 70.),
        ]
    );
    assert!(controller.jump_to(10_000.));
    assert_eq!(controller.offset(), 100.);
    tree.update_compositor(Instant::now())
        .expect("compositor update");
    assert_eq!(
        rect_origins(&tree.paint())
            .into_iter()
            .filter(|origin| origin.x < 80.)
            .collect::<Vec<_>>(),
        vec![
            Offset::new(0., -20.),
            Offset::new(0., 20.),
            Offset::new(0., 60.),
        ]
    );
    assert!(controller.jump_to(-1.));
    assert_eq!(controller.offset(), 0.);
}

#[test]
fn horizontal_scroll_preserves_axis_in_layout_transform_and_hit_testing() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            incular_widgets::internal::SingleChildScrollView::new(incular_widgets::Row::new(
                (0..5)
                    .map(|_| Widget::box_(Size::new(40., 30.), Color::WHITE))
                    .collect::<Vec<_>>(),
            ))
            .scroll_direction(Axis::Horizontal)
            .controller(controller.clone())
            .into(),
        )
        .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 40.)))
        .expect("layout");
    tree.update_compositor(Instant::now())
        .expect("compositor update");
    assert_eq!(controller.max_offset(), 100.);
    assert_eq!(
        tree.render_size(tree.render_id(root).unwrap()),
        Some(Size::new(100., 40.))
    );
    assert_eq!(
        rect_origins(&tree.paint())
            .into_iter()
            .filter(|origin| origin.y < 40.)
            .collect::<Vec<_>>(),
        vec![
            Offset::new(0., 0.),
            Offset::new(40., 0.),
            Offset::new(80., 0.)
        ]
    );
    assert!(controller.jump_to(20.));
    tree.update_compositor(Instant::now())
        .expect("compositor update");
    assert_eq!(
        rect_origins(&tree.paint())
            .into_iter()
            .filter(|origin| origin.y < 40.)
            .collect::<Vec<_>>(),
        vec![
            Offset::new(-20., 0.),
            Offset::new(20., 0.),
            Offset::new(60., 0.)
        ]
    );
    // The retained hit test uses the same horizontal content transform.
    let row = tree.children(root).unwrap()[0];
    let second = tree.children(row).unwrap()[1];
    assert_eq!(
        tree.element_for_render(tree.hit_test(Offset::new(30., 10.)).unwrap()),
        Some(second)
    );
    // Wheel input follows the configured axis; a horizontal delta must
    // not be discarded in favor of the vertical component.
    assert!(tree.scroll_at(Offset::new(30., 10.), Offset::new(20., 0.)));
    assert_eq!(controller.offset(), 40.);
}

#[test]
fn reverse_scroll_starts_at_the_physical_trailing_edge() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            incular_widgets::internal::ListView::builder(5, |_| {
                Widget::box_(Size::new(80., 40.), Color::WHITE)
            })
            .item_extent(40.)
            .reverse(true)
            .controller(controller.clone())
            .into(),
        )
        .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    tree.update_compositor(Instant::now())
        .expect("compositor update");
    assert_eq!(controller.max_offset(), 100.);
    let children = tree.children(root).unwrap().to_vec();
    assert_eq!(children.len(), 5);
    // Logical offset zero maps to the physical end for reverse views.
    assert_eq!(
        tree.render_origin(tree.render_id(children[0]).unwrap()).y,
        -100.
    );
    assert_eq!(
        tree.render_origin(tree.render_id(children[4]).unwrap()).y,
        60.
    );
    assert!(controller.jump_to(100.));
    tree.update_compositor(Instant::now())
        .expect("compositor update");
    assert_eq!(
        tree.render_origin(tree.render_id(children[0]).unwrap()).y,
        0.
    );
    assert_eq!(
        tree.render_origin(tree.render_id(children[4]).unwrap()).y,
        160.
    );
}

#[test]
fn page_view_static_children_fill_viewport_and_preserve_reverse_direction() {
    let controller = incular_widgets::PageController::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            incular_widgets::internal::PageView::new([
                Widget::box_(Size::new(20., 20.), Color::WHITE),
                Widget::box_(Size::new(20., 20.), Color::BLACK),
            ])
            .scroll_direction(Axis::Horizontal)
            .controller(controller.clone())
            .into(),
        )
        .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 40.)))
        .expect("layout");
    assert_eq!(controller.max_offset(), 100.);
    let pages = tree.children(root).expect("page children");
    assert_eq!(
        tree.render_size(tree.render_id(pages[0]).unwrap()),
        Some(Size::new(100., 40.))
    );
    assert_eq!(
        tree.render_size(tree.render_id(pages[1]).unwrap()),
        Some(Size::new(100., 40.))
    );
    assert!(controller.jump_to(100.));
    assert_eq!(
        tree.render_origin(tree.render_id(pages[0]).unwrap()).x,
        -100.
    );
    assert_eq!(tree.render_origin(tree.render_id(pages[1]).unwrap()).x, 0.);

    let reverse_controller = incular_widgets::PageController::new();
    let mut reverse_tree = WidgetTree::new();
    let reverse_root = reverse_tree
        .mount(
            incular_widgets::internal::PageView::new([
                Widget::box_(Size::new(20., 20.), Color::WHITE),
                Widget::box_(Size::new(20., 20.), Color::BLACK),
            ])
            .scroll_direction(Axis::Horizontal)
            .reverse(true)
            .controller(reverse_controller.clone())
            .into(),
        )
        .unwrap();
    reverse_tree
        .layout(Constraints::tight(Size::new(100., 40.)))
        .expect("layout");
    let reverse_pages = reverse_tree
        .children(reverse_root)
        .expect("reverse page children");
    assert_eq!(
        reverse_tree
            .render_origin(reverse_tree.render_id(reverse_pages[0]).unwrap())
            .x,
        -100.
    );
    assert_eq!(
        reverse_tree
            .render_origin(reverse_tree.render_id(reverse_pages[1]).unwrap())
            .x,
        0.
    );
}

#[test]
fn scroll_physics_survives_descriptor_lowering_and_controls_input() {
    let controller = ScrollController::new();
    let physics = ScrollPhysics::clamping().never_scrollable();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            incular_widgets::internal::SingleChildScrollView::new(incular_widgets::Column::new(
                (0..4)
                    .map(|_| Widget::box_(Size::new(80., 40.), Color::WHITE))
                    .collect::<Vec<_>>(),
            ))
            .physics(physics)
            .controller(controller.clone())
            .into(),
        )
        .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    let render = tree.render_id(root).unwrap();
    assert!(matches!(
        tree.render_object_kind(render).expect("scroll render"),
        RenderKind::Scroll { physics: retained, .. } if *retained == physics
    ));
    assert!(!tree.scroll_at(Offset::new(50., 50.), Offset::new(0., 20.)));
    assert_eq!(controller.offset(), 0.);
}

#[test]
fn persistent_headers_pin_in_flow_and_are_pushed_by_the_next_header() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            incular_widgets::SingleChildScrollView::new(incular_widgets::Column::new(vec![
                Widget::box_(Size::new(80., 40.), Color::BLACK),
                Widget::persistent_header(
                    controller.clone(),
                    Widget::box_(Size::new(80., 20.), Color::rgba(255, 0, 0, 255)),
                ),
                Widget::box_(Size::new(80., 60.), Color::WHITE),
                Widget::persistent_header(
                    controller.clone(),
                    Widget::box_(Size::new(80., 20.), Color::rgba(0, 255, 0, 255)),
                ),
                Widget::box_(Size::new(80., 200.), Color::BLACK),
            ]))
            .controller(controller.clone())
            .into(),
        )
        .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    let flow = tree.children(root).unwrap()[0];
    let children = tree.children(flow).unwrap();
    let first = tree.render_id(children[1]).unwrap();
    let second = tree.render_id(children[3]).unwrap();
    assert_eq!(tree.render_origin(first), Offset::new(0., 40.));
    assert_eq!(tree.render_origin(second), Offset::new(0., 120.));

    assert!(controller.jump_to(50.));
    assert!(
        tree.update_compositor(Instant::now())
            .expect("compositor update")
            .0
    );
    assert_eq!(tree.render_origin(first), Offset::ZERO);
    assert_eq!(tree.render_origin(second), Offset::new(0., 70.));

    assert!(controller.jump_to(130.));
    assert!(
        tree.update_compositor(Instant::now())
            .expect("compositor update")
            .0
    );
    assert_eq!(tree.render_origin(first), Offset::new(0., -30.));
    assert_eq!(tree.render_origin(second), Offset::ZERO);
    // The flattened retained display list applies the same dynamic
    // transforms and clips the displaced predecessor out of the viewport.
    assert!(!rect_origins(&tree.paint()).contains(&Offset::new(0., -30.)));
    assert!(rect_origins(&tree.paint()).contains(&Offset::ZERO));
}

#[test]
fn pinned_header_sliver_pins_vertical_without_rebuilding_on_scroll() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let slivers: Vec<Box<dyn incular_widgets::internal::Sliver>> = vec![
        Box::new(incular_widgets::internal::PinnedHeaderSliver::new(
            Widget::box_(Size::new(100., 20.), Color::WHITE),
        )),
        Box::new(incular_widgets::internal::SliverToBoxAdapter::new(
            Widget::box_(Size::new(100., 220.), Color::BLACK),
        )),
    ];
    let root = tree
        .mount(
            incular_widgets::internal::CustomScrollView::new(slivers)
                .controller(controller.clone())
                .into(),
        )
        .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    // A real sliver viewport retains its materialized sliver children
    // directly; there is no synthetic Column/flow box between the
    // viewport and its sliver children.
    let header = tree.children(root).expect("sliver children")[0];
    let header_render = tree.render_id(header).unwrap();
    assert_eq!(tree.render_size(header_render), Some(Size::new(100., 20.)));
    assert_eq!(tree.render_origin(header_render), Offset::ZERO);
    let before = tree.diagnostics();
    assert!(controller.jump_to(80.));
    assert!(
        tree.update_compositor(Instant::now())
            .expect("compositor update")
            .0
    );
    assert_eq!(tree.render_origin(header_render), Offset::ZERO);
    assert_eq!(tree.diagnostics().rebuilds, before.rebuilds);
}

#[test]
fn pinned_header_sliver_supports_horizontal_and_reverse_viewports() {
    let make_tree = |reverse| {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        let slivers: Vec<Box<dyn incular_widgets::internal::Sliver>> = vec![
            Box::new(incular_widgets::internal::PinnedHeaderSliver::new(
                Widget::box_(Size::new(20., 40.), Color::WHITE),
            )),
            Box::new(incular_widgets::internal::SliverToBoxAdapter::new(
                Widget::box_(Size::new(220., 40.), Color::BLACK),
            )),
        ];
        let root = tree
            .mount(
                incular_widgets::internal::CustomScrollView::new(slivers)
                    .scroll_direction(Axis::Horizontal)
                    .reverse(reverse)
                    .controller(controller.clone())
                    .into(),
            )
            .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 40.)))
            .expect("layout");
        let header = tree.children(root).expect("sliver children")[0];
        (tree, controller, header)
    };

    let (mut forward, controller, header) = make_tree(false);
    assert!(controller.jump_to(80.));
    forward
        .update_compositor(Instant::now())
        .expect("compositor update");
    assert_eq!(
        forward.render_origin(forward.render_id(header).unwrap()).x,
        0.
    );

    let (mut reverse, controller, header) = make_tree(true);
    assert_eq!(controller.offset(), 0.);
    reverse
        .update_compositor(Instant::now())
        .expect("compositor update");
    // Reverse content enters from the physical trailing edge, so the
    // pinned header's leading edge is the viewport's trailing edge.
    assert_eq!(
        reverse.render_origin(reverse.render_id(header).unwrap()).x,
        80.
    );
}

#[test]
fn notification_listener_bubbles_sliver_events_and_honors_stop() {
    use incular_scroll::ScrollNotificationType;

    let controller = ScrollController::new();
    let inner_events = Rc::new(RefCell::new(Vec::new()));
    let outer_events = Rc::new(RefCell::new(Vec::new()));
    let observed_inner = inner_events.clone();
    let observed_outer = outer_events.clone();
    let sliver: Box<dyn incular_widgets::internal::Sliver> =
        Box::new(incular_widgets::internal::SliverToBoxAdapter::new(
            Widget::box_(Size::new(100., 240.), Color::WHITE),
        ));
    let view: Widget = incular_widgets::internal::CustomScrollView::new(vec![sliver])
        .controller(controller.clone())
        .into();
    let inner: Widget = incular_widgets::internal::NotificationListener::new(view)
        .on_notification(move |notification| {
            observed_inner
                .borrow_mut()
                .push((notification.kind, notification.depth));
            true
        })
        .into();
    let mut tree = WidgetTree::new();
    let _root = tree
        .mount(
            incular_widgets::internal::NotificationListener::new(inner)
                .on_notification(move |notification| {
                    observed_outer
                        .borrow_mut()
                        .push((notification.kind, notification.depth));
                    false
                })
                .into(),
        )
        .expect("notification listener mount");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");

    assert!(controller.jump_to(40.));
    assert!(
        inner_events
            .borrow()
            .iter()
            .any(|(kind, depth)| *kind == ScrollNotificationType::Update && *depth == 0)
    );
    assert!(outer_events.borrow().is_empty());
}

#[test]
fn million_item_sliver_list_materializes_only_viewport_and_cache() {
    let controller = ScrollController::new();
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let mut tree = WidgetTree::new();
    tree.mount(fixed_sliver_list(
        1_000_000,
        40.,
        controller.clone(),
        move |index| {
            observed.set(observed.get() + 1);
            Widget::box_(Size::new(80., 40.), Color::rgba(index as u8, 0, 0, 255))
        },
    ))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 600.)))
        .expect("layout");
    let initial = tree.sliver_viewport_diagnostics().unwrap();
    assert!(initial.materialized_item_count < 100);
    assert_eq!(calls.get(), initial.materialized_item_count);
    assert!(controller.jump_to(900_000. * 40.));
    tree.layout(Constraints::tight(Size::new(100., 600.)))
        .expect("layout");
    let jumped = tree.sliver_viewport_diagnostics().unwrap();
    assert!(jumped.materialized_range.contains(&900_000));
    assert!(jumped.materialized_item_count < 100);
    // Direct arithmetic builds only the destination cache range, never
    // every preceding logical item.
    assert!(calls.get() < 200);
    assert!(jumped.element_count < 100);
    assert!(jumped.render_object_count < 100);
}

#[test]
fn million_item_sliver_list_deep_jump_builds_only_destination_rows() {
    let controller = ScrollController::new();
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let mut tree = WidgetTree::new();
    tree.mount(
        CustomScrollView::new(vec![Box::new(SliverVariedExtentList::new(
            1_000_000,
            |item| {
                if item % 2 == 0 { 32. } else { 56. }
            },
            move |item| {
                observed.set(observed.get() + 1);
                Widget::box_(
                    Size::new(80., if item % 2 == 0 { 32. } else { 56. }),
                    Color::rgba(item as u8, 0, 0, 255),
                )
            },
        ))
            as Box<dyn incular_widgets::internal::Sliver>])
        .controller(controller.clone())
        .into(),
    )
    .unwrap();
    let constraints = Constraints::tight(Size::new(100., 600.));
    tree.layout(constraints).expect("layout");
    // The explicit extent builder has an exact 44px mean for the
    // alternating 32px/56px sequence, so target the real prefix offset.
    assert!(controller.jump_to(900_000. * 44.));
    tree.layout(constraints).expect("layout");
    let diagnostics = tree.sliver_viewport_diagnostics().unwrap();
    assert!(diagnostics.materialized_range.contains(&900_000));
    assert!(diagnostics.materialized_item_count < 100);
    assert!(calls.get() < 200);
    // The cache window, not the skipped prefix, is what becomes exact.
    assert!(tree.children(tree.root().unwrap()).unwrap().len() < 100);
}

#[test]
fn variable_measurements_above_visible_anchor_compensate_scroll_offset() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    tree.mount(
        CustomScrollView::new(vec![Box::new(SliverList::builder(1_000_000, |item| {
            Widget::box_(
                Size::new(80., if item < 900_000 { 80. } else { 40. }),
                Color::WHITE,
            )
        }))
            as Box<dyn incular_widgets::internal::Sliver>])
        .controller(controller.clone())
        .into(),
    )
    .unwrap();
    let constraints = Constraints::tight(Size::new(100., 600.));
    tree.layout(constraints).expect("layout");
    let target = 900_000. * 48.;
    assert!(controller.jump_to(target));
    tree.layout(constraints).expect("layout");
    // Cached rows before the first visible row grow from their 48px
    // estimates to their 80px laid-out height. The controller compensates
    // so the logical anchor stays in place.
    assert!(controller.offset() > target);
}

#[test]
fn restored_sliver_list_offset_stays_viewport_bounded() {
    let scope = restoration_scope();
    let key = restoration_key("million-items");
    scope.set_json(&key, json!({ "offset": 900_000. * 40. }));
    let controller = ScrollController::restored(scope, key);
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let mut tree = WidgetTree::new();
    tree.mount(fixed_sliver_list(
        1_000_000,
        40.,
        controller.clone(),
        move |index| {
            observed.set(observed.get() + 1);
            Widget::box_(Size::new(80., 40.), Color::rgba(index as u8, 0, 0, 255))
        },
    ))
    .unwrap();

    tree.layout(Constraints::tight(Size::new(100., 600.)))
        .expect("layout");
    let diagnostics = tree.sliver_viewport_diagnostics().unwrap();
    assert!(diagnostics.materialized_range.contains(&900_000));
    assert!(diagnostics.materialized_item_count < 100);
    assert!(calls.get() < 100);
}

#[test]
fn page_controller_alias_restores_its_logical_position() {
    let scope = restoration_scope();
    let key = restoration_key("pager");
    scope.set_json(&key, json!({ "offset": 200. }));

    let controller = incular_widgets::PageController::restored(scope, key);
    controller
        .update_extents(500., 100.)
        .expect("free controller publishes");
    assert_eq!(controller.offset(), 200.);
}

#[test]
fn sliver_children_retain_identity_inside_the_cache_and_release_outside() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(fixed_sliver_list(10_000, 40., controller.clone(), |i| {
            Widget::box_(Size::new(80., 40.), Color::rgba(i as u8, 0, 0, 255))
        }))
        .unwrap();
    let constraints = Constraints::tight(Size::new(100., 100.));
    tree.layout(constraints).expect("layout");
    let before = tree.children(root).unwrap().to_vec();
    assert!(controller.jump_to(3.));
    tree.layout(constraints).expect("layout");
    assert_eq!(tree.children(root).unwrap(), before.as_slice());
    assert!(controller.jump_to(400.));
    tree.layout(constraints).expect("layout");
    assert!(before.iter().any(|id| !tree.element_exists(*id)));
    assert!(before.iter().any(|id| tree.element_exists(*id)));
    let after = tree.sliver_viewport_diagnostics().unwrap();
    assert!(after.element_count < 30);
    assert!(tree.diagnostics().items_unmounted > 0);
}

#[test]
fn sliver_count_changes_retain_valid_rows_and_release_invalid_ones() {
    let controller = ScrollController::new();
    let constraints = Constraints::tight(Size::new(100., 100.));
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(fixed_sliver_list(100, 40., controller.clone(), |_| {
            Widget::box_(Size::new(80., 40.), Color::WHITE)
        }))
        .unwrap();
    tree.layout(constraints).expect("layout");
    let retained = tree.children(root).unwrap()[0];
    tree.update(
        root,
        fixed_sliver_list(80, 40., controller.clone(), |_| {
            Widget::box_(Size::new(80., 40.), Color::WHITE)
        }),
    )
    .unwrap();
    tree.layout(constraints).expect("layout");
    assert!(tree.element_exists(retained));
    assert!(controller.jump_to(1_000.));
    tree.layout(constraints).expect("layout");
    tree.update(
        root,
        fixed_sliver_list(0, 40., controller.clone(), |_| {
            Widget::box_(Size::new(80., 40.), Color::WHITE)
        }),
    )
    .unwrap();
    tree.layout(constraints).expect("layout");
    assert_eq!(controller.offset(), 0.);
    assert!(tree.children(root).unwrap().is_empty());
}

#[test]
fn sliver_slot_replaces_an_incompatible_retained_widget() {
    let controller = ScrollController::new();
    let constraints = Constraints::tight(Size::new(100., 100.));
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(fixed_sliver_list(4, 40., controller.clone(), |_| {
            Widget::box_(Size::new(80., 40.), Color::WHITE)
        }))
        .unwrap();
    tree.layout(constraints).expect("layout");
    let previous = tree.children(root).unwrap().to_vec();

    tree.update(
        root,
        fixed_sliver_list(4, 40., controller, |_| {
            GestureDetector::new(Widget::box_(Size::new(80., 40.), Color::WHITE)).on_tap(|| {})
        }),
    )
    .unwrap();
    tree.layout(constraints).expect("layout");

    let current = tree.children(root).unwrap();
    assert!(!current.is_empty());
    assert!(current.iter().all(|id| !previous.contains(id)));
}

#[test]
fn scrollbar_geometry_and_drag_share_the_controller() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    tree.mount(
        incular_widgets::SingleChildScrollView::new(Widget::box_(
            Size::new(100., 1_000.),
            Color::WHITE,
        ))
        .controller(controller.clone())
        .into(),
    )
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    let geometry = tree.scrollbar_diagnostics().pop().unwrap();
    assert!(geometry.visible);
    assert_eq!(geometry.thumb.size.height, 24.);
    assert_eq!(geometry.thumb.origin.y, 0.);
    assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Down, Offset::new(95., 10.)));
    assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., 60.)));
    assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Up, Offset::new(95., 60.)));
    assert!(controller.offset() > 0.);
    assert!(controller.offset() <= controller.max_offset());
    assert!(controller.scroll_by(0.25));
    assert!(controller.offset().fract() > 0.);
}

#[test]
fn scrollbar_geometry_round_trips_offsets_and_thumb_tops() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    tree.mount(
        incular_widgets::SingleChildScrollView::new(Widget::box_(
            Size::new(100., 4_000.),
            Color::WHITE,
        ))
        .controller(controller.clone())
        .into(),
    )
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 600.)))
        .expect("layout");
    let geometry = tree.scrollbar_diagnostics().pop().unwrap();
    assert_eq!(geometry.thumb.size.height, 90.);
    assert_eq!(geometry.thumb_travel, 510.);
    for fraction in [0., 0.1, 0.25, 0.5, 0.75, 0.9, 1.] {
        let offset = fraction * geometry.max_scroll_extent;
        let thumb_top = geometry.thumb_top_for_offset(offset);
        assert!((geometry.offset_for_thumb_top(thumb_top) - offset).abs() < 0.01);
        assert!(
            (geometry.thumb_top_for_offset(geometry.offset_for_thumb_top(thumb_top)) - thumb_top)
                .abs()
                < 0.01
        );
    }
}

#[test]
fn sliver_list_small_thumb_drag_is_continuous_and_reversible() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    tree.mount(fixed_sliver_list(100, 40., controller.clone(), |_| {
        Widget::box_(Size::new(100., 40.), Color::WHITE)
    }))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 600.)))
        .expect("layout");
    let geometry = tree.scrollbar_diagnostics().pop().unwrap();
    assert_eq!(controller.max_offset(), 3_400.);
    assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Down, Offset::new(95., 5.)));
    for y in [10., 15., 25.] {
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., y)));
    }
    let down = controller.offset();
    assert!(down > 0. && down < controller.max_offset() * 0.1);
    assert!((down - geometry.offset_for_thumb_top(20.)).abs() < 0.01);
    assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., 10.)));
    assert!(controller.offset() < down);
    assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Up, Offset::new(95., 10.)));
    assert!(!tree.scrollbar_drag_diagnostics().active);
}

#[test]
fn sliver_list_minimum_thumb_drag_uses_actual_travel_and_stays_bounded() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    tree.mount(fixed_sliver_list(
        1_000_000,
        40.,
        controller.clone(),
        |_| Widget::box_(Size::new(100., 40.), Color::WHITE),
    ))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 600.)))
        .expect("layout");
    let geometry = tree.scrollbar_diagnostics().pop().unwrap();
    assert_eq!(geometry.thumb.size.height, 24.);
    assert_eq!(geometry.thumb_travel, 576.);
    assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Down, Offset::new(95., 12.)));
    assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., 13.)));
    let one_pixel = controller.offset();
    assert!((one_pixel - geometry.max_scroll_extent / geometry.thumb_travel).abs() < 0.1);
    assert!(one_pixel < controller.max_offset());
    assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., 300.)));
    let middle = controller.offset();
    assert!(middle > controller.max_offset() * 0.45 && middle < controller.max_offset() * 0.55);
    tree.layout(Constraints::tight(Size::new(100., 600.)))
        .expect("layout");
    let middle_rows = tree.sliver_viewport_diagnostics().unwrap();
    assert!(middle_rows.materialized_range.contains(&(500_000usize)));
    assert!(middle_rows.materialized_item_count < 100);
    assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., 588.)));
    assert_eq!(controller.offset(), controller.max_offset());
    assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., 300.)));
    assert!(controller.offset() < controller.max_offset());
    assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Up, Offset::new(95., 300.)));
}

#[test]
fn scrollbar_does_not_scroll_when_thumb_has_no_travel() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    tree.mount(
        incular_widgets::SingleChildScrollView::new(Widget::box_(
            Size::new(100., 1_000.),
            Color::WHITE,
        ))
        .controller(controller.clone())
        .into(),
    )
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 20.)))
        .expect("layout");
    let geometry = tree.scrollbar_diagnostics().pop().unwrap();
    assert_eq!(geometry.thumb_travel, 0.);
    assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Down, Offset::new(95., 10.)));
    assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., 100.)));
    assert_eq!(controller.offset(), 0.);
}

#[test]
fn scrollbar_stays_synchronized_after_wheel_and_programmatic_offset_changes() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    tree.mount(
        incular_widgets::SingleChildScrollView::new(Widget::box_(
            Size::new(100., 4_000.),
            Color::WHITE,
        ))
        .controller(controller.clone())
        .into(),
    )
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 600.)))
        .expect("layout");
    assert!(tree.scroll_at(Offset::new(50., 50.), Offset::new(0., 600.)));
    let after_wheel = tree.scrollbar_diagnostics().pop().unwrap();
    assert!(after_wheel.thumb.origin.y > after_wheel.track.origin.y);
    assert!(controller.jump_to(controller.max_offset() * 0.5));
    let middle = tree.scrollbar_diagnostics().pop().unwrap();
    let grab_y = middle.thumb.origin.y + middle.thumb.size.height * 0.5;
    assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Down, Offset::new(95., grab_y)));
    assert!(tree.scrollbar_pointer(
        incular_core::PointerPhase::Move,
        Offset::new(95., grab_y - 10.)
    ));
    assert!(controller.offset() < controller.max_offset() * 0.5);
    let events = tree.diagnostics().scroll_events;
    assert!(tree.scrollbar_pointer(
        incular_core::PointerPhase::Move,
        Offset::new(95., grab_y - 10.)
    ));
    assert_eq!(tree.diagnostics().scroll_events, events);
}

#[test]
fn wheel_at_nested_scroll_transfers_child_boundary_remainder_to_parent_once() {
    let outer = ScrollController::new();
    let inner = ScrollController::new();
    let nested: Widget = SizedBox::from_size(Size::new(100., 100.))
        .child(
            incular_widgets::SingleChildScrollView::new(Widget::box_(
                Size::new(100., 300.),
                Color::WHITE,
            ))
            .controller(inner.clone()),
        )
        .into();
    let content: Widget = incular_widgets::Column::new(vec![
        Widget::box_(Size::new(100., 10.), Color::BLACK),
        nested,
        Widget::box_(Size::new(100., 1_000.), Color::WHITE),
    ])
    .into();
    let mut tree = WidgetTree::new();
    tree.mount(
        incular_widgets::SingleChildScrollView::new(content)
            .controller(outer.clone())
            .into(),
    )
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert!(outer.jump_to(100.));
    assert_eq!(inner.offset(), 0.);
    // At the inner top, upward wheel delta is consumed by the outer
    // viewport only; it is never duplicated into both controllers.
    assert!(tree.scroll_at(Offset::new(50., 20.), Offset::new(0., -40.)));
    assert_eq!(inner.offset(), 0.);
    assert_eq!(outer.offset(), 60.);
}

fn keyed_sliver_row(key: u64) -> Widget {
    Widget::box_(Size::new(80., 40.), Color::WHITE).with_key(Key::Value(key))
}

fn keyed_sliver_view(controller: &ScrollController, keys: &[u64]) -> Widget {
    let keys = keys.to_vec();
    CustomScrollView::new(vec![
        Box::new(SliverList::builder(keys.len(), move |index| {
            keyed_sliver_row(keys[index])
        })) as Box<dyn Sliver>,
    ])
    .controller(controller.clone())
    .into()
}

/// Labeled unequal rows for measurement-transfer scenarios. Labels read
/// `row{key}` so identity, paint, hits, and semantics can all name the
/// same row. Semantics merges into the row element (no extra level),
/// so child counts and hit targets match plain boxes.
fn labeled_row(height: f32, key: u64) -> Widget {
    Widget::from(
        incular_widgets::Semantics::new(Widget::box_(Size::new(100., height), Color::WHITE))
            .role(SemanticRole::Group)
            .label(format!("row{key}")),
    )
    .with_key(Key::Value(key))
}

/// Bare (fallback-estimate) unequal rows through ListView: measured
/// truth comes only from layout passes.
fn bare_labeled_view(controller: &ScrollController, heights: &[f32], keys: &[u64]) -> Widget {
    assert_eq!(heights.len(), keys.len());
    let heights = heights.to_vec();
    let keys = keys.to_vec();
    incular_widgets::ListView::builder(heights.len(), move |index| {
        labeled_row(heights[index], keys[index])
    })
    .controller(controller.clone())
    .into()
}

/// Seeded unequal rows: the extent builder carries exact per-row
/// knowledge, so fresh indexes are exact without visiting anything.
fn seeded_labeled_view(controller: &ScrollController, heights: &[f32], keys: &[u64]) -> Widget {
    assert_eq!(heights.len(), keys.len());
    let heights = heights.to_vec();
    let keys = keys.to_vec();
    let seed_heights = heights.clone();
    incular_widgets::ListView::builder(heights.len(), move |index| {
        labeled_row(heights[index], keys[index])
    })
    .item_extent_builder(move |index| seed_heights[index])
    .controller(controller.clone())
    .into()
}

fn labeled_paint_origins(tree: &mut WidgetTree) -> Vec<Offset> {
    rect_origins(&tree.paint())
        .into_iter()
        .filter(|origin| origin.x < 100.)
        .collect()
}

#[test]
fn dynamic_keyed_rotation_lookups_stay_linear() {
    // An N-child rotation moves every element through the keyed path:
    // compatibility work stays linear, the keyed index is built once
    // per reconcile pass (hygiene plus one converging pass here, each
    // filing every keyed old exactly once), only the hygiene pass
    // probes it (the converging pass resolves positionally), and
    // nothing mounts or unmounts. Repeated full scans would grow
    // quadratically instead.
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let keys: Vec<u64> = (0..8).collect();
    let root = tree.mount(keyed_sliver_view(&controller, &keys)).unwrap();
    tree.layout(Constraints::tight(Size::new(100., 400.)))
        .expect("layout");
    let before_ids = tree.children(root).unwrap().to_vec();
    assert_eq!(before_ids.len(), 8);
    let before = tree.diagnostics();
    tree.update(
        root,
        keyed_sliver_view(&controller, &[4, 5, 6, 7, 0, 1, 2, 3]),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 400.)))
        .expect("layout");
    let after_ids = tree.children(root).unwrap().to_vec();
    assert_eq!(
        after_ids,
        [
            before_ids[4],
            before_ids[5],
            before_ids[6],
            before_ids[7],
            before_ids[0],
            before_ids[1],
            before_ids[2],
            before_ids[3]
        ]
    );
    let after = tree.diagnostics();
    assert_eq!(after.key_maps_built - before.key_maps_built, 2);
    assert_eq!(after.key_map_entries - before.key_map_entries, 16);
    assert_eq!(after.key_lookups - before.key_lookups, 8);
    assert!(
        after.key_comparisons - before.key_comparisons <= 32,
        "compatibility work stays linear in moved children"
    );
    assert_eq!(after.mounts - before.mounts, 0);
    assert_eq!(after.unmounts - before.unmounts, 0);
    // No-op update: identical keys resolve positionally with no keyed
    // probes at all.
    let settled = tree.diagnostics();
    tree.update(
        root,
        keyed_sliver_view(&controller, &[4, 5, 6, 7, 0, 1, 2, 3]),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 400.)))
        .expect("layout");
    assert_eq!(tree.children(root).unwrap(), &after_ids[..]);
    let quiet = tree.diagnostics();
    assert_eq!(quiet.key_lookups - settled.key_lookups, 0);
    assert_eq!(quiet.mounts - settled.mounts, 0);
    assert_eq!(quiet.unmounts - settled.unmounts, 0);
}

fn base_heights(count: usize) -> Vec<f32> {
    (0..count).map(|i| 30. + 4. * i as f32).collect()
}

fn base_keys(count: usize) -> Vec<u64> {
    (0..count as u64).collect()
}

fn mount_baseline(seeded: bool) -> (WidgetTree, ScrollController, ElementId) {
    let heights = base_heights(40);
    let keys = base_keys(40);
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(if seeded {
            seeded_labeled_view(&controller, &heights, &keys)
        } else {
            bare_labeled_view(&controller, &heights, &keys)
        })
        .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert!(controller.jump_to(150.));
    tree.update_compositor(Instant::now())
        .expect("compositor update");
    (tree, controller, root)
}

fn snapshot(
    tree: &mut WidgetTree,
    controller: &ScrollController,
    root: ElementId,
) -> (f32, f32, Vec<Offset>, Vec<ElementId>, String) {
    tree.update_compositor(Instant::now())
        .expect("compositor update");
    tree.update_semantics();
    (
        controller.offset(),
        controller.max_offset(),
        labeled_paint_origins(tree),
        tree.children(root).unwrap().to_vec(),
        tree.semantics_debug_dump(),
    )
}

fn hit_element(tree: &WidgetTree, point: Offset) -> Option<ElementId> {
    tree.hit_test(point)
        .and_then(|render| tree.element_for_render(render))
}

/// Widget keys of materialized children, in order. Inverts
/// `element_with_key` over the scenario's candidate keys.
fn window_keys(tree: &WidgetTree, kids: &[ElementId], candidates: &[u64]) -> Vec<u64> {
    kids.iter()
        .map(|kid| {
            candidates
                .iter()
                .find(|key| tree.element_with_key(&Key::Value(**key)) == Some(*kid))
                .copied()
                .expect("materialized child carries a scenario key")
        })
        .collect()
}

#[test]
fn transfer_unequal_rebuild_holds_anchor() {
    // Identical descriptor rebuild while scrolled: offset and paint
    // stay put, every old element survives (new rows may join the
    // estimate-widened window), and the total matches the window
    // model exactly — bare or seeded.
    for seeded in [false, true] {
        let (mut tree, controller, root) = mount_baseline(seeded);
        let before = tree.children(root).unwrap().to_vec();
        let heights = base_heights(40);
        let keys = base_keys(40);
        tree.update(
            root,
            if seeded {
                seeded_labeled_view(&controller, &heights, &keys)
            } else {
                bare_labeled_view(&controller, &heights, &keys)
            },
        )
        .expect("update");
        tree.layout(Constraints::tight(Size::new(100., 100.)))
            .expect("layout");
        let (offset, max, paint, kids, dump) = snapshot(&mut tree, &controller, root);
        assert_eq!(offset, 150., "seeded={seeded}: anchor holds");
        assert_eq!(
            paint,
            vec![
                Offset::new(0., -6.),
                Offset::new(0., 40.),
                Offset::new(0., 90.)
            ]
        );
        assert!(kids.len() < 40, "seeded={seeded}: still lazy");
        for id in &before {
            assert!(kids.contains(id), "seeded={seeded}: no element lost");
        }
        let window = window_keys(&tree, &kids, &keys);
        // Total composition (hand-derived best-available accounting):
        // materialized rows contribute adopted/measured truth, rows
        // never visited contribute honest estimates — seeded exactness
        // where the variant carries explicit knowledge (48.0 lazy
        // fallback elsewhere) — plus retained truth in slots whose
        // rows left the window (same content, informative, harmless).
        // Bare here: truth rows 0..=10 (550) + retained row-11 truth
        // (74) + 28 fallbacks (1344) = 1968, max 1868.
        if seeded {
            assert_eq!(max, 4220.);
        } else {
            assert_eq!(window, (0..11).collect::<Vec<_>>());
            assert_eq!(max, 1868.);
        }
        for key in [4u64, 5, 6] {
            assert!(
                dump.contains(&format!("label=Some(\"row{key}\")")),
                "seeded={seeded}: visible row{key} in semantics"
            );
        }
    }
}

#[test]
fn transfer_insertion_before_viewport_keeps_pixels() {
    // Insert a 50px row (fresh key) at index 0 of a scrolled list:
    // pixels hold (offset stays 150), total grows by exactly the new
    // row, paint shows the shifted rows, and the fresh row
    // materializes without scrolling. Seeded and bare agree because
    // identity adoption carries true sizes and the gate suppresses
    // the index-confused correction.
    for seeded in [false, true] {
        let (mut tree, controller, root) = mount_baseline(seeded);
        let mut heights = vec![50.];
        heights.extend_from_slice(&base_heights(40));
        let mut keys = vec![100u64];
        keys.extend_from_slice(&base_keys(40));
        tree.update(
            root,
            if seeded {
                seeded_labeled_view(&controller, &heights, &keys)
            } else {
                bare_labeled_view(&controller, &heights, &keys)
            },
        )
        .expect("update");
        tree.layout(Constraints::tight(Size::new(100., 100.)))
            .expect("layout");
        let (offset, max, paint, kids, dump) = snapshot(&mut tree, &controller, root);
        assert_eq!(offset, 150., "seeded={seeded}: pixels hold");
        assert_eq!(
            paint,
            vec![
                Offset::new(0., -36.),
                Offset::new(0., 2.),
                Offset::new(0., 44.),
                Offset::new(0., 90.)
            ],
            "seeded={seeded}: shifted rows paint exact"
        );
        // Window at 150 by adopted-truth cumulative holds new rows
        // 0..=10 (X plus old rows 0..=9): eleven materialized of
        // forty-one, far rows never visited.
        assert_eq!(kids.len(), 11);
        assert!(kids.len() < 41, "seeded={seeded}: still lazy");
        // Fresh row leads; every materialized old element moved
        // exactly one slot.
        assert_eq!(tree.element_with_key(&Key::Value(100)), Some(kids[0]));
        for (position, key) in base_keys(40).iter().enumerate().take(10) {
            assert_eq!(
                tree.element_with_key(&Key::Value(*key)),
                Some(kids[position + 1]),
                "seeded={seeded}: old row {key} moved +1"
            );
        }
        // Viewport point y=10 is content y=160, inside new row 4
        // (old row 3, key 3): hits track moved content, not indices.
        assert_eq!(
            hit_element(&tree, Offset::new(50., 10.)),
            tree.element_with_key(&Key::Value(3)),
            "seeded={seeded}: hit lands on shifted row"
        );
        for key in [100u64, 1, 2, 3, 4] {
            assert!(
                dump.contains(&format!("label=Some(\"row{key}\")")),
                "seeded={seeded}: row{key} in semantics"
            );
        }
        if seeded {
            assert_eq!(max, 4270.);
        } else {
            // Adopted window truth (X50 + rows 0..=9 truth 480 = 530)
            // with 30 fallbacks (1440): total 1970, max 1870.
            assert_eq!(max, 1870.);
        }
    }
}

#[test]
fn transfer_removal_before_viewport_keeps_pixels() {
    // Drop index 0 of a scrolled list: pixels hold, total loses
    // exactly the removed row, paint shows the shifted rows, and the
    // removed label leaves semantics. Keyed moves gate the anchor
    // correction in both variants.
    for seeded in [false, true] {
        let (mut tree, controller, root) = mount_baseline(seeded);
        let heights = base_heights(40)[1..].to_vec();
        let keys = base_keys(40)[1..].to_vec();
        tree.update(
            root,
            if seeded {
                seeded_labeled_view(&controller, &heights, &keys)
            } else {
                bare_labeled_view(&controller, &heights, &keys)
            },
        )
        .expect("update");
        tree.layout(Constraints::tight(Size::new(100., 100.)))
            .expect("layout");
        let (offset, max, paint, kids, dump) = snapshot(&mut tree, &controller, root);
        assert_eq!(offset, 150., "seeded={seeded}: pixels hold");
        // New mapping: r1 0-34, r2 34-72, r3 72-114, r4 114-160,
        // r5 160-210, r6 210-264.
        assert_eq!(
            paint,
            vec![
                Offset::new(0., -36.),
                Offset::new(0., 10.),
                Offset::new(0., 60.)
            ]
        );
        assert!(kids.len() < 39, "seeded={seeded}: still lazy");
        assert_eq!(kids.len(), 10);
        for (position, key) in keys.iter().enumerate().take(kids.len()) {
            assert_eq!(
                tree.element_with_key(&Key::Value(*key)),
                Some(kids[position]),
                "seeded={seeded}: surviving row {key} kept identity"
            );
        }
        // Content y=160 sits in new row 5 (old row 5, key 5).
        assert_eq!(
            hit_element(&tree, Offset::new(50., 10.)),
            tree.element_with_key(&Key::Value(5)),
            "seeded={seeded}: hit follows shifted content"
        );
        for key in [4u64, 5, 6] {
            assert!(
                dump.contains(&format!("label=Some(\"row{key}\")")),
                "seeded={seeded}: visible row{key} in semantics"
            );
        }
        assert!(
            !dump.contains("label=Some(\"row0\")"),
            "seeded={seeded}: removed label gone from semantics"
        );
        if seeded {
            assert_eq!(max, 4190.);
        } else {
            // Adopted window truth (new rows 0..=11 = old rows
            // 1..=12: 672) with 27 fallbacks (1296): total 1968.
            assert_eq!(max, 1868.);
        }
    }
}

#[test]
fn transfer_distant_reorder_keeps_pixels() {
    // Swap indices 0 and 8 (unequal heights travel with their keys):
    // cumulative offsets above the viewport change, yet pixels hold
    // because identity adoption carries true sizes and the gate
    // suppresses the index-confused correction.
    for seeded in [false, true] {
        let (mut tree, controller, root) = mount_baseline(seeded);
        let mut heights = base_heights(40);
        heights.swap(0, 8);
        let mut keys = base_keys(40);
        keys.swap(0, 8);
        tree.update(
            root,
            if seeded {
                seeded_labeled_view(&controller, &heights, &keys)
            } else {
                bare_labeled_view(&controller, &heights, &keys)
            },
        )
        .expect("update");
        tree.layout(Constraints::tight(Size::new(100., 100.)))
            .expect("layout");
        let (offset, max, paint, kids, dump) = snapshot(&mut tree, &controller, root);
        assert_eq!(offset, 150., "seeded={seeded}: pixels hold");
        // New cumulative: 62, 96, 134, 176, 222, ... : rows 2, 3, 4
        // paint at -16, 26, 72.
        assert_eq!(
            paint,
            vec![
                Offset::new(0., -16.),
                Offset::new(0., 26.),
                Offset::new(0., 72.)
            ]
        );
        assert!(kids.len() < 40, "seeded={seeded}: still lazy");
        assert_eq!(kids.len(), 11);
        assert_eq!(tree.element_with_key(&Key::Value(8)), Some(kids[0]));
        assert_eq!(tree.element_with_key(&Key::Value(0)), Some(kids[8]));
        // Viewport point y=10 is content y=160, inside new row 3
        // (old row 3, key 3): hits track moved content, not indices.
        assert_eq!(
            hit_element(&tree, Offset::new(50., 10.)),
            tree.element_with_key(&Key::Value(3)),
            "seeded={seeded}: hit follows reordered content"
        );
        for key in [3u64, 4, 5] {
            assert!(
                dump.contains(&format!("label=Some(\"row{key}\")")),
                "seeded={seeded}: visible row{key} in semantics"
            );
        }
        if seeded {
            assert_eq!(max, 4220.);
        } else {
            // Same multiset in new positions: adopted truth rows
            // 0..=10 sums identically (550), plus retained row-11
            // truth (74, same content) with 28 fallbacks (1344).
            assert_eq!(max, 1868.);
        }
    }
}

#[test]
fn transfer_replacement_content_revalidates_coherently() {
    // Completely different content (uniform rows, fresh keys): no row
    // identity survives, so transferred guesses are demoted and the
    // first sight refines like a fresh estimate — content-stable,
    // exact totals, coherent paint/hit/semantics, stale labels gone.
    for seeded in [false, true] {
        let (mut tree, controller, root) = mount_baseline(seeded);
        let before = tree.children(root).unwrap().to_vec();
        let heights = vec![44.; 40];
        let keys: Vec<u64> = (50..90).collect();
        tree.update(
            root,
            if seeded {
                seeded_labeled_view(&controller, &heights, &keys)
            } else {
                bare_labeled_view(&controller, &heights, &keys)
            },
        )
        .expect("update");
        tree.layout(Constraints::tight(Size::new(100., 100.)))
            .expect("layout");
        let (offset, max, paint, kids, dump) = snapshot(&mut tree, &controller, root);
        assert!(kids.len() < 40, "seeded={seeded}: still lazy");
        let mut distinct = kids.clone();
        distinct.sort_by_key(|id| format!("{id:?}"));
        distinct.dedup();
        assert_eq!(
            distinct.len(),
            kids.len(),
            "seeded={seeded}: no aliased rows"
        );
        for id in &before {
            assert!(!kids.contains(id), "seeded={seeded}: stale rows unmounted");
        }
        for (position, key) in keys.iter().enumerate().take(kids.len()) {
            assert_eq!(
                tree.element_with_key(&Key::Value(*key)),
                Some(kids[position]),
                "seeded={seeded}: fresh row {key} placed"
            );
        }
        for key in [50u64, 51, 52, 53] {
            assert!(
                dump.contains(&format!("label=Some(\"row{key}\")")),
                "seeded={seeded}: fresh row{key} in semantics"
            );
        }
        assert!(
            !dump.contains("label=Some(\"row5\")"),
            "seeded={seeded}: stale labels gone"
        );
        if seeded {
            // Forty uniform 44px rows: total 1760, max 1660. Offset
            // holds (exact seeds, nothing to refine); rows at 44px
            // pitch paint at -18, 26, 70.
            assert_eq!(offset, 150.);
            assert_eq!(max, 1660.);
            assert_eq!(
                paint,
                vec![
                    Offset::new(0., -18.),
                    Offset::new(0., 26.),
                    Offset::new(0., 70.)
                ]
            );
        } else {
            // Bare replacement: no row identity survives, so the
            // transferred guesses are demoted and the first sight
            // refines like fresh estimates — the anchor holds the
            // newly measured content stable instead of jumping.
            // Estimates place row 3 at 144; truth is 132; the -12
            // correction lands at 138 with rows at 44px pitch. Twelve
            // rows materialize (12×44 measured) with 28 fallbacks:
            // total 1872, max 1772 — honest estimates, no stale rows.
            assert_eq!(offset, 138.);
            assert_eq!(kids.len(), 12);
            assert_eq!(max, 1772.);
            assert_eq!(
                paint,
                vec![
                    Offset::new(0., -6.),
                    Offset::new(0., 38.),
                    Offset::new(0., 82.)
                ]
            );
        }
    }
}

#[test]
fn dynamic_duplicate_widget_keys_are_rejected_without_aliasing() {
    // Old keyed children [7, 9], desired widget keys [9, 9]: the first
    // desired child moves old 9, and the positional lookup for the
    // second must not select old 9 again. The framework's duplicate-key
    // policy applies — no aliased element, no silent second copy.
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = tree.mount(keyed_sliver_view(&controller, &[7, 9])).unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    let before = tree.children(root).unwrap().to_vec();
    assert_eq!(before.len(), 2);
    let mounts = tree.diagnostics().mounts;
    let unmounts = tree.diagnostics().unmounts;
    tree.update(root, keyed_sliver_view(&controller, &[9, 9]))
        .expect("dynamic update defers to layout");
    let error = tree
        .layout(Constraints::tight(Size::new(100., 100.)))
        .unwrap_err();
    assert!(
        matches!(error, TreeError::DuplicateKey { .. }),
        "unexpected failure: {error:?}"
    );
    assert_eq!(tree.children(root).unwrap(), &before[..]);
    for id in &before {
        assert!(tree.element_exists(*id));
    }
    assert_eq!(tree.diagnostics().mounts, mounts);
    assert_eq!(tree.diagnostics().unmounts, unmounts);
}

#[test]
fn dynamic_keyed_reorder_moves_elements_without_rebuild() {
    // Valid reorder: distinct element IDs follow their keys, nothing
    // mounts or unmounts, and key lookup still resolves each key to
    // its original element — retained state survives structurally,
    // not just as repainted labels.
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(keyed_sliver_view(&controller, &[7, 9, 11]))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 200.)))
        .expect("layout");
    let before = tree.children(root).unwrap().to_vec();
    assert_eq!(before.len(), 3);
    let mounts = tree.diagnostics().mounts;
    let unmounts = tree.diagnostics().unmounts;
    tree.update(root, keyed_sliver_view(&controller, &[11, 7, 9]))
        .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 200.)))
        .expect("layout");
    let after = tree.children(root).unwrap().to_vec();
    assert_eq!(after, &[before[2], before[0], before[1]]);
    assert_eq!(tree.element_with_key(&Key::Value(11)), Some(before[2]));
    assert_eq!(tree.element_with_key(&Key::Value(7)), Some(before[0]));
    assert_eq!(tree.element_with_key(&Key::Value(9)), Some(before[1]));
    assert_eq!(tree.diagnostics().mounts, mounts);
    assert_eq!(tree.diagnostics().unmounts, unmounts);
}

#[test]
fn dynamic_mixed_keyed_unkeyed_children_reconcile() {
    // Keyed children follow their keys while the unkeyed child takes
    // the positional path: only the displaced unkeyed element turns
    // over, and keyed identities never alias.
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let rows = |keys: Vec<Option<u64>>| {
        let count = keys.len();
        CustomScrollView::new(vec![Box::new(SliverList::builder(count, move |index| {
            let row = Widget::box_(Size::new(80., 40.), Color::WHITE);
            match keys[index] {
                Some(key) => row.with_key(Key::Value(key)),
                None => row,
            }
        })) as Box<dyn Sliver>])
        .controller(controller.clone())
        .into()
    };
    let root = tree.mount(rows(vec![Some(7), None, Some(9)])).unwrap();
    tree.layout(Constraints::tight(Size::new(100., 200.)))
        .expect("layout");
    let before = tree.children(root).unwrap().to_vec();
    assert_eq!(before.len(), 3);
    let mounts = tree.diagnostics().mounts;
    let unmounts = tree.diagnostics().unmounts;
    tree.update(root, rows(vec![None, Some(9), Some(7)]))
        .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 200.)))
        .expect("layout");
    let after = tree.children(root).unwrap().to_vec();
    assert_eq!(after.len(), 3);
    assert_ne!(after[0], before[0]);
    assert_ne!(after[0], before[1]);
    assert_ne!(after[0], before[2]);
    assert_eq!(after[1], before[2]);
    assert_eq!(after[2], before[0]);
    assert!(!tree.element_exists(before[1]));
    assert_eq!(tree.element_with_key(&Key::Value(9)), Some(before[2]));
    assert_eq!(tree.element_with_key(&Key::Value(7)), Some(before[0]));
    assert_eq!(tree.diagnostics().mounts, mounts + 1);
    assert_eq!(tree.diagnostics().unmounts, unmounts + 1);
}

#[test]
fn dynamic_keyed_type_change_remounts() {
    // Same key but an incompatible type cannot reuse the element: the
    // old element unmounts exactly once and a fresh one takes the key.
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = tree.mount(keyed_sliver_view(&controller, &[7])).unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    let before = tree.children(root).unwrap().to_vec();
    assert_eq!(before.len(), 1);
    let mounts = tree.diagnostics().mounts;
    let unmounts = tree.diagnostics().unmounts;
    tree.update(
        root,
        CustomScrollView::new(vec![Box::new(SliverList::builder(1, |_| {
            Widget::from(incular_widgets::Text::new("seven")).with_key(Key::Value(7))
        })) as Box<dyn Sliver>])
        .controller(controller.clone())
        .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    let after = tree.children(root).unwrap().to_vec();
    assert_eq!(after.len(), 1);
    assert_ne!(after[0], before[0]);
    assert!(!tree.element_exists(before[0]));
    assert_eq!(tree.element_with_key(&Key::Value(7)), Some(after[0]));
    assert_eq!(tree.diagnostics().mounts, mounts + 1);
    assert_eq!(tree.diagnostics().unmounts, unmounts + 1);
}

#[test]
fn dynamic_removed_children_unmount_exactly_once() {
    // Removal unmounts each dropped child once, and a later sibling
    // set never resurrects the dead ids: the returning key mounts
    // fresh instead of aliasing a corpse.
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(keyed_sliver_view(&controller, &[7, 9, 11]))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 200.)))
        .expect("layout");
    let before = tree.children(root).unwrap().to_vec();
    assert_eq!(before.len(), 3);
    let mounts = tree.diagnostics().mounts;
    let unmounts = tree.diagnostics().unmounts;
    tree.update(root, keyed_sliver_view(&controller, &[7]))
        .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 200.)))
        .expect("layout");
    assert_eq!(tree.children(root).unwrap(), &[before[0]]);
    assert!(!tree.element_exists(before[1]));
    assert!(!tree.element_exists(before[2]));
    assert_eq!(tree.diagnostics().unmounts, unmounts + 2);
    tree.update(root, keyed_sliver_view(&controller, &[9]))
        .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 200.)))
        .expect("layout");
    let revived = tree.children(root).unwrap().to_vec();
    assert_eq!(revived.len(), 1);
    assert_ne!(revived[0], before[1]);
    assert_eq!(tree.element_with_key(&Key::Value(9)), Some(revived[0]));
    assert_eq!(tree.diagnostics().mounts, mounts + 1);
    assert_eq!(tree.diagnostics().unmounts, unmounts + 3);
}
