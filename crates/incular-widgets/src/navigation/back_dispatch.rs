//! Ordered, retained back dispatch.
//!
//! Flutter's `BackButtonDispatcher` gives the newest child priority, then
//! invokes the parent's callback.  The route/navigation crate has a separate
//! dispatcher because it cannot depend on Widgets without a cycle.  This
//! dispatcher is the widget-side lifecycle bridge: applications or the
//! runtime install the navigation crate's `dispatch_back` call as the
//! fallback, while retained widget scopes register ahead of it.

use serde_json::Value;
use std::{
    cell::RefCell,
    collections::HashSet,
    fmt,
    rc::{Rc, Weak},
};

type BackHandler = Rc<dyn Fn() -> BackHandlerResult>;
type PopCanPop = Rc<dyn Fn() -> bool>;
type PopInvoked = Rc<dyn Fn(PopAttempt)>;
type Fallback = Rc<dyn Fn() -> PopAttempt>;

/// The result of one platform back/pop attempt.
///
/// Route results cross the Widgets/navigation boundary as JSON.  A caller
/// that has no result can use `None`; a blocked attempt still invokes every
/// registered pop callback with `did_pop == false`.
#[derive(Clone, Debug, PartialEq)]
pub struct PopAttempt {
    /// Whether a route or nested navigator actually popped.
    pub did_pop: bool,
    /// The result supplied by the popped route, when it is serializable at
    /// the renderer-independent boundary.
    pub result: Option<Value>,
    /// Whether the fallback consumed the platform action.
    pub handled: bool,
    /// Whether a pop guard consumed the action without mutating navigation.
    pub blocked: bool,
}

impl PopAttempt {
    /// Creates an accepted pop attempt.
    #[must_use]
    pub fn popped(result: Option<Value>) -> Self {
        Self {
            did_pop: true,
            result,
            handled: true,
            blocked: false,
        }
    }

    /// Creates a handled attempt that did not mutate a route stack.
    #[must_use]
    pub const fn rejected() -> Self {
        Self {
            did_pop: false,
            result: None,
            handled: true,
            blocked: false,
        }
    }

    /// Creates a guard-blocked attempt.
    #[must_use]
    pub const fn blocked() -> Self {
        Self {
            did_pop: false,
            result: None,
            handled: true,
            blocked: true,
        }
    }

    /// Creates an unhandled attempt.
    #[must_use]
    pub const fn unhandled() -> Self {
        Self {
            did_pop: false,
            result: None,
            handled: false,
            blocked: false,
        }
    }
}

/// The result returned by a retained back callback.
#[derive(Clone, Debug, PartialEq)]
pub struct BackHandlerResult {
    /// Whether the callback consumed the back action.
    pub handled: bool,
    /// Whether the callback consumed it because a pop was blocked.
    pub blocked: bool,
    /// Optional pop result to propagate to surrounding scopes.
    pub pop: Option<PopAttempt>,
}

impl BackHandlerResult {
    /// An unhandled callback result.
    #[must_use]
    pub const fn unhandled() -> Self {
        Self {
            handled: false,
            blocked: false,
            pop: None,
        }
    }

    /// A handled callback result without a route result.
    #[must_use]
    pub const fn handled() -> Self {
        Self {
            handled: true,
            blocked: false,
            pop: None,
        }
    }

    /// Converts a pop attempt into a dispatch result.
    #[must_use]
    pub fn from_pop(pop: PopAttempt) -> Self {
        Self {
            handled: pop.handled,
            blocked: pop.blocked,
            pop: Some(pop),
        }
    }
}

struct HandlerEntry {
    id: u64,
    callback: BackHandler,
}

struct ChildEntry {
    id: u64,
    child: Weak<RefCell<DispatcherState>>,
}

struct PopGuardEntry {
    id: u64,
    can_pop: PopCanPop,
    callback: PopInvoked,
}

struct DispatcherState {
    next_id: u64,
    handlers: Vec<HandlerEntry>,
    children: Vec<ChildEntry>,
    pop_guards: Vec<PopGuardEntry>,
    fallback: Option<Fallback>,
    parent: Option<Weak<RefCell<DispatcherState>>>,
    parent_link: Option<BackRegistration>,
}

impl Default for DispatcherState {
    fn default() -> Self {
        Self {
            next_id: 1,
            handlers: Vec::new(),
            children: Vec::new(),
            pop_guards: Vec::new(),
            fallback: None,
            parent: None,
            parent_link: None,
        }
    }
}

#[derive(Clone, Copy)]
enum RegistrationKind {
    Handler,
    Child,
    PopGuard,
    Noop,
}

/// Owns a retained back-dispatch registration.
///
/// Keep this token when registering a callback manually.  Widget wrappers
/// retain it in their layout-builder closure and therefore automatically
/// unregister when their retained element is removed.
pub struct BackRegistration {
    state: Weak<RefCell<DispatcherState>>,
    kind: RegistrationKind,
    id: u64,
}

impl fmt::Debug for BackRegistration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackRegistration")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl Drop for BackRegistration {
    fn drop(&mut self) {
        let Some(state) = self.state.upgrade() else {
            return;
        };
        let mut state = state.borrow_mut();
        match self.kind {
            RegistrationKind::Handler => {
                state.handlers.retain(|entry| entry.id != self.id);
            }
            RegistrationKind::Child => {
                state.children.retain(|entry| entry.id != self.id);
            }
            RegistrationKind::PopGuard => {
                state.pop_guards.retain(|entry| entry.id != self.id);
            }
            RegistrationKind::Noop => {}
        }
    }
}

/// Report returned after routing one platform back action.
#[derive(Clone, Debug, PartialEq)]
pub struct BackDispatchReport {
    /// Whether exactly one callback/fallback consumed the action.
    pub handled: bool,
    /// Depth of the dispatcher that handled or blocked the action.  The root
    /// dispatcher is depth zero.
    pub depth: usize,
    /// Whether a pop guard consumed the action without a stack mutation.
    pub blocked: bool,
    /// The pop result propagated through the dispatch chain, when one exists.
    pub pop: Option<PopAttempt>,
}

impl BackDispatchReport {
    fn from_result(result: BackHandlerResult, depth: usize) -> Self {
        Self {
            handled: result.handled,
            depth,
            blocked: result.blocked,
            pop: result.pop,
        }
    }
}

/// Retained, priority-ordered back dispatcher for widget scopes.
#[derive(Clone, Default)]
pub struct BackButtonDispatcher {
    state: Rc<RefCell<DispatcherState>>,
}

impl fmt::Debug for BackButtonDispatcher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.state.borrow();
        formatter
            .debug_struct("BackButtonDispatcher")
            .field("handlers", &state.handlers.len())
            .field("children", &state.children.len())
            .field("pop_guards", &state.pop_guards.len())
            .finish()
    }
}

impl BackButtonDispatcher {
    /// Creates an empty root dispatcher.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a dispatcher whose final fallback is `fallback`.
    #[must_use]
    pub fn with_fallback(fallback: impl Fn() -> PopAttempt + 'static) -> Self {
        let dispatcher = Self::new();
        dispatcher.set_fallback(fallback);
        dispatcher
    }

    /// Installs the final navigation fallback.  The navigation crate's
    /// `BackDispatcher::dispatch_back` can be adapted here without a crate
    /// dependency cycle.
    pub fn set_fallback(&self, fallback: impl Fn() -> PopAttempt + 'static) {
        self.state.borrow_mut().fallback = Some(Rc::new(fallback));
    }

    /// Removes the final navigation fallback.
    pub fn clear_fallback(&self) {
        self.state.borrow_mut().fallback = None;
    }

    /// Returns whether this dispatcher has any registered handler, child, or
    /// pop guard.
    #[must_use]
    pub fn has_callbacks(&self) -> bool {
        let state = self.state.borrow();
        !state.handlers.is_empty() || !state.children.is_empty() || !state.pop_guards.is_empty()
    }

    /// Returns the number of local back handlers.
    #[must_use]
    pub fn handler_count(&self) -> usize {
        self.state.borrow().handlers.len()
    }

    /// Returns the number of attached child dispatchers.
    #[must_use]
    pub fn child_count(&self) -> usize {
        self.state.borrow().children.len()
    }

    /// Returns the number of registered pop guards.
    #[must_use]
    pub fn pop_scope_count(&self) -> usize {
        self.state.borrow().pop_guards.len()
    }

    /// Registers a callback in this dispatcher.  New callbacks run before
    /// older callbacks, matching the newest-priority rule used by nested
    /// Flutter back dispatchers.
    pub fn add_handler(
        &self,
        callback: impl Fn() -> BackHandlerResult + 'static,
    ) -> BackRegistration {
        let callback = Rc::new(callback);
        let (id, state) = {
            let mut state = self.state.borrow_mut();
            let id = next_id(&mut state);
            state.handlers.push(HandlerEntry { id, callback });
            (id, Rc::downgrade(&self.state))
        };
        BackRegistration {
            state,
            kind: RegistrationKind::Handler,
            id,
        }
    }

    /// Registers a Flutter-style boolean back callback.
    pub fn add_callback(&self, callback: impl Fn() -> bool + 'static) -> BackRegistration {
        self.add_handler(move || {
            if callback() {
                BackHandlerResult::handled()
            } else {
                BackHandlerResult::unhandled()
            }
        })
    }

    /// Alias for [`Self::add_callback`] useful to retained scope adapters.
    pub fn register_callback(&self, callback: impl Fn() -> bool + 'static) -> BackRegistration {
        self.add_callback(callback)
    }

    /// Registers a pop guard and its post-attempt callback.
    pub fn register_pop_scope(
        &self,
        can_pop: impl Fn() -> bool + 'static,
        callback: impl Fn(PopAttempt) + 'static,
    ) -> BackRegistration {
        let can_pop = Rc::new(can_pop);
        let callback = Rc::new(callback);
        let (id, state) = {
            let mut state = self.state.borrow_mut();
            let id = next_id(&mut state);
            state.pop_guards.push(PopGuardEntry {
                id,
                can_pop,
                callback,
            });
            (id, Rc::downgrade(&self.state))
        };
        BackRegistration {
            state,
            kind: RegistrationKind::PopGuard,
            id,
        }
    }

    /// Attaches a child dispatcher.  The returned token owns the attachment.
    pub fn attach_child(&self, child: &BackButtonDispatcher) -> BackRegistration {
        if Rc::ptr_eq(&self.state, &child.state) {
            return BackRegistration::noop();
        }
        let (id, state) = {
            let mut state = self.state.borrow_mut();
            state
                .children
                .retain(|entry| entry.child.strong_count() != 0);
            if state.children.iter().any(|entry| {
                entry
                    .child
                    .upgrade()
                    .is_some_and(|candidate| Rc::ptr_eq(&candidate, &child.state))
            }) {
                return BackRegistration::noop();
            }
            let id = next_id(&mut state);
            state.children.push(ChildEntry {
                id,
                child: Rc::downgrade(&child.state),
            });
            (id, Rc::downgrade(&self.state))
        };
        BackRegistration {
            state,
            kind: RegistrationKind::Child,
            id,
        }
    }

    /// Creates and attaches a child dispatcher with a parent-owned lifecycle
    /// link.  The child remains the newest priority branch until another child
    /// calls [`Self::take_priority`].
    #[must_use]
    pub fn create_child(&self) -> Self {
        let child = Self::new();
        child.state.borrow_mut().parent = Some(Rc::downgrade(&self.state));
        let link = self.attach_child(&child);
        child.state.borrow_mut().parent_link = Some(link);
        child
    }

    /// Alias matching the longer Flutter dispatcher terminology.
    #[must_use]
    pub fn create_child_back_button_dispatcher(&self) -> Self {
        self.create_child()
    }

    /// Moves `child` to the newest-priority position.
    pub fn defer_to(&self, child: &BackButtonDispatcher) {
        let target = Rc::downgrade(&child.state);
        let mut state = self.state.borrow_mut();
        if let Some(index) = state.children.iter().position(|entry| {
            entry
                .child
                .upgrade()
                .is_some_and(|candidate| Rc::ptr_eq(&candidate, &child.state))
        }) {
            let entry = state.children.remove(index);
            state.children.push(entry);
        } else if target.strong_count() != 0 {
            let id = next_id(&mut state);
            state.children.push(ChildEntry { id, child: target });
        }
    }

    /// Gives this child priority over its siblings, or clears child priority
    /// when called on a root dispatcher.
    pub fn take_priority(&self) {
        let parent = self.state.borrow().parent.clone();
        if let Some(parent) = parent.and_then(|parent| parent.upgrade()) {
            Self { state: parent }.defer_to(self);
        } else {
            self.state.borrow_mut().children.clear();
        }
    }

    /// Detaches a child without invalidating the child dispatcher itself.
    pub fn detach_child(&self, child: &BackButtonDispatcher) {
        let target = Rc::downgrade(&child.state);
        let mut state = self.state.borrow_mut();
        state.children.retain(|entry| !entry.child.ptr_eq(&target));
    }

    /// Dispatches a back action through children (newest first), then local
    /// callbacks, pop guards, and finally the navigation fallback.
    #[must_use]
    pub fn dispatch_back(&self) -> BackDispatchReport {
        self.dispatch_back_inner(0, &mut HashSet::new(), None)
    }

    /// Alias for [`Self::dispatch_back`] used by platform adapters.
    #[must_use]
    pub fn invoke_callback(&self) -> BackDispatchReport {
        self.dispatch_back()
    }

    /// Dispatches with a one-shot fallback.  This is the integration seam for
    /// callers that keep the navigation fallback outside the widget scope.
    #[must_use]
    pub fn dispatch_back_with_fallback(
        &self,
        fallback: impl Fn() -> PopAttempt + 'static,
    ) -> BackDispatchReport {
        self.dispatch_back_inner(0, &mut HashSet::new(), Some(Rc::new(fallback)))
    }

    fn dispatch_back_inner(
        &self,
        depth: usize,
        visited: &mut HashSet<usize>,
        fallback_override: Option<Fallback>,
    ) -> BackDispatchReport {
        let identity = Rc::as_ptr(&self.state) as usize;
        if !visited.insert(identity) {
            return BackDispatchReport {
                handled: false,
                depth,
                blocked: false,
                pop: None,
            };
        }

        let (children, handlers, guards, fallback) = {
            let mut state = self.state.borrow_mut();
            state
                .children
                .retain(|entry| entry.child.strong_count() != 0);
            let children = state
                .children
                .iter()
                .filter_map(|entry| entry.child.upgrade())
                .collect::<Vec<_>>();
            let handlers = state
                .handlers
                .iter()
                .map(|entry| entry.callback.clone())
                .collect::<Vec<_>>();
            let guards = state
                .pop_guards
                .iter()
                .map(|entry| (entry.can_pop.clone(), entry.callback.clone()))
                .collect::<Vec<_>>();
            (children, handlers, guards, state.fallback.clone())
        };

        for child in children.into_iter().rev() {
            let report = Self { state: child }.dispatch_back_inner(depth + 1, visited, None);
            if report.handled || report.blocked {
                return report;
            }
        }

        for handler in handlers.into_iter().rev() {
            let result = handler();
            if result.handled || result.blocked {
                return BackDispatchReport::from_result(result, depth);
            }
        }

        let fallback = fallback_override.or(fallback);
        dispatch_pop(guards, fallback, depth)
    }
}

impl BackRegistration {
    fn noop() -> Self {
        Self {
            state: Weak::new(),
            kind: RegistrationKind::Noop,
            id: 0,
        }
    }
}

fn next_id(state: &mut DispatcherState) -> u64 {
    let id = state.next_id;
    state.next_id = state.next_id.wrapping_add(1).max(1);
    id
}

fn dispatch_pop(
    guards: Vec<(PopCanPop, PopInvoked)>,
    fallback: Option<Fallback>,
    depth: usize,
) -> BackDispatchReport {
    if guards.is_empty() {
        let Some(fallback) = fallback else {
            return BackDispatchReport {
                handled: false,
                depth,
                blocked: false,
                pop: None,
            };
        };
        let attempt = fallback();
        return BackDispatchReport {
            handled: attempt.handled,
            depth,
            blocked: attempt.blocked,
            pop: Some(attempt),
        };
    }

    let can_pop = guards.iter().all(|(can_pop, _)| can_pop());
    if !can_pop {
        let attempt = PopAttempt::blocked();
        for (_, callback) in &guards {
            callback(attempt.clone());
        }
        return BackDispatchReport {
            handled: true,
            depth,
            blocked: true,
            pop: Some(attempt),
        };
    }

    let attempt = fallback.map_or_else(PopAttempt::unhandled, |fallback| fallback());
    for (_, callback) in &guards {
        callback(attempt.clone());
    }
    BackDispatchReport {
        handled: attempt.handled,
        depth,
        blocked: attempt.blocked,
        pop: Some(attempt),
    }
}
