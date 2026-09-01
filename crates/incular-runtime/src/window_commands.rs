use crate::application_types::{WindowError, WindowRestorationId};
use crate::context::BuildContext;
use crate::tasks::RuntimeWake;
use crate::window_state::WindowManager;
use incular_platform::{
    NativeRequestId, PlatformCapabilities, PlatformOperationError, PlatformOperationResult,
    WindowCommand, WindowId, WindowOperation, WindowOptions,
};
use incular_widgets::Widget;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
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
    receiver: oneshot::Receiver<PlatformOperationResult>,
    bridge: Arc<WindowCommandBridge>,
    completed: bool,
    cancelled: Arc<AtomicBool>,
}

impl NativeOperationRequest {
    #[must_use]
    pub const fn id(&self) -> NativeRequestId {
        self.id
    }

    /// Non-blocking inspection for tests, embedders, and event-driven code.
    pub fn try_result(&mut self) -> Option<PlatformOperationResult> {
        match self.receiver.try_recv() {
            Ok(result) => {
                self.completed = true;
                Some(result)
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => None,
            Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                self.completed = true;
                Some(Err(PlatformOperationError::unavailable()))
            }
        }
    }
}

impl Future for NativeOperationRequest {
    type Output = PlatformOperationResult;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.receiver).poll(context) {
            Poll::Ready(Ok(result)) => {
                self.completed = true;
                Poll::Ready(result)
            }
            Poll::Ready(Err(_)) => {
                self.completed = true;
                Poll::Ready(Err(PlatformOperationError::unavailable()))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Drop for NativeOperationRequest {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        self.cancelled.store(true, Ordering::Release);
        self.bridge.cancel_request(self.id);
    }
}

/// A thread-safe reference to a generational Incular window. Methods only
/// enqueue data for the UI/event-loop turn; they never touch native state.
#[derive(Clone)]
pub struct WindowHandle {
    pub(crate) id: WindowId,
    pub(crate) bridge: Arc<WindowCommandBridge>,
    pub(crate) capabilities: Arc<RwLock<PlatformCapabilities>>,
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
    pub(crate) sender: mpsc::Sender<QueuedWindowCommand>,
    pub(crate) cancellation_sender: mpsc::Sender<NativeRequestId>,
    pub(crate) wake: Mutex<Option<Arc<dyn RuntimeWake>>>,
    pub(crate) next_request: AtomicU64,
    /// Serializes enqueue-vs-stop so shutdown can close the gate and then
    /// drain the receivers knowing no later command can cross the boundary.
    pub(crate) active: Mutex<bool>,
}

impl WindowCommandBridge {
    pub(crate) fn send(&self, command: WindowCommand) -> Result<(), WindowCommandEnqueueError> {
        let active = self.active.lock().expect("window command active gate");
        if !*active {
            return Err(WindowCommandEnqueueError::RuntimeStopped);
        }
        self.sender
            .send(QueuedWindowCommand {
                command,
                request: None,
            })
            .map_err(|_| WindowCommandEnqueueError::RuntimeStopped)?;
        drop(active);
        self.wake();
        Ok(())
    }

    pub(crate) fn request(
        self: &Arc<Self>,
        window_id: WindowId,
        operation: WindowOperation,
    ) -> Result<NativeOperationRequest, WindowCommandEnqueueError> {
        let active = self.active.lock().expect("window command active gate");
        if !*active {
            return Err(WindowCommandEnqueueError::RuntimeStopped);
        }
        let request_id = self
            .next_request
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map(NativeRequestId::new)
            .map_err(|_| WindowCommandEnqueueError::RequestIdExhausted)?;
        let (sender, receiver) = oneshot::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        self.sender
            .send(QueuedWindowCommand {
                command: WindowCommand::with_request(window_id, request_id, operation),
                request: Some(QueuedNativeRequest {
                    sender,
                    cancelled: cancelled.clone(),
                }),
            })
            .map_err(|_| WindowCommandEnqueueError::RuntimeStopped)?;
        drop(active);
        self.wake();
        Ok(NativeOperationRequest {
            id: request_id,
            receiver,
            bridge: self.clone(),
            completed: false,
            cancelled,
        })
    }

    pub(crate) fn cancel_request(&self, request_id: NativeRequestId) {
        let _ = self.cancellation_sender.send(request_id);
        self.wake();
    }

    pub(crate) fn stop(&self) {
        *self.active.lock().expect("window command active gate") = false;
    }

    fn wake(&self) {
        if let Some(wake) = self
            .wake
            .lock()
            .expect("window command wake mutex")
            .as_ref()
        {
            wake.wake();
        }
    }

    pub(crate) fn set_wake(&self, wake: Arc<dyn RuntimeWake>) {
        *self.wake.lock().expect("window command wake mutex") = Some(wake);
    }
}
