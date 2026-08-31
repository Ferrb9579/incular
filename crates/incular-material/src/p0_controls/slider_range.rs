use crate::SliderInteraction;
use crate::foundation::{StateProperty, Theme, WidgetState, WidgetStates};
use incular_config::EdgeInsets;
use incular_controls::current_control_theme;
use incular_core::{Color, KeyboardKey, NamedKey};
use incular_semantics::{Role as SemanticRole, SemanticActionKind, SemanticState};
use incular_text::TextStyle;
use incular_widgets::internal::ExplicitSemantics;
use incular_widgets::{
    Border, Container, FocusNode, GestureDetector, HitTestBehavior, KeyboardListener, Positioned,
    Stack, Widget,
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
            if let Some(theme) = Theme::of_shared(context) {
                let slider_theme = &theme.selection_controls().slider_theme;
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
            root.into()
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
        // `incular-controls` currently exposes the single-thumb slider.  The
        // Material range adapter owns only the coordination of two thumbs and
        // delegates gesture recognition to the same retained GestureDetector
        // primitive, so focus/semantics/layout still follow the shared stack.
        let initial_values = value.values.normalized();
        let revision = Rc::new(Cell::new(0_u64));
        let min = value.min.min(value.max);
        let max = value.max.max(value.min);
        let span = (max - min).max(f32::EPSILON);
        let step = value
            .divisions
            .map(|count| span / count.max(1) as f32)
            .unwrap_or(0.01);
        let minimum_separation = value.minimum_separation.unwrap_or(0.0).min(span);
        let initial_start = initial_values.start.clamp(min, max);
        let initial_end = initial_values
            .end
            .clamp((initial_start + minimum_separation).min(max), max);
        let current = Rc::new(Cell::new(RangeValues::new(initial_start, initial_end)));
        let active_thumb = Rc::new(Cell::new(true));
        let on_changed = value.on_changed;
        let on_start = value.on_change_start;
        let on_end = value.on_change_end;
        let enabled = value.enabled;
        let labels = value.labels;
        let rtl = value.rtl;
        let focus_node = value.focus_node;
        let autofocus = value.autofocus;
        let keyboard_quantize: Rc<dyn Fn(f32) -> f32> = Rc::new(move |raw: f32| {
            ((raw.clamp(min, max) - min) / step)
                .round()
                .mul_add(step, min)
                .clamp(min, max)
        });
        let keyboard_on_changed = on_changed.clone();
        let keyboard_on_start = on_start.clone();
        let keyboard_on_end = on_end.clone();
        let content = {
            // The retained builder is an `Fn` and may run for many frames;
            // give it its own cheap `Rc` handles so keyboard state remains
            // available after the first materialization.
            let current = current.clone();
            let revision = revision.clone();
            let active_thumb = active_thumb.clone();
            Widget::stateful_layout_builder(revision.clone(), move |context, constraints| {
                // Use the available width when the parent is bounded, while
                // retaining a compact intrinsic size for unconstrained overlays.
                let track_width = if constraints.max_width.is_finite() {
                    constraints.max_width.clamp(1.0, 480.0)
                } else {
                    180.0
                };
                let theme = current_control_theme(context);
                let track_height = theme.slider.track_height.max(1.0);
                let thumb_size = theme.slider.thumb_size.max(track_height);
                let active_color = theme.colors.accent;
                let inactive_color = theme.colors.border_strong;
                let disabled_color = theme.colors.disabled_foreground;
                let quantize: Rc<dyn Fn(f32) -> f32> = Rc::new(move |raw: f32| {
                    ((raw.clamp(min, max) - min) / step)
                        .round()
                        .mul_add(step, min)
                        .clamp(min, max)
                });
                let values = current.get().normalized();
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
                let thumb = |selected: bool| {
                    Container::new()
                        .width(thumb_size)
                        .height(thumb_size)
                        .radius(thumb_size * 0.5)
                        .color(if selected {
                            active_color
                        } else {
                            theme.colors.surface
                        })
                        .border(Border::new(
                            1.0,
                            if enabled {
                                active_color
                            } else {
                                disabled_color
                            },
                        ))
                };

                let track_active_thumb = active_thumb.clone();
                let track_hit: Widget = GestureDetector::new(
                    Container::new()
                        .width(track_width)
                        .height(32.0)
                        .color(Color::TRANSPARENT),
                )
                .behavior(HitTestBehavior::Opaque)
                .on_tap({
                    let current = current.clone();
                    let revision = revision.clone();
                    let on_changed = on_changed.clone();
                    move || {
                        if !enabled {
                            return;
                        }
                        let mut next = current.get();
                        // Accessible activation advances the nearer thumb by one
                        // division, matching the single-slider fallback.
                        if (next.start - min) <= (max - next.end) {
                            next.start =
                                (next.start + step).min((next.end - minimum_separation).max(min));
                            track_active_thumb.set(true);
                        } else {
                            next.end = (next.end + step)
                                .max(next.start + minimum_separation)
                                .min(max);
                            track_active_thumb.set(false);
                        }
                        current.set(next);
                        revision.set(revision.get().wrapping_add(1));
                        if let Some(callback) = on_changed.as_ref() {
                            callback(next);
                        }
                    }
                })
                .into();

                let start_began = Rc::new(Cell::new(false));
                let start_thumb: Widget = GestureDetector::new(thumb(false))
                    .behavior(HitTestBehavior::Opaque)
                    .on_horizontal_drag_update({
                        let current = current.clone();
                        let revision = revision.clone();
                        let on_changed = on_changed.clone();
                        let on_start = on_start.clone();
                        let quantize = quantize.clone();
                        let began = start_began.clone();
                        let active_thumb = active_thumb.clone();
                        move |delta| {
                            if !enabled {
                                return;
                            }
                            if !began.replace(true) {
                                active_thumb.set(true);
                                if let Some(callback) = on_start.as_ref() {
                                    callback(current.get());
                                }
                            }
                            let mut next = current.get();
                            next.start = quantize(next.start + delta.x / track_width * span)
                                .min((next.end - minimum_separation).max(min));
                            current.set(next);
                            revision.set(revision.get().wrapping_add(1));
                            if let Some(callback) = on_changed.as_ref() {
                                callback(next);
                            }
                        }
                    })
                    .on_horizontal_drag_end({
                        let current = current.clone();
                        let on_end = on_end.clone();
                        let began = start_began.clone();
                        move |_| {
                            if began.replace(false)
                                && let Some(callback) = on_end.as_ref()
                            {
                                callback(current.get());
                            }
                        }
                    })
                    .into();
                let end_began = Rc::new(Cell::new(false));
                let end_thumb: Widget = GestureDetector::new(thumb(false))
                    .behavior(HitTestBehavior::Opaque)
                    .on_horizontal_drag_update({
                        let current = current.clone();
                        let revision = revision.clone();
                        let on_changed = on_changed.clone();
                        let on_start = on_start.clone();
                        let quantize = quantize.clone();
                        let began = end_began.clone();
                        let active_thumb = active_thumb.clone();
                        move |delta| {
                            if !enabled {
                                return;
                            }
                            if !began.replace(true) {
                                active_thumb.set(false);
                                if let Some(callback) = on_start.as_ref() {
                                    callback(current.get());
                                }
                            }
                            let mut next = current.get();
                            next.end = quantize(next.end + delta.x / track_width * span)
                                .max((next.start + minimum_separation).min(max));
                            current.set(next);
                            revision.set(revision.get().wrapping_add(1));
                            if let Some(callback) = on_changed.as_ref() {
                                callback(next);
                            }
                        }
                    })
                    .on_horizontal_drag_end({
                        let current = current.clone();
                        let on_end = on_end.clone();
                        let began = end_began.clone();
                        move |_| {
                            if began.replace(false)
                                && let Some(callback) = on_end.as_ref()
                            {
                                callback(current.get());
                            }
                        }
                    })
                    .into();
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
                let value_text = if let Some(labels) = labels.as_ref() {
                    format!("{} – {}", labels.start, labels.end)
                } else {
                    format!("{:.3} – {:.3}", values.start, values.end)
                };
                let root: Widget = Container::new()
                    .width(track_width)
                    .height(32.0)
                    .child(stack)
                    .into();
                root.semantics(
                    ExplicitSemantics::new(SemanticRole::Slider)
                        .value(value_text)
                        .state(SemanticState {
                            enabled,
                            focusable: enabled,
                            numeric_value: Some(f64::from(values.start)),
                            numeric_min: Some(f64::from(min)),
                            numeric_max: Some(f64::from(max)),
                            numeric_step: Some(f64::from(step)),
                            ..SemanticState::default()
                        })
                        .actions(if enabled {
                            vec![
                                SemanticActionKind::Focus,
                                SemanticActionKind::Activate,
                                SemanticActionKind::Increment,
                                SemanticActionKind::Decrement,
                            ]
                        } else {
                            Vec::new()
                        }),
                )
            })
        };
        if !enabled {
            return content;
        }
        let keyboard_value = current.clone();
        let keyboard_revision = revision.clone();
        let keyboard_active_thumb = active_thumb.clone();
        let keyboard_focus = focus_node.unwrap_or_default();
        let keyboard_quantize = keyboard_quantize.clone();
        KeyboardListener::new(content)
            .focus_node(keyboard_focus)
            .autofocus(autofocus)
            .on_key(move |event| {
                if !event.state.is_down() {
                    return false;
                }
                let action = match &event.key {
                    KeyboardKey::Named(NamedKey::ArrowLeft) => Some(if rtl { 1.0 } else { -1.0 }),
                    KeyboardKey::Named(NamedKey::ArrowRight) => Some(if rtl { -1.0 } else { 1.0 }),
                    KeyboardKey::Named(NamedKey::ArrowUp) => Some(1.0),
                    KeyboardKey::Named(NamedKey::ArrowDown) => Some(-1.0),
                    KeyboardKey::Named(NamedKey::Home) => None,
                    KeyboardKey::Named(NamedKey::End) => None,
                    KeyboardKey::Character(text) if text == " " => Some(1.0),
                    _ => return false,
                };
                let mut next = keyboard_value.get().normalized();
                let start_thumb = keyboard_active_thumb.get();
                let current_value = if start_thumb { next.start } else { next.end };
                let target = match &event.key {
                    KeyboardKey::Named(NamedKey::Home) => min,
                    KeyboardKey::Named(NamedKey::End) => max,
                    _ => current_value + action.unwrap_or(1.0) * step,
                };
                let target = keyboard_quantize(target);
                if start_thumb {
                    next.start = target.min((next.end - minimum_separation).max(min));
                } else {
                    next.end = target.max((next.start + minimum_separation).min(max));
                }
                if next == keyboard_value.get() {
                    return true;
                }
                if let Some(callback) = keyboard_on_start.as_ref() {
                    callback(next);
                }
                keyboard_value.set(next);
                keyboard_revision.set(keyboard_revision.get().wrapping_add(1));
                if let Some(callback) = keyboard_on_changed.as_ref() {
                    callback(next);
                }
                if let Some(callback) = keyboard_on_end.as_ref() {
                    callback(next);
                }
                true
            })
            .into()
    }
}
