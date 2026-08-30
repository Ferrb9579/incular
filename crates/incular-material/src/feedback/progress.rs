use super::helpers::finite_non_negative;
use incular_config::EdgeInsets;
use incular_core::{Color, Size};
use incular_widgets::BorderRadius;
use typed_builder::TypedBuilder;

/// Stroke cap options used by [`ProgressIndicatorThemeData`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProgressIndicatorStrokeCap {
    #[default]
    Butt,
    Round,
    Square,
}

/// Theme data shared by Material linear and circular progress indicators.
///
/// The existing `LinearProgressIndicator` and `CircularProgressIndicator`
/// descriptors consume their own defaults; this record provides the complete
/// retained configuration surface so a parent/theme integration can resolve
/// those defaults without introducing renderer-specific state.
#[derive(Clone, Copy, Debug, Default, PartialEq, TypedBuilder)]
pub struct ProgressIndicatorThemeData {
    #[builder(default, setter(strip_option))]
    pub color: Option<Color>,
    #[builder(default, setter(strip_option))]
    pub linear_track_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    pub circular_track_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    pub refresh_background_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    pub stop_indicator_color: Option<Color>,
    #[builder(default, setter(transform = |value: f32| Some(finite_non_negative(value))))]
    pub stop_indicator_radius: Option<f32>,
    #[builder(default, setter(transform = |value: f32| Some(finite_non_negative(value))))]
    pub stroke_width: Option<f32>,
    #[builder(default, setter(strip_option))]
    pub stroke_cap: Option<ProgressIndicatorStrokeCap>,
    #[builder(default)]
    pub stroke_align: Option<f32>,
    #[builder(default, setter(transform = |value: f32| Some(finite_non_negative(value))))]
    pub track_gap: Option<f32>,
    #[builder(default, setter(transform = |value: f32| Some(finite_non_negative(value))))]
    pub linear_track_height: Option<f32>,
    #[builder(default, setter(strip_option))]
    pub border_radius: Option<BorderRadius>,
    #[builder(default, setter(strip_option))]
    pub padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    pub constraints: Option<Size>,
}

impl ProgressIndicatorThemeData {
    #[must_use]
    pub fn color(mut self, value: Color) -> Self {
        self.color = Some(value);
        self
    }

    #[must_use]
    pub fn linear_track_color(mut self, value: Color) -> Self {
        self.linear_track_color = Some(value);
        self
    }

    #[must_use]
    pub fn circular_track_color(mut self, value: Color) -> Self {
        self.circular_track_color = Some(value);
        self
    }

    #[must_use]
    pub fn refresh_background_color(mut self, value: Color) -> Self {
        self.refresh_background_color = Some(value);
        self
    }

    #[must_use]
    pub fn stop_indicator_color(mut self, value: Color) -> Self {
        self.stop_indicator_color = Some(value);
        self
    }

    #[must_use]
    pub fn stop_indicator_radius(mut self, value: f32) -> Self {
        self.stop_indicator_radius = Some(finite_non_negative(value));
        self
    }

    #[must_use]
    pub fn stroke_width(mut self, value: f32) -> Self {
        self.stroke_width = Some(finite_non_negative(value));
        self
    }

    #[must_use]
    pub fn stroke_cap(mut self, value: ProgressIndicatorStrokeCap) -> Self {
        self.stroke_cap = Some(value);
        self
    }

    #[must_use]
    pub fn stroke_align(mut self, value: f32) -> Self {
        self.stroke_align = Some(value);
        self
    }

    #[must_use]
    pub fn track_gap(mut self, value: f32) -> Self {
        self.track_gap = Some(finite_non_negative(value));
        self
    }

    #[must_use]
    pub fn linear_track_height(mut self, value: f32) -> Self {
        self.linear_track_height = Some(finite_non_negative(value));
        self
    }

    #[must_use]
    pub fn border_radius(mut self, value: BorderRadius) -> Self {
        self.border_radius = Some(value);
        self
    }

    #[must_use]
    pub fn padding(mut self, value: EdgeInsets) -> Self {
        self.padding = Some(value);
        self
    }

    #[must_use]
    pub fn constraints(mut self, value: Size) -> Self {
        self.constraints = Some(value);
        self
    }

    /// Fills unset fields from `fallback`, matching Flutter theme-data merge
    /// semantics while retaining explicit values from `self`.
    #[must_use]
    pub fn merge(self, fallback: Self) -> Self {
        Self {
            color: self.color.or(fallback.color),
            linear_track_color: self.linear_track_color.or(fallback.linear_track_color),
            circular_track_color: self.circular_track_color.or(fallback.circular_track_color),
            refresh_background_color: self
                .refresh_background_color
                .or(fallback.refresh_background_color),
            stop_indicator_color: self.stop_indicator_color.or(fallback.stop_indicator_color),
            stop_indicator_radius: self
                .stop_indicator_radius
                .or(fallback.stop_indicator_radius),
            stroke_width: self.stroke_width.or(fallback.stroke_width),
            stroke_cap: self.stroke_cap.or(fallback.stroke_cap),
            stroke_align: self.stroke_align.or(fallback.stroke_align),
            track_gap: self.track_gap.or(fallback.track_gap),
            linear_track_height: self.linear_track_height.or(fallback.linear_track_height),
            border_radius: self.border_radius.or(fallback.border_radius),
            padding: self.padding.or(fallback.padding),
            constraints: self.constraints.or(fallback.constraints),
        }
    }
}

/// Places progress-indicator theme data in the retained build environment.
#[derive(Clone, TypedBuilder)]
pub struct ProgressIndicatorTheme {
    #[builder(setter(into))]
    data: ProgressIndicatorThemeData,
    #[builder(setter(into))]
    child: incular_widgets::Widget,
}

impl ProgressIndicatorTheme {
    #[must_use]
    pub fn new(
        data: ProgressIndicatorThemeData,
        child: impl Into<incular_widgets::Widget>,
    ) -> Self {
        Self {
            data,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn data(mut self, data: ProgressIndicatorThemeData) -> Self {
        self.data = data;
        self
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<incular_widgets::Widget>) -> Self {
        self.child = child.into();
        self
    }

    #[must_use]
    pub fn data_value(&self) -> ProgressIndicatorThemeData {
        self.data
    }
}

impl From<ProgressIndicatorTheme> for incular_widgets::Widget {
    fn from(value: ProgressIndicatorTheme) -> Self {
        incular_widgets::Widget::environment_scope(value.data, value.child)
    }
}

/// Reads the nearest retained [`ProgressIndicatorThemeData`].
#[must_use]
pub fn current_progress_indicator_theme() -> Option<ProgressIndicatorThemeData> {
    incular_widgets::internal::current_build_environment::<ProgressIndicatorThemeData>()
}
