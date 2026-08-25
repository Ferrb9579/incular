//! Convenience form composition over Incular's retained editing semantics.
use incular_widgets::Widget;
use std::rc::Rc;
#[derive(Clone)]
pub struct Form {
    child: Option<Widget>,
    disabled: bool,
    on_submit: Option<Rc<dyn Fn() + 'static>>,
}
impl Default for Form {
    fn default() -> Self {
        Self::new()
    }
}
impl Form {
    #[must_use]
    pub fn new() -> Self {
        Self {
            child: None,
            disabled: false,
            on_submit: None,
        }
    }
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }
    #[must_use]
    pub fn disabled(mut self, value: bool) -> Self {
        self.disabled = value;
        self
    }
    #[must_use]
    pub fn on_submit(mut self, cb: impl Fn() + 'static) -> Self {
        self.on_submit = Some(Rc::new(cb));
        self
    }
}
impl From<Form> for Widget {
    fn from(value: Form) -> Self {
        value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into())
    }
}
