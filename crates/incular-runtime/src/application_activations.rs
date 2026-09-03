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
/// Activations received before any listener exists remain buffered. Once a
/// listener is present, every queued/native event is delivered exactly once to
/// every listener live for that dispatch, in arrival order.
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

fn drain(state: &Rc<RefCell<ActivationState>>) {
    {
        let mut state = state.borrow_mut();
        state
            .listeners
            .retain(|listener| listener.strong_count() != 0);
        if state.dispatching || state.listeners.is_empty() || state.pending.is_empty() {
            return;
        }
        state.dispatching = true;
    }

    loop {
        let Some((activation, listeners)) = ({
            let mut state = state.borrow_mut();
            state
                .listeners
                .retain(|listener| listener.strong_count() != 0);
            if state.listeners.is_empty() {
                state.dispatching = false;
                None
            } else if let Some(activation) = state.pending.pop_front() {
                Some((
                    activation,
                    state
                        .listeners
                        .iter()
                        .filter_map(Weak::upgrade)
                        .collect::<Vec<_>>(),
                ))
            } else {
                state.dispatching = false;
                None
            }
        }) else {
            break;
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
