//! Compound switch anatomy built on the retained checkbox-style behavior.

use crate::{ControlTheme, Switch};
use incular_core::Color;
use incular_semantics::{Role as SemanticRole, SemanticActionKind, SemanticState};
use incular_widgets::{Button as RawButton, ExplicitSemantics, Widget};
use std::rc::Rc;

#[derive(Clone)]
pub struct Root {
    checked: bool,
    default_checked: bool,
    enabled: bool,
    read_only: bool,
    child: Option<Widget>,
    on_change: Option<Rc<dyn Fn(bool) + 'static>>,
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
            checked: false,
            default_checked: false,
            enabled: true,
            read_only: false,
            child: None,
            on_change: None,
        }
    }
    #[must_use]
    pub fn checked(mut self, value: bool) -> Self {
        self.checked = value;
        self
    }
    #[must_use]
    pub fn default_checked(mut self, value: bool) -> Self {
        self.default_checked = value;
        self.checked = value;
        self
    }
    #[must_use]
    pub fn is_checked(&self) -> bool {
        self.checked
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
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }
    #[must_use]
    pub fn on_checked_change(mut self, callback: impl Fn(bool) + 'static) -> Self {
        self.on_change = Some(Rc::new(callback));
        self
    }
    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        if let Some(child) = &self.child {
            let mut button = RawButton::with_child(child.clone())
                .color(Color::TRANSPARENT)
                .disabled_color(theme.colors.disabled_surface)
                .enabled(self.enabled && !self.read_only);
            if let Some(callback) = self.on_change.clone() {
                if self.enabled && !self.read_only {
                    let checked = self.checked;
                    button = button.on_click(move || callback(!checked));
                }
            }
            let raw: Widget = button.into();
            return raw.semantics(
                ExplicitSemantics::new(SemanticRole::Button)
                    .state(SemanticState {
                        enabled: self.enabled,
                        focusable: self.enabled || self.read_only,
                        checked: Some(self.checked),
                        ..SemanticState::default()
                    })
                    .actions(if self.enabled && !self.read_only {
                        [SemanticActionKind::Focus, SemanticActionKind::Activate].to_vec()
                    } else {
                        vec![SemanticActionKind::Focus]
                    }),
            );
        }
        let mut switch = Switch::new(self.checked).enabled(self.enabled && !self.read_only);
        if let Some(callback) = self.on_change.clone() {
            switch = switch.on_changed(move |value| callback(value));
        }
        switch.into()
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

#[derive(Clone)]
pub struct Thumb {
    child: Option<Widget>,
}
impl Default for Thumb {
    fn default() -> Self {
        Self::new()
    }
}
impl Thumb {
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
impl From<Thumb> for Widget {
    fn from(value: Thumb) -> Self {
        value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into())
    }
}
