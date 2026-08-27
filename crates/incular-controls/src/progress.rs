//! Progress indicators with a small, retained-friendly visual composition.
//!
//! Progress deliberately owns only value and visual policy. Animation belongs
//! to the runtime/compositor; an indeterminate indicator is still a real
//! track/indicator subtree rather than an empty placeholder.

use crate::theme::ControlTheme;
use incular_core::Color;
use incular_semantics::{Role as SemanticRole, SemanticState};
use incular_widgets::internal::ExplicitSemantics;
use incular_widgets::{BorderRadius, BoxDecoration, Container, Positioned, Stack, Widget};
use std::rc::Rc;

#[derive(Clone)]
pub struct Root {
    value: Option<f32>,
    min: f32,
    max: f32,
    width: f32,
    height: Option<f32>,
    label: Option<String>,
    child: Option<Widget>,
}

impl Default for Root {
    fn default() -> Self {
        Self::new()
    }
}

impl Root {
    #[must_use]
    pub fn new() -> Self {
        Self {
            value: Some(0.),
            min: 0.,
            max: 1.,
            width: 180.,
            height: None,
            label: None,
            child: None,
        }
    }

    /// Sets a determinate value. Values are clamped into the configured range.
    #[must_use]
    pub fn value(mut self, value: f32) -> Self {
        self.value = Some(value);
        self
    }

    /// Switches to a retained indeterminate indicator.
    #[must_use]
    pub fn indeterminate(mut self, value: bool) -> Self {
        self.value = value.then_some(f32::NAN);
        self
    }

    #[must_use]
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min = min.min(max);
        self.max = max.max(min);
        self
    }

    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(0.);
        self
    }

    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height.max(0.));
        self
    }

    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Replaces the visual indicator while retaining this descriptor's value
    /// and semantics. This is the equivalent of a component part slot.
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }

    #[must_use]
    pub fn is_indeterminate(&self) -> bool {
        self.value.is_some_and(f32::is_nan)
    }

    #[must_use]
    pub fn normalized_value(&self) -> Option<f32> {
        let value = self.value?;
        if value.is_nan() {
            return None;
        }
        let span = (self.max - self.min).max(f32::EPSILON);
        Some(((value - self.min) / span).clamp(0., 1.))
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        if let Some(child) = &self.child {
            return child.clone();
        }

        let height = self
            .height
            .unwrap_or(theme.progress.height)
            .max(1.)
            .min(self.width.max(1.));
        let width = self.width.max(1.);
        let radius = theme.progress.radius.max(0.);
        let track_color = with_alpha(theme.colors.border, theme.progress.track_alpha);
        let ratio = self.normalized_value().unwrap_or(0.33);
        let indicator_width = (width * ratio).clamp(1., width);
        let track: Widget = Container::new()
            .width(width)
            .height(height)
            .decoration(
                BoxDecoration::new()
                    .color(track_color)
                    .border_radius(BorderRadius::circular(radius)),
            )
            .into();
        let indicator: Widget = Container::new()
            .width(indicator_width)
            .height(height)
            .decoration(
                BoxDecoration::new()
                    .color(theme.colors.accent)
                    .border_radius(BorderRadius::circular(radius)),
            )
            .into();
        let visual: Widget =
            Stack::new([track, Positioned::new(indicator).left(0.).top(0.).into()]).into();

        let value_text = self
            .normalized_value()
            .map(|value| format!("{:.0}%", value * 100.))
            .unwrap_or_else(|| "In progress".to_owned());
        let semantics = ExplicitSemantics::new(SemanticRole::GenericContainer)
            .label(self.label.clone().unwrap_or_else(|| "Progress".to_owned()))
            .value(value_text)
            .state(SemanticState {
                enabled: true,
                ..SemanticState::default()
            });
        visual.semantics(semantics)
    }
}

impl From<Root> for Widget {
    fn from(value: Root) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| {
            let theme = crate::theme::current_control_theme();
            value.build(&theme)
        })
    }
}

fn with_alpha(color: Color, alpha: u8) -> Color {
    Color::rgba(color.red, color.green, color.blue, alpha)
}
