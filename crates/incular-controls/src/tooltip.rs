//! Tooltip provider and compound parts. Delay policy belongs to the runtime;
//! this module keeps the composition surface stable.
pub use crate::popup::{
    Arrow, Description as PopupDescription, Popup, Portal, Positioner, Root, Trigger,
};
use typed_builder::TypedBuilder;

#[derive(Clone, Default, TypedBuilder)]
pub struct Provider {
    #[builder(default = false)]
    skip_delay: bool,
}

impl Provider {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn skip_delay(mut self, value: bool) -> Self {
        self.skip_delay = value;
        self
    }
}
impl From<Provider> for incular_widgets::Widget {
    fn from(_: Provider) -> Self {
        incular_widgets::SizedBox::shrink().into()
    }
}
