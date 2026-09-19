//! Slider anatomy and value constraints.

use crate::theme::ControlTheme;
use incular_config::Axis;
use incular_core::{Color, KeyboardKey, NamedKey};
use incular_semantics::{Role as SemanticRole, SemanticAction, SemanticState};
use incular_widgets::{
    Container, FocusNode, GestureDetector, HitTestBehavior, KeyboardListener, Positioned,
    Semantics, Stack, TapUpDetails, Widget,
};
use std::cell::Cell;
use std::rc::Rc;
use typed_builder::TypedBuilder;

/// Neutral two-thumb range values shared by styled range-slider adapters.
#[doc(hidden)]
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

/// Identifies one logical thumb of a neutral range slider.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeThumb {
    Start,
    End,
}

/// Retained value/constraint owner for two-thumb sliders.
///
/// Material and other design systems provide visuals and vocabulary while this
/// model owns normalization, quantization, separation, no-op filtering and the
/// revision that invalidates presentation.
#[doc(hidden)]
#[derive(Clone)]
pub struct RangeSliderModel {
    values: Rc<Cell<RangeValues>>,
    revision: Rc<Cell<u64>>,
    min: f32,
    max: f32,
    step: f32,
    minimum_separation: f32,
}

impl RangeSliderModel {
    #[must_use]
    pub fn new(
        values: RangeValues,
        min: f32,
        max: f32,
        step: f32,
        minimum_separation: f32,
    ) -> Self {
        let min = min.min(max);
        let max = max.max(min);
        let span = (max - min).max(f32::EPSILON);
        let step = if step.is_finite() {
            step.abs().max(f32::EPSILON)
        } else {
            0.01_f32.min(span)
        };
        let minimum_separation = if minimum_separation.is_finite() {
            minimum_separation.max(0.0).min(span)
        } else {
            0.0
        };
        let mut values = values.normalized();
        values.start = values.start.clamp(min, max);
        values.end = values
            .end
            .clamp((values.start + minimum_separation).min(max), max);
        let model = Self {
            values: Rc::new(Cell::new(values)),
            revision: Rc::new(Cell::new(0)),
            min,
            max,
            step,
            minimum_separation,
        };
        let normalized = model.normalize(values);
        model.values.set(normalized);
        model
    }

    #[must_use]
    pub fn values(&self) -> RangeValues {
        self.values.get()
    }

    #[must_use]
    pub fn revision(&self) -> Rc<Cell<u64>> {
        self.revision.clone()
    }

    #[must_use]
    pub const fn min(&self) -> f32 {
        self.min
    }

    #[must_use]
    pub const fn max(&self) -> f32 {
        self.max
    }

    #[must_use]
    pub const fn step(&self) -> f32 {
        self.step
    }

    #[must_use]
    pub const fn minimum_separation(&self) -> f32 {
        self.minimum_separation
    }

    #[must_use]
    pub fn quantize(&self, raw: f32) -> f32 {
        ((raw.clamp(self.min, self.max) - self.min) / self.step)
            .round()
            .mul_add(self.step, self.min)
            .clamp(self.min, self.max)
    }

    #[must_use]
    pub fn adjust(&self, thumb: RangeThumb, delta: f32) -> Option<RangeValues> {
        let current = self.values();
        let raw = match thumb {
            RangeThumb::Start => current.start + delta,
            RangeThumb::End => current.end + delta,
        };
        self.set_thumb(thumb, raw)
    }

    #[must_use]
    pub fn set_thumb(&self, thumb: RangeThumb, raw: f32) -> Option<RangeValues> {
        let mut next = self.values();
        match thumb {
            RangeThumb::Start => {
                next.start = self
                    .quantize(raw)
                    .min((next.end - self.minimum_separation).max(self.min));
            }
            RangeThumb::End => {
                next.end = self
                    .quantize(raw)
                    .max((next.start + self.minimum_separation).min(self.max));
            }
        }
        self.commit(next)
    }

    #[must_use]
    pub fn set_from_origin(
        &self,
        thumb: RangeThumb,
        origin: RangeValues,
        total_delta_value: f32,
    ) -> Option<RangeValues> {
        let raw = match thumb {
            RangeThumb::Start => origin.start + total_delta_value,
            RangeThumb::End => origin.end + total_delta_value,
        };
        self.set_thumb(thumb, raw)
    }

    #[must_use]
    pub fn set_edge(&self, thumb: RangeThumb, maximum: bool) -> Option<RangeValues> {
        self.set_thumb(thumb, if maximum { self.max } else { self.min })
    }

    fn normalize(&self, mut values: RangeValues) -> RangeValues {
        values = values.normalized();
        values.start = self.quantize(values.start);
        values.end = self.quantize(values.end);
        if values.end - values.start < self.minimum_separation {
            values.end = (values.start + self.minimum_separation).min(self.max);
            if values.end - values.start < self.minimum_separation {
                values.start = (values.end - self.minimum_separation).max(self.min);
            }
        }
        values
    }

    fn commit(&self, values: RangeValues) -> Option<RangeValues> {
        let next = self.normalize(values);
        if next == self.values.get() {
            return None;
        }
        self.values.set(next);
        self.revision.set(
            self.revision
                .get()
                .checked_add(1)
                .expect("revision exhausted"),
        );
        Some(next)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, TypedBuilder)]
pub struct Range {
    #[builder(default = 0.0)]
    pub min: f32,
    #[builder(default = 1.0)]
    pub max: f32,
    #[builder(default = 0.01)]
    pub step: f32,
}
impl Default for Range {
    fn default() -> Self {
        Self {
            min: 0.,
            max: 1.,
            step: 0.01,
        }
    }
}
impl Range {
    #[must_use]
    pub fn clamp(self, value: f32) -> f32 {
        let step = self.step.max(f32::EPSILON);
        ((value.clamp(self.min, self.max) - self.min) / step)
            .round()
            .mul_add(step, self.min)
            .clamp(self.min, self.max)
    }
}

#[derive(Clone, TypedBuilder)]
pub struct Root {
    #[builder(
        default = Rc::new(Cell::new(0.0)),
        setter(
            fn transform(value: f32) -> Rc<Cell<f32>> {
                Rc::new(Cell::new(value))
            }
        )
    )]
    value: Rc<Cell<f32>>,
    #[builder(default = Rc::new(Cell::new(0)), setter(skip))]
    revision: Rc<Cell<u64>>,
    #[builder(default)]
    range: Range,
    #[builder(default = Axis::Horizontal)]
    orientation: Axis,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default, setter(strip_option))]
    secondary_value: Option<f32>,
    #[builder(default, setter(strip_option))]
    active_track_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    inactive_track_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    secondary_track_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    thumb_color: Option<Color>,
    #[builder(default = true)]
    tap_enabled: bool,
    #[builder(default = true)]
    drag_enabled: bool,
    #[builder(default, setter(strip_option, into))]
    semantic_value: Option<String>,
    #[builder(default, setter(strip_option))]
    track_extent: Option<f32>,
    #[builder(default)]
    rtl: bool,
    #[builder(default = FocusNode::new(), setter(skip))]
    focus_node: FocusNode,
    #[builder(default)]
    autofocus: bool,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
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
    on_change: Option<Rc<dyn Fn(f32) + 'static>>,
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
impl Default for Root {
    fn default() -> Self {
        Self::builder().build()
    }
}
impl Root {
    #[must_use]
    pub fn new() -> Self {
        Self::builder().build()
    }
    #[must_use]
    pub fn value(self, value: f32) -> Self {
        self.value.set(self.range.clamp(value));
        self
    }
    #[must_use]
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.range.min = min.min(max);
        self.range.max = max.max(min);
        self.value.set(self.range.clamp(self.value.get()));
        self
    }
    #[must_use]
    pub fn step(mut self, value: f32) -> Self {
        self.range.step = value.abs().max(f32::EPSILON);
        self
    }
    #[must_use]
    pub fn vertical(mut self) -> Self {
        self.orientation = Axis::Vertical;
        self
    }
    #[must_use]
    pub fn disabled(mut self, value: bool) -> Self {
        self.enabled = !value;
        self
    }

    /// Sets the optional secondary progress segment used by Material sliders.
    #[must_use]
    pub fn secondary_value(mut self, value: Option<f32>) -> Self {
        self.secondary_value = value.map(|value| self.range.clamp(value));
        self
    }

    #[must_use]
    pub fn active_track_color(mut self, value: Color) -> Self {
        self.active_track_color = Some(value);
        self
    }

    #[must_use]
    pub fn inactive_track_color(mut self, value: Color) -> Self {
        self.inactive_track_color = Some(value);
        self
    }

    #[must_use]
    pub fn secondary_track_color(mut self, value: Color) -> Self {
        self.secondary_track_color = Some(value);
        self
    }

    #[must_use]
    pub fn thumb_color(mut self, value: Color) -> Self {
        self.thumb_color = Some(value);
        self
    }

    /// Controls whether tapping the track activates the slider.  Material's
    /// `SliderInteraction` maps its tap policies to this headless flag.
    #[must_use]
    pub fn tap_enabled(mut self, value: bool) -> Self {
        self.tap_enabled = value;
        self
    }

    /// Controls whether dragging the thumb/track changes the value.
    #[must_use]
    pub fn drag_enabled(mut self, value: bool) -> Self {
        self.drag_enabled = value;
        self
    }

    /// Overrides the semantic value string while retaining the numeric value
    /// for the control's range mechanics.
    #[must_use]
    pub fn semantic_value(mut self, value: impl Into<String>) -> Self {
        self.semantic_value = Some(value.into());
        self
    }

    /// Sets a preferred logical track extent.  An omitted extent keeps the
    /// controls default, while Material can provide a theme/layout-specific
    /// value without forking slider interaction.
    #[must_use]
    pub fn track_extent(mut self, value: f32) -> Self {
        self.track_extent = Some(value.max(1.0));
        self
    }

    /// Mirrors Flutter's direction-aware slider behavior. In right-to-left
    /// horizontal layouts the logical left/right keys are reversed while the
    /// visual track remains unchanged.
    #[must_use]
    pub fn rtl(mut self, value: bool) -> Self {
        self.rtl = value;
        self
    }

    /// Associates a retained focus node with the slider's keyboard surface.
    #[must_use]
    pub fn focus_node(mut self, value: FocusNode) -> Self {
        self.focus_node = value;
        self
    }

    /// Requests the slider focus when its retained element is mounted.
    #[must_use]
    pub fn autofocus(mut self, value: bool) -> Self {
        self.autofocus = value;
        self
    }
    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }
    #[must_use]
    pub fn on_value_change(mut self, cb: impl Fn(f32) + 'static) -> Self {
        self.on_change = Some(Rc::new(cb));
        self
    }

    /// Registers a callback at the beginning of a retained drag.
    #[must_use]
    pub fn on_change_start(mut self, cb: impl Fn(f32) + 'static) -> Self {
        self.on_change_start = Some(Rc::new(cb));
        self
    }

    /// Registers a callback when a retained drag ends.
    #[must_use]
    pub fn on_change_end(mut self, cb: impl Fn(f32) + 'static) -> Self {
        self.on_change_end = Some(Rc::new(cb));
        self
    }

    /// Resolves the default slider anatomy from the active control theme.
    /// Applications can still replace the complete visual with [`Root::child`]
    /// while retaining the descriptor's value and callback policy.
    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let current_value = self.range.clamp(self.value.get());
        self.value.set(current_value);
        let ratio = ((current_value - self.range.min)
            / (self.range.max - self.range.min).max(f32::EPSILON))
        .clamp(0., 1.);
        let visual_ratio = if self.rtl && self.orientation == Axis::Horizontal {
            1.0 - ratio
        } else {
            ratio
        };
        let track_extent = self.track_extent.unwrap_or(180.0);
        let track_thickness = theme.slider.track_height.max(1.);
        let thumb_size = theme.slider.thumb_size.max(track_thickness);
        let inactive_color = self
            .inactive_track_color
            .unwrap_or(theme.colors.border_strong);
        let active_color = self.active_track_color.unwrap_or(if self.enabled {
            theme.colors.accent
        } else {
            theme.colors.disabled_foreground
        });
        let secondary_color = self
            .secondary_track_color
            .unwrap_or(theme.colors.accent_active);
        let thumb_color = self.thumb_color.unwrap_or(theme.colors.surface);
        let visual: Widget = if let Some(child) = &self.child {
            child.clone()
        } else {
            let track = if self.orientation == Axis::Horizontal {
                Container::new()
                    .width(track_extent)
                    .height(track_thickness)
                    .color(if self.enabled {
                        inactive_color
                    } else {
                        theme.colors.disabled_surface
                    })
            } else {
                Container::new()
                    .width(track_thickness)
                    .height(track_extent)
                    .color(if self.enabled {
                        inactive_color
                    } else {
                        theme.colors.disabled_surface
                    })
            };
            let indicator_extent = (track_extent * ratio).max(track_thickness);
            let indicator = if self.orientation == Axis::Horizontal {
                Container::new()
                    .width(indicator_extent)
                    .height(track_thickness)
                    .color(active_color)
            } else {
                Container::new()
                    .width(track_thickness)
                    .height(indicator_extent)
                    .color(active_color)
            };
            let secondary_indicator: Option<Widget> = self.secondary_value.map(|secondary| {
                let secondary_ratio = ((secondary - self.range.min)
                    / (self.range.max - self.range.min).max(f32::EPSILON))
                .clamp(0.0, 1.0);
                if self.orientation == Axis::Horizontal {
                    Container::new()
                        .width((track_extent * secondary_ratio).max(track_thickness))
                        .height(track_thickness)
                        .color(secondary_color)
                        .into()
                } else {
                    Container::new()
                        .width(track_thickness)
                        .height((track_extent * secondary_ratio).max(track_thickness))
                        .color(secondary_color)
                        .into()
                }
            });
            let thumb = Container::new()
                .width(thumb_size)
                .height(thumb_size)
                .color(thumb_color)
                .border(incular_widgets::Border::new(
                    theme.button.border_width,
                    if self.enabled {
                        inactive_color
                    } else {
                        theme.colors.disabled_foreground
                    },
                ))
                .radius(theme.slider.radius);
            if self.orientation == Axis::Horizontal {
                let mut children = vec![Widget::from(track)];
                if let Some(indicator) = secondary_indicator {
                    children.push(Widget::from(if self.rtl {
                        Positioned::new(indicator).right(0.).top(0.)
                    } else {
                        Positioned::new(indicator).left(0.).top(0.)
                    }));
                }
                children.push(Widget::from(if self.rtl {
                    Positioned::new(indicator).right(0.).top(0.)
                } else {
                    Positioned::new(indicator).left(0.).top(0.)
                }));
                children.push(Widget::from(
                    Positioned::new(thumb)
                        .left((track_extent * visual_ratio - thumb_size * 0.5).max(0.))
                        .top((track_thickness - thumb_size) * 0.5),
                ));
                Stack::new(children).into()
            } else {
                let mut children = vec![Widget::from(track)];
                if let Some(indicator) = secondary_indicator {
                    children.push(Widget::from(Positioned::new(indicator).left(0.).bottom(0.)));
                }
                children.push(Widget::from(Positioned::new(indicator).left(0.).bottom(0.)));
                children.push(Widget::from(
                    Positioned::new(thumb)
                        .left((track_thickness - thumb_size) * 0.5)
                        .bottom((track_extent * ratio - thumb_size * 0.5).max(0.)),
                ));
                Stack::new(children).into()
            }
        };
        let value = self.value.clone();
        let revision = self.revision.clone();
        let range = self.range;
        let orientation = self.orientation;
        let callback = self.on_change.clone();
        let tap = {
            let value = value.clone();
            let revision = revision.clone();
            let callback = callback.clone();
            let rtl = self.rtl;
            move |details: TapUpDetails| {
                let axis_position = if orientation == Axis::Horizontal {
                    details.local_position.x
                } else {
                    track_extent - details.local_position.y
                };
                let mut tapped_ratio = (axis_position / track_extent).clamp(0.0, 1.0);
                if rtl && orientation == Axis::Horizontal {
                    tapped_ratio = 1.0 - tapped_ratio;
                }
                let next = range.clamp(range.min + tapped_ratio * (range.max - range.min));
                update_value(&value, &revision, &callback, next);
            }
        };
        let drag_value = value.clone();
        let drag_revision = revision.clone();
        let drag_callback = callback.clone();
        let drag_start_callback = self.on_change_start.clone();
        let drag_end_callback = self.on_change_end.clone();
        let drag_rtl = self.rtl;
        let drag_started = Rc::new(Cell::new(false));
        let drag_started_for_update = drag_started.clone();
        let drag = move |delta: incular_core::Offset| {
            let travel = if orientation == Axis::Horizontal {
                let direction = if drag_rtl { -1.0 } else { 1.0 };
                direction * delta.x / track_extent
            } else {
                -delta.y / track_extent
            };
            let next = range.clamp(current_value + travel * (range.max - range.min));
            if (next - drag_value.get()).abs() <= f32::EPSILON {
                return;
            }
            if !drag_started_for_update.replace(true)
                && let Some(callback) = &drag_start_callback
            {
                callback(next);
            }
            drag_value.set(next);
            drag_revision.set(
                drag_revision
                    .get()
                    .checked_add(1)
                    .expect("revision exhausted"),
            );
            if let Some(callback) = &drag_callback {
                callback(next);
            }
        };
        let mut interactive: Widget = if self.enabled && (self.tap_enabled || self.drag_enabled) {
            let detector = GestureDetector::new(visual).behavior(HitTestBehavior::Opaque);
            let detector = if self.tap_enabled {
                detector.on_tap_up(tap)
            } else {
                detector
            };
            if self.drag_enabled && orientation == Axis::Horizontal {
                let detector = detector.on_horizontal_drag_update(drag);
                if let Some(callback) = drag_end_callback {
                    let drag_value = value.clone();
                    let drag_started = drag_started.clone();
                    detector
                        .on_horizontal_drag_end(move |_| {
                            if drag_started.replace(false) {
                                callback(drag_value.get());
                            }
                        })
                        .into()
                } else {
                    detector.into()
                }
            } else if self.drag_enabled {
                let detector = detector.on_vertical_drag_update(drag);
                if let Some(callback) = drag_end_callback {
                    let drag_value = value.clone();
                    let drag_started = drag_started.clone();
                    detector
                        .on_vertical_drag_end(move |_| {
                            if drag_started.replace(false) {
                                callback(drag_value.get());
                            }
                        })
                        .into()
                } else {
                    detector.into()
                }
            } else {
                detector.into()
            }
        } else {
            visual
        };

        // Sliders are keyboard controls as well as pointer controls. Keep the
        // value and callback in the same retained cells used by pointer drag,
        // so arrow/Home/End/repeat events cannot diverge from mouse behavior.
        if self.enabled {
            let keyboard_value = value.clone();
            let keyboard_revision = revision.clone();
            let keyboard_callback = callback.clone();
            let keyboard_range = range;
            let keyboard_orientation = orientation;
            let keyboard_rtl = self.rtl;
            let focus_node = self.focus_node.clone();
            let keyboard = KeyboardListener::new(interactive)
                .focus_node(focus_node)
                .autofocus(self.autofocus)
                .on_key(move |event| {
                    if !event.state.is_down() {
                        return false;
                    }
                    let action = match &event.key {
                        KeyboardKey::Named(NamedKey::ArrowLeft)
                            if keyboard_orientation == Axis::Horizontal =>
                        {
                            Some(if keyboard_rtl { 1.0 } else { -1.0 })
                        }
                        KeyboardKey::Named(NamedKey::ArrowRight)
                            if keyboard_orientation == Axis::Horizontal =>
                        {
                            Some(if keyboard_rtl { -1.0 } else { 1.0 })
                        }
                        KeyboardKey::Named(NamedKey::ArrowUp)
                            if keyboard_orientation == Axis::Vertical =>
                        {
                            Some(1.0)
                        }
                        KeyboardKey::Named(NamedKey::ArrowDown)
                            if keyboard_orientation == Axis::Vertical =>
                        {
                            Some(-1.0)
                        }
                        // Semantic Increment/Decrement synthesizes
                        // ArrowRight/ArrowLeft; honor them as value steps
                        // even for vertical sliders so the shared fallback
                        // reaches value logic in both orientations. Visual
                        // RTL mapping stays on the horizontal arms above.
                        KeyboardKey::Named(NamedKey::ArrowRight) => Some(1.0),
                        KeyboardKey::Named(NamedKey::ArrowLeft) => Some(-1.0),
                        KeyboardKey::Named(NamedKey::Home) => None,
                        KeyboardKey::Named(NamedKey::End) => None,
                        KeyboardKey::Character(text) if text == " " => Some(1.0),
                        _ => return false,
                    };
                    let current = keyboard_value.get();
                    let next = match &event.key {
                        KeyboardKey::Named(NamedKey::Home) => keyboard_range.min,
                        KeyboardKey::Named(NamedKey::End) => keyboard_range.max,
                        _ => keyboard_range
                            .clamp(current + action.unwrap_or(1.0) * keyboard_range.step),
                    };
                    if (next - current).abs() > f32::EPSILON {
                        keyboard_value.set(next);
                        keyboard_revision.set(
                            keyboard_revision
                                .get()
                                .checked_add(1)
                                .expect("revision exhausted"),
                        );
                        if let Some(callback) = &keyboard_callback {
                            callback(next);
                        }
                    }
                    true
                });
            interactive = keyboard.into();
        }
        let value_text = self
            .semantic_value
            .clone()
            .unwrap_or_else(|| format!("{:.3}", range.clamp(current_value)));
        let mut semantics = Semantics::new(interactive)
            .role(SemanticRole::Slider)
            .value(value_text)
            .state(SemanticState {
                enabled: self.enabled,
                focusable: self.enabled,
                numeric_value: Some(f64::from(range.clamp(current_value))),
                numeric_min: Some(f64::from(range.min)),
                numeric_max: Some(f64::from(range.max)),
                numeric_step: Some(f64::from(range.step)),
                ..SemanticState::default()
            });
        if self.enabled {
            semantics = semantics.action(SemanticAction::Focus);
            let increment_value = value.clone();
            let increment_revision = revision.clone();
            let increment_callback = callback.clone();
            semantics = semantics.on_increase(move || {
                let next = range.clamp(increment_value.get() + range.step);
                update_value(
                    &increment_value,
                    &increment_revision,
                    &increment_callback,
                    next,
                );
            });
            let decrement_value = value;
            let decrement_revision = revision;
            let decrement_callback = callback;
            semantics = semantics.on_decrease(move || {
                let next = range.clamp(decrement_value.get() - range.step);
                update_value(
                    &decrement_value,
                    &decrement_revision,
                    &decrement_callback,
                    next,
                );
            });
        }
        semantics.into()
    }
}

fn update_value(
    value: &Rc<Cell<f32>>,
    revision: &Rc<Cell<u64>>,
    callback: &Option<Rc<dyn Fn(f32) + 'static>>,
    next: f32,
) {
    if (next - value.get()).abs() <= f32::EPSILON {
        return;
    }
    value.set(next);
    revision.set(revision.get().checked_add(1).expect("revision exhausted"));
    if let Some(callback) = callback {
        callback(next);
    }
}
impl From<Root> for Widget {
    fn from(value: Root) -> Self {
        let value = Rc::new(value);
        let revision = value.revision.clone();
        Widget::stateful_layout_builder(revision, move |context, _| {
            let theme = crate::theme::current_control_theme(context);
            value.build(&theme)
        })
    }
}
#[derive(Clone, TypedBuilder)]
pub struct Control {
    #[builder(setter(into))]
    child: Widget,
}
impl Control {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Control> for Widget {
    fn from(value: Control) -> Self {
        value.child
    }
}
#[derive(Clone, TypedBuilder)]
pub struct Track {
    #[builder(setter(into))]
    child: Widget,
}
impl Track {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Track> for Widget {
    fn from(value: Track) -> Self {
        value.child
    }
}
#[derive(Clone, TypedBuilder)]
pub struct Indicator {
    #[builder(setter(into))]
    child: Widget,
}
impl Indicator {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Indicator> for Widget {
    fn from(value: Indicator) -> Self {
        value.child
    }
}
#[derive(Clone, TypedBuilder)]
pub struct Thumb {
    #[builder(setter(into))]
    child: Widget,
}
impl Thumb {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Thumb> for Widget {
    fn from(value: Thumb) -> Self {
        value.child
    }
}
