use incular_platform::{
    CapabilitySupport, Clipboard, ClipboardCapabilities, ClipboardError, ClipboardWriteReport,
    DataTransfer, TransferData, TransferFormat, TransferFormatCapabilities, TransferImage,
    TransferItem, TransferReadRequest, TransferRepresentation,
};
use std::{borrow::Cow, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeClipboardWriteKind {
    Html { include_plain_text: bool },
    Image,
    Files,
    PlainText,
}

fn plan_native_clipboard_write(
    transfer: &DataTransfer,
) -> Result<(NativeClipboardWriteKind, ClipboardWriteReport), ClipboardError> {
    if transfer.items().len() != 1 {
        return Err(ClipboardError::UnsupportedCombination);
    }
    let formats = transfer.formats();
    let has = |format: &TransferFormat| formats.contains(format);
    let (kind, written) = if has(&TransferFormat::Html) {
        let include_plain_text = has(&TransferFormat::PlainText);
        let mut written = vec![TransferFormat::Html];
        if include_plain_text {
            written.push(TransferFormat::PlainText);
        }
        (
            NativeClipboardWriteKind::Html { include_plain_text },
            written,
        )
    } else if has(&TransferFormat::Rgba8Image) {
        (
            NativeClipboardWriteKind::Image,
            vec![TransferFormat::Rgba8Image],
        )
    } else if has(&TransferFormat::Files) {
        (NativeClipboardWriteKind::Files, vec![TransferFormat::Files])
    } else if has(&TransferFormat::PlainText) {
        (
            NativeClipboardWriteKind::PlainText,
            vec![TransferFormat::PlainText],
        )
    } else {
        return Err(formats.first().cloned().map_or(
            ClipboardError::UnsupportedCombination,
            ClipboardError::UnsupportedFormat,
        ));
    };
    let skipped = formats
        .into_iter()
        .filter(|format| !written.contains(format))
        .collect();
    Ok((kind, ClipboardWriteReport { written, skipped }))
}

/// Returns the exact formats the desktop clipboard can commit atomically for
/// this transfer. This is exposed only for deterministic integration tests and
/// embedders that want to preview loss before issuing a write.
#[doc(hidden)]
pub fn desktop_clipboard_write_report(
    transfer: &DataTransfer,
) -> Result<ClipboardWriteReport, ClipboardError> {
    plan_native_clipboard_write(transfer).map(|(_, report)| report)
}

/// Native desktop clipboard. Initialization failures remain observable on every
/// read/write; process-local storage requires explicitly selecting MemoryClipboard.
pub(crate) struct DesktopClipboard {
    native: Result<arboard::Clipboard, ClipboardError>,
}

impl DesktopClipboard {
    pub(crate) fn new() -> Self {
        Self {
            native: arboard::Clipboard::new()
                .map_err(|error| ClipboardError::Backend(error.to_string())),
        }
    }

    pub(crate) fn native_capabilities(&self) -> ClipboardCapabilities {
        if self.native.is_err() {
            return ClipboardCapabilities {
                read: TransferFormatCapabilities::all(CapabilitySupport::Unsupported),
                write: TransferFormatCapabilities::all(CapabilitySupport::Unsupported),
                lazy_write: CapabilitySupport::Unsupported,
            };
        }
        let standard = TransferFormatCapabilities {
            plain_text: CapabilitySupport::Supported,
            html: CapabilitySupport::Supported,
            uri_list: CapabilitySupport::Unsupported,
            files: CapabilitySupport::Supported,
            rgba8_image: CapabilitySupport::Supported,
            custom: CapabilitySupport::Unsupported,
        };
        ClipboardCapabilities {
            read: standard,
            write: standard,
            lazy_write: CapabilitySupport::Unsupported,
        }
    }

    fn read_native(
        clipboard: &mut arboard::Clipboard,
        request: &TransferReadRequest,
    ) -> Result<DataTransfer, ClipboardError> {
        let supported_requested = [
            TransferFormat::PlainText,
            TransferFormat::Html,
            TransferFormat::Files,
            TransferFormat::Rgba8Image,
        ]
        .into_iter()
        .filter(|format| request.includes(format))
        .collect::<Vec<_>>();
        if supported_requested.is_empty() && !request.requested_formats().is_empty() {
            return Err(ClipboardError::UnsupportedFormat(
                request.requested_formats()[0].clone(),
            ));
        }

        let mut representations = Vec::new();
        let mut first_error = None;
        for format in supported_requested {
            let result = match format {
                TransferFormat::PlainText => clipboard
                    .get()
                    .text()
                    .map(TransferRepresentation::plain_text),
                TransferFormat::Html => clipboard.get().html().map(TransferRepresentation::html),
                TransferFormat::Files => clipboard
                    .get()
                    .file_list()
                    .map(TransferRepresentation::files),
                TransferFormat::Rgba8Image => clipboard.get().image().and_then(|image| {
                    let width = u32::try_from(image.width)
                        .map_err(|_| arboard::Error::ConversionFailure)?;
                    let height = u32::try_from(image.height)
                        .map_err(|_| arboard::Error::ConversionFailure)?;
                    let rgba: Arc<[u8]> = match image.bytes {
                        Cow::Borrowed(bytes) => Arc::from(bytes),
                        Cow::Owned(bytes) => Arc::from(bytes),
                    };
                    TransferImage::new(width, height, rgba)
                        .map(TransferRepresentation::rgba8_image)
                        .map_err(|_| arboard::Error::ConversionFailure)
                }),
                TransferFormat::UriList | TransferFormat::Custom(_) => unreachable!(),
            };
            match result {
                Ok(representation) => representations.push(representation),
                Err(arboard::Error::ContentNotAvailable) => {}
                Err(error) => {
                    first_error.get_or_insert_with(|| ClipboardError::Backend(error.to_string()));
                }
            }
        }

        if !representations.is_empty() {
            return TransferItem::new(representations)
                .map(DataTransfer::single)
                .map_err(ClipboardError::from);
        }
        Err(first_error.unwrap_or(ClipboardError::Unavailable))
    }

    fn write_native(
        clipboard: &mut arboard::Clipboard,
        transfer: &DataTransfer,
    ) -> Result<ClipboardWriteReport, ClipboardError> {
        let (plan, report) = plan_native_clipboard_write(transfer)?;
        match plan {
            NativeClipboardWriteKind::Html { include_plain_text } => {
                let Some(TransferData::Text(html)) = transfer.materialize(&TransferFormat::Html)?
                else {
                    unreachable!("HTML transfer format validates its payload")
                };
                let alt = if include_plain_text {
                    let Some(TransferData::Text(text)) =
                        transfer.materialize(&TransferFormat::PlainText)?
                    else {
                        unreachable!("plain-text transfer format validates its payload")
                    };
                    Some(text)
                } else {
                    None
                };
                clipboard
                    .set()
                    .html(html.as_ref(), alt.as_deref())
                    .map_err(|error| ClipboardError::Backend(error.to_string()))?;
            }
            NativeClipboardWriteKind::Image => {
                let Some(TransferData::Rgba8Image(image)) =
                    transfer.materialize(&TransferFormat::Rgba8Image)?
                else {
                    unreachable!("image transfer format validates its payload")
                };
                let width = usize::try_from(image.width()).map_err(|_| {
                    ClipboardError::Backend("image width does not fit usize".to_owned())
                })?;
                let height = usize::try_from(image.height()).map_err(|_| {
                    ClipboardError::Backend("image height does not fit usize".to_owned())
                })?;
                clipboard
                    .set()
                    .image(arboard::ImageData {
                        width,
                        height,
                        bytes: Cow::Borrowed(image.rgba()),
                    })
                    .map_err(|error| ClipboardError::Backend(error.to_string()))?;
            }
            NativeClipboardWriteKind::Files => {
                let Some(TransferData::Files(paths)) =
                    transfer.materialize(&TransferFormat::Files)?
                else {
                    unreachable!("file transfer format validates its payload")
                };
                clipboard
                    .set()
                    .file_list(paths.as_ref())
                    .map_err(|error| ClipboardError::Backend(error.to_string()))?;
            }
            NativeClipboardWriteKind::PlainText => {
                let Some(TransferData::Text(text)) =
                    transfer.materialize(&TransferFormat::PlainText)?
                else {
                    unreachable!("plain-text transfer format validates its payload")
                };
                clipboard
                    .set()
                    .text(text.as_ref())
                    .map_err(|error| ClipboardError::Backend(error.to_string()))?;
            }
        }
        Ok(report)
    }
}

impl Clipboard for DesktopClipboard {
    fn capabilities(&self) -> ClipboardCapabilities {
        self.native_capabilities()
    }

    fn read(&mut self, request: TransferReadRequest) -> Result<DataTransfer, ClipboardError> {
        let native = self.native.as_mut().map_err(|error| error.clone())?;
        Self::read_native(native, &request)
    }

    fn write(&mut self, transfer: DataTransfer) -> Result<ClipboardWriteReport, ClipboardError> {
        let native = self.native.as_mut().map_err(|error| error.clone())?;
        Self::write_native(native, &transfer)
    }
}
