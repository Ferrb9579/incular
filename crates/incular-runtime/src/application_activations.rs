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

#[derive(Default)]
struct ActivationState {
    pending: VecDeque<ApplicationActivation>,
    listeners: Vec<Weak<dyn Fn(ApplicationActivation)>>,
}

/// Delivery flag beside the queue. The flag lives in a `Cell`, not in
/// the `RefCell`, so releasing a drain never needs a borrow and cannot
/// fail. Queue borrows stay scoped to enqueue/dequeue/snapshot
/// operations and never span a callback.
#[derive(Default)]
struct ActivationDrain {
    queue: RefCell<ActivationState>,
    active: Cell<bool>,
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
///   in arrival order, while delivery completes without listener failure.
///   A listener publishing reentrantly enqueues behind the in-flight
///   event. Delivery is synchronous fan-out, not the runtime request
///   channel: native code pushes through
///   `Application::handle_application_activation`, and the bridge forwards
///   mapped URLs into the route provider, whose own listeners (such as the
///   widgets router) apply them.
/// - Listener panic: delivery is not transactional. A panicking listener
///   aborts its event's remaining deliveries — that event is not replayed
///   to listeners that missed it, so exactly-once holds only for
///   failure-free delivery. The queue itself is preserved and the drain
///   releases, so a later publish resumes with the queued events first.
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
#[derive(Clone, Default)]
pub struct ApplicationActivationService {
    state: Rc<ActivationDrain>,
}

impl fmt::Debug for ApplicationActivationService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.state.queue.borrow();
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
            .queue
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
        self.state.queue.borrow_mut().pending.push_back(activation);
        drain(&self.state);
    }

    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.state.queue.borrow().pending.len()
    }
}

/// Owns an in-progress activation drain and is the sole authority
/// releasing it. The flag release is infallible (`Cell::set`); the
/// queue itself is left untouched, so undelivered activations stay
/// buffered and resume on the next drain.
struct DispatchGuard {
    shared: Rc<ActivationDrain>,
}
impl DispatchGuard {
    fn acquire(shared: &Rc<ActivationDrain>) -> Option<Self> {
        if shared.active.get() {
            return None;
        }
        {
            let mut queue = shared.queue.borrow_mut();
            queue
                .listeners
                .retain(|listener| listener.strong_count() != 0);
            if queue.listeners.is_empty() || queue.pending.is_empty() {
                return None;
            }
        }
        // No user code runs between the checks above and arming the flag,
        // so no reentrant dispatch can interleave here.
        shared.active.set(true);
        Some(Self {
            shared: shared.clone(),
        })
    }
}
impl Drop for DispatchGuard {
    fn drop(&mut self) {
        self.shared.active.set(false);
    }
}

fn drain(state: &Rc<ActivationDrain>) {
    let Some(_drain) = DispatchGuard::acquire(state) else {
        return;
    };

    loop {
        let next = {
            let mut queue = state.queue.borrow_mut();
            queue
                .listeners
                .retain(|listener| listener.strong_count() != 0);
            if queue.listeners.is_empty() {
                return;
            }
            queue.pending.pop_front().map(|activation| {
                (
                    activation,
                    queue
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
