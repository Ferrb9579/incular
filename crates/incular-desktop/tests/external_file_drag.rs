use incular_core::Offset;
use incular_desktop::ExternalFileDragState;
use incular_platform::{ExternalDragPhase, TransferData, TransferFormat, TransferOperation};
use std::path::PathBuf;

fn files(event: &incular_platform::ExternalDragEvent) -> Vec<PathBuf> {
    let Some(TransferData::Files(paths)) = event
        .transfer
        .materialize(&TransferFormat::Files)
        .expect("file payload materializes")
    else {
        panic!("external file drag must contain files");
    };
    paths.to_vec()
}

#[test]
fn winit_file_samples_become_one_ordered_multi_file_drop() {
    let mut state = ExternalFileDragState::default();
    let first = state.hover(PathBuf::from("first.txt"), Offset::new(4.0, 8.0));
    assert_eq!(first.phase, ExternalDragPhase::Enter);
    assert_eq!(files(&first), [PathBuf::from("first.txt")]);
    assert_eq!(
        first.allowed_operations.preferred(),
        Some(TransferOperation::Copy)
    );

    let second = state.hover(PathBuf::from("second.txt"), Offset::new(5.0, 9.0));
    assert_eq!(second.phase, ExternalDragPhase::Over);
    assert_eq!(
        files(&second),
        [PathBuf::from("first.txt"), PathBuf::from("second.txt")]
    );

    // Drop events may arrive in another order; the hover order is the source
    // order exposed to widgets, with genuinely new paths appended afterwards.
    state.queue_drop(PathBuf::from("second.txt"), Offset::new(6.0, 10.0));
    state.queue_drop(PathBuf::from("first.txt"), Offset::new(6.0, 10.0));
    state.queue_drop(PathBuf::from("third.txt"), Offset::new(6.0, 10.0));
    let drop = state.take_drop().expect("one normalized drop");
    assert_eq!(drop.phase, ExternalDragPhase::Drop);
    assert_eq!(
        files(&drop),
        [
            PathBuf::from("first.txt"),
            PathBuf::from("second.txt"),
            PathBuf::from("third.txt")
        ]
    );
    assert!(
        state.take_drop().is_none(),
        "drop is finalized exactly once"
    );
    assert!(!state.is_active());
}

#[test]
fn cancellation_does_not_erase_an_already_queued_drop() {
    let mut state = ExternalFileDragState::default();
    let _ = state.hover(PathBuf::from("one.txt"), Offset::ZERO);
    state.queue_drop(PathBuf::from("one.txt"), Offset::new(1.0, 1.0));
    assert!(state.cancel(Offset::new(2.0, 2.0)).is_none());
    assert!(state.take_drop().is_some());
}

#[test]
fn hover_cancel_preserves_the_current_file_payload() {
    let mut state = ExternalFileDragState::default();
    let _ = state.hover(PathBuf::from("one.txt"), Offset::ZERO);
    let _ = state.hover(PathBuf::from("two.txt"), Offset::ZERO);
    let cancel = state.cancel(Offset::new(3.0, 4.0)).expect("cancel event");
    assert_eq!(cancel.phase, ExternalDragPhase::Cancel);
    assert_eq!(
        files(&cancel),
        [PathBuf::from("one.txt"), PathBuf::from("two.txt")]
    );
    assert!(!state.is_active());
}
