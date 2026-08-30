use std::{rc::Rc, sync::Arc};

use super::common::{finite_non_negative, normalized, with_alpha};
use crate::feedback::current_progress_indicator_theme;
use incular_controls::{ControlTheme, current_control_theme};
use incular_core::{Color, Offset, Size};
use incular_rendering::{Canvas, LineCap, LineJoin, Path, Stroke};
use incular_widgets::{Semantics, Widget};
use typed_builder::TypedBuilder;

/// Material linear progress indicator.
///
/// `None` is represented as an indeterminate indicator.  The actual retained
/// track/indicator composition is shared with the controls crate so value
/// clamping, sizing, and accessibility remain consistent with Incular's
/// existing progress implementation.
#[derive(Clone, TypedBuilder)]
pub struct LinearProgressIndicator {
    #[builder(default, setter(strip_option))]
    value: Option<f32>,
    #[builder(default = 0.0)]
    min: f32,
    #[builder(default = 1.0)]
    max: f32,
    #[builder(default = 180.0, setter(transform = |value: f32| finite_non_negative(value)))]
    width: f32,
    #[builder(default, setter(transform = |value: f32| Some(finite_non_negative(value))))]
    height: Option<f32>,
    #[builder(default, setter(strip_option, into))]
    label: Option<String>,
}

impl Default for LinearProgressIndicator {
    fn default() -> Self {
        Self::new()
    }
}

impl LinearProgressIndicator {
    /// Creates an indeterminate indicator, matching Flutter's default.
    #[must_use]
    pub fn new() -> Self {
        Self {
            value: None,
            min: 0.0,
            max: 1.0,
            width: 180.0,
            height: None,
            label: None,
        }
    }

    /// Sets a determinate value.
    #[must_use]
    pub fn value(mut self, value: f32) -> Self {
        self.value = Some(value);
        self
    }

    /// Sets the indicator to indeterminate mode.
    #[must_use]
    pub fn indeterminate(mut self, value: bool) -> Self {
        if value {
            self.value = None;
        } else if self.value.is_none() {
            self.value = Some(self.min);
        }
        self
    }

    /// Sets the value directly, with `None` meaning indeterminate.
    #[must_use]
    pub fn value_option(mut self, value: Option<f32>) -> Self {
        self.value = value;
        self
    }

    #[must_use]
    pub fn range(mut self, min: f32, max: f32) -> Self {
        if min.is_finite() && max.is_finite() {
            self.min = min.min(max);
            self.max = max.max(min);
        }
        self
    }

    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = finite_non_negative(width);
        self
    }

    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(finite_non_negative(height));
        self
    }

    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let indicator_theme = current_progress_indicator_theme().unwrap_or_default();
        let mut controls_theme = theme.clone();
        if let Some(color) = indicator_theme.color {
            controls_theme.colors.accent = color;
        }
        if let Some(track_color) = indicator_theme.linear_track_color {
            controls_theme.colors.border = track_color;
        }
        let mut progress = incular_controls::progress::Root::new()
            .range(self.min, self.max)
            .width(self.width);
        if let Some(height) = self.height {
            progress = progress.height(height);
        } else if let Some(height) = indicator_theme.linear_track_height {
            progress = progress.height(height);
        }
        if let Some(label) = self.label.as_ref() {
            progress = progress.label(label.clone());
        }
        progress = match self.value {
            Some(value) => progress.value(value),
            None => progress.indeterminate(true),
        };
        progress.build(&controls_theme)
    }
}

impl From<LinearProgressIndicator> for Widget {
    fn from(value: LinearProgressIndicator) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

/// A retained circular progress indicator. The ring is lowered to the shared
/// renderer path/display-list primitives so determinate values paint the
/// actual progress arc while indeterminate values still expose a stable
/// Material-sized visual without a continuously scheduled frame.
#[derive(Clone, TypedBuilder)]
pub struct CircularProgressIndicator {
    #[builder(default, setter(strip_option))]
    value: Option<f32>,
    #[builder(default = 0.0)]
    min: f32,
    #[builder(default = 1.0)]
    max: f32,
    #[builder(default = 24.0, setter(transform = |value: f32| finite_non_negative(value)))]
    size: f32,
    #[builder(default = 2.0, setter(transform = |value: f32| finite_non_negative(value)))]
    stroke_width: f32,
    #[builder(default, setter(strip_option))]
    color: Option<Color>,
    #[builder(default, setter(strip_option))]
    background_color: Option<Color>,
    #[builder(default, setter(strip_option, into))]
    label: Option<String>,
}

impl Default for CircularProgressIndicator {
    fn default() -> Self {
        Self::new()
    }
}

impl CircularProgressIndicator {
    /// Creates an indeterminate circular indicator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            value: None,
            min: 0.0,
            max: 1.0,
            size: 24.0,
            stroke_width: 2.0,
            color: None,
            background_color: None,
            label: None,
        }
    }

    #[must_use]
    pub fn value(mut self, value: f32) -> Self {
        self.value = Some(value);
        self
    }

    #[must_use]
    pub fn value_option(mut self, value: Option<f32>) -> Self {
        self.value = value;
        self
    }

    #[must_use]
    pub fn indeterminate(mut self, value: bool) -> Self {
        if value {
            self.value = None;
        } else if self.value.is_none() {
            self.value = Some(self.min);
        }
        self
    }

    #[must_use]
    pub fn range(mut self, min: f32, max: f32) -> Self {
        if min.is_finite() && max.is_finite() {
            self.min = min.min(max);
            self.max = max.max(min);
        }
        self
    }

    #[must_use]
    pub fn size(mut self, size: f32) -> Self {
        self.size = finite_non_negative(size);
        self
    }

    #[must_use]
    pub fn stroke_width(mut self, stroke_width: f32) -> Self {
        self.stroke_width = finite_non_negative(stroke_width);
        self
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    #[must_use]
    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let indicator_theme = current_progress_indicator_theme().unwrap_or_default();
        let size = self.size.max(1.0);
        let stroke = self
            .stroke_width
            .max(indicator_theme.stroke_width.unwrap_or(0.0))
            .min(size * 0.5)
            .max(0.1);
        let track = self
            .background_color
            .or(indicator_theme.circular_track_color)
            .unwrap_or_else(|| with_alpha(theme.colors.border, 96));
        let progress = self
            .color
            .or(indicator_theme.color)
            .unwrap_or(theme.colors.accent);
        let ratio = normalized(self.value, self.min, self.max);
        let center = Offset::new(size * 0.5, size * 0.5);
        let radius = (size - stroke) * 0.5;
        let mut canvas = Canvas::default();
        let track_path = Arc::new(circle_arc(center, radius, 0.0, std::f32::consts::TAU));
        canvas.stroke_path(
            track_path,
            track,
            Stroke {
                width: stroke,
                cap: LineCap::Round,
                join: LineJoin::Round,
                miter_limit: 4.0,
            },
        );
        let progress_end = ratio.map_or(1.25 * std::f32::consts::PI, |value| {
            -std::f32::consts::FRAC_PI_2 + value * std::f32::consts::TAU
        });
        let progress_start = -std::f32::consts::FRAC_PI_2;
        if ratio.is_none_or(|value| value > 0.0) {
            let progress_path = Arc::new(circle_arc(center, radius, progress_start, progress_end));
            canvas.stroke_path(
                progress_path,
                progress,
                Stroke {
                    width: stroke,
                    cap: LineCap::Round,
                    join: LineJoin::Round,
                    miter_limit: 4.0,
                },
            );
        }
        let visual: Widget =
            incular_widgets::CustomPaint::new(Size::new(size, size), canvas.finish()).into();
        let value_text = ratio
            .map(|value| format!("{:.0}%", value * 100.0))
            .unwrap_or_else(|| "In progress".to_owned());
        Semantics::new(visual)
            .label(self.label.clone().unwrap_or_else(|| "Progress".to_owned()))
            .value(value_text)
            .into()
    }
}

fn circle_arc(center: Offset, radius: f32, start: f32, end: f32) -> Path {
    let span = (end - start).abs().max(0.001);
    let steps = ((span / (std::f32::consts::TAU / 48.0)).ceil() as usize).clamp(2, 96);
    let direction = if end >= start { 1.0 } else { -1.0 };
    let mut builder = Path::builder();
    for index in 0..=steps {
        let t = index as f32 / steps as f32;
        let angle = start + direction * span * t;
        let point = Offset::new(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        );
        if index == 0 {
            builder.move_to(point);
        } else {
            builder.line_to(point);
        }
    }
    builder.build()
}

impl From<CircularProgressIndicator> for Widget {
    fn from(value: CircularProgressIndicator) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}
