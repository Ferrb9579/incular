//! Scrolling and sliver tests.

mod common;

use common::*;
use incular_config::{Axis, Constraints};
use incular_core::{Color, Offset, Size};
use incular_scroll::ScrollPhysics;
use incular_widgets::internal::*;
use serde_json::json;
use std::{cell::Cell, rc::Rc, time::Instant};

#[test]
fn scroll_positions_are_logical_clamped_and_clip_the_viewport() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    tree.mount(Widget::scroll_view(
        controller.clone(),
        Widget::column(
            (0..5)
                .map(|_| Widget::box_(Size::new(80., 40.), Color::WHITE))
                .collect::<Vec<_>>(),
        ),
    ))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)));
    let _ = tree.update_compositor(Instant::now());
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
    let _ = tree.update_compositor(Instant::now());
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
    let _ = tree.update_compositor(Instant::now());
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
            incular_widgets::internal::SingleChildScrollView::new(Widget::row(
                (0..5)
                    .map(|_| Widget::box_(Size::new(40., 30.), Color::WHITE))
                    .collect::<Vec<_>>(),
            ))
            .scroll_direction(Axis::Horizontal)
            .controller(controller.clone())
            .into(),
        )
        .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 40.)));
    let _ = tree.update_compositor(Instant::now());
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
    let _ = tree.update_compositor(Instant::now());
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
    tree.layout(Constraints::tight(Size::new(100., 100.)));
    let _ = tree.update_compositor(Instant::now());
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
    let _ = tree.update_compositor(Instant::now());
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
                Widget::fixed_box(Size::new(20., 20.), Color::WHITE),
                Widget::fixed_box(Size::new(20., 20.), Color::BLACK),
            ])
            .scroll_direction(Axis::Horizontal)
            .controller(controller.clone())
            .into(),
        )
        .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 40.)));
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
                Widget::fixed_box(Size::new(20., 20.), Color::WHITE),
                Widget::fixed_box(Size::new(20., 20.), Color::BLACK),
            ])
            .scroll_direction(Axis::Horizontal)
            .reverse(true)
            .controller(reverse_controller.clone())
            .into(),
        )
        .unwrap();
    reverse_tree.layout(Constraints::tight(Size::new(100., 40.)));
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
            incular_widgets::internal::SingleChildScrollView::new(Widget::column(
                (0..4)
                    .map(|_| Widget::box_(Size::new(80., 40.), Color::WHITE))
                    .collect::<Vec<_>>(),
            ))
            .physics(physics)
            .controller(controller.clone())
            .into(),
        )
        .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)));
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
        .mount(Widget::scroll_view(
            controller.clone(),
            Widget::column(vec![
                Widget::fixed_box(Size::new(80., 40.), Color::BLACK),
                Widget::persistent_header(
                    controller.clone(),
                    Widget::fixed_box(Size::new(80., 20.), Color::rgba(255, 0, 0, 255)),
                ),
                Widget::fixed_box(Size::new(80., 60.), Color::WHITE),
                Widget::persistent_header(
                    controller.clone(),
                    Widget::fixed_box(Size::new(80., 20.), Color::rgba(0, 255, 0, 255)),
                ),
                Widget::fixed_box(Size::new(80., 200.), Color::BLACK),
            ]),
        ))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)));
    let flow = tree.children(root).unwrap()[0];
    let children = tree.children(flow).unwrap();
    let first = tree.render_id(children[1]).unwrap();
    let second = tree.render_id(children[3]).unwrap();
    assert_eq!(tree.render_origin(first), Offset::new(0., 40.));
    assert_eq!(tree.render_origin(second), Offset::new(0., 120.));

    assert!(controller.jump_to(50.));
    assert!(tree.update_compositor(Instant::now()).0);
    assert_eq!(tree.render_origin(first), Offset::ZERO);
    assert_eq!(tree.render_origin(second), Offset::new(0., 70.));

    assert!(controller.jump_to(130.));
    assert!(tree.update_compositor(Instant::now()).0);
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
            Widget::fixed_box(Size::new(100., 20.), Color::WHITE),
        )),
        Box::new(incular_widgets::internal::SliverToBoxAdapter::new(
            Widget::fixed_box(Size::new(100., 220.), Color::BLACK),
        )),
    ];
    let root = tree
        .mount(
            incular_widgets::internal::CustomScrollView::new(slivers)
                .controller(controller.clone())
                .into(),
        )
        .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)));
    // A real sliver viewport retains its materialized sliver children
    // directly; there is no synthetic Column/flow box between the
    // viewport and its sliver children.
    let header = tree.children(root).expect("sliver children")[0];
    let header_render = tree.render_id(header).unwrap();
    assert_eq!(tree.render_size(header_render), Some(Size::new(100., 20.)));
    assert_eq!(tree.render_origin(header_render), Offset::ZERO);
    let before = tree.diagnostics();
    assert!(controller.jump_to(80.));
    assert!(tree.update_compositor(Instant::now()).0);
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
                Widget::fixed_box(Size::new(20., 40.), Color::WHITE),
            )),
            Box::new(incular_widgets::internal::SliverToBoxAdapter::new(
                Widget::fixed_box(Size::new(220., 40.), Color::BLACK),
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
        tree.layout(Constraints::tight(Size::new(100., 40.)));
        let header = tree.children(root).expect("sliver children")[0];
        (tree, controller, header)
    };

    let (mut forward, controller, header) = make_tree(false);
    assert!(controller.jump_to(80.));
    forward.update_compositor(Instant::now());
    assert_eq!(
        forward.render_origin(forward.render_id(header).unwrap()).x,
        0.
    );

    let (mut reverse, controller, header) = make_tree(true);
    assert_eq!(controller.offset(), 0.);
    reverse.update_compositor(Instant::now());
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
            Widget::fixed_box(Size::new(100., 240.), Color::WHITE),
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
    tree.layout(Constraints::tight(Size::new(100., 100.)));

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
    tree.layout(Constraints::tight(Size::new(100., 600.)));
    let initial = tree.sliver_viewport_diagnostics().unwrap();
    assert!(initial.materialized_item_count < 100);
    assert_eq!(calls.get(), initial.materialized_item_count);
    assert!(controller.jump_to(900_000. * 40.));
    tree.layout(Constraints::tight(Size::new(100., 600.)));
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
    tree.layout(constraints);
    // The explicit extent builder has an exact 44px mean for the
    // alternating 32px/56px sequence, so target the real prefix offset.
    assert!(controller.jump_to(900_000. * 44.));
    tree.layout(constraints);
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
    tree.layout(constraints);
    let target = 900_000. * 48.;
    assert!(controller.jump_to(target));
    tree.layout(constraints);
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

    tree.layout(Constraints::tight(Size::new(100., 600.)));
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
    controller.update_extents(500., 100.);
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
    tree.layout(constraints);
    let before = tree.children(root).unwrap().to_vec();
    assert!(controller.jump_to(3.));
    tree.layout(constraints);
    assert_eq!(tree.children(root).unwrap(), before.as_slice());
    assert!(controller.jump_to(400.));
    tree.layout(constraints);
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
    tree.layout(constraints);
    let retained = tree.children(root).unwrap()[0];
    tree.update(
        root,
        fixed_sliver_list(80, 40., controller.clone(), |_| {
            Widget::box_(Size::new(80., 40.), Color::WHITE)
        }),
    )
    .unwrap();
    tree.layout(constraints);
    assert!(tree.element_exists(retained));
    assert!(controller.jump_to(1_000.));
    tree.layout(constraints);
    tree.update(
        root,
        fixed_sliver_list(0, 40., controller.clone(), |_| {
            Widget::box_(Size::new(80., 40.), Color::WHITE)
        }),
    )
    .unwrap();
    tree.layout(constraints);
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
    tree.layout(constraints);
    let previous = tree.children(root).unwrap().to_vec();

    tree.update(
        root,
        fixed_sliver_list(4, 40., controller, |_| {
            GestureDetector::new(Widget::box_(Size::new(80., 40.), Color::WHITE)).on_tap(|| {})
        }),
    )
    .unwrap();
    tree.layout(constraints);

    let current = tree.children(root).unwrap();
    assert!(!current.is_empty());
    assert!(current.iter().all(|id| !previous.contains(id)));
}

#[test]
fn scrollbar_geometry_and_drag_share_the_controller() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    tree.mount(Widget::scroll_view(
        controller.clone(),
        Widget::fixed_box(Size::new(100., 1_000.), Color::WHITE),
    ))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)));
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
    tree.mount(Widget::scroll_view(
        controller.clone(),
        Widget::fixed_box(Size::new(100., 4_000.), Color::WHITE),
    ))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 600.)));
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
        Widget::fixed_box(Size::new(100., 40.), Color::WHITE)
    }))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 600.)));
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
        |_| Widget::fixed_box(Size::new(100., 40.), Color::WHITE),
    ))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 600.)));
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
    tree.layout(Constraints::tight(Size::new(100., 600.)));
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
    tree.mount(Widget::scroll_view(
        controller.clone(),
        Widget::fixed_box(Size::new(100., 1_000.), Color::WHITE),
    ))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 20.)));
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
    tree.mount(Widget::scroll_view(
        controller.clone(),
        Widget::fixed_box(Size::new(100., 4_000.), Color::WHITE),
    ))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 600.)));
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
        .child(Widget::scroll_view(
            inner.clone(),
            Widget::fixed_box(Size::new(100., 300.), Color::WHITE),
        ))
        .into();
    let content = Widget::column(vec![
        Widget::fixed_box(Size::new(100., 10.), Color::BLACK),
        nested,
        Widget::fixed_box(Size::new(100., 1_000.), Color::WHITE),
    ]);
    let mut tree = WidgetTree::new();
    tree.mount(Widget::scroll_view(outer.clone(), content))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)));
    assert!(outer.jump_to(100.));
    assert_eq!(inner.offset(), 0.);
    // At the inner top, upward wheel delta is consumed by the outer
    // viewport only; it is never duplicated into both controllers.
    assert!(tree.scroll_at(Offset::new(50., 20.), Offset::new(0., -40.)));
    assert_eq!(inner.offset(), 0.);
    assert_eq!(outer.offset(), 60.);
}
