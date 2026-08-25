//! Radio-group compound anatomy.

use crate::{ControlTheme, Radio};
use incular_core::Color;
use incular_semantics::{Role as SemanticRole, SemanticActionKind, SemanticState};
use incular_widgets::{Button as RawButton, ExplicitSemantics, Widget};
use std::rc::Rc;

#[derive(Clone)]
pub struct Group<T: Clone + PartialEq + 'static> {
    selected: Option<T>,
    enabled: bool,
    child: Option<Widget>,
}
impl<T: Clone + PartialEq + 'static> Group<T> {
    #[must_use]
    pub fn new() -> Self { Self { selected: None, enabled: true, child: None } }
    #[must_use]
    pub fn value(mut self, value: T) -> Self { self.selected = Some(value); self }
    #[must_use]
    pub fn disabled(mut self, value: bool) -> Self { self.enabled = !value; self }
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self { self.child = Some(child.into()); self }
}
impl<T: Clone + PartialEq + 'static> Default for Group<T> { fn default() -> Self { Self::new() } }
impl<T: Clone + PartialEq + 'static> From<Group<T>> for Widget {
    fn from(value: Group<T>) -> Self { value.child.unwrap_or_else(|| incular_widgets::SizedBox::shrink().into()) }
}

#[derive(Clone)]
pub struct Root<T: Clone + PartialEq + 'static> {
    value: T,
    selected: Option<T>,
    label: Option<String>,
    enabled: bool,
    child: Option<Widget>,
    on_change: Option<Rc<dyn Fn(T) + 'static>>,
}
impl<T: Clone + PartialEq + 'static> Root<T> {
    #[must_use]
    pub fn new(value: T) -> Self { Self { value, selected: None, label: None, enabled: true, child: None, on_change: None } }
    #[must_use]
    pub fn selected(mut self, value: T) -> Self { self.selected = Some(value); self }
    #[must_use]
    pub fn label(mut self, value: impl Into<String>) -> Self { self.label = Some(value.into()); self }
    #[must_use]
    pub fn disabled(mut self, value: bool) -> Self { self.enabled = !value; self }
    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self { self.child = Some(value.into()); self }
    #[must_use]
    pub fn on_value_change(mut self, callback: impl Fn(T) + 'static) -> Self { self.on_change = Some(Rc::new(callback)); self }
    #[must_use]
    pub fn build(&self, _theme: &ControlTheme) -> Widget {
        if let Some(child) = &self.child {
            let selected = self.selected.as_ref() == Some(&self.value);
            let mut button = RawButton::with_child(child.clone())
                .color(Color::TRANSPARENT)
                .enabled(self.enabled);
            if let Some(callback) = self.on_change.clone() {
                if self.enabled {
                    let value = self.value.clone();
                    button = button.on_click(move || callback(value.clone()));
                }
            }
            let raw: Widget = button.into();
            return raw.semantics(
                ExplicitSemantics::new(SemanticRole::Radio)
                    .label(self.label.clone().unwrap_or_default())
                    .state(SemanticState {
                        enabled: self.enabled,
                        focusable: self.enabled,
                        checked: Some(selected),
                        ..SemanticState::default()
                    })
                    .actions(if self.enabled {
                        [SemanticActionKind::Focus, SemanticActionKind::Activate].to_vec()
                    } else {
                        vec![SemanticActionKind::Focus]
                    }),
            );
        }
        let mut radio = Radio::new(self.value.clone(), self.selected.clone())
            .label(self.label.clone().unwrap_or_default());
        if self.enabled {
            if let Some(callback) = self.on_change.clone() {
                radio = radio.on_changed(move |value| callback(value));
            }
        }
        radio.into()
    }
}
impl<T: Clone + PartialEq + 'static> From<Root<T>> for Widget {
    fn from(value: Root<T>) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| {
            let theme = incular_widgets::current_build_environment::<ControlTheme>().unwrap_or_default();
            value.build(&theme)
        })
    }
}

#[derive(Clone)]
pub struct Indicator { child: Option<Widget> }
impl Indicator { #[must_use] pub fn new() -> Self { Self { child: None } } #[must_use] pub fn child(mut self, child: impl Into<Widget>) -> Self { self.child = Some(child.into()); self } }
impl Default for Indicator { fn default() -> Self { Self::new() } }
impl From<Indicator> for Widget { fn from(value: Indicator) -> Self { value.child.unwrap_or_else(|| incular_widgets::SizedBox::shrink().into()) } }
