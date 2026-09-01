//! Portable display geometry and top-level placement values.
//!
//! Desktop-global coordinates are intentionally distinct from widget/view
//! coordinates. A backend only publishes values that its window system can
//! actually observe; Wayland top-level placement, for example, remains
//! unavailable rather than being synthesized.

use crate::PhysicalSize;
use std::fmt;

/// Generational identity for one display discovered by the active backend.
///
/// Names, enumeration order, and native monitor handles are not identities.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DisplayId {
    index: u32,
    generation: u32,
}

impl DisplayId {
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

impl fmt::Debug for DisplayId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "DisplayId({}, {})", self.index, self.generation)
    }
}

impl fmt::Display for DisplayId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Display#{}@{}", self.index + 1, self.generation)
    }
}

/// Physical desktop-global point in native screen pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PhysicalScreenPosition {
    pub x: i32,
    pub y: i32,
}

/// Physical point relative to the top-left of one known display.
///
/// Unlike [`PhysicalScreenPosition`], this value is meaningful even when the
/// backend cannot expose a desktop-global origin. Converting it to screen space
/// requires a [`DisplaySnapshot::physical_bounds`] value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PhysicalDisplayPosition {
    pub x: i32,
    pub y: i32,
}

impl PhysicalDisplayPosition {
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

impl PhysicalScreenPosition {
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Physical desktop-global rectangle in native screen pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PhysicalScreenRect {
    pub origin: PhysicalScreenPosition,
    pub size: PhysicalSize,
}

impl PhysicalScreenRect {
    #[must_use]
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            origin: PhysicalScreenPosition::new(x, y),
            size: PhysicalSize::new(width, height),
        }
    }

    #[must_use]
    pub fn centered_position(self, outer_size: PhysicalSize) -> PhysicalScreenPosition {
        let x = i64::from(self.origin.x)
            + (i64::from(self.size.width) - i64::from(outer_size.width)) / 2;
        let y = i64::from(self.origin.y)
            + (i64::from(self.size.height) - i64::from(outer_size.height)) / 2;
        PhysicalScreenPosition::new(clamp_i64_to_i32(x), clamp_i64_to_i32(y))
    }

    #[must_use]
    pub fn intersection(self, other: Self) -> Option<Self> {
        let left = i64::from(self.origin.x).max(i64::from(other.origin.x));
        let top = i64::from(self.origin.y).max(i64::from(other.origin.y));
        let right = (i64::from(self.origin.x) + i64::from(self.size.width))
            .min(i64::from(other.origin.x) + i64::from(other.size.width));
        let bottom = (i64::from(self.origin.y) + i64::from(self.size.height))
            .min(i64::from(other.origin.y) + i64::from(other.size.height));
        (right > left && bottom > top).then(|| {
            Self::new(
                clamp_i64_to_i32(left),
                clamp_i64_to_i32(top),
                u32::try_from(right - left).unwrap_or(u32::MAX),
                u32::try_from(bottom - top).unwrap_or(u32::MAX),
            )
        })
    }
}

/// Logical desktop-global point, only published by backends with a coherent
/// logical global coordinate space.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LogicalScreenPosition {
    pub x: f64,
    pub y: f64,
}

/// Logical point relative to one display, independent of desktop-global
/// coordinate availability.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LogicalDisplayPosition {
    pub x: f64,
    pub y: f64,
}

impl LogicalDisplayPosition {
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Logical desktop-global rectangle. Mixed-DPI backends that cannot provide a
/// coherent global logical coordinate space leave this absent rather than
/// dividing a physical global origin by one monitor's scale factor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalScreenRect {
    pub origin: LogicalScreenPosition,
    pub width: f64,
    pub height: f64,
}

/// Immutable portable snapshot for one currently connected display.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplaySnapshot {
    pub id: DisplayId,
    pub name: Option<String>,
    pub scale_factor: f64,
    /// Physical pixel extent even when the window system does not expose a
    /// desktop-global origin (notably Wayland top-level placement).
    pub physical_size: PhysicalSize,
    pub physical_bounds: Option<PhysicalScreenRect>,
    pub logical_bounds: Option<LogicalScreenRect>,
    pub physical_work_area: Option<PhysicalScreenRect>,
    pub logical_work_area: Option<LogicalScreenRect>,
    pub is_primary: bool,
}

impl DisplaySnapshot {
    #[must_use]
    pub fn logical_size(&self) -> Option<(f64, f64)> {
        (self.scale_factor.is_finite() && self.scale_factor > 0.0).then(|| {
            (
                f64::from(self.physical_size.width) / self.scale_factor,
                f64::from(self.physical_size.height) / self.scale_factor,
            )
        })
    }

    #[must_use]
    pub fn placement_rect(&self, area: DisplayPlacementArea) -> Option<PhysicalScreenRect> {
        match area {
            DisplayPlacementArea::WorkArea => self.physical_work_area,
            DisplayPlacementArea::FullBounds => self.physical_bounds,
        }
    }

    /// Converts display-local logical coordinates into display-local physical
    /// coordinates. No desktop-global origin is involved, so this remains
    /// well-defined on mixed-DPI desktops and Wayland.
    #[must_use]
    pub fn logical_to_physical_local(
        &self,
        position: LogicalDisplayPosition,
    ) -> Option<PhysicalDisplayPosition> {
        if !self.scale_factor.is_finite()
            || self.scale_factor <= 0.0
            || !position.x.is_finite()
            || !position.y.is_finite()
        {
            return None;
        }
        let x = position.x * self.scale_factor;
        let y = position.y * self.scale_factor;
        Some(PhysicalDisplayPosition::new(
            rounded_f64_to_i32(x)?,
            rounded_f64_to_i32(y)?,
        ))
    }

    /// Converts display-local physical coordinates into logical coordinates.
    #[must_use]
    pub fn physical_to_logical_local(
        &self,
        position: PhysicalDisplayPosition,
    ) -> Option<LogicalDisplayPosition> {
        (self.scale_factor.is_finite() && self.scale_factor > 0.0).then(|| {
            LogicalDisplayPosition::new(
                f64::from(position.x) / self.scale_factor,
                f64::from(position.y) / self.scale_factor,
            )
        })
    }

    /// Resolves a display-local physical point into desktop-global physical
    /// coordinates when this backend actually exposes a global display origin.
    #[must_use]
    pub fn screen_position(
        &self,
        position: PhysicalDisplayPosition,
    ) -> Option<PhysicalScreenPosition> {
        let bounds = self.physical_bounds?;
        Some(PhysicalScreenPosition::new(
            checked_i64_to_i32(i64::from(bounds.origin.x) + i64::from(position.x))?,
            checked_i64_to_i32(i64::from(bounds.origin.y) + i64::from(position.y))?,
        ))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DisplayPlacementArea {
    #[default]
    WorkArea,
    FullBounds,
}

const fn clamp_i64_to_i32(value: i64) -> i32 {
    if value < i32::MIN as i64 {
        i32::MIN
    } else if value > i32::MAX as i64 {
        i32::MAX
    } else {
        value as i32
    }
}

fn checked_i64_to_i32(value: i64) -> Option<i32> {
    i32::try_from(value).ok()
}

fn rounded_f64_to_i32(value: f64) -> Option<i32> {
    if !value.is_finite() || value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
        return None;
    }
    Some(value.round() as i32)
}
