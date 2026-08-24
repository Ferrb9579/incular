use std::ops::{Add, AddAssign, Sub, SubAssign};

/// Insets on the four physical edges of a rectangle.
///
/// Values are kept as supplied so callers can use an inset as a lightweight
/// configuration value. Layout algorithms sanitize non-finite and negative
/// values before applying them; [`EdgeInsets::is_valid`] is available when a
/// caller wants to validate eagerly.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EdgeInsets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl EdgeInsets {
    pub const ZERO: Self = Self {
        left: 0.0,
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
    };

    #[must_use]
    pub const fn zero() -> Self {
        Self::ZERO
    }

    #[must_use]
    pub const fn all(value: f32) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }

    #[must_use]
    pub const fn symmetric(horizontal: f32, vertical: f32) -> Self {
        Self {
            left: horizontal,
            top: vertical,
            right: horizontal,
            bottom: vertical,
        }
    }

    #[must_use]
    pub const fn horizontal_insets(horizontal: f32) -> Self {
        Self::symmetric(horizontal, 0.0)
    }

    #[must_use]
    pub const fn vertical_insets(vertical: f32) -> Self {
        Self::symmetric(0.0, vertical)
    }

    #[must_use]
    pub const fn from_xy(horizontal: f32, vertical: f32) -> Self {
        Self::symmetric(horizontal, vertical)
    }

    #[must_use]
    pub const fn only(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    #[must_use]
    pub const fn from_ltrb(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self::only(left, top, right, bottom)
    }

    #[must_use]
    pub const fn horizontal(self) -> f32 {
        self.left + self.right
    }

    #[must_use]
    pub const fn vertical(self) -> f32 {
        self.top + self.bottom
    }

    #[must_use]
    pub const fn dimensions(self) -> (f32, f32) {
        (self.horizontal(), self.vertical())
    }

    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.left == 0.0 && self.top == 0.0 && self.right == 0.0 && self.bottom == 0.0
    }

    #[must_use]
    pub fn is_valid(self) -> bool {
        [self.left, self.top, self.right, self.bottom]
            .into_iter()
            .all(|value| value.is_finite() && value >= 0.0)
    }

    /// Replaces invalid or negative values with zero.
    #[must_use]
    pub fn normalized(self) -> Self {
        Self::only(
            clean(self.left),
            clean(self.top),
            clean(self.right),
            clean(self.bottom),
        )
    }

    #[must_use]
    pub fn clamp(self, min: f32, max: f32) -> Self {
        let (min, max) = if min.is_finite() && max.is_finite() && min <= max {
            (min, max)
        } else {
            (0.0, f32::MAX)
        };
        Self::only(
            self.left.clamp(min, max),
            self.top.clamp(min, max),
            self.right.clamp(min, max),
            self.bottom.clamp(min, max),
        )
    }

    #[must_use]
    pub fn inset(self, amount: f32) -> Self {
        Self::only(
            (self.left - amount).max(0.0),
            (self.top - amount).max(0.0),
            (self.right - amount).max(0.0),
            (self.bottom - amount).max(0.0),
        )
    }

    #[must_use]
    pub fn outset(self, amount: f32) -> Self {
        Self::only(
            (self.left + amount).max(0.0),
            (self.top + amount).max(0.0),
            (self.right + amount).max(0.0),
            (self.bottom + amount).max(0.0),
        )
    }
}

fn clean(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

impl Add for EdgeInsets {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::only(
            self.left + rhs.left,
            self.top + rhs.top,
            self.right + rhs.right,
            self.bottom + rhs.bottom,
        )
    }
}

impl AddAssign for EdgeInsets {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for EdgeInsets {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::only(
            self.left - rhs.left,
            self.top - rhs.top,
            self.right - rhs.right,
            self.bottom - rhs.bottom,
        )
    }
}

impl SubAssign for EdgeInsets {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl std::ops::Mul<f32> for EdgeInsets {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::only(
            self.left * rhs,
            self.top * rhs,
            self.right * rhs,
            self.bottom * rhs,
        )
    }
}

impl std::ops::Div<f32> for EdgeInsets {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self::only(
            self.left / rhs,
            self.top / rhs,
            self.right / rhs,
            self.bottom / rhs,
        )
    }
}

impl std::ops::Neg for EdgeInsets {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::only(-self.left, -self.top, -self.right, -self.bottom)
    }
}

impl incular_core::Lerp for EdgeInsets {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: f32, b: f32| a + (b - a) * t;
        Self::only(
            mix(self.left, other.left),
            mix(self.top, other.top),
            mix(self.right, other.right),
            mix(self.bottom, other.bottom),
        )
    }
}

/// An immutable set of directional offsets in logical pixels.
/// Resolves against a [`crate::TextDirection`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EdgeInsetsDirectional {
    pub start: f32,
    pub top: f32,
    pub end: f32,
    pub bottom: f32,
}

impl EdgeInsetsDirectional {
    pub const ZERO: Self = Self {
        start: 0.0,
        top: 0.0,
        end: 0.0,
        bottom: 0.0,
    };

    #[must_use]
    pub const fn zero() -> Self {
        Self::ZERO
    }

    #[must_use]
    pub const fn all(value: f32) -> Self {
        Self {
            start: value,
            top: value,
            end: value,
            bottom: value,
        }
    }

    #[must_use]
    pub const fn only(start: f32, top: f32, end: f32, bottom: f32) -> Self {
        Self {
            start,
            top,
            end,
            bottom,
        }
    }

    #[must_use]
    pub const fn symmetric(horizontal: f32, vertical: f32) -> Self {
        Self {
            start: horizontal,
            top: vertical,
            end: horizontal,
            bottom: vertical,
        }
    }

    #[must_use]
    pub const fn from_ste_b(start: f32, top: f32, end: f32, bottom: f32) -> Self {
        Self::only(start, top, end, bottom)
    }

    #[must_use]
    pub const fn resolve(self, direction: crate::TextDirection) -> EdgeInsets {
        match direction {
            crate::TextDirection::Ltr => {
                EdgeInsets::only(self.start, self.top, self.end, self.bottom)
            }
            crate::TextDirection::Rtl => {
                EdgeInsets::only(self.end, self.top, self.start, self.bottom)
            }
        }
    }

    #[must_use]
    pub const fn horizontal(self) -> f32 {
        self.start + self.end
    }

    #[must_use]
    pub const fn vertical(self) -> f32 {
        self.top + self.bottom
    }
}

impl Add for EdgeInsetsDirectional {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::only(
            self.start + rhs.start,
            self.top + rhs.top,
            self.end + rhs.end,
            self.bottom + rhs.bottom,
        )
    }
}

impl Sub for EdgeInsetsDirectional {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::only(
            self.start - rhs.start,
            self.top - rhs.top,
            self.end - rhs.end,
            self.bottom - rhs.bottom,
        )
    }
}

impl std::ops::Mul<f32> for EdgeInsetsDirectional {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::only(
            self.start * rhs,
            self.top * rhs,
            self.end * rhs,
            self.bottom * rhs,
        )
    }
}

impl std::ops::Div<f32> for EdgeInsetsDirectional {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self::only(
            self.start / rhs,
            self.top / rhs,
            self.end / rhs,
            self.bottom / rhs,
        )
    }
}

impl incular_core::Lerp for EdgeInsetsDirectional {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: f32, b: f32| a + (b - a) * t;
        Self::only(
            mix(self.start, other.start),
            mix(self.top, other.top),
            mix(self.end, other.end),
            mix(self.bottom, other.bottom),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_and_dimensions_are_consistent() {
        let insets = EdgeInsets::from_xy(2.0, 3.0);
        assert_eq!(insets, EdgeInsets::only(2.0, 3.0, 2.0, 3.0));
        assert_eq!(insets.dimensions(), (4.0, 6.0));
    }

    #[test]
    fn normalization_removes_invalid_values() {
        let normalized = EdgeInsets::only(-1.0, f32::NAN, 2.0, f32::INFINITY).normalized();
        assert_eq!(normalized, EdgeInsets::only(0.0, 0.0, 2.0, 0.0));
    }
}
