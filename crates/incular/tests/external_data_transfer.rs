#[cfg(feature = "desktop")]
use incular::prelude::*;

#[cfg(feature = "desktop")]
#[test]
fn desktop_prelude_exposes_external_file_transfer_api() {
    let target: Widget = ExternalDropTarget::new(SizedBox::shrink())
        .on_will_accept(|event| {
            event
                .transfer
                .representation(&TransferFormat::Files)
                .is_some()
                .then_some(TransferOperation::Copy)
        })
        .on_drop(|event, operation| {
            assert_eq!(operation, TransferOperation::Copy);
            assert!(
                event
                    .transfer
                    .representation(&TransferFormat::Files)
                    .is_some()
            );
        })
        .into();

    let transfer = DataTransfer::files(["document.txt"]);
    assert!(transfer.representation(&TransferFormat::Files).is_some());
    drop(target);
}
