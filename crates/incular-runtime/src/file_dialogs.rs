//! Window-owned asynchronous native file-dialog requests.
use crate::request_channel::{RequestChannel, RequestReceiver};

use crate::tasks::RuntimeWake;
use incular_platform::{
    CapabilitySupport, DocumentDescriptor, FileContentTypeKind, FileDialogCapabilities,
    FileDialogError, FileDialogKind, FileDialogOption, FileDialogOptions, FileDialogOutcome,
    FileDialogRequest as PortableRequest, FileDialogRequestId, FileDialogSelection,
    PlatformCapabilities, WindowId,
};
use std::{
    collections::VecDeque,
    future::Future,
    marker::PhantomData,
    path::PathBuf,
    pin::Pin,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    task::{Context, Poll},
};
use tokio::sync::oneshot;

type FileDialogResult = Result<FileDialogOutcome, FileDialogError>;

/// Native-handle-free request passed from the runtime to a desktop host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeFileDialogRequest {
    pub request_id: FileDialogRequestId,
    pub window_id: WindowId,
    pub request: PortableRequest,
}

/// Completion returned by a desktop host on the application's UI/event-loop
/// thread. Late completions are rejected by request/window identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeFileDialogCompletion {
    pub request_id: FileDialogRequestId,
    pub window_id: WindowId,
    pub result: FileDialogResult,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileDialogCompletionStatus {
    Completed,
    InvalidResult,
    UnknownRequest,
    TargetMismatch,
}

/// Cloneable, window-associated application service for native file dialogs.
/// Requests contain no native parent handle; the desktop host resolves the
/// [`WindowId`] to the real parent only when it starts the native dialog.
#[derive(Clone)]
pub struct FileDialogService {
    window_id: WindowId,
    bridge: Arc<FileDialogBridge>,
    capabilities: Arc<RwLock<PlatformCapabilities>>,
}

impl std::fmt::Debug for FileDialogService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("FileDialogService")
            .field(&self.window_id)
            .finish()
    }
}

impl FileDialogService {
    pub(crate) fn new(
        window_id: WindowId,
        bridge: Arc<FileDialogBridge>,
        capabilities: Arc<RwLock<PlatformCapabilities>>,
    ) -> Self {
        Self {
            window_id,
            bridge,
            capabilities,
        }
    }

    #[must_use]
    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    #[must_use]
    pub fn capabilities(&self) -> FileDialogCapabilities {
        self.capabilities
            .read()
            .expect("window capability snapshot lock")
            .application_services
            .file_dialogs
    }

    /// Opens one native file picker and resolves to `None` when the user
    /// cancels it. Backend failures and lifecycle invalidation remain errors.
    pub async fn open_file(
        &self,
        options: FileDialogOptions,
    ) -> Result<Option<DocumentDescriptor>, FileDialogError> {
        self.request_open_file(options)?.await
    }

    /// Opens a native multi-file picker, preserving the backend's selection
    /// order exactly. User cancellation resolves to `None`.
    pub async fn open_files(
        &self,
        options: FileDialogOptions,
    ) -> Result<Option<Vec<DocumentDescriptor>>, FileDialogError> {
        self.request_open_files(options)?.await
    }

    /// Opens a native save picker. Incular returns the selected destination
    /// path only; it never creates, truncates, or writes the file.
    pub async fn save_file(
        &self,
        options: FileDialogOptions,
    ) -> Result<Option<DocumentDescriptor>, FileDialogError> {
        self.request_save_file(options)?.await
    }

    /// Opens a native folder picker and resolves to `None` on user cancel.
    pub async fn select_folder(
        &self,
        options: FileDialogOptions,
    ) -> Result<Option<PathBuf>, FileDialogError> {
        self.request_select_folder(options)?.await
    }

    /// Creates an explicit request handle for callers that need request
    /// identity, manual polling, or cancellation-by-drop before awaiting.
    pub fn request_open_file(
        &self,
        options: FileDialogOptions,
    ) -> Result<FileDialogRequest<Option<DocumentDescriptor>>, FileDialogError> {
        self.request(FileDialogKind::OpenFile, options, map_single_document)
    }

    /// Explicit-handle form of [`Self::open_files`].
    pub fn request_open_files(
        &self,
        options: FileDialogOptions,
    ) -> Result<FileDialogRequest<Option<Vec<DocumentDescriptor>>>, FileDialogError> {
        self.request(FileDialogKind::OpenFiles, options, map_multiple_documents)
    }

    /// Explicit-handle form of [`Self::save_file`].
    pub fn request_save_file(
        &self,
        options: FileDialogOptions,
    ) -> Result<FileDialogRequest<Option<DocumentDescriptor>>, FileDialogError> {
        self.request(FileDialogKind::SaveFile, options, map_single_document)
    }

    /// Explicit-handle form of [`Self::select_folder`].
    pub fn request_select_folder(
        &self,
        options: FileDialogOptions,
    ) -> Result<FileDialogRequest<Option<PathBuf>>, FileDialogError> {
        self.request(FileDialogKind::SelectFolder, options, map_folder)
    }

    fn request<T>(
        &self,
        kind: FileDialogKind,
        options: FileDialogOptions,
        map: fn(FileDialogResult) -> Result<T, FileDialogError>,
    ) -> Result<FileDialogRequest<T>, FileDialogError> {
        let request = PortableRequest::new(kind, options).map_err(FileDialogError::from)?;
        let raw = self.bridge.request(self.window_id, request)?;
        Ok(FileDialogRequest {
            raw,
            map,
            marker: PhantomData,
        })
    }
}

fn map_single_document(
    result: FileDialogResult,
) -> Result<Option<DocumentDescriptor>, FileDialogError> {
    match result? {
        FileDialogOutcome::Cancelled => Ok(None),
        FileDialogOutcome::Selected(FileDialogSelection::Documents(activation))
            if activation.documents().len() == 1 =>
        {
            Ok(activation.documents().first().cloned())
        }
        FileDialogOutcome::Selected(_) => Err(FileDialogError::InvalidResult),
    }
}

fn map_multiple_documents(
    result: FileDialogResult,
) -> Result<Option<Vec<DocumentDescriptor>>, FileDialogError> {
    match result? {
        FileDialogOutcome::Cancelled => Ok(None),
        FileDialogOutcome::Selected(FileDialogSelection::Documents(activation))
            if !activation.is_empty() =>
        {
            Ok(Some(activation.documents().to_vec()))
        }
        FileDialogOutcome::Selected(_) => Err(FileDialogError::InvalidResult),
    }
}

fn map_folder(result: FileDialogResult) -> Result<Option<PathBuf>, FileDialogError> {
    match result? {
        FileDialogOutcome::Cancelled => Ok(None),
        FileDialogOutcome::Selected(FileDialogSelection::Folder(path)) => Ok(Some(path)),
        FileDialogOutcome::Selected(_) => Err(FileDialogError::InvalidResult),
    }
}

/// Awaitable typed result for one native file dialog.
///
/// Dropping this handle detaches the caller from result delivery. If the native
/// dialog is already active, the runtime keeps that modal slot occupied until
/// its backend completion arrives rather than opening a second dialog over it.
#[must_use = "file-dialog requests must be awaited, inspected, or explicitly dropped"]
pub struct FileDialogRequest<T> {
    raw: RawFileDialogRequest,
    map: fn(FileDialogResult) -> Result<T, FileDialogError>,
    marker: PhantomData<T>,
}

impl<T> Unpin for FileDialogRequest<T> {}

impl<T> FileDialogRequest<T> {
    #[must_use]
    pub const fn id(&self) -> FileDialogRequestId {
        self.raw.id
    }

    pub fn try_result(&mut self) -> Option<Result<T, FileDialogError>> {
        self.raw.try_result().map(self.map)
    }
}

impl<T> Future for FileDialogRequest<T> {
    type Output = Result<T, FileDialogError>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.as_mut().get_mut().raw).poll(context) {
            Poll::Ready(result) => Poll::Ready((self.map)(result)),
            Poll::Pending => Poll::Pending,
        }
    }
}

struct RawFileDialogRequest {
    id: FileDialogRequestId,
    receiver: RequestReceiver<FileDialogResult>,
}
impl RawFileDialogRequest {
    fn try_result(&mut self) -> Option<FileDialogResult> {
        self.receiver
            .try_result(|| Err(FileDialogError::ApplicationStopped))
    }
}
impl Future for RawFileDialogRequest {
    type Output = FileDialogResult;
    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        self.receiver
            .poll_result(context, || Err(FileDialogError::ApplicationStopped))
    }
}

pub(crate) struct QueuedFileDialogRequest {
    pub(crate) request_id: FileDialogRequestId,
    pub(crate) window_id: WindowId,
    pub(crate) request: PortableRequest,
    pub(crate) sender: oneshot::Sender<FileDialogResult>,
    pub(crate) cancelled: Arc<AtomicBool>,
}

pub(crate) struct FileDialogBridge {
    channel: RequestChannel<QueuedFileDialogRequest>,
    cancellation_sender: mpsc::Sender<FileDialogRequestId>,
}
impl FileDialogBridge {
    pub(crate) fn new(
        sender: mpsc::Sender<QueuedFileDialogRequest>,
        cancellation_sender: mpsc::Sender<FileDialogRequestId>,
    ) -> Self {
        Self {
            channel: RequestChannel::new(sender),
            cancellation_sender,
        }
    }
    fn request(
        self: &Arc<Self>,
        window_id: WindowId,
        request: PortableRequest,
    ) -> Result<RawFileDialogRequest, FileDialogError> {
        self.channel
            .request(|id| {
                let request_id = FileDialogRequestId::new(id);
                let (sender, receiver) = oneshot::channel();
                let cancelled = Arc::new(AtomicBool::new(false));
                let queued = QueuedFileDialogRequest {
                    request_id,
                    window_id,
                    request,
                    sender,
                    cancelled: cancelled.clone(),
                };
                let bridge = self.clone();
                let receiver = RequestReceiver::new(receiver, move || {
                    cancelled.store(true, Ordering::Release);
                    bridge.cancel(request_id);
                });
                (
                    queued,
                    RawFileDialogRequest {
                        id: request_id,
                        receiver,
                    },
                )
            })
            .map_err(|_| FileDialogError::ApplicationStopped)
    }
    fn cancel(&self, id: FileDialogRequestId) {
        self.channel.cancel(&self.cancellation_sender, id);
    }
    pub(crate) fn set_wake(&self, wake: Arc<dyn RuntimeWake>) {
        self.channel.set_wake(wake);
    }
    pub(crate) fn stop(&self) {
        self.channel.stop();
    }
}

/// Deterministic in-memory host for headless integration tests and custom
/// embedders. Scripted outcomes are consumed FIFO and the adapter records the
/// exact portable requests it received.
#[derive(Debug)]
pub struct MemoryFileDialogAdapter {
    capabilities: FileDialogCapabilities,
    scripted: VecDeque<FileDialogResult>,
    observed: Vec<NativeFileDialogRequest>,
}

impl Default for MemoryFileDialogAdapter {
    fn default() -> Self {
        Self {
            capabilities: FileDialogCapabilities::all(CapabilitySupport::Supported),
            scripted: VecDeque::new(),
            observed: Vec::new(),
        }
    }
}

impl MemoryFileDialogAdapter {
    #[must_use]
    pub const fn capabilities(&self) -> FileDialogCapabilities {
        self.capabilities
    }

    #[must_use]
    pub fn with_capabilities(mut self, capabilities: FileDialogCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    pub fn push_result(&mut self, result: FileDialogResult) {
        self.scripted.push_back(result);
    }

    pub fn respond(&mut self, request: NativeFileDialogRequest) -> NativeFileDialogCompletion {
        let result = self
            .capabilities
            .validate_request(&request.request)
            .map(|()| {
                self.scripted
                    .pop_front()
                    .unwrap_or(Ok(FileDialogOutcome::Cancelled))
            })
            .unwrap_or_else(Err);
        self.observed.push(request.clone());
        NativeFileDialogCompletion {
            request_id: request.request_id,
            window_id: request.window_id,
            result,
        }
    }

    #[must_use]
    pub fn observed(&self) -> &[NativeFileDialogRequest] {
        &self.observed
    }
}

pub(crate) fn explicitly_unsupported_request(
    capabilities: FileDialogCapabilities,
    request: &PortableRequest,
) -> Option<FileDialogError> {
    if capabilities.operation(request.kind) == CapabilitySupport::Unsupported {
        return Some(FileDialogError::UnsupportedOperation(request.kind));
    }
    if capabilities.parent_window == CapabilitySupport::Unsupported {
        return Some(FileDialogError::UnsupportedOption(
            FileDialogOption::ParentWindow,
        ));
    }
    if request
        .options
        .filters_value()
        .iter()
        .any(|filter| !filter.extensions_list().is_empty())
        && capabilities.extension_filters == CapabilitySupport::Unsupported
    {
        return Some(FileDialogError::UnsupportedOption(
            FileDialogOption::ExtensionFilters,
        ));
    }
    for content_type in request
        .options
        .filters_value()
        .iter()
        .flat_map(|filter| filter.content_types_list())
    {
        let (support, option) = match content_type.kind() {
            FileContentTypeKind::Mime => (
                capabilities.mime_type_filters,
                FileDialogOption::MimeTypeFilters,
            ),
            FileContentTypeKind::TypeIdentifier => (
                capabilities.type_identifier_filters,
                FileDialogOption::TypeIdentifierFilters,
            ),
        };
        if support == CapabilitySupport::Unsupported {
            return Some(FileDialogError::UnsupportedOption(option));
        }
    }
    if request.options.suggested_file_name_value().is_some()
        && capabilities.suggested_file_name == CapabilitySupport::Unsupported
    {
        return Some(FileDialogError::UnsupportedOption(
            FileDialogOption::SuggestedFileName,
        ));
    }
    if request.options.suggested_directory_value().is_some()
        && capabilities.suggested_directory == CapabilitySupport::Unsupported
    {
        return Some(FileDialogError::UnsupportedOption(
            FileDialogOption::SuggestedDirectory,
        ));
    }
    None
}

pub(crate) fn result_matches_request(
    request: &PortableRequest,
    outcome: &FileDialogOutcome,
) -> bool {
    match (request.kind, outcome) {
        (_, FileDialogOutcome::Cancelled) => true,
        (
            FileDialogKind::OpenFile | FileDialogKind::SaveFile,
            FileDialogOutcome::Selected(FileDialogSelection::Documents(activation)),
        ) => activation.documents().len() == 1,
        (
            FileDialogKind::OpenFiles,
            FileDialogOutcome::Selected(FileDialogSelection::Documents(activation)),
        ) => !activation.is_empty(),
        (
            FileDialogKind::SelectFolder,
            FileDialogOutcome::Selected(FileDialogSelection::Folder(_)),
        ) => true,
        _ => false,
    }
}
