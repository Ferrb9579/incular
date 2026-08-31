//! Visibility and offstage layout primitives.

use typed_builder::TypedBuilder;

use super::SizedBox;
use crate::{Widget, WidgetKind};

/// Conditionally displays a child or hides it from layout, paint, hit-testing, and semantics.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct Visibility {
    #[builder(default = true)]
    visible: bool,
    #[builder(default)]
    maintain_state: bool,
    #[builder(default)]
    maintain_size: bool,
    #[builder(default)]
    maintain_animation: bool,
    #[builder(default)]
    maintain_semantics: bool,
    #[builder(default, setter(strip_option, into))]
    replacement: Option<Widget>,
    #[builder(setter(into))]
    child: Widget,
}

impl Visibility {
    /// Creates a Visibility wrapper.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            visible: true,
            maintain_state: false,
            maintain_size: false,
            maintain_animation: false,
            maintain_semantics: false,
            replacement: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    #[must_use]
    pub fn maintain_state(mut self, maintain: bool) -> Self {
        self.maintain_state = maintain;
        self
    }

    #[must_use]
    pub fn maintain_size(mut self, maintain: bool) -> Self {
        self.maintain_size = maintain;
        self
    }

    #[must_use]
    pub fn replacement(mut self, replacement: impl Into<Widget>) -> Self {
        self.replacement = Some(replacement.into());
        self
    }
}

impl From<Visibility> for Widget {
    fn from(value: Visibility) -> Self {
        if !value.visible && !value.maintain_state && !value.maintain_size {
            value
                .replacement
                .unwrap_or_else(|| SizedBox::shrink().into())
        } else {
            Widget::from_kind(WidgetKind::Visibility {
                visible: value.visible,
                child: value.child,
            })
        }
    }
}

/// Hides its child offstage without unmounting the retained element.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct Offstage {
    #[builder(default = true)]
    offstage: bool,
    #[builder(setter(into))]
    child: Widget,
}

impl Offstage {
    /// Creates an Offstage widget.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            offstage: true,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn offstage(mut self, offstage: bool) -> Self {
        self.offstage = offstage;
        self
    }
}

impl From<Offstage> for Widget {
    fn from(value: Offstage) -> Self {
        Visibility::new(value.child)
            .visible(!value.offstage)
            .maintain_state(true)
            .into()
    }
}
