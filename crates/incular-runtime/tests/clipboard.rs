use incular_platform::{
    CapabilitySupport, DataTransfer, TransferData, TransferFormat, TransferItem,
    TransferReadRequest, TransferRepresentation,
};
use incular_runtime::Runtime;
use incular_widgets::SizedBox;

#[test]
fn runtime_exposes_rich_clipboard_without_text_only_side_channel() {
    let mut runtime = Runtime::new(SizedBox::shrink().into()).expect("runtime");
    let capabilities = runtime.clipboard_capabilities();
    assert_eq!(capabilities.read.plain_text, CapabilitySupport::Supported);
    assert_eq!(capabilities.read.html, CapabilitySupport::Supported);

    let transfer = DataTransfer::single(
        TransferItem::new([
            TransferRepresentation::plain_text("plain"),
            TransferRepresentation::html("<b>plain</b>"),
        ])
        .unwrap(),
    );
    let report = runtime.write_clipboard(transfer).expect("memory write");
    assert_eq!(
        report.written,
        vec![TransferFormat::PlainText, TransferFormat::Html]
    );

    let html = runtime
        .read_clipboard(TransferReadRequest::format(TransferFormat::Html))
        .expect("HTML read");
    assert_eq!(
        html.materialize(&TransferFormat::Html).unwrap(),
        Some(TransferData::text("<b>plain</b>"))
    );

    runtime.set_clipboard_text("replacement");
    let plain = runtime
        .read_clipboard(TransferReadRequest::all())
        .expect("plain-text convenience writes rich model");
    assert_eq!(plain.formats(), vec![TransferFormat::PlainText]);
}
