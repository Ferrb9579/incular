//! Portable contracts for native file and folder dialogs.
//!
//! Native window handles and native dialog objects deliberately do not appear
//! here. A desktop adapter receives these values, associates the dialog with
//! the requesting native window, and returns only paths/document descriptors.

use crate::CapabilitySupport;
use std::{
    collections::HashSet,
    fmt,
    path::{Path, PathBuf},
    sync::Arc,
};

/// Stable runtime identity for one file-dialog request. It is unrelated to a
/// native dialog object or operating-system handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileDialogRequestId(u64);

impl FileDialogRequestId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// One file/document path selected by the operating system or delivered by an
/// application activation. The path is preserved exactly; Incular does not
/// canonicalize it or take ownership of document persistence.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DocumentDescriptor {
    path: PathBuf,
}

impl DocumentDescriptor {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn into_path(self) -> PathBuf {
        self.path
    }
}

impl From<PathBuf> for DocumentDescriptor {
    fn from(value: PathBuf) -> Self {
        Self::new(value)
    }
}

impl From<DocumentDescriptor> for PathBuf {
    fn from(value: DocumentDescriptor) -> Self {
        value.into_path()
    }
}

/// Application-level open-document value intentionally shared with the future
/// OS activation pipeline. It transports intent only; Incular does not create a
/// document model, load bytes, or choose a persistence policy.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DocumentActivation {
    documents: Arc<[DocumentDescriptor]>,
}

impl DocumentActivation {
    #[must_use]
    pub fn new(documents: impl IntoIterator<Item = DocumentDescriptor>) -> Self {
        Self {
            documents: documents.into_iter().collect::<Vec<_>>().into(),
        }
    }

    #[must_use]
    pub fn from_paths(paths: impl IntoIterator<Item = impl Into<PathBuf>>) -> Self {
        Self::new(
            paths
                .into_iter()
                .map(|path| DocumentDescriptor::new(path.into())),
        )
    }

    #[must_use]
    pub fn documents(&self) -> &[DocumentDescriptor] {
        &self.documents
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }
}

/// Stable semantic identity for a file filter. Native display labels are
/// presentation only and never determine filter identity.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileDialogFilterId(String);

impl FileDialogFilterId {
    pub fn new(value: impl Into<String>) -> Result<Self, FileDialogValidationError> {
        let value = value.into();
        if !valid_identifier(&value) {
            return Err(FileDialogValidationError::InvalidFilterId(value));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FileDialogFilterId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Normalized file extension without a leading dot.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileExtension(String);

impl FileExtension {
    pub fn new(value: impl AsRef<str>) -> Result<Self, FileDialogValidationError> {
        let original = value.as_ref();
        let normalized = original.trim().trim_start_matches('.').to_ascii_lowercase();
        if normalized.is_empty()
            || normalized.chars().any(|character| {
                character.is_control()
                    || character.is_whitespace()
                    || matches!(character, '/' | '\\' | '*' | '?' | '[' | ']')
            })
        {
            return Err(FileDialogValidationError::InvalidExtension(
                original.to_owned(),
            ));
        }
        Ok(Self(normalized))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Semantic family of a file content-type identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FileContentTypeKind {
    /// Internet media type such as `image/png`.
    Mime,
    /// Platform-neutral representation of a UTI-like/native type identifier
    /// such as `public.jpeg`.
    TypeIdentifier,
}

/// Semantic content type accepted by a file filter. MIME types are normalized
/// to ASCII lowercase because MIME type/subtype tokens are case-insensitive;
/// UTI-like identifiers are preserved verbatim apart from surrounding space.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileContentType {
    value: String,
    kind: FileContentTypeKind,
}

impl FileContentType {
    pub fn new(value: impl AsRef<str>) -> Result<Self, FileDialogValidationError> {
        let original = value.as_ref();
        let trimmed = original.trim();
        if trimmed.is_empty()
            || trimmed.chars().any(|character| {
                character.is_control() || character.is_whitespace() || character == '\0'
            })
        {
            return Err(FileDialogValidationError::InvalidContentType(
                original.to_owned(),
            ));
        }
        let (normalized, kind) = if trimmed.contains('/') {
            let mut parts = trimmed.split('/');
            let Some(kind) = parts.next() else {
                unreachable!()
            };
            let Some(subtype) = parts.next() else {
                return Err(FileDialogValidationError::InvalidContentType(
                    original.to_owned(),
                ));
            };
            if parts.next().is_some() || kind.is_empty() || subtype.is_empty() {
                return Err(FileDialogValidationError::InvalidContentType(
                    original.to_owned(),
                ));
            }
            (trimmed.to_ascii_lowercase(), FileContentTypeKind::Mime)
        } else {
            (trimmed.to_owned(), FileContentTypeKind::TypeIdentifier)
        };
        Ok(Self {
            value: normalized,
            kind,
        })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }

    #[must_use]
    pub const fn kind(&self) -> FileContentTypeKind {
        self.kind
    }
}

/// One semantic filter with optional native-facing presentation text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileDialogFilter {
    id: FileDialogFilterId,
    label: Option<String>,
    extensions: Arc<[FileExtension]>,
    content_types: Arc<[FileContentType]>,
}

impl FileDialogFilter {
    pub fn new(
        id: FileDialogFilterId,
        label: Option<String>,
        extensions: impl IntoIterator<Item = FileExtension>,
        content_types: impl IntoIterator<Item = FileContentType>,
    ) -> Result<Self, FileDialogValidationError> {
        if label
            .as_ref()
            .is_some_and(|label| label.trim().is_empty() || label.chars().any(char::is_control))
        {
            return Err(FileDialogValidationError::InvalidFilterLabel);
        }
        let extensions = deduplicate(extensions);
        let content_types = deduplicate(content_types);
        if extensions.is_empty() && content_types.is_empty() {
            return Err(FileDialogValidationError::EmptyFilter(id));
        }
        Ok(Self {
            id,
            label,
            extensions: extensions.into(),
            content_types: content_types.into(),
        })
    }

    pub fn extensions(
        id: impl Into<String>,
        label: impl Into<String>,
        extensions: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Self, FileDialogValidationError> {
        let extensions = extensions
            .into_iter()
            .map(FileExtension::new)
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(
            FileDialogFilterId::new(id)?,
            Some(label.into()),
            extensions,
            [],
        )
    }

    pub fn content_types(
        id: impl Into<String>,
        label: impl Into<String>,
        content_types: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Self, FileDialogValidationError> {
        let content_types = content_types
            .into_iter()
            .map(FileContentType::new)
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(
            FileDialogFilterId::new(id)?,
            Some(label.into()),
            [],
            content_types,
        )
    }

    #[must_use]
    pub const fn id(&self) -> &FileDialogFilterId {
        &self.id
    }

    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    #[must_use]
    pub fn extensions_list(&self) -> &[FileExtension] {
        &self.extensions
    }

    #[must_use]
    pub fn content_types_list(&self) -> &[FileContentType] {
        &self.content_types
    }
}

fn deduplicate<T>(values: impl IntoIterator<Item = T>) -> Vec<T>
where
    T: Clone + Eq + std::hash::Hash,
{
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && !value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileDialogValidationError {
    InvalidFilterId(String),
    InvalidFilterLabel,
    InvalidExtension(String),
    InvalidContentType(String),
    EmptyFilter(FileDialogFilterId),
    DuplicateFilterId(FileDialogFilterId),
    InvalidSuggestedFileName,
    OptionNotApplicable(FileDialogOption),
}

impl fmt::Display for FileDialogValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFilterId(value) => {
                write!(formatter, "invalid file-dialog filter id {value:?}")
            }
            Self::InvalidFilterLabel => {
                formatter.write_str("file-dialog filter label must be non-empty printable text")
            }
            Self::InvalidExtension(value) => write!(formatter, "invalid file extension {value:?}"),
            Self::InvalidContentType(value) => {
                write!(formatter, "invalid file content type {value:?}")
            }
            Self::EmptyFilter(id) => write!(
                formatter,
                "file-dialog filter {id} has no extensions or content types"
            ),
            Self::DuplicateFilterId(id) => {
                write!(formatter, "duplicate file-dialog filter id {id}")
            }
            Self::InvalidSuggestedFileName => {
                formatter.write_str("suggested file name must be a single non-empty file name")
            }
            Self::OptionNotApplicable(option) => write!(
                formatter,
                "file-dialog option {option:?} does not apply to this dialog kind"
            ),
        }
    }
}

impl std::error::Error for FileDialogValidationError {}

/// Native file-dialog operation requested by application code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FileDialogKind {
    OpenFile,
    OpenFiles,
    SaveFile,
    SelectFolder,
}

/// Semantically important optional behavior a backend may support or reject.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FileDialogOption {
    ExtensionFilters,
    MimeTypeFilters,
    TypeIdentifierFilters,
    SuggestedFileName,
    SuggestedDirectory,
    ParentWindow,
}

/// Shared options for native file-dialog operations.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileDialogOptions {
    title: Option<String>,
    filters: Arc<[FileDialogFilter]>,
    suggested_file_name: Option<String>,
    suggested_directory: Option<PathBuf>,
}

impl FileDialogOptions {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn filters(
        mut self,
        filters: impl IntoIterator<Item = FileDialogFilter>,
    ) -> Result<Self, FileDialogValidationError> {
        let filters = filters.into_iter().collect::<Vec<_>>();
        let mut ids = HashSet::with_capacity(filters.len());
        for filter in &filters {
            if !ids.insert(filter.id.clone()) {
                return Err(FileDialogValidationError::DuplicateFilterId(
                    filter.id.clone(),
                ));
            }
        }
        self.filters = filters.into();
        Ok(self)
    }

    pub fn suggested_file_name(
        mut self,
        name: impl Into<String>,
    ) -> Result<Self, FileDialogValidationError> {
        let name = name.into();
        if name.trim().is_empty()
            || name.chars().any(char::is_control)
            || name.contains(['/', '\\'])
            || matches!(name.as_str(), "." | "..")
        {
            return Err(FileDialogValidationError::InvalidSuggestedFileName);
        }
        self.suggested_file_name = Some(name);
        Ok(self)
    }

    #[must_use]
    pub fn suggested_directory(mut self, directory: impl Into<PathBuf>) -> Self {
        self.suggested_directory = Some(directory.into());
        self
    }

    #[must_use]
    pub fn title_value(&self) -> Option<&str> {
        self.title.as_deref()
    }

    #[must_use]
    pub fn filters_value(&self) -> &[FileDialogFilter] {
        &self.filters
    }

    #[must_use]
    pub fn suggested_file_name_value(&self) -> Option<&str> {
        self.suggested_file_name.as_deref()
    }

    #[must_use]
    pub fn suggested_directory_value(&self) -> Option<&Path> {
        self.suggested_directory.as_deref()
    }
}

/// Fully validated, native-handle-free dialog request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileDialogRequest {
    pub kind: FileDialogKind,
    pub options: FileDialogOptions,
}

impl FileDialogRequest {
    pub fn new(
        kind: FileDialogKind,
        options: FileDialogOptions,
    ) -> Result<Self, FileDialogValidationError> {
        if kind == FileDialogKind::SelectFolder {
            for filter in options.filters.iter() {
                if !filter.extensions.is_empty() {
                    return Err(FileDialogValidationError::OptionNotApplicable(
                        FileDialogOption::ExtensionFilters,
                    ));
                }
                if let Some(content_type) = filter.content_types.first() {
                    let option = match content_type.kind() {
                        FileContentTypeKind::Mime => FileDialogOption::MimeTypeFilters,
                        FileContentTypeKind::TypeIdentifier => {
                            FileDialogOption::TypeIdentifierFilters
                        }
                    };
                    return Err(FileDialogValidationError::OptionNotApplicable(option));
                }
            }
        }
        if kind != FileDialogKind::SaveFile && options.suggested_file_name.is_some() {
            return Err(FileDialogValidationError::OptionNotApplicable(
                FileDialogOption::SuggestedFileName,
            ));
        }
        Ok(Self { kind, options })
    }
}

/// Portable result payload from a native file dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileDialogSelection {
    Documents(DocumentActivation),
    Folder(PathBuf),
}

/// User-visible completion of a native file dialog. User cancellation is a
/// normal outcome rather than a backend error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileDialogOutcome {
    Selected(FileDialogSelection),
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileDialogError {
    InvalidRequest(FileDialogValidationError),
    ParentClosed,
    ApplicationStopped,
    UnsupportedOperation(FileDialogKind),
    UnsupportedOption(FileDialogOption),
    InvalidResult,
    Backend(String),
}

impl fmt::Display for FileDialogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(error) => {
                write!(formatter, "invalid file-dialog request: {error}")
            }
            Self::ParentClosed => formatter.write_str("file-dialog parent window closed"),
            Self::ApplicationStopped => {
                formatter.write_str("file-dialog application service stopped")
            }
            Self::UnsupportedOperation(kind) => {
                write!(formatter, "file-dialog operation {kind:?} is unsupported")
            }
            Self::UnsupportedOption(option) => {
                write!(formatter, "file-dialog option {option:?} is unsupported")
            }
            Self::InvalidResult => formatter
                .write_str("file-dialog backend returned a result incompatible with its request"),
            Self::Backend(message) => write!(formatter, "file-dialog backend failed: {message}"),
        }
    }
}

impl std::error::Error for FileDialogError {}

impl From<FileDialogValidationError> for FileDialogError {
    fn from(value: FileDialogValidationError) -> Self {
        Self::InvalidRequest(value)
    }
}

/// Detailed native-dialog capabilities. Keeping options independent prevents a
/// backend from claiming full semantic support merely because it can show a
/// basic file picker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FileDialogCapabilities {
    pub open_file: CapabilitySupport,
    pub open_files: CapabilitySupport,
    pub save_file: CapabilitySupport,
    pub select_folder: CapabilitySupport,
    pub extension_filters: CapabilitySupport,
    pub mime_type_filters: CapabilitySupport,
    pub type_identifier_filters: CapabilitySupport,
    pub suggested_file_name: CapabilitySupport,
    pub suggested_directory: CapabilitySupport,
    pub parent_window: CapabilitySupport,
}

impl FileDialogCapabilities {
    #[must_use]
    pub const fn all(support: CapabilitySupport) -> Self {
        Self {
            open_file: support,
            open_files: support,
            save_file: support,
            select_folder: support,
            extension_filters: support,
            mime_type_filters: support,
            type_identifier_filters: support,
            suggested_file_name: support,
            suggested_directory: support,
            parent_window: support,
        }
    }

    #[must_use]
    pub const fn operation(self, kind: FileDialogKind) -> CapabilitySupport {
        match kind {
            FileDialogKind::OpenFile => self.open_file,
            FileDialogKind::OpenFiles => self.open_files,
            FileDialogKind::SaveFile => self.save_file,
            FileDialogKind::SelectFolder => self.select_folder,
        }
    }

    pub fn validate_request(&self, request: &FileDialogRequest) -> Result<(), FileDialogError> {
        if self.operation(request.kind) != CapabilitySupport::Supported {
            return Err(FileDialogError::UnsupportedOperation(request.kind));
        }
        if self.parent_window != CapabilitySupport::Supported {
            return Err(FileDialogError::UnsupportedOption(
                FileDialogOption::ParentWindow,
            ));
        }
        if request
            .options
            .filters
            .iter()
            .any(|filter| !filter.extensions.is_empty())
            && self.extension_filters != CapabilitySupport::Supported
        {
            return Err(FileDialogError::UnsupportedOption(
                FileDialogOption::ExtensionFilters,
            ));
        }
        for content_type in request
            .options
            .filters
            .iter()
            .flat_map(|filter| filter.content_types.iter())
        {
            let (support, option) = match content_type.kind() {
                FileContentTypeKind::Mime => {
                    (self.mime_type_filters, FileDialogOption::MimeTypeFilters)
                }
                FileContentTypeKind::TypeIdentifier => (
                    self.type_identifier_filters,
                    FileDialogOption::TypeIdentifierFilters,
                ),
            };
            if support != CapabilitySupport::Supported {
                return Err(FileDialogError::UnsupportedOption(option));
            }
        }
        if request.options.suggested_file_name.is_some()
            && self.suggested_file_name != CapabilitySupport::Supported
        {
            return Err(FileDialogError::UnsupportedOption(
                FileDialogOption::SuggestedFileName,
            ));
        }
        if request.options.suggested_directory.is_some()
            && self.suggested_directory != CapabilitySupport::Supported
        {
            return Err(FileDialogError::UnsupportedOption(
                FileDialogOption::SuggestedDirectory,
            ));
        }
        Ok(())
    }
}
