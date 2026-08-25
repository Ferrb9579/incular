//! Compound checkbox anatomy and tri-state convenience API.

use crate::{Checkbox, ControlTheme};
use incular_core::Color;
use incular_semantics::{Role as SemanticRole, SemanticActionKind, SemanticState};
use incular_widgets::{Button as RawButton, ExplicitSemantics, Widget};
use std::rc::Rc;

/// Explicit checkbox value, including the mixed state used by tree views.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CheckedState {
    #[default]
    Unchecked,
    Checked,
    Indeterminate,
}

impl CheckedState {
    #[must_use]
    pub const fn is_checked(self) -> bool {
        matches!(self, Self::Checked)
    }
}

/// Compound checkbox root. `child` is optional; when omitted the default
/// polished Incular indicator and optional label are composed for the caller.
#[derive(Clone)]
pub struct Root {
    state: CheckedState,
    enabled: bool,
    read_only: bool,
    required: bool,
    label: Option<String>,
    child: Option<Widget>,
    on_change: Option<Rc<dyn Fn(CheckedState) + 'static>>,
}

impl Default for Root {
    fn default() -> Self {
        Self::new()
    }
}

impl Root {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: CheckedState::Unchecked,
            enabled: true,
            read_only: false,
            required: false,
            label: None,
            child: None,
            on_change: None,
        }
    }
    #[must_use]
    pub fn checked(mut self, checked: bool) -> Self {
        self.state = if checked {
            CheckedState::Checked
        } else {
            CheckedState::Unchecked
        };
        self
    }
    #[must_use]
    pub fn default_checked(self, checked: bool) -> Self {
        self.checked(checked)
    }
    #[must_use]
    pub fn state(mut self, state: CheckedState) -> Self {
        self.state = state;
        self
    }
    #[must_use]
    pub fn indeterminate(self, value: bool) -> Self {
        self.state(if value {
            CheckedState::Indeterminate
        } else {
            CheckedState::Unchecked
        })
    }
    #[must_use]
    pub fn disabled(mut self, value: bool) -> Self {
        self.enabled = !value;
        self
    }
    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }
    #[must_use]
    pub fn read_only(mut self, value: bool) -> Self {
        self.read_only = value;
        self
    }
    #[must_use]
    pub fn required(mut self, value: bool) -> Self {
        self.required = value;
        self
    }
    #[must_use]
    pub fn is_required(&self) -> bool {
        self.required
    }
    #[must_use]
    pub fn state_value(&self) -> CheckedState {
        self.state
    }
    #[must_use]
    pub fn label(mut self, value: impl Into<String>) -> Self {
        self.label = Some(value.into());
        self
    }
    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }
    #[must_use]
    pub fn on_checked_change(mut self, callback: impl Fn(CheckedState) + 'static) -> Self {
        self.on_change = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let mut checkbox =
            Checkbox::new(self.state.is_checked()).enabled(self.enabled && !self.read_only);
        if let Some(label) = &self.label {
            checkbox = checkbox.label(label.clone());
        }
        if let Some(callback) = self.on_change.clone() {
            let state = self.state;
            checkbox = checkbox.on_changed(move |next| {
                callback(if next {
                    CheckedState::Checked
                } else if state == CheckedState::Indeterminate {
                    CheckedState::Indeterminate
                } else {
                    CheckedState::Unchecked
                });
            });
        }
        if let Some(child) = self.child.clone() {
            // A custom child is a visual slot. The root still owns the one
            // retained hit target and semantic state; replacing the visual
            // never drops checkbox behavior.
            let state = self.state;
            let mut button = RawButton::with_child(child)
                .color(Color::TRANSPARENT)
                .disabled_color(theme.colors.disabled_surface)
                .enabled(self.enabled && !self.read_only);
            if let Some(callback) = self.on_change.clone() {
                if self.enabled && !self.read_only {
                    button = button.on_click(move || {
                        callback(if state.is_checked() {
                            CheckedState::Unchecked
                        } else {
                            CheckedState::Checked
                        })
                    });
                }
            }
            let raw: Widget = button.into();
            return raw.semantics(
                ExplicitSemantics::new(SemanticRole::Checkbox)
                    .label(self.label.clone().unwrap_or_default())
                    .state(SemanticState {
                        enabled: self.enabled,
                        focusable: self.enabled || self.read_only,
                        checked: match self.state {
                            CheckedState::Indeterminate => None,
                            CheckedState::Unchecked => Some(false),
                            CheckedState::Checked => Some(true),
                        },
                        ..SemanticState::default()
                    })
                    .actions(if self.enabled && !self.read_only {
                        [SemanticActionKind::Focus, SemanticActionKind::Activate].to_vec()
                    } else {
                        vec![SemanticActionKind::Focus]
                    }),
            );
        }
        checkbox.into()
    }
}

impl From<Root> for Widget {
    fn from(value: Root) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| {
            let theme =
                incular_widgets::current_build_environment::<ControlTheme>().unwrap_or_default();
            value.build(&theme)
        })
    }
}

/// Visual indicator slot. It is transparent and can be replaced by an icon,
/// path, or application widget without changing root semantics.
#[derive(Clone)]
pub struct Indicator {
    child: Option<Widget>,
}
impl Default for Indicator {
    fn default() -> Self {
        Self::new()
    }
}
impl Indicator {
    #[must_use]
    pub fn new() -> Self {
        Self { child: None }
    }
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }
}
impl From<Indicator> for Widget {
    fn from(value: Indicator) -> Self {
        value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into())
    }
}

/// Shared value collection for checkbox groups. Rendering remains ordinary
/// child composition; the group owns selection state and validation metadata.
#[derive(Clone)]
pub struct Group {
    values: Rc<std::cell::RefCell<Vec<String>>>,
    enabled: bool,
    child: Option<Widget>,
}
impl Default for Group {
    fn default() -> Self {
        Self::new()
    }
}
impl Group {
    #[must_use]
    pub fn new() -> Self {
        Self {
            values: Rc::new(std::cell::RefCell::new(Vec::new())),
            enabled: true,
            child: None,
        }
    }
    #[must_use]
    pub fn values(self, values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.values
            .borrow_mut()
            .extend(values.into_iter().map(Into::into));
        self
    }
    #[must_use]
    pub fn disabled(mut self, value: bool) -> Self {
        self.enabled = !value;
        self
    }
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }
    #[must_use]
    pub fn selected(&self) -> Vec<String> {
        self.values.borrow().clone()
    }
}
impl From<Group> for Widget {
    fn from(value: Group) -> Self {
        value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into())
    }
}
