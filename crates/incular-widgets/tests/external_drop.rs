use incular_config::Constraints;
use incular_core::{Color, Offset, Size};
use incular_platform::{
    DataTransfer, ExternalDragEvent, ExternalDragPhase, TransferOperation, TransferOperations,
};
use incular_widgets::extensions::ExternalDropTarget;
use incular_widgets::internal::WidgetTree;
use incular_widgets::{Column, DragDropContext, DragTarget, Widget};
use std::{cell::Cell, path::PathBuf, rc::Rc};

fn file_event(phase: ExternalDragPhase, position: Offset) -> ExternalDragEvent {
    file_event_with_operations(phase, position, TransferOperations::COPY)
}

fn file_event_with_operations(
    phase: ExternalDragPhase,
    position: Offset,
    allowed_operations: TransferOperations,
) -> ExternalDragEvent {
    ExternalDragEvent {
        phase,
        transfer: DataTransfer::files([PathBuf::from("report.pdf")]),
        position,
        allowed_operations,
    }
}

#[test]
fn external_drop_enters_updates_and_drops_exactly_once() {
    let entered = Rc::new(Cell::new(0));
    let updated = Rc::new(Cell::new(0));
    let dropped = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    tree.mount(
        ExternalDropTarget::new(Widget::box_(Size::new(100.0, 80.0), Color::BLACK))
            .on_enter({
                let entered = entered.clone();
                move |_, operation| {
                    assert_eq!(operation, TransferOperation::Copy);
                    entered.set(entered.get() + 1);
                }
            })
            .on_update({
                let updated = updated.clone();
                move |_, operation| {
                    assert_eq!(operation, TransferOperation::Copy);
                    updated.set(updated.get() + 1);
                }
            })
            .on_drop({
                let dropped = dropped.clone();
                move |event, operation| {
                    assert_eq!(operation, TransferOperation::Copy);
                    assert_eq!(event.transfer.items().len(), 1);
                    dropped.set(dropped.get() + 1);
                }
            })
            .into(),
    )
    .expect("mount external target");
    tree.layout(Constraints::tight(Size::new(100.0, 80.0)))
        .expect("layout external target");

    let enter = tree.dispatch_external_drag(file_event(
        ExternalDragPhase::Enter,
        Offset::new(20.0, 20.0),
    ));
    assert_eq!(enter.requested_operation, Some(TransferOperation::Copy));
    let over =
        tree.dispatch_external_drag(file_event(ExternalDragPhase::Over, Offset::new(25.0, 20.0)));
    assert_eq!(over.requested_operation, Some(TransferOperation::Copy));
    let drop =
        tree.dispatch_external_drag(file_event(ExternalDragPhase::Drop, Offset::new(25.0, 20.0)));
    assert_eq!(drop.requested_operation, Some(TransferOperation::Copy));
    assert_eq!(entered.get(), 1);
    assert_eq!(updated.get(), 1, "only the over phase is an update sample");
    assert_eq!(dropped.get(), 1);
}

#[test]
fn external_drop_leave_and_cancel_have_distinct_lifecycle_callbacks() {
    let left = Rc::new(Cell::new(0));
    let cancelled = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    tree.mount(
        ExternalDropTarget::new(Widget::box_(Size::new(80.0, 80.0), Color::BLACK))
            .on_leave({
                let left = left.clone();
                move |_| left.set(left.get() + 1)
            })
            .on_cancel({
                let cancelled = cancelled.clone();
                move |_| cancelled.set(cancelled.get() + 1)
            })
            .into(),
    )
    .unwrap();
    tree.layout(Constraints::tight(Size::new(80.0, 80.0)))
        .unwrap();

    let _ = tree.dispatch_external_drag(file_event(
        ExternalDragPhase::Enter,
        Offset::new(10.0, 10.0),
    ));
    let _ = tree.dispatch_external_drag(file_event(
        ExternalDragPhase::Leave,
        Offset::new(100.0, 10.0),
    ));
    assert_eq!(left.get(), 1);
    assert_eq!(cancelled.get(), 0);

    let _ = tree.dispatch_external_drag(file_event(
        ExternalDragPhase::Enter,
        Offset::new(10.0, 10.0),
    ));
    let _ = tree.dispatch_external_drag(file_event(
        ExternalDragPhase::Cancel,
        Offset::new(10.0, 10.0),
    ));
    assert_eq!(left.get(), 1);
    assert_eq!(cancelled.get(), 1);
}

#[test]
fn target_cannot_request_unadvertised_move_operation() {
    let entered = Rc::new(Cell::new(0));
    let dropped = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    tree.mount(
        ExternalDropTarget::new(Widget::box_(Size::new(80.0, 80.0), Color::BLACK))
            .on_will_accept(|_| Some(TransferOperation::Move))
            .on_enter({
                let entered = entered.clone();
                move |_, _| entered.set(entered.get() + 1)
            })
            .on_drop({
                let dropped = dropped.clone();
                move |_, _| dropped.set(dropped.get() + 1)
            })
            .into(),
    )
    .unwrap();
    tree.layout(Constraints::tight(Size::new(80.0, 80.0)))
        .unwrap();

    let response = tree.dispatch_external_drag(file_event(
        ExternalDragPhase::Enter,
        Offset::new(10.0, 10.0),
    ));
    assert_eq!(response.requested_operation, None);
    let response =
        tree.dispatch_external_drag(file_event(ExternalDragPhase::Drop, Offset::new(10.0, 10.0)));
    assert_eq!(response.requested_operation, None);
    assert_eq!(entered.get(), 0);
    assert_eq!(dropped.get(), 0);
}

#[test]
fn changing_operation_on_same_target_updates_without_reentering() {
    let entered = Rc::new(Cell::new(0));
    let updated = Rc::new(Cell::new(0));
    let left = Rc::new(Cell::new(0));
    let updated_operation = Rc::new(Cell::new(None));
    let mut tree = WidgetTree::new();
    tree.mount(
        ExternalDropTarget::new(Widget::box_(Size::new(80.0, 80.0), Color::BLACK))
            .on_will_accept(|event| {
                if event.allowed_operations.contains(TransferOperation::Link) {
                    Some(TransferOperation::Link)
                } else {
                    Some(TransferOperation::Copy)
                }
            })
            .on_enter({
                let entered = entered.clone();
                move |event, operation| {
                    assert_eq!(event.phase, ExternalDragPhase::Enter);
                    assert_eq!(operation, TransferOperation::Copy);
                    entered.set(entered.get() + 1);
                }
            })
            .on_update({
                let updated = updated.clone();
                let updated_operation = updated_operation.clone();
                move |event, operation| {
                    assert_eq!(event.phase, ExternalDragPhase::Over);
                    updated.set(updated.get() + 1);
                    updated_operation.set(Some(operation));
                }
            })
            .on_leave({
                let left = left.clone();
                move |_| left.set(left.get() + 1)
            })
            .into(),
    )
    .unwrap();
    tree.layout(Constraints::tight(Size::new(80.0, 80.0)))
        .unwrap();

    let enter = tree.dispatch_external_drag(file_event_with_operations(
        ExternalDragPhase::Enter,
        Offset::new(10.0, 10.0),
        TransferOperations::COPY,
    ));
    assert_eq!(enter.requested_operation, Some(TransferOperation::Copy));

    let over = tree.dispatch_external_drag(file_event_with_operations(
        ExternalDragPhase::Over,
        Offset::new(20.0, 10.0),
        TransferOperations::LINK,
    ));
    assert_eq!(over.requested_operation, Some(TransferOperation::Link));
    assert_eq!(entered.get(), 1);
    assert_eq!(updated.get(), 1);
    assert_eq!(updated_operation.get(), Some(TransferOperation::Link));
    assert_eq!(left.get(), 0);
}

#[test]
fn moving_between_targets_emits_normalized_lifecycle_phases() {
    let first_leave_phase = Rc::new(Cell::new(None));
    let second_enter_phase = Rc::new(Cell::new(None));
    let first: Widget = ExternalDropTarget::new(Widget::box_(Size::new(80.0, 40.0), Color::BLACK))
        .on_leave({
            let first_leave_phase = first_leave_phase.clone();
            move |event| first_leave_phase.set(Some(event.phase))
        })
        .into();
    let second: Widget = ExternalDropTarget::new(Widget::box_(Size::new(80.0, 40.0), Color::BLACK))
        .on_enter({
            let second_enter_phase = second_enter_phase.clone();
            move |event, _| second_enter_phase.set(Some(event.phase))
        })
        .into();
    let mut tree = WidgetTree::new();
    tree.mount(Column::new([first, second]).into()).unwrap();
    tree.layout(Constraints::tight(Size::new(80.0, 80.0)))
        .unwrap();

    let _ = tree.dispatch_external_drag(file_event(
        ExternalDragPhase::Enter,
        Offset::new(10.0, 10.0),
    ));
    let _ =
        tree.dispatch_external_drag(file_event(ExternalDragPhase::Over, Offset::new(10.0, 60.0)));

    assert_eq!(first_leave_phase.get(), Some(ExternalDragPhase::Leave));
    assert_eq!(second_enter_phase.get(), Some(ExternalDragPhase::Enter));
}

#[test]
fn compatible_rebuild_uses_current_external_drop_callback_generation() {
    let old_drop = Rc::new(Cell::new(0));
    let new_drop = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            ExternalDropTarget::new(Widget::box_(Size::new(80.0, 80.0), Color::BLACK))
                .on_drop({
                    let old_drop = old_drop.clone();
                    move |_, _| old_drop.set(old_drop.get() + 1)
                })
                .into(),
        )
        .unwrap();
    tree.layout(Constraints::tight(Size::new(80.0, 80.0)))
        .unwrap();
    let _ = tree.dispatch_external_drag(file_event(
        ExternalDragPhase::Enter,
        Offset::new(10.0, 10.0),
    ));

    tree.update(
        root,
        ExternalDropTarget::new(Widget::box_(Size::new(80.0, 80.0), Color::BLACK))
            .on_drop({
                let new_drop = new_drop.clone();
                move |_, _| new_drop.set(new_drop.get() + 1)
            })
            .into(),
    )
    .unwrap();
    tree.layout(Constraints::tight(Size::new(80.0, 80.0)))
        .unwrap();
    let _ =
        tree.dispatch_external_drag(file_event(ExternalDragPhase::Drop, Offset::new(10.0, 10.0)));
    assert_eq!(old_drop.get(), 0);
    assert_eq!(new_drop.get(), 1);
}

#[test]
fn unmounted_external_target_is_cancelled_and_never_receives_drop() {
    let cancelled = Rc::new(Cell::new(0));
    let dropped = Rc::new(Cell::new(0));
    let target: Widget = ExternalDropTarget::new(Widget::box_(Size::new(80.0, 80.0), Color::BLACK))
        .on_cancel({
            let cancelled = cancelled.clone();
            move |_| cancelled.set(cancelled.get() + 1)
        })
        .on_drop({
            let dropped = dropped.clone();
            move |_, _| dropped.set(dropped.get() + 1)
        })
        .into();
    let mut tree = WidgetTree::new();
    let root = tree.mount(Column::new([target]).into()).unwrap();
    tree.layout(Constraints::tight(Size::new(80.0, 80.0)))
        .unwrap();
    let _ = tree.dispatch_external_drag(file_event(
        ExternalDragPhase::Enter,
        Offset::new(10.0, 10.0),
    ));

    tree.update(root, Column::new(Vec::<Widget>::new()).into())
        .unwrap();
    assert_eq!(cancelled.get(), 1);
    tree.verify_invariants().expect("no stale external target");
    let _ =
        tree.dispatch_external_drag(file_event(ExternalDragPhase::Drop, Offset::new(10.0, 10.0)));
    assert_eq!(dropped.get(), 0);
}

#[test]
fn external_transfer_never_enters_the_local_typed_drag_protocol() {
    let context = DragDropContext::<String>::new();
    let local_entered = Rc::new(Cell::new(0));
    let local_dropped = Rc::new(Cell::new(0));
    let external_dropped = Rc::new(Cell::new(0));
    let local_target: Widget = DragTarget::new(
        context.clone(),
        Widget::box_(Size::new(80.0, 80.0), Color::BLACK),
    )
    .on_enter({
        let local_entered = local_entered.clone();
        move |_| local_entered.set(local_entered.get() + 1)
    })
    .on_drop({
        let local_dropped = local_dropped.clone();
        move |_| local_dropped.set(local_dropped.get() + 1)
    })
    .into();
    let mut tree = WidgetTree::new();
    tree.mount(
        ExternalDropTarget::new(local_target)
            .on_drop({
                let external_dropped = external_dropped.clone();
                move |_, _| external_dropped.set(external_dropped.get() + 1)
            })
            .into(),
    )
    .unwrap();
    tree.layout(Constraints::tight(Size::new(80.0, 80.0)))
        .unwrap();

    let _ = tree.dispatch_external_drag(file_event(
        ExternalDragPhase::Enter,
        Offset::new(10.0, 10.0),
    ));
    let _ =
        tree.dispatch_external_drag(file_event(ExternalDragPhase::Drop, Offset::new(10.0, 10.0)));
    assert_eq!(external_dropped.get(), 1);
    assert_eq!(local_entered.get(), 0);
    assert_eq!(local_dropped.get(), 0);
    assert!(!context.is_dragging());
}
