//! Ambient configuration and inherited environment widgets.

use incular_config::TextDirection;
use incular_core::Color;
use incular_scroll::{ScrollController, ScrollPhysics};
use incular_text::TextStyle;
use std::rc::Rc;

use crate::{BuildContext, LayoutBuilder, Widget};

/// Directionality provides ambient text direction (`Ltr` or `Rtl`) to its subtree.
#[derive(Clone, Debug, PartialEq)]
pub struct Directionality {
    text_direction: TextDirection,
    child: Widget,
}

impl Directionality {
    #[must_use]
    pub fn new(text_direction: TextDirection, child: impl Into<Widget>) -> Self {
        Self {
            text_direction,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn text_direction(&self) -> TextDirection {
        self.text_direction
    }
}

impl From<Directionality> for Widget {
    fn from(value: Directionality) -> Self {
        value.child
    }
}

/// Sets the default [`TextStyle`] for descendant [`Text`](crate::Text) widgets.
#[derive(Clone, Debug, PartialEq)]
pub struct DefaultTextStyle {
    style: TextStyle,
    child: Widget,
}

impl DefaultTextStyle {
    #[must_use]
    pub fn new(style: TextStyle, child: impl Into<Widget>) -> Self {
        Self {
            style,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn style(&self) -> &TextStyle {
        &self.style
    }
}

impl From<DefaultTextStyle> for Widget {
    fn from(value: DefaultTextStyle) -> Self {
        value.child
    }
}

/// Defines default color, size, and opacity for descendant [`Icon`](crate::Icon) widgets.
#[derive(Clone, Debug, PartialEq)]
pub struct IconTheme {
    color: Option<Color>,
    size: Option<f32>,
    opacity: Option<f32>,
    child: Widget,
}

impl IconTheme {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            color: None,
            size: None,
            opacity: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    #[must_use]
    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size.max(0.0));
        self
    }

    #[must_use]
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = Some(opacity.clamp(0.0, 1.0));
        self
    }
}

impl From<IconTheme> for Widget {
    fn from(value: IconTheme) -> Self {
        value.child
    }
}

/// Device / viewport orientation.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Orientation {
    Portrait,
    Landscape,
}

/// Builds a widget subtree depending on the parent's orientation.
#[derive(Clone)]
#[allow(clippy::type_complexity)]
pub struct OrientationBuilder {
    builder: Rc<dyn Fn(&BuildContext, Orientation) -> Widget>,
}

impl OrientationBuilder {
    #[must_use]
    pub fn new<W>(builder: impl Fn(&BuildContext, Orientation) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            builder: Rc::new(move |ctx, orient| builder(ctx, orient).into()),
        }
    }
}

impl From<OrientationBuilder> for Widget {
    fn from(value: OrientationBuilder) -> Self {
        let builder = value.builder;
        LayoutBuilder::new(move |constraints| {
            let orientation = if constraints.max_width > constraints.max_height {
                Orientation::Landscape
            } else {
                Orientation::Portrait
            };
            let dummy_ctx = BuildContext;
            builder(&dummy_ctx, orientation)
        })
        .into()
    }
}

/// Controls ambient scroll physics and behavior for descendant scroll views.
#[derive(Clone, Debug, PartialEq)]
pub struct ScrollConfiguration {
    physics: Option<ScrollPhysics>,
    child: Widget,
}

impl ScrollConfiguration {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            physics: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn physics(mut self, physics: ScrollPhysics) -> Self {
        self.physics = Some(physics);
        self
    }
}

impl From<ScrollConfiguration> for Widget {
    fn from(value: ScrollConfiguration) -> Self {
        value.child
    }
}

/// Associates a [`ScrollController`] with the subtree as the default primary scroll controller.
#[derive(Clone, Debug, PartialEq)]
pub struct PrimaryScrollController {
    controller: ScrollController,
    automatically_inherit_for_platforms: bool,
    child: Widget,
}

impl PrimaryScrollController {
    #[must_use]
    pub fn new(controller: ScrollController, child: impl Into<Widget>) -> Self {
        Self {
            controller,
            automatically_inherit_for_platforms: true,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn controller(&self) -> &ScrollController {
        &self.controller
    }
}

impl From<PrimaryScrollController> for Widget {
    fn from(value: PrimaryScrollController) -> Self {
        value.child
    }
}

/// Enables or disables animation ticking for its subtree.
#[derive(Clone, Debug, PartialEq)]
pub struct TickerMode {
    enabled: bool,
    child: Widget,
}

impl TickerMode {
    #[must_use]
    pub fn new(enabled: bool, child: impl Into<Widget>) -> Self {
        Self {
            enabled,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl From<TickerMode> for Widget {
    fn from(value: TickerMode) -> Self {
        value.child
    }
}

/// Marks content as sensitive to obscure it from window sharing, screen recording, and diagnostics.
#[derive(Clone, Debug, PartialEq)]
pub struct SensitiveContent {
    sensitive: bool,
    child: Widget,
}

impl SensitiveContent {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            sensitive: true,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn sensitive(mut self, sensitive: bool) -> Self {
        self.sensitive = sensitive;
        self
    }
}

impl From<SensitiveContent> for Widget {
    fn from(value: SensitiveContent) -> Self {
        value.child
    }
}

/// Boundary host coordinating sensitive content obscuration.
#[derive(Clone, Debug, PartialEq)]
pub struct SensitiveContentHost {
    child: Widget,
}

impl SensitiveContentHost {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<SensitiveContentHost> for Widget {
    fn from(value: SensitiveContentHost) -> Self {
        value.child
    }
}

/// Isolates inherited widget lookups across boundaries.
#[derive(Clone, Debug, PartialEq)]
pub struct LookupBoundary {
    child: Widget,
}

impl LookupBoundary {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<LookupBoundary> for Widget {
    fn from(value: LookupBoundary) -> Self {
        value.child
    }
}
