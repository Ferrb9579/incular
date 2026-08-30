//! Focused behavior checks for the Flutter reorderable-list parity surface.

use incular_config::{Axis, Constraints};
use incular_core::{Color, Offset, PointerPhase, Size};
use incular_gestures::{PointerEvent, PointerGestureRecognizer};
use incular_scroll::ScrollController;
use incular_widgets::internal::{
    ReorderableDelayedDragStartListener, ReorderableDragStartListener, ReorderableList, Widget,
    WidgetTree,
};
use std::{cell::RefCell, rc::Rc, time::Duration};

fn dispatch(
    tree: &mut WidgetTree,
    pointer: u64,
    time: std::time::Instant,
    phase: PointerPhase,
    position: Offset,
) {
    let _ = tree.dispatch_gesture(PointerEvent {
        pointer,
        position,
        phase,
        time,
    });
}

fn row(index: usize) -> Widget {
    Widget::box_(Size::new(100., 40.), Color::rgba(index as u8, 0, 0, 255))
}

#[test]
fn reorder_controller_adjusts_legacy_and_final_destinations() {
    let controller = incular_widgets::internal::SliverReorderController::new(4);

    assert!(controller.reorder(1, 4));
    assert_eq!(controller.order(), vec![0, 2, 3, 1]);
    assert!(!controller.reorder(3, 3));

    assert!(controller.reorder(3, 1));
    assert_eq!(controller.order(), vec![0, 1, 2, 3]);
    assert!(!controller.reorder(4, 0));
}

#[test]
fn box_reorderable_list_uses_listener_index_and_flutter_callback_conventions() {
    let starts = Rc::new(RefCell::new(Vec::new()));
    let ends = Rc::new(RefCell::new(Vec::new()));
    let reorders = Rc::new(RefCell::new(Vec::new()));
    let scroll = ScrollController::new();
    let list = ReorderableList::new(4, |index| {
        ReorderableDragStartListener::new(index, row(index))
    })
    .item_extent(40.)
    .scroll_direction(Axis::Vertical)
    .controller(scroll.clone())
    .on_reorder({
        let reorders = reorders.clone();
        move |old, new| reorders.borrow_mut().push((old, new))
    })
    .on_reorder_start({
        let starts = starts.clone();
        move |index| starts.borrow_mut().push(index)
    })
    .on_reorder_end({
        let ends = ends.clone();
        move |index| ends.borrow_mut().push(index)
    });
    let order = list.reorder_controller();
    let mut tree = WidgetTree::new();
    tree.mount(list.into()).unwrap();
    tree.layout(Constraints::tight(Size::new(100., 140.)));

    assert_eq!(scroll.max_offset(), 20.);
    let now = std::time::Instant::now();
    dispatch(&mut tree, 1, now, PointerPhase::Down, Offset::new(10., 20.));
    dispatch(
        &mut tree,
        1,
        now + Duration::from_millis(1),
        PointerPhase::Move,
        Offset::new(10., 24.),
    );
    dispatch(
        &mut tree,
        1,
        now + Duration::from_millis(2),
        PointerPhase::Move,
        Offset::new(10., 130.),
    );
    dispatch(
        &mut tree,
        1,
        now + Duration::from_millis(3),
        PointerPhase::Up,
        Offset::new(10., 130.),
    );

    assert_eq!(order.order(), vec![1, 2, 3, 0]);
    assert_eq!(*starts.borrow(), vec![0]);
    assert_eq!(*reorders.borrow(), vec![(0, 4)]);
    assert_eq!(*ends.borrow(), vec![4]);
}

#[test]
fn adjusted_on_reorder_item_reports_post_removal_index_and_same_drop_ends() {
    let reorders = Rc::new(RefCell::new(Vec::new()));
    let ends = Rc::new(RefCell::new(Vec::new()));
    let list = ReorderableList::new(3, |index| {
        ReorderableDragStartListener::new(index, row(index))
    })
    .item_extent(40.)
    .on_reorder_item({
        let reorders = reorders.clone();
        move |old, new| reorders.borrow_mut().push((old, new))
    })
    .on_reorder_end({
        let ends = ends.clone();
        move |index| ends.borrow_mut().push(index)
    });
    let order = list.reorder_controller();
    let mut tree = WidgetTree::new();
    tree.mount(list.into()).unwrap();
    tree.layout(Constraints::tight(Size::new(100., 140.)));

    let now = std::time::Instant::now();
    dispatch(&mut tree, 2, now, PointerPhase::Down, Offset::new(10., 60.));
    dispatch(
        &mut tree,
        2,
        now + Duration::from_millis(1),
        PointerPhase::Move,
        Offset::new(10., 80.),
    );
    dispatch(
        &mut tree,
        2,
        now + Duration::from_millis(2),
        PointerPhase::Move,
        Offset::new(10., 60.),
    );
    dispatch(
        &mut tree,
        2,
        now + Duration::from_millis(3),
        PointerPhase::Up,
        Offset::new(10., 60.),
    );

    assert_eq!(order.order(), vec![0, 1, 2]);
    assert!(reorders.borrow().is_empty());
    assert_eq!(*ends.borrow(), vec![1]);
}

#[test]
fn delayed_listener_rejects_early_motion_and_starts_after_long_press() {
    let starts = Rc::new(RefCell::new(Vec::new()));
    let ends = Rc::new(RefCell::new(Vec::new()));
    let reorders = Rc::new(RefCell::new(Vec::new()));
    let list = ReorderableList::new(4, |index| {
        ReorderableDelayedDragStartListener::new(index, row(index))
    })
    .item_extent(40.)
    .on_reorder_item({
        let reorders = reorders.clone();
        move |old, new| reorders.borrow_mut().push((old, new))
    })
    .on_reorder_start({
        let starts = starts.clone();
        move |index| starts.borrow_mut().push(index)
    })
    .on_reorder_end({
        let ends = ends.clone();
        move |index| ends.borrow_mut().push(index)
    });
    let order = list.reorder_controller();
    let mut tree = WidgetTree::new();
    tree.mount(list.into()).unwrap();
    tree.layout(Constraints::tight(Size::new(100., 140.)));

    let now = std::time::Instant::now();
    dispatch(&mut tree, 3, now, PointerPhase::Down, Offset::new(10., 20.));
    dispatch(
        &mut tree,
        3,
        now + Duration::from_millis(100),
        PointerPhase::Move,
        Offset::new(10., 100.),
    );
    dispatch(
        &mut tree,
        3,
        now + Duration::from_millis(101),
        PointerPhase::Up,
        Offset::new(10., 100.),
    );
    assert_eq!(order.order(), vec![0, 1, 2, 3]);
    assert!(starts.borrow().is_empty());

    dispatch(
        &mut tree,
        4,
        now + Duration::from_secs(1),
        PointerPhase::Down,
        Offset::new(10., 20.),
    );
    dispatch(
        &mut tree,
        4,
        now + Duration::from_secs(1) + PointerGestureRecognizer::LONG_PRESS_TIMEOUT,
        PointerPhase::Move,
        Offset::new(10., 20.),
    );
    dispatch(
        &mut tree,
        4,
        now + Duration::from_secs(1)
            + PointerGestureRecognizer::LONG_PRESS_TIMEOUT
            + Duration::from_millis(1),
        PointerPhase::Move,
        Offset::new(10., 130.),
    );
    dispatch(
        &mut tree,
        4,
        now + Duration::from_secs(1)
            + PointerGestureRecognizer::LONG_PRESS_TIMEOUT
            + Duration::from_millis(2),
        PointerPhase::Up,
        Offset::new(10., 130.),
    );

    assert_eq!(order.order(), vec![1, 2, 3, 0]);
    assert_eq!(*starts.borrow(), vec![0]);
    assert_eq!(*reorders.borrow(), vec![(0, 3)]);
    assert_eq!(*ends.borrow(), vec![4]);
}

#[test]
fn box_reorderable_list_requires_an_enabled_listener_for_a_drag() {
    let order = {
        let list = ReorderableList::new(2, |index| row(index))
            .item_extent(40.)
            .with_reorder_controller(incular_widgets::internal::SliverReorderController::new(2));
        let order = list.reorder_controller();
        let mut tree = WidgetTree::new();
        tree.mount(list.into()).unwrap();
        tree.layout(Constraints::tight(Size::new(100., 80.)));
        let now = std::time::Instant::now();
        dispatch(&mut tree, 5, now, PointerPhase::Down, Offset::new(10., 20.));
        dispatch(
            &mut tree,
            5,
            now + Duration::from_millis(1),
            PointerPhase::Move,
            Offset::new(10., 65.),
        );
        dispatch(
            &mut tree,
            5,
            now + Duration::from_millis(2),
            PointerPhase::Up,
            Offset::new(10., 65.),
        );
        order.order()
    };
    assert_eq!(order, vec![0, 1]);

    let disabled = ReorderableDragStartListener::new(0, row(0)).enabled(false);
    let list = ReorderableList::new(2, move |index| {
        if index == 0 {
            disabled.clone()
        } else {
            ReorderableDragStartListener::new(index, row(index))
        }
    })
    .item_extent(40.);
    let order = list.reorder_controller();
    let mut tree = WidgetTree::new();
    tree.mount(list.into()).unwrap();
    tree.layout(Constraints::tight(Size::new(100., 80.)));
    let now = std::time::Instant::now();
    dispatch(&mut tree, 6, now, PointerPhase::Down, Offset::new(10., 20.));
    dispatch(
        &mut tree,
        6,
        now + Duration::from_millis(1),
        PointerPhase::Move,
        Offset::new(10., 80.),
    );
    dispatch(
        &mut tree,
        6,
        now + Duration::from_millis(2),
        PointerPhase::Up,
        Offset::new(10., 80.),
    );
    assert_eq!(order.order(), vec![0, 1]);
}
