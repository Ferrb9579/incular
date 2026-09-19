use crate::SliderInteraction;
use crate::material_theme::{StateProperty, WidgetState, WidgetStates};
use incular_config::EdgeInsets;
use incular_controls::slider::{RangeSliderModel, RangeThumb, RangeValues as ControlRangeValues};
use incular_core::{Color, KeyboardEvent, KeyboardKey, NamedKey};
use incular_semantics::{Role as SemanticRole, SemanticAction, SemanticState};
use incular_text::TextStyle;
use incular_widgets::{
    Border, Container, FocusNode, GestureDetector, HitTestBehavior, KeyboardListener, Positioned,
    Semantics, Stack, TapUpDetails, Widget,
};
use std::cell::Cell;
use std::rc::Rc;
use typed_builder::TypedBuilder;

/// Slider range value used by both single and range sliders.
#[derive(Clone, Copy, Debug, PartialEq, TypedBuilder)]
pub struct RangeValues {
    #[builder(setter(into))]
    pub start: f32,
    #[builder(setter(into))]
    pub end: f32,
}

impl RangeValues {
    #[must_use]
    pub const fn new(start: f32, end: f32) -> Self {
        Self { start, end }
    }

    #[must_use]
    pub fn normalized(self) -> Self {
        if self.start <= self.end {
            self
        } else {
            Self::new(self.end, self.start)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, TypedBuilder)]
pub struct RangeLabels {
    #[builder(setter(into))]
    pub start: String,
    #[builder(setter(into))]
    pub end: String,
}

impl RangeLabels {
    #[must_use]
    pub fn new(start: impl Into<String>, end: impl Into<String>) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
        }
    }
}

/// P0 Material slider theme data. Shape objects remain renderer-neutral; the
/// optional state properties are resolved by the Material wrapper before it
/// enters the controls slider.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
pub struct SliderThemeData {
    #[builder(default, setter(transform = |value: f32| Some(value.max(0.0))))]
    pub track_height: Option<f32>,
    #[builder(default, setter(transform = |value: f32| Some(value.max(1.0))))]
    pub thumb_size: Option<f32>,
    #[builder(default, setter(strip_option, into))]
    pub active_track_color: Option<StateProperty<Color>>,
    #[builder(default, setter(strip_option, into))]
    pub inactive_track_color: Option<StateProperty<Color>>,
    #[builder(default, setter(strip_option, into))]
    pub secondary_active_track_color: Option<StateProperty<Color>>,
    #[builder(default, setter(strip_option))]
    pub disabled_active_track_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    pub disabled_inactive_track_color: Option<Color>,
    #[builder(default, setter(strip_option, into))]
    pub thumb_color: Option<StateProperty<Color>>,
    #[builder(default, setter(strip_option, into))]
    pub overlay_color: Option<StateProperty<Color>>,
    #[builder(default, setter(strip_option, into))]
    pub tick_mark_color: Option<StateProperty<Color>>,
    #[builder(default, setter(strip_option))]
    pub value_indicator_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    pub value_indicator_text_style: Option<TextStyle>,
    #[builder(default, setter(strip_option))]
    pub show_value_indicator: Option<bool>,
    #[builder(default, setter(strip_option))]
    pub padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    pub min: Option<f32>,
    #[builder(default, setter(strip_option))]
    pub max: Option<f32>,
}

impl SliderThemeData {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn track_height(mut self, value: f32) -> Self {
        self.track_height = Some(value.max(0.0));
        self
    }

    /// Sets the logical diameter used by the shared slider thumb renderer.
    #[must_use]
    pub fn thumb_size(mut self, value: f32) -> Self {
        self.thumb_size = Some(value.max(1.0));
        self
    }

    #[must_use]
    pub fn active_track_color(mut self, value: impl Into<StateProperty<Color>>) -> Self {
        self.active_track_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn inactive_track_color(mut self, value: impl Into<StateProperty<Color>>) -> Self {
        self.inactive_track_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn secondary_active_track_color(mut self, value: impl Into<StateProperty<Color>>) -> Self {
        self.secondary_active_track_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn disabled_active_track_color(mut self, value: Color) -> Self {
        self.disabled_active_track_color = Some(value);
        self
    }

    #[must_use]
    pub fn disabled_inactive_track_color(mut self, value: Color) -> Self {
        self.disabled_inactive_track_color = Some(value);
        self
    }

    #[must_use]
    pub fn thumb_color(mut self, value: impl Into<StateProperty<Color>>) -> Self {
        self.thumb_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn overlay_color(mut self, value: impl Into<StateProperty<Color>>) -> Self {
        self.overlay_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn tick_mark_color(mut self, value: impl Into<StateProperty<Color>>) -> Self {
        self.tick_mark_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn value_indicator_color(mut self, value: Color) -> Self {
        self.value_indicator_color = Some(value);
        self
    }

    #[must_use]
    pub fn value_indicator_text_style(mut self, value: TextStyle) -> Self {
        self.value_indicator_text_style = Some(value);
        self
    }

    #[must_use]
    pub fn show_value_indicator(mut self, value: bool) -> Self {
        self.show_value_indicator = Some(value);
        self
    }

    #[must_use]
    pub fn padding(mut self, value: EdgeInsets) -> Self {
        self.padding = Some(value);
        self
    }

    #[must_use]
    pub fn min(mut self, value: f32) -> Self {
        self.min = Some(value);
        self
    }

    #[must_use]
    pub fn max(mut self, value: f32) -> Self {
        self.max = Some(value);
        self
    }
}

/// Controlled Material slider. The controls slider owns pointer anatomy and
/// semantics; this wrapper maps Material's range/division/callback vocabulary.
#[derive(Clone, TypedBuilder)]
pub struct Slider {
    #[builder(default = 0.0)]
    value: f32,
    #[builder(default = 0.0)]
    min: f32,
    #[builder(default = 1.0)]
    max: f32,
    #[builder(default, setter(strip_option))]
    divisions: Option<usize>,
    #[builder(default, setter(strip_option))]
    secondary_track_value: Option<f32>,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default, setter(strip_option, into))]
    label: Option<String>,
    #[builder(default = SliderInteraction::TapAndSlide)]
    interaction: SliderInteraction,
    #[builder(default)]
    rtl: bool,
    #[builder(default, setter(strip_option))]
    focus_node: Option<FocusNode>,
    #[builder(default)]
    autofocus: bool,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn(f32) + 'static>>
            where
                F: Fn(f32) + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_changed: Option<Rc<dyn Fn(f32) + 'static>>,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn(f32) + 'static>>
            where
                F: Fn(f32) + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_change_start: Option<Rc<dyn Fn(f32) + 'static>>,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn(f32) + 'static>>
            where
                F: Fn(f32) + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_change_end: Option<Rc<dyn Fn(f32) + 'static>>,
}

impl Default for Slider {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl Slider {
    #[must_use]
    pub fn new(value: f32) -> Self {
        Self {
            value,
            min: 0.0,
            max: 1.0,
            divisions: None,
            secondary_track_value: None,
            enabled: true,
            label: None,
            interaction: SliderInteraction::TapAndSlide,
            rtl: false,
            focus_node: None,
            autofocus: false,
            on_changed: None,
            on_change_start: None,
            on_change_end: None,
        }
    }

    #[must_use]
    pub fn defaulted() -> Self {
        Self::new(0.0)
    }

    #[must_use]
    pub fn value(mut self, value: f32) -> Self {
        self.value = value;
        self
    }

    #[must_use]
    pub fn min(mut self, value: f32) -> Self {
        self.min = value;
        self
    }

    #[must_use]
    pub fn max(mut self, value: f32) -> Self {
        self.max = value;
        self
    }

    #[must_use]
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min = min.min(max);
        self.max = max.max(min);
        self
    }

    #[must_use]
    pub fn divisions(mut self, value: Option<usize>) -> Self {
        self.divisions = value;
        self
    }

    #[must_use]
    pub fn label(mut self, value: impl Into<String>) -> Self {
        self.label = Some(value.into());
        self
    }

    #[must_use]
    pub fn secondary_track_value(mut self, value: Option<f32>) -> Self {
        self.secondary_track_value = value;
        self
    }

    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }

    #[must_use]
    pub fn disabled(self, value: bool) -> Self {
        self.enabled(!value)
    }

    #[must_use]
    pub fn interaction(mut self, value: SliderInteraction) -> Self {
        self.interaction = value;
        self
    }

    /// Configures logical left/right keyboard behavior for horizontal sliders.
    /// Set this from the ambient directionality when constructing a slider
    /// outside a `Directionality` scope.
    #[must_use]
    pub fn rtl(mut self, value: bool) -> Self {
        self.rtl = value;
        self
    }

    /// Uses an existing retained focus node for keyboard and accessibility
    /// interaction.
    #[must_use]
    pub fn focus_node(mut self, value: FocusNode) -> Self {
        self.focus_node = Some(value);
        self
    }

    /// Requests keyboard focus when the slider is mounted.
    #[must_use]
    pub fn autofocus(mut self, value: bool) -> Self {
        self.autofocus = value;
        self
    }

    #[must_use]
    pub fn on_changed(mut self, callback: impl Fn(f32) + 'static) -> Self {
        self.on_changed = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_change_start(mut self, callback: impl Fn(f32) + 'static) -> Self {
        self.on_change_start = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_change_end(mut self, callback: impl Fn(f32) + 'static) -> Self {
        self.on_change_end = Some(Rc::new(callback));
        self
    }
}

impl From<Slider> for Widget {
    fn from(value: Slider) -> Self {
        let value = Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            let value = value.as_ref().clone();
            let step = value
                .divisions
                .map(|divisions| (value.max - value.min) / divisions.max(1) as f32)
                .unwrap_or(0.01);
            let (tap_enabled, drag_enabled) = match value.interaction {
                SliderInteraction::TapAndSlide => (true, true),
                SliderInteraction::TapOnly => (true, false),
                SliderInteraction::SlideOnly | SliderInteraction::SlideThumb => (false, true),
            };
            let (theme, selection) = super::resolved_control_theme(context);
            let mut root = incular_controls::slider::Root::new()
                .range(value.min, value.max)
                .step(step)
                .value(value.value)
                .secondary_value(value.secondary_track_value)
                .tap_enabled(tap_enabled)
                .drag_enabled(drag_enabled)
                .rtl(value.rtl)
                .disabled(!value.enabled);
            if let Some(node) = value.focus_node {
                root = root.focus_node(node);
            }
            root = root.autofocus(value.autofocus);
            if let Some(label) = value.label.clone() {
                root = root.semantic_value(label);
            }
            if let Some(selection) = selection.as_ref() {
                let slider_theme = &selection.slider_theme;
                let state = if value.enabled {
                    WidgetStates::default()
                } else {
                    WidgetStates::default().with(WidgetState::Disabled)
                };
                if let Some(color) = slider_theme
                    .active_track_color
                    .as_ref()
                    .map(|property| property.resolve(state))
                {
                    root = root.active_track_color(color);
                }
                if let Some(color) = slider_theme
                    .inactive_track_color
                    .as_ref()
                    .map(|property| property.resolve(state))
                {
                    root = root.inactive_track_color(color);
                }
                if let Some(color) = slider_theme
                    .secondary_active_track_color
                    .as_ref()
                    .map(|property| property.resolve(state))
                {
                    root = root.secondary_track_color(color);
                }
                if let Some(color) = slider_theme
                    .thumb_color
                    .as_ref()
                    .map(|property| property.resolve(state))
                {
                    root = root.thumb_color(color);
                }
                if !value.enabled {
                    if let Some(color) = slider_theme.disabled_active_track_color {
                        root = root.active_track_color(color);
                    }
                    if let Some(color) = slider_theme.disabled_inactive_track_color {
                        root = root.inactive_track_color(color);
                    }
                }
            }
            if let Some(callback) = value.on_changed {
                root = root.on_value_change(move |next| callback(next));
            }
            if let Some(callback) = value.on_change_start {
                root = root.on_change_start(move |next| callback(next));
            }
            if let Some(callback) = value.on_change_end {
                root = root.on_change_end(move |next| callback(next));
            }
            root.build(&theme)
        }))
    }
}

/// Material range slider with two controlled thumbs.
#[derive(Clone, TypedBuilder)]
pub struct RangeSlider {
    #[builder(setter(transform = |values: RangeValues| values.normalized()))]
    pub(super) values: RangeValues,
    #[builder(default = 0.0)]
    min: f32,
    #[builder(default = 1.0)]
    max: f32,
    #[builder(default, setter(strip_option))]
    divisions: Option<usize>,
    #[builder(default, setter(transform = |value: f32| Some(value.max(0.0))))]
    pub(super) minimum_separation: Option<f32>,
    #[builder(default, setter(strip_option))]
    labels: Option<RangeLabels>,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default)]
    rtl: bool,
    #[builder(default, setter(strip_option))]
    focus_node: Option<FocusNode>,
    #[builder(default)]
    autofocus: bool,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn(RangeValues) + 'static>>
            where
                F: Fn(RangeValues) + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_changed: Option<Rc<dyn Fn(RangeValues) + 'static>>,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn(RangeValues) + 'static>>
            where
                F: Fn(RangeValues) + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_change_start: Option<Rc<dyn Fn(RangeValues) + 'static>>,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn(RangeValues) + 'static>>
            where
                F: Fn(RangeValues) + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_change_end: Option<Rc<dyn Fn(RangeValues) + 'static>>,
}

impl RangeSlider {
    #[must_use]
    pub fn new(values: RangeValues) -> Self {
        Self {
            values: values.normalized(),
            min: 0.0,
            max: 1.0,
            divisions: None,
            minimum_separation: None,
            labels: None,
            enabled: true,
            rtl: false,
            focus_node: None,
            autofocus: false,
            on_changed: None,
            on_change_start: None,
            on_change_end: None,
        }
    }

    #[must_use]
    pub fn values(mut self, values: RangeValues) -> Self {
        self.values = values.normalized();
        self
    }

    #[must_use]
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min = min.min(max);
        self.max = max.max(min);
        self
    }

    #[must_use]
    pub fn min(mut self, value: f32) -> Self {
        self.min = value.min(self.max);
        self
    }

    #[must_use]
    pub fn max(mut self, value: f32) -> Self {
        self.max = value.max(self.min);
        self
    }

    #[must_use]
    pub fn divisions(mut self, value: Option<usize>) -> Self {
        self.divisions = value;
        self
    }

    /// Keeps the two thumbs apart by at least this logical value.
    #[must_use]
    pub fn minimum_separation(mut self, value: f32) -> Self {
        self.minimum_separation = Some(value.max(0.0));
        self
    }

    #[must_use]
    pub fn labels(mut self, value: RangeLabels) -> Self {
        self.labels = Some(value);
        self
    }

    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }

    #[must_use]
    pub fn rtl(mut self, value: bool) -> Self {
        self.rtl = value;
        self
    }

    #[must_use]
    pub fn focus_node(mut self, value: FocusNode) -> Self {
        self.focus_node = Some(value);
        self
    }

    #[must_use]
    pub fn autofocus(mut self, value: bool) -> Self {
        self.autofocus = value;
        self
    }

    #[must_use]
    pub fn on_changed(mut self, callback: impl Fn(RangeValues) + 'static) -> Self {
        self.on_changed = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_change_start(mut self, callback: impl Fn(RangeValues) + 'static) -> Self {
        self.on_change_start = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_change_end(mut self, callback: impl Fn(RangeValues) + 'static) -> Self {
        self.on_change_end = Some(Rc::new(callback));
        self
    }
}

impl From<RangeSlider> for Widget {
    fn from(value: RangeSlider) -> Self {
        let initial_values = value.values.normalized();
        let min = value.min.min(value.max);
        let max = value.max.max(value.min);
        let span = (max - min).max(f32::EPSILON);
        let step = value
            .divisions
            .map(|count| span / count.max(1) as f32)
            .unwrap_or(0.01);
        let minimum_separation = value.minimum_separation.unwrap_or(0.0).min(span);
        let model = RangeSliderModel::new(
            ControlRangeValues::new(initial_values.start, initial_values.end),
            min,
            max,
            step,
            minimum_separation,
        );
        let revision = model.revision();
        let start_dragging = Rc::new(Cell::new(false));
        let end_dragging = Rc::new(Cell::new(false));
        let start_drag_origin = Rc::new(Cell::new(None::<ControlRangeValues>));
        let end_drag_origin = Rc::new(Cell::new(None::<ControlRangeValues>));
        let on_changed = value.on_changed;
        let on_start = value.on_change_start;
        let on_end = value.on_change_end;
        let enabled = value.enabled;
        let labels = value.labels;
        let rtl = value.rtl;
        let start_focus = value.focus_node.unwrap_or_default();
        let end_focus = FocusNode::new();
        let autofocus = value.autofocus;
        let model_for_build = model.clone();
        Widget::stateful_layout_builder(revision, move |context, constraints| {
            let track_width = if constraints.max_width().is_finite() {
                constraints.max_width().clamp(1.0, 480.0)
            } else {
                180.0
            };
            let (theme, selection) = super::resolved_control_theme(context);
            let track_height = theme.slider.track_height.max(1.0);
            let thumb_size = theme.slider.thumb_size.max(track_height);
            let states = if enabled {
                WidgetStates::default()
            } else {
                WidgetStates::default().with(WidgetState::Disabled)
            };
            let slider_theme = selection.as_ref().map(|selection| &selection.slider_theme);
            let active_color = slider_theme
                .and_then(|theme| theme.active_track_color.as_ref())
                .map_or(theme.colors.accent, |property| property.resolve(states));
            let inactive_color = slider_theme
                .and_then(|theme| theme.inactive_track_color.as_ref())
                .map_or(theme.colors.border_strong, |property| {
                    property.resolve(states)
                });
            let thumb_color = slider_theme
                .and_then(|theme| theme.thumb_color.as_ref())
                .map_or(theme.colors.surface, |property| property.resolve(states));
            let disabled_color = if enabled {
                theme.colors.disabled_foreground
            } else {
                slider_theme
                    .and_then(|theme| theme.disabled_inactive_track_color)
                    .unwrap_or(theme.colors.disabled_foreground)
            };
            let values = model_for_build.values();
            let start_ratio = ((values.start - min) / span).clamp(0.0, 1.0);
            let end_ratio = ((values.end - min) / span).clamp(0.0, 1.0);

            let track = Container::new()
                .width(track_width)
                .height(track_height)
                .radius(track_height * 0.5)
                .color(if enabled {
                    inactive_color
                } else {
                    disabled_color
                });
            let active = Container::new()
                .width((end_ratio - start_ratio) * track_width)
                .height(track_height)
                .radius(track_height * 0.5)
                .color(if enabled {
                    active_color
                } else {
                    disabled_color
                });
            let thumb = || {
                Container::new()
                    .width(thumb_size)
                    .height(thumb_size)
                    .radius(thumb_size * 0.5)
                    .color(thumb_color)
                    .border(Border::new(
                        1.0,
                        if enabled {
                            active_color
                        } else {
                            disabled_color
                        },
                    ))
            };

            let track_hit: Widget = GestureDetector::new(
                Container::new()
                    .width(track_width)
                    .height(32.0)
                    .color(Color::TRANSPARENT),
            )
            .behavior(HitTestBehavior::Opaque)
            .on_tap_up({
                let model = model_for_build.clone();
                let on_changed = on_changed.clone();
                move |details: TapUpDetails| {
                    if !enabled {
                        return;
                    }
                    let mut ratio = (details.local_position.x / track_width).clamp(0.0, 1.0);
                    if rtl {
                        ratio = 1.0 - ratio;
                    }
                    let raw = min + ratio * span;
                    let current = model.values();
                    let thumb = if (raw - current.start).abs() <= (raw - current.end).abs() {
                        RangeThumb::Start
                    } else {
                        RangeThumb::End
                    };
                    if let Some(next) = model.set_thumb(thumb, raw) {
                        notify_range(&on_changed, next);
                    }
                }
            })
            .into();

            let start_thumb = range_thumb_widget(
                thumb().into(),
                RangeThumb::Start,
                &model_for_build,
                track_width,
                span,
                rtl,
                enabled,
                start_focus.clone(),
                autofocus,
                start_dragging.clone(),
                start_drag_origin.clone(),
                on_start.clone(),
                on_changed.clone(),
                on_end.clone(),
                labels.as_ref().map(|labels| labels.start.clone()),
            );
            let end_thumb = range_thumb_widget(
                thumb().into(),
                RangeThumb::End,
                &model_for_build,
                track_width,
                span,
                rtl,
                enabled,
                end_focus.clone(),
                false,
                end_dragging.clone(),
                end_drag_origin.clone(),
                on_start.clone(),
                on_changed.clone(),
                on_end.clone(),
                labels.as_ref().map(|labels| labels.end.clone()),
            );

            let stack: Widget = Stack::new([
                Widget::from(Positioned::new(track).left(0.0).top(14.0)),
                Widget::from(
                    Positioned::new(active)
                        .left(start_ratio * track_width)
                        .top(14.0),
                ),
                Widget::from(Positioned::new(track_hit).left(0.0).top(0.0)),
                Widget::from(
                    Positioned::new(start_thumb)
                        .left((start_ratio * track_width - thumb_size * 0.5).max(0.0))
                        .top((14.0 + track_height * 0.5 - thumb_size * 0.5).max(0.0)),
                ),
                Widget::from(
                    Positioned::new(end_thumb)
                        .left((end_ratio * track_width - thumb_size * 0.5).max(0.0))
                        .top((14.0 + track_height * 0.5 - thumb_size * 0.5).max(0.0)),
                ),
            ])
            .into();
            Container::new()
                .width(track_width)
                .height(32.0)
                .child(stack)
                .into()
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn range_thumb_widget(
    visual: Widget,
    thumb: RangeThumb,
    model: &RangeSliderModel,
    track_width: f32,
    span: f32,
    rtl: bool,
    enabled: bool,
    focus_node: FocusNode,
    autofocus: bool,
    dragging: Rc<Cell<bool>>,
    drag_origin: Rc<Cell<Option<ControlRangeValues>>>,
    on_start: Option<Rc<dyn Fn(RangeValues) + 'static>>,
    on_changed: Option<Rc<dyn Fn(RangeValues) + 'static>>,
    on_end: Option<Rc<dyn Fn(RangeValues) + 'static>>,
    semantic_label: Option<String>,
) -> Widget {
    let mut interactive: Widget = GestureDetector::new(visual)
        .behavior(HitTestBehavior::Opaque)
        .on_horizontal_drag_update({
            let model = model.clone();
            let dragging = dragging.clone();
            let drag_origin = drag_origin.clone();
            let on_start = on_start.clone();
            let on_changed = on_changed.clone();
            move |delta| {
                if !enabled {
                    return;
                }
                if !dragging.replace(true) {
                    let origin = model.values();
                    drag_origin.set(Some(origin));
                    notify_range(&on_start, origin);
                }
                let origin = drag_origin.get().unwrap_or_else(|| model.values());
                let direction = if rtl { -1.0 } else { 1.0 };
                if let Some(next) =
                    model.set_from_origin(thumb, origin, direction * delta.x / track_width * span)
                {
                    notify_range(&on_changed, next);
                }
            }
        })
        .on_horizontal_drag_end({
            let model = model.clone();
            let dragging = dragging.clone();
            let drag_origin = drag_origin.clone();
            let on_end = on_end.clone();
            move |_| {
                drag_origin.set(None);
                if dragging.replace(false) {
                    notify_range(&on_end, model.values());
                }
            }
        })
        .into();

    if enabled {
        let keyboard_model = model.clone();
        let keyboard_on_start = on_start.clone();
        let keyboard_on_changed = on_changed.clone();
        let keyboard_on_end = on_end.clone();
        interactive = KeyboardListener::new(interactive)
            .focus_node(focus_node)
            .autofocus(autofocus)
            .on_key(move |event| {
                handle_range_key(
                    &event,
                    &keyboard_model,
                    thumb,
                    rtl,
                    &keyboard_on_start,
                    &keyboard_on_changed,
                    &keyboard_on_end,
                )
            })
            .into();
    }

    let values = model.values();
    let numeric_value = match thumb {
        RangeThumb::Start => values.start,
        RangeThumb::End => values.end,
    };
    let value_text = semantic_label.unwrap_or_else(|| format!("{numeric_value:.3}"));
    let mut semantics = Semantics::new(interactive)
        .role(SemanticRole::Slider)
        .label(match thumb {
            RangeThumb::Start => "Range start",
            RangeThumb::End => "Range end",
        })
        .value(value_text)
        .state(SemanticState {
            enabled,
            focusable: enabled,
            numeric_value: Some(f64::from(numeric_value)),
            numeric_min: Some(f64::from(model.min())),
            numeric_max: Some(f64::from(model.max())),
            numeric_step: Some(f64::from(model.step())),
            ..SemanticState::default()
        });
    if enabled {
        semantics = semantics.action(SemanticAction::Focus);
        let increase_model = model.clone();
        let increase_changed = on_changed.clone();
        semantics = semantics.on_increase(move || {
            if let Some(next) = increase_model.adjust(thumb, increase_model.step()) {
                notify_range(&increase_changed, next);
            }
        });
        let decrease_model = model.clone();
        let decrease_changed = on_changed;
        semantics = semantics.on_decrease(move || {
            if let Some(next) = decrease_model.adjust(thumb, -decrease_model.step()) {
                notify_range(&decrease_changed, next);
            }
        });
    }
    semantics.into()
}

fn handle_range_key(
    event: &KeyboardEvent,
    model: &RangeSliderModel,
    thumb: RangeThumb,
    rtl: bool,
    on_start: &Option<Rc<dyn Fn(RangeValues) + 'static>>,
    on_changed: &Option<Rc<dyn Fn(RangeValues) + 'static>>,
    on_end: &Option<Rc<dyn Fn(RangeValues) + 'static>>,
) -> bool {
    if !event.state.is_down() {
        return false;
    }
    let before = model.values();
    let next = match &event.key {
        KeyboardKey::Named(NamedKey::ArrowLeft) => {
            model.adjust(thumb, if rtl { model.step() } else { -model.step() })
        }
        KeyboardKey::Named(NamedKey::ArrowRight) => {
            model.adjust(thumb, if rtl { -model.step() } else { model.step() })
        }
        KeyboardKey::Named(NamedKey::ArrowUp) => model.adjust(thumb, model.step()),
        KeyboardKey::Named(NamedKey::ArrowDown) => model.adjust(thumb, -model.step()),
        KeyboardKey::Named(NamedKey::Home) => model.set_edge(thumb, false),
        KeyboardKey::Named(NamedKey::End) => model.set_edge(thumb, true),
        KeyboardKey::Character(text) if text == " " => model.adjust(thumb, model.step()),
        _ => return false,
    };
    if let Some(next) = next {
        notify_range(on_start, before);
        notify_range(on_changed, next);
        notify_range(on_end, next);
    }
    true
}

fn notify_range(callback: &Option<Rc<dyn Fn(RangeValues) + 'static>>, values: ControlRangeValues) {
    if let Some(callback) = callback {
        callback(RangeValues::new(values.start, values.end));
    }
}
