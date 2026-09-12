use std::{
    any::Any,
    cell::RefCell,
    collections::{HashMap, HashSet},
    fmt,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

/// Delivers one effect callback in isolation: every callback runs even when
/// an earlier one panics. Only the first panic is kept for resuming; later
/// ones are dropped after their callbacks complete.
fn deliver_isolated(pending: &mut Option<Box<dyn Any + Send>>, callback: impl FnOnce()) {
    if let Err(payload) = catch_unwind(AssertUnwindSafe(callback)) {
        pending.get_or_insert(payload);
    }
}

/// Resumes the first isolated panic, if any — unless already unwinding
/// (notably disposal during an unwind), where resuming would abort the
/// process: terminal marks stand and the panic is swallowed by necessity.
fn resume_isolated(pending: Option<Box<dyn Any + Send>>) {
    if let Some(payload) = pending
        && !std::thread::panicking()
    {
        resume_unwind(payload);
    }
}

use serde_json::Value;

use incular_widgets::Widget;

use super::{
    NAVIGATOR_SNAPSHOT_FORMAT_VERSION, NavigatorSnapshot, Page, PageKey, RestorableRoute, Route,
    RouteId, RoutePresentation, RouteScopeKey, RouteSettings, RouteTransition,
};

/// Observable lifecycle emitted by a [`Navigator`].
///
/// Events carry declarative route values, never internal arena handles. An
/// observer is notified after the stack mutation has completed, so it may
/// safely inspect the navigator or schedule application work from its
/// callback. Events are historical nested delivery: each describes its own
/// commit in commit order, so under reentrant navigation a delivered
/// event's current route need not equal the navigator's latest state.
/// Observers needing the latest state read it from the navigator.
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

/// Effects owned by one committed stack mutation.
///
/// Collected while the state borrow is held, dispatched after it ends:
/// scope cleanups first, then route-lifetime endings, then observer events
/// in commit order. Reentrant navigation inside a cleanup, lifetime
/// callback, or observer completes as a nested commit; nested effects
/// dispatch before the outer dispatch continues, so committed transitions
/// are never reordered. Events own route snapshots, so a retired value
/// referenced by an in-flight event drops when the event does — still
/// outside any borrow.
#[derive(Default)]
struct CommitEffects {
    cleanups: Vec<(RouteScopeKey, RouteScopeCleanup)>,
    lifetimes: Vec<RouteLifetime>,
    events: Vec<NavigationEvent>,
}

/// Mounted-route liveness handle.
///
/// Each stack entry owns exactly one lifetime. Keyed reconciliation moves
/// the entry — and its lifetime — so reorder and retained updates preserve
/// identity; permanent removal ends it exactly once. The handle is
/// deliberately neutral: it carries no task, focus, or widget types, so
/// runtime owners can bind their own scopes to it without the navigation
/// crate depending on them. Event snapshots retain [`Route`] values, never
/// this handle, so observing a removed route cannot keep its mounted
/// lifetime alive.
#[derive(Clone)]
pub struct RouteLifetime {
    state: Rc<RefCell<RouteLifetimeState>>,
}

struct RouteLifetimeState {
    ended: bool,
    callbacks: Vec<std::rc::Weak<dyn Fn()>>,
}

type RouteLifetimeCallback = Rc<dyn Fn()>;

/// Keeps one route-lifetime callback registered.
///
/// Dropping the subscription unregisters its callback; the lifetime itself
/// is unaffected and still ends on removal.
#[must_use]
pub struct RouteLifetimeSubscription {
    _callback: Option<RouteLifetimeCallback>,
}

impl RouteLifetime {
    fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(RouteLifetimeState {
                ended: false,
                callbacks: Vec::new(),
            })),
        }
    }

    /// Returns whether the route is still mounted. Once ended, never live again.
    #[must_use]
    pub fn is_live(&self) -> bool {
        !self.state.borrow().ended
    }

    /// Runs `callback` when the route is permanently removed, after the
    /// navigator borrow ends. Callbacks of an already-ended lifetime run
    /// immediately, so a late subscription cannot miss the signal.
    pub fn on_ended(&self, callback: impl Fn() + 'static) -> RouteLifetimeSubscription {
        let callback: RouteLifetimeCallback = Rc::new(callback);
        let ended = {
            let mut state = self.state.borrow_mut();
            if state.ended {
                true
            } else {
                state.callbacks.push(Rc::downgrade(&callback));
                false
            }
        };
        if ended {
            callback();
        }
        RouteLifetimeSubscription {
            _callback: Some(callback),
        }
    }

    /// Ends the lifetime exactly once, invoking removal callbacks outside
    /// any borrow. Terminal commitment precedes delivery: the ended mark is
    /// set before any callback runs, so a panicking callback cannot leave
    /// this lifetime (or a sibling's mandatory cancellation) uncommitted.
    /// Every callback is still attempted in isolation; the first panic
    /// resumes after delivery unless already unwinding. Reentrant removal
    /// completes as a nested commit first; a second `end` is a no-op, so
    /// every removal path shares this call.
    fn end(&self) {
        let callbacks = {
            let mut state = self.state.borrow_mut();
            if state.ended {
                return;
            }
            state.ended = true;
            let live = state
                .callbacks
                .iter()
                .filter_map(|callback| callback.upgrade())
                .collect::<Vec<_>>();
            state.callbacks.clear();
            live
        };
        let mut pending = None;
        for callback in callbacks {
            deliver_isolated(&mut pending, move || callback());
        }
        resume_isolated(pending);
    }
}

impl fmt::Debug for RouteLifetime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RouteLifetime")
            .field("live", &self.is_live())
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for RouteLifetimeSubscription {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RouteLifetimeSubscription")
            .finish_non_exhaustive()
    }
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

/// One mounted route with its restoration and lifetime metadata.
///
/// The entry keeps a route and its metadata together so ordinary and
/// restorable routes can interleave without positional zip bookkeeping.
/// `key` records the declarative page key that claimed the entry, if any;
/// entries pushed imperatively carry none unless their page supplied one.
/// `lifetime` is the entry's mounted identity: keyed reuse moves it with
/// the entry, and permanent removal ends it exactly once through `end`.
#[derive(Clone)]
struct RouteEntry {
    route: Route,
    restorable: Option<RestorableRoute>,
    key: Option<PageKey>,
    lifetime: RouteLifetime,
}

/// A declarative page list rejected before touching navigator state.
///
/// Carries the first duplicated page key. The stack, revision, and
/// observers are unchanged: validation runs before any mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DuplicatePageKey {
    key: PageKey,
}
impl DuplicatePageKey {
    /// Returns the duplicated page key.
    #[must_use]
    pub fn key(&self) -> &PageKey {
        &self.key
    }
}
impl fmt::Display for DuplicatePageKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "duplicate page key in one declarative update: {}",
            self.key
        )
    }
}
impl std::error::Error for DuplicatePageKey {}

#[derive(Default)]
struct NavigatorState {
    next_id: u64,
    revision: u64,
    routes: Vec<RouteEntry>,
    route_scope_cleanup: Option<RouteScopeCleanup>,
    pop_guard: Option<PopGuard>,
    observers: Vec<std::rc::Weak<NavigatorObserverEntry>>,
}

impl Drop for NavigatorState {
    /// Explicit disposal policy: navigator disposal ends every mounted
    /// lifetime exactly once, invoking removal callbacks. No navigator
    /// clone can be live here — any clone keeps this state alive — so
    /// callbacks observe disposal only through their own side effects,
    /// never through the navigator. Callback panics resume when unwinding
    /// is not already in progress; during an existing unwind they are
    /// swallowed by necessity while the terminal marks stand.
    fn drop(&mut self) {
        let lifetimes: Vec<RouteLifetime> = self
            .routes
            .iter()
            .map(|entry| entry.lifetime.clone())
            .collect();
        for lifetime in lifetimes {
            lifetime.end();
        }
    }
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

    /// Notifies observers of one committed event. Each observer runs
    /// isolated (a panicking observer cannot veto later ones); panics are
    /// reported through `pending` for the dispatch to resume afterwards.
    fn notify(&self, event: NavigationEvent, pending: &mut Option<Box<dyn Any + Send>>) {
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
            let event = event.clone();
            deliver_isolated(pending, move || (observer.callback)(event));
        }
    }

    /// Dispatches one commit's effects after its borrow ends: scope
    /// cleanups, then lifetime endings, then observer events. All callbacks
    /// run isolated, so every removed route ends and every mandatory
    /// cancellation fires even when an earlier callback panics; the first
    /// panic resumes after delivery (swallowed only while unwinding).
    /// Reentrant navigation inside any callback completes as a nested
    /// commit first.
    fn dispatch_effects(&self, effects: CommitEffects) {
        let mut pending = None;
        for (scope_key, cleanup) in effects.cleanups {
            deliver_isolated(&mut pending, move || cleanup(&scope_key));
        }
        for lifetime in effects.lifetimes {
            // `end` commits its terminal mark before delivering, and resumes
            // its own first panic here — caught below, so siblings still end.
            deliver_isolated(&mut pending, move || lifetime.end());
        }
        for event in effects.events {
            self.notify(event, &mut pending);
        }
        resume_isolated(pending);
    }

    /// The active-route event for a committed top transition, if the top
    /// route identity changed. Each event describes its own commit, so a
    /// reentrant commit's events follow the outer commit's events without
    /// restating them.
    fn active_transition(
        previous: Option<Route>,
        current: Option<Route>,
    ) -> Option<NavigationEvent> {
        (previous.as_ref().map(|route| route.id) != current.as_ref().map(|route| route.id))
            .then(|| NavigationEvent::ActiveRouteChanged { previous, current })
    }

    /// Installs a guard consulted before an explicit [`Self::pop`] or
    /// [`Self::replace`]. It is useful for unsaved-work confirmation flows.
    /// A replaced guard drops after the borrow ends: guard closures may
    /// own application values whose destructors reenter the navigator.
    pub fn set_pop_guard(&self, guard: impl Fn(&Route) -> PopDecision + 'static) {
        let retired = {
            let mut state = self.state.borrow_mut();
            state.pop_guard.replace(Rc::new(guard))
        };
        drop(retired);
    }

    /// Removes the current pop guard.
    pub fn clear_pop_guard(&self) {
        let retired = { self.state.borrow_mut().pop_guard.take() };
        drop(retired);
    }
    pub fn push(&self, route: Route) -> RouteId {
        self.push_entry(route, None, None)
    }
    /// Pushes a declarative page as a new route lifetime.
    ///
    /// Imperative pushes perform no identity validation: pushing the same
    /// key twice creates two live entries claiming it. A later keyed
    /// reconciliation keeps the topmost claim and drops the rest.
    pub fn push_page(&self, page: Page) -> RouteId {
        let key = page.key.clone();
        self.push_entry(page.into(), None, key)
    }

    fn push_entry(
        &self,
        mut route: Route,
        restorable: Option<RestorableRoute>,
        key: Option<PageKey>,
    ) -> RouteId {
        let (id, previous, route) = {
            let mut state = self.state.borrow_mut();
            let previous = state.routes.last().map(|entry| entry.route.clone());
            state.next_id = state.next_id.wrapping_add(1).max(1);
            route.id = RouteId(state.next_id);
            let id = route.id;
            state.routes.push(RouteEntry {
                route: route.clone(),
                restorable,
                key,
                lifetime: RouteLifetime::new(),
            });
            state.revision = state.revision.wrapping_add(1);
            (id, previous, route)
        };
        let mut effects = CommitEffects::default();
        effects.events.push(NavigationEvent::Pushed {
            route: route.clone(),
        });
        effects
            .events
            .extend(Self::active_transition(previous, Some(route)));
        self.dispatch_effects(effects);
        id
    }

    pub(crate) fn push_registered_restorable(&self, page: Page, route: RestorableRoute) -> RouteId {
        let key = page.key.clone();
        let pushed = Route {
            id: RouteId(0),
            settings: RouteSettings::new(page.name.clone()),
            name: page.name,
            child: page.child,
            transition: RouteTransition::None,
            presentation: RoutePresentation::default(),
        };
        self.push_entry(pushed, Some(route), key)
    }

    pub(crate) fn replace_with_restored_routes(&self, routes: Vec<(Page, RestorableRoute)>) {
        debug_assert!(!routes.is_empty());
        // Retired entries drop after the borrow ends: route children may
        // own application values whose destructors reenter the navigator.
        let (previous, current, retired) = {
            let mut state = self.state.borrow_mut();
            let previous = state.routes.last().map(|entry| entry.route.clone());
            let mut next = Vec::with_capacity(routes.len());
            for (page, route) in routes {
                state.next_id = state.next_id.wrapping_add(1).max(1);
                next.push(RouteEntry {
                    route: Route {
                        id: RouteId(state.next_id),
                        settings: RouteSettings::new(page.name.clone()),
                        name: page.name,
                        child: page.child,
                        transition: RouteTransition::None,
                        presentation: RoutePresentation::default(),
                    },
                    restorable: Some(route),
                    key: None,
                    lifetime: RouteLifetime::new(),
                });
            }
            let retired = std::mem::replace(&mut state.routes, next);
            state.revision = state.revision.wrapping_add(1);
            let current = state.routes.last().map(|entry| entry.route.clone());
            (previous, current, retired)
        };
        let mut effects = CommitEffects::default();
        effects
            .lifetimes
            .extend(retired.iter().map(|entry| entry.lifetime.clone()));
        drop(retired);
        effects
            .events
            .extend(Self::active_transition(previous, current));
        self.dispatch_effects(effects);
    }

    pub(crate) fn replace_with_fallback(&self, page: Page) {
        let (previous, current, retired) = {
            let mut state = self.state.borrow_mut();
            let previous = state.routes.last().map(|entry| entry.route.clone());
            state.next_id = state.next_id.wrapping_add(1).max(1);
            let id = RouteId(state.next_id);
            let retired = std::mem::replace(
                &mut state.routes,
                vec![RouteEntry {
                    route: Route {
                        id,
                        settings: RouteSettings::new(page.name.clone()),
                        name: page.name,
                        child: page.child,
                        transition: RouteTransition::None,
                        presentation: RoutePresentation::default(),
                    },
                    restorable: None,
                    key: None,
                    lifetime: RouteLifetime::new(),
                }],
            );
            state.revision = state.revision.wrapping_add(1);
            let current = state.routes.last().map(|entry| entry.route.clone());
            (previous, current, retired)
        };
        let mut effects = CommitEffects::default();
        effects
            .lifetimes
            .extend(retired.iter().map(|entry| entry.lifetime.clone()));
        drop(retired);
        effects
            .events
            .extend(Self::active_transition(previous, current));
        self.dispatch_effects(effects);
    }

    /// Installs a bridge that removes a route-specific restoration scope after
    /// a registered route is explicitly and permanently popped.
    ///
    /// The navigation crate does not own persistence storage, so applications
    /// or the runtime can wire this small callback to their restoration manager
    /// without creating a dependency cycle. The callback receives only a
    /// stable [`RouteScopeKey`], never a [`RouteId`] or live widget. A
    /// replaced bridge drops after the borrow ends, like pop guards.
    pub fn set_route_scope_cleanup(&self, cleanup: impl Fn(&RouteScopeKey) + 'static) {
        let retired = {
            let mut state = self.state.borrow_mut();
            state.route_scope_cleanup.replace(Rc::new(cleanup))
        };
        drop(retired);
    }

    /// Removes the optional route-scope cleanup bridge.
    pub fn clear_route_scope_cleanup(&self) {
        let retired = { self.state.borrow_mut().route_scope_cleanup.take() };
        drop(retired);
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
            .routes
            .iter()
            .map(|entry| &entry.restorable)
            .take_while(|route| route.is_some())
            .flatten()
            .cloned()
            .collect::<Vec<_>>();
        NavigatorSnapshot {
            format_version: NAVIGATOR_SNAPSHOT_FORMAT_VERSION,
            active_route: routes.len().checked_sub(1),
            routes,
        }
    }

    /// Returns the restorable metadata for the current live route, if it was
    /// pushed through the route registry's restorable navigation API.
    #[must_use]
    pub fn current_restorable_route(&self) -> Option<RestorableRoute> {
        self.state
            .borrow()
            .routes
            .last()
            .and_then(|entry| entry.restorable.clone())
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
        let Some(entry) = state.routes.last_mut() else {
            return false;
        };
        let Some(route) = entry.restorable.as_mut() else {
            return false;
        };
        if route.state == value {
            return false;
        }
        route.state = value;
        state.revision = state.revision.wrapping_add(1);
        true
    }

    /// Reconciles the stack to declarative pages.
    ///
    /// Only page keys identify retained routes: a keyed page reuses the
    /// live entry carrying that key (preserving its route ID, restoration
    /// metadata, and presentation while replacing the child widget).
    /// Names are routing metadata, never identity, so repeated names with
    /// different keys coexist. Pages without a key carry no identity and
    /// always mount anew.
    ///
    /// The caller's iterator drains fully before any state is borrowed,
    /// so an iterator may inspect or mutate the navigator; those changes
    /// complete first and reconciliation then matches against the mutated
    /// stack. Identity validation runs before any mutation: a duplicated
    /// key rejects the whole update, leaving the stack, revision, and
    /// observers untouched.
    ///
    /// Claim lookup is indexed, not a repeated linear search: each live
    /// entry is visited once to build the table, then each page resolves
    /// in constant time. When several live entries claim one key (only
    /// possible through imperative pushes, which perform no identity
    /// validation), the topmost claim wins as the presented route; the
    /// others are dropped as unclaimed.
    pub fn set_pages(&self, pages: impl IntoIterator<Item = Page>) -> Result<(), DuplicatePageKey> {
        let pages: Vec<Page> = pages.into_iter().collect();
        let mut seen = HashSet::new();
        for page in &pages {
            if let Some(key) = &page.key
                && !seen.insert(key.clone())
            {
                return Err(DuplicatePageKey { key: key.clone() });
            }
        }
        let (previous_top, current_top, retired, retired_children) = {
            let mut state = self.state.borrow_mut();
            let previous_top = state.routes.last().map(|entry| entry.route.clone());
            let mut previous: Vec<Option<RouteEntry>> = std::mem::take(&mut state.routes)
                .into_iter()
                .map(Some)
                .collect();
            let mut by_key: HashMap<PageKey, Vec<usize>> = HashMap::new();
            for (index, entry) in previous.iter().enumerate() {
                if let Some(key) = entry.as_ref().and_then(|entry| entry.key.clone()) {
                    by_key.entry(key).or_default().push(index);
                }
            }
            let mut next = Vec::with_capacity(pages.len());
            // Replaced children retire outside the borrow below: assignment
            // would drop the old child here, where its destructor could
            // reenter the navigator. Top-route snapshots do not protect
            // non-top children.
            let mut retired_children: Vec<Widget> = Vec::new();
            for page in pages {
                // Pop takes the topmost claimant: slots ascend, so the
                // last slot is the most recently pushed entry.
                let claimed = match &page.key {
                    Some(key) => by_key.get_mut(key).and_then(|slots| slots.pop()),
                    None => None,
                };
                if let Some(index) = claimed {
                    let mut entry = previous[index]
                        .take()
                        .expect("a claimed slot holds its entry exactly once");
                    // A renamed page updates the name and its settings
                    // mirror; arguments, scope, restoration metadata,
                    // presentation, and the route ID are preserved.
                    entry.route.name = page.name.clone();
                    entry.route.settings.rename(page.name.clone());
                    retired_children.push(std::mem::replace(&mut entry.route.child, page.child));
                    next.push(entry);
                } else {
                    state.next_id = state.next_id.wrapping_add(1).max(1);
                    next.push(RouteEntry {
                        route: Route {
                            id: RouteId(state.next_id),
                            settings: RouteSettings::new(page.name.clone()),
                            name: page.name,
                            child: page.child,
                            transition: RouteTransition::None,
                            presentation: RoutePresentation::default(),
                        },
                        restorable: None,
                        key: page.key,
                        lifetime: RouteLifetime::new(),
                    });
                }
            }
            // Unmatched entries retire outside the borrow below, without
            // the pop-only scope bridge: declarative reconciliation retains
            // persisted scope data by design.
            let retired: Vec<RouteEntry> = previous.into_iter().flatten().collect();
            state.routes = next;
            state.revision = state.revision.wrapping_add(1);
            let current_top = state.routes.last().map(|entry| entry.route.clone());
            (previous_top, current_top, retired, retired_children)
        };
        let mut effects = CommitEffects::default();
        // Declarative removal ends lifetimes through the same retirement
        // path as pop and replace — while persisted restoration data stays
        // retained by design, independent of lifetime ending.
        effects
            .lifetimes
            .extend(retired.iter().map(|entry| entry.lifetime.clone()));
        drop(retired);
        drop(retired_children);
        effects
            .events
            .extend(Self::active_transition(previous_top, current_top));
        self.dispatch_effects(effects);
        Ok(())
    }
    /// Attempts a guarded pop. Unlike [`Self::pop`], this distinguishes an
    /// empty navigator from a blocked operation. If a guard mutates the stack,
    /// its changes remain and this outer operation returns `Blocked`.
    pub fn maybe_pop(&self) -> PopResult {
        let (candidate, guard, revision) = {
            let state = self.state.borrow();
            (
                state.routes.last().map(|entry| entry.route.clone()),
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
        let (route, lifetime, cleanup) = {
            let mut state = self.state.borrow_mut();
            if state.revision != revision
                || state.routes.last().map(|entry| entry.route.id) != Some(candidate.id)
            {
                return PopResult::Blocked;
            }
            let entry = state
                .routes
                .pop()
                .expect("a non-empty guarded navigator must remain non-empty");
            state.revision = state.revision.wrapping_add(1);
            let cleanup = entry
                .restorable
                .as_ref()
                .filter(|route| route.removes_scope_on_pop())
                .and_then(|route| route.scope_key.clone())
                .zip(state.route_scope_cleanup.clone());
            (entry.route, entry.lifetime, cleanup)
        };
        let current = self.current();
        let mut effects = CommitEffects::default();
        effects.cleanups.extend(cleanup);
        effects.lifetimes.push(lifetime);
        effects.events.push(NavigationEvent::Popped {
            route: route.clone(),
        });
        effects
            .events
            .extend(Self::active_transition(Some(route.clone()), current));
        self.dispatch_effects(effects);
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
                state.routes.last().map(|entry| entry.route.clone()),
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
        let (previous, previous_lifetime, route, cleanup) = {
            let mut state = self.state.borrow_mut();
            if state.revision != revision
                || state.routes.last().map(|entry| entry.route.id) != Some(candidate.id)
            {
                return None;
            }
            let previous = state
                .routes
                .pop()
                .expect("a non-empty guarded navigator must remain non-empty");
            let cleanup = previous
                .restorable
                .as_ref()
                .filter(|restorable| restorable.removes_scope_on_pop())
                .and_then(|restorable| restorable.scope_key.clone())
                .zip(state.route_scope_cleanup.clone());
            state.next_id = state.next_id.wrapping_add(1).max(1);
            route.id = RouteId(state.next_id);
            state.routes.push(RouteEntry {
                route: route.clone(),
                restorable: None,
                key: None,
                lifetime: RouteLifetime::new(),
            });
            state.revision = state.revision.wrapping_add(1);
            (previous.route, previous.lifetime, route, cleanup)
        };
        let mut effects = CommitEffects::default();
        effects.cleanups.extend(cleanup);
        effects.lifetimes.push(previous_lifetime);
        effects.events.push(NavigationEvent::Replaced {
            previous: previous.clone(),
            route: route.clone(),
        });
        effects
            .events
            .extend(Self::active_transition(Some(previous.clone()), Some(route)));
        self.dispatch_effects(effects);
        Some(previous)
    }
    /// Returns the mounted lifetime for a live route id, if present.
    ///
    /// The handle tracks the entry, not the snapshot: keyed reuse keeps it
    /// alive across reorder and retained updates, and permanent removal ends
    /// it. Unknown or removed ids yield `None`.
    #[must_use]
    pub fn lifetime_of(&self, id: RouteId) -> Option<RouteLifetime> {
        self.state
            .borrow()
            .routes
            .iter()
            .find(|entry| entry.route.id == id)
            .map(|entry| entry.lifetime.clone())
    }
    #[must_use]
    pub fn current(&self) -> Option<Route> {
        self.state
            .borrow()
            .routes
            .last()
            .map(|entry| entry.route.clone())
    }
    #[must_use]
    pub fn routes(&self) -> Vec<Route> {
        self.state
            .borrow()
            .routes
            .iter()
            .map(|entry| entry.route.clone())
            .collect()
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

/// Why a child attachment was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackAttachError {
    /// A dispatcher cannot parent itself.
    SelfAttachment,
    /// The child already reaches the parent, so the edge would close a
    /// directed cycle and back dispatch could recurse through it forever.
    Cycle,
}
impl fmt::Display for BackAttachError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SelfAttachment => {
                formatter.write_str("a back dispatcher cannot attach to itself")
            }
            Self::Cycle => formatter.write_str(
                "attaching this child would cycle back dispatch: it already reaches its parent",
            ),
        }
    }
}
impl std::error::Error for BackAttachError {}

/// Coordinates one navigator and its nested navigator descendants for a
/// platform back action.
///
/// The attachment topology is an acyclic graph by design: a child may be
/// shared because dispatch follows a single active chain, but no
/// directed cycle may exist. Each dispatcher owns no routes itself: a
/// child navigator remains entirely independent. Applications attach
/// children where their route hierarchy creates them, then mark the
/// focused child active. Back travels through that active branch first
/// and only reaches an ancestor when the child cannot pop. Because every
/// active edge follows an attachment edge, an acyclic graph makes
/// dispatch descend strictly and always terminate.
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

    /// Attaches a nested navigator dispatcher.
    ///
    /// Reattaching the same child is an idempotent no-op. Attachments
    /// that would cycle back dispatch — self-attachment, or a child that
    /// already reaches this dispatcher — are rejected before any
    /// mutation, leaving children and the active selection unchanged.
    pub fn attach_child(&self, child: &BackDispatcher) -> Result<(), BackAttachError> {
        if Rc::ptr_eq(&self.state, &child.state) {
            return Err(BackAttachError::SelfAttachment);
        }
        if child.reaches(self) {
            return Err(BackAttachError::Cycle);
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
        Ok(())
    }

    /// Whether `target` is reachable from this dispatcher by following
    /// attachment edges. Dead entries are skipped, never traversed.
    fn reaches(&self, target: &BackDispatcher) -> bool {
        let mut visited: HashSet<*const RefCell<BackDispatcherState>> = HashSet::new();
        let mut stack = vec![self.state.clone()];
        while let Some(node) = stack.pop() {
            if Rc::ptr_eq(&node, &target.state) {
                return true;
            }
            if !visited.insert(Rc::as_ptr(&node)) {
                continue;
            }
            let children: Vec<Rc<RefCell<BackDispatcherState>>> = node
                .borrow()
                .children
                .iter()
                .filter_map(std::rc::Weak::upgrade)
                .collect();
            stack.extend(children);
        }
        false
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
