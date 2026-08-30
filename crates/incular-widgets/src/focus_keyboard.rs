//! Focus, focus traversal, and raw keyboard listener widgets.

use std::rc::Rc;

use incular_core::KeyboardEvent;
#[allow(unused_imports)]
pub use incular_gestures::{
    Action, ActionResult, Actions, Command, CommandId, FocusManager, FocusNode, FocusScopeNode,
    FocusScopeSubscription, FocusTraversalPolicy, FocusTraversalPolicyKind, Intent,
    LogicalShortcutKey, OrderedTraversalPolicy, ReadingOrderTraversalPolicy, ShortcutKey,
    ShortcutTrigger, Shortcuts, WidgetOrderTraversalPolicy,
};

use crate::{GestureDetector, Widget, WidgetKind};

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
        let child = value
            .child
            .unwrap_or_else(|| crate::SizedBox::shrink().into());
        // Focus is a retained runtime concern, not a paint-only wrapper. Keep
        // the node on a gesture callback record so WidgetTree's existing
        // focus traversal and keyboard dispatch can observe it.
        let node = value
            .node
            .or_else(|| (value.autofocus || !value.can_request_focus).then(FocusNode::new));
        let Some(node) = node else {
            return child;
        };
        if !value.can_request_focus {
            node.set_can_request_focus(false);
        }
        GestureDetector::new(child)
            .focus_node(node)
            .autofocus(value.autofocus)
            .into()
    }
}

/// A widget that establishes a scoped focus tree traversal domain.
#[derive(Clone, Default)]
pub struct FocusScope {
    autofocus: bool,
    policy: FocusTraversalPolicyKind,
    child: Option<Widget>,
}

impl FocusScope {
    /// Creates a new FocusScope.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            autofocus: false,
            policy: FocusTraversalPolicyKind::WidgetOrder,
            child: Some(child.into()),
        }
    }

    /// Automatically requests focus for its primary descendant on mount.
    #[must_use]
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }

    /// Selects the traversal policy for this scope's descendants.
    #[must_use]
    pub fn policy(mut self, policy: FocusTraversalPolicyKind) -> Self {
        self.policy = policy;
        self
    }

    #[must_use]
    pub fn reading_order(self) -> Self {
        self.policy(FocusTraversalPolicyKind::ReadingOrder)
    }

    #[must_use]
    pub fn widget_order(self) -> Self {
        self.policy(FocusTraversalPolicyKind::WidgetOrder)
    }

    #[must_use]
    pub fn ordered(self) -> Self {
        self.policy(FocusTraversalPolicyKind::Ordered)
    }
}

impl From<FocusScope> for Widget {
    fn from(value: FocusScope) -> Self {
        value
            .child
            .unwrap_or_else(|| crate::SizedBox::shrink().into())
            .with_focus_traversal_policy(value.policy)
            .with_focus_scope_autofocus(value.autofocus)
    }
}

/// A widget that listens for keyboard events routed from platform input.
#[derive(Clone, Default)]
pub struct KeyboardListener {
    on_key: Option<Rc<dyn Fn(KeyboardEvent) -> bool>>,
    on_key_down: Option<Rc<dyn Fn(KeyboardEvent)>>,
    on_key_repeat: Option<Rc<dyn Fn(KeyboardEvent)>>,
    on_key_up: Option<Rc<dyn Fn(KeyboardEvent)>>,
    shortcut_dispatch: Option<Rc<dyn Fn(KeyboardEvent) -> bool>>,
    focus_node: Option<FocusNode>,
    autofocus: bool,
    include_semantics: bool,
    child: Option<Widget>,
}

impl KeyboardListener {
    /// Creates a KeyboardListener wrapping a child widget.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            on_key: None,
            on_key_down: None,
            on_key_repeat: None,
            on_key_up: None,
            shortcut_dispatch: None,
            focus_node: None,
            autofocus: false,
            include_semantics: true,
            child: Some(child.into()),
        }
    }

    /// Installs an event handler for the complete key stream. Returning
    /// `true` consumes the event; returning `false` lets a parent shortcut or
    /// text editor continue handling it.
    #[must_use]
    pub fn on_key(mut self, callback: impl Fn(KeyboardEvent) -> bool + 'static) -> Self {
        self.on_key = Some(Rc::new(callback));
        self
    }

    /// Alias for [`Self::on_key`] using the platform-neutral event naming.
    #[must_use]
    pub fn on_key_event(self, callback: impl Fn(KeyboardEvent) -> bool + 'static) -> Self {
        self.on_key(callback)
    }

    /// Sets the key-down event handler.
    #[must_use]
    pub fn on_key_down(mut self, callback: impl Fn(KeyboardEvent) + 'static) -> Self {
        self.on_key_down = Some(Rc::new(callback));
        self
    }

    /// Sets the auto-repeat key-down handler. This is called only when
    /// [`KeyboardEvent::repeat`] is true.
    #[must_use]
    pub fn on_key_repeat(mut self, callback: impl Fn(KeyboardEvent) + 'static) -> Self {
        self.on_key_repeat = Some(Rc::new(callback));
        self
    }

    /// Sets the key-up event handler.
    #[must_use]
    pub fn on_key_up(mut self, callback: impl Fn(KeyboardEvent) + 'static) -> Self {
        self.on_key_up = Some(Rc::new(callback));
        self
    }

    /// Connects this listener to a typed shortcut/action scope.
    ///
    /// The registries remain ordinary Rust values owned by the application;
    /// the retained widget stores only a type-erased event dispatcher. This
    /// keeps the runtime independent from every command enum while allowing
    /// real platform key events to reach [`Shortcuts::handle_actions`].
    #[must_use]
    pub fn with_shortcuts<C: CommandId>(
        mut self,
        shortcuts: Rc<Shortcuts<C>>,
        actions: Rc<Actions<C>>,
    ) -> Self {
        self.shortcut_dispatch = Some(Rc::new(move |event| {
            shortcuts.handle_actions(event, &actions)
        }));
        self
    }

    /// Uses an existing focus node for this listener.
    #[must_use]
    pub fn focus_node(mut self, node: FocusNode) -> Self {
        self.focus_node = Some(node);
        self
    }

    /// Alias matching the focus-node builder used by [`Focus`].
    #[must_use]
    pub fn node(self, node: FocusNode) -> Self {
        self.focus_node(node)
    }

    /// Requests the configured focus node when this listener is mounted.
    #[must_use]
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }

    /// Controls whether this listener contributes its keyboard affordance to
    /// the semantic tree.
    #[must_use]
    pub fn include_semantics(mut self, include: bool) -> Self {
        self.include_semantics = include;
        self
    }

    /// Returns the configured focus node, if any.
    #[must_use]
    pub fn configured_focus_node(&self) -> Option<FocusNode> {
        self.focus_node.clone()
    }

    /// Returns whether autofocus is enabled.
    #[must_use]
    pub const fn is_autofocus(&self) -> bool {
        self.autofocus
    }

    /// Returns whether semantics are included.
    #[must_use]
    pub const fn includes_semantics(&self) -> bool {
        self.include_semantics
    }

    /// Dispatches one keyboard event through the listener callbacks.
    ///
    /// The generic `on_key` callback gets first refusal. The phase-specific
    /// callback then runs for its matching event, with repeat events exposed
    /// separately in addition to the normal key-down callback.
    #[must_use]
    pub fn handle(&self, event: KeyboardEvent) -> bool {
        self.to_callbacks().handle_keyboard(event)
    }
}

impl From<KeyboardListener> for Widget {
    fn from(value: KeyboardListener) -> Self {
        let callbacks = value.to_callbacks();
        let child = value
            .child
            .unwrap_or_else(|| crate::SizedBox::shrink().into());
        Widget::from_kind(WidgetKind::Gesture {
            behavior: crate::gestures::HitTestBehavior::DeferToChild,
            callbacks: Box::new(callbacks),
            child: Box::new(child),
        })
    }
}

impl KeyboardListener {
    fn to_callbacks(&self) -> incular_gestures::GestureCallbacks {
        incular_gestures::GestureCallbacks {
            on_key: self.on_key.clone(),
            on_key_down: self.on_key_down.clone(),
            on_key_repeat: self.on_key_repeat.clone(),
            on_key_up: self.on_key_up.clone(),
            on_shortcut: self.shortcut_dispatch.clone(),
            focus_node: self.focus_node.clone(),
            autofocus: self.autofocus,
            include_semantics: self.include_semantics,
            ..incular_gestures::GestureCallbacks::default()
        }
    }
}

/// Establishes a focus traversal policy group for its descendants.
#[derive(Clone, Default)]
pub struct FocusTraversalGroup {
    policy: FocusTraversalPolicyKind,
    child: Option<Widget>,
}

impl FocusTraversalGroup {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            policy: FocusTraversalPolicyKind::ReadingOrder,
            child: Some(child.into()),
        }
    }

    /// Selects the policy used for descendants of this group.
    #[must_use]
    pub fn policy(mut self, policy: FocusTraversalPolicyKind) -> Self {
        self.policy = policy;
        self
    }

    #[must_use]
    pub fn reading_order(self) -> Self {
        self.policy(FocusTraversalPolicyKind::ReadingOrder)
    }

    #[must_use]
    pub fn widget_order(self) -> Self {
        self.policy(FocusTraversalPolicyKind::WidgetOrder)
    }

    #[must_use]
    pub fn ordered(self) -> Self {
        self.policy(FocusTraversalPolicyKind::Ordered)
    }
}

impl From<FocusTraversalGroup> for Widget {
    fn from(value: FocusTraversalGroup) -> Self {
        let policy = value.policy;
        value
            .child
            .unwrap_or_else(|| crate::SizedBox::shrink().into())
            .with_focus_traversal_policy(policy)
    }
}

/// Customizes the focus traversal ordering of a widget.
#[derive(Clone, Default)]
pub struct FocusTraversalOrder {
    order: Option<f64>,
    child: Option<Widget>,
}

impl FocusTraversalOrder {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            order: None,
            child: Some(child.into()),
        }
    }

    /// Supplies the explicit numeric order used by an ordered traversal
    /// group. Equal orders retain widget order.
    #[must_use]
    pub fn order(mut self, order: f64) -> Self {
        self.order = order.is_finite().then_some(order);
        self
    }
}

impl From<FocusTraversalOrder> for Widget {
    fn from(value: FocusTraversalOrder) -> Self {
        let order = value.order;
        value
            .child
            .unwrap_or_else(|| crate::SizedBox::shrink().into())
            .with_focus_traversal_order(order)
    }
}

/// Excludes a subtree from receiving focus.
#[derive(Clone, Debug, PartialEq)]
pub struct ExcludeFocus {
    excluding: bool,
    child: Widget,
}

impl ExcludeFocus {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            excluding: true,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn excluding(mut self, excluding: bool) -> Self {
        self.excluding = excluding;
        self
    }
}

impl From<ExcludeFocus> for Widget {
    fn from(value: ExcludeFocus) -> Self {
        value.child.with_excluded_focus(value.excluding)
    }
}

/// Excludes a subtree from tab traversal while still allowing direct programmatic focus.
#[derive(Clone, Debug, PartialEq)]
pub struct ExcludeFocusTraversal {
    excluding: bool,
    child: Widget,
}

impl ExcludeFocusTraversal {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            excluding: true,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn excluding(mut self, excluding: bool) -> Self {
        self.excluding = excluding;
        self
    }
}

impl From<ExcludeFocusTraversal> for Widget {
    fn from(value: ExcludeFocusTraversal) -> Self {
        value.child.with_excluded_focus_traversal(value.excluding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use incular_core::{Code, KeyboardKey, NamedKey};
    use std::cell::Cell;

    fn key_down(code: Code) -> KeyboardEvent {
        KeyboardEvent::key_down(KeyboardKey::Named(NamedKey::Unidentified), code)
    }

    #[test]
    fn keyboard_listener_routes_down_repeat_and_up_events() {
        let down = Rc::new(Cell::new(0));
        let repeat = Rc::new(Cell::new(0));
        let up = Rc::new(Cell::new(0));
        let observed_down = down.clone();
        let observed_repeat = repeat.clone();
        let observed_up = up.clone();
        let listener = KeyboardListener::new(crate::SizedBox::shrink())
            .on_key_down(move |_| observed_down.set(observed_down.get() + 1))
            .on_key_repeat(move |_| observed_repeat.set(observed_repeat.get() + 1))
            .on_key_up(move |_| observed_up.set(observed_up.get() + 1));

        assert!(listener.handle(key_down(Code::KeyA)));
        let mut repeated = key_down(Code::KeyA);
        repeated.repeat = true;
        assert!(listener.handle(repeated));
        assert!(listener.handle(KeyboardEvent::key_up(
            KeyboardKey::Named(NamedKey::Unidentified),
            Code::KeyA,
        )));
        assert_eq!(down.get(), 2);
        assert_eq!(repeat.get(), 1);
        assert_eq!(up.get(), 1);
    }

    #[test]
    fn keyboard_listener_generic_handler_can_consume_event() {
        let phases = Rc::new(Cell::new(0));
        let observed = phases.clone();
        let listener = KeyboardListener::new(crate::SizedBox::shrink()).on_key(move |_| {
            observed.set(observed.get() + 1);
            true
        });
        assert!(listener.handle(key_down(Code::Enter)));
        assert_eq!(phases.get(), 1);
    }

    #[test]
    fn keyboard_listener_keeps_focus_and_semantics_configuration() {
        let node = FocusNode::new();
        let listener = KeyboardListener::new(crate::SizedBox::shrink())
            .focus_node(node.clone())
            .autofocus(true)
            .include_semantics(false);
        assert_eq!(listener.configured_focus_node(), Some(node));
        assert!(listener.is_autofocus());
        assert!(!listener.includes_semantics());
    }
}
