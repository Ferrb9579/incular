//! Corner-radius geometry shared by painting and decoration descriptors.

use incular_rendering::CornerRadii;
use typed_builder::TypedBuilder;

/// A 2D radius for rounded corners.
#[derive(Clone, Copy, Debug, Default, PartialEq, TypedBuilder)]
pub struct Radius {
    #[builder(default = 0.0)]
    pub x: f32,
    #[builder(default = 0.0)]
    pub y: f32,
}

impl Radius {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    #[must_use]
    pub const fn circular(radius: f32) -> Self {
        Self {
            x: radius,
            y: radius,
        }
    }

    #[must_use]
    pub const fn elliptical(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    #[must_use]
    pub const fn zero() -> Self {
        Self::ZERO
    }
}

impl incular_core::Lerp for Radius {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {
            x: self.x.lerp(&other.x, t).max(0.0),
            y: self.y.lerp(&other.y, t).max(0.0),
        }
    }
}

/// An immutable set of radii for each of the four corners of a box.
#[derive(Clone, Copy, Debug, Default, PartialEq, TypedBuilder)]
pub struct BorderRadius {
    #[builder(default = Radius::ZERO)]
    pub top_left: Radius,
    #[builder(default = Radius::ZERO)]
    pub top_right: Radius,
    #[builder(default = Radius::ZERO)]
    pub bottom_right: Radius,
    #[builder(default = Radius::ZERO)]
    pub bottom_left: Radius,
}

impl BorderRadius {
    pub const ZERO: Self = Self {
        top_left: Radius::ZERO,
        top_right: Radius::ZERO,
        bottom_right: Radius::ZERO,
        bottom_left: Radius::ZERO,
    };

    #[must_use]
    pub const fn all(radius: Radius) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    #[must_use]
    pub const fn circular(radius: f32) -> Self {
        Self::all(Radius::circular(radius))
    }

    #[must_use]
    pub const fn vertical(top: Radius, bottom: Radius) -> Self {
        Self {
            top_left: top,
            top_right: top,
            bottom_right: bottom,
            bottom_left: bottom,
        }
    }

    #[must_use]
    pub const fn horizontal(left: Radius, right: Radius) -> Self {
        Self {
            top_left: left,
            top_right: right,
            bottom_right: right,
            bottom_left: left,
        }
    }

    #[must_use]
    pub const fn only(
        top_left: Radius,
        top_right: Radius,
        bottom_right: Radius,
        bottom_left: Radius,
    ) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    #[must_use]
    pub const fn zero() -> Self {
        Self::ZERO
    }

    #[must_use]
    pub const fn resolve(self, _direction: incular_config::TextDirection) -> Self {
        self
    }

    #[must_use]
    pub fn to_corner_radii(self) -> CornerRadii {
        CornerRadii {
            top_left: self.top_left.x,
            top_right: self.top_right.x,
            bottom_right: self.bottom_right.x,
            bottom_left: self.bottom_left.x,
        }
    }
}

impl incular_core::Lerp for BorderRadius {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {
            top_left: self.top_left.lerp(&other.top_left, t),
            top_right: self.top_right.lerp(&other.top_right, t),
            bottom_right: self.bottom_right.lerp(&other.bottom_right, t),
            bottom_left: self.bottom_left.lerp(&other.bottom_left, t),
        }
    }
}

impl From<BorderRadius> for CornerRadii {
    fn from(value: BorderRadius) -> Self {
        value.to_corner_radii()
    }
}

/// An immutable set of directional radii for the four corners of a box.
#[derive(Clone, Copy, Debug, Default, PartialEq, TypedBuilder)]
pub struct BorderRadiusDirectional {
    #[builder(default = Radius::ZERO)]
    pub top_start: Radius,
    #[builder(default = Radius::ZERO)]
    pub top_end: Radius,
    #[builder(default = Radius::ZERO)]
    pub bottom_end: Radius,
    #[builder(default = Radius::ZERO)]
    pub bottom_start: Radius,
}

impl BorderRadiusDirectional {
    pub const ZERO: Self = Self {
        top_start: Radius::ZERO,
        top_end: Radius::ZERO,
        bottom_end: Radius::ZERO,
        bottom_start: Radius::ZERO,
    };

    #[must_use]
    pub const fn all(radius: Radius) -> Self {
        Self {
            top_start: radius,
            top_end: radius,
            bottom_end: radius,
            bottom_start: radius,
        }
    }

    #[must_use]
    pub const fn circular(radius: f32) -> Self {
        Self::all(Radius::circular(radius))
    }

    #[must_use]
    pub const fn vertical(top: Radius, bottom: Radius) -> Self {
        Self {
            top_start: top,
            top_end: top,
            bottom_end: bottom,
            bottom_start: bottom,
        }
    }

    #[must_use]
    pub const fn horizontal(start: Radius, end: Radius) -> Self {
        Self {
            top_start: start,
            top_end: end,
            bottom_end: end,
            bottom_start: start,
        }
    }

    #[must_use]
    pub const fn only(
        top_start: Radius,
        top_end: Radius,
        bottom_end: Radius,
        bottom_start: Radius,
    ) -> Self {
        Self {
            top_start,
            top_end,
            bottom_end,
            bottom_start,
        }
    }

    #[must_use]
    pub const fn zero() -> Self {
        Self::ZERO
    }

    #[must_use]
    pub const fn resolve(self, direction: incular_config::TextDirection) -> BorderRadius {
        match direction {
            incular_config::TextDirection::Ltr => BorderRadius {
                top_left: self.top_start,
                top_right: self.top_end,
                bottom_right: self.bottom_end,
                bottom_left: self.bottom_start,
            },
            incular_config::TextDirection::Rtl => BorderRadius {
                top_left: self.top_end,
                top_right: self.top_start,
                bottom_right: self.bottom_start,
                bottom_left: self.bottom_end,
            },
        }
    }
}

impl incular_core::Lerp for BorderRadiusDirectional {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {
            top_start: self.top_start.lerp(&other.top_start, t),
            top_end: self.top_end.lerp(&other.top_end, t),
            bottom_end: self.bottom_end.lerp(&other.bottom_end, t),
            bottom_start: self.bottom_start.lerp(&other.bottom_start, t),
        }
    }
}
