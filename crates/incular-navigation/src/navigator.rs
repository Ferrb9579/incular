use std::{cell::RefCell, fmt, rc::Rc};

use serde_json::Value;

use super::{
    NAVIGATOR_SNAPSHOT_FORMAT_VERSION, NavigatorSnapshot, Page, RestorableRoute, Route, RouteId,
    RoutePresentation, RouteScopeKey, RouteSettings, RouteTransition,
};

/// Observable lifecycle emitted by a [`Navigator`].
///
/// Events carry declarative route values, never internal arena handles. An
/// observer is notified after the stack mutation has completed, so it may
/// safely inspect the navigator or schedule application work from its
/// callback.
#[derive(Clone)]
pub enum NavigationEvent {
    /// A route was appended to the stack.
    Pushed { route: Route },
    /// A route was removed from the stack.
    Popped { route: Route },
    /// The active route was replaced in place.
    Replaced { previous: Route, route: Route },
    /// The route presented by this navigator changed.
    ActiveRouteChanged {
        previous: Option<Route>,
        current: Option<Route>,
    },
}

/// Decision supplied by a route-pop guard.
///
/// A guard can return [`Self::Deny`] while it presents an unsaved-work dialog;
/// the application may perform a later explicit pop after the user confirms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopDecision {
    /// Permit the pending stack mutation.
    Allow,
    /// Keep the route in place.
    Deny,
}

/// Result of an attempted guarded pop.
#[derive(Clone)]
pub enum PopResult {
    /// The top route was removed.
    Popped(Box<Route>),
    /// A guard declined the mutation or changed the stack during its callback.
    Blocked,
    /// The navigator was empty.
    Empty,
}

struct NavigatorObserverEntry {
    callback: Box<dyn Fn(NavigationEvent)>,
}

/// Lifetime token returned by [`Navigator::observe`].
///
/// Keep the token for as long as events are wanted. Dropping the final token
/// automatically unregisters its callback without exposing implementation
/// identities in the public API.
#[must_use]
pub struct NavigatorObserver {
    entry: Rc<NavigatorObserverEntry>,
}
impl NavigatorObserver {
    /// Returns whether this retained subscription is still alive.
    #[must_use]
    pub fn is_active(&self) -> bool {
        Rc::strong_count(&self.entry) != 0
    }
}
impl fmt::Debug for NavigatorObserver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NavigatorObserver")
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
struct NavigatorState {
    next_id: u64,
    revision: u64,
    routes: Vec<Route>,
    restorable_routes: Vec<Option<RestorableRoute>>,
    route_scope_cleanup: Option<RouteScopeCleanup>,
    pop_guard: Option<PopGuard>,
    observers: Vec<std::rc::Weak<NavigatorObserverEntry>>,
}

type RouteScopeCleanup = Rc<dyn Fn(&RouteScopeKey)>;
type PopGuard = Rc<dyn Fn(&Route) -> PopDecision>;

/// Cloneable stack navigator. Applications may drive it imperatively or
/// reconcile it from declarative page state.
#[derive(Clone, Default)]
pub struct Navigator {
    state: Rc<RefCell<NavigatorState>>,
}
impl Navigator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Registers a lifecycle callback for this navigator.
    ///
    /// The returned token owns the subscription. This avoids globally
    /// registered callbacks and allows a child navigator to disappear without
    /// explicit observer bookkeeping.
    pub fn observe(&self, callback: impl Fn(NavigationEvent) + 'static) -> NavigatorObserver {
        let entry = Rc::new(NavigatorObserverEntry {
            callback: Box::new(callback),
        });
        self.state
            .borrow_mut()
            .observers
            .push(Rc::downgrade(&entry));
        NavigatorObserver { entry }
    }

    fn notify(&self, event: NavigationEvent) {
        let observers = {
            let mut state = self.state.borrow_mut();
            state.observers.retain(|entry| entry.strong_count() != 0);
            state
                .observers
                .iter()
                .filter_map(std::rc::Weak::upgrade)
                .collect::<Vec<_>>()
        };
        for observer in observers {
            (observer.callback)(event.clone());
        }
    }

    fn notify_active_route(&self, previous: Option<Route>, current: Option<Route>) {
        if previous.as_ref().map(|route| route.id) != current.as_ref().map(|route| route.id) {
            self.notify(NavigationEvent::ActiveRouteChanged { previous, current });
        }
    }

    /// Installs a guard consulted before an explicit [`Self::pop`] or
    /// [`Self::replace`]. It is useful for unsaved-work confirmation flows.
    pub fn set_pop_guard(&self, guard: impl Fn(&Route) -> PopDecision + 'static) {
        self.state.borrow_mut().pop_guard = Some(Rc::new(guard));
    }

    /// Removes the current pop guard.
    pub fn clear_pop_guard(&self) {
        self.state.borrow_mut().pop_guard = None;
    }
    pub fn push(&self, mut route: Route) -> RouteId {
        let (id, previous, route) = {
            let mut state = self.state.borrow_mut();
            let previous = state.routes.last().cloned();
            state.next_id = state.next_id.wrapping_add(1).max(1);
            route.id = RouteId(state.next_id);
            let id = route.id;
            state.routes.push(route.clone());
            state.restorable_routes.push(None);
            state.revision = state.revision.wrapping_add(1);
            (id, previous, route)
        };
        self.notify(NavigationEvent::Pushed {
            route: route.clone(),
        });
        self.notify_active_route(previous, Some(route));
        id
    }
    pub fn push_page(&self, page: Page) -> RouteId {
        self.push(page.into())
    }

    pub(crate) fn push_registered_restorable(&self, page: Page, route: RestorableRoute) -> RouteId {
        let (id, previous, pushed) = {
            let mut state = self.state.borrow_mut();
            let previous = state.routes.last().cloned();
            state.next_id = state.next_id.wrapping_add(1).max(1);
            let id = RouteId(state.next_id);
            let pushed = Route {
                id,
                settings: RouteSettings::new(page.name.clone()),
                name: page.name,
                child: page.child,
                transition: RouteTransition::None,
                presentation: RoutePresentation::default(),
            };
            state.routes.push(pushed.clone());
            state.restorable_routes.push(Some(route));
            state.revision = state.revision.wrapping_add(1);
            (id, previous, pushed)
        };
        self.notify(NavigationEvent::Pushed {
            route: pushed.clone(),
        });
        self.notify_active_route(previous, Some(pushed));
        id
    }

    pub(crate) fn replace_with_restored_routes(&self, routes: Vec<(Page, RestorableRoute)>) {
        debug_assert!(!routes.is_empty());
        let mut state = self.state.borrow_mut();
        let mut next_routes = Vec::with_capacity(routes.len());
        let mut next_restorable = Vec::with_capacity(routes.len());
        for (page, route) in routes {
            state.next_id = state.next_id.wrapping_add(1).max(1);
            next_routes.push(Route {
                id: RouteId(state.next_id),
                settings: RouteSettings::new(page.name.clone()),
                name: page.name,
                child: page.child,
                transition: RouteTransition::None,
                presentation: RoutePresentation::default(),
            });
            next_restorable.push(Some(route));
        }
        state.routes = next_routes;
        state.restorable_routes = next_restorable;
        state.revision = state.revision.wrapping_add(1);
    }

    pub(crate) fn replace_with_fallback(&self, page: Page) {
        let mut state = self.state.borrow_mut();
        state.next_id = state.next_id.wrapping_add(1).max(1);
        state.routes = vec![Route {
            id: RouteId(state.next_id),
            settings: RouteSettings::new(page.name.clone()),
            name: page.name,
            child: page.child,
            transition: RouteTransition::None,
            presentation: RoutePresentation::default(),
        }];
        state.restorable_routes = vec![None];
        state.revision = state.revision.wrapping_add(1);
    }

    /// Installs a bridge that removes a route-specific restoration scope after
    /// a registered route is explicitly and permanently popped.
    ///
    /// The navigation crate does not own persistence storage, so applications
    /// or the runtime can wire this small callback to their restoration manager
    /// without creating a dependency cycle. The callback receives only a
    /// stable [`RouteScopeKey`], never a [`RouteId`] or live widget.
    pub fn set_route_scope_cleanup(&self, cleanup: impl Fn(&RouteScopeKey) + 'static) {
        self.state.borrow_mut().route_scope_cleanup = Some(Rc::new(cleanup));
    }

    /// Removes the optional route-scope cleanup bridge.
    pub fn clear_route_scope_cleanup(&self) {
        self.state.borrow_mut().route_scope_cleanup = None;
    }

    /// Returns the versioned persistence payload for the longest fully
    /// restorable stack prefix.
    ///
    /// Ordinary routes and all entries above the first ordinary route are
    /// intentionally omitted. This prevents a transient dialog, overlay, or
    /// unregistered page from being resurrected as an invalid partial stack at
    /// the next launch.
    #[must_use]
    pub fn restoration_snapshot(&self) -> NavigatorSnapshot {
        let state = self.state.borrow();
        let routes = state
            .restorable_routes
            .iter()
            .take_while(|route| route.is_some())
            .flatten()
            .cloned()
            .collect::<Vec<_>>();
        NavigatorSnapshot {
            format_version: NAVIGATOR_SNAPSHOT_FORMAT_VERSION,
            active_route: (!routes.is_empty()).then_some(routes.len() - 1),
            routes,
        }
    }

    /// Returns the restorable metadata for the current live route, if it was
    /// pushed through the route registry's restorable navigation API.
    #[must_use]
    pub fn current_restorable_route(&self) -> Option<RestorableRoute> {
        self.state
            .borrow()
            .restorable_routes
            .last()
            .cloned()
            .flatten()
    }

    /// Replaces the serializable application state of the current registered
    /// route. Callers persist the fresh [`Self::restoration_snapshot`] through
    /// their opt-in runtime restoration value after this mutation.
    ///
    /// This does not affect the live widget directly. Applications rebuild or
    /// update their page state normally, while only the declarative JSON value
    /// becomes available to the next process session.
    pub fn set_current_restorable_state(&self, value: Value) -> bool {
        let mut state = self.state.borrow_mut();
        let Some(Some(route)) = state.restorable_routes.last_mut() else {
            return false;
        };
        if route.state == value {
            return false;
        }
        route.state = value;
        state.revision = state.revision.wrapping_add(1);
        true
    }

    /// Reconciles the stack to declarative pages. Names identify retained
    /// route positions, preserving their IDs while replacing child widgets.
    pub fn set_pages(&self, pages: impl IntoIterator<Item = Page>) {
        let mut state = self.state.borrow_mut();
        let mut previous = std::mem::take(&mut state.routes)
            .into_iter()
            .zip(std::mem::take(&mut state.restorable_routes))
            .collect::<Vec<_>>();
        let mut next = Vec::new();
        let mut next_restorable = Vec::new();
        for page in pages {
            if let Some(index) = previous
                .iter()
                .position(|(route, _)| route.name == page.name)
            {
                let (mut route, restorable) = previous.remove(index);
                route.child = page.child;
                next.push(route);
                next_restorable.push(restorable);
            } else {
                state.next_id = state.next_id.wrapping_add(1).max(1);
                next.push(Route {
                    id: RouteId(state.next_id),
                    settings: RouteSettings::new(page.name.clone()),
                    name: page.name,
                    child: page.child,
                    transition: RouteTransition::None,
                    presentation: RoutePresentation::default(),
                });
                next_restorable.push(None);
            }
        }
        state.routes = next;
        state.restorable_routes = next_restorable;
        state.revision = state.revision.wrapping_add(1);
    }
    /// Attempts a guarded pop. Unlike [`Self::pop`], this distinguishes an
    /// empty navigator from a blocked operation. If a guard mutates the stack,
    /// its changes remain and this outer operation returns `Blocked`.
    pub fn maybe_pop(&self) -> PopResult {
        let (candidate, guard, revision) = {
            let state = self.state.borrow();
            (
                state.routes.last().cloned(),
                state.pop_guard.clone(),
                state.revision,
            )
        };
        let Some(candidate) = candidate else {
            return PopResult::Empty;
        };
        if guard.is_some_and(|guard| guard(&candidate) == PopDecision::Deny) {
            return PopResult::Blocked;
        }
        let (route, cleanup) = {
            let mut state = self.state.borrow_mut();
            if state.revision != revision
                || state.routes.last().map(|route| route.id) != Some(candidate.id)
            {
                return PopResult::Blocked;
            }
            let route = state
                .routes
                .pop()
                .expect("a non-empty guarded navigator must remain non-empty");
            let restorable = state.restorable_routes.pop().flatten();
            state.revision = state.revision.wrapping_add(1);
            let cleanup = restorable
                .as_ref()
                .filter(|route| route.removes_scope_on_pop())
                .and_then(|route| route.scope_key.clone())
                .zip(state.route_scope_cleanup.clone());
            (route, cleanup)
        };
        if let Some((scope_key, cleanup)) = cleanup {
            cleanup(&scope_key);
        }
        let current = self.current();
        self.notify(NavigationEvent::Popped {
            route: route.clone(),
        });
        self.notify_active_route(Some(route.clone()), current);
        PopResult::Popped(Box::new(route))
    }
    pub fn pop(&self) -> Option<Route> {
        match self.maybe_pop() {
            PopResult::Popped(route) => Some(*route),
            PopResult::Blocked | PopResult::Empty => None,
        }
    }
    /// Replaces the guarded top route. Returns `None` without replacing when
    /// a guard denies the operation or mutates the stack during its callback.
    pub fn replace(&self, mut route: Route) -> Option<Route> {
        let (candidate, guard, revision) = {
            let state = self.state.borrow();
            (
                state.routes.last().cloned(),
                state.pop_guard.clone(),
                state.revision,
            )
        };
        let Some(candidate) = candidate else {
            self.push(route);
            return None;
        };
        if guard.is_some_and(|guard| guard(&candidate) == PopDecision::Deny) {
            return None;
        }
        let (previous, route, cleanup) = {
            let mut state = self.state.borrow_mut();
            if state.revision != revision
                || state.routes.last().map(|route| route.id) != Some(candidate.id)
            {
                return None;
            }
            let previous = state
                .routes
                .pop()
                .expect("a non-empty guarded navigator must remain non-empty");
            let restorable = state.restorable_routes.pop().flatten();
            let cleanup = restorable
                .as_ref()
                .filter(|restorable| restorable.removes_scope_on_pop())
                .and_then(|restorable| restorable.scope_key.clone())
                .zip(state.route_scope_cleanup.clone());
            state.next_id = state.next_id.wrapping_add(1).max(1);
            route.id = RouteId(state.next_id);
            state.routes.push(route.clone());
            state.restorable_routes.push(None);
            state.revision = state.revision.wrapping_add(1);
            (previous, route, cleanup)
        };
        if let Some((scope_key, cleanup)) = cleanup {
            cleanup(&scope_key);
        }
        self.notify(NavigationEvent::Replaced {
            previous: previous.clone(),
            route: route.clone(),
        });
        self.notify_active_route(Some(previous.clone()), Some(route));
        Some(previous)
    }
    #[must_use]
    pub fn current(&self) -> Option<Route> {
        self.state.borrow().routes.last().cloned()
    }
    #[must_use]
    pub fn routes(&self) -> Vec<Route> {
        self.state.borrow().routes.clone()
    }

    /// Monotonic stack revision. Applications that bridge navigator state to
    /// a `Signal` can compare this value without exposing retained internals.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.state.borrow().revision
    }
    #[must_use]
    pub fn can_pop(&self) -> bool {
        self.state.borrow().routes.len() > 1
    }
}

struct BackDispatcherState {
    navigator: Navigator,
    children: Vec<std::rc::Weak<RefCell<BackDispatcherState>>>,
    active_child: Option<std::rc::Weak<RefCell<BackDispatcherState>>>,
}

/// Result of routing one platform back action through nested navigators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackDispatchReport {
    /// `true` when exactly one navigator accepted the action.
    pub handled: bool,
    /// Depth of the navigator that handled or blocked the action. Root is 0.
    pub depth: usize,
    /// `true` when a pop guard blocked the outer operation (including when
    /// the guard itself changed the stack).
    pub blocked: bool,
}
impl BackDispatchReport {
    const fn unhandled(depth: usize) -> Self {
        Self {
            handled: false,
            depth,
            blocked: false,
        }
    }
}

/// Coordinates one navigator and its nested navigator descendants for a
/// platform back action.
///
/// Each dispatcher owns no routes itself: a child navigator remains entirely
/// independent. Applications attach children where their route hierarchy
/// creates them, then mark the focused child active. Back travels through that
/// active branch first and only reaches an ancestor when the child cannot pop.
#[derive(Clone)]
pub struct BackDispatcher {
    state: Rc<RefCell<BackDispatcherState>>,
}
impl BackDispatcher {
    /// Creates a dispatcher for one independent navigator stack.
    #[must_use]
    pub fn new(navigator: Navigator) -> Self {
        Self {
            state: Rc::new(RefCell::new(BackDispatcherState {
                navigator,
                children: Vec::new(),
                active_child: None,
            })),
        }
    }

    /// Returns the independently owned navigator served by this dispatcher.
    #[must_use]
    pub fn navigator(&self) -> Navigator {
        self.state.borrow().navigator.clone()
    }

    /// Attaches a nested navigator dispatcher. Reattaching the same child is
    /// harmless; dispatching remains deterministic in attachment order.
    pub fn attach_child(&self, child: &BackDispatcher) {
        if Rc::ptr_eq(&self.state, &child.state) {
            return;
        }
        let mut state = self.state.borrow_mut();
        state
            .children
            .retain(|candidate| candidate.strong_count() != 0);
        if !state
            .children
            .iter()
            .any(|candidate| candidate.ptr_eq(&Rc::downgrade(&child.state)))
        {
            state.children.push(Rc::downgrade(&child.state));
        }
    }

    /// Marks an attached child as the focused branch for subsequent back
    /// actions. A detached child is ignored rather than causing a panic.
    pub fn set_active_child(&self, child: Option<&BackDispatcher>) {
        let mut state = self.state.borrow_mut();
        state
            .children
            .retain(|candidate| candidate.strong_count() != 0);
        state.active_child = child.and_then(|child| {
            state
                .children
                .iter()
                .find(|candidate| candidate.ptr_eq(&Rc::downgrade(&child.state)))
                .cloned()
        });
    }

    /// Removes a child from this dispatcher. The child's navigator remains
    /// valid; it simply no longer participates in this ancestor's back route.
    pub fn detach_child(&self, child: &BackDispatcher) {
        let target = Rc::downgrade(&child.state);
        let mut state = self.state.borrow_mut();
        state
            .children
            .retain(|candidate| !candidate.ptr_eq(&target));
        if state
            .active_child
            .as_ref()
            .is_some_and(|candidate| candidate.ptr_eq(&target))
        {
            state.active_child = None;
        }
    }

    /// Dispatches a single back action from the focused deepest navigator
    /// outward. A successful result always mutates at most one stack.
    #[must_use]
    pub fn dispatch_back(&self) -> BackDispatchReport {
        self.dispatch_back_at_depth(0)
    }

    fn dispatch_back_at_depth(&self, depth: usize) -> BackDispatchReport {
        let active_child = {
            let mut state = self.state.borrow_mut();
            state
                .children
                .retain(|candidate| candidate.strong_count() != 0);
            state.active_child.as_ref().and_then(std::rc::Weak::upgrade)
        };
        if let Some(child_state) = active_child {
            let child = Self { state: child_state };
            let report = child.dispatch_back_at_depth(depth + 1);
            if report.handled || report.blocked {
                return report;
            }
        }

        let navigator = self.navigator();
        if !navigator.can_pop() {
            return BackDispatchReport::unhandled(depth);
        }
        match navigator.maybe_pop() {
            PopResult::Popped(_) => BackDispatchReport {
                handled: true,
                depth,
                blocked: false,
            },
            PopResult::Blocked => BackDispatchReport {
                handled: true,
                depth,
                blocked: true,
            },
            PopResult::Empty => BackDispatchReport::unhandled(depth),
        }
    }
}
