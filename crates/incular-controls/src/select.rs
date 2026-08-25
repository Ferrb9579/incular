//! Select/listbox parts.
pub use crate::popup::{Arrow, Popup, Portal, Positioner, Trigger};
use incular_widgets::Widget;
use std::rc::Rc;

#[derive(Clone)]
pub struct Root {
    value: Option<String>,
    child: Option<Widget>,
    on_change: Option<Rc<dyn Fn(String) + 'static>>,
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
            value: None,
            child: None,
            on_change: None,
        }
    }
    #[must_use]
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }
    #[must_use]
    pub fn default_value(self, value: impl Into<String>) -> Self {
        self.value(value)
    }
    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }
    #[must_use]
    pub fn on_value_change(mut self, cb: impl Fn(String) + 'static) -> Self {
        self.on_change = Some(Rc::new(cb));
        self
    }
}
impl From<Root> for Widget {
    fn from(value: Root) -> Self {
        value.child.unwrap_or_else(|| {
            let label = value.value.unwrap_or_else(|| "Select".to_owned());
            incular_controls_button(label)
        })
    }
}

fn incular_controls_button(label: String) -> Widget {
    crate::Button::new(label).into()
}
#[derive(Clone)]
pub struct List {
    child: Widget,
}
impl List {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<List> for Widget {
    fn from(value: List) -> Self {
        value.child
    }
}
#[derive(Clone)]
pub struct Item {
    value: String,
    child: Widget,
    disabled: bool,
}
impl Item {
    #[must_use]
    pub fn new(value: impl Into<String>, child: impl Into<Widget>) -> Self {
        Self {
            value: value.into(),
            child: child.into(),
            disabled: false,
        }
    }
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
    #[must_use]
    pub fn is_disabled(&self) -> bool {
        self.disabled
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
