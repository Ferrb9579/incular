//! Diagnostics, debugging, keep-alive, and framework utility widgets.

use crate::{BoxShadow, Center, ColoredBox, Column, Text, Widget};
use incular_config::TextDirection;
use incular_core::{Color, Offset, Rect, Size};
use incular_text::{FontWeight, TextStyle};
use std::f32::consts::FRAC_PI_4;
use typed_builder::TypedBuilder;

/// An expandable/collapsible container widget.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct Expansible {
    #[builder(default = false)]
    expanded: bool,
    #[builder(setter(into))]
    header: Widget,
    #[builder(setter(into))]
    body: Widget,
}

impl Expansible {
    #[must_use]
    pub fn new(header: impl Into<Widget>, body: impl Into<Widget>) -> Self {
        Self {
            expanded: false,
            header: header.into(),
            body: body.into(),
        }
    }

    #[must_use]
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }
}

impl From<Expansible> for Widget {
    fn from(value: Expansible) -> Self {
        if value.expanded {
            Column::new([value.header, value.body]).into()
        } else {
            value.header
        }
    }
}

/// Displays framework build or layout runtime errors cleanly on screen.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct ErrorWidget {
    #[builder(default = String::new(), setter(into))]
    message: String,
}

impl Default for ErrorWidget {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl ErrorWidget {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<ErrorWidget> for Widget {
    fn from(value: ErrorWidget) -> Self {
        ColoredBox::new(
            Color::rgba(180, 40, 40, 255),
            Center::new(Text::new(format!("Error: {}", value.message))),
        )
        .into()
    }
}

/// Where a [`Banner`] is positioned relative to the ambient layout direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BannerLocation {
    #[default]
    TopStart,
    TopEnd,
    BottomStart,
    BottomEnd,
}

const BANNER_OFFSET: f32 = 40.0;
const BANNER_HEIGHT: f32 = 12.0;
const BANNER_BOTTOM_OFFSET: f32 = BANNER_OFFSET + std::f32::consts::FRAC_1_SQRT_2 * BANNER_HEIGHT;

fn default_banner_color() -> Color {
    Color::rgba(183, 28, 28, 160)
}

fn default_banner_text_style() -> TextStyle {
    TextStyle::new()
        .color(Color::WHITE)
        .font_size(BANNER_HEIGHT * 0.85)
        .font_weight(FontWeight::W900)
        .height(Some(1.0))
}

fn default_banner_shadow() -> BoxShadow {
    BoxShadow::new(Color::rgba(0, 0, 0, 127), Offset::ZERO, 6.0, 0.0)
}

/// The local geometry used by the retained banner painter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BannerGeometry {
    pub translation: Offset,
    pub rotation: f32,
    pub rect: Rect,
}

/// Computes Flutter's fixed 45-degree corner geometry in logical pixels.
pub(crate) fn banner_geometry(
    size: Size,
    location: BannerLocation,
    layout_direction: TextDirection,
) -> BannerGeometry {
    let x = match (layout_direction, location) {
        (TextDirection::Rtl, BannerLocation::TopStart) => size.width,
        (TextDirection::Ltr, BannerLocation::TopStart) => 0.0,
        (TextDirection::Rtl, BannerLocation::TopEnd) => 0.0,
        (TextDirection::Ltr, BannerLocation::TopEnd) => size.width,
        (TextDirection::Rtl, BannerLocation::BottomStart) => size.width - BANNER_BOTTOM_OFFSET,
        (TextDirection::Ltr, BannerLocation::BottomStart) => BANNER_BOTTOM_OFFSET,
        (TextDirection::Rtl, BannerLocation::BottomEnd) => BANNER_BOTTOM_OFFSET,
        (TextDirection::Ltr, BannerLocation::BottomEnd) => size.width - BANNER_BOTTOM_OFFSET,
    };
    let y = match location {
        BannerLocation::TopStart | BannerLocation::TopEnd => 0.0,
        BannerLocation::BottomStart | BannerLocation::BottomEnd => {
            size.height - BANNER_BOTTOM_OFFSET
        }
    };
    let rotation = match (layout_direction, location) {
        (TextDirection::Rtl, BannerLocation::TopStart | BannerLocation::BottomEnd)
        | (TextDirection::Ltr, BannerLocation::BottomStart | BannerLocation::TopEnd) => FRAC_PI_4,
        (TextDirection::Ltr, BannerLocation::TopStart | BannerLocation::BottomEnd)
        | (TextDirection::Rtl, BannerLocation::BottomStart | BannerLocation::TopEnd) => -FRAC_PI_4,
    };
    BannerGeometry {
        translation: Offset::new(x, y),
        rotation,
        rect: Rect::from_origin_size(
            Offset::new(-BANNER_OFFSET, BANNER_OFFSET - BANNER_HEIGHT),
            Size::new(BANNER_OFFSET * 2.0, BANNER_HEIGHT),
        ),
    }
}

/// Converts Flutter's public blur-radius convention to the sigma used by the
/// renderer-neutral compositor.
pub(crate) fn banner_shadow_sigma(blur_radius: f32) -> f32 {
    let radius = if blur_radius.is_finite() {
        blur_radius.max(0.0)
    } else {
        0.0
    };
    radius * 0.577_350_26 + 0.5
}

/// Displays a diagonal message above the corner of another widget.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct Banner {
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(setter(into))]
    message: String,
    #[builder(default, setter(strip_option))]
    text_direction: Option<TextDirection>,
    location: BannerLocation,
    #[builder(default, setter(strip_option))]
    layout_direction: Option<TextDirection>,
    #[builder(default = default_banner_color())]
    color: Color,
    #[builder(default = default_banner_text_style())]
    text_style: TextStyle,
    #[builder(default = default_banner_shadow())]
    shadow: BoxShadow,
}

impl Banner {
    /// Creates a banner with Flutter's documented diagnostic defaults.
    #[must_use]
    pub fn new(message: impl Into<String>, location: BannerLocation) -> Self {
        Self {
            child: None,
            message: message.into(),
            text_direction: None,
            location,
            layout_direction: None,
            color: default_banner_color(),
            text_style: default_banner_text_style(),
            shadow: default_banner_shadow(),
        }
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn text_direction(mut self, direction: TextDirection) -> Self {
        self.text_direction = Some(direction);
        self
    }

    #[must_use]
    pub fn layout_direction(mut self, direction: TextDirection) -> Self {
        self.layout_direction = Some(direction);
        self
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    #[must_use]
    pub fn text_style(mut self, style: TextStyle) -> Self {
        self.text_style = style;
        self
    }

    #[must_use]
    pub fn shadow(mut self, shadow: BoxShadow) -> Self {
        self.shadow = shadow;
        self
    }

    #[must_use]
    pub fn location(&self) -> BannerLocation {
        self.location
    }

    #[must_use]
    pub fn configured_text_direction(&self) -> Option<TextDirection> {
        self.text_direction
    }

    #[must_use]
    pub fn configured_layout_direction(&self) -> Option<TextDirection> {
        self.layout_direction
    }

    #[must_use]
    pub fn color_value(&self) -> Color {
        self.color
    }

    #[must_use]
    pub fn text_style_value(&self) -> &TextStyle {
        &self.text_style
    }

    #[must_use]
    pub fn shadow_value(&self) -> BoxShadow {
        self.shadow
    }
}

impl From<Banner> for Widget {
    fn from(value: Banner) -> Self {
        Widget::from_kind(crate::WidgetKind::Banner {
            message: value.message,
            text_direction: value.text_direction,
            location: value.location,
            layout_direction: value.layout_direction,
            color: value.color,
            text_style: value.text_style,
            shadow: value.shadow,
            child: value.child.map(Box::new),
        })
    }
}

/// Displays the `DEBUG` banner only when the crate is built with assertions.
///
/// The widget is opt-in: applications must place it in their tree (or enable
/// the corresponding `MaterialApp` policy) before any banner is rendered.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct CheckedModeBanner {
    #[builder(setter(into))]
    child: Widget,
}

impl CheckedModeBanner {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }

    /// Returns whether this build includes checked/debug assertions.
    #[must_use]
    pub const fn is_enabled() -> bool {
        cfg!(debug_assertions)
    }
}

impl From<CheckedModeBanner> for Widget {
    fn from(value: CheckedModeBanner) -> Self {
        if cfg!(debug_assertions) {
            Banner::new("DEBUG", BannerLocation::TopEnd)
                .text_direction(TextDirection::Ltr)
                .layout_direction(TextDirection::Ltr)
                .child(value.child)
                .into()
        } else {
            value.child
        }
    }
}

/// Mount point for the runtime performance overlay.
///
/// The widget itself is intentionally inert until the application installs a
/// runtime overlay with `Runtime::install_performance_overlay`. Keeping the
/// mount point as a real keyed retained node means installation updates one
/// element instead of rebuilding the application root.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PerformanceOverlay;

impl PerformanceOverlay {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl From<PerformanceOverlay> for Widget {
    fn from(_: PerformanceOverlay) -> Self {
        crate::tree::performance_overlay_placeholder()
    }
}
