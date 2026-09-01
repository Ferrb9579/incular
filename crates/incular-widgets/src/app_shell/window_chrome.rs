//! Retained hit regions for native custom window chrome.
//!
//! These widgets carry no native handle and perform no manual desktop geometry.
//! Runtime resolves a primary-button press and asks the owning platform backend
//! to begin the compositor/window-manager-native move or resize operation.

use crate::raw_input::RawInputKind;
use crate::{Widget, WidgetKind};
use incular_core::WindowResizeDirection;

/// Transparent region that starts a native window move when the user presses
/// unhandled space within it.
///
/// Interactive descendants such as buttons, text fields, or gesture detectors
/// take precedence, so a titlebar can wrap its whole visual row without making
/// window-control buttons draggable.
#[derive(Clone, Debug)]
pub struct WindowDragRegion {
    child: Widget,
}

impl WindowDragRegion {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<WindowDragRegion> for Widget {
    fn from(value: WindowDragRegion) -> Self {
        Widget::from_kind(WidgetKind::RawInput {
            kind: RawInputKind::WindowDragRegion,
            child: Some(value.child),
        })
    }
}

/// Transparent region that starts a native edge/corner resize gesture.
#[derive(Clone, Debug)]
pub struct WindowResizeRegion {
    direction: WindowResizeDirection,
    child: Widget,
}

impl WindowResizeRegion {
    #[must_use]
    pub fn new(direction: WindowResizeDirection, child: impl Into<Widget>) -> Self {
        Self {
            direction,
            child: child.into(),
        }
    }

    #[must_use]
    pub const fn direction(&self) -> WindowResizeDirection {
        self.direction
    }
}

impl From<WindowResizeRegion> for Widget {
    fn from(value: WindowResizeRegion) -> Self {
        Widget::from_kind(WidgetKind::RawInput {
            kind: RawInputKind::WindowResizeRegion {
                direction: value.direction,
            },
            child: Some(value.child),
        })
    }
}
