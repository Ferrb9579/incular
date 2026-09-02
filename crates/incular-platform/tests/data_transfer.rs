use incular_platform::{
    Clipboard, DataTransfer, ExternalDragEvent, ExternalDragPhase, MediaType, MemoryClipboard,
    TransferData, TransferDataError, TransferFormat, TransferImage, TransferItem,
    TransferOperation, TransferOperations, TransferReadRequest, TransferRepresentation,
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

#[test]
fn clipboard_roundtrips_multiple_representations_of_one_item() {
    let item = TransferItem::new([
        TransferRepresentation::plain_text("hello"),
        TransferRepresentation::html("<strong>hello</strong>"),
    ])
    .expect("unique transfer representations");
    let mut clipboard = MemoryClipboard::default();
    let report = clipboard
        .write(DataTransfer::single(item))
        .expect("memory clipboard write");
    assert_eq!(
        report.written,
        vec![TransferFormat::PlainText, TransferFormat::Html]
    );

    let transfer = clipboard
        .read(TransferReadRequest::all())
        .expect("memory clipboard read");
    assert_eq!(
        transfer.materialize(&TransferFormat::PlainText).unwrap(),
        Some(TransferData::text("hello"))
    );
    assert_eq!(
        transfer.materialize(&TransferFormat::Html).unwrap(),
        Some(TransferData::text("<strong>hello</strong>"))
    );
}

#[test]
fn text_convenience_is_a_view_of_the_rich_clipboard_model() {
    let mut clipboard = MemoryClipboard::default();
    clipboard.set_text("plain".to_owned());
    assert_eq!(clipboard.get_text().as_deref(), Some("plain"));
    let transfer = clipboard
        .read(TransferReadRequest::all())
        .expect("stored transfer");
    assert_eq!(transfer.formats(), vec![TransferFormat::PlainText]);
}

#[test]
fn file_transfer_preserves_multiple_file_order() {
    let expected = vec![
        PathBuf::from("C:/first.txt"),
        PathBuf::from("C:/second.txt"),
        PathBuf::from("C:/third.txt"),
    ];
    let transfer = DataTransfer::files(expected.clone());
    let Some(TransferData::Files(paths)) = transfer.materialize(&TransferFormat::Files).unwrap()
    else {
        panic!("file representation must materialize as files");
    };
    assert_eq!(paths.as_ref(), expected.as_slice());
}

#[test]
fn image_and_custom_bytes_roundtrip_without_copying_backing_storage() {
    let rgba: Arc<[u8]> = vec![1, 2, 3, 4, 5, 6, 7, 8].into();
    let image = TransferImage::new(2, 1, rgba.clone()).expect("valid RGBA8 image");
    assert!(Arc::ptr_eq(&rgba, &image.shared_rgba()));

    let custom_bytes: Arc<[u8]> = vec![9, 8, 7, 6].into();
    let custom = MediaType::new("application/x-incular-test").unwrap();
    let item = TransferItem::new([
        TransferRepresentation::rgba8_image(image.clone()),
        TransferRepresentation::custom(custom.clone(), custom_bytes.clone()),
    ])
    .unwrap();
    let transfer = DataTransfer::single(item);
    assert_eq!(
        transfer.materialize(&TransferFormat::Rgba8Image).unwrap(),
        Some(TransferData::Rgba8Image(image))
    );
    let Some(TransferData::Bytes(bytes)) = transfer
        .materialize(&TransferFormat::Custom(custom))
        .unwrap()
    else {
        panic!("custom representation must materialize as bytes");
    };
    assert!(Arc::ptr_eq(&bytes, &custom_bytes));
}

#[test]
fn lazy_payload_is_not_eagerly_materialized_and_is_cached_once() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_provider = calls.clone();
    let custom = MediaType::new("com.example.large-payload").unwrap();
    let custom_format = TransferFormat::Custom(custom.clone());
    let item = TransferItem::new([
        TransferRepresentation::plain_text("preview"),
        TransferRepresentation::lazy(custom_format.clone(), move || {
            calls_for_provider.fetch_add(1, Ordering::SeqCst);
            Ok(TransferData::bytes(vec![42; 1024]))
        }),
    ])
    .unwrap();
    let mut clipboard = MemoryClipboard::default();
    clipboard
        .write(DataTransfer::single(item))
        .expect("memory write retains providers");
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    assert_eq!(clipboard.get_text().as_deref(), Some("preview"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let custom_only = clipboard
        .read(TransferReadRequest::format(custom_format.clone()))
        .expect("custom representation remains available");
    assert!(matches!(
        custom_only.materialize(&custom_format),
        Ok(Some(TransferData::Bytes(_)))
    ));
    assert!(matches!(
        custom_only.materialize(&custom_format),
        Ok(Some(TransferData::Bytes(_)))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn lazy_provider_type_mismatch_is_rejected_at_materialization() {
    let representation = TransferRepresentation::lazy(TransferFormat::Files, || {
        Ok(TransferData::text("not files"))
    });
    assert!(matches!(
        representation.materialize(),
        Err(TransferDataError::TypeMismatch {
            format: TransferFormat::Files,
            data: "text"
        })
    ));
}

#[test]
fn winit_style_file_drag_truthfully_allows_copy_only() {
    let event = ExternalDragEvent::files(
        ExternalDragPhase::Enter,
        [PathBuf::from("report.pdf")],
        incular_core::Offset::new(12.0, 8.0),
    );
    assert!(event.allowed_operations.contains(TransferOperation::Copy));
    assert!(!event.allowed_operations.contains(TransferOperation::Move));
    assert!(!event.allowed_operations.contains(TransferOperation::Link));
    assert_eq!(event.allowed_operations, TransferOperations::COPY);
}
