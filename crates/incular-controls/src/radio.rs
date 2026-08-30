//! Radio-group compound anatomy.

use crate::{ControlTheme, Radio};
use incular_core::Color;
use incular_semantics::{Role as SemanticRole, SemanticActionKind, SemanticState};
use incular_widgets::internal::ExplicitSemantics;
use incular_widgets::{Border, Widget, internal::ActionSurface};
use std::rc::Rc;
use typed_builder::TypedBuilder;

#[derive(Clone, TypedBuilder)]
pub struct Group<T: Clone + PartialEq + 'static> {
    #[builder(default, setter(strip_option))]
    selected: Option<T>,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
}
impl<T: Clone + PartialEq + 'static> Group<T> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn value(mut self, value: T) -> Self {
        self.selected = Some(value);
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
}
impl<T: Clone + PartialEq + 'static> Default for Group<T> {
    fn default() -> Self {
        Self::builder().build()
    }
}
impl<T: Clone + PartialEq + 'static> From<Group<T>> for Widget {
    fn from(value: Group<T>) -> Self {
        value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into())
    }
}

#[derive(Clone, TypedBuilder)]
pub struct Root<T: Clone + PartialEq + 'static> {
    value: T,
    #[builder(default, setter(strip_option))]
    selected: Option<T>,
    #[builder(default, setter(strip_option, into))]
    label: Option<String>,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default, setter(strip_option))]
    active_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    inactive_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    dot_color: Option<Color>,
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
    on_change: Option<Rc<dyn Fn(T) + 'static>>,
}
impl<T: Clone + PartialEq + 'static> Root<T> {
    #[must_use]
    pub fn new(value: T) -> Self {
        Self {
            value,
            selected: None,
            label: None,
            enabled: true,
            child: None,
            active_color: None,
            inactive_color: None,
            dot_color: None,
            side: None,
            on_change: None,
        }
    }
    #[must_use]
    pub fn selected(mut self, value: T) -> Self {
        self.selected = Some(value);
        self
    }
    #[must_use]
    pub fn label(mut self, value: impl Into<String>) -> Self {
        self.label = Some(value.into());
        self
    }
    #[must_use]
    pub fn disabled(mut self, value: bool) -> Self {
        self.enabled = !value;
        self
    }
    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }
    #[must_use]
    pub fn active_color(mut self, value: Color) -> Self {
        self.active_color = Some(value);
        self
    }
    #[must_use]
    pub fn inactive_color(mut self, value: Color) -> Self {
        self.inactive_color = Some(value);
        self
    }
    #[must_use]
    pub fn dot_color(mut self, value: Color) -> Self {
        self.dot_color = Some(value);
        self
    }
    #[must_use]
    pub fn side(mut self, value: Border) -> Self {
        self.side = Some(value);
        self
    }
    #[must_use]
    pub fn on_value_change(mut self, callback: impl Fn(T) + 'static) -> Self {
        self.on_change = Some(Rc::new(callback));
        self
    }
    #[must_use]
    pub fn build(&self, _theme: &ControlTheme) -> Widget {
        if let Some(child) = &self.child {
            let selected = self.selected.as_ref() == Some(&self.value);
            let mut button = ActionSurface::with_child(child.clone())
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
        if let Some(color) = self.active_color {
            radio = radio.active_color(color);
        }
        if let Some(color) = self.inactive_color {
            radio = radio.inactive_color(color);
        }
        if let Some(color) = self.dot_color {
            radio = radio.dot_color(color);
        }
        if let Some(side) = self.side {
            radio = radio
                .border_color(side.top.color)
                .border_width(side.top.width);
        }
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
            let theme = incular_widgets::internal::current_build_environment::<ControlTheme>()
                .unwrap_or_default();
            value.build(&theme)
        })
    }
}

#[derive(Clone, TypedBuilder)]
pub struct Indicator {
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
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
impl Default for Indicator {
    fn default() -> Self {
        Self::builder().build()
    }
}
impl From<Indicator> for Widget {
    fn from(value: Indicator) -> Self {
        value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into())
    }
}
