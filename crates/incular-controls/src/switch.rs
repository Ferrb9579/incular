//! Compound switch anatomy built on the retained checkbox-style behavior.

use crate::{ControlTheme, Switch};
use incular_core::Color;
use incular_semantics::{Role as SemanticRole, SemanticActionKind, SemanticState};
use incular_widgets::{
    Widget,
    internal::{ActionSurface, ExplicitSemantics},
};
use std::rc::Rc;
use typed_builder::TypedBuilder;

#[derive(Clone, TypedBuilder)]
pub struct Root {
    #[builder(default)]
    default_checked: bool,
    #[builder(default = *default_checked)]
    checked: bool,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default)]
    read_only: bool,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default, setter(strip_option))]
    active_track_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    inactive_track_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    active_thumb_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    inactive_thumb_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    outline_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    outline_width: Option<f32>,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn(bool) + 'static>>
            where
                F: Fn(bool) + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_change: Option<Rc<dyn Fn(bool) + 'static>>,
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
    pub fn active_thumb_color(mut self, value: Color) -> Self {
        self.active_thumb_color = Some(value);
        self
    }
    #[must_use]
    pub fn inactive_thumb_color(mut self, value: Color) -> Self {
        self.inactive_thumb_color = Some(value);
        self
    }
    #[must_use]
    pub fn outline_color(mut self, value: Color) -> Self {
        self.outline_color = Some(value);
        self
    }

    #[must_use]
    pub fn outline_width(mut self, value: f32) -> Self {
        self.outline_width = Some(value.max(0.0));
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
            let mut button = ActionSurface::with_child(child.clone())
                .color(Color::TRANSPARENT)
                .disabled_color(theme.colors.disabled_surface)
                .enabled(self.enabled && !self.read_only);
            if let Some(callback) = self.on_change.clone()
                && self.enabled
                && !self.read_only
            {
                let checked = self.checked;
                button = button.on_click(move || callback(!checked));
            }
            let raw: Widget = button.into();
            return raw.semantics(
                ExplicitSemantics::new(SemanticRole::Switch)
                    .state(SemanticState {
                        enabled: self.enabled,
                        focusable: self.enabled || self.read_only,
                        checked: Some(self.checked.into()),
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
        if let Some(color) = self.active_track_color {
            switch = switch.active_track_color(color);
        }
        if let Some(color) = self.inactive_track_color {
            switch = switch.inactive_track_color(color);
        }
        if let Some(color) = self.active_thumb_color {
            switch = switch.active_thumb_color(color);
        }
        if let Some(color) = self.inactive_thumb_color {
            switch = switch.inactive_thumb_color(color);
        }
        if let Some(color) = self.outline_color {
            switch = switch.outline_color(color);
        }
        if let Some(width) = self.outline_width {
            switch = switch.outline_width(width);
        }
        if let Some(callback) = self.on_change.clone() {
            switch = switch.on_changed(move |value| callback(value));
        }
        switch.into()
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

#[derive(Clone, TypedBuilder)]
pub struct Thumb {
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
}
impl Default for Thumb {
    fn default() -> Self {
        Self::builder().build()
    }
}
impl Thumb {
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
impl From<Thumb> for Widget {
    fn from(value: Thumb) -> Self {
        value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into())
    }
}
