#![allow(clippy::float_cmp)]
#![allow(dead_code)]

#[path = "../src/advanced_scrolling/mod.rs"]
mod advanced_scrolling;

use std::{cell::RefCell, rc::Rc, time::Duration};

use incular_config::{Axis, AxisDirection, Clip};
use incular_core::{Offset, Size};
use incular_scroll::ScrollController;

use advanced_scrolling::{
    CacheExtentStyle, ChildVicinity, DiagonalDragBehavior, TwoDimensionalChildDelegate,
    TwoDimensionalScrollView,
};
use advanced_scrolling::{
    ChangeReportingBehavior, FixedExtentScrollController, ListWheelViewport, WheelChildDelegate,
    WheelProjection,
};
use advanced_scrolling::{
    DraggableScrollableActuator, DraggableScrollableController, DraggableScrollableState,
    DraggableSnap,
};
use advanced_scrolling::{RawScrollbar, RawScrollbarOrientation, RawScrollbarStyle};

#[test]
fn wheel_projects_children_and_selects_by_fixed_extent() {
    let controller = ScrollController::new();
    let selected = Rc::new(RefCell::new(Vec::new()));
    let selected_for_callback = selected.clone();
    let mut viewport = ListWheelViewport::new(
        controller.clone(),
        20.0,
        WheelChildDelegate::children((0..12).collect::<Vec<_>>()),
    );
    viewport.set_projection(WheelProjection::new(2.0, 0.003, 0.0, 1.0));
    viewport.set_magnifier(true, 1.2);
    viewport.set_over_under_center_opacity(0.45);
    viewport.set_selection_callback(
        ChangeReportingBehavior::OnScrollUpdate,
        Some(move |index| selected_for_callback.borrow_mut().push(index)),
    );

    let first = viewport.layout(Size::new(100.0, 100.0));
    assert_eq!(controller.max_offset(), 220.0);
    assert_eq!(first.selected_index, Some(0));
    let center = first
        .children
        .iter()
        .find(|child| child.in_center)
        .expect("center child");
    assert!(center.angle.abs() < 1.0e-6);
    assert!(center.transform.values[14].is_finite());
    assert_eq!(first.hit_test(Offset::new(50.0, 50.0)), Some(0));
    assert!(first.children.iter().any(|child| child.angle.abs() > 0.0));
    assert!(first.children.iter().any(|child| child.opacity < 1.0));

    assert!(viewport.jump_to_item(5));
    let fifth = viewport.layout(Size::new(100.0, 100.0));
    assert_eq!(fifth.selected_index, Some(5));
    assert_eq!(viewport.selected_item(), Some(5));
    assert!(selected.borrow().contains(&5));

    let fixed = FixedExtentScrollController::with_item_extent(3, 20.0);
    let fixed_controller = fixed.controller();
    fixed_controller.update_extents(260.0, 100.0);
    assert_eq!(fixed.selected_item(20.0), 3);
    assert!(fixed.jump_to_item(7, 20.0));
    assert_eq!(fixed.selected_item(20.0), 7);
}

#[test]
fn raw_scrollbar_maps_vertical_and_horizontal_thumb_drags() {
    let controller = ScrollController::new();
    controller.set_metrics_context(Axis::Vertical, false);
    controller.update_extents(500.0, 100.0);
    controller.jump_to(200.0);
    let mut scrollbar = RawScrollbar::new(controller.clone());
    let size = Size::new(120.0, 100.0);
    let geometry = scrollbar.geometry(size);
    assert!(geometry.visible);
    assert_eq!(geometry.thumb.size.height, 24.0);
    assert!((geometry.thumb.origin.y - 38.0).abs() < 1.0e-5);
    let thumb_point = Offset::new(
        geometry.thumb.origin.x + geometry.thumb.size.width * 0.5,
        geometry.thumb.origin.y + geometry.thumb.size.height * 0.5,
    );
    assert!(scrollbar.pointer_down(size, thumb_point));
    assert!(scrollbar.is_dragging());
    assert!(scrollbar.pointer_move(size, Offset::new(116.0, 88.0)));
    assert!((controller.offset() - controller.max_offset()).abs() < 1.0e-5);
    scrollbar.pointer_up();
    assert!(!scrollbar.is_dragging());

    controller.set_metrics_context(Axis::Horizontal, false);
    let mut horizontal = RawScrollbar::new(controller.clone());
    horizontal.set_style(RawScrollbarStyle {
        orientation: RawScrollbarOrientation::Bottom,
        ..RawScrollbarStyle::default()
    });
    let horizontal_geometry = horizontal.geometry(Size::new(120.0, 100.0));
    assert!(horizontal_geometry.visible);
    assert!(horizontal_geometry.track.size.width > horizontal_geometry.thumb.size.width);
    assert!(horizontal.hit_test(
        Size::new(120.0, 100.0),
        Offset::new(
            horizontal_geometry.thumb.origin.x + horizontal_geometry.thumb.size.width * 0.5,
            horizontal_geometry.thumb.origin.y + 4.0,
        ),
    ));
}

#[test]
fn draggable_sheet_resizes_hands_off_and_actuator_resets() {
    let controller = DraggableScrollableController::new();
    let state = DraggableScrollableState::new(
        0.25,
        1.0,
        0.5,
        true,
        DraggableSnap {
            enabled: true,
            sizes: vec![0.5, 0.75],
            animation_duration: Duration::from_millis(100),
        },
        controller.clone(),
    );
    let notifications = Rc::new(RefCell::new(Vec::new()));
    let notifications_for_listener = notifications.clone();
    let _subscription = state.add_notification_listener(move |notification| {
        notifications_for_listener.borrow_mut().push(notification);
        false
    });
    state.set_parent_height(400.0);
    state.set_inner_extents(1_000.0, 200.0);

    let resize = state.apply_user_offset(-40.0);
    assert!((state.extent().current_size - 0.6).abs() < 1.0e-5);
    assert!(
        (resize.sheet_consumed + 40.0).abs() < 2.0e-4,
        "delta={resize:?}, extent={:?}",
        state.extent()
    );
    assert_eq!(resize.inner_consumed, 0.0);

    let handoff = state.apply_user_offset(400.0);
    assert_eq!(state.extent().current_size, 0.25);
    assert!(handoff.sheet_consumed > 0.0);
    assert!(handoff.inner_consumed > 0.0);
    assert!(!notifications.borrow().is_empty());

    let target = state.snap_target(0.0).expect("snap target");
    assert_eq!(target.size, 0.25);
    let mut animation = controller
        .animate_to(0.75, Duration::from_millis(100))
        .expect("attached animation");
    assert!(animation.tick(Duration::from_millis(50)));
    assert!(state.extent().current_size > 0.5);
    assert!(!animation.tick(Duration::from_millis(50)));
    assert_eq!(state.extent().current_size, 0.75);

    let actuator = DraggableScrollableActuator::new();
    state.attach_actuator(&actuator);
    state.inner_controller().jump_to(300.0);
    assert!(actuator.reset());
    assert_eq!(state.extent().current_size, 0.5);
    assert_eq!(state.inner_controller().offset(), 0.0);
}

#[test]
fn two_dimensional_viewport_keeps_axis_ranges_cache_and_hit_tests_independent() {
    let delegate = TwoDimensionalChildDelegate::new(10, 12, Some);
    let mut view = TwoDimensionalScrollView::new(delegate, 20.0, 30.0);
    view.viewport_mut()
        .set_cache_extent(0.0, CacheExtentStyle::Pixels);
    view.viewport_mut().set_clip_behavior(Clip::HardEdge);
    let initial = view.layout(Size::new(90.0, 60.0));
    assert_eq!(initial.content_size, Size::new(360.0, 200.0));
    assert_eq!(
        view.scrollable().horizontal_controller().max_offset(),
        270.0
    );
    assert_eq!(view.scrollable().vertical_controller().max_offset(), 140.0);
    assert!(initial.row_range.len() <= 4);
    assert!(initial.column_range.len() <= 4);
    assert_eq!(
        initial.hit_test(Offset::new(15.0, 10.0)),
        Some(ChildVicinity::new(0, 0))
    );
    assert!(
        initial
            .children
            .iter()
            .all(|child| child.constraints.max_width == 30.0)
    );

    view.scrollable_mut()
        .set_diagonal_drag_behavior(DiagonalDragBehavior::Free);
    let delta = view.apply_delta(Offset::new(45.0, 35.0));
    assert_eq!(delta.horizontal.consumed, 45.0);
    assert_eq!(delta.vertical.consumed, 35.0);
    let scrolled = view.layout(Size::new(90.0, 60.0));
    assert!(
        scrolled
            .children
            .iter()
            .any(|child| child.vicinity.x_index > 0)
    );
    assert!(
        scrolled
            .children
            .iter()
            .any(|child| child.vicinity.y_index > 0)
    );

    view.viewport_mut()
        .set_axis_directions(AxisDirection::Left, AxisDirection::Up);
    let reversed = view.layout(Size::new(90.0, 60.0));
    let first_visible = reversed
        .children
        .iter()
        .find(|child| child.visible)
        .expect("visible reversed child");
    assert!(first_visible.paint_offset.x <= 90.0);
    assert!(first_visible.paint_offset.y <= 60.0);
    assert_eq!(
        reversed.horizontal_sliver_constraints.axis,
        Axis::Horizontal
    );
    assert_eq!(reversed.vertical_sliver_constraints.axis, Axis::Vertical);

    let row_revision = view.viewport().row_revision();
    let measured =
        view.viewport_mut()
            .layout_with_measure(Size::new(90.0, 60.0), |vicinity, constraints| {
                Size::new(
                    (10.0 + vicinity.x_index as f32).min(constraints.max_width),
                    (8.0 + vicinity.y_index as f32).min(constraints.max_height),
                )
            });
    assert!(view.viewport().row_revision() > row_revision);
    assert!(measured.content_size.width < 360.0);
    assert!(measured.content_size.height < 200.0);
}
