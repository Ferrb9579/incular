//! Material density, tap-target, and retained surface primitives.

use incular_config::{Clip, Constraints, EdgeInsets};
use incular_core::{Color, Offset, Size};
use incular_text::TextStyle;
use incular_widgets::internal::DropShadow;
use incular_widgets::{BorderRadius, BoxDecoration, Container, DefaultTextStyle, Widget};
use std::time::Duration;
use typed_builder::TypedBuilder;

use super::helpers::mix;
use super::theme_data::Theme;

/// Material density adjustment used by layout and hit-target calculations.
#[derive(Clone, Copy, Debug, PartialEq, TypedBuilder)]
pub struct VisualDensity {
    #[builder(default = 0.0)]
    pub horizontal: f32,
    #[builder(default = 0.0)]
    pub vertical: f32,
}

impl VisualDensity {
    pub const STANDARD: Self = Self::new(0.0, 0.0);
    pub const COMFORTABLE: Self = Self::new(-1.0, -1.0);
    pub const COMPACT: Self = Self::new(-2.0, -2.0);

    #[must_use]
    pub const fn new(horizontal: f32, vertical: f32) -> Self {
        Self {
            horizontal,
            vertical,
        }
    }

    #[must_use]
    pub fn base_size_adjustment(self) -> Offset {
        // Incular's `Size` intentionally rejects negative extents, while
        // Flutter's density adjustment is signed. `Offset` is the matching
        // renderer-neutral signed pair and keeps compact density lossless.
        Offset::new(self.horizontal * 4.0, self.vertical * 4.0)
    }

    #[must_use]
    pub fn effective_constraints(self, constraints: Constraints) -> Constraints {
        let adjustment = self.base_size_adjustment();
        Constraints::new(
            (constraints.min_width + adjustment.x).max(0.0),
            (constraints.max_width + adjustment.x).max(0.0),
            (constraints.min_height + adjustment.y).max(0.0),
            (constraints.max_height + adjustment.y).max(0.0),
        )
    }

    #[must_use]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self::new(
            self.horizontal + (other.horizontal - self.horizontal) * t,
            self.vertical + (other.vertical - self.vertical) * t,
        )
    }
}

impl Default for VisualDensity {
    fn default() -> Self {
        Self::STANDARD
    }
}

/// Material's minimum interactive target policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MaterialTapTargetSize {
    #[default]
    Padded,
    ShrinkWrap,
}

impl MaterialTapTargetSize {
    #[must_use]
    pub fn minimum_size(self) -> Size {
        match self {
            Self::Padded => Size::new(48.0, 48.0),
            Self::ShrinkWrap => Size::ZERO,
        }
    }
}

/// Material surface kinds.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MaterialType {
    #[default]
    Canvas,
    Card,
    Circle,
    Button,
    Transparency,
}

/// A retained Material surface. Elevation is lowered to the shared shadow
/// renderer, so changing it affects paint/composite output rather than being a
/// passive style field.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct Material {
    #[builder(default = MaterialType::Canvas)]
    pub material_type: MaterialType,
    #[builder(default = Color::TRANSPARENT)]
    pub color: Color,
    #[builder(default = Color::rgba(0, 0, 0, 100))]
    pub shadow_color: Color,
    #[builder(default, setter(strip_option, into))]
    pub surface_tint_color: Option<Color>,
    #[builder(default = 0.0, setter(transform = |value: f32| value.max(0.0)))]
    pub elevation: f32,
    #[builder(default = BorderRadius::ZERO)]
    pub border_radius: BorderRadius,
    #[builder(default, setter(strip_option))]
    pub padding: Option<EdgeInsets>,
    #[builder(default = Clip::None)]
    pub clip_behavior: Clip,
    #[builder(default = Duration::from_millis(200))]
    pub animation_duration: Duration,
    #[builder(setter(into))]
    pub child: Widget,
}

impl Material {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            material_type: MaterialType::Canvas,
            color: Color::TRANSPARENT,
            shadow_color: Color::rgba(0, 0, 0, 100),
            surface_tint_color: None,
            elevation: 0.0,
            border_radius: BorderRadius::ZERO,
            padding: None,
            clip_behavior: Clip::None,
            animation_duration: Duration::from_millis(200),
            child: child.into(),
        }
    }

    #[must_use]
    pub fn material_type(mut self, value: MaterialType) -> Self {
        self.material_type = value;
        self
    }
    #[must_use]
    pub fn color(mut self, value: Color) -> Self {
        self.color = value;
        self
    }
    #[must_use]
    pub fn shadow_color(mut self, value: Color) -> Self {
        self.shadow_color = value;
        self
    }
    #[must_use]
    pub fn surface_tint_color(mut self, value: Color) -> Self {
        self.surface_tint_color = Some(value);
        self
    }
    #[must_use]
    pub fn elevation(mut self, value: f32) -> Self {
        self.elevation = value.max(0.0);
        self
    }
    #[must_use]
    pub fn border_radius(mut self, value: BorderRadius) -> Self {
        self.border_radius = value;
        self
    }
    /// Sets a uniform corner radius. This is a convenience alias for the
    /// canonical [`Material::border_radius`] builder.
    #[must_use]
    pub fn radius(self, value: f32) -> Self {
        self.border_radius(BorderRadius::circular(value.max(0.0)))
    }
    /// Adds inner padding around the material child.
    #[must_use]
    pub fn padding(mut self, value: EdgeInsets) -> Self {
        self.padding = Some(value);
        self
    }
    #[must_use]
    pub fn clip_behavior(mut self, value: Clip) -> Self {
        self.clip_behavior = value;
        self
    }
    #[must_use]
    pub fn animation_duration(mut self, value: Duration) -> Self {
        self.animation_duration = value;
        self
    }
}

impl From<Material> for Widget {
    fn from(value: Material) -> Self {
        let text_style = Theme::of_shared().map_or_else(TextStyle::default, |theme| {
            theme.core().text_theme.body_medium.clone()
        });
        let child: Widget = DefaultTextStyle::new(text_style, value.child).into();
        let mut surface_color = value.color;
        if let Some(tint) = value.surface_tint_color {
            let amount = (0.05 + value.elevation / 240.0).min(0.20);
            surface_color = mix(surface_color, tint, amount);
        }
        let mut surface = Container::with_child(child)
            .decoration(
                BoxDecoration::new()
                    .color(surface_color)
                    .border_radius(value.border_radius),
            )
            .clip_behavior(value.clip_behavior);
        if let Some(padding) = value.padding {
            surface = surface.padding(padding);
        }
        if value.material_type == MaterialType::Circle {
            surface =
                surface.decoration(BoxDecoration::new().shape(incular_widgets::BoxShape::Circle));
        }
        if value.elevation > 0.0 {
            DropShadow::new(
                Offset::new(0.0, value.elevation * 0.18),
                (value.elevation * 0.55).max(1.0),
                value.shadow_color,
                surface,
            )
            .into()
        } else {
            surface.into()
        }
    }
}
