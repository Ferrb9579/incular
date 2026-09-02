//! Shared popup/portal descriptors. The rendering layer remains owned by
//! `incular-widgets`; controls only describe composition and positioning.

use incular_config::EdgeInsets;
use incular_core::Offset;
use incular_widgets::internal::TransientPlacementOverride;
use incular_widgets::{
    OverlayPortal as RawOverlayPortal, TransientAlignment, TransientPlacement,
    TransientPresentation, TransientRole, TransientSide, Widget,
};

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
    presentation: TransientPresentation,
    role: TransientRole,
}
impl OverlayPortal {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            overlay: None,
            open: false,
            presentation: TransientPresentation::Auto,
            role: TransientRole::Popover,
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

    #[must_use]
    pub fn presentation(mut self, value: TransientPresentation) -> Self {
        self.presentation = value;
        self
    }

    #[must_use]
    pub fn role(mut self, value: TransientRole) -> Self {
        self.role = value;
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
            .presentation(value.presentation)
            .role(value.role)
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
    anchor: Option<Offset>,
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
            anchor: None,
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
        self.anchor = Some(value);
        self
    }

    /// Converts the controls-layer vocabulary into Incular's authoritative
    /// transient placement policy. Collision handling itself is owned solely by
    /// `incular-widgets`.
    #[must_use]
    pub fn placement(&self) -> TransientPlacement {
        let side = match self.side {
            Side::Top => TransientSide::Top,
            Side::Left => TransientSide::Left,
            Side::Bottom => TransientSide::Bottom,
            Side::Right => TransientSide::Right,
        };
        let alignment = match self.align {
            Align::Start => TransientAlignment::Start,
            Align::Center => TransientAlignment::Center,
            Align::End => TransientAlignment::End,
        };
        let alignment_offset = match side {
            TransientSide::Top | TransientSide::Bottom => Offset::new(self.align_offset, 0.0),
            TransientSide::Left | TransientSide::Right => Offset::new(0.0, self.align_offset),
        };
        TransientPlacement::new()
            .side(side)
            .alignment(alignment)
            .side_offset(self.side_offset)
            .alignment_offset(alignment_offset)
            .safe_margin(self.collision_padding)
    }
}
impl From<AnchoredPositioner> for Widget {
    fn from(value: AnchoredPositioner) -> Self {
        let mut override_ = TransientPlacementOverride::new(value.placement());
        if let Some(anchor) = value.anchor {
            override_ = override_.anchor_point(anchor);
        }
        Widget::environment_scope(override_, value.child)
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
