use crate::application_types::{WindowError, WindowRestorationId};
use crate::context::BuildContext;
use crate::tasks::RuntimeWake;
use crate::window_state::WindowManager;
use incular_platform::{WindowCommand, WindowId, WindowOperation, WindowOptions};
use incular_widgets::Widget;
use std::sync::{Arc, Mutex, mpsc};

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

/// A thread-safe reference to a generational Incular window. Methods only
/// enqueue data for the UI/event-loop turn; they never touch native state.
#[derive(Clone)]
pub struct WindowHandle {
    pub(crate) id: WindowId,
    pub(crate) bridge: Arc<WindowCommandBridge>,
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

    pub fn set_title(&self, title: impl Into<String>) -> bool {
        self.send(WindowOperation::SetTitle(title.into()))
    }

    pub fn set_visible(&self, visible: bool) -> bool {
        self.send(WindowOperation::SetVisible(visible))
    }

    pub fn request_logical_size(&self, size: incular_core::Size) -> bool {
        self.send(WindowOperation::SetLogicalSize(size))
    }

    pub fn request_focus(&self) -> bool {
        self.send(WindowOperation::RequestFocus)
    }

    pub fn request_redraw(&self) -> bool {
        self.send(WindowOperation::RequestRedraw)
    }

    pub fn close(&self) -> bool {
        self.send(WindowOperation::Close)
    }

    fn send(&self, operation: WindowOperation) -> bool {
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

pub(crate) struct WindowCommandBridge {
    pub(crate) sender: mpsc::Sender<WindowCommand>,
    pub(crate) wake: Mutex<Option<Arc<dyn RuntimeWake>>>,
}

impl WindowCommandBridge {
    pub(crate) fn send(&self, command: WindowCommand) -> bool {
        if self.sender.send(command).is_err() {
            return false;
        }
        if let Some(wake) = self
            .wake
            .lock()
            .expect("window command wake mutex")
            .as_ref()
        {
            wake.wake();
        }
        true
    }

    pub(crate) fn set_wake(&self, wake: Arc<dyn RuntimeWake>) {
        *self.wake.lock().expect("window command wake mutex") = Some(wake);
    }
}
