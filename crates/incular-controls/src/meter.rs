//! Measurement meter built on the same visual track as the Progress control.

use crate::{ControlTheme, progress};
use incular_semantics::{Role as SemanticRole, SemanticState};
use incular_widgets::{Widget, internal::ExplicitSemantics};
use std::rc::Rc;
use typed_builder::TypedBuilder;

#[derive(Clone, TypedBuilder)]
pub struct Root {
    #[builder(default = 0.)]
    value: f32,
    #[builder(default = 0.)]
    min: f32,
    #[builder(default = 1.)]
    max: f32,
    #[builder(default, setter(strip_option))]
    low: Option<f32>,
    #[builder(default, setter(strip_option))]
    high: Option<f32>,
    #[builder(default, setter(strip_option))]
    optimum: Option<f32>,
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
            ExplicitSemantics::new(SemanticRole::Meter)
                .label(self.label.clone().unwrap_or_else(|| "Meter".to_owned()))
                .value(value)
                .description(description)
                .state(SemanticState {
                    enabled: true,
                    numeric_value: Some(f64::from(self.value)),
                    numeric_min: Some(f64::from(self.min)),
                    numeric_max: Some(f64::from(self.max)),
                    ..SemanticState::default()
                }),
        )
    }
}

impl From<Root> for Widget {
    fn from(value: Root) -> Self {
        let value = Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            let theme = crate::theme::current_control_theme(context);
            value.build(&theme)
        }))
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
