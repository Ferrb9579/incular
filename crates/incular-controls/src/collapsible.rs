//! Collapsible and accordion state containers.

use incular_widgets::Widget;
use std::rc::Rc;

#[derive(Clone)]
pub struct Root {
    open: bool,
    enabled: bool,
    child: Option<Widget>,
    on_change: Option<Rc<dyn Fn(bool) + 'static>>,
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
            open: false,
            enabled: true,
            child: None,
            on_change: None,
        }
    }
    #[must_use]
    pub fn open(mut self, value: bool) -> Self {
        self.open = value;
        self
    }
    #[must_use]
    pub fn default_open(self, value: bool) -> Self {
        self.open(value)
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
    #[must_use]
    pub fn on_open_change(mut self, callback: impl Fn(bool) + 'static) -> Self {
        self.on_change = Some(Rc::new(callback));
        self
    }
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
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
pub struct Trigger {
    child: Widget,
}
impl Trigger {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Trigger> for Widget {
    fn from(value: Trigger) -> Self {
        value.child
    }
}
#[derive(Clone)]
pub struct Panel {
    child: Widget,
}
impl Panel {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Panel> for Widget {
    fn from(value: Panel) -> Self {
        value.child
    }
}

#[derive(Clone)]
pub struct Accordion {
    child: Widget,
    multiple: bool,
    collapsible: bool,
}
impl Accordion {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            multiple: false,
            collapsible: true,
        }
    }
    #[must_use]
    pub fn multiple(mut self, value: bool) -> Self {
        self.multiple = value;
        self
    }
    #[must_use]
    pub fn collapsible(mut self, value: bool) -> Self {
        self.collapsible = value;
        self
    }
}
impl From<Accordion> for Widget {
    fn from(value: Accordion) -> Self {
        value.child
    }
}
