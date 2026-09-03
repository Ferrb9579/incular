//! Portable application-shell resource descriptions.
//!
//! These values deliberately contain no native handles. Runtime owns resource
//! identity and callback lifetime; desktop adapters translate immutable
//! snapshots into notification-area/status-bar, notification, and taskbar/Dock
//! APIs at the native boundary.

use crate::{WindowIcon, WindowId};
use std::fmt;

macro_rules! generational_id {
    ($name:ident, $label:literal) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name {
            index: u32,
            generation: u32,
        }

        impl $name {
            #[must_use]
            pub const fn from_parts(index: u32, generation: u32) -> Self {
                Self { index, generation }
            }

            #[must_use]
            pub const fn index(self) -> u32 {
                self.index
            }

            #[must_use]
            pub const fn generation(self) -> u32 {
                self.generation
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(
                    formatter,
                    concat!($label, "Id({}, {})"),
                    self.index, self.generation
                )
            }
        }
    };
}

generational_id!(TrayItemId, "TrayItem");
generational_id!(NotificationId, "Notification");

/// Shell facility requested by an application-scoped operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ApplicationShellFeature {
    TrayOrStatusItem,
    Notifications,
    NotificationActions,
    NotificationUpdate,
    NotificationDismiss,
    TaskbarProgress,
    ApplicationBadge,
    TaskbarOverlayIcon,
}

/// Typed failure from a native application-shell operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationShellError {
    Unsupported(ApplicationShellFeature),
    PermissionDenied,
    PlatformConfigurationRequired(String),
    StaleResource,
    ApplicationStopped,
    NativeFailure(String),
}

impl fmt::Display for ApplicationShellError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(feature) => write!(formatter, "{feature:?} is not supported"),
            Self::PermissionDenied => formatter.write_str("application-shell permission denied"),
            Self::PlatformConfigurationRequired(detail) => {
                write!(formatter, "platform configuration is required: {detail}")
            }
            Self::StaleResource => formatter.write_str("application-shell resource is stale"),
            Self::ApplicationStopped => formatter.write_str("application has stopped"),
            Self::NativeFailure(detail) => {
                write!(formatter, "native shell operation failed: {detail}")
            }
        }
    }
}

impl std::error::Error for ApplicationShellError {}

/// Native presentation for one application-owned tray/status item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayItemPresentation {
    pub icon: Option<WindowIcon>,
    pub tooltip: Option<String>,
    pub title: Option<String>,
    pub visible: bool,
}

impl TrayItemPresentation {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn icon(mut self, icon: WindowIcon) -> Self {
        self.icon = Some(icon);
        self
    }

    #[must_use]
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    #[must_use]
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }
}

impl Default for TrayItemPresentation {
    fn default() -> Self {
        Self {
            icon: None,
            tooltip: None,
            title: None,
            visible: true,
        }
    }
}

/// Stable application-defined identity for a notification action.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NotificationActionId(String);

impl NotificationActionId {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for NotificationActionId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for NotificationActionId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// One optional native notification action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationAction {
    pub id: NotificationActionId,
    pub label: String,
}

impl NotificationAction {
    #[must_use]
    pub fn new(id: impl Into<NotificationActionId>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// Complete desired presentation of one desktop notification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationPresentation {
    pub title: String,
    pub body: String,
    pub icon: Option<WindowIcon>,
    pub actions: Vec<NotificationAction>,
}

impl NotificationPresentation {
    #[must_use]
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            icon: None,
            actions: Vec::new(),
        }
    }

    #[must_use]
    pub fn icon(mut self, icon: WindowIcon) -> Self {
        self.icon = Some(icon);
        self
    }

    #[must_use]
    pub fn action(mut self, action: NotificationAction) -> Self {
        self.actions.push(action);
        self
    }
}

/// Progress state rendered by a taskbar button or application Dock tile.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum TaskbarProgress {
    #[default]
    None,
    Indeterminate,
    Normal(f64),
    Paused(f64),
    Error(f64),
}

impl TaskbarProgress {
    pub fn normal(value: f64) -> Result<Self, TaskbarProgressError> {
        validate_progress(value).map(Self::Normal)
    }

    pub fn paused(value: f64) -> Result<Self, TaskbarProgressError> {
        validate_progress(value).map(Self::Paused)
    }

    pub fn error(value: f64) -> Result<Self, TaskbarProgressError> {
        validate_progress(value).map(Self::Error)
    }

    #[must_use]
    pub const fn fraction(self) -> Option<f64> {
        match self {
            Self::Normal(value) | Self::Paused(value) | Self::Error(value) => Some(value),
            Self::None | Self::Indeterminate => None,
        }
    }
}

fn validate_progress(value: f64) -> Result<f64, TaskbarProgressError> {
    (value.is_finite() && (0.0..=1.0).contains(&value))
        .then_some(value)
        .ok_or(TaskbarProgressError)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskbarProgressError;

impl fmt::Display for TaskbarProgressError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("taskbar progress must be a finite fraction between 0 and 1")
    }
}

impl std::error::Error for TaskbarProgressError {}

/// Badge rendered by a taskbar/Dock integration when the active platform has
/// native badge support.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ApplicationBadge {
    #[default]
    None,
    Count(u64),
    Text(String),
}

/// Desired application shell state. `window_id` is a native taskbar-button
/// target on platforms that require one (notably Windows); application-scoped
/// Dock implementations may ignore it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TaskbarDockState {
    pub window_id: Option<WindowId>,
    pub progress: TaskbarProgress,
    pub badge: ApplicationBadge,
    pub overlay_icon: Option<WindowIcon>,
}

impl TaskbarDockState {
    #[must_use]
    pub fn for_window(window_id: WindowId) -> Self {
        Self {
            window_id: Some(window_id),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn progress(mut self, progress: TaskbarProgress) -> Self {
        self.progress = progress;
        self
    }

    #[must_use]
    pub fn badge(mut self, badge: ApplicationBadge) -> Self {
        self.badge = badge;
        self
    }

    #[must_use]
    pub fn overlay_icon(mut self, icon: WindowIcon) -> Self {
        self.overlay_icon = Some(icon);
        self
    }
}
