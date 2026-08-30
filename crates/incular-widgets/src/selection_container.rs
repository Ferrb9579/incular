//! Flutter-shaped selection container widget descriptors.

use crate::{SelectionAreaController, SelectionContainerDelegate, Widget};

/// A retained selection boundary around a widget subtree.
///
/// The boundary owns a delegate/controller pair and is isolated from any
/// nested selection boundary. Use [`SelectionContainer::disabled`] to keep a
/// subtree visible while preventing its selectable children from registering.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectionContainer {
    child: Widget,
    delegate: SelectionContainerDelegate,
}

impl SelectionContainer {
    /// Creates an enabled container with a fresh selection registry.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::with_delegate(SelectionContainerDelegate::new(), child)
    }

    /// Creates a container backed by an explicit retained delegate.
    #[must_use]
    pub fn with_delegate(delegate: SelectionContainerDelegate, child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            delegate,
        }
    }

    /// Creates an enabled container backed by an explicit controller.
    #[must_use]
    pub fn with_controller(controller: SelectionAreaController, child: impl Into<Widget>) -> Self {
        Self::with_delegate(
            SelectionContainerDelegate::with_controller(controller),
            child,
        )
    }

    /// Creates a container that deliberately does not register selectable
    /// descendants. This mirrors Flutter's disabled selection boundary.
    #[must_use]
    pub fn disabled(child: impl Into<Widget>) -> Self {
        Self::with_delegate(SelectionContainerDelegate::disabled(), child)
    }

    #[must_use]
    pub fn delegate(&self) -> &SelectionContainerDelegate {
        &self.delegate
    }

    #[must_use]
    pub fn controller(&self) -> SelectionAreaController {
        self.delegate.controller()
    }

    #[must_use]
    pub fn child(&self) -> &Widget {
        &self.child
    }
}

impl From<SelectionContainer> for Widget {
    fn from(value: SelectionContainer) -> Self {
        Widget::selection_container(value.delegate, value.child)
    }
}
