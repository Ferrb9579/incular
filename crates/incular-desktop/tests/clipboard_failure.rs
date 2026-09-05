// Compile the private adapter without adding test-only hooks to the library.
#[allow(dead_code)]
mod adapter {
    include!("../src/clipboard.rs");

    #[test]
    fn unavailable_native_clipboard_never_reports_a_local_write_success() {
        for error in [
            ClipboardError::Unavailable,
            ClipboardError::Backend("cannot open display".into()),
        ] {
            let mut clipboard = DesktopClipboard {
                native: Err(error.clone()),
            };
            assert_eq!(
                clipboard.capabilities().write.plain_text,
                CapabilitySupport::Unsupported
            );
            for _ in 0..2 {
                let transfer = DataTransfer::single(
                    TransferItem::new([TransferRepresentation::plain_text("not copied")]).unwrap(),
                );
                assert_eq!(clipboard.write(transfer), Err(error.clone()));
                assert_eq!(
                    clipboard.read(TransferReadRequest::all()),
                    Err(error.clone())
                );
            }
        }
    }
}

#[test]
fn explicitly_selected_memory_clipboard_remains_available() {
    use incular_platform::{
        Clipboard, DataTransfer, MemoryClipboard, TransferItem, TransferReadRequest,
        TransferRepresentation,
    };
    let transfer = DataTransfer::single(
        TransferItem::new([TransferRepresentation::plain_text("local only")]).unwrap(),
    );
    let mut clipboard = MemoryClipboard::default();
    assert!(clipboard.write(transfer).is_ok());
    assert!(clipboard.read(TransferReadRequest::all()).is_ok());
}
