use std::fmt;

use incular_core::{PointerDeviceKind, PointerPhase};

/// Portable metadata attached to one native pointer sample before its physical
/// position is normalized into logical window coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerMetadata {
    pub pointer: u64,
    pub device: u64,
    pub kind: PointerDeviceKind,
    pub buttons: u32,
    pub button: Option<u32>,
    pub phase: PointerPhase,
}

/// Native cursor grab policy for one window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CursorGrabMode {
    /// Release any pointer confinement or lock owned by the window.
    #[default]
    None,
    /// Keep the pointer inside the window while preserving absolute motion.
    Confined,
    /// Lock the pointer for relative-motion style interaction.
    Locked,
}

/// Finite logical cursor position relative to a window's client area.
///
/// The validated wrapper prevents NaN/infinity from reaching native APIs whose
/// integer/physical conversion behavior is platform-dependent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalWindowPosition {
    x: f64,
    y: f64,
}

impl LogicalWindowPosition {
    pub fn new(x: f64, y: f64) -> Result<Self, LogicalWindowPositionError> {
        if !x.is_finite() || !y.is_finite() {
            return Err(LogicalWindowPositionError);
        }
        Ok(Self { x, y })
    }

    #[must_use]
    pub const fn x(self) -> f64 {
        self.x
    }

    #[must_use]
    pub const fn y(self) -> f64 {
        self.y
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LogicalWindowPositionError;

impl fmt::Display for LogicalWindowPositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("logical window position must contain finite coordinates")
    }
}

impl std::error::Error for LogicalWindowPositionError {}
