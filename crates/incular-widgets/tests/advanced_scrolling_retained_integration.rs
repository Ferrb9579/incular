use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use incular_config::Constraints;
use incular_widgets::internal::{WidgetKind, WidgetTree};
use incular_widgets::{
    Axis, CacheExtentStyle, ChangeReportingBehavior, ChildVicinity, Clip, Color,
    DiagonalDragBehavior, DraggableScrollableActuator, DraggableScrollableSheet,
    ListWheelScrollView, ListWheelViewport, Offset, RawScrollbar, RawScrollbarOrientation,
    RawScrollbarStyle, ScrollController, Size, TwoDimensionalChildDelegate,
    TwoDimensionalScrollView, TwoDimensionalViewport, WheelChildDelegate, Widget,
};

fn wheel_children(count: usize) -> Vec<Widget> {
    (0..count)
        .map(|index| Widget::fixed_box(Size::new(80.0, 20.0), Color::WHITE).with_key(index as u64))
        .collect()
}

fn grid_delegate() -> TwoDimensionalChildDelegate<Widget> {
    TwoDimensionalChildDelegate::new(4, 5, |vicinity| {
        Some(Widget::fixed_box(
            Size::new(
                10.0 + vicinity.x_index as f32,
                10.0 + vicinity.y_index as f32,
            ),
            Color::WHITE,
        ))
    })
}

#[test]
fn raw_scrollbar_geometry_drag_and_retained_conversions_are_public() {
    let controller = ScrollController::new();
    controller.set_metrics_context(Axis::Vertical, false);
    controller.update_extents(500.0, 100.0);
    assert!(controller.jump_to(200.0));

    let viewport_size = Size::new(120.0, 100.0);
    let mut scrollbar = RawScrollbar::new(controller.clone());
    let geometry = scrollbar.geometry(viewport_size);
    assert!(geometry.visible);
    assert_eq!(geometry.content_extent, 500.0);
    assert_eq!(geometry.viewport_extent, 100.0);
    assert!(geometry.thumb_travel > 0.0);

    let thumb_center = Offset::new(
        geometry.thumb.origin.x + geometry.thumb.size.width * 0.5,
        geometry.thumb.origin.y + geometry.thumb.size.height * 0.5,
    );
    assert!(geometry.thumb_contains(thumb_center));
    assert!(scrollbar.hit_test(viewport_size, thumb_center));
    assert!(scrollbar.pointer_down(viewport_size, thumb_center));
    assert!(scrollbar.is_dragging());
    assert!(scrollbar.pointer_move(
        viewport_size,
        Offset::new(
            thumb_center.x,
            geometry.track.origin.y + geometry.track.size.height
        ),
    ));
    scrollbar.pointer_up();
    assert!(!scrollbar.is_dragging());
    assert!((controller.offset() - controller.max_offset()).abs() < 1.0e-5);

    scrollbar.set_style(RawScrollbarStyle {
        orientation: RawScrollbarOrientation::Left,
        ..RawScrollbarStyle::default()
    });
    let retained = scrollbar.with_child(Widget::fixed_box(Size::new(80.0, 100.0), Color::WHITE));
    match retained.kind().clone() {
        WidgetKind::RawScrollbar {
            controller: retained_controller,
            style,
            child,
        } => {
            assert_eq!(retained_controller, controller);
            assert_eq!(style.orientation, RawScrollbarOrientation::Left);
            assert!(matches!(child.kind().clone(), WidgetKind::Box { .. }));
        }
        _ => panic!("RawScrollbar::with_child did not retain a RawScrollbar widget"),
    }

    let constructor_widget = Widget::raw_scrollbar(
        controller.clone(),
        Widget::box_(Size::new(80.0, 100.0), Color::WHITE),
    );
    assert!(matches!(
        constructor_widget.kind().clone(),
        WidgetKind::RawScrollbar { .. }
    ));

    let converted_widget: Widget = RawScrollbar::new(controller).into();
    assert!(matches!(
        converted_widget.kind().clone(),
        WidgetKind::RawScrollbar { .. }
    ));
}

#[test]
fn wheel_viewport_and_scroll_view_layout_hit_test_and_retain_public_models() {
    let controller = ScrollController::new();
    let selected = Rc::new(RefCell::new(Vec::new()));
    let selected_for_callback = selected.clone();
    let mut viewport = ListWheelViewport::new(
        controller.clone(),
        20.0,
        WheelChildDelegate::children(wheel_children(8)),
    );
    viewport.set_selection_callback(
        ChangeReportingBehavior::OnScrollUpdate,
        Some(move |index| selected_for_callback.borrow_mut().push(index)),
    );

    let initial = viewport.layout(Size::new(100.0, 100.0));
    assert_eq!(initial.selected_index, Some(0));
    assert_eq!(initial.max_scroll_extent, 140.0);
    assert_eq!(initial.hit_test(Offset::new(50.0, 50.0)), Some(0));
    assert!(initial.children.iter().any(|child| child.visible));

    assert!(viewport.jump_to_item(3));
    let scrolled = viewport.layout(Size::new(100.0, 100.0));
    assert_eq!(scrolled.selected_index, Some(3));
    assert_eq!(viewport.selected_item(), Some(3));
    assert!(selected.borrow().contains(&3));

    let retained_viewport = Widget::list_wheel_viewport(viewport);
    assert!(matches!(
        retained_viewport.kind().clone(),
        WidgetKind::ListWheelViewport { .. }
    ));

    let converted_viewport: Widget = ListWheelViewport::with_new_controller(
        20.0,
        WheelChildDelegate::children(wheel_children(3)),
    )
    .into();
    assert!(matches!(
        converted_viewport.kind().clone(),
        WidgetKind::ListWheelViewport { .. }
    ));

    let scroll_controller = ScrollController::new();
    let mut scroll_view = ListWheelScrollView::new(
        scroll_controller,
        20.0,
        WheelChildDelegate::children(wheel_children(5)),
    );
    let scroll_layout = scroll_view.layout(Size::new(100.0, 80.0));
    assert_eq!(scroll_layout.selected_index, Some(0));
    assert_eq!(scroll_layout.hit_test(Offset::new(50.0, 40.0)), Some(0));

    let retained_scroll_view: Widget = scroll_view.into();
    assert!(matches!(
        retained_scroll_view.kind().clone(),
        WidgetKind::ListWheelScrollView { .. }
    ));
}

#[test]
fn draggable_sheet_state_handoff_reset_and_retained_conversions_are_public() {
    let sheet = DraggableScrollableSheet::new(|inner| {
        Widget::scroll_view(
            inner.clone(),
            Widget::fixed_box(Size::new(100.0, 600.0), Color::WHITE),
        )
    })
    .extents(0.25, 1.0, 0.5)
    .expand(false)
    .should_close_on_min_extent(false)
    .snap([0.75], Duration::from_millis(80));
    let sheet_controller = sheet.controller();
    let (state, child) = sheet.mount();
    assert!(sheet_controller.is_attached());
    assert!(matches!(child.kind().clone(), WidgetKind::Scroll { .. }));

    let notifications = Rc::new(RefCell::new(Vec::new()));
    let notifications_for_listener = notifications.clone();
    let _subscription = state.add_notification_listener(move |notification| {
        notifications_for_listener
            .borrow_mut()
            .push(notification.extent);
        false
    });
    state.set_parent_height(400.0);
    state.set_inner_extents(1_000.0, 200.0);
    assert!(state.set_size(0.75, true));
    assert!((state.extent().current_size - 0.75).abs() < 1.0e-5);
    assert!(notifications.borrow().contains(&0.75));

    let handoff = state.apply_user_offset(400.0);
    assert!(handoff.sheet_consumed.is_finite());
    assert!(handoff.inner_consumed.is_finite());
    assert!(handoff.unconsumed.is_finite());

    let actuator = DraggableScrollableActuator::new();
    state.attach_actuator(&actuator);
    assert!(state.inner_controller().jump_to(100.0));
    assert!(actuator.reset());
    assert!((state.extent().current_size - 0.5).abs() < 1.0e-5);
    assert_eq!(state.inner_controller().offset(), 0.0);
    assert!(!state.extent().has_dragged);

    let retained_sheet: Widget = sheet.into();
    assert!(matches!(
        retained_sheet.kind().clone(),
        WidgetKind::DraggableScrollableSheet { .. }
    ));

    let retained_actuator = actuator
        .clone()
        .with_child(Widget::box_(Size::new(10.0, 10.0), Color::WHITE));
    assert!(matches!(
        retained_actuator.kind().clone(),
        WidgetKind::DraggableScrollableActuator { .. }
    ));

    let converted_actuator: Widget = actuator.into();
    assert!(matches!(
        converted_actuator.kind().clone(),
        WidgetKind::DraggableScrollableActuator { .. }
    ));
}

#[test]
fn two_dimensional_views_keep_axis_state_hit_testing_and_retained_conversions() {
    let mut view = TwoDimensionalScrollView::new(grid_delegate(), 20.0, 30.0);
    view.viewport_mut()
        .set_cache_extent(0.0, CacheExtentStyle::Pixels);
    view.scrollable_mut()
        .set_diagonal_drag_behavior(DiagonalDragBehavior::Free);

    let initial = view.layout(Size::new(90.0, 60.0));
    assert!(initial.has_visual_overflow);
    assert_eq!(
        initial.hit_test(Offset::new(10.0, 10.0)),
        Some(ChildVicinity::new(0, 0))
    );
    assert!(view.scrollable().horizontal_controller().max_offset() > 0.0);
    assert!(view.scrollable().vertical_controller().max_offset() > 0.0);

    let delta = view.apply_delta(Offset::new(25.0, 15.0));
    assert!((delta.horizontal.consumed - 25.0).abs() < 1.0e-5);
    assert!((delta.vertical.consumed - 15.0).abs() < 1.0e-5);
    assert_eq!(view.scrollable().horizontal_controller().offset(), 25.0);
    assert_eq!(view.scrollable().vertical_controller().offset(), 15.0);

    let scrolled = view.layout(Size::new(90.0, 60.0));
    assert_eq!(
        scrolled.hit_test(Offset::new(10.0, 10.0)),
        Some(ChildVicinity::new(1, 1))
    );

    let retained_scroll_view: Widget = view.into();
    assert!(matches!(
        retained_scroll_view.kind().clone(),
        WidgetKind::TwoDimensionalScrollView { .. }
    ));

    let horizontal_controller = ScrollController::new();
    let vertical_controller = ScrollController::new();
    let mut viewport = TwoDimensionalViewport::new(
        grid_delegate(),
        horizontal_controller.clone(),
        vertical_controller.clone(),
        20.0,
        30.0,
    );
    viewport.set_cache_extent(0.0, CacheExtentStyle::Pixels);
    let viewport_layout = viewport.layout(Size::new(90.0, 60.0));
    assert_eq!(
        viewport_layout.hit_test(Offset::new(10.0, 10.0)),
        Some(ChildVicinity::new(0, 0))
    );
    assert!(horizontal_controller.max_offset() > 0.0);
    assert!(vertical_controller.max_offset() > 0.0);

    let retained_viewport = Widget::two_dimensional_viewport(viewport);
    assert!(matches!(
        retained_viewport.kind().clone(),
        WidgetKind::TwoDimensionalViewport { .. }
    ));

    let converted_viewport: Widget = TwoDimensionalViewport::new(
        grid_delegate(),
        ScrollController::new(),
        ScrollController::new(),
        20.0,
        30.0,
    )
    .into();
    assert!(matches!(
        converted_viewport.kind().clone(),
        WidgetKind::TwoDimensionalViewport { .. }
    ));
}

#[test]
fn retained_two_dimensional_runtime_cache_survives_compatible_config_updates() {
    let builds = Rc::new(Cell::new(0_usize));
    let builds_for_delegate = builds.clone();
    let delegate = TwoDimensionalChildDelegate::new(20, 20, move |vicinity| {
        builds_for_delegate.set(builds_for_delegate.get() + 1);
        Some(
            Widget::fixed_box(Size::new(20.0, 20.0), Color::WHITE)
                .with_key((vicinity.y_index * 20 + vicinity.x_index) as u64),
        )
    });
    let horizontal = ScrollController::new();
    let vertical = ScrollController::new();
    let constraints = Constraints::tight(Size::new(80.0, 60.0));

    let mut first = TwoDimensionalViewport::new(
        delegate.clone(),
        horizontal.clone(),
        vertical.clone(),
        20.0,
        20.0,
    );
    first.set_cache_extent(0.0, CacheExtentStyle::Pixels);

    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::two_dimensional_viewport(first))
        .expect("two-dimensional viewport mounts");
    tree.layout(constraints).expect("initial layout");
    let initial_builds = builds.get();
    assert!(initial_builds > 0);

    let mut updated = TwoDimensionalViewport::new(delegate, horizontal, vertical, 20.0, 20.0);
    updated.set_cache_extent(0.0, CacheExtentStyle::Pixels);
    updated.set_clip_behavior(Clip::AntiAlias);
    tree.update(root, Widget::two_dimensional_viewport(updated))
        .expect("compatible viewport update");
    tree.layout(constraints).expect("updated layout");

    assert_eq!(
        builds.get(),
        initial_builds,
        "a config-only update must reuse the retained cell cache"
    );
}

#[test]
fn retained_draggable_sheet_preserves_extent_and_rebinds_updated_config() {
    let first = DraggableScrollableSheet::new(|inner| {
        Widget::scroll_view(
            inner.clone(),
            Widget::fixed_box(Size::new(100.0, 600.0), Color::WHITE),
        )
    })
    .extents(0.25, 1.0, 0.5)
    .expand(false);
    let first_controller = first.controller();
    let constraints = Constraints::tight(Size::new(120.0, 400.0));

    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::draggable_scrollable_sheet(first))
        .expect("sheet mounts");
    tree.layout(constraints).expect("initial sheet layout");
    assert!(first_controller.jump_to(0.75));
    assert_eq!(first_controller.size(), Some(0.75));

    let replacement_builds = Rc::new(Cell::new(0_usize));
    let replacement_builds_for_builder = replacement_builds.clone();
    let replacement = DraggableScrollableSheet::new(move |inner| {
        replacement_builds_for_builder.set(replacement_builds_for_builder.get() + 1);
        Widget::scroll_view(
            inner.clone(),
            Widget::fixed_box(Size::new(100.0, 800.0), Color::WHITE),
        )
    })
    .extents(0.2, 0.8, 0.4)
    .expand(false);
    let replacement_controller = replacement.controller();

    tree.update(root, Widget::draggable_scrollable_sheet(replacement))
        .expect("compatible sheet update");
    tree.layout(constraints).expect("updated sheet layout");

    assert!(!first_controller.is_attached());
    assert!(replacement_controller.is_attached());
    assert_eq!(replacement_controller.size(), Some(0.75));
    assert_eq!(replacement_builds.get(), 1);
}
