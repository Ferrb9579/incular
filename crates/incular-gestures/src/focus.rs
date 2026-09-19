use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::{Rc, Weak};

use incular_core::Rect;

use crate::details::PointerDeviceKind;

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
    enabled: Cell<bool>,
    can_request_focus: Cell<bool>,
    skip_traversal: Cell<bool>,
    descendants_are_focusable: Cell<bool>,
    descendants_are_traversable: Cell<bool>,
    rect: Cell<Rect>,
    traversal_order: Cell<Option<f64>>,
    changes: incular_core::reactivity::DependencySource,
    revision: Cell<u64>,
}

/// Owns one live subscription to a [`FocusNode`] change stream.
///
/// The retained focus tree keeps subscriptions weakly in the node, so a
/// widget can be replaced or unmounted without leaving a callback attached to
/// an application-owned node.
pub struct FocusNodeSubscription {
    _subscription: incular_core::reactivity::Subscription,
}

impl FocusState {
    fn notify(&self) {
        self.revision.set(
            self.revision
                .get()
                .checked_add(1)
                .expect("revision exhausted"),
        );
        self.changes.notify();
    }

    fn set_focused_silent(&self, focused: bool) -> bool {
        self.focused.replace(focused) != focused
    }

    fn set_focused(&self, focused: bool) {
        if self.set_focused_silent(focused) {
            self.notify();
        }
    }
}

impl Default for FocusNode {
    fn default() -> Self {
        Self {
            state: Rc::new(FocusState {
                focused: Cell::new(false),
                enabled: Cell::new(true),
                can_request_focus: Cell::new(true),
                skip_traversal: Cell::new(false),
                descendants_are_focusable: Cell::new(true),
                descendants_are_traversable: Cell::new(true),
                rect: Cell::new(Rect::default()),
                traversal_order: Cell::new(None),
                changes: incular_core::reactivity::DependencySource::default(),
                revision: Cell::new(0),
            }),
        }
    }
}
impl FocusNode {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Subscribes to focus and focus-property changes.
    #[must_use]
    pub fn observe(&self, callback: impl Fn() + 'static) -> FocusNodeSubscription {
        FocusNodeSubscription {
            _subscription: self.state.changes.subscribe((), callback),
        }
    }

    /// Monotonic state revision for retained consumers that poll focus state.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.state.revision.get()
    }

    pub fn request_focus(&self) {
        if self.can_request_focus() {
            self.state.set_focused(true);
        }
    }
    pub fn unfocus(&self) {
        self.state.set_focused(false);
    }
    #[must_use]
    pub fn has_focus(&self) -> bool {
        self.state.changes.track();
        self.state.focused.get()
    }
    #[must_use]
    pub fn has_primary_focus(&self) -> bool {
        self.state.changes.track();
        self.state.focused.get()
    }
    /// Excludes or includes this node in manager-driven traversal.
    pub fn set_can_request_focus(&self, can_request_focus: bool) {
        if self.state.can_request_focus.replace(can_request_focus) != can_request_focus {
            if !self.can_request_focus() {
                self.unfocus();
            }
            self.state.notify();
        } else if !can_request_focus {
            self.unfocus();
        }
    }
    #[must_use]
    pub fn can_request_focus(&self) -> bool {
        self.state.changes.track();
        self.state.enabled.get() && self.state.can_request_focus.get()
    }

    /// Enables or disables this hosted focus node without overwriting its
    /// application-controlled `can_request_focus` value.
    pub fn set_enabled(&self, enabled: bool) {
        if self.state.enabled.replace(enabled) != enabled {
            if !enabled {
                self.unfocus();
            }
            self.state.notify();
        }
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.state.enabled.get()
    }
    pub fn set_skip_traversal(&self, skip: bool) {
        if self.state.skip_traversal.replace(skip) != skip {
            self.state.notify();
        }
    }
    #[must_use]
    pub fn skip_traversal(&self) -> bool {
        self.state.skip_traversal.get()
    }
    pub fn set_descendants_are_focusable(&self, focusable: bool) {
        if self.state.descendants_are_focusable.replace(focusable) != focusable {
            self.state.notify();
        }
    }
    #[must_use]
    pub fn descendants_are_focusable(&self) -> bool {
        self.state.descendants_are_focusable.get()
    }
    pub fn set_descendants_are_traversable(&self, traversable: bool) {
        if self.state.descendants_are_traversable.replace(traversable) != traversable {
            self.state.notify();
        }
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

/// The modality used to decide whether focus and hover highlights are
/// visible. Keyboard navigation is traditional; touch interaction is touch
/// mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FocusHighlightMode {
    Touch,
    #[default]
    Traditional,
}

/// Controls how the retained focus system selects its highlight mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FocusHighlightStrategy {
    #[default]
    Automatic,
    AlwaysTouch,
    AlwaysTraditional,
}

struct HighlightModeState {
    strategy: FocusHighlightStrategy,
    last_input_mode: Option<FocusHighlightMode>,
    mode: FocusHighlightMode,
    observers: Vec<Weak<HighlightModeObserverEntry>>,
    pending: VecDeque<HighlightNotification>,
    dispatching: bool,
}

struct HighlightNotification {
    mode: FocusHighlightMode,
    observers: Vec<Weak<HighlightModeObserverEntry>>,
}

impl HighlightModeState {
    fn commit_mode(&mut self, mode: FocusHighlightMode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        self.observers
            .retain(|observer| observer.strong_count() != 0);
        self.pending.push_back(HighlightNotification {
            mode,
            observers: self.observers.clone(),
        });
    }
}

struct HighlightModeObserverEntry {
    active: Cell<bool>,
    callback: Rc<dyn Fn(FocusHighlightMode)>,
}

/// Owns the process-local focus-highlight modality used by retained widgets.
///
/// The object is a cheap handle to the current UI thread's modality state.
/// This mirrors Flutter's process-local `FocusManager` behavior while keeping
/// the native platform event loop outside the gestures crate.
///
/// Changes commit before callbacks run, with no state borrow held during
/// delivery. A callback may query or change this manager: nested changes commit
/// immediately, but their notifications follow the current event in commit
/// order. An event describes its transition; [`Self::mode`] always returns the
/// latest committed state, which another callback may already have changed.
/// Application-triggered sequences must converge.
///
/// A callback panic propagates without reverting committed state. Unfinished
/// notifications are discarded, and later changes can dispatch normally.
#[derive(Clone, Copy, Debug, Default)]
pub struct FocusHighlightManager;

pub struct FocusHighlightSubscription {
    entry: Rc<HighlightModeObserverEntry>,
}

impl Drop for FocusHighlightSubscription {
    fn drop(&mut self) {
        self.entry.active.set(false);
    }
}

thread_local! {
    static HIGHLIGHT_MODE: RefCell<HighlightModeState> = const { RefCell::new(HighlightModeState {
            strategy: FocusHighlightStrategy::Automatic,
            last_input_mode: None,
            mode: FocusHighlightMode::Traditional,
            observers: Vec::new(),
            pending: VecDeque::new(),
            dispatching: false,
        }) };
}

/// Releases dispatch ownership on return or unwind. Pending snapshots contain
/// weak registrations, so aborting delivery cannot extend a callback's lifetime.
struct HighlightDispatchGuard;

impl Drop for HighlightDispatchGuard {
    fn drop(&mut self) {
        let abandoned = HIGHLIGHT_MODE.with(|state| {
            let mut state = state.borrow_mut();
            state.dispatching = false;
            std::mem::take(&mut state.pending)
        });
        drop(abandoned);
    }
}

impl FocusHighlightManager {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    #[must_use]
    pub fn mode(self) -> FocusHighlightMode {
        HIGHLIGHT_MODE.with(|state| state.borrow().mode)
    }

    #[must_use]
    pub fn strategy(self) -> FocusHighlightStrategy {
        HIGHLIGHT_MODE.with(|state| state.borrow().strategy)
    }

    pub fn set_strategy(self, strategy: FocusHighlightStrategy) {
        Self::update(|state| {
            if state.strategy == strategy {
                return;
            }
            state.strategy = strategy;
            let mode = match strategy {
                FocusHighlightStrategy::Automatic => state.last_input_mode.unwrap_or(state.mode),
                FocusHighlightStrategy::AlwaysTouch => FocusHighlightMode::Touch,
                FocusHighlightStrategy::AlwaysTraditional => FocusHighlightMode::Traditional,
            };
            state.commit_mode(mode);
        });
    }

    /// Forces a mode, primarily for deterministic embedders and tests. Future
    /// automatic input updates may change it again.
    pub fn set_mode(self, mode: FocusHighlightMode) {
        Self::update(|state| state.commit_mode(mode));
    }

    /// Records keyboard input, which selects traditional highlights in
    /// automatic mode.
    pub fn note_keyboard_input(self) {
        Self::update(|state| {
            if state.last_input_mode != Some(FocusHighlightMode::Traditional) {
                state.last_input_mode = Some(FocusHighlightMode::Traditional);
                if state.strategy == FocusHighlightStrategy::Automatic {
                    state.commit_mode(FocusHighlightMode::Traditional);
                }
            }
        });
    }

    /// Records pointer input using Flutter's touch/stylus versus traditional
    /// modality split.
    pub fn note_pointer_input(self, kind: PointerDeviceKind) {
        let requires_touch_highlights = matches!(
            kind,
            PointerDeviceKind::Touch
                | PointerDeviceKind::Stylus
                | PointerDeviceKind::InvertedStylus
        );
        if !requires_touch_highlights {
            return;
        }
        Self::update(|state| {
            if state.last_input_mode != Some(FocusHighlightMode::Touch) {
                state.last_input_mode = Some(FocusHighlightMode::Touch);
                if state.strategy == FocusHighlightStrategy::Automatic {
                    state.commit_mode(FocusHighlightMode::Touch);
                }
            }
        });
    }

    /// Observes committed mode transitions in registration order. Dropping the
    /// token cancels delivery, including an event already queued. Registration
    /// during a callback starts with the next subsequently committed change.
    #[must_use]
    pub fn observe(
        self,
        callback: impl Fn(FocusHighlightMode) + 'static,
    ) -> FocusHighlightSubscription {
        let entry = Rc::new(HighlightModeObserverEntry {
            active: Cell::new(true),
            callback: Rc::new(callback),
        });
        HIGHLIGHT_MODE.with(|state| {
            let mut state = state.borrow_mut();
            state
                .observers
                .retain(|observer| observer.strong_count() != 0);
            state.observers.push(Rc::downgrade(&entry));
        });
        FocusHighlightSubscription { entry }
    }

    fn update(update: impl FnOnce(&mut HighlightModeState)) {
        HIGHLIGHT_MODE.with(|state| update(&mut state.borrow_mut()));
        let owns_dispatch = HIGHLIGHT_MODE.with(|state| {
            let mut state = state.borrow_mut();
            if state.dispatching || state.pending.is_empty() {
                return false;
            }
            state.dispatching = true;
            true
        });
        if !owns_dispatch {
            return;
        }

        let _dispatch = HighlightDispatchGuard;
        loop {
            let next = HIGHLIGHT_MODE.with(|state| state.borrow_mut().pending.pop_front());
            let Some(notification) = next else {
                break;
            };
            for observer in notification.observers {
                if let Some(observer) = observer.upgrade()
                    && observer.active.get()
                {
                    (observer.callback)(notification.mode);
                }
            }
        }
    }
}

/// Retained focus/hover state used by `FocusableActionDetector`.
pub struct FocusBehavior {
    node: FocusNode,
    enabled: Cell<bool>,
    hovering: Cell<bool>,
    focused: Cell<bool>,
    can_show_highlight: Cell<bool>,
    focus_highlight_visible: Cell<bool>,
    hover_highlight_visible: Cell<bool>,
    on_focus_change: Option<Rc<dyn Fn(bool)>>,
    on_show_focus_highlight: Option<Rc<dyn Fn(bool)>>,
    on_show_hover_highlight: Option<Rc<dyn Fn(bool)>>,
    focus_subscription: RefCell<Option<FocusNodeSubscription>>,
    highlight_subscription: RefCell<Option<FocusHighlightSubscription>>,
}

impl FocusBehavior {
    #[must_use]
    pub fn new(
        node: FocusNode,
        enabled: bool,
        on_focus_change: Option<Rc<dyn Fn(bool)>>,
        on_show_focus_highlight: Option<Rc<dyn Fn(bool)>>,
        on_show_hover_highlight: Option<Rc<dyn Fn(bool)>>,
    ) -> Rc<Self> {
        node.set_enabled(enabled);
        let can_show_highlight =
            FocusHighlightManager::new().mode() == FocusHighlightMode::Traditional;
        let focused = node.has_focus();
        let behavior = Rc::new(Self {
            focused: Cell::new(focused),
            node,
            enabled: Cell::new(enabled),
            hovering: Cell::new(false),
            can_show_highlight: Cell::new(can_show_highlight),
            focus_highlight_visible: Cell::new(focused && enabled && can_show_highlight),
            hover_highlight_visible: Cell::new(false),
            on_focus_change,
            on_show_focus_highlight,
            on_show_hover_highlight,
            focus_subscription: RefCell::new(None),
            highlight_subscription: RefCell::new(None),
        });
        let weak = Rc::downgrade(&behavior);
        let focus_subscription = behavior.node.observe(move || {
            if let Some(behavior) = weak.upgrade() {
                behavior.handle_focus_change();
            }
        });
        *behavior.focus_subscription.borrow_mut() = Some(focus_subscription);
        let weak = Rc::downgrade(&behavior);
        let highlight_subscription = FocusHighlightManager::new().observe(move |mode| {
            if let Some(behavior) = weak.upgrade() {
                behavior.update_highlight_mode(mode);
            }
        });
        *behavior.highlight_subscription.borrow_mut() = Some(highlight_subscription);
        behavior
    }

    #[must_use]
    pub fn node(&self) -> FocusNode {
        self.node.clone()
    }

    #[must_use]
    pub fn is_hovering(&self) -> bool {
        self.hovering.get()
    }

    #[must_use]
    pub fn is_focused(&self) -> bool {
        self.focused.get()
    }

    pub fn mouse_enter(&self) {
        if !self.hovering.replace(true) {
            self.trigger_highlight_callbacks();
        }
    }

    pub fn mouse_exit(&self) {
        if self.hovering.replace(false) {
            self.trigger_highlight_callbacks();
        }
    }

    /// Carries hover presence over a retained widget update without
    /// replaying an enter callback for an already-hovered region.
    /// Derived visibility syncs silently so the next exit still flips;
    /// replacement callbacks keep learning only subsequent transitions.
    pub fn restore_hovering(&self, hovering: bool) {
        self.hovering.set(hovering);
        self.hover_highlight_visible
            .set(self.hover_highlight_visible());
    }

    fn handle_focus_change(&self) {
        let focused = self.node.has_focus();
        if self.focused.replace(focused) != focused {
            self.trigger_highlight_callbacks();
            if let Some(callback) = &self.on_focus_change {
                callback(focused);
            }
        }
    }

    fn update_highlight_mode(&self, mode: FocusHighlightMode) {
        self.can_show_highlight
            .set(mode == FocusHighlightMode::Traditional);
        self.trigger_highlight_callbacks();
    }

    fn focus_highlight_visible(&self) -> bool {
        self.focused.get() && self.enabled.get() && self.can_show_highlight.get()
    }

    fn hover_highlight_visible(&self) -> bool {
        self.hovering.get() && self.enabled.get() && self.can_show_highlight.get()
    }

    fn trigger_highlight_callbacks(&self) {
        let focus_after = self.focus_highlight_visible();
        let hover_after = self.hover_highlight_visible();
        if self.focus_highlight_visible.replace(focus_after) != focus_after
            && let Some(callback) = &self.on_show_focus_highlight
        {
            callback(focus_after);
        }
        if self.hover_highlight_visible.replace(hover_after) != hover_after
            && let Some(callback) = &self.on_show_hover_highlight
        {
            callback(hover_after);
        }
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

#[derive(Default)]
struct FocusTransition {
    changed: Vec<Rc<FocusState>>,
}

impl FocusTransition {
    fn set(&mut self, state: Rc<FocusState>, focused: bool) {
        if state.set_focused_silent(focused)
            && !self.changed.iter().any(|known| Rc::ptr_eq(known, &state))
        {
            self.changed.push(state);
        }
    }

    fn notify(self) {
        for state in self.changed {
            state.notify();
        }
    }
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
        self.unregister_silent(node).notify();
    }

    fn unregister_silent(&mut self, node: &FocusNode) -> FocusTransition {
        self.nodes.retain(|known| {
            known
                .upgrade()
                .is_some_and(|known| !Rc::ptr_eq(&known, &node.state))
        });
        let mut transition = FocusTransition::default();
        transition.set(node.state.clone(), false);
        transition
    }
    /// Gives this node exclusive focus within the scope.
    #[must_use]
    pub fn request_focus(&mut self, node: &FocusNode) -> bool {
        let (requested, transition) = self.request_focus_silent(node);
        transition.notify();
        requested
    }

    fn request_focus_silent(&mut self, node: &FocusNode) -> (bool, FocusTransition) {
        self.register(node);
        if !node.can_request_focus() {
            return (false, FocusTransition::default());
        }
        let mut transition = FocusTransition::default();
        for known in self.nodes.iter().filter_map(Weak::upgrade) {
            transition.set(known, false);
        }
        transition.set(node.state.clone(), true);
        (true, transition)
    }
    pub fn clear_focus(&mut self) {
        self.clear_focus_silent().notify();
    }

    fn clear_focus_silent(&mut self) -> FocusTransition {
        let mut transition = FocusTransition::default();
        for node in self.nodes.iter().filter_map(Weak::upgrade) {
            transition.set(node, false);
        }
        self.prune();
        transition
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
        let (next, transition) = self.focus_next_silent(reverse);
        transition.notify();
        next
    }

    fn focus_next_silent(&mut self, reverse: bool) -> (Option<FocusNode>, FocusTransition) {
        self.prune();
        let nodes: Vec<_> = self
            .nodes
            .iter()
            .filter_map(Weak::upgrade)
            .map(|state| FocusNode { state })
            .collect();
        let nodes = policy_nodes(self.policy, &nodes);
        let Some(first) = nodes.first().cloned() else {
            return (None, FocusTransition::default());
        };
        let current = nodes.iter().position(|node| node.state.focused.get());
        let next = match current {
            Some(index) if reverse => nodes[(index + nodes.len() - 1) % nodes.len()].clone(),
            Some(index) => nodes[(index + 1) % nodes.len()].clone(),
            None if reverse => nodes.last().cloned().unwrap_or(first),
            None => first,
        };
        let mut transition = FocusTransition::default();
        for node in self.nodes.iter().filter_map(Weak::upgrade) {
            transition.set(node, false);
        }
        transition.set(next.state.clone(), true);
        (Some(next), transition)
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
    _subscription: incular_core::reactivity::Subscription,
}

struct FocusScopeState {
    manager: RefCell<FocusManager>,
    last_focused: RefCell<Option<FocusNode>>,
    parent: RefCell<Option<Weak<FocusScopeState>>>,
    changes: incular_core::reactivity::DependencySource,
    revision: Cell<u64>,
}

impl FocusScopeState {
    fn bump_revision(&self) {
        self.revision.set(
            self.revision
                .get()
                .checked_add(1)
                .expect("revision exhausted"),
        );
        self.changes.notify();
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
                changes: incular_core::reactivity::DependencySource::default(),
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
        FocusScopeSubscription {
            _subscription: self.state.changes.subscribe((), callback),
        }
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
        let transition = self.state.manager.borrow_mut().unregister_silent(node);
        let clear_last = self
            .state
            .last_focused
            .borrow()
            .as_ref()
            .is_some_and(|focused| focused == node);
        if clear_last {
            self.state.last_focused.borrow_mut().take();
        }
        self.state.bump_revision();
        transition.notify();
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
            let parent_transition = parent_focused
                .as_ref()
                .map(|_| parent_manager.clear_focus_silent());
            drop(parent_manager);
            if let Some(parent_focused) = parent_focused {
                *parent.state.last_focused.borrow_mut() = Some(parent_focused);
            }
            let mut manager = self.state.manager.borrow_mut();
            let previous = manager.focused();
            let (requested, transition) = manager.request_focus_silent(node);
            drop(manager);
            if !requested {
                return false;
            }
            *self.state.last_focused.borrow_mut() = previous;
            if parent_transition.is_some() {
                parent.state.bump_revision();
            }
            self.state.bump_revision();
            if let Some(parent_transition) = parent_transition {
                parent_transition.notify();
            }
            transition.notify();
            return true;
        }
        let mut manager = self.state.manager.borrow_mut();
        let previous = manager.focused();
        let (requested, transition) = manager.request_focus_silent(node);
        if requested {
            *self.state.last_focused.borrow_mut() = previous;
            drop(manager);
            self.state.bump_revision();
            transition.notify();
        }
        requested
    }

    /// Clears the current focus while retaining it for [`Self::restore_focus`].
    pub fn clear_focus(&self) {
        let mut manager = self.state.manager.borrow_mut();
        let previous = manager.focused();
        let transition = manager.clear_focus_silent();
        drop(manager);
        *self.state.last_focused.borrow_mut() = previous;
        self.state.bump_revision();
        transition.notify();
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
        let (next, transition) = manager.focus_next_silent(reverse);
        if next.is_some() {
            *self.state.last_focused.borrow_mut() = previous;
            drop(manager);
            self.state.bump_revision();
            transition.notify();
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
        let (restored, transition) = manager.request_focus_silent(&node);
        if restored {
            *self.state.last_focused.borrow_mut() = Some(node);
            drop(manager);
            self.state.bump_revision();
            transition.notify();
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
