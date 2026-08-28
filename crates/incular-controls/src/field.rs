//! Accessible form-field anatomy. The root is intentionally visual-neutral;
//! labels, descriptions, and errors compose with any input control.

use incular_config::CrossAxisAlignment;
use incular_semantics::{Role as SemanticRole, SemanticState};
use incular_widgets::internal::ExplicitSemantics;
use incular_widgets::{Column, Text, Widget};
use std::rc::Rc;
use typed_builder::TypedBuilder;

#[derive(Clone, Default, TypedBuilder)]
pub struct Root {
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    label: Option<String>,
    #[builder(default, setter(strip_option, into))]
    description: Option<String>,
    #[builder(default, setter(strip_option, into))]
    error: Option<String>,
    #[builder(default = false)]
    disabled: bool,
    #[builder(default, setter(strip_option))]
    invalid: Option<bool>,
}
impl Root {
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
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
    #[must_use]
    pub fn error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self.invalid = Some(true);
        self
    }
    #[must_use]
    pub fn disabled(mut self, value: bool) -> Self {
        self.disabled = value;
        self
    }
    #[must_use]
    pub fn invalid(mut self, value: bool) -> Self {
        self.invalid = Some(value);
        self
    }
    #[must_use]
    pub fn is_invalid(&self) -> bool {
        self.invalid.unwrap_or_else(|| self.error.is_some())
    }
    #[must_use]
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    #[must_use]
    pub fn build(&self) -> Widget {
        let mut children = Vec::new();
        if let Some(label) = &self.label {
            children.push(Text::new(label.clone()).into());
        }
        if let Some(child) = &self.child {
            children.push(child.clone());
        }
        if let Some(description) = &self.description {
            children.push(Text::new(description.clone()).into());
        }
        if let Some(error) = &self.error {
            children.push(Text::new(error.clone()).into());
        }
        // Form fields are block-level controls: labels and validation text
        // align with the leading edge while the input may expand to the
        // available width.  Column's generic default is centered, which made
        // a full-width field look detached from its label in desktop layouts.
        let visual: Widget = Column::new(children)
            .spacing(4.)
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .into();
        let invalid = self.is_invalid();
        visual.semantics(
            ExplicitSemantics::new(SemanticRole::GenericContainer)
                .label(self.label.clone().unwrap_or_default())
                .description(
                    self.description
                        .clone()
                        .or_else(|| self.error.clone())
                        .unwrap_or_default(),
                )
                .state(SemanticState {
                    enabled: !self.disabled,
                    read_only: invalid,
                    ..SemanticState::default()
                }),
        )
    }
}
impl From<Root> for Widget {
    fn from(value: Root) -> Self {
        value.build()
    }
}

#[derive(Clone, TypedBuilder)]
pub struct Label {
    #[builder(setter(into))]
    child: Widget,
}
impl Label {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self::builder()
            .child(incular_widgets::Text::new(label))
            .build()
    }
    #[must_use]
    pub fn child(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
}
impl From<Label> for Widget {
    fn from(value: Label) -> Self {
        value.child
    }
}

#[derive(Clone, TypedBuilder)]
pub struct Description {
    #[builder(setter(into))]
    child: Widget,
}

#[derive(Clone, TypedBuilder)]
pub struct Control {
    #[builder(setter(into))]
    child: Widget,
}
impl Control {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
}
impl From<Control> for Widget {
    fn from(value: Control) -> Self {
        value.child
    }
}
impl Description {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self::builder()
            .child(incular_widgets::Text::new(text))
            .build()
    }
    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
}
impl From<Description> for Widget {
    fn from(value: Description) -> Self {
        value.child
    }
}

#[derive(Clone, TypedBuilder)]
pub struct Error {
    #[builder(setter(into))]
    child: Widget,
}
impl Error {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self::builder()
            .child(incular_widgets::Text::new(text))
            .build()
    }
    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
}
impl From<Error> for Widget {
    fn from(value: Error) -> Self {
        value.child
    }
}

/// Small validation state handle useful when a form updates without replacing
/// its entire widget descriptor.
#[derive(Clone, Default)]
pub struct ValidationState {
    invalid: Rc<std::cell::Cell<bool>>,
}
impl ValidationState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn invalid(&self) -> bool {
        self.invalid.get()
    }
    pub fn set_invalid(&self, value: bool) {
        self.invalid.set(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_builder_matches_semantic_defaults_and_composes_widgets() {
        let default = Root::default();
        assert_eq!(Root::new().is_disabled(), default.is_disabled());
        assert_eq!(Root::new().is_invalid(), default.is_invalid());

        let root = Root::builder()
            .child(Text::new("control"))
            .label("Name")
            .description("Your display name")
            .error("Required")
            .disabled(true)
            .build();

        assert!(root.is_disabled());
        assert!(root.is_invalid());
        assert_eq!(
            root.child.as_ref().and_then(Widget::text_if_any).as_deref(),
            Some("control")
        );

        let _: Widget = root.into();
    }

    #[test]
    fn explicit_invalid_override_remains_stronger_than_error_inference() {
        assert!(Root::new().error("invalid").is_invalid());
        assert!(!Root::new().error("invalid").invalid(false).is_invalid());
        assert!(
            !Root::builder()
                .error("invalid")
                .invalid(false)
                .build()
                .is_invalid()
        );
    }

    #[test]
    fn required_child_descriptors_accept_arbitrary_widgets() {
        let label = Label::builder().child(Text::new("Label")).build();
        let description = Description::builder()
            .child(Text::new("Description"))
            .build();
        let control = Control::builder().child(Text::new("Control")).build();
        let error = Error::builder().child(Text::new("Error")).build();

        assert_eq!(label.child.text_if_any().as_deref(), Some("Label"));
        assert_eq!(
            description.child.text_if_any().as_deref(),
            Some("Description")
        );
        assert_eq!(control.child.text_if_any().as_deref(), Some("Control"));
        assert_eq!(error.child.text_if_any().as_deref(), Some("Error"));
    }

    #[test]
    fn legacy_constructors_and_widget_conversions_remain_available() {
        let _: Widget = Label::new("Label").into();
        let _: Widget = Label::child(Text::new("Label")).into();
        let _: Widget = Description::new("Description").into();
        let _: Widget = Control::new(Text::new("Control")).into();
        let _: Widget = Error::new("Error").into();
    }
}
