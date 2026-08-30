//! Icon-only and floating action Material button families.

use super::common::ButtonSpec;
use super::style::{ButtonKind, ButtonStyleConfig};
use incular_config::{CrossAxisAlignment, EdgeInsets};
use incular_controls::{ButtonStyle, ButtonVariant};
use incular_core::Size;
use incular_widgets::{Row, SizedBox, Widget};
use typed_builder::TypedBuilder;

/// Material icon-only action button.
#[derive(Clone, TypedBuilder)]
#[builder(mutators(
    pub fn icon(self, icon: impl Into<String>) {
        self.spec = self.spec.clone().set_child(incular_widgets::Text::new(icon));
    }
    pub fn child(self, child: impl Into<Widget>) {
        self.spec = self.spec.clone().set_child(child);
    }
    pub fn style(self, style: ButtonStyle) {
        self.spec = self.spec.clone().style(style);
    }
    pub fn enabled(self, value: bool) {
        self.spec = self.spec.clone().enabled(value);
    }
    pub fn on_click<F>(self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.spec = self.spec.clone().on_click(callback);
    }
    pub fn on_pressed<F>(self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.spec = self.spec.clone().on_click(callback);
    }
    pub fn focusable_when_disabled(self, value: bool) {
        self.spec = self.spec.clone().focusable_when_disabled(value);
    }
    pub fn loading(self, value: bool) {
        self.spec = self.spec.clone().loading(value);
    }
))]
pub struct IconButton {
    #[builder(via_mutators = ButtonSpec::child(ButtonKind::Icon, SizedBox::shrink()))]
    spec: ButtonSpec,
    #[builder(default)]
    selected: bool,
    #[builder(default, setter(strip_option, into))]
    tooltip: Option<String>,
}

impl Default for IconButton {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl IconButton {
    #[must_use]
    pub fn new(icon: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::child(ButtonKind::Icon, incular_widgets::Text::new(icon)),
            selected: false,
            tooltip: None,
        }
    }

    #[must_use]
    pub fn icon(icon: impl Into<Widget>) -> Self {
        Self::with_child(icon)
    }

    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self {
            spec: ButtonSpec::child(ButtonKind::Icon, child),
            selected: false,
            tooltip: None,
        }
    }

    #[must_use]
    pub fn selected(mut self, value: bool) -> Self {
        self.selected = value;
        self
    }

    #[must_use]
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    #[must_use]
    pub fn tooltip(mut self, value: impl Into<String>) -> Self {
        self.tooltip = Some(value.into());
        self
    }

    #[must_use]
    pub fn style_from(config: ButtonStyleConfig) -> ButtonStyle {
        config.into_style(ButtonVariant::Ghost)
    }

    #[must_use]
    pub fn style(mut self, value: ButtonStyle) -> Self {
        self.spec = self.spec.clone().style(value);
        self
    }

    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().enabled(value);
        self
    }

    #[must_use]
    pub fn on_click(mut self, callback: impl Fn() + 'static) -> Self {
        self.spec = self.spec.clone().on_click(callback);
        self
    }

    #[must_use]
    pub fn on_pressed(self, callback: impl Fn() + 'static) -> Self {
        self.on_click(callback)
    }

    #[must_use]
    pub fn focusable_when_disabled(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().focusable_when_disabled(value);
        self
    }

    #[must_use]
    pub fn loading(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().loading(value);
        self
    }
}

impl From<IconButton> for Widget {
    fn from(mut value: IconButton) -> Self {
        if let Some(tooltip) = value.tooltip.take() {
            value.spec.semantic_label = Some(tooltip);
        } else {
            value.spec.semantic_label = Some("Icon button".to_owned());
        }
        value.spec.into_widget()
    }
}

/// Material floating action button.
#[derive(Clone, TypedBuilder)]
#[builder(mutators(
    pub fn child(self, child: impl Into<Widget>) {
        self.spec = self.spec.clone().set_child(child);
    }
    pub fn small(self) {
        self.size = FloatingActionButtonSize::Small;
    }
    pub fn large(self) {
        self.size = FloatingActionButtonSize::Large;
    }
    pub fn extended(self, label: impl Into<String>) {
        let label = label.into();
        self.size = FloatingActionButtonSize::Extended;
        self.spec = self
            .spec
            .clone()
            .set_child(incular_widgets::Text::new(label.clone()));
        self.spec.semantic_label = Some(label);
    }
    pub fn extended_with_icon(self, icon: impl Into<Widget>, label: impl Into<String>) {
        let label = label.into();
        self.size = FloatingActionButtonSize::Extended;
        self.spec = self.spec.clone().set_child(
            Row::new([
                icon.into(),
                SizedBox::new().width(8.0).into(),
                incular_widgets::Text::new(label.clone()).into(),
            ])
            .alignment(CrossAxisAlignment::Center),
        );
        self.spec.semantic_label = Some(label);
    }
    pub fn style(self, style: ButtonStyle) {
        self.spec = self.spec.clone().style(style);
    }
    pub fn on_click<F>(self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.spec = self.spec.clone().on_click(callback);
    }
    pub fn on_pressed<F>(self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.spec = self.spec.clone().on_click(callback);
    }
    pub fn enabled(self, value: bool) {
        self.spec = self.spec.clone().enabled(value);
    }
    pub fn focusable_when_disabled(self, value: bool) {
        self.spec = self.spec.clone().focusable_when_disabled(value);
    }
    pub fn loading(self, value: bool) {
        self.spec = self.spec.clone().loading(value);
    }
))]
pub struct FloatingActionButton {
    #[builder(via_mutators = ButtonSpec::child(ButtonKind::Floating, SizedBox::shrink()))]
    spec: ButtonSpec,
    #[builder(via_mutators = FloatingActionButtonSize::Regular)]
    size: FloatingActionButtonSize,
    #[builder(default, setter(strip_option, into))]
    tooltip: Option<String>,
    #[builder(default, setter(strip_option, into))]
    hero_tag: Option<String>,
}

impl Default for FloatingActionButton {
    fn default() -> Self {
        Self::builder().build()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FloatingActionButtonSize {
    Regular,
    Small,
    Large,
    Extended,
}

impl FloatingActionButton {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            spec: ButtonSpec::child(ButtonKind::Floating, child),
            size: FloatingActionButtonSize::Regular,
            tooltip: None,
            hero_tag: None,
        }
    }

    #[must_use]
    pub fn small(child: impl Into<Widget>) -> Self {
        Self {
            size: FloatingActionButtonSize::Small,
            ..Self::new(child)
        }
    }

    #[must_use]
    pub fn large(child: impl Into<Widget>) -> Self {
        Self {
            size: FloatingActionButtonSize::Large,
            ..Self::new(child)
        }
    }

    #[must_use]
    pub fn extended(label: impl Into<String>) -> Self {
        let label = label.into();
        let mut value = Self::new(incular_widgets::Text::new(label.clone()));
        value.size = FloatingActionButtonSize::Extended;
        value.spec.semantic_label = Some(label);
        value
    }

    #[must_use]
    pub fn extended_with_icon(icon: impl Into<Widget>, label: impl Into<String>) -> Self {
        let label = label.into();
        let mut value = Self::new(
            Row::new([
                icon.into(),
                SizedBox::new().width(8.0).into(),
                incular_widgets::Text::new(label.clone()).into(),
            ])
            .alignment(CrossAxisAlignment::Center),
        );
        value.size = FloatingActionButtonSize::Extended;
        value.spec.semantic_label = Some(label);
        value
    }

    #[must_use]
    pub fn style_from(config: ButtonStyleConfig) -> ButtonStyle {
        config.into_style(ButtonVariant::Primary)
    }

    #[must_use]
    pub fn style(mut self, value: ButtonStyle) -> Self {
        self.spec = self.spec.clone().style(value);
        self
    }

    #[must_use]
    pub fn tooltip(mut self, value: impl Into<String>) -> Self {
        self.tooltip = Some(value.into());
        self
    }

    #[must_use]
    pub fn hero_tag(mut self, value: impl Into<String>) -> Self {
        self.hero_tag = Some(value.into());
        self
    }

    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().enabled(value);
        self
    }

    #[must_use]
    pub fn on_click(mut self, callback: impl Fn() + 'static) -> Self {
        self.spec = self.spec.clone().on_click(callback);
        self
    }

    #[must_use]
    pub fn on_pressed(self, callback: impl Fn() + 'static) -> Self {
        self.on_click(callback)
    }

    #[must_use]
    pub fn focusable_when_disabled(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().focusable_when_disabled(value);
        self
    }

    #[must_use]
    pub fn loading(mut self, value: bool) -> Self {
        self.spec = self.spec.clone().loading(value);
        self
    }

    /// Returns the configured hero tag. The runtime owns actual hero-flight
    /// coordination; this accessor makes the retained configuration inspectable.
    #[must_use]
    pub fn hero_tag_value(&self) -> Option<&str> {
        self.hero_tag.as_deref()
    }
}

impl From<FloatingActionButton> for Widget {
    fn from(mut value: FloatingActionButton) -> Self {
        let size = match value.size {
            FloatingActionButtonSize::Regular => 56.0,
            FloatingActionButtonSize::Small => 40.0,
            FloatingActionButtonSize::Large => 96.0,
            FloatingActionButtonSize::Extended => 48.0,
        };
        let mut size_style = ButtonStyle::new().border_radius(size * 0.5);
        if value.size == FloatingActionButtonSize::Extended {
            size_style = size_style
                .minimum_size(Size::new(80.0, 48.0))
                .padding(EdgeInsets::symmetric(16.0, 8.0));
        } else {
            size_style = size_style
                .minimum_size(Size::new(size, size))
                .fixed_size(Size::new(size, size));
        }
        value.spec = value.spec.style(size_style);
        value.spec.semantic_label = value.tooltip.take().or(value.spec.semantic_label);
        if value.spec.semantic_label.is_none() {
            value.spec.semantic_label = Some("Floating action button".to_owned());
        }
        value.spec.into_widget()
    }
}
