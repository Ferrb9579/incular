//! Shared clipboard and external data-transfer model.
//!
//! A transfer item may advertise several representations of the same logical
//! value (for example plain text and HTML). Payloads are reference counted and
//! lazy providers are materialized at most once, so large data is never cloned
//! merely because a transfer crosses the platform/runtime/widget boundaries.

use crate::CapabilitySupport;
use incular_core::Offset;
use std::{
    collections::HashSet,
    fmt,
    path::PathBuf,
    sync::{Arc, OnceLock},
};

/// Stable identifier for an application-defined transfer representation.
///
/// MIME types (`application/json`) and UTI-like identifiers
/// (`com.example.document`) are both accepted. Incular deliberately does not
/// reinterpret or normalize application-defined identifiers.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MediaType(String);

impl MediaType {
    pub fn new(value: impl Into<String>) -> Result<Self, TransferDataError> {
        let value = value.into();
        if value.is_empty()
            || value.trim() != value
            || value.chars().any(|character| {
                character.is_control() || character.is_whitespace() || character == '\0'
            })
        {
            return Err(TransferDataError::InvalidMediaType(value));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MediaType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Portable representation kinds shared by clipboard and external drag/drop.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TransferFormat {
    PlainText,
    Html,
    UriList,
    Files,
    Rgba8Image,
    Custom(MediaType),
}

impl TransferFormat {
    #[must_use]
    pub fn media_type(&self) -> &str {
        match self {
            Self::PlainText => "text/plain;charset=utf-8",
            Self::Html => "text/html",
            Self::UriList => "text/uri-list",
            Self::Files => "application/x-incular-file-list",
            Self::Rgba8Image => "application/x-incular-rgba8",
            Self::Custom(media_type) => media_type.as_str(),
        }
    }
}

/// Owned, unpremultiplied RGBA8 image transfer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferImage {
    width: u32,
    height: u32,
    rgba: Arc<[u8]>,
}

impl TransferImage {
    pub fn new(
        width: u32,
        height: u32,
        rgba: impl Into<Arc<[u8]>>,
    ) -> Result<Self, TransferDataError> {
        let rgba = rgba.into();
        let expected = u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|pixels| pixels.checked_mul(4))
            .and_then(|bytes| usize::try_from(bytes).ok())
            .ok_or(TransferDataError::InvalidImageDimensions { width, height })?;
        if rgba.len() != expected {
            return Err(TransferDataError::InvalidImageLength {
                width,
                height,
                actual: rgba.len(),
                expected,
            });
        }
        Ok(Self {
            width,
            height,
            rgba,
        })
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    #[must_use]
    pub fn shared_rgba(&self) -> Arc<[u8]> {
        self.rgba.clone()
    }
}

/// Materialized transfer data. Large payload variants retain shared storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransferData {
    Text(Arc<str>),
    UriList(Arc<[String]>),
    Files(Arc<[PathBuf]>),
    Rgba8Image(TransferImage),
    Bytes(Arc<[u8]>),
}

impl TransferData {
    #[must_use]
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(Arc::<str>::from(value.into()))
    }

    #[must_use]
    pub fn uri_list(values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::UriList(
            values
                .into_iter()
                .map(Into::into)
                .collect::<Vec<_>>()
                .into(),
        )
    }

    #[must_use]
    pub fn files(values: impl IntoIterator<Item = impl Into<PathBuf>>) -> Self {
        Self::Files(
            values
                .into_iter()
                .map(Into::into)
                .collect::<Vec<_>>()
                .into(),
        )
    }

    #[must_use]
    pub fn bytes(value: impl Into<Arc<[u8]>>) -> Self {
        Self::Bytes(value.into())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransferDataError {
    InvalidMediaType(String),
    InvalidImageDimensions {
        width: u32,
        height: u32,
    },
    InvalidImageLength {
        width: u32,
        height: u32,
        actual: usize,
        expected: usize,
    },
    DuplicateFormat(TransferFormat),
    TypeMismatch {
        format: TransferFormat,
        data: &'static str,
    },
    ProviderFailed(String),
}

impl fmt::Display for TransferDataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMediaType(value) => {
                write!(formatter, "invalid transfer media type {value:?}")
            }
            Self::InvalidImageDimensions { width, height } => {
                write!(
                    formatter,
                    "RGBA8 image dimensions overflow: {width}x{height}"
                )
            }
            Self::InvalidImageLength {
                width,
                height,
                actual,
                expected,
            } => write!(
                formatter,
                "RGBA8 image {width}x{height} requires {expected} bytes, got {actual}"
            ),
            Self::DuplicateFormat(format) => {
                write!(formatter, "duplicate transfer representation {format:?}")
            }
            Self::TypeMismatch { format, data } => write!(
                formatter,
                "transfer representation {format:?} cannot materialize {data} data"
            ),
            Self::ProviderFailed(message) => {
                write!(formatter, "transfer provider failed: {message}")
            }
        }
    }
}

impl std::error::Error for TransferDataError {}

struct LazyTransferData {
    provider: Arc<dyn Fn() -> Result<TransferData, TransferDataError> + Send + Sync>,
    value: OnceLock<Result<TransferData, TransferDataError>>,
}

impl LazyTransferData {
    fn materialize(&self) -> Result<TransferData, TransferDataError> {
        self.value.get_or_init(|| (self.provider)()).clone()
    }
}

#[derive(Clone)]
enum TransferPayload {
    Immediate(TransferData),
    Lazy(Arc<LazyTransferData>),
}

impl TransferPayload {
    fn materialize(&self) -> Result<TransferData, TransferDataError> {
        match self {
            Self::Immediate(data) => Ok(data.clone()),
            Self::Lazy(data) => data.materialize(),
        }
    }
}

impl fmt::Debug for TransferPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Immediate(data) => formatter.debug_tuple("Immediate").field(data).finish(),
            Self::Lazy(_) => formatter.write_str("Lazy(<provider>)"),
        }
    }
}

impl PartialEq for TransferPayload {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Immediate(left), Self::Immediate(right)) => left == right,
            (Self::Lazy(left), Self::Lazy(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }
}

impl Eq for TransferPayload {}

/// One advertised representation of a logical transfer item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferRepresentation {
    format: TransferFormat,
    payload: TransferPayload,
}

impl TransferRepresentation {
    fn immediate(format: TransferFormat, data: TransferData) -> Self {
        debug_assert!(data_matches_format(&format, &data));
        Self {
            format,
            payload: TransferPayload::Immediate(data),
        }
    }

    #[must_use]
    pub fn plain_text(value: impl Into<String>) -> Self {
        Self::immediate(TransferFormat::PlainText, TransferData::text(value))
    }

    #[must_use]
    pub fn html(value: impl Into<String>) -> Self {
        Self::immediate(TransferFormat::Html, TransferData::text(value))
    }

    #[must_use]
    pub fn uri_list(values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::immediate(TransferFormat::UriList, TransferData::uri_list(values))
    }

    #[must_use]
    pub fn files(values: impl IntoIterator<Item = impl Into<PathBuf>>) -> Self {
        Self::immediate(TransferFormat::Files, TransferData::files(values))
    }

    #[must_use]
    pub fn rgba8_image(image: TransferImage) -> Self {
        Self::immediate(TransferFormat::Rgba8Image, TransferData::Rgba8Image(image))
    }

    #[must_use]
    pub fn custom(media_type: MediaType, bytes: impl Into<Arc<[u8]>>) -> Self {
        Self::immediate(
            TransferFormat::Custom(media_type),
            TransferData::bytes(bytes),
        )
    }

    /// Creates a delayed representation. The provider is invoked on first
    /// materialization and its result (including failure) is cached and shared
    /// by every clone of this representation.
    #[must_use]
    pub fn lazy(
        format: TransferFormat,
        provider: impl Fn() -> Result<TransferData, TransferDataError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            format,
            payload: TransferPayload::Lazy(Arc::new(LazyTransferData {
                provider: Arc::new(provider),
                value: OnceLock::new(),
            })),
        }
    }

    #[must_use]
    pub const fn format(&self) -> &TransferFormat {
        &self.format
    }

    pub fn materialize(&self) -> Result<TransferData, TransferDataError> {
        let data = self.payload.materialize()?;
        if data_matches_format(&self.format, &data) {
            Ok(data)
        } else {
            Err(TransferDataError::TypeMismatch {
                format: self.format.clone(),
                data: transfer_data_name(&data),
            })
        }
    }
}

fn data_matches_format(format: &TransferFormat, data: &TransferData) -> bool {
    matches!(
        (format, data),
        (
            TransferFormat::PlainText | TransferFormat::Html,
            TransferData::Text(_)
        ) | (TransferFormat::UriList, TransferData::UriList(_))
            | (TransferFormat::Files, TransferData::Files(_))
            | (TransferFormat::Rgba8Image, TransferData::Rgba8Image(_))
            | (TransferFormat::Custom(_), TransferData::Bytes(_))
    )
}

fn transfer_data_name(data: &TransferData) -> &'static str {
    match data {
        TransferData::Text(_) => "text",
        TransferData::UriList(_) => "URI list",
        TransferData::Files(_) => "file list",
        TransferData::Rgba8Image(_) => "RGBA8 image",
        TransferData::Bytes(_) => "bytes",
    }
}

/// Multiple representations of one logical clipboard/drag item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferItem {
    representations: Arc<[TransferRepresentation]>,
}

impl TransferItem {
    pub fn new(
        representations: impl IntoIterator<Item = TransferRepresentation>,
    ) -> Result<Self, TransferDataError> {
        let representations = representations.into_iter().collect::<Vec<_>>();
        let mut formats = HashSet::with_capacity(representations.len());
        for representation in &representations {
            if !formats.insert(representation.format.clone()) {
                return Err(TransferDataError::DuplicateFormat(
                    representation.format.clone(),
                ));
            }
        }
        Ok(Self {
            representations: representations.into(),
        })
    }

    #[must_use]
    pub fn single(representation: TransferRepresentation) -> Self {
        Self {
            representations: vec![representation].into(),
        }
    }

    #[must_use]
    pub fn representations(&self) -> &[TransferRepresentation] {
        &self.representations
    }

    #[must_use]
    pub fn representation(&self, format: &TransferFormat) -> Option<&TransferRepresentation> {
        self.representations
            .iter()
            .find(|representation| representation.format() == format)
    }

    fn filtered(&self, request: &TransferReadRequest) -> Option<Self> {
        let representations = self
            .representations
            .iter()
            .filter(|representation| request.includes(representation.format()))
            .cloned()
            .collect::<Vec<_>>();
        (!representations.is_empty()).then(|| Self {
            representations: representations.into(),
        })
    }
}

/// Ordered collection of clipboard or external drag items.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DataTransfer {
    items: Arc<[TransferItem]>,
}

impl DataTransfer {
    #[must_use]
    pub fn new(items: impl IntoIterator<Item = TransferItem>) -> Self {
        Self {
            items: items.into_iter().collect::<Vec<_>>().into(),
        }
    }

    #[must_use]
    pub fn single(item: TransferItem) -> Self {
        Self {
            items: vec![item].into(),
        }
    }

    #[must_use]
    pub fn plain_text(value: impl Into<String>) -> Self {
        Self::single(TransferItem::single(TransferRepresentation::plain_text(
            value,
        )))
    }

    #[must_use]
    pub fn files(paths: impl IntoIterator<Item = impl Into<PathBuf>>) -> Self {
        Self::single(TransferItem::single(TransferRepresentation::files(paths)))
    }

    #[must_use]
    pub fn items(&self) -> &[TransferItem] {
        &self.items
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    #[must_use]
    pub fn representation(&self, format: &TransferFormat) -> Option<&TransferRepresentation> {
        self.items
            .iter()
            .find_map(|item| item.representation(format))
    }

    pub fn materialize(
        &self,
        format: &TransferFormat,
    ) -> Result<Option<TransferData>, TransferDataError> {
        self.representation(format)
            .map(TransferRepresentation::materialize)
            .transpose()
    }

    pub fn plain_text_value(&self) -> Result<Option<Arc<str>>, TransferDataError> {
        match self.materialize(&TransferFormat::PlainText)? {
            Some(TransferData::Text(text)) => Ok(Some(text)),
            Some(data) => Err(TransferDataError::TypeMismatch {
                format: TransferFormat::PlainText,
                data: transfer_data_name(&data),
            }),
            None => Ok(None),
        }
    }

    #[must_use]
    pub fn formats(&self) -> Vec<TransferFormat> {
        let mut formats = Vec::new();
        for item in self.items.iter() {
            for representation in item.representations.iter() {
                if !formats.contains(&representation.format) {
                    formats.push(representation.format.clone());
                }
            }
        }
        formats
    }

    #[must_use]
    pub fn filtered(&self, request: &TransferReadRequest) -> Self {
        Self::new(self.items.iter().filter_map(|item| item.filtered(request)))
    }
}

/// Requested clipboard representations. Empty means every representation the
/// backend can enumerate without inventing conversions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TransferReadRequest {
    formats: Arc<[TransferFormat]>,
}

impl TransferReadRequest {
    #[must_use]
    pub fn all() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn format(format: TransferFormat) -> Self {
        Self {
            formats: vec![format].into(),
        }
    }

    #[must_use]
    pub fn formats(formats: impl IntoIterator<Item = TransferFormat>) -> Self {
        let mut unique = Vec::new();
        for format in formats {
            if !unique.contains(&format) {
                unique.push(format);
            }
        }
        Self {
            formats: unique.into(),
        }
    }

    #[must_use]
    pub fn requested_formats(&self) -> &[TransferFormat] {
        &self.formats
    }

    #[must_use]
    pub fn includes(&self, format: &TransferFormat) -> bool {
        self.formats.is_empty() || self.formats.iter().any(|requested| requested == format)
    }
}

/// Per-format support for one direction of clipboard access.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TransferFormatCapabilities {
    pub plain_text: CapabilitySupport,
    pub html: CapabilitySupport,
    pub uri_list: CapabilitySupport,
    pub files: CapabilitySupport,
    pub rgba8_image: CapabilitySupport,
    pub custom: CapabilitySupport,
}

impl TransferFormatCapabilities {
    #[must_use]
    pub const fn support(self, format: &TransferFormat) -> CapabilitySupport {
        match format {
            TransferFormat::PlainText => self.plain_text,
            TransferFormat::Html => self.html,
            TransferFormat::UriList => self.uri_list,
            TransferFormat::Files => self.files,
            TransferFormat::Rgba8Image => self.rgba8_image,
            TransferFormat::Custom(_) => self.custom,
        }
    }

    #[must_use]
    pub const fn all(support: CapabilitySupport) -> Self {
        Self {
            plain_text: support,
            html: support,
            uri_list: support,
            files: support,
            rgba8_image: support,
            custom: support,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ClipboardCapabilities {
    pub read: TransferFormatCapabilities,
    pub write: TransferFormatCapabilities,
    /// Whether the backend can retain a provider without materializing its data
    /// during the `write` call.
    pub lazy_write: CapabilitySupport,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardWriteReport {
    pub written: Vec<TransferFormat>,
    pub skipped: Vec<TransferFormat>,
}

impl ClipboardWriteReport {
    #[must_use]
    pub fn all_written(formats: Vec<TransferFormat>) -> Self {
        Self {
            written: formats,
            skipped: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClipboardError {
    Unavailable,
    UnsupportedFormat(TransferFormat),
    UnsupportedCombination,
    Transfer(TransferDataError),
    Backend(String),
}

impl fmt::Display for ClipboardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("clipboard content is unavailable"),
            Self::UnsupportedFormat(format) => {
                write!(formatter, "clipboard format {format:?} is unsupported")
            }
            Self::UnsupportedCombination => {
                formatter.write_str("clipboard cannot represent this combination atomically")
            }
            Self::Transfer(error) => error.fmt(formatter),
            Self::Backend(message) => write!(formatter, "clipboard backend failed: {message}"),
        }
    }
}

impl std::error::Error for ClipboardError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transfer(error) => Some(error),
            _ => None,
        }
    }
}

impl From<TransferDataError> for ClipboardError {
    fn from(value: TransferDataError) -> Self {
        Self::Transfer(value)
    }
}

/// Clipboard boundary shared by memory, desktop, and custom hosts.
pub trait Clipboard {
    fn capabilities(&self) -> ClipboardCapabilities;

    fn read(&mut self, request: TransferReadRequest) -> Result<DataTransfer, ClipboardError>;

    fn write(&mut self, transfer: DataTransfer) -> Result<ClipboardWriteReport, ClipboardError>;

    /// Plain-text convenience layered over [`Self::read`].
    fn get_text(&mut self) -> Option<String> {
        let transfer = self
            .read(TransferReadRequest::format(TransferFormat::PlainText))
            .ok()?;
        transfer
            .plain_text_value()
            .ok()?
            .map(|text| text.to_string())
    }

    /// Plain-text convenience layered over [`Self::write`].
    fn set_text(&mut self, text: String) {
        let _ = self.write(DataTransfer::plain_text(text));
    }
}

#[derive(Default)]
pub struct MemoryClipboard {
    transfer: Option<DataTransfer>,
}

impl Clipboard for MemoryClipboard {
    fn capabilities(&self) -> ClipboardCapabilities {
        ClipboardCapabilities {
            read: TransferFormatCapabilities::all(CapabilitySupport::Supported),
            write: TransferFormatCapabilities::all(CapabilitySupport::Supported),
            lazy_write: CapabilitySupport::Supported,
        }
    }

    fn read(&mut self, request: TransferReadRequest) -> Result<DataTransfer, ClipboardError> {
        let transfer = self.transfer.as_ref().ok_or(ClipboardError::Unavailable)?;
        let filtered = transfer.filtered(&request);
        if filtered.is_empty() {
            Err(ClipboardError::Unavailable)
        } else {
            Ok(filtered)
        }
    }

    fn write(&mut self, transfer: DataTransfer) -> Result<ClipboardWriteReport, ClipboardError> {
        let formats = transfer.formats();
        self.transfer = Some(transfer);
        Ok(ClipboardWriteReport::all_written(formats))
    }
}

/// One operation an external drag source permits a target to request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransferOperation {
    Copy,
    Move,
    Link,
}

/// Compact operation set used for drag/drop negotiation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TransferOperations(u8);

impl TransferOperations {
    pub const COPY: Self = Self(1 << 0);
    pub const MOVE: Self = Self(1 << 1);
    pub const LINK: Self = Self(1 << 2);

    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn contains(self, operation: TransferOperation) -> bool {
        let bit = match operation {
            TransferOperation::Copy => Self::COPY.0,
            TransferOperation::Move => Self::MOVE.0,
            TransferOperation::Link => Self::LINK.0,
        };
        self.0 & bit != 0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn preferred(self) -> Option<TransferOperation> {
        if self.contains(TransferOperation::Copy) {
            Some(TransferOperation::Copy)
        } else if self.contains(TransferOperation::Move) {
            Some(TransferOperation::Move)
        } else if self.contains(TransferOperation::Link) {
            Some(TransferOperation::Link)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExternalDragPhase {
    Enter,
    Over,
    Leave,
    Cancel,
    Drop,
}

/// Normalized incoming drag sample. `transfer` is the same shared transfer
/// model used by the clipboard; backends never reinterpret a file as text or
/// fabricate unsupported rich representations.
#[derive(Clone, Debug, PartialEq)]
pub struct ExternalDragEvent {
    pub phase: ExternalDragPhase,
    pub transfer: DataTransfer,
    pub position: Offset,
    pub allowed_operations: TransferOperations,
}

impl ExternalDragEvent {
    #[must_use]
    pub fn files(
        phase: ExternalDragPhase,
        paths: impl IntoIterator<Item = impl Into<PathBuf>>,
        position: Offset,
    ) -> Self {
        Self {
            phase,
            transfer: DataTransfer::files(paths),
            position,
            // Winit's file-drop abstraction has no protocol for promising a
            // source-side move/delete. Copy is therefore the only truthful
            // operation until a richer native adapter says otherwise.
            allowed_operations: TransferOperations::COPY,
        }
    }
}

/// Target negotiation result returned to a native adapter when it can honor a
/// requested drag operation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExternalDragResponse {
    pub requested_operation: Option<TransferOperation>,
}
