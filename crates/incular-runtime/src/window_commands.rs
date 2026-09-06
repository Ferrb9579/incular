use crate::application_types::{WindowError, WindowRestorationId};
use crate::context::BuildContext;
use crate::file_dialogs::FileDialogService;
use crate::request_channel::{ChannelError, RequestChannel, RequestReceiver};
use crate::tasks::RuntimeWake;
use crate::transient_presentation::TransientPresentationResolution;
use crate::window_state::WindowManager;
use incular_platform::{
    CapabilitySupport, CursorGrabMode, DisplayPlacementArea, DisplaySnapshot, Fullscreen,
    LogicalSizeLimits, LogicalWindowPosition, NativeRequestId, PhysicalScreenPosition,
    PlatformCapabilities, PlatformOperationError, PlatformOperationResult, UserAttentionType,
    WindowCommand, WindowIcon, WindowId, WindowLevel, WindowObservedState, WindowOperation,
    WindowOptions,
};
use incular_widgets::Widget;
use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    task::{Context, Poll},
};
use tokio::sync::oneshot;

/// A portable UI-thread command emitted by the application for a native
/// desktop adapter. It contains Incular IDs and options only—never Winit
/// IDs, native pointers, or a GPU surface.
#[derive(Clone, Debug, PartialEq)]
pub enum NativeWindowCommand {
    Create {
        window_id: WindowId,
        options: WindowOptions,
    },
    Operate(WindowCommand),
}

/// Failure to enqueue a window command into the runtime/UI bridge.
///
/// This says nothing about native execution. Operations that need a native
/// result return a [`NativeOperationRequest`] after successful enqueue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowCommandEnqueueError {
    RuntimeStopped,
    RequestIdExhausted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WindowPlacementError {
    Unsupported,
    OuterSizeUnavailable,
    DisplayGeometryUnavailable(DisplayPlacementArea),
    Enqueue(WindowCommandEnqueueError),
}

#[derive(Default)]
pub(crate) struct DisplayCatalog {
    displays: BTreeMap<incular_platform::DisplayId, DisplaySnapshot>,
    primary: Option<incular_platform::DisplayId>,
}

impl DisplayCatalog {
    pub(crate) fn replace(
        &mut self,
        displays: impl IntoIterator<Item = DisplaySnapshot>,
        primary: Option<incular_platform::DisplayId>,
    ) {
        self.displays = displays
            .into_iter()
            .map(|display| (display.id, display))
            .collect();
        self.primary = primary.filter(|id| self.displays.contains_key(id));
    }

    pub(crate) fn all(&self) -> Vec<DisplaySnapshot> {
        self.displays.values().cloned().collect()
    }

    pub(crate) fn get(&self, id: incular_platform::DisplayId) -> Option<DisplaySnapshot> {
        self.displays.get(&id).cloned()
    }

    pub(crate) fn primary(&self) -> Option<DisplaySnapshot> {
        self.primary.and_then(|id| self.get(id))
    }
}

impl std::fmt::Display for WindowPlacementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => formatter.write_str("top-level window placement is unsupported"),
            Self::OuterSizeUnavailable => {
                formatter.write_str("native outer window size is not available yet")
            }
            Self::DisplayGeometryUnavailable(area) => {
                write!(formatter, "display {area:?} geometry is unavailable")
            }
            Self::Enqueue(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for WindowPlacementError {}

impl From<WindowCommandEnqueueError> for WindowPlacementError {
    fn from(value: WindowCommandEnqueueError) -> Self {
        Self::Enqueue(value)
    }
}

/// Runtime disposition after a backend reports one native completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeOperationCompletionStatus {
    Completed,
    UnknownRequest,
    TargetMismatch,
    StaleTarget,
}

impl std::fmt::Display for WindowCommandEnqueueError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RuntimeStopped => {
                formatter.write_str("the Incular runtime command bridge has stopped")
            }
            Self::RequestIdExhausted => {
                formatter.write_str("the native operation request-id space is exhausted")
            }
        }
    }
}

impl std::error::Error for WindowCommandEnqueueError {}

/// Awaitable result of one native platform operation.
///
/// Dropping the request only detaches result delivery; it does not attempt to
/// undo a native operation that may already have executed. The runtime removes
/// the pending completion registration on the next UI turn.
#[must_use = "native operation requests must be awaited, inspected, or explicitly dropped"]
pub struct NativeOperationRequest {
    id: NativeRequestId,
    receiver: RequestReceiver<PlatformOperationResult>,
}

impl NativeOperationRequest {
    #[must_use]
    pub const fn id(&self) -> NativeRequestId {
        self.id
    }

    /// Non-blocking inspection for tests, embedders, and event-driven code.
    pub fn try_result(&mut self) -> Option<PlatformOperationResult> {
        self.receiver
            .try_result(|| Err(PlatformOperationError::unavailable()))
    }
}

impl Future for NativeOperationRequest {
    type Output = PlatformOperationResult;
    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        self.receiver
            .poll_result(context, || Err(PlatformOperationError::unavailable()))
    }
}

/// A thread-safe reference to a generational Incular window. Methods only
/// enqueue data for the UI/event-loop turn; they never touch native state.
#[derive(Clone)]
pub struct WindowHandle {
    pub(crate) id: WindowId,
    pub(crate) bridge: Arc<WindowCommandBridge>,
    pub(crate) file_dialogs: FileDialogService,
    pub(crate) capabilities: Arc<RwLock<PlatformCapabilities>>,
    pub(crate) observed_state: Arc<RwLock<WindowObservedState>>,
    pub(crate) displays: Arc<RwLock<DisplayCatalog>>,
    pub(crate) transient_presentations: Arc<RwLock<Vec<TransientPresentationResolution>>>,
}

impl std::fmt::Debug for WindowHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("WindowHandle")
            .field(&self.id)
            .finish()
    }
}

impl WindowHandle {
    #[must_use]
    pub const fn id(&self) -> WindowId {
        self.id
    }

    #[must_use]
    pub fn capabilities(&self) -> PlatformCapabilities {
        *self
            .capabilities
            .read()
            .expect("window capability snapshot lock")
    }

    /// Native file/folder dialog service associated with this window. The
    /// service retains only the Incular window identity; native parent handles
    /// remain inside the desktop adapter.
    #[must_use]
    pub fn file_dialogs(&self) -> FileDialogService {
        self.file_dialogs.clone()
    }

    #[must_use]
    pub fn observed_state(&self) -> WindowObservedState {
        *self
            .observed_state
            .read()
            .expect("window observed-state snapshot lock")
    }

    /// Resolved native-vs-overlay presentation for visible retained transients
    /// in this window. The native adapter updates this after host negotiation;
    /// an empty list means no visible semantic transient is currently present.
    #[must_use]
    pub fn transient_presentations(&self) -> Vec<TransientPresentationResolution> {
        self.transient_presentations
            .read()
            .expect("transient presentation snapshot lock")
            .clone()
    }

    /// Immutable snapshots for displays currently published by the native
    /// backend. Removed displays disappear immediately; a later display using
    /// the same registry slot carries a different generation.
    #[must_use]
    pub fn displays(&self) -> Vec<DisplaySnapshot> {
        self.displays.read().expect("display catalog lock").all()
    }

    #[must_use]
    pub fn display(&self, id: incular_platform::DisplayId) -> Option<DisplaySnapshot> {
        self.displays.read().expect("display catalog lock").get(id)
    }

    #[must_use]
    pub fn primary_display(&self) -> Option<DisplaySnapshot> {
        self.displays
            .read()
            .expect("display catalog lock")
            .primary()
    }

    #[must_use]
    pub fn current_display(&self) -> Option<DisplaySnapshot> {
        let id = self.observed_state().current_display?;
        self.display(id)
    }

    pub fn set_title(&self, title: impl Into<String>) -> Result<(), WindowCommandEnqueueError> {
        self.send(WindowOperation::SetTitle(title.into()))
    }

    pub fn set_visible(&self, visible: bool) -> Result<(), WindowCommandEnqueueError> {
        self.send(WindowOperation::SetVisible(visible))
    }

    pub fn request_logical_size(
        &self,
        size: incular_core::Size,
    ) -> Result<(), WindowCommandEnqueueError> {
        self.send(WindowOperation::SetLogicalSize(size))
    }

    pub fn set_outer_position(
        &self,
        position: PhysicalScreenPosition,
    ) -> Result<(), WindowCommandEnqueueError> {
        self.send(WindowOperation::SetOuterPosition(position))
    }

    /// Centers this window using already-observed native outer size and one
    /// immutable display snapshot. Work-area centering never falls back to full
    /// monitor bounds: callers must choose that policy explicitly.
    pub fn center_on_display(
        &self,
        display: &DisplaySnapshot,
        area: DisplayPlacementArea,
    ) -> Result<PhysicalScreenPosition, WindowPlacementError> {
        if self.capabilities().display.set_window_position == CapabilitySupport::Unsupported {
            return Err(WindowPlacementError::Unsupported);
        }
        let outer_size = self
            .observed_state()
            .outer_size
            .ok_or(WindowPlacementError::OuterSizeUnavailable)?;
        let bounds = display
            .placement_rect(area)
            .ok_or(WindowPlacementError::DisplayGeometryUnavailable(area))?;
        let target = bounds.centered_position(outer_size);
        self.set_outer_position(target)?;
        Ok(target)
    }

    /// Moves the window to a physical point relative to one known display.
    /// This is intentionally unavailable when the display snapshot has no
    /// desktop-global origin (for example a Wayland top-level display).
    pub fn move_relative_to_display(
        &self,
        display: &DisplaySnapshot,
        position: incular_platform::PhysicalDisplayPosition,
    ) -> Result<PhysicalScreenPosition, WindowPlacementError> {
        if self.capabilities().display.set_window_position == CapabilitySupport::Unsupported {
            return Err(WindowPlacementError::Unsupported);
        }
        let target = display.screen_position(position).ok_or(
            WindowPlacementError::DisplayGeometryUnavailable(DisplayPlacementArea::FullBounds),
        )?;
        self.set_outer_position(target)?;
        Ok(target)
    }

    /// Begins a compositor/window-manager-owned interactive move. The native
    /// backend may require this call to follow a real primary-button press.
    pub fn begin_move_drag(&self) -> Result<NativeOperationRequest, WindowCommandEnqueueError> {
        self.bridge.request(self.id, WindowOperation::BeginMoveDrag)
    }

    /// Begins a compositor/window-manager-owned interactive resize from one
    /// edge/corner. Incular never emulates this by changing global coordinates.
    pub fn begin_resize_drag(
        &self,
        direction: incular_core::WindowResizeDirection,
    ) -> Result<NativeOperationRequest, WindowCommandEnqueueError> {
        self.bridge
            .request(self.id, WindowOperation::BeginResizeDrag(direction))
    }

    pub fn set_minimized(&self, minimized: bool) -> Result<(), WindowCommandEnqueueError> {
        self.send(WindowOperation::SetMinimized(minimized))
    }

    pub fn set_maximized(&self, maximized: bool) -> Result<(), WindowCommandEnqueueError> {
        self.send(WindowOperation::SetMaximized(maximized))
    }

    pub fn set_fullscreen(
        &self,
        fullscreen: Option<Fullscreen>,
    ) -> Result<(), WindowCommandEnqueueError> {
        self.send(WindowOperation::SetFullscreen(fullscreen))
    }

    pub fn set_resizable(&self, resizable: bool) -> Result<(), WindowCommandEnqueueError> {
        self.send(WindowOperation::SetResizable(resizable))
    }

    pub fn set_decorations(&self, decorations: bool) -> Result<(), WindowCommandEnqueueError> {
        self.send(WindowOperation::SetDecorations(decorations))
    }

    pub fn set_logical_size_limits(
        &self,
        limits: LogicalSizeLimits,
    ) -> Result<(), WindowCommandEnqueueError> {
        self.send(WindowOperation::SetLogicalSizeLimits(limits))
    }

    pub fn set_window_level(&self, level: WindowLevel) -> Result<(), WindowCommandEnqueueError> {
        self.send(WindowOperation::SetWindowLevel(level))
    }

    pub fn set_window_icon(
        &self,
        icon: Option<WindowIcon>,
    ) -> Result<(), WindowCommandEnqueueError> {
        self.send(WindowOperation::SetWindowIcon(icon))
    }

    pub fn request_user_attention(
        &self,
        attention: UserAttentionType,
    ) -> Result<(), WindowCommandEnqueueError> {
        self.send(WindowOperation::RequestUserAttention(Some(attention)))
    }

    pub fn cancel_user_attention(&self) -> Result<(), WindowCommandEnqueueError> {
        self.send(WindowOperation::RequestUserAttention(None))
    }

    /// Requests native cursor confinement/locking. The returned request
    /// completes with a typed platform result; the runtime does not claim a
    /// grab succeeded merely because the command was enqueued.
    pub fn set_cursor_grab(
        &self,
        mode: CursorGrabMode,
    ) -> Result<NativeOperationRequest, WindowCommandEnqueueError> {
        self.bridge
            .request(self.id, WindowOperation::SetCursorGrab(mode))
    }

    /// Shows or hides the native cursor for this window.
    pub fn set_cursor_visible(
        &self,
        visible: bool,
    ) -> Result<NativeOperationRequest, WindowCommandEnqueueError> {
        self.bridge
            .request(self.id, WindowOperation::SetCursorVisible(visible))
    }

    /// Warps the native cursor to a validated logical client-area position.
    pub fn set_cursor_position(
        &self,
        position: LogicalWindowPosition,
    ) -> Result<NativeOperationRequest, WindowCommandEnqueueError> {
        self.bridge
            .request(self.id, WindowOperation::SetCursorPosition(position))
    }

    /// Requests the window's normalized content-capture policy. The command
    /// is applied by the native adapter on its event-loop thread. Unsupported
    /// adapters resolve the returned request with a typed platform error.
    pub fn set_content_sensitivity(
        &self,
        sensitivity: incular_config::ContentSensitivity,
    ) -> Result<NativeOperationRequest, WindowCommandEnqueueError> {
        self.bridge
            .request(self.id, WindowOperation::SetContentSensitivity(sensitivity))
    }

    pub fn request_focus(&self) -> Result<(), WindowCommandEnqueueError> {
        self.send(WindowOperation::RequestFocus)
    }

    pub fn request_redraw(&self) -> Result<(), WindowCommandEnqueueError> {
        self.send(WindowOperation::RequestRedraw)
    }

    pub fn close(&self) -> Result<(), WindowCommandEnqueueError> {
        self.send(WindowOperation::Close)
    }

    fn send(&self, operation: WindowOperation) -> Result<(), WindowCommandEnqueueError> {
        self.bridge.send(WindowCommand::new(self.id, operation))
    }
}

/// Cloneable UI-side capability for opening retained windows after startup.
/// It contains no native handle and can be retained by button callbacks; native
/// creation is still queued for the active desktop event-loop callback.
#[derive(Clone)]
pub struct WindowOpener {
    pub(crate) manager: WindowManager,
}

impl WindowOpener {
    pub fn open_window(
        &self,
        options: WindowOptions,
        root: Widget,
    ) -> Result<WindowHandle, WindowError> {
        self.manager.open_window(options, root)
    }

    pub fn open_window_with(
        &self,
        options: WindowOptions,
        build: impl FnMut(&mut BuildContext) -> Widget + 'static,
    ) -> Result<WindowHandle, WindowError> {
        self.manager.open_window_with(options, build)
    }

    /// Opens a window whose descriptor is persisted under a stable
    /// application-provided ID. Its factory still lives in application code;
    /// no widget or native handle is serialized.
    pub fn open_restorable_window_with(
        &self,
        restoration_id: WindowRestorationId,
        kind: impl Into<String>,
        options: WindowOptions,
        build: impl FnMut(&mut BuildContext) -> Widget + 'static,
    ) -> Result<WindowHandle, WindowError> {
        self.manager
            .open_restorable_window_with(restoration_id, kind, options, build)
    }
}

pub(crate) struct QueuedNativeRequest {
    pub(crate) sender: oneshot::Sender<PlatformOperationResult>,
    pub(crate) cancelled: Arc<AtomicBool>,
}

pub(crate) struct QueuedWindowCommand {
    pub(crate) command: WindowCommand,
    pub(crate) request: Option<QueuedNativeRequest>,
}

pub(crate) struct WindowCommandBridge {
    channel: RequestChannel<QueuedWindowCommand>,
    cancellation_sender: mpsc::Sender<NativeRequestId>,
}

impl WindowCommandBridge {
    pub(crate) fn new(
        sender: mpsc::Sender<QueuedWindowCommand>,
        cancellation_sender: mpsc::Sender<NativeRequestId>,
    ) -> Self {
        Self {
            channel: RequestChannel::new(sender),
            cancellation_sender,
        }
    }
    pub(crate) fn send(&self, command: WindowCommand) -> Result<(), WindowCommandEnqueueError> {
        self.channel
            .send(QueuedWindowCommand {
                command,
                request: None,
            })
            .map_err(channel_error)
    }
    pub(crate) fn request(
        self: &Arc<Self>,
        window_id: WindowId,
        operation: WindowOperation,
    ) -> Result<NativeOperationRequest, WindowCommandEnqueueError> {
        self.channel
            .request(|id| {
                let request_id = NativeRequestId::new(id);
                let (sender, receiver) = oneshot::channel();
                let cancelled = Arc::new(AtomicBool::new(false));
                let queued = QueuedWindowCommand {
                    command: WindowCommand::with_request(window_id, request_id, operation),
                    request: Some(QueuedNativeRequest {
                        sender,
                        cancelled: cancelled.clone(),
                    }),
                };
                let bridge = self.clone();
                let receiver = RequestReceiver::new(receiver, move || {
                    cancelled.store(true, Ordering::Release);
                    bridge.cancel_request(request_id);
                });
                (
                    queued,
                    NativeOperationRequest {
                        id: request_id,
                        receiver,
                    },
                )
            })
            .map_err(channel_error)
    }
    pub(crate) fn cancel_request(&self, request_id: NativeRequestId) {
        self.channel.cancel(&self.cancellation_sender, request_id);
    }
    pub(crate) fn stop(&self) {
        self.channel.stop();
    }
    pub(crate) fn set_wake(&self, wake: Arc<dyn RuntimeWake>) {
        self.channel.set_wake(wake);
    }
}
fn channel_error(error: ChannelError) -> WindowCommandEnqueueError {
    match error {
        ChannelError::Stopped => WindowCommandEnqueueError::RuntimeStopped,
        ChannelError::IdExhausted => WindowCommandEnqueueError::RequestIdExhausted,
    }
}
