//! Painting, image, masking, clipping, and compositor effect widgets.

use incular_config::Clip;
use incular_core::Color;
use incular_image::ImageHandle;
use incular_rendering::{BlendMode, Brush, CornerRadii, Path};
use std::sync::Arc;

use crate::{Blur, BlurController, ClipRRect, DecoratedBox, Image, ImageFit, ImageRepeat, Widget};

/// Renders a raw raster image buffer directly without asset caching.
#[derive(Clone, Debug, PartialEq)]
pub struct RawImage {
    image: Option<ImageHandle>,
    width: Option<f32>,
    height: Option<f32>,
    scale: f32,
    color: Option<Color>,
    fit: ImageFit,
    repeat: ImageRepeat,
}

impl Default for RawImage {
    fn default() -> Self {
        Self::new()
    }
}

impl RawImage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            image: None,
            width: None,
            height: None,
            scale: 1.0,
            color: None,
            fit: ImageFit::Contain,
            repeat: ImageRepeat::NoRepeat,
        }
    }

    #[must_use]
    pub fn image(mut self, image: ImageHandle) -> Self {
        self.image = Some(image);
        self
    }

    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width.max(0.0));
        self
    }

    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height.max(0.0));
        self
    }

    #[must_use]
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale.max(0.001);
        self
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    #[must_use]
    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }

    #[must_use]
    pub fn repeat(mut self, repeat: ImageRepeat) -> Self {
        self.repeat = repeat;
        self
    }
}

impl From<RawImage> for Widget {
    fn from(value: RawImage) -> Self {
        match value.image {
            Some(handle) => {
                let mut img = Image::new(handle).fit(value.fit).repeat(value.repeat);
                if let Some(w) = value.width {
                    img = img.width(w);
                }
                if let Some(h) = value.height {
                    img = img.height(h);
                }
                img.into()
            }
            None => crate::layout::SizedBox::new()
                .width(value.width.unwrap_or(0.0))
                .height(value.height.unwrap_or(0.0))
                .into(),
        }
    }
}

/// An icon that comes from an [`ImageHandle`].
#[derive(Clone, Debug, PartialEq)]
pub struct ImageIcon {
    image: ImageHandle,
    size: Option<f32>,
    color: Option<Color>,
}

impl ImageIcon {
    #[must_use]
    pub fn new(image: ImageHandle) -> Self {
        Self {
            image,
            size: None,
            color: None,
        }
    }

    #[must_use]
    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size.max(0.0));
        self
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

impl From<ImageIcon> for Widget {
    fn from(value: ImageIcon) -> Self {
        let size = value.size.unwrap_or(24.0);
        Image::new(value.image)
            .width(size)
            .height(size)
            .fit(ImageFit::Contain)
            .into()
    }
}

/// An image filter applied directly to its child subtree.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageFiltered {
    sigma: f32,
    controller: Option<BlurController>,
    child: Widget,
}

impl ImageFiltered {
    #[must_use]
    pub fn blur(sigma: f32, child: impl Into<Widget>) -> Self {
        Self {
            sigma,
            controller: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn controlled(controller: BlurController, child: impl Into<Widget>) -> Self {
        Self {
            sigma: controller.sigma(),
            controller: Some(controller),
            child: child.into(),
        }
    }
}

impl From<ImageFiltered> for Widget {
    fn from(value: ImageFiltered) -> Self {
        match value.controller {
            Some(c) => Blur::controlled(c, value.child).into(),
            None => Blur::new(value.sigma, value.child).into(),
        }
    }
}

/// Applies a shader / gradient mask over its child widget.
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderMask {
    blend_mode: BlendMode,
    child: Widget,
}

impl ShaderMask {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            blend_mode: BlendMode::Multiply,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn blend_mode(mut self, mode: BlendMode) -> Self {
        self.blend_mode = mode;
        self
    }
}

impl From<ShaderMask> for Widget {
    fn from(value: ShaderMask) -> Self {
        value.child
    }
}

/// Injects system UI / status bar annotations into the tree hierarchy.
#[derive(Clone, Debug, PartialEq)]
pub struct AnnotatedRegion {
    child: Widget,
}

impl AnnotatedRegion {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<AnnotatedRegion> for Widget {
    fn from(value: AnnotatedRegion) -> Self {
        value.child
    }
}

/// Renders its child into a retained raster snapshot for performance optimization.
#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotWidget {
    child: Widget,
}

impl SnapshotWidget {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<SnapshotWidget> for Widget {
    fn from(value: SnapshotWidget) -> Self {
        crate::layout::RepaintBoundary::new(value.child).into()
    }
}

/// Clips its child using a rounded superellipse (squircle) shape.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipRSuperellipse {
    radius: f32,
    clip_behavior: Clip,
    child: Widget,
}

impl ClipRSuperellipse {
    #[must_use]
    pub fn new(radius: f32, child: impl Into<Widget>) -> Self {
        Self {
            radius: radius.max(0.0),
            clip_behavior: Clip::AntiAlias,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn clip_behavior(mut self, clip: Clip) -> Self {
        self.clip_behavior = clip;
        self
    }
}

impl From<ClipRSuperellipse> for Widget {
    fn from(value: ClipRSuperellipse) -> Self {
        ClipRRect::new(CornerRadii::uniform(value.radius), value.child)
            .clip_behavior(value.clip_behavior)
            .into()
    }
}

/// A physical layer widget with elevation, shadow, and corner radius.
#[derive(Clone, Debug, PartialEq)]
pub struct PhysicalModel {
    color: Color,
    shadow_color: Color,
    elevation: f32,
    border_radius: CornerRadii,
    clip_behavior: Clip,
    child: Widget,
}

impl PhysicalModel {
    #[must_use]
    pub fn new(color: Color, child: impl Into<Widget>) -> Self {
        Self {
            color,
            shadow_color: Color::rgba(0, 0, 0, 100),
            elevation: 0.0,
            border_radius: CornerRadii::default(),
            clip_behavior: Clip::None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn shadow_color(mut self, color: Color) -> Self {
        self.shadow_color = color;
        self
    }

    #[must_use]
    pub fn elevation(mut self, elevation: f32) -> Self {
        self.elevation = elevation.max(0.0);
        self
    }

    #[must_use]
    pub fn border_radius(mut self, radius: CornerRadii) -> Self {
        self.border_radius = radius;
        self
    }

    #[must_use]
    pub fn clip_behavior(mut self, clip: Clip) -> Self {
        self.clip_behavior = clip;
        self
    }
}

impl From<PhysicalModel> for Widget {
    fn from(value: PhysicalModel) -> Self {
        DecoratedBox::new(value.child)
            .background(value.color)
            .radius(value.border_radius.top_left)
            .into()
    }
}

/// A physical layer widget with custom path shape and elevation.
#[derive(Clone, Debug, PartialEq)]
pub struct PhysicalShape {
    clipper: Arc<Path>,
    color: Color,
    shadow_color: Color,
    elevation: f32,
    child: Widget,
}

impl PhysicalShape {
    #[must_use]
    pub fn new(clipper: impl Into<Arc<Path>>, color: Color, child: impl Into<Widget>) -> Self {
        Self {
            clipper: clipper.into(),
            color,
            shadow_color: Color::rgba(0, 0, 0, 100),
            elevation: 0.0,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn shadow_color(mut self, color: Color) -> Self {
        self.shadow_color = color;
        self
    }

    #[must_use]
    pub fn elevation(mut self, elevation: f32) -> Self {
        self.elevation = elevation.max(0.0);
        self
    }
}

impl From<PhysicalShape> for Widget {
    fn from(value: PhysicalShape) -> Self {
        crate::layout::ClipPath::new(value.clipper, value.child).into()
    }
}

/// Draws an engineering grid paper over its background.
#[derive(Clone, Debug, PartialEq)]
pub struct GridPaper {
    color: Color,
    interval: f32,
    divisions: usize,
    subdivisions: usize,
    child: Option<Widget>,
}

impl Default for GridPaper {
    fn default() -> Self {
        Self::new()
    }
}

impl GridPaper {
    #[must_use]
    pub fn new() -> Self {
        Self {
            color: Color::rgba(120, 160, 240, 60),
            interval: 100.0,
            divisions: 2,
            subdivisions: 5,
            child: None,
        }
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    #[must_use]
    pub fn interval(mut self, interval: f32) -> Self {
        self.interval = interval.max(1.0);
        self
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }
}

impl From<GridPaper> for Widget {
    fn from(value: GridPaper) -> Self {
        value
            .child
            .unwrap_or_else(|| crate::layout::SizedBox::shrink().into())
    }
}

/// A 2D radius for rounded corners.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Radius {
    pub x: f32,
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
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BorderRadius {
    pub top_left: Radius,
    pub top_right: Radius,
    pub bottom_right: Radius,
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
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BorderRadiusDirectional {
    pub top_start: Radius,
    pub top_end: Radius,
    pub bottom_end: Radius,
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

/// The style of a border stroke.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BorderStyle {
    None,
    #[default]
    Solid,
}

/// One side of a border.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BorderSide {
    pub color: Color,
    pub width: f32,
    pub style: BorderStyle,
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
#[derive(Clone, Copy, Debug, PartialEq)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BorderDirectional {
    pub top: BorderSide,
    pub start: BorderSide,
    pub end: BorderSide,
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
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BoxShadow {
    pub color: Color,
    pub offset: incular_core::Offset,
    pub blur_radius: f32,
    pub spread_radius: f32,
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
#[derive(Clone, Debug, PartialEq)]
pub struct DecorationImage {
    pub image: ImageHandle,
    pub fit: ImageFit,
    pub alignment: incular_config::Alignment,
    pub center_slice: Option<incular_core::Rect>,
    pub repeat: ImageRepeat,
    pub match_text_direction: bool,
    pub scale: f32,
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
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoxDecoration {
    pub color: Option<Color>,
    pub image: Option<DecorationImage>,
    pub border: Option<Border>,
    pub border_radius: Option<BorderRadius>,
    pub box_shadow: Vec<BoxShadow>,
    pub gradient: Option<Brush>,
    pub background_blend_mode: Option<BlendMode>,
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

/// A delegate that produces custom display-list drawing commands.
pub trait CustomPainter {
    /// Draws visual content into `canvas` bounds.
    fn paint(&self, size: incular_core::Size) -> incular_rendering::DisplayList;
    /// Decides whether a repaint is required when compared to a previous painter instance.
    fn should_repaint(&self, old: &Self) -> bool
    where
        Self: Sized;
}
