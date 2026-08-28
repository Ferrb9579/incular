//! Collapsible and accordion state containers.

use incular_widgets::Widget;
use std::rc::Rc;
use typed_builder::TypedBuilder;

#[derive(Clone, TypedBuilder)]
pub struct Root {
    #[builder(default = false)]
    open: bool,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
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
    on_change: Option<Rc<dyn Fn(bool) + 'static>>,
}
impl Default for Root {
    fn default() -> Self {
        Self::builder().build()
    }
}
impl Root {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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

#[derive(Clone, TypedBuilder)]
pub struct Trigger {
    #[builder(setter(into))]
    child: Widget,
}
impl Trigger {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
}
impl From<Trigger> for Widget {
    fn from(value: Trigger) -> Self {
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
        Self::builder().child(child).build()
    }
}
impl From<Panel> for Widget {
    fn from(value: Panel) -> Self {
        value.child
    }
}

#[derive(Clone, TypedBuilder)]
pub struct Accordion {
    #[builder(setter(into))]
    child: Widget,
    #[builder(default = false)]
    multiple: bool,
    #[builder(default = true)]
    collapsible: bool,
}
impl Accordion {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
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

#[cfg(test)]
mod tests {
    use super::*;
    use incular_widgets::Text;

    #[test]
    fn root_builder_preserves_defaults_and_accepts_generic_children() {
        let default = Root::default();
        let built = Root::builder().child(Text::new("Content")).build();

        assert!(!default.open);
        assert!(default.enabled);
        assert!(default.child.is_none());
        assert!(default.on_change.is_none());
        assert!(!built.open);
        assert!(built.enabled);
        assert!(built.child.is_some());

        let root = Root::builder()
            .open(true)
            .enabled(false)
            .on_change(|_| {})
            .build();
        assert!(root.open);
        assert!(!root.enabled);
        assert!(root.on_change.is_some());
        let _: Widget = root.into();
    }

    #[test]
    fn required_parts_keep_required_children_and_explicit_accordion_defaults() {
        let trigger = Trigger::builder().child(Text::new("Trigger")).build();
        let panel = Panel::builder().child(Text::new("Panel")).build();
        let accordion = Accordion::builder().child(Text::new("Items")).build();

        assert!(trigger.child.text_if_any().is_some());
        assert!(panel.child.text_if_any().is_some());
        assert!(!accordion.multiple);
        assert!(accordion.collapsible);

        let _: Widget = Trigger::new(Text::new("Trigger")).into();
        let _: Widget = Panel::new(Text::new("Panel")).into();
        let _: Widget = Accordion::new(Text::new("Items")).into();
    }
}
