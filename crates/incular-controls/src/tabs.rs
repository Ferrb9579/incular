//! Tab anatomy and shared composite-navigation configuration.

use crate::{CompositeController, CompositeOrientation};
use incular_widgets::Widget;
use std::rc::Rc;

#[derive(Clone)]
pub struct Root {
    value: Option<String>,
    orientation: CompositeOrientation,
    automatic: bool,
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
            orientation: CompositeOrientation::Horizontal,
            automatic: true,
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
    pub fn vertical(mut self) -> Self {
        self.orientation = CompositeOrientation::Vertical;
        self
    }
    #[must_use]
    pub fn manual_activation(mut self, value: bool) -> Self {
        self.automatic = !value;
        self
    }
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }
    #[must_use]
    pub fn on_value_change(mut self, callback: impl Fn(String) + 'static) -> Self {
        self.on_change = Some(Rc::new(callback));
        self
    }
    #[must_use]
    pub fn navigation(&self) -> CompositeController {
        CompositeController::new().orientation_set(self.orientation)
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
pub struct Tab {
    value: String,
    child: Widget,
    disabled: bool,
}
impl Tab {
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
impl From<Tab> for Widget {
    fn from(value: Tab) -> Self {
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
pub struct Indicator {
    child: Option<Widget>,
}
impl Indicator {
    #[must_use]
    pub fn new() -> Self {
        Self { child: None }
    }
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }
}
impl Default for Indicator {
    fn default() -> Self {
        Self::new()
    }
}
impl From<Indicator> for Widget {
    fn from(value: Indicator) -> Self {
        value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into())
    }
}
