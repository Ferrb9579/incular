//! Painting and placeholder layout primitives.

use incular_core::{Color, Size};
use incular_rendering::DisplayList;
use typed_builder::TypedBuilder;

use super::SizedBox;
use crate::{DecoratedBox, Widget, WidgetKind};

/// Paints a solid background color behind a child widget.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct ColoredBox {
    #[builder(setter(into))]
    color: Color,
    #[builder(default, setter(strip_option, into))]
    pub(super) child: Option<Widget>,
}

impl ColoredBox {
    /// Creates a ColoredBox wrapping a child with a background color.
    #[must_use]
    pub fn new(color: Color, child: impl Into<Widget>) -> Self {
        Self {
            color,
            child: Some(child.into()),
        }
    }

    /// Sets the background color.
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Sets the child widget.
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }
}

impl From<ColoredBox> for Widget {
    fn from(value: ColoredBox) -> Self {
        let child = value.child.unwrap_or_else(|| SizedBox::shrink().into());
        DecoratedBox::new(child).background(value.color).into()
    }
}

/// Isolates display list painting and caching into a dedicated layer.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct RepaintBoundary {
    #[builder(setter(into))]
    child: Widget,
}

impl RepaintBoundary {
    /// Creates a RepaintBoundary.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<RepaintBoundary> for Widget {
    fn from(value: RepaintBoundary) -> Self {
        Widget::from_kind(WidgetKind::RepaintBoundary {
            child: Box::new(value.child),
        })
    }
}

/// Paints an explicit [`DisplayList`] with a fixed size.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct CustomPaint {
    size: Size,
    display_list: DisplayList,
}

impl CustomPaint {
    /// Creates a CustomPaint widget.
    #[must_use]
    pub fn new(size: Size, display_list: DisplayList) -> Self {
        Self { size, display_list }
    }

    /// Creates a CustomPaint widget from a custom painter.
    #[must_use]
    pub fn from_painter(size: Size, painter: impl crate::CustomPainter) -> Self {
        let display_list = painter.paint(size);
        Self { size, display_list }
    }
}

impl From<CustomPaint> for Widget {
    fn from(value: CustomPaint) -> Self {
        Widget::from_kind(WidgetKind::CustomPaint {
            size: value.size,
            display_list: value.display_list,
        })
    }
}

/// A placeholder box with cross lines and fallback sizing.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct Placeholder {
    #[builder(default = Color::rgba(117, 117, 117, 255))]
    color: Color,
    #[builder(default = 2.0, setter(transform = |width: f32| width.max(0.1)))]
    stroke_width: f32,
    #[builder(default = 400.0, setter(transform = |width: f32| width.max(0.0)))]
    fallback_width: f32,
    #[builder(default = 400.0, setter(transform = |height: f32| height.max(0.0)))]
    fallback_height: f32,
}

impl Default for Placeholder {
    fn default() -> Self {
        Self::new()
    }
}

impl Placeholder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            color: Color::rgba(117, 117, 117, 255),
            stroke_width: 2.0,
            fallback_width: 400.0,
            fallback_height: 400.0,
        }
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    #[must_use]
    pub fn stroke_width(mut self, width: f32) -> Self {
        self.stroke_width = width.max(0.1);
        self
    }

    #[must_use]
    pub fn fallback_width(mut self, width: f32) -> Self {
        self.fallback_width = width.max(0.0);
        self
    }

    #[must_use]
    pub fn fallback_height(mut self, height: f32) -> Self {
        self.fallback_height = height.max(0.0);
        self
    }
}

impl From<Placeholder> for Widget {
    fn from(value: Placeholder) -> Self {
        SizedBox::new()
            .width(value.fallback_width)
            .height(value.fallback_height)
            .child(ColoredBox::new(
                Color::rgba(value.color.red, value.color.green, value.color.blue, 32),
                SizedBox::shrink(),
            ))
            .into()
    }
}
