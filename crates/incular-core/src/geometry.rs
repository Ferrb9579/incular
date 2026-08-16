//! Small renderer-neutral geometry and paint values.

use std::ops::{Add, BitOr, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    #[must_use]
    pub fn new(width: f32, height: f32) -> Self {
        assert!(
            width.is_finite() && height.is_finite() && width >= 0.0 && height >= 0.0,
            "sizes must be finite and non-negative"
        );
        Self { width, height }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Offset {
    pub x: f32,
    pub y: f32,
}

impl Offset {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl Add for Offset {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Offset {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub origin: Offset,
    pub size: Size,
}

impl Rect {
    #[must_use]
    pub fn from_origin_size(origin: Offset, size: Size) -> Self {
        Self { origin, size }
    }

    #[must_use]
    pub fn contains(self, point: Offset) -> bool {
        point.x >= self.origin.x
            && point.x <= self.origin.x + self.size.width
            && point.y >= self.origin.y
            && point.y <= self.origin.y + self.size.height
    }

    #[must_use]
    pub fn intersects(self, other: Self) -> bool {
        self.origin.x < other.origin.x + other.size.width
            && other.origin.x < self.origin.x + self.size.width
            && self.origin.y < other.origin.y + other.size.height
            && other.origin.y < self.origin.y + self.size.height
    }

    #[must_use]
    pub fn intersection(self, other: Self) -> Option<Self> {
        let left = self.origin.x.max(other.origin.x);
        let top = self.origin.y.max(other.origin.y);
        let right = (self.origin.x + self.size.width).min(other.origin.x + other.size.width);
        let bottom = (self.origin.y + self.size.height).min(other.origin.y + other.size.height);
        (right >= left && bottom >= top).then(|| {
            Self::from_origin_size(
                Offset::new(left, top),
                Size::new(right - left, bottom - top),
            )
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Color {
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);
    pub const WHITE: Self = Self::rgba(255, 255, 255, 255);
    pub const BLACK: Self = Self::rgba(0, 0, 0, 255);

    #[must_use]
    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    #[must_use]
    pub const fn to_linear_rgba(self) -> [f32; 4] {
        [
            self.red as f32 / 255.0,
            self.green as f32 / 255.0,
            self.blue as f32 / 255.0,
            self.alpha as f32 / 255.0,
        ]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Transform {
    pub translation: Offset,
}

impl Transform {
    #[must_use]
    pub const fn translation(offset: Offset) -> Self {
        Self {
            translation: offset,
        }
    }

    #[must_use]
    pub const fn inverse_translation(self) -> Self {
        Self::translation(Offset::new(-self.translation.x, -self.translation.y))
    }

    #[must_use]
    pub const fn transform_point(self, point: Offset) -> Offset {
        Offset::new(point.x + self.translation.x, point.y + self.translation.y)
    }

    #[must_use]
    pub const fn inverse_transform_point(self, point: Offset) -> Offset {
        Offset::new(point.x - self.translation.x, point.y - self.translation.y)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DirtyFlags(u8);

impl DirtyFlags {
    pub const NONE: Self = Self(0);
    pub const BUILD: Self = Self(1);
    pub const LAYOUT: Self = Self(2);
    pub const PAINT: Self = Self(4);
    pub const COMPOSITE: Self = Self(8);
    pub const SEMANTICS: Self = Self(16);

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }
}

impl BitOr for DirtyFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
