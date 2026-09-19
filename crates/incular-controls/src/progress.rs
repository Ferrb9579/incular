//! Progress indicators with a small, retained-friendly visual composition.
//!
//! Progress deliberately owns only value and visual policy. Animation belongs
//! to the runtime/compositor; an indeterminate indicator is still a real
//! track/indicator subtree rather than an empty placeholder.

use crate::theme::ControlTheme;
use incular_core::Color;
use incular_semantics::{Role as SemanticRole, SemanticState};
use incular_widgets::internal::{AnimationRetargetBridge, ExplicitSemantics};
use incular_widgets::{
    AnimationController, BorderRadius, BoxDecoration, Container, Positioned, Stack, Widget,
};
use std::{cell::Cell, rc::Rc, time::Duration};
use typed_builder::TypedBuilder;

#[derive(Clone, TypedBuilder)]
pub struct Root {
    #[builder(default = Some(0.), setter(strip_option))]
    value: Option<f32>,
    #[builder(default = 0.)]
    min: f32,
    #[builder(default = 1.)]
    max: f32,
    #[builder(default = 180., setter(transform = |width: f32| width.max(0.)))]
    width: f32,
    #[builder(default, setter(transform = |height: f32| Some(height.max(0.))))]
    height: Option<f32>,
    #[builder(default, setter(strip_option, into))]
    label: Option<String>,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
}

impl Default for Root {
    fn default() -> Self {
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
}

impl Root {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self::default().child(child)
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
        if value {
            self.value = Some(f32::NAN);
        } else if self.value.is_none() || self.value.is_some_and(f32::is_nan) {
            self.value = Some(self.min);
        }
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
        self.build_with_phase(theme, 0.0)
    }

    fn build_with_phase(&self, theme: &ControlTheme, phase: f32) -> Widget {
        let visual: Widget = if let Some(child) = &self.child {
            child.clone()
        } else {
            let height = self
                .height
                .unwrap_or(theme.progress.height)
                .max(1.)
                .min(self.width.max(1.));
            let width = self.width.max(1.);
            let radius = theme.progress.radius.max(0.);
            let track_color = with_alpha(theme.colors.border, theme.progress.track_alpha);
            let determinate = self.normalized_value();
            let indicator_width = width * determinate.unwrap_or(0.33);
            let indicator_left = if determinate.is_none() {
                (width - indicator_width).max(0.0) * phase.clamp(0.0, 1.0)
            } else {
                0.0
            };
            let track: Widget = Container::new()
                .width(width)
                .height(height)
                .decoration(
                    BoxDecoration::new()
                        .color(track_color)
                        .border_radius(BorderRadius::circular(radius)),
                )
                .into();
            let mut children = vec![track];
            if indicator_width > 0.0 {
                let indicator: Widget = Container::new()
                    .width(indicator_width.min(width))
                    .height(height)
                    .decoration(
                        BoxDecoration::new()
                            .color(theme.colors.accent)
                            .border_radius(BorderRadius::circular(radius)),
                    )
                    .into();
                children.push(
                    Positioned::new(indicator)
                        .left(indicator_left)
                        .top(0.)
                        .into(),
                );
            }
            Stack::new(children).into()
        };

        let value_text = self
            .normalized_value()
            .map(|value| format!("{:.0}%", value * 100.))
            .unwrap_or_else(|| "In progress".to_owned());
        let semantics = ExplicitSemantics::new(SemanticRole::ProgressBar)
            .label(self.label.clone().unwrap_or_else(|| "Progress".to_owned()))
            .value(value_text)
            .state(SemanticState {
                enabled: true,
                busy: self.is_indeterminate(),
                numeric_value: self.value.filter(|value| value.is_finite()).map(f64::from),
                numeric_min: Some(f64::from(self.min)),
                numeric_max: Some(f64::from(self.max)),
                ..SemanticState::default()
            });
        visual.semantics(semantics)
    }
}

impl From<Root> for Widget {
    fn from(value: Root) -> Self {
        let value = Rc::new(value);
        if value.is_indeterminate() && value.child.is_none() {
            const CYCLE: Duration = Duration::from_millis(1200);
            let controller = AnimationController::new(CYCLE);
            let revision = Rc::new(Cell::new(0_u64));
            let value_for_build = value.clone();
            let controller_for_build = controller.clone();
            let animated = Widget::stateful_layout_builder(revision.clone(), move |context, _| {
                let theme = crate::theme::current_control_theme(context);
                value_for_build.build_with_phase(&theme, controller_for_build.value())
            });
            let replacement = controller.clone();
            let bridge =
                AnimationRetargetBridge::new(Rc::new(controller.clone()), move |previous| {
                    replacement.adopt_timeline_from(previous);
                });
            return Widget::animation_ticker_repeating_with_retarget(
                controller,
                true,
                revision,
                Some(bridge),
                animated,
            );
        }
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            let theme = crate::theme::current_control_theme(context);
            value.build(&theme)
        }))
    }
}

fn with_alpha(color: Color, alpha: u8) -> Color {
    Color::rgba(color.red, color.green, color.blue, alpha)
}
