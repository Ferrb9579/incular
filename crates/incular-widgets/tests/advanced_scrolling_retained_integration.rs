use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use incular_config::Constraints;
use incular_widgets::internal::{TreeError, WidgetTree};
use incular_widgets::{
    Axis, CacheExtentStyle, ChangeReportingBehavior, ChildVicinity, Clip, Color, Column,
    DiagonalDragBehavior, DraggableScrollableActuator, DraggableScrollableSheet,
    FixedExtentScrollController, ListWheelScrollView, ListWheelViewport, Offset, RawScrollbar,
    RawScrollbarOrientation, RawScrollbarStyle, ScrollController, SingleChildScrollView, Size,
    SizedBox, TwoDimensionalChildDelegate, TwoDimensionalScrollView, TwoDimensionalViewport,
    WheelChildDelegate, Widget,
};

fn wheel_children(count: usize) -> Vec<Widget> {
    (0..count)
        .map(|index| Widget::box_(Size::new(80.0, 20.0), Color::WHITE).with_key(index as u64))
        .collect()
}

fn grid_delegate() -> TwoDimensionalChildDelegate<Widget> {
    TwoDimensionalChildDelegate::new(4, 5, |vicinity| {
        Some(Widget::box_(
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
    let retained = scrollbar.with_child(Widget::box_(Size::new(80.0, 100.0), Color::WHITE));
    assert_eq!(retained.debug_type_name(), "RawScrollbar");

    let constructor_widget = RawScrollbar::new(controller.clone())
        .with_child(Widget::box_(Size::new(80.0, 100.0), Color::WHITE));
    assert_eq!(constructor_widget.debug_type_name(), "RawScrollbar");

    let converted_widget: Widget = RawScrollbar::new(controller).into();
    assert_eq!(converted_widget.debug_type_name(), "RawScrollbar");
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

    let retained_viewport = Widget::from(viewport);
    assert_eq!(retained_viewport.debug_type_name(), "ListWheelViewport");

    let converted_viewport: Widget = ListWheelViewport::with_new_controller(
        20.0,
        WheelChildDelegate::children(wheel_children(3)),
    )
    .into();
    assert_eq!(converted_viewport.debug_type_name(), "ListWheelViewport");

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
    assert_eq!(
        retained_scroll_view.debug_type_name(),
        "ListWheelScrollView"
    );
}

#[test]
fn draggable_sheet_state_handoff_reset_and_retained_conversions_are_public() {
    let sheet: DraggableScrollableSheet<Widget> = DraggableScrollableSheet::new(|inner| {
        incular_widgets::SingleChildScrollView::new(Widget::box_(
            Size::new(100.0, 600.0),
            Color::WHITE,
        ))
        .controller(inner.clone())
        .into()
    })
    .extents(0.25, 1.0, 0.5)
    .expand(false)
    .should_close_on_min_extent(false)
    .snap([0.75], Duration::from_millis(80));
    let sheet_controller = sheet.controller();
    let (state, child) = sheet.mount();
    assert!(sheet_controller.is_attached());
    assert_eq!(child.debug_type_name(), "ScrollView");

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
    assert_eq!(retained_sheet.debug_type_name(), "DraggableScrollableSheet");

    let retained_actuator = actuator
        .clone()
        .with_child(Widget::box_(Size::new(10.0, 10.0), Color::WHITE));
    assert_eq!(
        retained_actuator.debug_type_name(),
        "DraggableScrollableActuator"
    );

    let converted_actuator: Widget = actuator.into();
    assert_eq!(
        converted_actuator.debug_type_name(),
        "DraggableScrollableActuator"
    );
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
    assert_eq!(
        retained_scroll_view.debug_type_name(),
        "TwoDimensionalScrollView"
    );

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

    let retained_viewport = Widget::from(viewport);
    assert_eq!(
        retained_viewport.debug_type_name(),
        "TwoDimensionalViewport"
    );

    let converted_viewport: Widget = TwoDimensionalViewport::new(
        grid_delegate(),
        ScrollController::new(),
        ScrollController::new(),
        20.0,
        30.0,
    )
    .into();
    assert_eq!(
        converted_viewport.debug_type_name(),
        "TwoDimensionalViewport"
    );
}

#[test]
fn retained_two_dimensional_runtime_cache_survives_compatible_config_updates() {
    let builds = Rc::new(Cell::new(0_usize));
    let builds_for_delegate = builds.clone();
    let delegate = TwoDimensionalChildDelegate::new(20, 20, move |vicinity| {
        builds_for_delegate.set(builds_for_delegate.get() + 1);
        Some(
            Widget::box_(Size::new(20.0, 20.0), Color::WHITE)
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
        .mount(Widget::from(first))
        .expect("two-dimensional viewport mounts");
    tree.layout(constraints).expect("initial layout");
    let initial_builds = builds.get();
    assert!(initial_builds > 0);

    let mut updated = TwoDimensionalViewport::new(delegate, horizontal, vertical, 20.0, 20.0);
    updated.set_cache_extent(0.0, CacheExtentStyle::Pixels);
    updated.set_clip_behavior(Clip::AntiAlias);
    tree.update(root, Widget::from(updated))
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
        incular_widgets::SingleChildScrollView::new(Widget::box_(
            Size::new(100.0, 600.0),
            Color::WHITE,
        ))
        .controller(inner.clone())
        .into()
    })
    .extents(0.25, 1.0, 0.5)
    .expand(false);
    let first_controller = first.controller();
    let constraints = Constraints::tight(Size::new(120.0, 400.0));

    let mut tree = WidgetTree::new();
    let root = tree.mount(Widget::from(first)).expect("sheet mounts");
    tree.layout(constraints).expect("initial sheet layout");
    assert!(first_controller.jump_to(0.75));
    assert_eq!(first_controller.size(), Some(0.75));

    let replacement_builds = Rc::new(Cell::new(0_usize));
    let replacement_builds_for_builder = replacement_builds.clone();
    let replacement = DraggableScrollableSheet::new(move |inner| {
        replacement_builds_for_builder.set(replacement_builds_for_builder.get() + 1);
        incular_widgets::SingleChildScrollView::new(Widget::box_(
            Size::new(100.0, 800.0),
            Color::WHITE,
        ))
        .controller(inner.clone())
        .into()
    })
    .extents(0.2, 0.8, 0.4)
    .expand(false);
    let replacement_controller = replacement.controller();

    tree.update(root, Widget::from(replacement))
        .expect("compatible sheet update");
    tree.layout(constraints).expect("updated sheet layout");

    assert!(!first_controller.is_attached());
    assert!(replacement_controller.is_attached());
    assert_eq!(replacement_controller.size(), Some(0.75));
    assert_eq!(replacement_builds.get(), 1);
}

#[test]
fn two_wheels_competing_for_one_controller_rejected() {
    // Same metric-owner principle as ordinary viewports: the second live
    // wheel fails before overwriting the shared record, with both
    // viewport identities, while the first keeps its range.
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let wheel = |controller: ScrollController, items: usize| -> Widget {
        let view: Widget = ListWheelScrollView::new(
            controller,
            20.0,
            WheelChildDelegate::children(wheel_children(items)),
        )
        .into();
        SizedBox::from_dimensions(Some(200.), Some(100.), Some(view)).into()
    };
    let root = tree
        .mount(
            Column::new(vec![
                wheel(controller.clone(), 8),
                wheel(controller.clone(), 5),
            ])
            .into(),
        )
        .expect("mount defers attachment");
    let error = tree
        .layout(Constraints::tight(Size::new(200., 250.)))
        .unwrap_err();
    match error {
        TreeError::DuplicateScrollAttachment {
            owner_tree,
            owner,
            attempted,
        } => {
            assert_eq!(owner_tree, tree.tree_id());
            assert!(owner.is_some());
            assert_ne!(owner, Some(attempted));
        }
        other => panic!("unexpected failure: {other:?}"),
    }
    // First wheel's range stands (8 items): the rejected second wrote
    // nothing.
    assert_eq!(controller.max_offset(), 140.);
    let _ = root;
}

#[test]
fn ordinary_and_wheel_sharing_one_controller_rejected() {
    // An ordinary viewport and a wheel viewport are different writers to
    // the same record: the second one fails regardless of family.
    let controller = ScrollController::new();
    let scrolled: Widget =
        SingleChildScrollView::new(Widget::box_(Size::new(200., 300.), Color::WHITE))
            .controller(controller.clone())
            .into();
    let ordinary: Widget = SizedBox::from_dimensions(Some(200.), Some(100.), Some(scrolled)).into();
    let wheel_view: Widget = ListWheelScrollView::new(
        controller.clone(),
        20.0,
        WheelChildDelegate::children(wheel_children(8)),
    )
    .into();
    let wheeled: Widget =
        SizedBox::from_dimensions(Some(200.), Some(100.), Some(wheel_view)).into();
    let mut tree = WidgetTree::new();
    tree.mount(Column::new(vec![ordinary, wheeled]).into())
        .expect("mount defers attachment");
    tree.layout(Constraints::tight(Size::new(200., 250.)))
        .expect_err("mixed-family sharing is rejected");
    // The ordinary viewport laid out first and owns the record.
    assert_eq!(controller.content_extent(), 300.);
    assert_eq!(controller.viewport_extent(), 100.);
    assert_eq!(controller.max_offset(), 200.);
}

#[test]
fn fixed_extent_wrapper_shares_without_claiming() {
    // The wrapper is not a viewport: pre-attachment operations and reads
    // never claim ownership, while two wheels built from its single inner
    // handle still compete (proved by the rejection below, not by
    // labeling).
    let fixed = FixedExtentScrollController::new(2);
    assert_eq!(fixed.controller().metric_owner(), None);
    assert_eq!(fixed.selected_item(20.0), 2);
    assert!(fixed.jump_to_item(4, 20.0));
    assert_eq!(fixed.controller().metric_owner(), None);
    let first_wheel: Widget = ListWheelScrollView::new(
        fixed.controller(),
        20.0,
        WheelChildDelegate::children(wheel_children(8)),
    )
    .into();
    let second_wheel: Widget = ListWheelScrollView::new(
        fixed.controller(),
        20.0,
        WheelChildDelegate::children(wheel_children(8)),
    )
    .into();
    let boxed_first: Widget =
        SizedBox::from_dimensions(Some(200.), Some(100.), Some(first_wheel)).into();
    let boxed_second: Widget =
        SizedBox::from_dimensions(Some(200.), Some(100.), Some(second_wheel)).into();
    let mut tree = WidgetTree::new();
    tree.mount(Column::new(vec![boxed_first, boxed_second]).into())
        .expect("mount defers attachment");
    tree.layout(Constraints::tight(Size::new(200., 250.)))
        .expect_err("two wheels on one inner controller compete");
    assert_eq!(fixed.controller().max_offset(), 140.);
}

#[test]
fn contested_headless_write_never_disturbs_the_live_owner() {
    // Metric writes are last-writer-wins, but ownership is not: a
    // headless wheel-model layout over an owned controller writes
    // geometry without claiming, so the live attachment (identity, tree,
    // offset) is undisturbed — and the owner's next layout restores its
    // geometry deterministically.
    let controller = ScrollController::new();
    let ordinary = |content_height: f32| -> Widget {
        let scrolled: Widget =
            SingleChildScrollView::new(Widget::box_(Size::new(200., content_height), Color::WHITE))
                .controller(controller.clone())
                .into();
        SizedBox::from_dimensions(Some(200.), Some(100.), Some(scrolled)).into()
    };
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Column::new(vec![ordinary(300.)]).into())
        .expect("mount defers attachment");
    tree.layout(Constraints::tight(Size::new(200., 150.)))
        .expect("layout");
    assert!(controller.jump_to(30.));
    assert_eq!(controller.max_offset(), 200.);
    let attachment = controller.attachment_id().expect("ordinary owns X");
    assert_eq!(controller.metric_owner(), Some(tree.tree_id()));
    // Headless contest: no element, no claim — but the write lands.
    let mut model = ListWheelViewport::new(
        controller.clone(),
        20.0,
        WheelChildDelegate::children(wheel_children(8)),
    );
    let written = model.layout(Size::new(100.0, 100.0));
    assert!(written.selected_index.is_some());
    assert_eq!(controller.max_offset(), 140.);
    // Ownership intact: same attachment, same tree, same offset. A
    // clean relayout is correctly a no-op, so the contested values
    // stand until the owner really lays out again.
    assert_eq!(controller.attachment_id(), Some(attachment));
    assert_eq!(controller.metric_owner(), Some(tree.tree_id()));
    assert_eq!(controller.offset(), 30.);
    tree.layout(Constraints::tight(Size::new(200., 150.)))
        .expect("layout");
    assert_eq!(controller.max_offset(), 140.);
    // The owner's next real layout restores authoritative geometry on
    // the untouched attachment: new content overwrites the contested
    // values with the owner's own measurements.
    tree.update(root, Column::new(vec![ordinary(320.)]).into())
        .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 150.)))
        .expect("layout");
    assert_eq!(controller.content_extent(), 320.);
    assert_eq!(controller.viewport_extent(), 100.);
    assert_eq!(controller.max_offset(), 220.);
    assert_eq!(controller.attachment_id(), Some(attachment));
}

#[test]
fn headless_wheel_publication_rejected_while_attached() {
    // A headless wheel model sharing an attached controller publishes
    // through the checked path: rejection preserves content extent,
    // viewport extent, offset, revision, ownership, and notifications,
    // and the owner drives on undisturbed.
    let controller = ScrollController::new();
    let scrolled: Widget =
        SingleChildScrollView::new(Widget::box_(Size::new(200., 300.), Color::WHITE))
            .controller(controller.clone())
            .into();
    let ordinary: Widget = SizedBox::from_dimensions(Some(200.), Some(100.), Some(scrolled)).into();
    let mut tree = WidgetTree::new();
    tree.mount(Column::new(vec![ordinary]).into())
        .expect("mount defers attachment");
    tree.layout(Constraints::tight(Size::new(200., 150.)))
        .expect("layout");
    assert!(controller.jump_to(30.));
    let attachment = controller.attachment_id().expect("ordinary owns");
    let revision = controller.revision();
    let mut model = ListWheelViewport::new(
        controller.clone(),
        20.0,
        WheelChildDelegate::children(wheel_children(8)),
    );
    let error = match model.layout_unattached(Size::new(100.0, 100.0)) {
        Ok(_) => panic!("owned controller refuses headless wheel publication"),
        Err(error) => error,
    };
    assert_eq!(error.owner_tree(), Some(tree.tree_id()));
    assert_eq!(controller.content_extent(), 300.);
    assert_eq!(controller.viewport_extent(), 100.);
    assert_eq!(controller.max_offset(), 200.);
    assert_eq!(controller.offset(), 30.);
    assert_eq!(controller.revision(), revision);
    assert_eq!(controller.attachment_id(), Some(attachment));
    assert_eq!(controller.metric_owner(), Some(tree.tree_id()));
    tree.layout(Constraints::tight(Size::new(200., 150.)))
        .expect("owner drives on");
    assert_eq!(controller.max_offset(), 200.);
}

#[test]
fn ordinary_to_wheel_lifecycle_through_teardown_and_remount() {
    // One controller across families and teardown: ordinary owns X; a
    // wheel claim on X is rejected without disturbing the owner; after
    // detach the wheel takes X; the wheel swaps X for Y; dropping the
    // tree (window-teardown shape) frees the live controller silently;
    // the replaced-away handle remounts as a wheel elsewhere.
    let owned = ScrollController::new();
    let next = ScrollController::new();
    let ordinary = |controller: ScrollController| -> Widget {
        let scrolled: Widget =
            SingleChildScrollView::new(Widget::box_(Size::new(200., 300.), Color::WHITE))
                .controller(controller)
                .into();
        SizedBox::from_dimensions(Some(200.), Some(100.), Some(scrolled)).into()
    };
    let wheel = |controller: ScrollController| -> Widget {
        let view: Widget = ListWheelScrollView::new(
            controller,
            20.0,
            WheelChildDelegate::children(wheel_children(8)),
        )
        .into();
        SizedBox::from_dimensions(Some(200.), Some(100.), Some(view)).into()
    };
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Column::new(vec![ordinary(owned.clone())]).into())
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(200., 150.)))
        .expect("layout");
    assert_eq!(owned.max_offset(), 200.);
    let first_attachment = owned.attachment_id().expect("ordinary owns X");
    tree.update(
        root,
        Column::new(vec![ordinary(owned.clone()), wheel(owned.clone())]).into(),
    )
    .expect("update");
    match tree
        .layout(Constraints::tight(Size::new(200., 250.)))
        .unwrap_err()
    {
        TreeError::DuplicateScrollAttachment {
            owner_tree,
            owner,
            attempted: _,
        } => {
            assert_eq!(owner_tree, tree.tree_id());
            assert!(owner.is_some());
        }
        other => panic!("unexpected failure: {other:?}"),
    }
    assert_eq!(owned.attachment_id(), Some(first_attachment));
    assert_eq!(owned.max_offset(), 200.);
    tree.update(root, Column::new(Vec::<Widget>::new()).into())
        .expect("unmount");
    tree.layout(Constraints::tight(Size::new(200., 150.)))
        .expect("layout");
    assert_eq!(owned.metric_owner(), None);
    let root = tree
        .mount(Column::new(vec![wheel(owned.clone())]).into())
        .expect("remount");
    tree.layout(Constraints::tight(Size::new(200., 150.)))
        .expect("layout");
    assert_eq!(owned.max_offset(), 140.);
    let wheel_attachment = owned.attachment_id().expect("wheel owns X");
    assert_ne!(wheel_attachment, first_attachment);
    tree.update(root, Column::new(vec![wheel(next.clone())]).into())
        .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 150.)))
        .expect("layout");
    assert_eq!(next.max_offset(), 140.);
    assert_eq!(owned.metric_owner(), None);
    assert!(next.begin_activity());
    drop(tree);
    assert_eq!(next.metric_owner(), None);
    assert!(next.begin_activity());
    assert!(next.end_activity());
    let mut abroad = WidgetTree::new();
    abroad
        .mount(Column::new(vec![wheel(owned.clone())]).into())
        .expect("mount");
    abroad
        .layout(Constraints::tight(Size::new(200., 150.)))
        .expect("layout");
    assert_eq!(owned.max_offset(), 140.);
}

#[test]
fn wheel_replacement_unmount_and_preattach_behave() {
    // Replacement swaps the driver with the old handle freed; unmount
    // releases for remount; deferred pre-attachment jumps apply on first
    // layout; reads never claim.
    let first = ScrollController::new();
    let second = ScrollController::new();
    let mut tree = WidgetTree::new();
    let wheel = |controller: ScrollController| -> Widget {
        let view: Widget = ListWheelScrollView::new(
            controller,
            20.0,
            WheelChildDelegate::children(wheel_children(8)),
        )
        .into();
        SizedBox::from_dimensions(Some(200.), Some(100.), Some(view)).into()
    };
    let root = tree
        .mount(Column::new(vec![wheel(first.clone())]).into())
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(200., 150.)))
        .expect("layout");
    assert_eq!(first.max_offset(), 140.);
    // Replacement: the new controller drives, the old one is free.
    tree.update(root, Column::new(vec![wheel(second.clone())]).into())
        .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 150.)))
        .expect("layout");
    assert_eq!(second.max_offset(), 140.);
    // The replaced-away handle is free across trees too: a foreign
    // mount claims it, and dropping that tree releases it again.
    {
        let mut foreign = WidgetTree::new();
        let abroad: Widget = ListWheelScrollView::new(
            first.clone(),
            20.0,
            WheelChildDelegate::children(wheel_children(8)),
        )
        .into();
        let boxed: Widget = SizedBox::from_dimensions(Some(200.), Some(100.), Some(abroad)).into();
        foreign
            .mount(Column::new(vec![boxed]).into())
            .expect("mount");
        foreign
            .layout(Constraints::tight(Size::new(200., 150.)))
            .expect("replaced-away handle remounts abroad");
        assert_eq!(first.max_offset(), 140.);
        assert_eq!(first.metric_owner(), Some(foreign.tree_id()));
    }
    assert_eq!(first.metric_owner(), None);
    // Unmount releases; remount elsewhere reclaims with new geometry.
    tree.update(root, Column::new(Vec::<Widget>::new()).into())
        .expect("unmount");
    tree.layout(Constraints::tight(Size::new(200., 150.)))
        .expect("layout");
    let root = tree
        .mount(Column::new(vec![wheel(first.clone())]).into())
        .expect("remount");
    tree.layout(Constraints::tight(Size::new(200., 150.)))
        .expect("layout");
    assert_eq!(first.max_offset(), 140.);
    let _ = root;
}
