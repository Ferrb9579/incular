//! Menu, context-menu, and menubar parts. All navigation consumers can share
//! `CompositeController` from this crate.

pub use crate::popup::{
    Arrow, Close, Description, Popup, Portal, Positioner, Root as PopupRoot, Title, Trigger,
};
use incular_widgets::Widget;
use typed_builder::TypedBuilder;

#[derive(Clone, Default, TypedBuilder)]
pub struct Root {
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default = false)]
    modal: bool,
}

impl Root {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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

#[derive(Clone, TypedBuilder)]
pub struct Item {
    #[builder(setter(into))]
    child: Widget,
    #[builder(default = false)]
    disabled: bool,
}

impl Item {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
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

#[derive(Clone, Default, TypedBuilder)]
pub struct Separator {
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
}

impl Separator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }
}

impl From<Separator> for Widget {
    fn from(value: Separator) -> Self {
        value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into())
    }
}

#[derive(Clone, TypedBuilder)]
pub struct Group {
    #[builder(setter(into))]
    child: Widget,
}

impl Group {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
}

impl From<Group> for Widget {
    fn from(value: Group) -> Self {
        value.child
    }
}
pub type ContextMenu = Root;
pub type Menubar = Root;
