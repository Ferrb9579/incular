use crate::foundation::{StateProperty, WidgetState, WidgetStates};
use incular_controls::CheckedState;
use incular_core::Color;
use incular_widgets::{Border, BorderRadius, Widget};
use std::{cell::Cell, rc::Rc};
use typed_builder::TypedBuilder;

/// Material checkbox with a controlled nullable value.
#[derive(Clone, TypedBuilder)]
pub struct Checkbox {
    #[builder(default, setter(strip_option))]
    pub(super) value: Option<bool>,
    #[builder(default)]
    tristate: bool,
    #[builder(default = true)]
    pub(super) enabled: bool,
    #[builder(default)]
    autofocus: bool,
    #[builder(default)]
    is_error: bool,
    #[builder(default, setter(strip_option, into))]
    fill_color: Option<StateProperty<Color>>,
    #[builder(default, setter(strip_option, into))]
    check_color: Option<StateProperty<Color>>,
    #[builder(default, setter(strip_option))]
    side: Option<Border>,
    #[builder(default, setter(strip_option))]
    shape: Option<BorderRadius>,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn(Option<bool>) + 'static>>
            where
                F: Fn(Option<bool>) + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_changed: Option<Rc<dyn Fn(Option<bool>) + 'static>>,
}

impl Default for Checkbox {
    fn default() -> Self {
        Self::builder().build()
    }
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

    /// Uses the scoped theme's error color for the enabled indicator's outline
    /// and checked or mixed fill. Explicit fill and side values take precedence.
    #[must_use]
    pub fn is_error(mut self, value: bool) -> Self {
        self.is_error = value;
        self
    }

    /// Resolves the Material fill against the configured checked/error/disabled state and
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

    fn build(self, theme: &incular_controls::ControlTheme) -> Widget {
        let state = match self.value {
            Some(true) => CheckedState::Checked,
            Some(false) => CheckedState::Unchecked,
            None if self.tristate => CheckedState::Indeterminate,
            None => CheckedState::Unchecked,
        };
        let mut root = incular_controls::checkbox::Root::new()
            .state(state)
            .enabled(self.enabled);
        if self.is_error && self.enabled {
            root = root
                .active_color(theme.colors.error)
                .border_color(theme.colors.error);
            if state == CheckedState::Indeterminate {
                root = root.inactive_color(theme.colors.error);
            }
        }
        let mut states = WidgetStates::default();
        if self.value == Some(true) {
            states = states.with(WidgetState::Selected);
        }
        if self.is_error {
            states = states.with(WidgetState::Error);
        }
        if !self.enabled {
            states = states.with(WidgetState::Disabled);
        }
        if let Some(property) = self.fill_color {
            let color = property.resolve(states);
            root = root.active_color(color);
            if state == CheckedState::Indeterminate {
                root = root.inactive_color(color);
            }
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
        let widget = root.build(theme);
        if self.autofocus {
            incular_widgets::Focus::new(widget).autofocus(true).into()
        } else {
            widget
        }
    }
}

impl From<Checkbox> for Widget {
    fn from(value: Checkbox) -> Self {
        incular_widgets::LayoutBuilder::new(move |context, _| {
            value
                .clone()
                .build(&incular_controls::current_control_theme(context))
        })
        .into()
    }
}

/// Material radio with a typed group value.
#[derive(Clone, TypedBuilder)]
pub struct Radio<T: Clone + PartialEq + 'static> {
    #[builder(setter(into))]
    value: T,
    #[builder(default, setter(strip_option))]
    group_value: Option<T>,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default)]
    toggleable: bool,
    #[builder(default)]
    autofocus: bool,
    #[builder(default, setter(strip_option, into))]
    fill_color: Option<StateProperty<Color>>,
    #[builder(default, setter(strip_option, into))]
    check_color: Option<StateProperty<Color>>,
    #[builder(default, setter(strip_option))]
    side: Option<Border>,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn(T) + 'static>>
            where
                F: Fn(T) + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_changed: Option<Rc<dyn Fn(T) + 'static>>,
    #[builder(default, setter(skip))]
    group_state: Option<Rc<std::cell::RefCell<Option<T>>>>,
    #[builder(default, setter(skip))]
    group_revision: Option<Rc<Cell<u64>>>,
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
            group_state: None,
            group_revision: None,
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
        if let (Some(state), Some(revision)) =
            (value.group_state.clone(), value.group_revision.clone())
        {
            let value = Rc::new(value);
            return Widget::stateful_layout_builder(revision, move |_, _| {
                value.build_with_group(state.borrow().clone())
            });
        }
        value.build_with_group(value.group_value.clone())
    }
}

impl<T: Clone + PartialEq + 'static> Radio<T> {
    fn build_with_group(&self, group_value: Option<T>) -> Widget {
        let autofocus = self.autofocus;
        let selected = group_value.as_ref() == Some(&self.value);
        let mut selected_states = WidgetStates::default();
        if selected {
            selected_states = selected_states.with(WidgetState::Selected);
        }
        let mut normal_states = WidgetStates::default();
        if !self.enabled {
            selected_states = selected_states.with(WidgetState::Disabled);
            normal_states = normal_states.with(WidgetState::Disabled);
        }
        let mut root = incular_controls::selection::Radio::new(self.value.clone(), group_value)
            .enabled(self.enabled)
            .toggleable(self.toggleable);
        if let Some(property) = self.fill_color.as_ref() {
            root = root
                .active_color(property.resolve(selected_states))
                .inactive_color(property.resolve(normal_states));
        }
        if let Some(property) = self.check_color.as_ref() {
            root = root.dot_color(property.resolve(selected_states));
        }
        if let Some(side) = self.side {
            root = root
                .border_color(side.top.color)
                .border_width(side.top.width);
        }
        if let Some(callback) = self.on_changed.clone() {
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
    revision: Rc<Cell<u64>>,
    on_changed: Option<Rc<dyn Fn(T) + 'static>>,
}

impl<T: Clone + PartialEq + 'static> RadioGroup<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            selected: Rc::new(std::cell::RefCell::new(None)),
            revision: Rc::new(Cell::new(0)),
            on_changed: None,
        }
    }

    #[must_use]
    pub fn selected(&self) -> Option<T> {
        self.selected.borrow().clone()
    }

    pub fn set_selected(&self, value: Option<T>) {
        if *self.selected.borrow() != value {
            *self.selected.borrow_mut() = value;
            self.revision.set(self.revision.get().wrapping_add(1));
        }
    }

    #[must_use]
    pub fn on_changed(mut self, callback: impl Fn(T) + 'static) -> Self {
        self.on_changed = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn radio(&self, value: T) -> Radio<T> {
        let state = self.selected.clone();
        let revision = self.revision.clone();
        let callback = self.on_changed.clone();
        let mut radio = Radio::new(value);
        radio.group_state = Some(state.clone());
        radio.group_revision = Some(revision.clone());
        if let Some(callback) = callback {
            radio = radio.on_changed(move |value| {
                let next = Some(value.clone());
                if *state.borrow() != next {
                    *state.borrow_mut() = next;
                    revision.set(revision.get().wrapping_add(1));
                    callback(value);
                }
            });
        } else {
            radio = radio.on_changed(move |value| {
                let next = Some(value);
                if *state.borrow() != next {
                    *state.borrow_mut() = next;
                    revision.set(revision.get().wrapping_add(1));
                }
            });
        }
        radio
    }
}

impl<T: Clone + PartialEq + 'static> Default for RadioGroup<T> {
    fn default() -> Self {
        Self::new()
    }
}
