use std::rc::Rc;

use super::common::finite_non_negative;
use crate::foundation::Material;
use incular_config::{Alignment, Clip, EdgeInsets, StackFit};
use incular_controls::{ControlTheme, current_control_theme};
use incular_core::Color;
use incular_widgets::{
    Border, BorderRadius, BoxDecoration, Container, IgnorePointer, Padding, Positioned, Semantics,
    SizedBox, Stack, Widget,
};
use typed_builder::TypedBuilder;

/// Alias used by callers that want a generic surface vocabulary rather than
/// Flutter's `Material` name.
pub type Surface = Material;

/// A Material circular avatar.
#[derive(Clone, TypedBuilder)]
pub struct CircleAvatar {
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default, setter(strip_option))]
    background: Option<Color>,
    #[builder(default, setter(strip_option))]
    foreground: Option<Color>,
    #[builder(default = 20.0, setter(transform = |value: f32| finite_non_negative(value)))]
    radius: f32,
}

impl CircleAvatar {
    #[must_use]
    pub fn new() -> Self {
        Self {
            child: None,
            background: None,
            foreground: None,
            radius: 20.0,
        }
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }

    #[must_use]
    pub fn background_color(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    #[must_use]
    pub fn foreground_color(mut self, color: Color) -> Self {
        self.foreground = Some(color);
        self
    }

    #[must_use]
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = finite_non_negative(radius);
        self
    }
}

impl Default for CircleAvatar {
    fn default() -> Self {
        Self::new()
    }
}

impl From<CircleAvatar> for Widget {
    fn from(value: CircleAvatar) -> Self {
        let child = value.child.unwrap_or_else(|| SizedBox::shrink().into());
        Container::new()
            .width(value.radius * 2.0)
            .height(value.radius * 2.0)
            .alignment(Alignment::CENTER)
            .color(value.background.unwrap_or(Color::rgba(120, 120, 120, 255)))
            .radius(value.radius)
            .child(child)
            .into()
    }
}

/// Material card composition.
#[derive(Clone, TypedBuilder)]
pub struct Card {
    #[builder(setter(into))]
    child: Widget,
    #[builder(default, setter(strip_option))]
    color: Option<Color>,
    #[builder(default, setter(strip_option))]
    shadow_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    surface_tint_color: Option<Color>,
    #[builder(default = 1.0, setter(transform = |value: f32| finite_non_negative(value)))]
    elevation: f32,
    #[builder(default, setter(transform = |value: f32| Some(finite_non_negative(value))))]
    radius: Option<f32>,
    #[builder(default, setter(strip_option))]
    shape: Option<BorderRadius>,
    #[builder(default = Some(EdgeInsets::all(16.0)))]
    padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    margin: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    border: Option<Border>,
    #[builder(default = Clip::None)]
    clip_behavior: Clip,
    #[builder(default)]
    border_on_foreground: bool,
    #[builder(default = true)]
    semantic_container: bool,
}

impl Card {
    /// Creates a card around `child`.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            color: None,
            shadow_color: None,
            surface_tint_color: None,
            elevation: 1.0,
            radius: None,
            shape: None,
            padding: Some(EdgeInsets::all(16.0)),
            margin: None,
            border: None,
            clip_behavior: Clip::None,
            border_on_foreground: false,
            semantic_container: true,
        }
    }

    /// Material filled-card defaults (no elevation, tonal surface).
    #[must_use]
    pub fn filled(child: impl Into<Widget>) -> Self {
        Self::new(child)
            .elevation(0.0)
            .color(Color::rgba(245, 240, 247, 255))
    }

    /// Material elevated-card defaults.
    #[must_use]
    pub fn elevated(child: impl Into<Widget>) -> Self {
        Self::new(child).elevation(1.0)
    }

    /// Material outlined-card defaults.
    #[must_use]
    pub fn outlined(child: impl Into<Widget>) -> Self {
        Self::new(child)
            .elevation(0.0)
            .border(Border::new(1.0, Color::rgba(121, 116, 126, 255)))
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    #[must_use]
    pub fn shadow_color(mut self, color: Color) -> Self {
        self.shadow_color = Some(color);
        self
    }

    #[must_use]
    pub fn surface_tint_color(mut self, color: Color) -> Self {
        self.surface_tint_color = Some(color);
        self
    }

    #[must_use]
    pub fn elevation(mut self, elevation: f32) -> Self {
        self.elevation = finite_non_negative(elevation);
        self
    }

    #[must_use]
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = Some(finite_non_negative(radius));
        self.shape = None;
        self
    }

    #[must_use]
    pub fn shape(mut self, shape: BorderRadius) -> Self {
        self.shape = Some(shape);
        self.radius = None;
        self
    }

    #[must_use]
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = Some(padding);
        self
    }

    #[must_use]
    pub fn margin(mut self, margin: EdgeInsets) -> Self {
        self.margin = Some(margin);
        self
    }

    #[must_use]
    pub fn border(mut self, border: Border) -> Self {
        self.border = Some(border);
        self
    }

    #[must_use]
    pub fn clip_behavior(mut self, clip_behavior: Clip) -> Self {
        self.clip_behavior = clip_behavior;
        self
    }

    /// Paints the border after the content when true, before it when false.
    #[must_use]
    pub fn border_on_foreground(mut self, value: bool) -> Self {
        self.border_on_foreground = value;
        self
    }

    /// Exposes a semantic group around the card while preserving child actions.
    #[must_use]
    pub fn semantic_container(mut self, value: bool) -> Self {
        self.semantic_container = value;
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let radius = self
            .shape
            .unwrap_or_else(|| BorderRadius::circular(self.radius.unwrap_or(theme.radius.md)));
        let mut content: Widget =
            Padding::new(self.padding.unwrap_or_default(), self.child.clone()).into();
        if let Some(border) = self.border {
            let decoration: Widget = Container::new()
                .decoration(BoxDecoration::new().border(border).border_radius(radius))
                .into();
            let outline: Widget =
                Positioned::fill(IgnorePointer::new(decoration.exclude_semantics())).into();
            // These keys belong to the card's private visual slots, not the
            // application's child. Changing paint order must preserve its state.
            let outline = outline.with_key(0_u64);
            content = content.with_key(1_u64);
            let layers = if self.border_on_foreground {
                [content, outline]
            } else {
                [outline, content]
            };
            content = Stack::new(layers)
                .fit(StackFit::Passthrough)
                .clip_behavior(Clip::None)
                .into();
        }
        let mut material = Material::new(content)
            .color(self.color.unwrap_or(theme.colors.surface))
            .elevation(self.elevation)
            .shadow_color(self.shadow_color.unwrap_or(Color::rgba(0, 0, 0, 100)))
            .border_radius(radius)
            .clip_behavior(self.clip_behavior);
        if let Some(tint) = self.surface_tint_color {
            material = material.surface_tint_color(tint);
        }
        let material: Widget = material.into();
        let material = if self.semantic_container {
            Semantics::new(material)
                .role(incular_semantics::Role::Group)
                .into()
        } else {
            material
        };
        let mut surface = Container::with_child(material);
        if let Some(margin) = self.margin {
            surface = surface.margin(margin);
        }
        surface.into()
    }
}

impl From<Card> for Widget {
    fn from(value: Card) -> Self {
        let value = Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            value.build(&current_control_theme(context))
        }))
    }
}

/// A horizontal Material divider.
#[derive(Clone, TypedBuilder)]
pub struct Divider {
    #[builder(default, setter(strip_option))]
    color: Option<Color>,
    #[builder(default = 1.0, setter(transform = |value: f32| finite_non_negative(value)))]
    thickness: f32,
    #[builder(default, setter(transform = |value: f32| finite_non_negative(value)))]
    indent: f32,
    #[builder(default, setter(transform = |value: f32| finite_non_negative(value)))]
    end_indent: f32,
}

impl Divider {
    #[must_use]
    pub fn new() -> Self {
        Self {
            color: None,
            thickness: 1.0,
            indent: 0.0,
            end_indent: 0.0,
        }
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    #[must_use]
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = finite_non_negative(thickness);
        self
    }

    #[must_use]
    pub fn indent(mut self, indent: f32) -> Self {
        self.indent = finite_non_negative(indent);
        self
    }

    #[must_use]
    pub fn end_indent(mut self, end_indent: f32) -> Self {
        self.end_indent = finite_non_negative(end_indent);
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        Container::new()
            .height(self.thickness.max(0.1))
            .margin(EdgeInsets::only(self.indent, 0.0, self.end_indent, 0.0))
            .color(self.color.unwrap_or(theme.colors.border))
            .into()
    }
}

impl Default for Divider {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Divider> for Widget {
    fn from(value: Divider) -> Self {
        let value = Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            value.build(&current_control_theme(context))
        }))
    }
}

/// A vertical Material divider for rows and navigation rails.
#[derive(Clone, TypedBuilder)]
pub struct VerticalDivider {
    #[builder(default, setter(strip_option))]
    color: Option<Color>,
    #[builder(default = 1.0, setter(transform = |value: f32| finite_non_negative(value)))]
    thickness: f32,
    #[builder(default, setter(transform = |value: f32| finite_non_negative(value)))]
    indent: f32,
    #[builder(default, setter(transform = |value: f32| finite_non_negative(value)))]
    end_indent: f32,
}

impl VerticalDivider {
    #[must_use]
    pub fn new() -> Self {
        Self {
            color: None,
            thickness: 1.0,
            indent: 0.0,
            end_indent: 0.0,
        }
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    #[must_use]
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = finite_non_negative(thickness);
        self
    }

    #[must_use]
    pub fn indent(mut self, indent: f32) -> Self {
        self.indent = finite_non_negative(indent);
        self
    }

    #[must_use]
    pub fn end_indent(mut self, end_indent: f32) -> Self {
        self.end_indent = finite_non_negative(end_indent);
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        Container::new()
            .width(self.thickness.max(0.1))
            .margin(EdgeInsets::only(0.0, self.indent, 0.0, self.end_indent))
            .color(self.color.unwrap_or(theme.colors.border))
            .into()
    }
}

impl Default for VerticalDivider {
    fn default() -> Self {
        Self::new()
    }
}

impl From<VerticalDivider> for Widget {
    fn from(value: VerticalDivider) -> Self {
        let value = Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            value.build(&current_control_theme(context))
        }))
    }
}
