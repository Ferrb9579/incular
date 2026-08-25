//! Tooltip provider and compound parts. Delay policy belongs to the runtime;
//! this module keeps the composition surface stable.
pub use crate::popup::{
    Arrow, Description as PopupDescription, Popup, Portal, Positioner, Root, Trigger,
};

#[derive(Clone, Default)]
pub struct Provider {
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
