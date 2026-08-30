use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use incular_core::Rect;

/// Shared focus handle usable by controls that are rebuilt often.
///
/// For exclusive traversal use a [`FocusManager`]; calling
/// [`Self::request_focus`] directly is retained for standalone controls.
#[derive(Clone)]
pub struct FocusNode {
    state: Rc<FocusState>,
}
impl PartialEq for FocusNode {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}
impl Eq for FocusNode {}
impl std::fmt::Debug for FocusNode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FocusNode")
            .field("has_focus", &self.has_focus())
            .field("can_request_focus", &self.can_request_focus())
            .finish()
    }
}
struct FocusState {
    focused: Cell<bool>,
    can_request_focus: Cell<bool>,
    skip_traversal: Cell<bool>,
    descendants_are_focusable: Cell<bool>,
    descendants_are_traversable: Cell<bool>,
    rect: Cell<Rect>,
    traversal_order: Cell<Option<f64>>,
}

impl Default for FocusNode {
    fn default() -> Self {
        Self {
            state: Rc::new(FocusState {
                focused: Cell::new(false),
                can_request_focus: Cell::new(true),
                skip_traversal: Cell::new(false),
                descendants_are_focusable: Cell::new(true),
                descendants_are_traversable: Cell::new(true),
                rect: Cell::new(Rect::default()),
                traversal_order: Cell::new(None),
            }),
        }
    }
}
impl FocusNode {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn request_focus(&self) {
        if self.can_request_focus() {
            self.state.focused.set(true);
        }
    }
    pub fn unfocus(&self) {
        self.state.focused.set(false);
    }
    #[must_use]
    pub fn has_focus(&self) -> bool {
        self.state.focused.get()
    }
    #[must_use]
    pub fn has_primary_focus(&self) -> bool {
        self.state.focused.get()
    }
    /// Excludes or includes this node in manager-driven traversal.
    pub fn set_can_request_focus(&self, can_request_focus: bool) {
        self.state.can_request_focus.set(can_request_focus);
        if !can_request_focus {
            self.unfocus();
        }
    }
    #[must_use]
    pub fn can_request_focus(&self) -> bool {
        self.state.can_request_focus.get()
    }
    pub fn set_skip_traversal(&self, skip: bool) {
        self.state.skip_traversal.set(skip);
    }
    #[must_use]
    pub fn skip_traversal(&self) -> bool {
        self.state.skip_traversal.get()
    }
    pub fn set_descendants_are_focusable(&self, focusable: bool) {
        self.state.descendants_are_focusable.set(focusable);
    }
    #[must_use]
    pub fn descendants_are_focusable(&self) -> bool {
        self.state.descendants_are_focusable.get()
    }
    pub fn set_descendants_are_traversable(&self, traversable: bool) {
        self.state.descendants_are_traversable.set(traversable);
    }
    #[must_use]
    pub fn descendants_are_traversable(&self) -> bool {
        self.state.descendants_are_traversable.get()
    }

    /// Supplies the node's current layout bounds to geometry-aware traversal.
    /// Bounds are logical pixels in the same coordinate space as the widget
    /// tree. A node that never receives bounds remains in registration order.
    pub fn set_rect(&self, rect: Rect) {
        self.state.rect.set(rect);
    }

    #[must_use]
    pub fn rect(&self) -> Rect {
        self.state.rect.get()
    }

    /// Sets the explicit numeric order used by [`OrderedTraversalPolicy`].
    /// Equal or absent orders preserve widget registration order.
    pub fn set_traversal_order(&self, order: Option<f64>) {
        self.state
            .traversal_order
            .set(order.filter(|value| value.is_finite()));
    }

    #[must_use]
    pub fn traversal_order(&self) -> Option<f64> {
        self.state.traversal_order.get()
    }
}

/// Strategy for navigating keyboard focus through a collection of focus nodes.
pub trait FocusTraversalPolicy {
    fn find_first_focus(&self, nodes: &[FocusNode]) -> Option<FocusNode>;
    fn find_last_focus(&self, nodes: &[FocusNode]) -> Option<FocusNode>;
    fn next(&self, current: &FocusNode, nodes: &[FocusNode]) -> Option<FocusNode>;
    fn previous(&self, current: &FocusNode, nodes: &[FocusNode]) -> Option<FocusNode>;
}

/// Built-in traversal policies understood by retained widget groups.
///
/// The policy is intentionally a small value rather than a trait object so it
/// can cross rebuild boundaries and be stored in a widget descriptor. Custom
/// consumers can still implement [`FocusTraversalPolicy`] directly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FocusTraversalPolicyKind {
    /// Follow the retained widget/registration order.
    #[default]
    WidgetOrder,
    /// Sort by the top edge and then the leading edge of each node.
    ReadingOrder,
    /// Sort by [`FocusNode::traversal_order`], with registration order as the
    /// stable tie breaker.
    Ordered,
}

fn traversable(nodes: &[FocusNode]) -> Vec<FocusNode> {
    nodes
        .iter()
        .filter(|node| node.can_request_focus() && !node.skip_traversal())
        .cloned()
        .collect()
}

fn reading_order(mut nodes: Vec<FocusNode>) -> Vec<FocusNode> {
    // Keep rows together even when controls have slightly different heights.
    // The tolerance is deliberately relative to the row height, which makes
    // this work for both compact desktop controls and large touch targets.
    nodes.sort_by(|left, right| {
        let left_rect = left.rect();
        let right_rect = right.rect();
        let row_tolerance = left_rect
            .size
            .height
            .max(right_rect.size.height)
            .mul_add(0.5, 1.0);
        let vertical = left_rect.origin.y - right_rect.origin.y;
        if vertical.abs() > row_tolerance {
            vertical
                .partial_cmp(&0.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        } else {
            left_rect
                .origin
                .x
                .partial_cmp(&right_rect.origin.x)
                .unwrap_or(std::cmp::Ordering::Equal)
        }
    });
    nodes
}

fn explicit_order(mut nodes: Vec<FocusNode>) -> Vec<FocusNode> {
    nodes.sort_by(|left, right| {
        left.traversal_order()
            .unwrap_or(f64::INFINITY)
            .partial_cmp(&right.traversal_order().unwrap_or(f64::INFINITY))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    nodes
}

fn policy_nodes(policy: FocusTraversalPolicyKind, nodes: &[FocusNode]) -> Vec<FocusNode> {
    let nodes = traversable(nodes);
    match policy {
        FocusTraversalPolicyKind::WidgetOrder => nodes,
        FocusTraversalPolicyKind::ReadingOrder => reading_order(nodes),
        FocusTraversalPolicyKind::Ordered => explicit_order(nodes),
    }
}

/// Traversal policy following logical widget structure order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WidgetOrderTraversalPolicy;

impl FocusTraversalPolicy for WidgetOrderTraversalPolicy {
    fn find_first_focus(&self, nodes: &[FocusNode]) -> Option<FocusNode> {
        traversable(nodes).into_iter().next()
    }

    fn find_last_focus(&self, nodes: &[FocusNode]) -> Option<FocusNode> {
        traversable(nodes).into_iter().next_back()
    }

    fn next(&self, current: &FocusNode, nodes: &[FocusNode]) -> Option<FocusNode> {
        let traversable = traversable(nodes);
        if let Some(pos) = traversable.iter().position(|n| n == current) {
            traversable.get(pos + 1).cloned()
        } else {
            traversable.into_iter().next()
        }
    }

    fn previous(&self, current: &FocusNode, nodes: &[FocusNode]) -> Option<FocusNode> {
        let traversable = traversable(nodes);
        if let Some(pos) = traversable.iter().position(|n| n == current) {
            if pos > 0 {
                traversable.get(pos - 1).cloned()
            } else {
                None
            }
        } else {
            traversable.into_iter().next_back()
        }
    }
}

/// Traversal policy following natural reading order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReadingOrderTraversalPolicy;

impl FocusTraversalPolicy for ReadingOrderTraversalPolicy {
    fn find_first_focus(&self, nodes: &[FocusNode]) -> Option<FocusNode> {
        reading_order(traversable(nodes)).into_iter().next()
    }

    fn find_last_focus(&self, nodes: &[FocusNode]) -> Option<FocusNode> {
        reading_order(traversable(nodes)).into_iter().next_back()
    }

    fn next(&self, current: &FocusNode, nodes: &[FocusNode]) -> Option<FocusNode> {
        WidgetOrderTraversalPolicy.next(current, &reading_order(traversable(nodes)))
    }

    fn previous(&self, current: &FocusNode, nodes: &[FocusNode]) -> Option<FocusNode> {
        WidgetOrderTraversalPolicy.previous(current, &reading_order(traversable(nodes)))
    }
}

/// Traversal policy using explicit numeric ordering.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OrderedTraversalPolicy;

impl FocusTraversalPolicy for OrderedTraversalPolicy {
    fn find_first_focus(&self, nodes: &[FocusNode]) -> Option<FocusNode> {
        explicit_order(traversable(nodes)).into_iter().next()
    }

    fn find_last_focus(&self, nodes: &[FocusNode]) -> Option<FocusNode> {
        explicit_order(traversable(nodes)).into_iter().next_back()
    }

    fn next(&self, current: &FocusNode, nodes: &[FocusNode]) -> Option<FocusNode> {
        WidgetOrderTraversalPolicy.next(current, &explicit_order(traversable(nodes)))
    }

    fn previous(&self, current: &FocusNode, nodes: &[FocusNode]) -> Option<FocusNode> {
        WidgetOrderTraversalPolicy.previous(current, &explicit_order(traversable(nodes)))
    }
}

/// Owns a single focus scope.  Register nodes in visual traversal order, then
/// use [`Self::focus_next`] for Tab or Shift+Tab semantics.  The manager is
/// weakly registered, so unmounted/rebuilt controls cannot leak focus entries.
#[derive(Default)]
pub struct FocusManager {
    nodes: Vec<Weak<FocusState>>,
    policy: FocusTraversalPolicyKind,
}
impl FocusManager {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_policy(policy: FocusTraversalPolicyKind) -> Self {
        Self {
            nodes: Vec::new(),
            policy,
        }
    }

    pub fn set_policy(&mut self, policy: FocusTraversalPolicyKind) {
        self.policy = policy;
    }

    #[must_use]
    pub const fn policy(&self) -> FocusTraversalPolicyKind {
        self.policy
    }
    pub fn register(&mut self, node: &FocusNode) {
        self.prune();
        if !self.nodes.iter().any(|known| {
            known
                .upgrade()
                .is_some_and(|known| Rc::ptr_eq(&known, &node.state))
        }) {
            self.nodes.push(Rc::downgrade(&node.state));
        }
    }
    pub fn unregister(&mut self, node: &FocusNode) {
        self.nodes.retain(|known| {
            known
                .upgrade()
                .is_some_and(|known| !Rc::ptr_eq(&known, &node.state))
        });
        node.unfocus();
    }
    /// Gives this node exclusive focus within the scope.
    #[must_use]
    pub fn request_focus(&mut self, node: &FocusNode) -> bool {
        self.register(node);
        if !node.can_request_focus() {
            return false;
        }
        for known in self.nodes.iter().filter_map(Weak::upgrade) {
            known.focused.set(false);
        }
        node.state.focused.set(true);
        true
    }
    pub fn clear_focus(&mut self) {
        for node in self.nodes.iter().filter_map(Weak::upgrade) {
            node.focused.set(false);
        }
        self.prune();
    }
    #[must_use]
    pub fn focused(&mut self) -> Option<FocusNode> {
        self.prune();
        self.nodes
            .iter()
            .filter_map(Weak::upgrade)
            .find_map(|state| state.focused.get().then_some(FocusNode { state }))
    }
    /// Traverses to the next focusable node.  `reverse` provides Shift+Tab
    /// behavior and traversal wraps inside this scope.
    #[must_use]
    pub fn focus_next(&mut self, reverse: bool) -> Option<FocusNode> {
        self.prune();
        let nodes: Vec<_> = self
            .nodes
            .iter()
            .filter_map(Weak::upgrade)
            .map(|state| FocusNode { state })
            .collect();
        let nodes = policy_nodes(self.policy, &nodes);
        let first = nodes.first()?.clone();
        let current = nodes.iter().position(FocusNode::has_focus);
        let next = match current {
            Some(index) if reverse => nodes[(index + nodes.len() - 1) % nodes.len()].clone(),
            Some(index) => nodes[(index + 1) % nodes.len()].clone(),
            None if reverse => nodes.last()?.clone(),
            None => first,
        };
        for node in self.nodes.iter().filter_map(Weak::upgrade) {
            node.focused.set(false);
        }
        next.request_focus();
        Some(next)
    }
    #[must_use]
    pub fn registered_count(&mut self) -> usize {
        self.prune();
        self.nodes.len()
    }
    fn prune(&mut self) {
        self.nodes.retain(|node| node.strong_count() > 0);
    }
}

/// A retained focus-scope handle.
///
/// `FocusScopeNode` is the Rust-native equivalent of a scoped focus owner. It
/// owns a [`FocusManager`] behind interior mutability so a scope can be kept
/// outside rebuildable widget descriptors and shared by controls that need to
/// request focus. It intentionally does not participate in a Dart-style
/// Element hierarchy.
#[derive(Clone)]
pub struct FocusScopeNode {
    state: Rc<FocusScopeState>,
}

/// Owns one live subscription to a [`FocusScopeNode`] change stream.
///
/// The token is intentionally lightweight and single-threaded, matching the
/// retained UI model. Dropping it removes the callback from future delivery;
/// the scope prunes the corresponding weak entry on its next notification.
pub struct FocusScopeSubscription {
    _entry: Rc<FocusScopeObserverEntry>,
}

struct FocusScopeObserverEntry {
    callback: Rc<dyn Fn()>,
}

struct FocusScopeState {
    manager: RefCell<FocusManager>,
    last_focused: RefCell<Option<FocusNode>>,
    parent: RefCell<Option<Weak<FocusScopeState>>>,
    observers: RefCell<Vec<Weak<FocusScopeObserverEntry>>>,
    revision: Cell<u64>,
}

impl FocusScopeState {
    fn bump_revision(&self) {
        self.revision.set(self.revision.get().wrapping_add(1));
        let callbacks = {
            let mut observers = self.observers.borrow_mut();
            observers.retain(|observer| observer.strong_count() != 0);
            observers
                .iter()
                .filter_map(Weak::upgrade)
                .map(|observer| observer.callback.clone())
                .collect::<Vec<_>>()
        };
        for callback in callbacks {
            callback();
        }
    }
}

impl Default for FocusScopeNode {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for FocusScopeNode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut manager = self.state.manager.borrow_mut();
        formatter
            .debug_struct("FocusScopeNode")
            .field("registered_count", &manager.registered_count())
            .field("has_focus", &manager.focused().is_some())
            .finish()
    }
}

impl FocusScopeNode {
    /// Creates an empty retained focus scope.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Rc::new(FocusScopeState {
                manager: RefCell::new(FocusManager::new()),
                last_focused: RefCell::new(None),
                parent: RefCell::new(None),
                observers: RefCell::new(Vec::new()),
                revision: Cell::new(0),
            }),
        }
    }

    /// Creates a scope nested below `parent`.
    #[must_use]
    pub fn nested(parent: &Self) -> Self {
        let scope = Self::new();
        scope.set_parent(Some(parent));
        scope
    }

    /// Sets or clears this scope's parent relationship.
    pub fn set_parent(&self, parent: Option<&Self>) {
        let parent = parent.filter(|scope| !Rc::ptr_eq(&scope.state, &self.state));
        *self.state.parent.borrow_mut() = parent.map(|scope| Rc::downgrade(&scope.state));
        self.state.bump_revision();
    }

    /// Subscribes to focus registration and selection changes.
    ///
    /// Runtime builders use this to turn retained focus changes into ordinary
    /// element invalidation. Consumers outside a runtime may use the same
    /// stream for status indicators or accessibility adapters.
    #[must_use]
    pub fn observe(&self, callback: impl Fn() + 'static) -> FocusScopeSubscription {
        let entry = Rc::new(FocusScopeObserverEntry {
            callback: Rc::new(callback),
        });
        self.state
            .observers
            .borrow_mut()
            .push(Rc::downgrade(&entry));
        FocusScopeSubscription { _entry: entry }
    }

    /// Monotonic state revision, useful for diagnostics and non-runtime
    /// consumers that need to cheaply detect a focus change.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.state.revision.get()
    }

    /// Registers a focus node in this scope.
    pub fn register(&self, node: &FocusNode) {
        self.state.manager.borrow_mut().register(node);
        self.state.bump_revision();
    }

    /// Sets the traversal policy used by this retained scope.
    pub fn set_policy(&self, policy: FocusTraversalPolicyKind) {
        self.state.manager.borrow_mut().set_policy(policy);
        self.state.bump_revision();
    }

    #[must_use]
    pub fn policy(&self) -> FocusTraversalPolicyKind {
        self.state.manager.borrow().policy()
    }

    /// Removes a focus node from this scope and clears its focus.
    pub fn unregister(&self, node: &FocusNode) {
        self.state.manager.borrow_mut().unregister(node);
        if self
            .state
            .last_focused
            .borrow()
            .as_ref()
            .is_some_and(|focused| focused == node)
        {
            self.state.last_focused.borrow_mut().take();
        }
        self.state.bump_revision();
    }

    /// Gives a node exclusive focus in this scope.
    #[must_use]
    pub fn request_focus(&self, node: &FocusNode) -> bool {
        if !node.can_request_focus() {
            return false;
        }
        let parent = self
            .state
            .parent
            .borrow()
            .as_ref()
            .and_then(Weak::upgrade)
            .map(|state| Self { state });
        if let Some(parent) = &parent {
            let mut parent_manager = parent.state.manager.borrow_mut();
            let parent_focused = parent_manager.focused();
            if parent_focused.is_some() {
                *parent.state.last_focused.borrow_mut() = parent_focused;
                parent_manager.clear_focus();
            }
        }
        let mut manager = self.state.manager.borrow_mut();
        let previous = manager.focused();
        let requested = manager.request_focus(node);
        if requested {
            *self.state.last_focused.borrow_mut() = previous;
            drop(manager);
            if let Some(parent) = &parent {
                parent.state.bump_revision();
            }
            self.state.bump_revision();
        }
        requested
    }

    /// Clears the current focus while retaining it for [`Self::restore_focus`].
    pub fn clear_focus(&self) {
        let mut manager = self.state.manager.borrow_mut();
        *self.state.last_focused.borrow_mut() = manager.focused();
        manager.clear_focus();
        drop(manager);
        self.state.bump_revision();
    }

    /// Clears this scope's focus. This is an alias matching common focus
    /// controller terminology.
    pub fn unfocus(&self) {
        self.clear_focus();
    }

    /// Returns the currently focused node in this scope.
    #[must_use]
    pub fn focused(&self) -> Option<FocusNode> {
        self.state.manager.borrow_mut().focused()
    }

    /// Moves focus to the next/previous registered node.
    #[must_use]
    pub fn focus_next(&self, reverse: bool) -> Option<FocusNode> {
        let mut manager = self.state.manager.borrow_mut();
        let previous = manager.focused();
        let next = manager.focus_next(reverse);
        if next.is_some() {
            *self.state.last_focused.borrow_mut() = previous;
            drop(manager);
            self.state.bump_revision();
        }
        next
    }

    /// Restores the last focus that was active before this scope was cleared
    /// or moved. Returns `false` when the remembered node is no longer
    /// requestable.
    #[must_use]
    pub fn restore_focus(&self) -> bool {
        let Some(node) = self.state.last_focused.borrow().clone() else {
            return false;
        };
        let mut manager = self.state.manager.borrow_mut();
        let restored = manager.request_focus(&node);
        if restored {
            *self.state.last_focused.borrow_mut() = Some(node);
            drop(manager);
            self.state.bump_revision();
        }
        restored
    }

    /// Restores the focused child remembered by this scope's parent.
    #[must_use]
    pub fn restore_parent_focus(&self) -> bool {
        self.state
            .parent
            .borrow()
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|parent| {
                let scope = Self { state: parent };
                scope.restore_focus()
            })
    }

    /// Number of live nodes registered in this scope.
    #[must_use]
    pub fn registered_count(&self) -> usize {
        self.state.manager.borrow_mut().registered_count()
    }
}
