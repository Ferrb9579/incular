//! Combobox/autocomplete shells. Query filtering and virtualization are
//! delegated to the retained list/text primitives.
pub use crate::select::{Item, List};
use incular_widgets::Widget;
use std::rc::Rc;
use typed_builder::TypedBuilder;

#[derive(Clone, TypedBuilder)]
pub struct Root {
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default = String::new(), setter(into))]
    query: String,
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
    on_query_change: Option<Rc<dyn Fn(String) + 'static>>,
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
    pub fn query(mut self, value: impl Into<String>) -> Self {
        self.query = value.into();
        self
    }
    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }
    #[must_use]
    pub fn on_query_change(mut self, cb: impl Fn(String) + 'static) -> Self {
        self.on_query_change = Some(Rc::new(cb));
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

#[cfg(test)]
mod tests {
    use super::*;
    use incular_widgets::Text;

    #[test]
    fn builder_uses_combobox_defaults() {
        let root = Root::builder().build();
        assert!(root.child.is_none());
        assert!(root.query.is_empty());
        assert!(root.on_query_change.is_none());
    }

    #[test]
    fn builder_accepts_widget_child_and_callback() {
        let root = Root::builder()
            .child(Text::new("Search"))
            .query("ap")
            .on_query_change(|_| {})
            .build();
        assert!(root.child.is_some());
        assert_eq!(root.query, "ap");
        assert!(root.on_query_change.is_some());
    }
}
