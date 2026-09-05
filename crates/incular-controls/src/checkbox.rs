//! Compound checkbox anatomy and tri-state convenience API.

use crate::{Checkbox, ControlTheme};
use incular_core::Color;
use incular_widgets::{Border, BorderRadius, Widget};
use std::rc::Rc;
use typed_builder::TypedBuilder;

pub use incular_semantics::CheckedState;

/// Compound checkbox root. `child` is optional; when omitted the default
/// polished Incular indicator and optional label are composed for the caller.
#[derive(Clone, TypedBuilder)]
pub struct Root {
    #[builder(default)]
    state: CheckedState,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default)]
    read_only: bool,
    #[builder(default)]
    required: bool,
    #[builder(default, setter(strip_option, into))]
    label: Option<String>,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default, setter(strip_option))]
    active_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    inactive_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    check_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    border_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    border_width: Option<f32>,
    #[builder(default, setter(strip_option))]
    radius: Option<f32>,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn(CheckedState) + 'static>>
            where
                F: Fn(CheckedState) + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_change: Option<Rc<dyn Fn(CheckedState) + 'static>>,
}

impl Default for Root {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl Root {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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
    /// Sets the fill color used while the indicator is checked.
    #[must_use]
    pub fn active_color(mut self, value: Color) -> Self {
        self.active_color = Some(value);
        self
    }
    /// Sets the fill color used while the indicator is unchecked.
    #[must_use]
    pub fn inactive_color(mut self, value: Color) -> Self {
        self.inactive_color = Some(value);
        self
    }
    /// Sets the color of the check or mixed-state mark.
    #[must_use]
    pub fn check_color(mut self, value: Color) -> Self {
        self.check_color = Some(value);
        self
    }
    /// Sets the indicator outline color.
    #[must_use]
    pub fn border_color(mut self, value: Color) -> Self {
        self.border_color = Some(value);
        self
    }
    /// Sets the indicator outline from a renderer-neutral border descriptor.
    #[must_use]
    pub fn side(mut self, value: Border) -> Self {
        self.border_color = Some(value.top.color);
        self.border_width = Some(value.top.width.max(0.0));
        self
    }
    /// Sets the indicator corner radius.
    #[must_use]
    pub fn shape(mut self, value: BorderRadius) -> Self {
        self.radius = Some(value.top_left.x.max(0.0));
        self
    }
    #[must_use]
    pub fn radius(mut self, value: f32) -> Self {
        self.radius = Some(value.max(0.0));
        self
    }
    #[must_use]
    pub fn on_checked_change(mut self, callback: impl Fn(CheckedState) + 'static) -> Self {
        self.on_change = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let mut checkbox = Checkbox::new(self.state.is_checked())
            .indeterminate(self.state == CheckedState::Indeterminate)
            .enabled(self.enabled)
            .read_only(self.read_only)
            .required(self.required);
        if let Some(color) = self.active_color {
            checkbox = checkbox.active_color(color);
        }
        if let Some(color) = self.inactive_color {
            checkbox = checkbox.inactive_color(color);
        }
        if let Some(color) = self.check_color {
            checkbox = checkbox.check_color(color);
        }
        if let Some(color) = self.border_color {
            checkbox = checkbox.border_color(color);
        }
        if let Some(width) = self.border_width {
            checkbox = checkbox.border_width(width);
        }
        if let Some(radius) = self.radius {
            checkbox = checkbox.radius(radius);
        }
        if let Some(label) = &self.label {
            checkbox = checkbox.label(label.clone());
        }
        if let Some(callback) = self.on_change.clone() {
            checkbox = checkbox.on_changed(move |next| callback(next.into()));
        }
        if let Some(child) = self.child.clone() {
            checkbox = checkbox.child(child);
        }
        Widget::environment_scope(theme.clone(), checkbox.into())
    }
}

impl From<Root> for Widget {
    fn from(value: Root) -> Self {
        let value = Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            let theme = crate::theme::current_control_theme(context);
            value.build(&theme)
        }))
    }
}

/// Visual indicator slot. It is transparent and can be replaced by an icon,
/// path, or application widget without changing root semantics.
#[derive(Clone, TypedBuilder)]
pub struct Indicator {
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
}
impl Default for Indicator {
    fn default() -> Self {
        Self::builder().build()
    }
}
impl Indicator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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
#[derive(Clone, TypedBuilder)]
pub struct Group {
    #[builder(
        default = Rc::new(std::cell::RefCell::new(Vec::new())),
        setter(
            fn transform<I>(values: I) -> Rc<std::cell::RefCell<Vec<String>>>
            where
                I: IntoIterator,
                I::Item: Into<String>,
            {
                Rc::new(std::cell::RefCell::new(
                    values.into_iter().map(Into::into).collect(),
                ))
            }
        )
    )]
    values: Rc<std::cell::RefCell<Vec<String>>>,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
}
impl Default for Group {
    fn default() -> Self {
        Self::builder().build()
    }
}
impl Group {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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
