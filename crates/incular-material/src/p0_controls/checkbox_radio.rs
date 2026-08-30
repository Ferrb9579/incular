use crate::foundation::{StateProperty, WidgetState, WidgetStates};
use incular_controls::CheckedState;
use incular_core::Color;
use incular_widgets::{Border, BorderRadius, Widget};
use std::rc::Rc;
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
