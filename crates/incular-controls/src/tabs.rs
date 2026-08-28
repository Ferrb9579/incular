//! Tab anatomy and shared composite-navigation configuration.

use crate::{CompositeController, CompositeOrientation};
use incular_widgets::Widget;
use std::rc::Rc;
use typed_builder::TypedBuilder;

#[derive(Clone, TypedBuilder)]
pub struct Root {
    #[builder(default, setter(strip_option, into))]
    value: Option<String>,
    #[builder(default)]
    orientation: CompositeOrientation,
    #[builder(default = true)]
    automatic: bool,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn(String) + 'static>>
            where
                F: Fn(String) + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_change: Option<Rc<dyn Fn(String) + 'static>>,
}
impl Default for Root {
    fn default() -> Self {
        Self::builder().build()
    }
}
impl Root {
    #[must_use]
    pub fn new() -> Self {
        Self::builder().build()
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

#[derive(Clone, TypedBuilder)]
pub struct List {
    #[builder(setter(into))]
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
#[derive(Clone, TypedBuilder)]
pub struct Tab {
    #[builder(setter(into))]
    value: String,
    #[builder(setter(into))]
    child: Widget,
    #[builder(default)]
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
#[derive(Clone, TypedBuilder)]
pub struct Panel {
    #[builder(setter(into))]
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
#[derive(Clone, TypedBuilder)]
pub struct Indicator {
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
}
impl Indicator {
    #[must_use]
    pub fn new() -> Self {
        Self::builder().build()
    }
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }
}
impl Default for Indicator {
    fn default() -> Self {
        Self::builder().build()
    }
}
impl From<Indicator> for Widget {
    fn from(value: Indicator) -> Self {
        value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use incular_widgets::Text;

    #[test]
    fn builders_use_tabs_defaults() {
        let root = Root::builder().build();
        assert!(root.value.is_none());
        assert_eq!(root.orientation, CompositeOrientation::Horizontal);
        assert!(root.automatic);
        assert!(root.child.is_none());
        assert!(root.on_change.is_none());

        let indicator = Indicator::builder().build();
        assert!(indicator.child.is_none());
    }

    #[test]
    fn builders_preserve_required_parts_and_widget_composition() {
        let root = Root::builder()
            .value("overview")
            .orientation(CompositeOrientation::Vertical)
            .automatic(false)
            .child(Text::new("Tabs"))
            .on_change(|_| {})
            .build();
        assert_eq!(root.orientation, CompositeOrientation::Vertical);
        assert!(!root.automatic);
        assert!(root.child.is_some());
        assert!(root.on_change.is_some());
        assert_eq!(
            root.navigation().orientation(),
            CompositeOrientation::Vertical
        );

        let list = List::builder().child(Text::new("List")).build();
        let tab = Tab::builder()
            .value("overview")
            .child(Text::new("Overview"))
            .build();
        let panel = Panel::builder().child(Text::new("Panel")).build();
        let _: Widget = list.into();
        let _: Widget = tab.into();
        let _: Widget = panel.into();
    }
}
