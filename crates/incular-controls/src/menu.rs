//! Menu, context-menu, and menubar parts. All navigation consumers can share
//! `CompositeController` from this crate.

pub use crate::popup::{
    Arrow, Close, Description, Popup, Portal, Positioner, Root as PopupRoot, Title, Trigger,
};
use incular_widgets::Widget;

#[derive(Clone)]
pub struct Root {
    child: Option<Widget>,
    modal: bool,
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
            child: None,
            modal: false,
        }
    }
    #[must_use]
    pub fn modal(mut self, value: bool) -> Self {
        self.modal = value;
        self
    }
    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }
}
impl From<Root> for Widget {
    fn from(value: Root) -> Self {
        value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into())
    }
}
#[derive(Clone)]
pub struct Item {
    child: Widget,
    disabled: bool,
}
impl Item {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            disabled: false,
        }
    }
    #[must_use]
    pub fn disabled(mut self, value: bool) -> Self {
        self.disabled = value;
        self
    }
}
impl From<Item> for Widget {
    fn from(value: Item) -> Self {
        value.child
    }
}
#[derive(Clone, Default)]
pub struct Separator {
    child: Option<Widget>,
}
impl From<Separator> for Widget {
    fn from(value: Separator) -> Self {
        value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into())
    }
}
#[derive(Clone)]
pub struct Group {
    child: Widget,
}
impl Group {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Group> for Widget {
    fn from(value: Group) -> Self {
        value.child
    }
}
pub type ContextMenu = Root;
pub type Menubar = Root;
