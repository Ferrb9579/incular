//! Shared popup/portal descriptors. The rendering layer remains owned by
//! `incular-widgets`; controls only describe composition and positioning.

use incular_config::EdgeInsets;
use incular_core::Offset;
use incular_widgets::{OverlayPortal as RawOverlayPortal, Positioned, Widget};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Side {
    Top,
    Left,
    #[default]
    Bottom,
    Right,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    Start,
    #[default]
    Center,
    End,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TransitionStatus {
    Starting,
    #[default]
    Open,
    Ending,
    Closed,
}

#[derive(Clone)]
pub struct OverlayPortal {
    child: Widget,
    overlay: Option<Widget>,
    open: bool,
}
impl OverlayPortal {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            overlay: None,
            open: false,
        }
    }
    #[must_use]
    pub fn overlay(mut self, value: impl Into<Widget>) -> Self {
        self.overlay = Some(value.into());
        self
    }
    #[must_use]
    pub fn open(mut self, value: bool) -> Self {
        self.open = value;
        self
    }
}
impl From<OverlayPortal> for Widget {
    fn from(value: OverlayPortal) -> Self {
        RawOverlayPortal::new(value.child)
            .overlay_child(
                value
                    .overlay
                    .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into()),
            )
            .show(value.open)
            .into()
    }
}

#[derive(Clone)]
pub struct AnchoredPositioner {
    child: Widget,
    side: Side,
    align: Align,
    side_offset: f32,
    align_offset: f32,
    collision_padding: EdgeInsets,
    anchor: Offset,
}
impl AnchoredPositioner {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            side: Side::Bottom,
            align: Align::Center,
            side_offset: 4.,
            align_offset: 0.,
            collision_padding: EdgeInsets::all(8.),
            anchor: Offset::ZERO,
        }
    }
    #[must_use]
    pub fn side(mut self, value: Side) -> Self {
        self.side = value;
        self
    }
    #[must_use]
    pub fn align(mut self, value: Align) -> Self {
        self.align = value;
        self
    }
    #[must_use]
    pub fn side_offset(mut self, value: f32) -> Self {
        self.side_offset = value;
        self
    }
    #[must_use]
    pub fn align_offset(mut self, value: f32) -> Self {
        self.align_offset = value;
        self
    }
    #[must_use]
    pub fn collision_padding(mut self, value: EdgeInsets) -> Self {
        self.collision_padding = value;
        self
    }

    /// Sets the anchor origin used by the retained fallback positioner. A
    /// native window host can update this value when the anchor moves.
    #[must_use]
    pub fn anchor(mut self, value: Offset) -> Self {
        self.anchor = value;
        self
    }
    /// The native layout engine takes the final anchor rectangle. This helper
    /// is intentionally deterministic and reusable by popup implementations.
    #[must_use]
    pub fn offset(&self, anchor: Offset) -> Offset {
        match self.side {
            Side::Top => Offset::new(anchor.x + self.align_offset, anchor.y - self.side_offset),
            Side::Left => Offset::new(anchor.x - self.side_offset, anchor.y + self.align_offset),
            Side::Bottom => Offset::new(anchor.x + self.align_offset, anchor.y + self.side_offset),
            Side::Right => Offset::new(anchor.x + self.side_offset, anchor.y + self.align_offset),
        }
    }
}
impl From<AnchoredPositioner> for Widget {
    fn from(value: AnchoredPositioner) -> Self {
        let origin = value.offset(value.anchor);
        Positioned::new(value.child)
            .left(origin.x.max(0.))
            .top(origin.y.max(0.))
            .into()
    }
}

#[derive(Clone)]
pub struct PopupLayer {
    child: Widget,
}
impl PopupLayer {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<PopupLayer> for Widget {
    fn from(value: PopupLayer) -> Self {
        value.child
    }
}

#[derive(Clone)]
pub struct DismissLayer {
    child: Widget,
    enabled: bool,
}
impl DismissLayer {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            enabled: true,
        }
    }
    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }
}
impl From<DismissLayer> for Widget {
    fn from(value: DismissLayer) -> Self {
        value.child
    }
}

#[derive(Clone)]
pub struct FocusTrap {
    child: Widget,
    enabled: bool,
}
impl FocusTrap {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            enabled: true,
        }
    }
    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }
}
impl From<FocusTrap> for Widget {
    fn from(value: FocusTrap) -> Self {
        if value.enabled {
            incular_widgets::FocusScope::new(value.child).into()
        } else {
            value.child
        }
    }
}
