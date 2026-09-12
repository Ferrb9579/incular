//! Application-level activation delivery and router bridging.

use incular_platform::ApplicationActivation;
use incular_widgets::{RouteInformation, RouteInformationProvider};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    fmt,
    rc::{Rc, Weak},
};

/// Retained callback for application activations.
pub type ApplicationActivationListener = Rc<dyn Fn(ApplicationActivation)>;

struct ActivationState {
    pending: VecDeque<ApplicationActivation>,
    listeners: Vec<Weak<dyn Fn(ApplicationActivation)>>,
    dispatching: bool,
}

/// Cloneable application-scoped activation stream.
///
/// Ownership and delivery contract:
///
/// - Identity: one activation is one [`ApplicationActivation`] value. There
///   are no IDs and no deduplication: two equal values are two activations
///   and both are delivered.
/// - Queued: [`publish`](Self::publish) appends to a service-owned FIFO.
///   Activations published before any listener exists (for example before
///   the route bridge is installed) wait there; there is no second queue.
/// - Delivered: exactly once per listener live for that event's snapshot,
///   in arrival order. A listener publishing reentrantly enqueues behind
///   the in-flight event. Delivery is synchronous fan-out, not the runtime
///   request channel: native code pushes through
///   `Application::handle_application_activation`, and the bridge forwards
///   mapped URLs into the route provider, whose own listeners (such as the
///   widgets router) apply them.
/// - Acknowledged/retried/discarded: none. Listeners return nothing to
///   acknowledge, nothing retries, and pending activations are never
///   dropped by the service — they wait until a listener subscribes, even
///   across bridge replacement or window closure, which this
///   application-scoped service never observes.
/// - Router installation and replacement: installing the route bridge
///   subscribes and immediately drains whatever queued earlier.
///   Replacing it (drop, then bridge again) leaves pending activations
///   buffered for the new bridge; the old bridge delivers nothing
///   further once its subscription dies.
#[derive(Clone)]
pub struct ApplicationActivationService {
    state: Rc<RefCell<ActivationState>>,
}

impl Default for ApplicationActivationService {
    fn default() -> Self {
        Self {
            state: Rc::new(RefCell::new(ActivationState {
                pending: VecDeque::new(),
                listeners: Vec::new(),
                dispatching: false,
            })),
        }
    }
}

impl fmt::Debug for ApplicationActivationService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.state.borrow();
        formatter
            .debug_struct("ApplicationActivationService")
            .field("pending", &state.pending.len())
            .field("listeners", &state.listeners.len())
            .finish()
    }
}

impl ApplicationActivationService {
    /// Observes future activations and drains any activations buffered before
    /// the first listener was installed.
    pub fn subscribe(
        &self,
        listener: impl Fn(ApplicationActivation) + 'static,
    ) -> ApplicationActivationSubscription {
        let listener: ApplicationActivationListener = Rc::new(listener);
        self.state
            .borrow_mut()
            .listeners
            .push(Rc::downgrade(&listener));
        drain(&self.state);
        ApplicationActivationSubscription {
            _listener: listener,
        }
    }

    #[doc(hidden)]
    pub fn publish(&self, activation: ApplicationActivation) {
        self.state.borrow_mut().pending.push_back(activation);
        drain(&self.state);
    }

    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.state.borrow().pending.len()
    }
}

/// Owns an in-progress activation drain. The guard is the sole authority
/// releasing the drain: normal completion and unwinding both funnel
/// through its `Drop`, so no delivery path resets the flag itself. On
/// unwind it releases the flag but keeps the queue — undelivered
/// activations stay buffered and resume on the next drain instead of
/// blackholing. The `try_borrow_mut` only fails if a borrow is live at
/// drop time; every borrow here is scoped to queue operations and never
/// spans a callback, so a failed release merely defers to the next
/// guard drop.
struct DispatchGuard {
    state: Rc<RefCell<ActivationState>>,
}
impl DispatchGuard {
    fn acquire(state: &Rc<RefCell<ActivationState>>) -> Option<Self> {
        let mut state_ref = state.borrow_mut();
        state_ref
            .listeners
            .retain(|listener| listener.strong_count() != 0);
        if state_ref.dispatching || state_ref.listeners.is_empty() || state_ref.pending.is_empty() {
            return None;
        }
        state_ref.dispatching = true;
        Some(Self {
            state: state.clone(),
        })
    }
}
impl Drop for DispatchGuard {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.try_borrow_mut() {
            state.dispatching = false;
        }
    }
}

fn drain(state: &Rc<RefCell<ActivationState>>) {
    let Some(_drain) = DispatchGuard::acquire(state) else {
        return;
    };

    loop {
        let next = {
            let mut state = state.borrow_mut();
            state
                .listeners
                .retain(|listener| listener.strong_count() != 0);
            if state.listeners.is_empty() {
                return;
            }
            state.pending.pop_front().map(|activation| {
                (
                    activation,
                    state
                        .listeners
                        .iter()
                        .filter_map(Weak::upgrade)
                        .collect::<Vec<_>>(),
                )
            })
        };
        let Some((activation, listeners)) = next else {
            return;
        };
        for listener in listeners {
            listener(activation.clone());
        }
    }
}

/// Owns one activation listener for as long as it should remain subscribed.
pub struct ApplicationActivationSubscription {
    _listener: ApplicationActivationListener,
}

impl fmt::Debug for ApplicationActivationSubscription {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApplicationActivationSubscription(..)")
    }
}

/// Keeps the deliberate application-activation to Router bridge alive.
///
/// URL interpretation is supplied by the application route layer as `map`;
/// desktop/native code never parses an application route.
pub struct ActivationRouteBridge {
    _subscription: ApplicationActivationSubscription,
    delivered_routes: Rc<Cell<u64>>,
}

impl fmt::Debug for ActivationRouteBridge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActivationRouteBridge")
            .field("delivered_routes", &self.delivered_routes.get())
            .finish()
    }
}

impl ActivationRouteBridge {
    pub(crate) fn new(
        activations: &ApplicationActivationService,
        provider: Rc<dyn RouteInformationProvider>,
        map: impl Fn(&str) -> Option<RouteInformation> + 'static,
    ) -> Self {
        let delivered_routes = Rc::new(Cell::new(0_u64));
        let delivered = delivered_routes.clone();
        let subscription = activations.subscribe(move |activation| {
            for url in activation.urls() {
                if let Some(information) = map(url.as_str()) {
                    provider.set_platform_route_information(information);
                    delivered.set(delivered.get().saturating_add(1));
                }
            }
        });
        Self {
            _subscription: subscription,
            delivered_routes,
        }
    }

    #[must_use]
    pub fn delivered_routes(&self) -> u64 {
        self.delivered_routes.get()
    }
}
