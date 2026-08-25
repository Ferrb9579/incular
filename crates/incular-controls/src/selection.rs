use crate::theme::ControlTheme;
use incular_config::Alignment;
use incular_core::{Color, Offset};
use incular_semantics::{Role as SemanticRole, SemanticActionKind, SemanticState};
use incular_widgets::{
    Border, BorderRadius, BoxDecoration, Button as RawButton, Container, ExplicitSemantics, Icon,
    OpacityController, Positioned, Row, ScaleController, SizedBox, Stack, Text,
    TranslationController, Widget,
};
use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

fn indicator_transition(theme: &ControlTheme) -> Duration {
    if theme.motion.reduced_motion {
        Duration::ZERO
    } else {
        Duration::from_millis(u64::from(theme.motion.pressed_duration_ms.max(120)))
    }
}

/// Platform-neutral styled checkbox toggle.
#[derive(Clone)]
pub struct Checkbox {
    value: Rc<Cell<bool>>,
    revision: Rc<Cell<u64>>,
    check_opacity: OpacityController,
    check_scale: ScaleController,
    indeterminate: bool,
    label: Option<String>,
    enabled: bool,
    on_changed: Option<Rc<dyn Fn(bool) + 'static>>,
}

impl Checkbox {
    #[must_use]
    pub fn new(value: bool) -> Self {
        let check_opacity = OpacityController::new();
        let check_scale = ScaleController::new();
        check_opacity.set_opacity(if value { 1. } else { 0. });
        check_scale.set_scale(if value { 1. } else { 0.8 });
        Self {
            value: Rc::new(Cell::new(value)),
            revision: Rc::new(Cell::new(0)),
            check_opacity,
            check_scale,
            indeterminate: false,
            label: None,
            enabled: true,
            on_changed: None,
        }
    }

    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Displays the mixed state while preserving the retained checked value.
    #[must_use]
    pub fn indeterminate(mut self, value: bool) -> Self {
        self.indeterminate = value;
        let visible = value || self.value.get();
        self.check_opacity
            .set_opacity(if visible { 1. } else { 0. });
        self.check_scale.set_scale(if visible { 1. } else { 0.8 });
        self
    }

    #[must_use]
    pub fn checked_state(&self) -> crate::checkbox::CheckedState {
        if self.indeterminate {
            crate::checkbox::CheckedState::Indeterminate
        } else if self.value.get() {
            crate::checkbox::CheckedState::Checked
        } else {
            crate::checkbox::CheckedState::Unchecked
        }
    }

    #[must_use]
    pub fn on_changed(mut self, on_changed: impl Fn(bool) + 'static) -> Self {
        self.on_changed = Some(Rc::new(on_changed));
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let is_checked = self.value.get();
        let indicator_visible = self.indeterminate || is_checked;
        let target_opacity = if indicator_visible { 1. } else { 0. };
        let target_scale = if indicator_visible { 1. } else { 0.8 };
        if (self.check_opacity.opacity() - target_opacity).abs() > f32::EPSILON {
            let duration = indicator_transition(theme);
            if duration.is_zero() {
                self.check_opacity.set_opacity(target_opacity);
                self.check_scale.set_scale(target_scale);
            } else {
                self.check_opacity
                    .animate_to(target_opacity, duration, Instant::now());
                self.check_scale
                    .animate_to(target_scale, duration, Instant::now());
            }
        }
        let box_color = if is_checked {
            theme.colors.accent
        } else {
            theme.colors.surface
        };
        let border = Border::new(
            1.5,
            if is_checked {
                theme.colors.accent
            } else {
                theme.colors.border_strong
            },
        );

        // The default indicator is a retained vector path, not a font glyph.
        // This keeps baselines and fallback-font behavior stable.
        let check_icon: Widget = if self.indeterminate {
            Icon::new(incular_widgets::icons::minus())
                .size(12.0)
                .brush(theme.colors.accent_foreground)
                .into()
        } else {
            Icon::new(incular_widgets::icons::check())
                .size(12.0)
                .brush(theme.colors.accent_foreground)
                .into()
        };
        let check_icon = Widget::controlled_scale(
            self.check_scale.clone(),
            Widget::controlled_opacity(self.check_opacity.clone(), check_icon),
        );
        let box_widget = Container::new()
            .width(theme.checkbox.indicator_size)
            .height(theme.checkbox.indicator_size)
            .alignment(Alignment::CENTER)
            .decoration(
                BoxDecoration::new()
                    .color(box_color)
                    .border(border)
                    .border_radius(BorderRadius::circular(theme.checkbox.radius)),
            )
            .child(check_icon);

        let value = self.value.clone();
        let revision = self.revision.clone();
        let check_opacity = self.check_opacity.clone();
        let check_scale = self.check_scale.clone();
        let on_changed = self.on_changed.clone();
        let transition = indicator_transition(theme);
        let content: Widget = if let Some(lbl) = self.label.as_ref() {
            Row::new([
                Widget::from(box_widget),
                Widget::from(SizedBox::new().width(8.0)),
                Widget::from(Text::new(lbl.clone()).style(theme.typography.body.clone())),
            ])
            .alignment(incular_config::CrossAxisAlignment::Center)
            .into()
        } else {
            box_widget.into()
        };
        let mut button = RawButton::with_child(content)
            .color(Color::TRANSPARENT)
            .disabled_color(theme.colors.disabled_surface)
            .enabled(self.enabled);

        if self.enabled {
            button = button.on_click(move || {
                let next = !value.get();
                value.set(next);
                revision.set(revision.get().wrapping_add(1));
                let target_opacity = if next { 1. } else { 0. };
                let target_scale = if next { 1. } else { 0.8 };
                if transition.is_zero() {
                    check_opacity.set_opacity(target_opacity);
                    check_scale.set_scale(target_scale);
                } else {
                    let now = Instant::now();
                    check_opacity.animate_to(target_opacity, transition, now);
                    check_scale.animate_to(target_scale, transition, now);
                }
                if let Some(cb) = &on_changed {
                    cb(next);
                }
            });
        }

        let raw: Widget = button.into();
        raw.semantics(
            ExplicitSemantics::new(SemanticRole::Checkbox)
                .label(self.label.clone().unwrap_or_default())
                .state(SemanticState {
                    enabled: self.enabled,
                    focusable: self.enabled,
                    checked: (!self.indeterminate).then_some(is_checked),
                    ..SemanticState::default()
                })
                .actions(if self.enabled {
                    vec![SemanticActionKind::Focus, SemanticActionKind::Activate]
                } else {
                    Vec::new()
                }),
        )
    }
}

impl From<Checkbox> for Widget {
    fn from(value: Checkbox) -> Self {
        let value = Rc::new(value);
        let revision = value.revision.clone();
        Widget::stateful_layout_builder(revision, move |_| {
            let theme =
                incular_widgets::current_build_environment::<ControlTheme>().unwrap_or_default();
            value.build(&theme)
        })
    }
}

/// Platform-neutral styled radio option button.
#[derive(Clone)]
pub struct Radio<T: PartialEq + Clone + 'static> {
    value: T,
    group_value: Option<T>,
    label: Option<String>,
    on_changed: Option<Rc<dyn Fn(T) + 'static>>,
}

impl<T: PartialEq + Clone + 'static> Radio<T> {
    #[must_use]
    pub fn new(value: T, group_value: Option<T>) -> Self {
        Self {
            value,
            group_value,
            label: None,
            on_changed: None,
        }
    }

    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[must_use]
    pub fn on_changed(mut self, on_changed: impl Fn(T) + 'static) -> Self {
        self.on_changed = Some(Rc::new(on_changed));
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let is_selected = self.group_value.as_ref() == Some(&self.value);
        let dot: Widget = if is_selected {
            Container::new()
                .width(theme.radio.indicator_size * 0.44)
                .height(theme.radio.indicator_size * 0.44)
                .decoration(
                    BoxDecoration::new()
                        .color(theme.colors.accent)
                        .border_radius(BorderRadius::circular(4.0)),
                )
                .into()
        } else {
            SizedBox::shrink().into()
        };

        let radio_circle = Container::new()
            .width(theme.radio.indicator_size)
            .height(theme.radio.indicator_size)
            .alignment(Alignment::CENTER)
            .decoration(
                BoxDecoration::new()
                    .color(theme.colors.surface)
                    .border(Border::new(
                        1.5,
                        if is_selected {
                            theme.colors.accent
                        } else {
                            theme.colors.border_strong
                        },
                    ))
                    .border_radius(BorderRadius::circular(theme.radio.indicator_size * 0.5)),
            )
            .child(dot);

        let target_val = self.value.clone();
        let content: Widget = if let Some(lbl) = self.label.as_ref() {
            Row::new([
                Widget::from(radio_circle),
                Widget::from(SizedBox::new().width(8.0)),
                Widget::from(Text::new(lbl.clone()).style(theme.typography.body.clone())),
            ])
            .alignment(incular_config::CrossAxisAlignment::Center)
            .into()
        } else {
            radio_circle.into()
        };
        let mut button = RawButton::with_child(content)
            .color(Color::TRANSPARENT)
            .enabled(true);

        if let Some(cb) = self.on_changed.clone() {
            button = button.on_click(move || cb(target_val.clone()));
        }

        let raw: Widget = button.into();
        raw.semantics(
            ExplicitSemantics::new(SemanticRole::Radio)
                .label(self.label.clone().unwrap_or_default())
                .state(SemanticState {
                    enabled: true,
                    focusable: true,
                    checked: Some(is_selected),
                    ..SemanticState::default()
                })
                .actions(vec![
                    SemanticActionKind::Focus,
                    SemanticActionKind::Activate,
                ]),
        )
    }
}

impl<T: PartialEq + Clone + 'static> From<Radio<T>> for Widget {
    fn from(value: Radio<T>) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| {
            let theme =
                incular_widgets::current_build_environment::<ControlTheme>().unwrap_or_default();
            value.build(&theme)
        })
    }
}

/// Platform-neutral styled toggle switch.
#[derive(Clone)]
pub struct Switch {
    value: Rc<Cell<bool>>,
    revision: Rc<Cell<u64>>,
    thumb_translation: TranslationController,
    thumb_initialized: Rc<Cell<bool>>,
    enabled: bool,
    on_changed: Option<Rc<dyn Fn(bool) + 'static>>,
}

impl Switch {
    #[must_use]
    pub fn new(value: bool) -> Self {
        Self {
            value: Rc::new(Cell::new(value)),
            revision: Rc::new(Cell::new(0)),
            thumb_translation: TranslationController::new(),
            thumb_initialized: Rc::new(Cell::new(false)),
            enabled: true,
            on_changed: None,
        }
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn on_changed(mut self, on_changed: impl Fn(bool) + 'static) -> Self {
        self.on_changed = Some(Rc::new(on_changed));
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let is_on = self.value.get();
        let thumb_travel = (theme.switch.width - 4. - theme.switch.thumb_size).max(0.);
        let target_offset = Offset::new(if is_on { thumb_travel } else { 0. }, 0.);
        let transition = if theme.motion.reduced_motion {
            Duration::ZERO
        } else {
            Duration::from_millis(u64::from(theme.motion.pressed_duration_ms.max(140)))
        };
        if !self.thumb_initialized.replace(true) {
            self.thumb_translation.set_offset(target_offset);
        } else if (self.thumb_translation.offset().x - target_offset.x).abs() > f32::EPSILON {
            if transition.is_zero() {
                self.thumb_translation.set_offset(target_offset);
            } else {
                self.thumb_translation
                    .animate_to(target_offset, transition, Instant::now());
            }
        }
        let track_color = if is_on {
            theme.colors.accent
        } else {
            theme.colors.surface_variant
        };
        let thumb_color = if is_on {
            theme.colors.accent_foreground
        } else {
            theme.colors.foreground_muted
        };

        let thumb = Container::new()
            .width(theme.switch.thumb_size)
            .height(theme.switch.thumb_size)
            .decoration(
                BoxDecoration::new()
                    .color(thumb_color)
                    .border_radius(BorderRadius::circular(theme.switch.thumb_size * 0.5)),
            );

        let thumb = Positioned::new(Widget::translate(
            self.thumb_translation.clone(),
            thumb.into(),
        ))
        .left(2.0)
        .top(2.0)
        .width(theme.switch.thumb_size)
        .height(theme.switch.thumb_size);
        let track = Container::new()
            .width(theme.switch.width)
            .height(theme.switch.height)
            .decoration(
                BoxDecoration::new()
                    .color(track_color)
                    .border(Border::new(1.0, theme.colors.border))
                    .border_radius(BorderRadius::circular(theme.switch.height * 0.5)),
            )
            .child(Stack::new([Widget::from(thumb)]));

        let value = self.value.clone();
        let revision = self.revision.clone();
        let thumb_translation = self.thumb_translation.clone();
        let on_changed = self.on_changed.clone();
        let mut button = RawButton::with_child(track)
            .color(Color::TRANSPARENT)
            .enabled(self.enabled);
        if self.enabled {
            button = button.on_click(move || {
                let next = !value.get();
                value.set(next);
                revision.set(revision.get().wrapping_add(1));
                let target = Offset::new(if next { thumb_travel } else { 0. }, 0.);
                if transition.is_zero() {
                    thumb_translation.set_offset(target);
                } else {
                    thumb_translation.animate_to(target, transition, Instant::now());
                }
                if let Some(cb) = &on_changed {
                    cb(next);
                }
            });
        }

        let raw: Widget = button.into();
        raw.semantics(
            ExplicitSemantics::new(SemanticRole::Button)
                .state(SemanticState {
                    enabled: self.enabled,
                    focusable: self.enabled,
                    checked: Some(is_on),
                    ..SemanticState::default()
                })
                .actions(if self.enabled {
                    vec![SemanticActionKind::Focus, SemanticActionKind::Activate]
                } else {
                    Vec::new()
                }),
        )
    }
}

impl From<Switch> for Widget {
    fn from(value: Switch) -> Self {
        let value = Rc::new(value);
        let revision = value.revision.clone();
        Widget::stateful_layout_builder(revision, move |_| {
            let theme =
                incular_widgets::current_build_environment::<ControlTheme>().unwrap_or_default();
            value.build(&theme)
        })
    }
}
