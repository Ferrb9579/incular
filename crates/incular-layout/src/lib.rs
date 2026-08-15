//! Constraint-based, renderer-independent layout primitives.
use incular_core::Size;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Constraints {
    pub min_width: f32,
    pub max_width: f32,
    pub min_height: f32,
    pub max_height: f32,
}
impl Constraints {
    #[must_use]
    pub fn new(min_width: f32, max_width: f32, min_height: f32, max_height: f32) -> Self {
        assert!(
            min_width.is_finite()
                && min_height.is_finite()
                && min_width >= 0.0
                && min_height >= 0.0
                && max_width >= min_width
                && max_height >= min_height
                && !max_width.is_nan()
                && !max_height.is_nan(),
            "invalid constraints"
        );
        Self {
            min_width,
            max_width,
            min_height,
            max_height,
        }
    }
    #[must_use]
    pub fn tight(size: Size) -> Self {
        Self::new(size.width, size.width, size.height, size.height)
    }
    #[must_use]
    pub fn loose(size: Size) -> Self {
        Self::new(0.0, size.width, 0.0, size.height)
    }
    #[must_use]
    pub fn unbounded() -> Self {
        Self::new(0.0, f32::INFINITY, 0.0, f32::INFINITY)
    }
    #[must_use]
    pub fn constrain(self, size: Size) -> Size {
        Size::new(
            size.width.clamp(self.min_width, self.max_width),
            size.height.clamp(self.min_height, self.max_height),
        )
    }
    #[must_use]
    pub fn loosen(self) -> Self {
        Self::new(0.0, self.max_width, 0.0, self.max_height)
    }
    #[must_use]
    pub fn deflate(self, horizontal: f32, vertical: f32) -> Self {
        Self::new(
            (self.min_width - horizontal).max(0.0),
            (self.max_width - horizontal).max(0.0),
            (self.min_height - vertical).max(0.0),
            (self.max_height - vertical).max(0.0),
        )
    }
    #[must_use]
    pub fn is_width_bounded(self) -> bool {
        self.max_width.is_finite()
    }
    #[must_use]
    pub fn is_height_bounded(self) -> bool {
        self.max_height.is_finite()
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeInsets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}
impl EdgeInsets {
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
    pub const fn horizontal(self) -> f32 {
        self.left + self.right
    }
    #[must_use]
    pub const fn vertical(self) -> f32 {
        self.top + self.bottom
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Alignment {
    pub x: f32,
    pub y: f32,
}
impl Alignment {
    pub const CENTER: Self = Self { x: 0.0, y: 0.0 };
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn constrains_each_axis() {
        let c = Constraints::new(10., 20., 5., 15.);
        assert_eq!(c.constrain(Size::new(30., 1.)), Size::new(20., 5.));
    }
    #[test]
    fn deflate_never_inverts() {
        assert_eq!(
            Constraints::tight(Size::new(5., 5.)).deflate(10., 10.),
            Constraints::tight(Size::ZERO)
        );
    }
}
