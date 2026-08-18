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

    /// Converts this sRGB color to the HSL representation used by style and
    /// theme code. Hue is expressed in degrees and alpha in `0.0..=1.0`.
    #[must_use]
    pub fn to_hsl(self) -> HslColor {
        HslColor::from_color(self)
    }

    /// Converts this sRGB color to HSV. Hue is expressed in degrees and alpha
    /// in `0.0..=1.0`.
    #[must_use]
    pub fn to_hsv(self) -> HsvColor {
        HsvColor::from_color(self)
    }
}

/// Hue, saturation, lightness, and opacity in an sRGB color space.
///
/// Hue is normalized to `0.0..360.0`; saturation, lightness, and alpha are
/// normalized to `0.0..=1.0`. Constructors accept non-finite values but
/// sanitize them, keeping configuration values safe to use in rendering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HslColor {
    pub hue: f32,
    pub saturation: f32,
    pub lightness: f32,
    pub alpha: f32,
}

impl HslColor {
    #[must_use]
    pub fn new(hue: f32, saturation: f32, lightness: f32, alpha: f32) -> Self {
        Self {
            hue: normalize_hue(hue),
            saturation: unit(saturation),
            lightness: unit(lightness),
            alpha: unit(alpha),
        }
    }

    #[must_use]
    pub fn from_color(color: Color) -> Self {
        let r = color.red as f32 / 255.0;
        let g = color.green as f32 / 255.0;
        let b = color.blue as f32 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        let lightness = (max + min) * 0.5;
        let saturation = if delta <= f32::EPSILON {
            0.0
        } else {
            delta / (1.0 - (2.0 * lightness - 1.0).abs())
        };
        Self::new(
            hue_from_rgb(r, g, b, max, delta),
            saturation,
            lightness,
            color.alpha as f32 / 255.0,
        )
    }

    #[must_use]
    pub fn to_color(self) -> Color {
        let h = normalize_hue(self.hue) / 360.0;
        let saturation = unit(self.saturation);
        let lightness = unit(self.lightness);
        let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
        let (r, g, b) = rgb_from_hue_chroma(h, chroma, lightness - chroma * 0.5);
        color_from_unit(r, g, b, self.alpha)
    }
}

/// Hue, saturation, value, and opacity in an sRGB color space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HsvColor {
    pub hue: f32,
    pub saturation: f32,
    pub value: f32,
    pub alpha: f32,
}

impl HsvColor {
    #[must_use]
    pub fn new(hue: f32, saturation: f32, value: f32, alpha: f32) -> Self {
        Self {
            hue: normalize_hue(hue),
            saturation: unit(saturation),
            value: unit(value),
            alpha: unit(alpha),
        }
    }

    #[must_use]
    pub fn from_color(color: Color) -> Self {
        let r = color.red as f32 / 255.0;
        let g = color.green as f32 / 255.0;
        let b = color.blue as f32 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        Self::new(
            hue_from_rgb(r, g, b, max, delta),
            if max <= f32::EPSILON {
                0.0
            } else {
                delta / max
            },
            max,
            color.alpha as f32 / 255.0,
        )
    }

    #[must_use]
    pub fn to_color(self) -> Color {
        let value = unit(self.value);
        let chroma = value * unit(self.saturation);
        let (r, g, b) =
            rgb_from_hue_chroma(normalize_hue(self.hue) / 360.0, chroma, value - chroma);
        color_from_unit(r, g, b, self.alpha)
    }
}

fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn normalize_hue(hue: f32) -> f32 {
    if hue.is_finite() {
        hue.rem_euclid(360.0)
    } else {
        0.0
    }
}

fn hue_from_rgb(r: f32, g: f32, b: f32, max: f32, delta: f32) -> f32 {
    if delta <= f32::EPSILON {
        return 0.0;
    }
    let hue = if (max - r).abs() <= f32::EPSILON {
        ((g - b) / delta).rem_euclid(6.0)
    } else if (max - g).abs() <= f32::EPSILON {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    };
    normalize_hue(hue * 60.0)
}

fn rgb_from_hue_chroma(hue: f32, chroma: f32, offset: f32) -> (f32, f32, f32) {
    let segment = hue * 6.0;
    let x = chroma * (1.0 - (segment.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = if segment < 1.0 {
        (chroma, x, 0.0)
    } else if segment < 2.0 {
        (x, chroma, 0.0)
    } else if segment < 3.0 {
        (0.0, chroma, x)
    } else if segment < 4.0 {
        (0.0, x, chroma)
    } else if segment < 5.0 {
        (x, 0.0, chroma)
    } else {
        (chroma, 0.0, x)
    };
    (r + offset, g + offset, b + offset)
}

fn color_from_unit(red: f32, green: f32, blue: f32, alpha: f32) -> Color {
    let byte = |value: f32| (unit(value) * 255.0).round() as u8;
    Color::rgba(byte(red), byte(green), byte(blue), byte(alpha))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsl_primary_colors_and_opacity_convert_to_srgb() {
        assert_eq!(
            HslColor::new(0.0, 1.0, 0.5, 1.0).to_color(),
            Color::rgba(255, 0, 0, 255)
        );
        assert_eq!(
            HslColor::new(120.0, 1.0, 0.5, 0.5).to_color(),
            Color::rgba(0, 255, 0, 128)
        );
        assert_eq!(
            HslColor::new(240.0, 1.0, 0.5, 1.0).to_color(),
            Color::rgba(0, 0, 255, 255)
        );
    }

    #[test]
    fn hsv_round_trip_preserves_srgb_bytes() {
        for color in [
            Color::rgba(12, 190, 73, 64),
            Color::rgba(255, 128, 0, 255),
            Color::rgba(33, 33, 33, 0),
        ] {
            assert_eq!(color.to_hsv().to_color(), color);
        }
    }

    #[test]
    fn hue_and_channels_are_sanitized() {
        let hsl = HslColor::new(-30.0, 4.0, -1.0, f32::NAN);
        assert_eq!(hsl.hue, 330.0);
        assert_eq!(hsl.saturation, 1.0);
        assert_eq!(hsl.lightness, 0.0);
        assert_eq!(hsl.alpha, 0.0);
    }
}
