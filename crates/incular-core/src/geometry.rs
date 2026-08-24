//! Small renderer-neutral geometry and paint values.

use std::ops::{Add, Sub};

use palette::{FromColor, Hsl, Hsv, Srgba, WithAlpha};

pub use kurbo::Affine;
use kurbo::{Point, Vec2};

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

    /// Converts the compact encoded sRGBA8 value to straight, linear-sRGB
    /// components. Palette owns the CPU transfer function; WGSL retains its
    /// equivalent explicit shader-side math at the GPU boundary.
    #[must_use]
    pub fn to_linear_rgba(self) -> [f32; 4] {
        let linear = Srgba::new(
            self.red as f32 / 255.0,
            self.green as f32 / 255.0,
            self.blue as f32 / 255.0,
            self.alpha as f32 / 255.0,
        )
        .into_linear();
        [
            linear.color.red,
            linear.color.green,
            linear.color.blue,
            linear.alpha,
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
        let source = palette_color(color);
        let hsl = Hsl::from_color(source.color);
        Self::new(
            hsl.hue.into_degrees(),
            hsl.saturation,
            hsl.lightness,
            source.alpha,
        )
    }

    #[must_use]
    pub fn to_color(self) -> Color {
        let hsl = Hsl::new_srgb(
            normalize_hue(self.hue),
            unit(self.saturation),
            unit(self.lightness),
        );
        color_from_palette(Srgba::from_color(hsl).with_alpha(unit(self.alpha)))
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
        let source = palette_color(color);
        let hsv = Hsv::from_color(source.color);
        Self::new(
            hsv.hue.into_degrees(),
            hsv.saturation,
            hsv.value,
            source.alpha,
        )
    }

    #[must_use]
    pub fn to_color(self) -> Color {
        let hsv = Hsv::new_srgb(
            normalize_hue(self.hue),
            unit(self.saturation),
            unit(self.value),
        );
        color_from_palette(Srgba::from_color(hsv).with_alpha(unit(self.alpha)))
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

fn palette_color(color: Color) -> Srgba {
    Srgba::new(
        color.red as f32 / 255.0,
        color.green as f32 / 255.0,
        color.blue as f32 / 255.0,
        color.alpha as f32 / 255.0,
    )
}

fn color_from_palette(color: Srgba) -> Color {
    let byte = |value: f32| (unit(value) * 255.0).round() as u8;
    Color::rgba(
        byte(color.color.red),
        byte(color.color.green),
        byte(color.color.blue),
        byte(color.alpha),
    )
}

/// A retained two-dimensional affine transform.
///
/// Incular keeps its ordinary layout geometry in `f32`; Kurbo is the single
/// authority for affine composition, inversion, and geometric bounds. The
/// conversion happens only at this boundary, so applications do not need to
/// adopt a second basic point/rectangle vocabulary for normal widget code.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform(Affine);

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    pub const IDENTITY: Self = Self(Affine::IDENTITY);

    #[must_use]
    pub fn translation(offset: Offset) -> Self {
        Self(Affine::translate(Vec2::new(
            f64::from(offset.x),
            f64::from(offset.y),
        )))
    }

    #[must_use]
    pub const fn scale(scale: f32) -> Self {
        Self(Affine::scale(scale as f64))
    }

    #[must_use]
    pub const fn scale_non_uniform(x: f32, y: f32) -> Self {
        Self(Affine::scale_non_uniform(x as f64, y as f64))
    }

    /// Rotates clockwise in Incular's y-down coordinate system.
    #[must_use]
    pub fn rotation(radians: f32) -> Self {
        Self(Affine::rotate(f64::from(radians)))
    }

    #[must_use]
    pub const fn skew(x: f32, y: f32) -> Self {
        Self(Affine::skew(x as f64, y as f64))
    }

    #[must_use]
    pub const fn from_kurbo(affine: Affine) -> Self {
        Self(affine)
    }

    #[must_use]
    pub const fn to_kurbo(self) -> Affine {
        self.0
    }

    /// Applies `other` first, then this transform.
    #[must_use]
    pub fn then(self, other: Self) -> Self {
        Self(self.0 * other.0)
    }

    #[must_use]
    pub fn inverse(self) -> Option<Self> {
        let inverse = self.0.inverse();
        inverse.is_finite().then_some(Self(inverse))
    }

    #[must_use]
    pub fn inverse_translation(self) -> Self {
        self.inverse().unwrap_or(Self::IDENTITY)
    }

    #[must_use]
    pub fn translation_offset(self) -> Offset {
        let coefficients = self.0.as_coeffs();
        Offset::new(coefficients[4] as f32, coefficients[5] as f32)
    }

    #[must_use]
    pub fn is_translation(self) -> bool {
        let [a, b, c, d, _, _] = self.0.as_coeffs();
        a == 1. && b == 0. && c == 0. && d == 1.
    }

    #[must_use]
    pub fn transform_point(self, point: Offset) -> Offset {
        let point = self.0 * Point::new(f64::from(point.x), f64::from(point.y));
        Offset::new(point.x as f32, point.y as f32)
    }

    #[must_use]
    pub fn inverse_transform_point(self, point: Offset) -> Option<Offset> {
        self.inverse().map(|inverse| inverse.transform_point(point))
    }

    #[must_use]
    pub fn transform_rect_bbox(self, rect: Rect) -> Rect {
        let rect = kurbo::Rect::new(
            f64::from(rect.origin.x),
            f64::from(rect.origin.y),
            f64::from(rect.origin.x + rect.size.width),
            f64::from(rect.origin.y + rect.size.height),
        );
        let rect = self.0.transform_rect_bbox(rect);
        Rect::from_origin_size(
            Offset::new(rect.x0 as f32, rect.y0 as f32),
            Size::new((rect.x1 - rect.x0) as f32, (rect.y1 - rect.y0) as f32),
        )
    }
}

/// Multi-dimensional damage invalidation representation across independent retained runtime phases.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Invalidation(u16);

impl Invalidation {
    /// No invalidation required.
    pub const NONE: Self = Self(0);
    /// Subtree widget build / structural reconciliation required.
    pub const BUILD: Self = Self(1 << 0);
    /// Layout measurement and positioning recalculation required.
    pub const LAYOUT: Self = Self(1 << 1);
    /// Picture display list re-recording required.
    pub const PAINT: Self = Self(1 << 2);
    /// Retained compositor layer properties or transforms modified.
    pub const COMPOSITE: Self = Self(1 << 3);
    /// Accessibility semantics tree update required.
    pub const SEMANTICS: Self = Self(1 << 4);
    /// Hit-test routing / pointer target cache update required.
    pub const HIT_TEST: Self = Self(1 << 5);

    /// Checks if no invalidation bits are set.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Checks whether all bits in `other` are set in `self`.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Checks whether any bits in `other` are set in `self`.
    #[must_use]
    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    /// Adds invalidation flags.
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    /// Removes invalidation flags.
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    /// Computes the union of two invalidation masks.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Computes the intersection of two invalidation masks.
    #[must_use]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}

/// Alias for [`Invalidation`] for backwards compatibility.
pub type DirtyFlags = Invalidation;

impl std::ops::BitOr for Invalidation {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for Invalidation {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitAnd for Invalidation {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl std::ops::BitAndAssign for Invalidation {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl std::ops::Not for Invalidation {
    type Output = Self;

    fn not(self) -> Self {
        Self(!self.0)
    }
}

/// A type that can be linearly interpolated between two values.
pub trait Lerp {
    /// Linearly interpolates between `self` and `other` with parameter `t` in `[0.0, 1.0]`.
    #[must_use]
    fn lerp(&self, other: &Self, t: f32) -> Self;
}

impl Lerp for f32 {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        self + (other - self) * t
    }
}

impl Lerp for f64 {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        self + (other - self) * f64::from(t)
    }
}

impl Lerp for Offset {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {
            x: self.x.lerp(&other.x, t),
            y: self.y.lerp(&other.y, t),
        }
    }
}

impl Lerp for Size {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {
            width: self.width.lerp(&other.width, t).max(0.0),
            height: self.height.lerp(&other.height, t).max(0.0),
        }
    }
}

impl Lerp for Rect {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {
            origin: self.origin.lerp(&other.origin, t),
            size: self.size.lerp(&other.size, t),
        }
    }
}

impl Lerp for Color {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| -> u8 {
            (f32::from(a) + (f32::from(b) - f32::from(a)) * t)
                .round()
                .clamp(0.0, 255.0) as u8
        };
        Self::rgba(
            mix(self.red, other.red),
            mix(self.green, other.green),
            mix(self.blue, other.blue),
            mix(self.alpha, other.alpha),
        )
    }
}

impl Lerp for HslColor {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: f32, b: f32| a + (b - a) * t;
        Self::new(
            mix(self.hue, other.hue),
            mix(self.saturation, other.saturation),
            mix(self.lightness, other.lightness),
            mix(self.alpha, other.alpha),
        )
    }
}

impl Lerp for HsvColor {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: f32, b: f32| a + (b - a) * t;
        Self::new(
            mix(self.hue, other.hue),
            mix(self.saturation, other.saturation),
            mix(self.value, other.value),
            mix(self.alpha, other.alpha),
        )
    }
}

/// Identifies the invalidation scope required when a property changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChangeImpact {
    /// No visual or layout change.
    #[default]
    None,
    /// Requires only compositor layer property / transform update.
    Composite,
    /// Requires painting display list regeneration without layout changes.
    Paint,
    /// Requires layout measurement and positioning recalculation.
    Layout,
    /// Requires rebuilding widget subtree.
    Build,
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
    fn affine_composition_inverse_and_bounds_use_kurbo() {
        let transform = Transform::translation(Offset::new(10., 20.))
            .then(Transform::rotation(std::f32::consts::FRAC_PI_2))
            .then(Transform::scale_non_uniform(2., 3.));
        let point = transform.transform_point(Offset::new(2., 0.));
        let restored = transform
            .inverse_transform_point(point)
            .expect("non-singular affine transform");
        assert!((restored.x - 2.).abs() < 0.0001);
        assert!(restored.y.abs() < 0.0001);

        let bounds = Transform::rotation(std::f32::consts::FRAC_PI_2)
            .transform_rect_bbox(Rect::from_origin_size(Offset::ZERO, Size::new(10., 20.)));
        assert!((bounds.size.width - 20.).abs() < 0.0001);
        assert!((bounds.size.height - 10.).abs() < 0.0001);
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

    #[test]
    fn linear_rgba_uses_palette_srgb_transfer_function() {
        let [red, green, blue, alpha] = Color::rgba(128, 128, 128, 128).to_linear_rgba();
        assert!((red - 0.215_861).abs() < 0.000_01);
        assert_eq!(red, green);
        assert_eq!(green, blue);
        assert!((alpha - 128.0 / 255.0).abs() < f32::EPSILON);
    }
}
