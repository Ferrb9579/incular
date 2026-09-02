//! Semantic transient presentation shared by menus, popovers, tooltips, and
//! other anchored UI.
//!
//! A transient is logically owned by the widget/window that opened it, but its
//! presentation is independent from top-level window sizing. Desktop backends
//! may lift it to a native popup surface; constrained hosts may keep it in the
//! owning view's overlay.

use incular_config::{EdgeInsets, TextDirection, TransientPresentation, TransientRole};
use incular_core::{Offset, Rect, Size};
use incular_rendering::SurfacePartitionId;
use std::rc::Rc;

/// Physical side of an anchor used by the platform-neutral transient placement
/// engine. Directional submenu defaults are resolved to one of these sides
/// before collision handling begins.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TransientSide {
    Top,
    Right,
    #[default]
    Bottom,
    Left,
}

impl TransientSide {
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Right => Self::Left,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
        }
    }
}

/// Alignment along the cross axis of an anchored transient.
///
/// `Start`/`End` are text-direction aware for top/bottom placement. For
/// left/right placement they mean the physical top/bottom edges respectively.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TransientAlignment {
    Start,
    #[default]
    Center,
    End,
}

/// Placement intent that changes role defaults without creating a second
/// placement implementation. Submenus prefer the outward inline direction;
/// ordinary anchored surfaces use their role's normal side.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TransientPlacementMode {
    #[default]
    Anchored,
    Submenu,
}

/// Reusable placement policy carried by a retained transient portal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransientPlacement {
    pub preferred_side: Option<TransientSide>,
    pub alignment: TransientAlignment,
    pub mode: TransientPlacementMode,
    /// Gap between the anchor edge and popup on the primary placement axis.
    pub side_offset: f32,
    /// Final view-local logical offset applied after anchor alignment.
    pub alignment_offset: Offset,
    /// Collision margin inside the available viewport/work area.
    pub safe_margin: EdgeInsets,
}

impl Default for TransientPlacement {
    fn default() -> Self {
        Self {
            preferred_side: None,
            alignment: TransientAlignment::Start,
            mode: TransientPlacementMode::Anchored,
            side_offset: 0.0,
            alignment_offset: Offset::ZERO,
            safe_margin: EdgeInsets::all(8.0),
        }
    }
}

impl TransientPlacement {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            preferred_side: None,
            alignment: TransientAlignment::Start,
            mode: TransientPlacementMode::Anchored,
            side_offset: 0.0,
            alignment_offset: Offset::ZERO,
            safe_margin: EdgeInsets::all(8.0),
        }
    }

    #[must_use]
    pub const fn side(mut self, side: TransientSide) -> Self {
        self.preferred_side = Some(side);
        self
    }

    #[must_use]
    pub const fn alignment(mut self, alignment: TransientAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    #[must_use]
    pub const fn submenu(mut self, submenu: bool) -> Self {
        self.mode = if submenu {
            TransientPlacementMode::Submenu
        } else {
            TransientPlacementMode::Anchored
        };
        self
    }

    #[must_use]
    pub fn side_offset(mut self, offset: f32) -> Self {
        self.side_offset = finite_or_zero(offset);
        self
    }

    #[must_use]
    pub fn alignment_offset(mut self, offset: Offset) -> Self {
        self.alignment_offset = Offset::new(finite_or_zero(offset.x), finite_or_zero(offset.y));
        self
    }

    #[must_use]
    pub fn safe_margin(mut self, margin: EdgeInsets) -> Self {
        self.safe_margin = margin.normalized();
        self
    }
}

/// Metadata produced by higher-level popup `Positioner` descriptors.
///
/// This is intentionally exposed only through `incular_widgets::internal`.
/// The retained `OverlayPortal` consumes it before placement so controls can
/// describe side/alignment without introducing a second positioning engine.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransientPlacementOverride {
    pub placement: TransientPlacement,
    pub anchor_override: Option<Rect>,
}

impl TransientPlacementOverride {
    #[must_use]
    pub const fn new(placement: TransientPlacement) -> Self {
        Self {
            placement,
            anchor_override: None,
        }
    }

    #[must_use]
    pub fn anchor_point(mut self, point: Offset) -> Self {
        self.anchor_override = Some(Rect::from_origin_size(point, Size::ZERO));
        self
    }
}

/// Pure input to [`place_transient`]. All geometry must use the same coordinate
/// space and unit. Incular's desktop adapter converts native work-area geometry
/// into the owning view's logical coordinate space before calling this policy,
/// so native and overlay presentation consume identical inputs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransientPlacementInput {
    pub anchor_rect: Rect,
    pub desired_size: Size,
    pub available_rect: Rect,
    pub role: TransientRole,
    pub text_direction: TextDirection,
    pub placement: TransientPlacement,
}

/// Deterministic result of the shared `flip -> shift -> constrain` policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransientPlacementResult {
    pub rect: Rect,
    pub side: TransientSide,
    pub alignment: TransientAlignment,
    pub flipped: bool,
    pub shifted: bool,
    pub constrained: bool,
}

/// Framework-level reason a transient requested dismissal. Native focus is not
/// encoded here; adapters normalize platform events into these semantic causes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransientDismissReason {
    OutsidePointer,
    Escape,
    FocusLost,
    ParentDeactivated,
    ExplicitSelection,
}

/// Role-independent dismissal switches carried by a retained transient.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransientDismissPolicy {
    pub outside_pointer: bool,
    pub escape: bool,
    pub focus_loss: bool,
    pub parent_deactivation: bool,
}

impl TransientDismissPolicy {
    #[must_use]
    pub const fn interactive() -> Self {
        Self {
            outside_pointer: true,
            escape: true,
            focus_loss: true,
            parent_deactivation: true,
        }
    }

    #[must_use]
    pub const fn tooltip() -> Self {
        Self {
            outside_pointer: false,
            escape: false,
            focus_loss: false,
            parent_deactivation: true,
        }
    }

    #[must_use]
    pub const fn for_role(role: TransientRole) -> Self {
        match role {
            TransientRole::Tooltip => Self::tooltip(),
            TransientRole::Popover
            | TransientRole::Menu
            | TransientRole::ContextMenu
            | TransientRole::ComboBox => Self::interactive(),
        }
    }

    #[must_use]
    pub const fn allows(self, reason: TransientDismissReason) -> bool {
        match reason {
            TransientDismissReason::OutsidePointer => self.outside_pointer,
            TransientDismissReason::Escape => self.escape,
            TransientDismissReason::FocusLost => self.focus_loss,
            TransientDismissReason::ParentDeactivated => self.parent_deactivation,
            TransientDismissReason::ExplicitSelection => true,
        }
    }
}

/// Places an anchored transient without consulting renderer or platform state.
///
/// The preferred side is flipped only when the opposite side has strictly less
/// primary-axis overflow. Cross-axis overflow is then shifted into the safe
/// area. Finally the popup is constrained to the safe area while preserving the
/// chosen anchor edge.
#[must_use]
pub fn place_transient(input: TransientPlacementInput) -> TransientPlacementResult {
    let placement = normalized_placement(input.placement);
    let available = inset_available(input.available_rect, placement.safe_margin);
    let anchor = normalized_rect(input.anchor_rect);
    let desired = normalized_size(input.desired_size);
    let preferred = placement
        .preferred_side
        .unwrap_or_else(|| default_side(input.role, placement.mode, input.text_direction));
    let preferred_origin = aligned_origin(
        anchor,
        desired,
        preferred,
        placement.alignment,
        input.text_direction,
        placement.side_offset,
        placement.alignment_offset,
    );
    let opposite = preferred.opposite();
    let opposite_origin = aligned_origin(
        anchor,
        desired,
        opposite,
        placement.alignment,
        input.text_direction,
        placement.side_offset,
        placement.alignment_offset,
    );
    let preferred_overflow = primary_overflow(preferred, preferred_origin, desired, available);
    let opposite_overflow = primary_overflow(opposite, opposite_origin, desired, available);
    let flipped = opposite_overflow < preferred_overflow;
    let side = if flipped { opposite } else { preferred };
    let mut origin = if flipped {
        opposite_origin
    } else {
        preferred_origin
    };

    let shifted_origin = shift_cross_axis(origin, desired, side, available);
    let mut shifted = shifted_origin != origin;
    origin = shifted_origin;

    let final_size = Size::new(
        desired.width.min(available.size.width.max(0.0)),
        desired.height.min(available.size.height.max(0.0)),
    );
    let constrained = final_size != desired;
    if constrained {
        // Re-anchor the changed primary dimension, then clamp both axes. This
        // is part of the constrain stage rather than a second placement pass.
        origin = aligned_origin(
            anchor,
            final_size,
            side,
            placement.alignment,
            input.text_direction,
            placement.side_offset,
            placement.alignment_offset,
        );
        let unclamped = origin;
        origin = clamp_origin(origin, final_size, available);
        shifted |= origin != unclamped;
    } else {
        let unclamped = origin;
        origin = clamp_primary_axis(origin, final_size, side, available);
        shifted |= origin != unclamped;
    }

    TransientPlacementResult {
        rect: Rect::from_origin_size(origin, final_size),
        side,
        alignment: placement.alignment,
        flipped,
        shifted,
        constrained,
    }
}

fn normalized_placement(mut placement: TransientPlacement) -> TransientPlacement {
    placement.side_offset = finite_or_zero(placement.side_offset);
    placement.alignment_offset = Offset::new(
        finite_or_zero(placement.alignment_offset.x),
        finite_or_zero(placement.alignment_offset.y),
    );
    placement.safe_margin = placement.safe_margin.normalized();
    placement
}

fn default_side(
    role: TransientRole,
    mode: TransientPlacementMode,
    direction: TextDirection,
) -> TransientSide {
    if mode == TransientPlacementMode::Submenu {
        return match direction {
            TextDirection::Ltr => TransientSide::Right,
            TextDirection::Rtl => TransientSide::Left,
        };
    }
    match role {
        TransientRole::Popover
        | TransientRole::Menu
        | TransientRole::ContextMenu
        | TransientRole::ComboBox
        | TransientRole::Tooltip => TransientSide::Bottom,
    }
}

fn normalized_size(size: Size) -> Size {
    Size::new(
        nonnegative_finite(size.width),
        nonnegative_finite(size.height),
    )
}

fn normalized_rect(rect: Rect) -> Rect {
    Rect::from_origin_size(
        Offset::new(finite_or_zero(rect.origin.x), finite_or_zero(rect.origin.y)),
        normalized_size(rect.size),
    )
}

fn inset_available(rect: Rect, margin: EdgeInsets) -> Rect {
    let origin = Offset::new(finite_or_zero(rect.origin.x), finite_or_zero(rect.origin.y));
    let size = normalized_size(rect.size);
    let left = nonnegative_finite(margin.left).min(size.width);
    let top = nonnegative_finite(margin.top).min(size.height);
    let right = nonnegative_finite(margin.right).min((size.width - left).max(0.0));
    let bottom = nonnegative_finite(margin.bottom).min((size.height - top).max(0.0));
    Rect::from_origin_size(
        origin + Offset::new(left, top),
        Size::new(
            (size.width - left - right).max(0.0),
            (size.height - top - bottom).max(0.0),
        ),
    )
}

fn aligned_origin(
    anchor: Rect,
    popup: Size,
    side: TransientSide,
    alignment: TransientAlignment,
    direction: TextDirection,
    side_offset: f32,
    offset: Offset,
) -> Offset {
    let anchor_right = anchor.origin.x + anchor.size.width;
    let anchor_bottom = anchor.origin.y + anchor.size.height;
    let x_alignment = || match alignment {
        TransientAlignment::Center => anchor.origin.x + (anchor.size.width - popup.width) * 0.5,
        TransientAlignment::Start => match direction {
            TextDirection::Ltr => anchor.origin.x,
            TextDirection::Rtl => anchor_right - popup.width,
        },
        TransientAlignment::End => match direction {
            TextDirection::Ltr => anchor_right - popup.width,
            TextDirection::Rtl => anchor.origin.x,
        },
    };
    let y_alignment = || match alignment {
        TransientAlignment::Start => anchor.origin.y,
        TransientAlignment::Center => anchor.origin.y + (anchor.size.height - popup.height) * 0.5,
        TransientAlignment::End => anchor_bottom - popup.height,
    };
    let origin = match side {
        TransientSide::Top => {
            Offset::new(x_alignment(), anchor.origin.y - popup.height - side_offset)
        }
        TransientSide::Right => Offset::new(anchor_right + side_offset, y_alignment()),
        TransientSide::Bottom => Offset::new(x_alignment(), anchor_bottom + side_offset),
        TransientSide::Left => {
            Offset::new(anchor.origin.x - popup.width - side_offset, y_alignment())
        }
    };
    origin + offset
}

fn primary_overflow(side: TransientSide, origin: Offset, size: Size, available: Rect) -> f32 {
    let right = available.origin.x + available.size.width;
    let bottom = available.origin.y + available.size.height;
    match side {
        TransientSide::Top => (available.origin.y - origin.y).max(0.0),
        TransientSide::Right => (origin.x + size.width - right).max(0.0),
        TransientSide::Bottom => (origin.y + size.height - bottom).max(0.0),
        TransientSide::Left => (available.origin.x - origin.x).max(0.0),
    }
}

fn shift_cross_axis(
    mut origin: Offset,
    size: Size,
    side: TransientSide,
    available: Rect,
) -> Offset {
    match side {
        TransientSide::Top | TransientSide::Bottom => {
            origin.x = clamp_axis(
                origin.x,
                size.width,
                available.origin.x,
                available.size.width,
            );
        }
        TransientSide::Left | TransientSide::Right => {
            origin.y = clamp_axis(
                origin.y,
                size.height,
                available.origin.y,
                available.size.height,
            );
        }
    }
    origin
}

fn clamp_primary_axis(
    mut origin: Offset,
    size: Size,
    side: TransientSide,
    available: Rect,
) -> Offset {
    match side {
        TransientSide::Top | TransientSide::Bottom => {
            origin.y = clamp_axis(
                origin.y,
                size.height,
                available.origin.y,
                available.size.height,
            );
        }
        TransientSide::Left | TransientSide::Right => {
            origin.x = clamp_axis(
                origin.x,
                size.width,
                available.origin.x,
                available.size.width,
            );
        }
    }
    origin
}

fn clamp_origin(mut origin: Offset, size: Size, available: Rect) -> Offset {
    origin.x = clamp_axis(
        origin.x,
        size.width,
        available.origin.x,
        available.size.width,
    );
    origin.y = clamp_axis(
        origin.y,
        size.height,
        available.origin.y,
        available.size.height,
    );
    origin
}

fn clamp_axis(origin: f32, extent: f32, available_origin: f32, available_extent: f32) -> f32 {
    let max = available_origin + (available_extent - extent).max(0.0);
    origin.clamp(available_origin, max)
}

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

fn nonnegative_finite(value: f32) -> f32 {
    finite_or_zero(value).max(0.0)
}

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

    #[doc(hidden)]
    #[must_use]
    pub const fn surface_partition(self) -> SurfacePartitionId {
        SurfacePartitionId::from_parts(self.index, self.generation)
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
    /// Nearest visible retained transient ancestor, used to preserve popup/menu
    /// chains across native surface boundaries.
    pub parent: Option<TransientSurfaceId>,
    pub role: TransientRole,
    pub presentation: TransientPresentation,
    pub anchor_rect: Rect,
    /// Unconstrained retained popup size before collision policy is applied.
    pub desired_size: Size,
    pub placement: TransientPlacement,
    pub text_direction: TextDirection,
    pub placement_result: TransientPlacementResult,
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

#[derive(Clone)]
pub(crate) struct TransientPortalMarker {
    pub role: TransientRole,
    pub presentation: TransientPresentation,
    pub placement: TransientPlacement,
    pub anchor_override: Option<Rect>,
    pub dismiss_policy: TransientDismissPolicy,
    pub on_dismiss: Option<Rc<dyn Fn(TransientDismissReason) + 'static>>,
    pub show: bool,
    pub anchor_child_index: usize,
    pub popup_child_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RetainedTransientPlacement {
    pub desired_size: Size,
    pub result: TransientPlacementResult,
}
