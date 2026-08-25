//! Combobox/autocomplete shells. Query filtering and virtualization are
//! delegated to the retained list/text primitives.
pub use crate::select::{Item, List};
use incular_widgets::Widget;
use std::rc::Rc;
#[derive(Clone)]
pub struct Root {
    child: Option<Widget>,
    query: String,
    on_query_change: Option<Rc<dyn Fn(String) + 'static>>,
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
            query: String::new(),
            on_query_change: None,
        }
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
