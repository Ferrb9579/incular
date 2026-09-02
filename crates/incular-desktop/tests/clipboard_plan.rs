use incular_desktop::desktop_clipboard_write_report;
use incular_platform::{
    ClipboardError, DataTransfer, MediaType, TransferFormat, TransferItem, TransferRepresentation,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn html_and_plain_text_are_the_only_atomic_rich_text_pair() {
    let transfer = DataTransfer::single(
        TransferItem::new([
            TransferRepresentation::plain_text("plain"),
            TransferRepresentation::html("<b>plain</b>"),
            TransferRepresentation::custom(
                MediaType::new("application/x-extra").unwrap(),
                vec![1, 2, 3],
            ),
        ])
        .unwrap(),
    );
    let report = desktop_clipboard_write_report(&transfer).unwrap();
    assert_eq!(
        report.written,
        vec![TransferFormat::Html, TransferFormat::PlainText]
    );
    assert_eq!(
        report.skipped,
        vec![TransferFormat::Custom(
            MediaType::new("application/x-extra").unwrap()
        )]
    );
}

#[test]
fn native_write_planning_never_materializes_unchosen_lazy_data() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_provider = calls.clone();
    let transfer = DataTransfer::single(
        TransferItem::new([
            TransferRepresentation::files(["report.pdf"]),
            TransferRepresentation::lazy(TransferFormat::PlainText, move || {
                calls_for_provider.fetch_add(1, Ordering::SeqCst);
                Ok(incular_platform::TransferData::text("fallback"))
            }),
        ])
        .unwrap(),
    );
    let report = desktop_clipboard_write_report(&transfer).unwrap();
    assert_eq!(report.written, vec![TransferFormat::Files]);
    assert_eq!(report.skipped, vec![TransferFormat::PlainText]);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn unsupported_only_format_is_rejected_instead_of_converted() {
    let custom = MediaType::new("com.example.document").unwrap();
    let transfer = DataTransfer::single(TransferItem::single(TransferRepresentation::custom(
        custom.clone(),
        vec![7, 8, 9],
    )));
    assert!(matches!(
        desktop_clipboard_write_report(&transfer),
        Err(ClipboardError::UnsupportedFormat(TransferFormat::Custom(media_type)))
            if media_type == custom
    ));
}

#[test]
fn multiple_logical_items_are_rejected_atomically() {
    let transfer = DataTransfer::new([
        TransferItem::single(TransferRepresentation::plain_text("first")),
        TransferItem::single(TransferRepresentation::plain_text("second")),
    ]);
    assert_eq!(
        desktop_clipboard_write_report(&transfer),
        Err(ClipboardError::UnsupportedCombination)
    );
}
