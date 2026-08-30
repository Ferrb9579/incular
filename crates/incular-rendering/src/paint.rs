use crate::effects::{BlendMode, ColorFilter};
use crate::geometry::CornerRadii;
use crate::gradients::{Brush, Shader};
use incular_core::{Color, Lerp, Offset};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Border {
    pub width: f32,
    pub color: Color,
}
impl Border {
    #[must_use]
    pub fn new(width: f32, color: Color) -> Self {
        Self {
            width: width.max(0.),
            color,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Decoration {
    pub background: Option<Brush>,
    pub border: Option<Border>,
    pub border_radius: CornerRadii,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    pub width: f32,
    pub cap: LineCap,
    pub join: LineJoin,
    pub miter_limit: f32,
}
impl Default for Stroke {
    fn default() -> Self {
        Self {
            width: 1.,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            miter_limit: 4.,
        }
    }
}

/// Whether a [`Paint`] covers the interior or the outline of a path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PaintStyle {
    #[default]
    Fill,
    Stroke,
}

/// A renderer-neutral paint description.
///
/// `Paint` is deliberately a small immutable builder around the authoritative
/// [`Brush`], [`Stroke`], [`BlendMode`], and [`ColorFilter`] types. It does not
/// introduce a second display command model; [`Canvas::draw_path`] lowers it
/// directly to the existing fill/stroke commands.
#[derive(Clone, Debug, PartialEq)]
pub struct Paint {
    brush: Brush,
    style: PaintStyle,
    stroke: Stroke,
    blend_mode: BlendMode,
    color_filter: Option<ColorFilter>,
}

impl Default for Paint {
    fn default() -> Self {
        Self {
            brush: Brush::Solid(Color::BLACK),
            style: PaintStyle::Fill,
            stroke: Stroke::default(),
            blend_mode: BlendMode::SrcOver,
            color_filter: None,
        }
    }
}

impl Paint {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn brush(mut self, brush: impl Into<Brush>) -> Self {
        self.brush = brush.into();
        self
    }

    #[must_use]
    pub fn shader(self, shader: impl Into<Shader>) -> Self {
        self.brush(shader)
    }

    #[must_use]
    pub fn color(self, color: Color) -> Self {
        self.brush(color)
    }

    #[must_use]
    pub fn style(mut self, style: PaintStyle) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    pub fn fill(self) -> Self {
        self.style(PaintStyle::Fill)
    }

    #[must_use]
    pub fn stroke(self) -> Self {
        self.style(PaintStyle::Stroke)
    }

    #[must_use]
    pub fn stroke_settings(mut self, stroke: Stroke) -> Self {
        self.stroke = Stroke {
            width: if stroke.width.is_finite() {
                stroke.width.max(0.)
            } else {
                0.
            },
            cap: stroke.cap,
            join: stroke.join,
            miter_limit: if stroke.miter_limit.is_finite() {
                stroke.miter_limit.max(0.)
            } else {
                0.
            },
        };
        self
    }

    #[must_use]
    pub fn stroke_width(mut self, width: f32) -> Self {
        self.stroke.width = if width.is_finite() { width.max(0.) } else { 0. };
        self
    }

    #[must_use]
    pub fn blend_mode(mut self, blend_mode: BlendMode) -> Self {
        self.blend_mode = blend_mode;
        self
    }

    #[must_use]
    pub fn color_filter(mut self, color_filter: Option<ColorFilter>) -> Self {
        self.color_filter = color_filter;
        self
    }

    #[must_use]
    pub fn brush_value(&self) -> &Brush {
        &self.brush
    }

    #[must_use]
    pub const fn style_value(&self) -> PaintStyle {
        self.style
    }

    #[must_use]
    pub const fn stroke_value(&self) -> Stroke {
        self.stroke
    }

    #[must_use]
    pub const fn blend_mode_value(&self) -> BlendMode {
        self.blend_mode
    }

    #[must_use]
    pub fn color_filter_value(&self) -> Option<ColorFilter> {
        self.color_filter
    }
}

/// Canonical single-shadow value used by text and other paint-producing
/// domains. Subsystems should use this type instead of defining a parallel
/// color/offset/blur tuple.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shadow {
    pub color: Color,
    pub offset: Offset,
    pub blur_radius: f32,
}

impl Shadow {
    #[must_use]
    pub const fn new(color: Color, offset: Offset, blur_radius: f32) -> Self {
        Self {
            color,
            offset,
            blur_radius: if blur_radius.is_finite() && blur_radius > 0. {
                blur_radius
            } else {
                0.
            },
        }
    }
}

impl Lerp for Shadow {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Self::new(
            self.color.lerp(&other.color, t),
            self.offset.lerp(&other.offset, t),
            self.blur_radius.lerp(&other.blur_radius, t),
        )
    }
}
