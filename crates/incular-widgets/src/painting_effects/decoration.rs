//! Box decoration primitives and their painting data.

use incular_core::{Color, Offset};
use incular_image::ImageHandle;
use incular_rendering::{BlendMode, Brush};
use typed_builder::TypedBuilder;

use super::geometry::BorderRadius;
use crate::tree::{ImageFit, ImageRepeat};

/// The style of a border stroke.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BorderStyle {
    None,
    #[default]
    Solid,
}

/// One side of a border.
#[derive(Clone, Copy, Debug, PartialEq, TypedBuilder)]
pub struct BorderSide {
    #[builder(default = Color::BLACK)]
    pub color: Color,
    #[builder(
        default = 1.0,
        setter(transform = |width: f32| width.max(0.0))
    )]
    pub width: f32,
    #[builder(default = BorderStyle::Solid)]
    pub style: BorderStyle,
    #[builder(default = -1.0)]
    pub stroke_align: f32,
}

impl Default for BorderSide {
    fn default() -> Self {
        Self {
            color: Color::BLACK,
            width: 1.0,
            style: BorderStyle::Solid,
            stroke_align: -1.0,
        }
    }
}

impl BorderSide {
    pub const NONE: Self = Self {
        color: Color::TRANSPARENT,
        width: 0.0,
        style: BorderStyle::None,
        stroke_align: -1.0,
    };

    #[must_use]
    pub const fn new(color: Color, width: f32, style: BorderStyle) -> Self {
        Self {
            color,
            width: if width >= 0.0 { width } else { 0.0 },
            style,
            stroke_align: -1.0,
        }
    }

    #[must_use]
    pub const fn solid(color: Color, width: f32) -> Self {
        Self::new(color, width, BorderStyle::Solid)
    }

    #[must_use]
    pub const fn none() -> Self {
        Self::NONE
    }
}

impl incular_core::Lerp for BorderSide {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        if self.style == BorderStyle::None && other.style == BorderStyle::None {
            return Self::NONE;
        }
        Self {
            color: self.color.lerp(&other.color, t),
            width: self.width.lerp(&other.width, t).max(0.0),
            style: if t < 0.5 { self.style } else { other.style },
            stroke_align: self.stroke_align.lerp(&other.stroke_align, t),
        }
    }
}

/// A box border configuration specifying stroke properties on all four sides.
#[derive(Clone, Copy, Debug, PartialEq, TypedBuilder)]
pub struct Border {
    pub top: BorderSide,
    pub right: BorderSide,
    pub bottom: BorderSide,
    pub left: BorderSide,
}

/// Alias for [`Border`], matching Flutter naming.
pub type BoxBorder = Border;

impl Border {
    #[must_use]
    pub fn new(width: f32, color: incular_core::Color) -> Self {
        Self::all(BorderSide::solid(color, width))
    }

    #[must_use]
    pub const fn all(side: BorderSide) -> Self {
        Self {
            top: side,
            right: side,
            bottom: side,
            left: side,
        }
    }

    #[must_use]
    pub const fn symmetric(vertical: BorderSide, horizontal: BorderSide) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    #[must_use]
    pub const fn only(
        top: BorderSide,
        right: BorderSide,
        bottom: BorderSide,
        left: BorderSide,
    ) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    #[must_use]
    pub const fn from_border_side(side: BorderSide) -> Self {
        Self::all(side)
    }

    #[must_use]
    pub const fn dimensions(self) -> incular_config::EdgeInsets {
        incular_config::EdgeInsets::only(
            self.left.width,
            self.top.width,
            self.right.width,
            self.bottom.width,
        )
    }
}

impl incular_core::Lerp for Border {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {
            top: self.top.lerp(&other.top, t),
            right: self.right.lerp(&other.right, t),
            bottom: self.bottom.lerp(&other.bottom, t),
            left: self.left.lerp(&other.left, t),
        }
    }
}

impl From<Border> for incular_rendering::Border {
    fn from(b: Border) -> Self {
        incular_rendering::Border::new(b.top.width, b.top.color)
    }
}

/// Directional border with start and end sides.
#[derive(Clone, Copy, Debug, Default, PartialEq, TypedBuilder)]
pub struct BorderDirectional {
    #[builder(default = BorderSide::default())]
    pub top: BorderSide,
    #[builder(default = BorderSide::default())]
    pub start: BorderSide,
    #[builder(default = BorderSide::default())]
    pub end: BorderSide,
    #[builder(default = BorderSide::default())]
    pub bottom: BorderSide,
}

impl BorderDirectional {
    #[must_use]
    pub const fn only(
        top: BorderSide,
        start: BorderSide,
        end: BorderSide,
        bottom: BorderSide,
    ) -> Self {
        Self {
            top,
            start,
            end,
            bottom,
        }
    }

    #[must_use]
    pub const fn resolve(self, direction: incular_config::TextDirection) -> Border {
        match direction {
            incular_config::TextDirection::Ltr => Border {
                top: self.top,
                right: self.end,
                bottom: self.bottom,
                left: self.start,
            },
            incular_config::TextDirection::Rtl => Border {
                top: self.top,
                right: self.start,
                bottom: self.bottom,
                left: self.end,
            },
        }
    }
}

impl incular_core::Lerp for BorderDirectional {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {
            top: self.top.lerp(&other.top, t),
            start: self.start.lerp(&other.start, t),
            end: self.end.lerp(&other.end, t),
            bottom: self.bottom.lerp(&other.bottom, t),
        }
    }
}

/// The geometric shape of a box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BoxShape {
    #[default]
    Rectangle,
    Circle,
}

/// Blur style for shadows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BlurStyle {
    #[default]
    Normal,
    Solid,
    Outer,
    Inner,
}

/// A shadow cast by a box.
#[derive(Clone, Copy, Debug, Default, PartialEq, TypedBuilder)]
pub struct BoxShadow {
    #[builder(default = Color::default())]
    pub color: Color,
    #[builder(default = Offset::default())]
    pub offset: incular_core::Offset,
    #[builder(default = 0.0)]
    pub blur_radius: f32,
    #[builder(default = 0.0)]
    pub spread_radius: f32,
    #[builder(default = BlurStyle::Normal)]
    pub blur_style: BlurStyle,
}

impl BoxShadow {
    #[must_use]
    pub const fn new(
        color: Color,
        offset: incular_core::Offset,
        blur_radius: f32,
        spread_radius: f32,
    ) -> Self {
        Self {
            color,
            offset,
            blur_radius,
            spread_radius,
            blur_style: BlurStyle::Normal,
        }
    }
}

impl incular_core::Lerp for BoxShadow {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {
            color: self.color.lerp(&other.color, t),
            offset: self.offset.lerp(&other.offset, t),
            blur_radius: self.blur_radius.lerp(&other.blur_radius, t).max(0.0),
            spread_radius: self.spread_radius.lerp(&other.spread_radius, t),
            blur_style: if t < 0.5 {
                self.blur_style
            } else {
                other.blur_style
            },
        }
    }
}

/// Gradient tiling mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TileMode {
    #[default]
    Clamp,
    Repeated,
    Mirror,
    Decal,
}

/// An image configuration drawn inside a box decoration.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct DecorationImage {
    pub image: ImageHandle,
    #[builder(default = ImageFit::Cover)]
    pub fit: ImageFit,
    #[builder(default = incular_config::Alignment::CENTER)]
    pub alignment: incular_config::Alignment,
    #[builder(default, setter(strip_option))]
    pub center_slice: Option<incular_core::Rect>,
    #[builder(default = ImageRepeat::NoRepeat)]
    pub repeat: ImageRepeat,
    #[builder(default = false)]
    pub match_text_direction: bool,
    #[builder(default = 1.0)]
    pub scale: f32,
    #[builder(default = 1.0)]
    pub opacity: f32,
}

impl DecorationImage {
    #[must_use]
    pub fn new(image: ImageHandle) -> Self {
        Self {
            image,
            fit: ImageFit::Cover,
            alignment: incular_config::Alignment::CENTER,
            center_slice: None,
            repeat: ImageRepeat::NoRepeat,
            match_text_direction: false,
            scale: 1.0,
            opacity: 1.0,
        }
    }
}

/// An immutable description of how to paint a box.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
pub struct BoxDecoration {
    #[builder(default, setter(strip_option))]
    pub color: Option<Color>,
    #[builder(default, setter(strip_option))]
    pub image: Option<DecorationImage>,
    #[builder(default, setter(strip_option))]
    pub border: Option<Border>,
    #[builder(default, setter(strip_option))]
    pub border_radius: Option<BorderRadius>,
    #[builder(
        default = Vec::new(),
        setter(transform = |shadows: impl IntoIterator<Item = BoxShadow>| {
            shadows.into_iter().collect::<Vec<BoxShadow>>()
        })
    )]
    pub box_shadow: Vec<BoxShadow>,
    #[builder(default, setter(strip_option, into))]
    pub gradient: Option<Brush>,
    #[builder(default, setter(strip_option))]
    pub background_blend_mode: Option<BlendMode>,
    #[builder(default = BoxShape::Rectangle)]
    pub shape: BoxShape,
}

impl BoxDecoration {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    #[must_use]
    pub fn image(mut self, image: DecorationImage) -> Self {
        self.image = Some(image);
        self
    }

    #[must_use]
    pub fn border(mut self, border: Border) -> Self {
        self.border = Some(border);
        self
    }

    #[must_use]
    pub fn border_radius(mut self, radius: BorderRadius) -> Self {
        self.border_radius = Some(radius);
        self
    }

    #[must_use]
    pub fn box_shadow<I>(mut self, shadows: I) -> Self
    where
        I: IntoIterator<Item = BoxShadow>,
    {
        self.box_shadow = shadows.into_iter().collect();
        self
    }

    #[must_use]
    pub fn gradient(mut self, gradient: impl Into<Brush>) -> Self {
        self.gradient = Some(gradient.into());
        self
    }

    #[must_use]
    pub fn background_blend_mode(mut self, mode: BlendMode) -> Self {
        self.background_blend_mode = Some(mode);
        self
    }

    #[must_use]
    pub fn shape(mut self, shape: BoxShape) -> Self {
        self.shape = shape;
        self
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        if self.shape == BoxShape::Circle && self.border_radius.is_some() {
            return false;
        }
        if self.background_blend_mode.is_some()
            && self.color.is_none()
            && self.gradient.is_none()
            && self.image.is_none()
        {
            return false;
        }
        true
    }

    #[must_use]
    pub fn change_impact(&self, other: &Self) -> incular_core::ChangeImpact {
        if self == other {
            return incular_core::ChangeImpact::None;
        }
        if self.border != other.border || self.shape != other.shape {
            incular_core::ChangeImpact::Layout
        } else {
            incular_core::ChangeImpact::Paint
        }
    }

    #[must_use]
    pub fn invalidation(&self, other: &Self) -> incular_core::Invalidation {
        if self == other {
            return incular_core::Invalidation::NONE;
        }
        if self.border != other.border || self.shape != other.shape {
            incular_core::Invalidation::LAYOUT | incular_core::Invalidation::PAINT
        } else {
            incular_core::Invalidation::PAINT
        }
    }
}

impl incular_core::Lerp for BoxDecoration {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            color: match (self.color, other.color) {
                (Some(a), Some(b)) => Some(a.lerp(&b, t)),
                (Some(a), None) => {
                    if t < 0.5 {
                        Some(a)
                    } else {
                        None
                    }
                }
                (None, Some(b)) => {
                    if t >= 0.5 {
                        Some(b)
                    } else {
                        None
                    }
                }
                (None, None) => None,
            },
            image: if t < 0.5 {
                self.image.clone()
            } else {
                other.image.clone()
            },
            border: match (self.border, other.border) {
                (Some(a), Some(b)) => Some(a.lerp(&b, t)),
                (Some(a), None) => {
                    if t < 0.5 {
                        Some(a)
                    } else {
                        None
                    }
                }
                (None, Some(b)) => {
                    if t >= 0.5 {
                        Some(b)
                    } else {
                        None
                    }
                }
                (None, None) => None,
            },
            border_radius: match (self.border_radius, other.border_radius) {
                (Some(a), Some(b)) => Some(a.lerp(&b, t)),
                (Some(a), None) => {
                    if t < 0.5 {
                        Some(a)
                    } else {
                        None
                    }
                }
                (None, Some(b)) => {
                    if t >= 0.5 {
                        Some(b)
                    } else {
                        None
                    }
                }
                (None, None) => None,
            },
            box_shadow: if t < 0.5 {
                self.box_shadow.clone()
            } else {
                other.box_shadow.clone()
            },
            gradient: if t < 0.5 {
                self.gradient.clone()
            } else {
                other.gradient.clone()
            },
            background_blend_mode: if t < 0.5 {
                self.background_blend_mode
            } else {
                other.background_blend_mode
            },
            shape: if t < 0.5 { self.shape } else { other.shape },
        }
    }
}
