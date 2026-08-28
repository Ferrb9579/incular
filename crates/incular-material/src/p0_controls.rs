//! Material P0 adapters for stateful controls, sliders, and tabs.
//!
//! The modules in `incular-controls` own the retained interaction mechanics.
//! These descriptors only add Material vocabulary, defaults, theme slots, and
//! the public Rust API expected by an ordinary Material application.

use crate::foundation::{StateProperty, Theme, WidgetState, WidgetStates};
use crate::{SliderInteraction, TabAlignment, TabBarIndicatorSize};
use incular_config::{CrossAxisAlignment, EdgeInsets, MainAxisAlignment};
use incular_controls::{CheckedState, current_control_theme};
use incular_core::{Color, KeyboardKey, NamedKey};
use incular_semantics::{Role as SemanticRole, SemanticActionKind, SemanticState};
use incular_text::TextStyle;
use incular_widgets::internal::ExplicitSemantics;
use incular_widgets::{
    Border, BorderRadius, Column, Container, FocusNode, GestureDetector, HitTestBehavior,
    KeyboardListener, PageView, Positioned, Row, Stack, Text, Widget,
};
use std::cell::Cell;
use std::rc::Rc;

/// Material checkbox with a controlled nullable value.
#[derive(Clone)]
pub struct Checkbox {
    value: Option<bool>,
    tristate: bool,
    enabled: bool,
    autofocus: bool,
    is_error: bool,
    fill_color: Option<StateProperty<Color>>,
    check_color: Option<StateProperty<Color>>,
    side: Option<Border>,
    shape: Option<BorderRadius>,
    on_changed: Option<Rc<dyn Fn(Option<bool>) + 'static>>,
}

impl Checkbox {
    #[must_use]
    pub fn new(value: bool) -> Self {
        Self {
            value: Some(value),
            tristate: false,
            enabled: true,
            autofocus: false,
            is_error: false,
            fill_color: None,
            check_color: None,
            side: None,
            shape: None,
            on_changed: None,
        }
    }

    #[must_use]
    pub fn value(mut self, value: Option<bool>) -> Self {
        self.value = value;
        self
    }

    #[must_use]
    pub fn tristate(mut self, value: bool) -> Self {
        self.tristate = value;
        if !value && self.value.is_none() {
            self.value = Some(false);
        }
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
    pub fn autofocus(mut self, value: bool) -> Self {
        self.autofocus = value;
        self
    }

    #[must_use]
    pub fn is_error(mut self, value: bool) -> Self {
        self.is_error = value;
        self
    }

    /// Resolves the Material fill against the initial checked/error state and
    /// passes the resulting paint value to the shared controls root.
    #[must_use]
    pub fn fill_color(mut self, value: impl Into<StateProperty<Color>>) -> Self {
        self.fill_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn check_color(mut self, value: impl Into<StateProperty<Color>>) -> Self {
        self.check_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn side(mut self, value: Border) -> Self {
        self.side = Some(value);
        self
    }

    #[must_use]
    pub fn shape(mut self, value: BorderRadius) -> Self {
        self.shape = Some(value);
        self
    }

    #[must_use]
    pub fn on_changed(mut self, callback: impl Fn(Option<bool>) + 'static) -> Self {
        self.on_changed = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_changed_bool(mut self, callback: impl Fn(bool) + 'static) -> Self {
        self.on_changed = Some(Rc::new(move |value| callback(value.unwrap_or(false))));
        self
    }

    #[must_use]
    pub fn is_checked(&self) -> Option<bool> {
        self.value
    }

    fn build(self) -> Widget {
        let state = match self.value {
            Some(true) => CheckedState::Checked,
            Some(false) => CheckedState::Unchecked,
            None => CheckedState::Indeterminate,
        };
        let mut root = incular_controls::checkbox::Root::new()
            .state(state)
            .enabled(self.enabled)
            .indeterminate(self.tristate && self.value.is_none());
        let mut states = WidgetStates::default();
        if self.value == Some(true) {
            states = states.with(WidgetState::Selected);
        }
        if self.is_error {
            states = states.with(WidgetState::Error);
        }
        if let Some(property) = self.fill_color {
            root = root.active_color(property.resolve(states));
        }
        if let Some(property) = self.check_color {
            root = root.check_color(property.resolve(states));
        }
        if let Some(side) = self.side {
            root = root.side(side);
        }
        if let Some(shape) = self.shape {
            root = root.shape(shape);
        }
        if let Some(callback) = self.on_changed {
            root = root.on_checked_change(move |next| {
                callback(match next {
                    CheckedState::Checked => Some(true),
                    CheckedState::Unchecked => Some(false),
                    CheckedState::Indeterminate => None,
                });
            });
        }
        // The shared root owns checkbox interaction. Attach a retained focus
        // node only for the explicit Material autofocus request so ordinary
        // checkboxes do not gain an extra traversal target.
        let widget: Widget = root.into();
        let _ = self.is_error;
        if self.autofocus {
            incular_widgets::Focus::new(widget).autofocus(true).into()
        } else {
            widget
        }
    }
}

impl From<Checkbox> for Widget {
    fn from(value: Checkbox) -> Self {
        value.build()
    }
}

/// Material radio with a typed group value.
#[derive(Clone)]
pub struct Radio<T: Clone + PartialEq + 'static> {
    value: T,
    group_value: Option<T>,
    enabled: bool,
    toggleable: bool,
    autofocus: bool,
    fill_color: Option<StateProperty<Color>>,
    check_color: Option<StateProperty<Color>>,
    side: Option<Border>,
    on_changed: Option<Rc<dyn Fn(T) + 'static>>,
}

impl<T: Clone + PartialEq + 'static> Radio<T> {
    #[must_use]
    pub fn new(value: T) -> Self {
        Self {
            value,
            group_value: None,
            enabled: true,
            toggleable: false,
            autofocus: false,
            fill_color: None,
            check_color: None,
            side: None,
            on_changed: None,
        }
    }

    #[must_use]
    pub fn group_value(mut self, value: Option<T>) -> Self {
        self.group_value = value;
        self
    }

    #[must_use]
    pub fn selected(self, value: Option<T>) -> Self {
        self.group_value(value)
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
    pub fn toggleable(mut self, value: bool) -> Self {
        self.toggleable = value;
        self
    }

    #[must_use]
    pub fn autofocus(mut self, value: bool) -> Self {
        self.autofocus = value;
        self
    }

    /// Resolves the Material radio fill for selected and unselected states.
    #[must_use]
    pub fn fill_color(mut self, value: impl Into<StateProperty<Color>>) -> Self {
        self.fill_color = Some(value.into());
        self
    }

    /// Sets the color used for the selected radio dot.
    #[must_use]
    pub fn check_color(mut self, value: impl Into<StateProperty<Color>>) -> Self {
        self.check_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn side(mut self, value: Border) -> Self {
        self.side = Some(value);
        self
    }

    #[must_use]
    pub fn on_changed(mut self, callback: impl Fn(T) + 'static) -> Self {
        self.on_changed = Some(Rc::new(callback));
        self
    }
}

impl<T: Clone + PartialEq + 'static> From<Radio<T>> for Widget {
    fn from(value: Radio<T>) -> Self {
        let autofocus = value.autofocus;
        let selected = value.group_value.as_ref() == Some(&value.value);
        let mut selected_states = WidgetStates::default();
        if selected {
            selected_states = selected_states.with(WidgetState::Selected);
        }
        let mut normal_states = WidgetStates::default();
        if !value.enabled {
            selected_states = selected_states.with(WidgetState::Disabled);
            normal_states = normal_states.with(WidgetState::Disabled);
        }
        let mut root =
            incular_controls::selection::Radio::new(value.value.clone(), value.group_value.clone())
                .enabled(value.enabled)
                .toggleable(value.toggleable);
        if let Some(property) = value.fill_color.as_ref() {
            root = root
                .active_color(property.resolve(selected_states))
                .inactive_color(property.resolve(normal_states));
        }
        if let Some(property) = value.check_color.as_ref() {
            root = root.dot_color(property.resolve(selected_states));
        }
        if let Some(side) = value.side {
            root = root
                .border_color(side.top.color)
                .border_width(side.top.width);
        }
        if let Some(callback) = value.on_changed {
            root = root.on_changed(move |next| callback(next));
        }
        let widget: Widget = root.into();
        if autofocus {
            incular_widgets::Focus::new(widget).autofocus(true).into()
        } else {
            widget
        }
    }
}

/// Retained typed radio-group state. Radio widgets remain controlled by the
/// application; this helper only coordinates a shared selection signal.
#[derive(Clone)]
pub struct RadioGroup<T: Clone + PartialEq + 'static> {
    selected: Rc<std::cell::RefCell<Option<T>>>,
    on_changed: Option<Rc<dyn Fn(T) + 'static>>,
}

impl<T: Clone + PartialEq + 'static> RadioGroup<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            selected: Rc::new(std::cell::RefCell::new(None)),
            on_changed: None,
        }
    }

    #[must_use]
    pub fn selected(&self) -> Option<T> {
        self.selected.borrow().clone()
    }

    pub fn set_selected(&self, value: Option<T>) {
        *self.selected.borrow_mut() = value;
    }

    #[must_use]
    pub fn on_changed(mut self, callback: impl Fn(T) + 'static) -> Self {
        self.on_changed = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn radio(&self, value: T) -> Radio<T> {
        let selected = self.selected();
        let state = self.selected.clone();
        let callback = self.on_changed.clone();
        let next = value.clone();
        let mut radio = Radio::new(value).group_value(selected);
        if let Some(callback) = callback {
            radio = radio.on_changed(move |value| {
                *state.borrow_mut() = Some(value.clone());
                callback(value);
            });
        } else {
            radio = radio.on_changed(move |value| {
                *state.borrow_mut() = Some(value);
            });
        }
        let _ = next;
        radio
    }
}

impl<T: Clone + PartialEq + 'static> Default for RadioGroup<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Material switch adapter over the shared switch mechanics.
#[derive(Clone)]
pub struct Switch {
    value: bool,
    enabled: bool,
    autofocus: bool,
    thumb_color: Option<StateProperty<Color>>,
    track_color: Option<StateProperty<Color>>,
    track_outline_color: Option<StateProperty<Color>>,
    track_outline_width: Option<f32>,
    on_changed: Option<Rc<dyn Fn(bool) + 'static>>,
}

impl Switch {
    #[must_use]
    pub fn new(value: bool) -> Self {
        Self {
            value,
            enabled: true,
            autofocus: false,
            thumb_color: None,
            track_color: None,
            track_outline_color: None,
            track_outline_width: None,
            on_changed: None,
        }
    }

    #[must_use]
    pub fn value(mut self, value: bool) -> Self {
        self.value = value;
        self
    }

    #[must_use]
    pub fn selected(self, value: bool) -> Self {
        self.value(value)
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
    pub fn autofocus(mut self, value: bool) -> Self {
        self.autofocus = value;
        self
    }

    #[must_use]
    pub fn thumb_color(mut self, value: impl Into<StateProperty<Color>>) -> Self {
        self.thumb_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn track_color(mut self, value: impl Into<StateProperty<Color>>) -> Self {
        self.track_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn track_outline_color(mut self, value: impl Into<StateProperty<Color>>) -> Self {
        self.track_outline_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn track_outline_width(mut self, value: f32) -> Self {
        self.track_outline_width = Some(value.max(0.0));
        self
    }

    #[must_use]
    pub fn on_changed(mut self, callback: impl Fn(bool) + 'static) -> Self {
        self.on_changed = Some(Rc::new(callback));
        self
    }
}

impl From<Switch> for Widget {
    fn from(value: Switch) -> Self {
        let mut on_states = WidgetStates::default().with(WidgetState::Selected);
        let mut off_states = WidgetStates::default();
        if !value.enabled {
            on_states = on_states.with(WidgetState::Disabled);
            off_states = off_states.with(WidgetState::Disabled);
        }
        let mut root = incular_controls::switch::Root::new()
            .checked(value.value)
            .enabled(value.enabled);
        if let Some(property) = value.track_color.as_ref() {
            root = root
                .active_track_color(property.resolve(on_states))
                .inactive_track_color(property.resolve(off_states));
        }
        if let Some(property) = value.thumb_color.as_ref() {
            root = root
                .active_thumb_color(property.resolve(on_states))
                .inactive_thumb_color(property.resolve(off_states));
        }
        if let Some(property) = value.track_outline_color.as_ref() {
            root = root.outline_color(property.resolve(if value.value {
                on_states
            } else {
                off_states
            }));
        }
        if let Some(width) = value.track_outline_width {
            root = root.outline_width(width);
        }
        if let Some(callback) = value.on_changed {
            root = root.on_checked_change(move |next| callback(next));
        }
        let widget: Widget = root.into();
        if value.autofocus {
            incular_widgets::Focus::new(widget).autofocus(true).into()
        } else {
            widget
        }
    }
}

/// Slider range value used by both single and range sliders.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangeValues {
    pub start: f32,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RangeLabels {
    pub start: String,
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
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SliderThemeData {
    pub track_height: Option<f32>,
    pub thumb_size: Option<f32>,
    pub active_track_color: Option<StateProperty<Color>>,
    pub inactive_track_color: Option<StateProperty<Color>>,
    pub secondary_active_track_color: Option<StateProperty<Color>>,
    pub disabled_active_track_color: Option<Color>,
    pub disabled_inactive_track_color: Option<Color>,
    pub thumb_color: Option<StateProperty<Color>>,
    pub overlay_color: Option<StateProperty<Color>>,
    pub tick_mark_color: Option<StateProperty<Color>>,
    pub value_indicator_color: Option<Color>,
    pub value_indicator_text_style: Option<TextStyle>,
    pub show_value_indicator: Option<bool>,
    pub padding: Option<EdgeInsets>,
    pub min: Option<f32>,
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
#[derive(Clone)]
pub struct Slider {
    value: f32,
    min: f32,
    max: f32,
    divisions: Option<usize>,
    secondary_track_value: Option<f32>,
    enabled: bool,
    label: Option<String>,
    interaction: SliderInteraction,
    rtl: bool,
    focus_node: Option<FocusNode>,
    autofocus: bool,
    on_changed: Option<Rc<dyn Fn(f32) + 'static>>,
    on_change_start: Option<Rc<dyn Fn(f32) + 'static>>,
    on_change_end: Option<Rc<dyn Fn(f32) + 'static>>,
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
        if let Some(theme) = Theme::of_shared() {
            let slider_theme = &theme.slider_theme;
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
    }
}

/// Material range slider with two controlled thumbs.
#[derive(Clone)]
pub struct RangeSlider {
    values: RangeValues,
    min: f32,
    max: f32,
    divisions: Option<usize>,
    minimum_separation: Option<f32>,
    labels: Option<RangeLabels>,
    enabled: bool,
    rtl: bool,
    focus_node: Option<FocusNode>,
    autofocus: bool,
    on_changed: Option<Rc<dyn Fn(RangeValues) + 'static>>,
    on_change_start: Option<Rc<dyn Fn(RangeValues) + 'static>>,
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
            Widget::stateful_layout_builder(revision.clone(), move |constraints| {
                // Use the available width when the parent is bounded, while
                // retaining a compact intrinsic size for unconstrained overlays.
                let track_width = if constraints.max_width.is_finite() {
                    constraints.max_width.clamp(1.0, 480.0)
                } else {
                    180.0
                };
                let theme = current_control_theme();
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
                            if began.replace(false) {
                                if let Some(callback) = on_end.as_ref() {
                                    callback(current.get());
                                }
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
                            if began.replace(false) {
                                if let Some(callback) = on_end.as_ref() {
                                    callback(current.get());
                                }
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

/// A Material tab descriptor. `text` and `icon` are convenience constructors;
/// a tab may also contain an arbitrary retained widget.
#[derive(Clone)]
pub struct Tab {
    child: Widget,
    icon: Option<Widget>,
    text: Option<String>,
    enabled: bool,
}

impl Tab {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            icon: None,
            text: None,
            enabled: true,
        }
    }

    #[must_use]
    pub fn text(value: impl Into<String>) -> Self {
        let text = value.into();
        Self {
            child: Text::new(text.clone()).into(),
            icon: None,
            text: Some(text),
            enabled: true,
        }
    }

    #[must_use]
    pub fn icon(icon: impl Into<Widget>) -> Self {
        let icon = icon.into();
        Self {
            child: icon.clone(),
            icon: Some(icon),
            text: None,
            enabled: true,
        }
    }

    #[must_use]
    pub fn icon_and_text(icon: impl Into<Widget>, text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            child: Row::new([icon.into(), Text::new(text.clone()).into()])
                .spacing(8.0)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .into(),
            icon: None,
            text: Some(text),
            enabled: true,
        }
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = child.into();
        self
    }

    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl From<Tab> for Widget {
    fn from(value: Tab) -> Self {
        let _ = (value.icon, value.text, value.enabled);
        value.child
    }
}

/// Retained controller shared by `TabBar` and `TabBarView`.
#[derive(Clone, Debug)]
pub struct TabController {
    length: usize,
    index: Rc<Cell<usize>>,
    revision: Rc<Cell<u64>>,
    page_controller: incular_scroll::ScrollController,
    page_extent: Rc<Cell<f32>>,
}

impl TabController {
    #[must_use]
    pub fn new(length: usize) -> Self {
        Self {
            length,
            index: Rc::new(Cell::new(0)),
            revision: Rc::new(Cell::new(0)),
            page_controller: incular_scroll::ScrollController::new(),
            page_extent: Rc::new(Cell::new(600.0)),
        }
    }

    #[must_use]
    pub fn length(&self) -> usize {
        self.length
    }

    #[must_use]
    pub fn index(&self) -> usize {
        self.index.get().min(self.length.saturating_sub(1))
    }

    pub fn set_index(&self, value: usize) {
        let index = value.min(self.length.saturating_sub(1));
        self.index.set(index);
        let offset = index as f32 * self.page_extent.get();
        if self.page_controller.max_offset() > 0.0 {
            self.page_controller.jump_to(offset);
        } else {
            self.page_controller.deferred_jump_to(offset);
        }
        self.revision.set(self.revision.get().wrapping_add(1));
    }

    pub fn animate_to(&self, value: usize) {
        self.set_index(value);
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision.get()
    }

    fn revision_cell(&self) -> Rc<Cell<u64>> {
        self.revision.clone()
    }

    /// Returns the retained page position shared by `TabBarView`.
    #[must_use]
    pub fn page_controller(&self) -> incular_scroll::ScrollController {
        self.page_controller.clone()
    }

    fn set_page_extent(&self, extent: f32) {
        self.page_extent.set(extent.max(1.0));
        let offset = self.index() as f32 * self.page_extent.get();
        if self.page_controller.max_offset() > 0.0 {
            self.page_controller.jump_to(offset);
        } else {
            self.page_controller.deferred_jump_to(offset);
        }
    }
}

impl Default for TabController {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Material tab bar. It deliberately uses ordinary buttons and a retained
/// controller; no second tab navigation engine is introduced.
#[derive(Clone)]
pub struct TabBar {
    tabs: Vec<Tab>,
    controller: Option<TabController>,
    selected_index: usize,
    scrollable: bool,
    alignment: TabAlignment,
    indicator_size: TabBarIndicatorSize,
    indicator_color: Option<Color>,
    on_tap: Option<Rc<dyn Fn(usize) + 'static>>,
}

impl TabBar {
    #[must_use]
    pub fn new(tabs: impl IntoIterator<Item = Tab>) -> Self {
        Self {
            tabs: tabs.into_iter().collect(),
            controller: None,
            selected_index: 0,
            scrollable: false,
            alignment: TabAlignment::Center,
            indicator_size: TabBarIndicatorSize::Tab,
            indicator_color: None,
            on_tap: None,
        }
    }

    #[must_use]
    pub fn controller(mut self, value: TabController) -> Self {
        self.controller = Some(value);
        self
    }

    #[must_use]
    pub fn selected_index(mut self, value: usize) -> Self {
        self.selected_index = value;
        self
    }

    #[must_use]
    pub fn is_scrollable(mut self, value: bool) -> Self {
        self.scrollable = value;
        self
    }

    #[must_use]
    pub fn tab_alignment(mut self, value: TabAlignment) -> Self {
        self.alignment = value;
        self
    }

    #[must_use]
    pub fn indicator_size(mut self, value: TabBarIndicatorSize) -> Self {
        self.indicator_size = value;
        self
    }

    #[must_use]
    pub fn indicator_color(mut self, value: Color) -> Self {
        self.indicator_color = Some(value);
        self
    }

    #[must_use]
    pub fn on_tap(mut self, callback: impl Fn(usize) + 'static) -> Self {
        self.on_tap = Some(Rc::new(callback));
        self
    }
}

impl TabBar {
    fn build(&self) -> Widget {
        let selected = self
            .controller
            .as_ref()
            .map_or(self.selected_index, TabController::index);
        let callback = self.on_tap.clone();
        let controller = self.controller.clone();
        let indicator = self
            .indicator_color
            .unwrap_or(Color::rgba(103, 80, 164, 255));
        let scrollable = self.scrollable;
        let alignment = match self.alignment {
            TabAlignment::Start | TabAlignment::StartOffset => MainAxisAlignment::Start,
            TabAlignment::Center => MainAxisAlignment::Center,
            TabAlignment::Fill => MainAxisAlignment::SpaceEvenly,
        };
        let indicator_size = self.indicator_size;
        let children = self.tabs.iter().cloned().enumerate().map(|(index, tab)| {
            let enabled = tab.enabled;
            let child: Widget = tab.child;
            let mut button = crate::TextButton::with_child(child);
            button = button.enabled(enabled);
            if enabled {
                let controller = controller.clone();
                if let Some(callback) = callback.clone() {
                    button = button.on_click(move || {
                        if let Some(controller) = controller.as_ref() {
                            controller.set_index(index);
                        }
                        callback(index);
                    });
                } else if let Some(controller) = controller {
                    button = button.on_click(move || controller.set_index(index));
                }
            }
            let visual: Widget = button.into();
            if index == selected {
                let indicator_width = match indicator_size {
                    TabBarIndicatorSize::Tab => 48.0,
                    TabBarIndicatorSize::Label => 32.0,
                };
                Column::new([
                    visual,
                    Container::new()
                        .height(2.0)
                        .width(indicator_width)
                        .color(indicator)
                        .into(),
                ])
                .into()
            } else {
                visual
            }
        });
        let row = Row::new(children)
            .main_axis_alignment(alignment)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .into();
        if scrollable {
            incular_widgets::SingleChildScrollView::new(row)
                .scroll_direction(incular_config::Axis::Horizontal)
                .into()
        } else {
            row
        }
    }
}

impl From<TabBar> for Widget {
    fn from(value: TabBar) -> Self {
        let value = Rc::new(value);
        let revision = value
            .controller
            .as_ref()
            .map(TabController::revision_cell)
            .unwrap_or_else(|| Rc::new(Cell::new(0)));
        Widget::stateful_layout_builder(revision, move |_| value.build())
    }
}

/// Tab content backed by the core `PageView` implementation.
#[derive(Clone)]
pub struct TabBarView {
    children: Vec<Widget>,
    controller: Option<TabController>,
    viewport_fraction: f32,
    physics: Option<incular_scroll::ScrollPhysics>,
}

impl TabBarView {
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            children: children.into_iter().map(Into::into).collect(),
            controller: None,
            viewport_fraction: 1.0,
            physics: None,
        }
    }

    #[must_use]
    pub fn controller(mut self, value: TabController) -> Self {
        self.controller = Some(value);
        self
    }

    #[must_use]
    pub fn viewport_fraction(mut self, value: f32) -> Self {
        self.viewport_fraction = value.max(0.01);
        self
    }

    #[must_use]
    pub fn physics(mut self, value: incular_scroll::ScrollPhysics) -> Self {
        self.physics = Some(value);
        self
    }
}

impl From<TabBarView> for Widget {
    fn from(value: TabBarView) -> Self {
        let controller = value
            .controller
            .unwrap_or_else(|| TabController::new(value.children.len()));
        let page_extent = 600.0 * value.viewport_fraction.max(0.01);
        controller.set_page_extent(page_extent);
        let mut page_view = PageView::new(value.children)
            .controller(controller.page_controller())
            .viewport_fraction(value.viewport_fraction);
        if let Some(physics) = value.physics {
            page_view = page_view.physics(physics);
        }
        page_view.into()
    }
}

/// A small theme descriptor for tab bars. The full component theme is kept in
/// the Material foundation and this value is useful for explicit overrides.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TabBarThemeData {
    pub label_color: Option<Color>,
    pub unselected_label_color: Option<Color>,
    pub indicator_color: Option<Color>,
    pub indicator_weight: Option<f32>,
    pub divider_color: Option<Color>,
    pub label_style: Option<TextStyle>,
    pub unselected_label_style: Option<TextStyle>,
    pub overlay_color: Option<StateProperty<Color>>,
    pub tab_alignment: Option<TabAlignment>,
}

/// Renderer-neutral Material icon catalog bridge. Applications may use any
/// `IconData` from `incular-text`; these stable names are convenient for the
/// common desktop glyphs and remain text based until an icon font is selected.
pub struct Icons;

#[allow(non_upper_case_globals)]
impl Icons {
    pub const ADD: &'static str = "+";
    pub const add: &'static str = Self::ADD;
    pub const CLOSE: &'static str = "×";
    pub const close: &'static str = Self::CLOSE;
    pub const MENU: &'static str = "☰";
    pub const menu: &'static str = Self::MENU;
    pub const ARROW_BACK: &'static str = "‹";
    pub const arrow_back: &'static str = Self::ARROW_BACK;
    pub const ARROW_FORWARD: &'static str = "›";
    pub const arrow_forward: &'static str = Self::ARROW_FORWARD;
    pub const CHECK: &'static str = "✓";
    pub const check: &'static str = Self::CHECK;
    pub const SETTINGS: &'static str = "⚙";
    pub const settings: &'static str = Self::SETTINGS;
    pub const SEARCH: &'static str = "⌕";
    pub const search: &'static str = Self::SEARCH;
    pub const MORE_VERT: &'static str = "⋮";
    pub const more_vert: &'static str = Self::MORE_VERT;
    pub const ARROW_DROP_DOWN: &'static str = "⌄";
    pub const arrow_drop_down: &'static str = Self::ARROW_DROP_DOWN;
    pub const ARROW_DROP_UP: &'static str = "⌃";
    pub const arrow_drop_up: &'static str = Self::ARROW_DROP_UP;
    pub const CHEVRON_LEFT: &'static str = "‹";
    pub const chevron_left: &'static str = Self::CHEVRON_LEFT;
    pub const CHEVRON_RIGHT: &'static str = "›";
    pub const chevron_right: &'static str = Self::CHEVRON_RIGHT;
    pub const DELETE: &'static str = "⌫";
    pub const delete: &'static str = Self::DELETE;
    pub const EDIT: &'static str = "✎";
    pub const edit: &'static str = Self::EDIT;
    pub const REFRESH: &'static str = "↻";
    pub const refresh: &'static str = Self::REFRESH;
    pub const PLAY_ARROW: &'static str = "▶";
    pub const play_arrow: &'static str = Self::PLAY_ARROW;
    pub const PAUSE: &'static str = "Ⅱ";
    pub const pause: &'static str = Self::PAUSE;
    pub const INFO_OUTLINE: &'static str = "ⓘ";
    pub const info_outline: &'static str = Self::INFO_OUTLINE;
    pub const HELP_OUTLINE: &'static str = "?";
    pub const help_outline: &'static str = Self::HELP_OUTLINE;
    pub const WARNING: &'static str = "⚠";
    pub const warning: &'static str = Self::WARNING;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controlled_material_controls_lower_to_widgets() {
        let _: Widget = Checkbox::new(false).tristate(true).value(None).into();
        let _: Widget = Radio::new("a").group_value(Some("b")).into();
        let _: Widget = Switch::new(true).enabled(false).into();
        let _: Widget = Slider::new(0.5).range(0.0, 10.0).divisions(Some(10)).into();
        let _: Widget = RangeSlider::new(RangeValues::new(0.2, 0.8)).into();
    }

    #[test]
    fn tabs_share_a_retained_controller_and_page_view() {
        let controller = TabController::new(2);
        controller.animate_to(1);
        assert_eq!(controller.index(), 1);
        let _: Widget = TabBar::new([Tab::text("One"), Tab::text("Two")])
            .controller(controller.clone())
            .into();
        let _: Widget = TabBarView::new([Text::new("One"), Text::new("Two")])
            .controller(controller)
            .into();
    }
}
