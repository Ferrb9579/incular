use incular_core::{Offset, Size};

/// Direction in which a linear layout consumes its main axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Axis {
    #[default]
    Horizontal,
    Vertical,
}

impl Axis {
    #[must_use]
    pub const fn is_horizontal(self) -> bool {
        matches!(self, Self::Horizontal)
    }

    #[must_use]
    pub const fn is_vertical(self) -> bool {
        matches!(self, Self::Vertical)
    }

    #[must_use]
    pub const fn main_extent(self, size: Size) -> f32 {
        match self {
            Self::Horizontal => size.width,
            Self::Vertical => size.height,
        }
    }

    #[must_use]
    pub const fn cross_extent(self, size: Size) -> f32 {
        match self {
            Self::Horizontal => size.height,
            Self::Vertical => size.width,
        }
    }

    #[must_use]
    pub fn size(self, main: f32, cross: f32) -> Size {
        match self {
            Self::Horizontal => Size::new(main, cross),
            Self::Vertical => Size::new(cross, main),
        }
    }

    #[must_use]
    pub const fn offset(self, main: f32, cross: f32) -> Offset {
        match self {
            Self::Horizontal => Offset::new(main, cross),
            Self::Vertical => Offset::new(cross, main),
        }
    }
}

/// Physical direction used by directional layout policies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AxisDirection {
    #[default]
    Right,
    Left,
    Down,
    Up,
}

impl AxisDirection {
    #[must_use]
    pub const fn axis(self) -> Axis {
        match self {
            Self::Right | Self::Left => Axis::Horizontal,
            Self::Down | Self::Up => Axis::Vertical,
        }
    }

    #[must_use]
    pub const fn is_reversed(self) -> bool {
        matches!(self, Self::Left | Self::Up)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextDirection {
    #[default]
    Ltr,
    Rtl,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum VerticalDirection {
    #[default]
    Down,
    Up,
}

/// Alignment expressed as a normalized point in the range `[-1, 1]`.
/// `(-1, -1)` is top-left, `(0, 0)` is center, and `(1, 1)` is bottom-right.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Alignment {
    pub x: f32,
    pub y: f32,
}

impl Alignment {
    pub const TOP_LEFT: Self = Self { x: -1.0, y: -1.0 };
    pub const TOP_CENTER: Self = Self { x: 0.0, y: -1.0 };
    pub const TOP_RIGHT: Self = Self { x: 1.0, y: -1.0 };
    pub const CENTER_LEFT: Self = Self { x: -1.0, y: 0.0 };
    pub const CENTER: Self = Self { x: 0.0, y: 0.0 };
    pub const CENTER_RIGHT: Self = Self { x: 1.0, y: 0.0 };
    pub const BOTTOM_LEFT: Self = Self { x: -1.0, y: 1.0 };
    pub const BOTTOM_CENTER: Self = Self { x: 0.0, y: 1.0 };
    pub const BOTTOM_RIGHT: Self = Self { x: 1.0, y: 1.0 };

    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    #[must_use]
    pub const fn along_size(self, size: Size) -> Offset {
        Offset::new(size.width * self.x * 0.5, size.height * self.y * 0.5)
    }

    /// Computes the child origin inside a parent. Values outside `[-1, 1]`
    /// intentionally permit overflow, matching Flutter's `Alignment`.
    #[must_use]
    pub const fn within(self, parent: Size, child: Size) -> Offset {
        Offset::new(
            (parent.width - child.width) * (self.x + 1.0) * 0.5,
            (parent.height - child.height) * (self.y + 1.0) * 0.5,
        )
    }

    #[must_use]
    pub fn inscribe(self, child: Size, parent: incular_core::Rect) -> incular_core::Rect {
        incular_core::Rect::from_origin_size(parent.origin + self.within(parent.size, child), child)
    }

    #[must_use]
    pub const fn resolve(self, _direction: TextDirection) -> Self {
        self
    }
}

impl std::ops::Add for Alignment {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl std::ops::Sub for Alignment {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl std::ops::Mul<f32> for Alignment {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl std::ops::Div<f32> for Alignment {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self::new(self.x / rhs, self.y / rhs)
    }
}

impl std::ops::Neg for Alignment {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y)
    }
}

impl incular_core::Lerp for Alignment {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: f32, b: f32| a + (b - a) * t;
        Self::new(mix(self.x, other.x), mix(self.y, other.y))
    }
}

/// An offset that's expressed as a fraction of a `[0, 1]` bounding box.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FractionalOffset {
    pub dx: f32,
    pub dy: f32,
}

impl FractionalOffset {
    pub const TOP_LEFT: Self = Self { dx: 0.0, dy: 0.0 };
    pub const TOP_CENTER: Self = Self { dx: 0.5, dy: 0.0 };
    pub const TOP_RIGHT: Self = Self { dx: 1.0, dy: 0.0 };
    pub const CENTER_LEFT: Self = Self { dx: 0.0, dy: 0.5 };
    pub const CENTER: Self = Self { dx: 0.5, dy: 0.5 };
    pub const CENTER_RIGHT: Self = Self { dx: 1.0, dy: 0.5 };
    pub const BOTTOM_LEFT: Self = Self { dx: 0.0, dy: 1.0 };
    pub const BOTTOM_CENTER: Self = Self { dx: 0.5, dy: 1.0 };
    pub const BOTTOM_RIGHT: Self = Self { dx: 1.0, dy: 1.0 };

    #[must_use]
    pub const fn new(dx: f32, dy: f32) -> Self {
        Self { dx, dy }
    }

    #[must_use]
    pub const fn from_alignment(alignment: Alignment) -> Self {
        Self {
            dx: (alignment.x + 1.0) / 2.0,
            dy: (alignment.y + 1.0) / 2.0,
        }
    }

    #[must_use]
    pub const fn to_alignment(self) -> Alignment {
        Alignment::new(self.dx * 2.0 - 1.0, self.dy * 2.0 - 1.0)
    }
}

impl incular_core::Lerp for FractionalOffset {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: f32, b: f32| a + (b - a) * t;
        Self::new(mix(self.dx, other.dx), mix(self.dy, other.dy))
    }
}

/// Alignment whose horizontal value is relative to text direction.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AlignmentDirectional {
    pub start: f32,
    pub y: f32,
}

impl AlignmentDirectional {
    pub const TOP_START: Self = Self {
        start: -1.0,
        y: -1.0,
    };
    pub const TOP_CENTER: Self = Self {
        start: 0.0,
        y: -1.0,
    };
    pub const TOP_END: Self = Self {
        start: 1.0,
        y: -1.0,
    };
    pub const CENTER_START: Self = Self {
        start: -1.0,
        y: 0.0,
    };
    pub const CENTER: Self = Self { start: 0.0, y: 0.0 };
    pub const CENTER_END: Self = Self { start: 1.0, y: 0.0 };
    pub const BOTTOM_START: Self = Self {
        start: -1.0,
        y: 1.0,
    };
    pub const BOTTOM_CENTER: Self = Self { start: 0.0, y: 1.0 };
    pub const BOTTOM_END: Self = Self { start: 1.0, y: 1.0 };

    #[must_use]
    pub const fn new(start: f32, y: f32) -> Self {
        Self { start, y }
    }

    #[must_use]
    pub const fn resolve(self, direction: TextDirection) -> Alignment {
        Alignment::new(
            match direction {
                TextDirection::Ltr => self.start,
                TextDirection::Rtl => -self.start,
            },
            self.y,
        )
    }
}

impl incular_core::Lerp for AlignmentDirectional {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: f32, b: f32| a + (b - a) * t;
        Self::new(mix(self.start, other.start), mix(self.y, other.y))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MainAxisSize {
    Min,
    #[default]
    Max,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MainAxisAlignment {
    #[default]
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

impl MainAxisAlignment {
    #[must_use]
    pub const fn is_distributed(self) -> bool {
        matches!(
            self,
            Self::SpaceBetween | Self::SpaceAround | Self::SpaceEvenly
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CrossAxisAlignment {
    #[default]
    Start,
    End,
    Center,
    Stretch,
    Baseline,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FlexFit {
    #[default]
    Tight,
    Loose,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WrapAlignment {
    #[default]
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WrapCrossAlignment {
    #[default]
    Start,
    End,
    Center,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Clip {
    None,
    #[default]
    HardEdge,
    AntiAlias,
    AntiAliasWithSaveLayer,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StackFit {
    #[default]
    Loose,
    Expand,
    Passthrough,
}

#[cfg(test)]
mod tests {
    use super::*;
    use incular_core::Size;

    #[test]
    fn alignment_places_child_in_remaining_space() {
        assert_eq!(
            Alignment::CENTER.within(Size::new(100.0, 80.0), Size::new(20.0, 10.0)),
            Offset::new(40.0, 35.0)
        );
        assert_eq!(
            AlignmentDirectional::CENTER_START.resolve(TextDirection::Rtl),
            Alignment::CENTER_RIGHT
        );
    }
}
