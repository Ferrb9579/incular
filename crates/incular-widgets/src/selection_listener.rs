//! Observable selection listener widget descriptor.

use crate::{SelectionListenerNotifier, Widget};

/// Exposes the selection details of the nearest selection boundary to a
/// [`SelectionListenerNotifier`]. Nested selection boundaries remain isolated
/// and do not bubble their selection into this listener.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectionListener {
    selection_notifier: SelectionListenerNotifier,
    child: Widget,
}

impl SelectionListener {
    #[must_use]
    pub fn new(selection_notifier: SelectionListenerNotifier, child: impl Into<Widget>) -> Self {
        Self {
            selection_notifier,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn selection_notifier(&self) -> &SelectionListenerNotifier {
        &self.selection_notifier
    }

    #[must_use]
    pub fn child(&self) -> &Widget {
        &self.child
    }
}

impl From<SelectionListener> for Widget {
    fn from(value: SelectionListener) -> Self {
        Widget::selection_listener(value.selection_notifier, value.child)
    }
}
