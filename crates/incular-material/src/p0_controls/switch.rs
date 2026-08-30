use crate::foundation::{StateProperty, WidgetState, WidgetStates};
use incular_core::Color;
use incular_widgets::Widget;
use std::rc::Rc;
use typed_builder::TypedBuilder;

/// Material switch adapter over the shared switch mechanics.
#[derive(Clone, TypedBuilder)]
pub struct Switch {
    #[builder(default)]
    pub(super) value: bool,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default)]
    autofocus: bool,
    #[builder(default, setter(strip_option, into))]
    thumb_color: Option<StateProperty<Color>>,
    #[builder(default, setter(strip_option, into))]
    track_color: Option<StateProperty<Color>>,
    #[builder(default, setter(strip_option, into))]
    track_outline_color: Option<StateProperty<Color>>,
    #[builder(default, setter(transform = |value: f32| Some(value.max(0.0))))]
    track_outline_width: Option<f32>,
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
    on_changed: Option<Rc<dyn Fn(bool) + 'static>>,
}

impl Default for Switch {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl Switch {
    #[must_use]
    pub fn new(value: bool) -> Self {
        Self {
            value,
            enabled: true,
            autofocus: false,
            thumb_color: None,
            track_color: None,
            track_outline_color: None,
            track_outline_width: None,
            on_changed: None,
        }
    }

    #[must_use]
    pub fn value(mut self, value: bool) -> Self {
        self.value = value;
        self
    }

    #[must_use]
    pub fn selected(self, value: bool) -> Self {
        self.value(value)
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
    pub fn thumb_color(mut self, value: impl Into<StateProperty<Color>>) -> Self {
        self.thumb_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn track_color(mut self, value: impl Into<StateProperty<Color>>) -> Self {
        self.track_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn track_outline_color(mut self, value: impl Into<StateProperty<Color>>) -> Self {
        self.track_outline_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn track_outline_width(mut self, value: f32) -> Self {
        self.track_outline_width = Some(value.max(0.0));
        self
    }

    #[must_use]
    pub fn on_changed(mut self, callback: impl Fn(bool) + 'static) -> Self {
        self.on_changed = Some(Rc::new(callback));
        self
    }
}

impl From<Switch> for Widget {
    fn from(value: Switch) -> Self {
        let mut on_states = WidgetStates::default().with(WidgetState::Selected);
        let mut off_states = WidgetStates::default();
        if !value.enabled {
            on_states = on_states.with(WidgetState::Disabled);
            off_states = off_states.with(WidgetState::Disabled);
        }
        let mut root = incular_controls::switch::Root::new()
            .checked(value.value)
            .enabled(value.enabled);
        if let Some(property) = value.track_color.as_ref() {
            root = root
                .active_track_color(property.resolve(on_states))
                .inactive_track_color(property.resolve(off_states));
        }
        if let Some(property) = value.thumb_color.as_ref() {
            root = root
                .active_thumb_color(property.resolve(on_states))
                .inactive_thumb_color(property.resolve(off_states));
        }
        if let Some(property) = value.track_outline_color.as_ref() {
            root = root.outline_color(property.resolve(if value.value {
                on_states
            } else {
                off_states
            }));
        }
        if let Some(width) = value.track_outline_width {
            root = root.outline_width(width);
        }
        if let Some(callback) = value.on_changed {
            root = root.on_checked_change(move |next| callback(next));
        }
        let widget: Widget = root.into();
        if value.autofocus {
            incular_widgets::Focus::new(widget).autofocus(true).into()
        } else {
            widget
        }
    }
}
