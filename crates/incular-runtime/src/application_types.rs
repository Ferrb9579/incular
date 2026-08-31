use crate::context::BuildContext;
use crate::profiling::SchedulerCounters;
use crate::tasks::TaskFailure;
use incular_core::RestorationKey;
use incular_platform::WindowOptionsError;
use incular_widgets::Widget;
use incular_widgets::internal::TreeError;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ApplicationSchedulerCounters {
    pub(crate) runtime_wakes: u64,
    pub(crate) redraw_requests: u64,
    pub(crate) frames_started: u64,
    pub(crate) frames_presented: u64,
    pub(crate) frames_skipped: u64,
}

impl ApplicationSchedulerCounters {
    pub(crate) fn apply_to(self, scheduler: &mut SchedulerCounters) {
        scheduler.runtime_wakes = self.runtime_wakes;
        scheduler.redraw_requests = self.redraw_requests;
        scheduler.frames_started = self.frames_started;
        scheduler.frames_presented = self.frames_presented;
        scheduler.frames_skipped = self.frames_skipped;
    }
}

/// Backend-neutral application lifecycle. Desktop adapters may only emit a
/// subset; runtime users must therefore treat transitions as advisory rather
/// than assume every state is observable on every platform.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ApplicationLifecycle {
    #[default]
    Starting,
    Active,
    Inactive,
    Suspended,
    Stopping,
    Terminated,
}

/// A lifecycle update normalized by a platform adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LifecycleTransition {
    pub previous: ApplicationLifecycle,
    pub current: ApplicationLifecycle,
}

/// Runtime-level failure report intended for application logging/telemetry
/// hooks. Framework panics never unwind into the native event-loop callback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeErrorReport {
    pub failure: TaskFailure,
}

/// Policy applied after a close accepts the final visible Incular window.
/// `ExitOnLastWindow` is the default so a hidden auxiliary window cannot keep
/// an application alive accidentally. `KeepRunning` is explicit headless or
/// background-service policy; a native adapter remains free to wait idle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LastWindowPolicy {
    #[default]
    ExitOnLastWindow,
    KeepRunning,
}

/// Failure while creating or addressing an Incular-owned window.
#[derive(Debug)]
pub enum WindowError {
    Options(WindowOptionsError),
    Tree(TreeError),
    Restoration(String),
    ApplicationStopped,
}

impl From<WindowOptionsError> for WindowError {
    fn from(error: WindowOptionsError) -> Self {
        Self::Options(error)
    }
}

impl From<TreeError> for WindowError {
    fn from(error: TreeError) -> Self {
        Self::Tree(error)
    }
}

impl std::fmt::Display for WindowError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Options(error) => write!(formatter, "invalid window options: {error}"),
            Self::Tree(error) => write!(formatter, "window tree error: {error}"),
            Self::Restoration(error) => write!(formatter, "invalid restoration window: {error}"),
            Self::ApplicationStopped => formatter.write_str("the Incular application has stopped"),
        }
    }
}

/// Application-defined identity for a window that should survive a restart.
/// It is intentionally unrelated to the session-local generational
/// window ID.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WindowRestorationId(pub(crate) RestorationKey);

impl WindowRestorationId {
    pub fn new(value: impl Into<String>) -> Result<Self, incular_core::RestorationKeyError> {
        RestorationKey::new(value).map(Self)
    }

    #[must_use]
    pub fn as_key(&self) -> &RestorationKey {
        &self.0
    }
}

/// Application-owned factory for a persisted auxiliary window kind. It is
/// registered in memory at startup; it is never serialized.
pub type RestorableWindowFactory = Rc<dyn Fn(&mut BuildContext) -> Widget>;

impl TryFrom<&str> for WindowRestorationId {
    type Error = incular_core::RestorationKeyError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[derive(Clone)]
pub(crate) struct RestorableWindowMetadata {
    pub(crate) id: WindowRestorationId,
    pub(crate) kind: String,
}

impl RestorableWindowMetadata {
    pub(crate) fn new(
        id: WindowRestorationId,
        kind: impl Into<String>,
    ) -> Result<Self, WindowError> {
        let kind = kind.into();
        if kind.trim().is_empty() || kind.chars().any(char::is_control) {
            return Err(WindowError::Restoration(
                "window kind must be a non-empty printable stable identifier".to_owned(),
            ));
        }
        Ok(Self { id, kind })
    }
}

impl std::error::Error for WindowError {}
