//! Accessible form-field anatomy. The root is intentionally visual-neutral;
//! labels, descriptions, and errors compose with any input control.

use incular_config::CrossAxisAlignment;
use incular_semantics::{Role as SemanticRole, SemanticState};
use incular_widgets::internal::ExplicitSemantics;
use incular_widgets::{Column, Text, Widget};
use std::rc::Rc;

#[derive(Clone)]
pub struct Root {
    child: Option<Widget>,
    label: Option<String>,
    description: Option<String>,
    error: Option<String>,
    disabled: bool,
    invalid: bool,
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
            label: None,
            description: None,
            error: None,
            disabled: false,
            invalid: false,
        }
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
        self.invalid = true;
        self
    }
    #[must_use]
    pub fn disabled(mut self, value: bool) -> Self {
        self.disabled = value;
        self
    }
    #[must_use]
    pub fn invalid(mut self, value: bool) -> Self {
        self.invalid = value;
        self
    }
    #[must_use]
    pub fn is_invalid(&self) -> bool {
        self.invalid
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
                    read_only: self.invalid,
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

#[derive(Clone)]
pub struct Label {
    child: Widget,
}
impl Label {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            child: incular_widgets::Text::new(label).into(),
        }
    }
    #[must_use]
    pub fn child(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Label> for Widget {
    fn from(value: Label) -> Self {
        value.child
    }
}

#[derive(Clone)]
pub struct Description {
    child: Widget,
}

#[derive(Clone)]
pub struct Control {
    child: Widget,
}
impl Control {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
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
        Self {
            child: incular_widgets::Text::new(text).into(),
        }
    }
}
impl From<Description> for Widget {
    fn from(value: Description) -> Self {
        value.child
    }
}

#[derive(Clone)]
pub struct Error {
    child: Widget,
}
impl Error {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            child: incular_widgets::Text::new(text).into(),
        }
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
