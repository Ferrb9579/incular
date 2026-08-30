//! Label-oriented Material button families.

use super::common::ButtonSpec;
use super::style::{ButtonKind, ButtonStyleConfig};
use incular_controls::{ButtonStyle, ButtonVariant};
use incular_widgets::{SizedBox, Widget};
use typed_builder::TypedBuilder;

/// High-emphasis Material elevated button.
#[derive(Clone, TypedBuilder)]
#[builder(mutators(
    pub fn label(self, label: impl Into<String>) {
        self.spec = self.spec.clone().set_label(label);
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
pub struct ElevatedButton {
    #[builder(via_mutators = ButtonSpec::child(ButtonKind::Elevated, SizedBox::shrink()))]
    spec: ButtonSpec,
}

impl Default for ElevatedButton {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl ElevatedButton {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::label(ButtonKind::Elevated, label),
        }
    }

    #[must_use]
    pub fn icon(icon: impl Into<Widget>, label: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::icon_label(ButtonKind::Elevated, icon, label),
        }
    }

    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self {
            spec: ButtonSpec::child(ButtonKind::Elevated, child),
        }
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

impl From<ElevatedButton> for Widget {
    fn from(value: ElevatedButton) -> Self {
        value.spec.into_widget()
    }
}

/// Filled Material button, including the Material 3 tonal variant.
#[derive(Clone, TypedBuilder)]
#[builder(mutators(
    pub fn label(self, label: impl Into<String>) {
        self.spec = self.spec.clone().set_label(label);
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
pub struct FilledButton {
    #[builder(via_mutators = ButtonSpec::child(ButtonKind::Filled, SizedBox::shrink()))]
    spec: ButtonSpec,
}

impl Default for FilledButton {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl FilledButton {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::label(ButtonKind::Filled, label),
        }
    }

    #[must_use]
    pub fn icon(icon: impl Into<Widget>, label: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::icon_label(ButtonKind::Filled, icon, label),
        }
    }

    #[must_use]
    pub fn tonal(label: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::label(ButtonKind::FilledTonal, label),
        }
    }

    #[must_use]
    pub fn tonal_icon(icon: impl Into<Widget>, label: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::icon_label(ButtonKind::FilledTonal, icon, label),
        }
    }

    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self {
            spec: ButtonSpec::child(ButtonKind::Filled, child),
        }
    }

    #[must_use]
    pub fn tonal_with_child(child: impl Into<Widget>) -> Self {
        Self {
            spec: ButtonSpec::child(ButtonKind::FilledTonal, child),
        }
    }

    #[must_use]
    pub fn style_from(config: ButtonStyleConfig) -> ButtonStyle {
        config.into_style(ButtonVariant::Primary)
    }

    #[must_use]
    pub fn tonal_style_from(config: ButtonStyleConfig) -> ButtonStyle {
        config.into_style(ButtonVariant::Primary)
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

impl From<FilledButton> for Widget {
    fn from(value: FilledButton) -> Self {
        value.spec.into_widget()
    }
}

/// Medium-emphasis outlined Material button.
#[derive(Clone, TypedBuilder)]
#[builder(mutators(
    pub fn label(self, label: impl Into<String>) {
        self.spec = self.spec.clone().set_label(label);
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
pub struct OutlinedButton {
    #[builder(via_mutators = ButtonSpec::child(ButtonKind::Outlined, SizedBox::shrink()))]
    spec: ButtonSpec,
}

impl Default for OutlinedButton {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl OutlinedButton {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::label(ButtonKind::Outlined, label),
        }
    }

    #[must_use]
    pub fn icon(icon: impl Into<Widget>, label: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::icon_label(ButtonKind::Outlined, icon, label),
        }
    }

    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self {
            spec: ButtonSpec::child(ButtonKind::Outlined, child),
        }
    }

    #[must_use]
    pub fn style_from(config: ButtonStyleConfig) -> ButtonStyle {
        config.into_style(ButtonVariant::Default)
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

impl From<OutlinedButton> for Widget {
    fn from(value: OutlinedButton) -> Self {
        value.spec.into_widget()
    }
}

/// Low-emphasis text Material button.
#[derive(Clone, TypedBuilder)]
#[builder(mutators(
    pub fn label(self, label: impl Into<String>) {
        self.spec = self.spec.clone().set_label(label);
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
pub struct TextButton {
    #[builder(via_mutators = ButtonSpec::child(ButtonKind::Text, SizedBox::shrink()))]
    spec: ButtonSpec,
}

impl Default for TextButton {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl TextButton {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::label(ButtonKind::Text, label),
        }
    }

    #[must_use]
    pub fn icon(icon: impl Into<Widget>, label: impl Into<String>) -> Self {
        Self {
            spec: ButtonSpec::icon_label(ButtonKind::Text, icon, label),
        }
    }

    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self {
            spec: ButtonSpec::child(ButtonKind::Text, child),
        }
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

impl From<TextButton> for Widget {
    fn from(value: TextButton) -> Self {
        value.spec.into_widget()
    }
}
