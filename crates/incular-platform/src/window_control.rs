//! Portable window-control values shared by runtime and native adapters.

use crate::{DisplayId, Fullscreen, PhysicalScreenPosition, PhysicalSize};
use incular_core::Size;
use std::{fmt, sync::Arc};

/// Portable z-level hints for a desktop top-level window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WindowLevel {
    #[default]
    Normal,
    AlwaysOnTop,
}

/// Strength of a desktop user-attention request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UserAttentionType {
    #[default]
    Informational,
    Critical,
}

/// Validated RGBA8 window-icon pixels.
///
/// The byte storage is shared so cloning `WindowOptions`, requests, and retained
/// state does not duplicate image data. Platform adapters create their native
/// icon object only at the native boundary.
#[derive(Clone, PartialEq, Eq)]
pub struct WindowIcon {
    rgba: Arc<[u8]>,
    width: u32,
    height: u32,
}

impl WindowIcon {
    pub fn from_rgba(
        rgba: impl Into<Vec<u8>>,
        width: u32,
        height: u32,
    ) -> Result<Self, WindowIconError> {
        let rgba = rgba.into();
        if width == 0 || height == 0 {
            return Err(WindowIconError::ZeroDimension);
        }
        let pixels = usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(WindowIconError::DimensionsOverflow)?;
        if rgba.len() != pixels {
            return Err(WindowIconError::InvalidByteCount {
                expected: pixels,
                actual: rgba.len(),
            });
        }
        Ok(Self {
            rgba: rgba.into(),
            width,
            height,
        })
    }

    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }
}

impl fmt::Debug for WindowIcon {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WindowIcon")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("rgba_bytes", &self.rgba.len())
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WindowIconError {
    ZeroDimension,
    DimensionsOverflow,
    InvalidByteCount { expected: usize, actual: usize },
}

impl fmt::Display for WindowIconError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDimension => formatter.write_str("window icon dimensions must be non-zero"),
            Self::DimensionsOverflow => {
                formatter.write_str("window icon dimensions overflow addressable memory")
            }
            Self::InvalidByteCount { expected, actual } => write!(
                formatter,
                "window icon RGBA byte count is {actual}, expected {expected}"
            ),
        }
    }
}

impl std::error::Error for WindowIconError {}

/// Validated dynamic logical-size constraints for a native window.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LogicalSizeLimits {
    minimum: Option<Size>,
    maximum: Option<Size>,
}

impl LogicalSizeLimits {
    pub fn new(
        minimum: Option<Size>,
        maximum: Option<Size>,
    ) -> Result<Self, LogicalSizeLimitsError> {
        if minimum.is_some_and(|size| !is_positive_size(size)) {
            return Err(LogicalSizeLimitsError::InvalidMinimum);
        }
        if maximum.is_some_and(|size| !is_positive_size(size)) {
            return Err(LogicalSizeLimitsError::InvalidMaximum);
        }
        if let (Some(minimum), Some(maximum)) = (minimum, maximum)
            && (minimum.width > maximum.width || minimum.height > maximum.height)
        {
            return Err(LogicalSizeLimitsError::MinimumExceedsMaximum);
        }
        Ok(Self { minimum, maximum })
    }

    #[must_use]
    pub const fn minimum(self) -> Option<Size> {
        self.minimum
    }

    #[must_use]
    pub const fn maximum(self) -> Option<Size> {
        self.maximum
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogicalSizeLimitsError {
    InvalidMinimum,
    InvalidMaximum,
    MinimumExceedsMaximum,
}

impl fmt::Display for LogicalSizeLimitsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidMinimum => "minimum logical size must be finite and positive",
            Self::InvalidMaximum => "maximum logical size must be finite and positive",
            Self::MinimumExceedsMaximum => {
                "minimum logical size cannot exceed maximum logical size"
            }
        })
    }
}

impl std::error::Error for LogicalSizeLimitsError {}

/// Application-requested mutable top-level state.
///
/// This is intentionally distinct from [`WindowObservedState`]. A setter only
/// changes requested state; the native backend publishes what the OS actually
/// reports separately.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowRequestedState {
    pub visible: bool,
    pub minimized: bool,
    pub maximized: bool,
    pub fullscreen: Option<Fullscreen>,
    pub resizable: bool,
    pub decorations: bool,
    pub size_limits: LogicalSizeLimits,
    pub level: WindowLevel,
}

/// Native state that can be queried reliably by the active backend.
///
/// `None` means the backend/session does not expose that observation; it does
/// not mean false.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WindowObservedState {
    pub visible: Option<bool>,
    pub minimized: Option<bool>,
    pub maximized: Option<bool>,
    pub fullscreen: Option<bool>,
    pub resizable: Option<bool>,
    pub decorations: Option<bool>,
    pub outer_position: Option<PhysicalScreenPosition>,
    pub outer_size: Option<PhysicalSize>,
    pub current_display: Option<DisplayId>,
}

const fn is_positive_size(size: Size) -> bool {
    size.width.is_finite() && size.height.is_finite() && size.width > 0.0 && size.height > 0.0
}
