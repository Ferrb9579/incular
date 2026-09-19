//! Convenience form composition over Incular's retained editing semantics.
use incular_widgets::Widget;
use std::rc::Rc;
use typed_builder::TypedBuilder;

#[derive(Clone)]
pub(crate) struct FormScope {
    controller: incular_widgets::Form,
    disabled: bool,
    on_submit: Option<Rc<dyn Fn() + 'static>>,
}

impl FormScope {
    pub(crate) fn disabled(&self) -> bool {
        self.disabled
    }

    pub(crate) fn submit(&self) -> bool {
        if self.disabled || !self.controller.submit() {
            return false;
        }
        if let Some(callback) = self.on_submit.as_ref() {
            callback();
        }
        true
    }
}

#[derive(Clone, TypedBuilder)]
pub struct Form {
    #[builder(default = incular_widgets::Form::new(), setter(skip))]
    controller: incular_widgets::Form,
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

    /// Returns the stable retained form controller used by this descriptor.
    #[must_use]
    pub fn controller(&self) -> incular_widgets::Form {
        self.controller.clone()
    }

    /// Registers a text editor with this form's existing validation/save owner.
    #[must_use]
    pub fn register(
        &self,
        controller: incular_widgets::internal::TextEditingController,
    ) -> incular_widgets::FormField {
        self.controller.register(controller)
    }

    /// Validates and saves live fields, then invokes the form submit callback.
    /// Disabled forms reject submission without invoking user callbacks.
    pub fn submit(&self) -> bool {
        FormScope {
            controller: self.controller.clone(),
            disabled: self.disabled,
            on_submit: self.on_submit.clone(),
        }
        .submit()
    }

    #[must_use]
    pub fn validate(&self) -> bool {
        !self.disabled && self.controller.validate()
    }

    pub fn reset(&self) {
        self.controller.reset();
    }
}
impl From<Form> for Widget {
    fn from(value: Form) -> Self {
        let child = value
            .child
            .clone()
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into());
        let scope = FormScope {
            controller: value.controller.clone(),
            disabled: value.disabled,
            on_submit: value.on_submit.clone(),
        };
        let child = Widget::environment_scope(scope, child);
        value.controller.child(child).into()
    }
}
