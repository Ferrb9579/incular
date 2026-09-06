//! Box constraints shared by all pure layout algorithms.

use incular_core::Size;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstraintError {
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Constraints {
    min_width: f32,
    max_width: f32,
    min_height: f32,
    max_height: f32,
}
impl Constraints {
    /// Constructs validated bounds.
    ///
    /// # Panics
    /// Panics for invalid bounds; use [`Self::try_new`] for fallible input.
    #[must_use]
    pub fn new(min_width: f32, max_width: f32, min_height: f32, max_height: f32) -> Self {
        Self::try_new(min_width, max_width, min_height, max_height).expect("invalid constraints")
    }

    /// Validates externally supplied bounds. Minimums must be finite and
    /// nonnegative; maximums may be infinite but must not be below minimums.
    pub fn try_new(
        min_width: f32,
        max_width: f32,
        min_height: f32,
        max_height: f32,
    ) -> Result<Self, ConstraintError> {
        if !min_width.is_finite()
            || !min_height.is_finite()
            || min_width < 0.0
            || min_height < 0.0
            || max_width.is_nan()
            || max_height.is_nan()
            || max_width < min_width
            || max_height < min_height
        {
            return Err(ConstraintError::Invalid);
        }
        Ok(Self {
            min_width,
            max_width,
            min_height,
            max_height,
        })
    }

    /// Returns the min width bound.
    #[must_use]
    pub const fn min_width(self) -> f32 {
        self.min_width
    }

    /// Returns the max width bound.
    #[must_use]
    pub const fn max_width(self) -> f32 {
        self.max_width
    }

    /// Returns the min height bound.
    #[must_use]
    pub const fn min_height(self) -> f32 {
        self.min_height
    }

    /// Returns the max height bound.
    #[must_use]
    pub const fn max_height(self) -> f32 {
        self.max_height
    }

    #[must_use]
    pub fn tight(size: Size) -> Self {
        Self::new(size.width, size.width, size.height, size.height)
    }
    #[must_use]
    pub fn loose(size: Size) -> Self {
        Self::new(0., size.width, 0., size.height)
    }
    #[must_use]
    pub fn unbounded() -> Self {
        Self::new(0., f32::INFINITY, 0., f32::INFINITY)
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
        Self::new(0., self.max_width, 0., self.max_height)
    }
    #[must_use]
    pub fn deflate(self, horizontal: f32, vertical: f32) -> Self {
        Self::new(
            (self.min_width - horizontal).max(0.),
            (self.max_width - horizontal).max(0.),
            (self.min_height - vertical).max(0.),
            (self.max_height - vertical).max(0.),
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
    #[must_use]
    pub fn biggest(self) -> Size {
        Size::new(
            if self.max_width.is_finite() {
                self.max_width
            } else {
                self.min_width
            },
            if self.max_height.is_finite() {
                self.max_height
            } else {
                self.min_height
            },
        )
    }
    #[must_use]
    pub fn smallest(self) -> Size {
        Size::new(self.min_width, self.min_height)
    }
}
