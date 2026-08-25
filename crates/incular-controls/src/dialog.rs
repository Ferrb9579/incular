//! Dialog anatomy. Behavior is shared with the popup/overlay foundation.
pub use crate::popup::{
    Arrow, Close, Description, Popup, Portal, Positioner, Root, Title, Trigger,
};

/// Dialog anatomy names for applications that prefer the full compound API.
pub type Backdrop = Portal;
pub type Viewport = Popup;

/// Alert dialog uses the same retained dialog state with a stricter semantic
/// contract supplied by the application shell.
pub type AlertRoot = crate::alert_dialog::Root;
