//! Convenience form composition over Incular's retained editing semantics.
use incular_widgets::Widget;
use std::rc::Rc;
use typed_builder::TypedBuilder;

#[derive(Clone, TypedBuilder)]
pub struct Form {
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default = false)]
    disabled: bool,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn() + 'static>>
            where
                F: Fn() + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_submit: Option<Rc<dyn Fn() + 'static>>,
}
impl Default for Form {
    fn default() -> Self {
        Self::builder().build()
    }
}
impl Form {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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

#[cfg(test)]
mod tests {
    use super::*;
    use incular_widgets::Text;

    #[test]
    fn builder_preserves_form_defaults_and_accepts_widgets_and_callbacks() {
        let default = Form::default();
        let built = Form::builder().build();

        assert!(default.child.is_none());
        assert!(built.child.is_none());
        assert!(!built.disabled);
        assert!(built.on_submit.is_none());

        let form = Form::builder()
            .child(Text::new("Fields"))
            .disabled(true)
            .on_submit(|| {})
            .build();
        assert!(form.child.is_some());
        assert!(form.disabled);
        assert!(form.on_submit.is_some());
        let _: Widget = form.into();
    }
}
