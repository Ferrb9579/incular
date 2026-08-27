//! Measurement meter built on the same visual track as the Progress control.

use crate::{ControlTheme, progress};
use incular_semantics::{Role as SemanticRole, SemanticState};
use incular_widgets::{Widget, internal::ExplicitSemantics};
use std::rc::Rc;

#[derive(Clone)]
pub struct Root {
    value: f32,
    min: f32,
    max: f32,
    low: Option<f32>,
    high: Option<f32>,
    optimum: Option<f32>,
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
            value: 0.,
            min: 0.,
            max: 1.,
            low: None,
            high: None,
            optimum: None,
            width: 180.,
            height: None,
            label: None,
            child: None,
        }
    }

    #[must_use]
    pub fn value(mut self, value: f32) -> Self {
        self.value = value;
        self
    }

    #[must_use]
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min = min.min(max);
        self.max = max.max(min);
        self
    }

    #[must_use]
    pub fn low(mut self, value: f32) -> Self {
        self.low = Some(value);
        self
    }

    #[must_use]
    pub fn high(mut self, value: f32) -> Self {
        self.high = Some(value);
        self
    }

    #[must_use]
    pub fn optimum(mut self, value: f32) -> Self {
        self.optimum = Some(value);
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

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }

    #[must_use]
    pub fn normalized_value(&self) -> f32 {
        let span = (self.max - self.min).max(f32::EPSILON);
        ((self.value - self.min) / span).clamp(0., 1.)
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let visual = progress::Root::new()
            .value(self.value)
            .range(self.min, self.max)
            .width(self.width)
            .height(self.height.unwrap_or(theme.meter.height))
            .child_if_present(self.child.clone())
            .build(theme);
        let value = format!("{:.0}%", self.normalized_value() * 100.);
        let mut description = format!("Value {value}");
        if let (Some(low), Some(high)) = (self.low, self.high) {
            description.push_str(&format!(", normal range {low}–{high}"));
        }
        if let Some(optimum) = self.optimum {
            description.push_str(&format!(", optimum {optimum}"));
        }
        visual.semantics(
            ExplicitSemantics::new(SemanticRole::GenericContainer)
                .label(self.label.clone().unwrap_or_else(|| "Meter".to_owned()))
                .value(value)
                .description(description)
                .state(SemanticState {
                    enabled: true,
                    ..SemanticState::default()
                }),
        )
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

trait ProgressChild {
    fn child_if_present(self, child: Option<Widget>) -> Self;
}

impl ProgressChild for progress::Root {
    fn child_if_present(self, child: Option<Widget>) -> Self {
        match child {
            Some(child) => self.child(child),
            None => self,
        }
    }
}
