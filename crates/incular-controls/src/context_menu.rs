//! Context-menu naming facade. Context menus reuse the regular menu engine;
//! only the anchor trigger differs at the platform input layer.

pub use crate::menu::{
    Arrow, Close, Description, Group, Item, PopupRoot as Root, Portal, Positioner, Separator, Title,
};

use incular_core::SECONDARY_POINTER_BUTTON;
use incular_widgets::{Listener, RawPointerEvent, Widget};
use std::rc::Rc;
use typed_builder::TypedBuilder;

/// Context-menu anchor that reacts to a real secondary-button press.
///
/// This is deliberately a thin wrapper over Widgets' metadata-rich `Listener`;
/// it owns no parallel gesture state and does not turn a secondary click into a
/// primary tap. The callback receives the original local pointer event so the
/// caller can position/open its retained menu model at the click location.
#[derive(Clone, TypedBuilder)]
pub struct Trigger {
    #[builder(setter(into))]
    child: Widget,
    #[builder(default, setter(skip))]
    on_open: Option<Rc<dyn Fn(RawPointerEvent)>>,
}

impl Trigger {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }

    #[must_use]
    pub fn on_open(mut self, callback: impl Fn(RawPointerEvent) + 'static) -> Self {
        self.on_open = Some(Rc::new(callback));
        self
    }
}

impl From<Trigger> for Widget {
    fn from(value: Trigger) -> Self {
        let Some(callback) = value.on_open else {
            return value.child;
        };
        Listener::new(value.child)
            .on_pointer_down(move |event| {
                if event.button == Some(SECONDARY_POINTER_BUTTON) {
                    callback(event);
                }
            })
            .into()
    }
}
