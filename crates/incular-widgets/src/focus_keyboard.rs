//! Focus, focus traversal, and raw keyboard listener widgets.

use std::rc::Rc;

use incular_core::KeyboardEvent;
pub use incular_gestures::{FocusManager, FocusNode};

use crate::Widget;

/// A widget that manages keyboard focus for a subtree.
#[derive(Clone, Default)]
pub struct Focus {
    node: Option<FocusNode>,
    autofocus: bool,
    can_request_focus: bool,
    child: Option<Widget>,
}

impl Focus {
    /// Creates a Focus boundary wrapping a child widget.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            node: None,
            autofocus: false,
            can_request_focus: true,
            child: Some(child.into()),
        }
    }

    /// Attaches an external [`FocusNode`].
    #[must_use]
    pub fn node(mut self, node: FocusNode) -> Self {
        self.node = Some(node);
        self
    }

    /// Automatically requests focus upon initial mount.
    #[must_use]
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }

    /// Controls whether focus can be requested by user interaction or programmatic focus.
    #[must_use]
    pub fn can_request_focus(mut self, can_request: bool) -> Self {
        self.can_request_focus = can_request;
        self
    }
}

impl From<Focus> for Widget {
    fn from(value: Focus) -> Self {
        value
            .child
            .unwrap_or_else(|| crate::SizedBox::shrink().into())
    }
}

/// A widget that establishes a scoped focus tree traversal domain.
#[derive(Clone, Default)]
pub struct FocusScope {
    autofocus: bool,
    child: Option<Widget>,
}

impl FocusScope {
    /// Creates a new FocusScope.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            autofocus: false,
            child: Some(child.into()),
        }
    }

    /// Automatically requests focus for its primary descendant on mount.
    #[must_use]
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }
}

impl From<FocusScope> for Widget {
    fn from(value: FocusScope) -> Self {
        value
            .child
            .unwrap_or_else(|| crate::SizedBox::shrink().into())
    }
}

/// A widget that listens for keyboard events routed from platform input.
#[derive(Clone, Default)]
pub struct KeyboardListener {
    on_key_down: Option<Rc<dyn Fn(KeyboardEvent)>>,
    on_key_up: Option<Rc<dyn Fn(KeyboardEvent)>>,
    child: Option<Widget>,
}

impl KeyboardListener {
    /// Creates a KeyboardListener wrapping a child widget.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            on_key_down: None,
            on_key_up: None,
            child: Some(child.into()),
        }
    }

    /// Sets the key-down event handler.
    #[must_use]
    pub fn on_key_down(mut self, callback: impl Fn(KeyboardEvent) + 'static) -> Self {
        self.on_key_down = Some(Rc::new(callback));
        self
    }

    /// Sets the key-up event handler.
    #[must_use]
    pub fn on_key_up(mut self, callback: impl Fn(KeyboardEvent) + 'static) -> Self {
        self.on_key_up = Some(Rc::new(callback));
        self
    }
}

impl From<KeyboardListener> for Widget {
    fn from(value: KeyboardListener) -> Self {
        value
            .child
            .unwrap_or_else(|| crate::SizedBox::shrink().into())
    }
}
