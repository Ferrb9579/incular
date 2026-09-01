//! Semantic transient presentation shared by menus, popovers, tooltips, and
//! other anchored UI.
//!
//! A transient is logically owned by the widget/window that opened it, but its
//! presentation is independent from top-level window sizing. Desktop backends
//! may lift it to a native popup surface; constrained hosts may keep it in the
//! owning view's overlay.

use incular_config::{TransientPresentation, TransientRole};
use incular_core::{Offset, Rect, Size};

/// Stable identity of a mounted transient portal. The identity is owned by
/// the retained tree, not by application code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TransientSurfaceId {
    index: u32,
    generation: u32,
}

impl TransientSurfaceId {
    #[doc(hidden)]
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

/// Retained, platform-neutral description of one visible transient.
///
/// Geometry is expressed in the owning view's logical coordinate space. A
/// native backend combines it with the parent surface's current desktop
/// position/scale when presenting the popup; therefore moving the parent does
/// not mutate widget layout or top-level content size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransientSurfaceSnapshot {
    pub id: TransientSurfaceId,
    pub role: TransientRole,
    pub presentation: TransientPresentation,
    pub anchor_rect: Rect,
    pub content_rect: Rect,
}

impl TransientSurfaceSnapshot {
    #[must_use]
    pub const fn content_size(self) -> Size {
        self.content_rect.size
    }

    #[must_use]
    pub const fn content_offset(self) -> Offset {
        self.content_rect.origin
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TransientPortalMarker {
    pub role: TransientRole,
    pub presentation: TransientPresentation,
    pub show: bool,
    pub popup_child_index: usize,
}
