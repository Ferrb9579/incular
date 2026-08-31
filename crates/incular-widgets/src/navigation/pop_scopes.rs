//! Pop scopes, nested navigator handoff, and retained back listeners.

use super::back_dispatch::{
    BackButtonDispatcher, BackDispatchReport, BackHandlerResult, BackRegistration, PopAttempt,
};
use crate::Widget;
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};

type PopCallback = Rc<dyn Fn(bool, Option<Value>)>;
type ResultCallback = Rc<dyn Fn(Option<Value>)>;
type PopHandler = Rc<dyn Fn() -> PopAttempt>;

#[derive(Default)]
struct PopScopeState {
    can_pop: bool,
    callback: Option<PopCallback>,
}

/// Retained state owner for [`PopScope`].
#[derive(Clone, Default)]
pub struct PopScopeController {
    state: Rc<RefCell<PopScopeState>>,
}

impl PopScopeController {
    /// Creates a controller whose route may initially pop.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current pop permission.
    #[must_use]
    pub fn can_pop(&self) -> bool {
        self.state.borrow().can_pop
    }

    /// Updates the pop permission without replacing the retained widget.
    pub fn set_can_pop(&self, can_pop: bool) {
        self.state.borrow_mut().can_pop = can_pop;
    }

    /// Installs a callback receiving the final did-pop bit and JSON result.
    pub fn set_on_pop_invoked_with_result(&self, callback: impl Fn(bool, Option<Value>) + 'static) {
        self.state.borrow_mut().callback = Some(Rc::new(callback));
    }

    /// Installs the compatibility callback that ignores the route result.
    pub fn set_on_pop_invoked(&self, callback: impl Fn(bool) + 'static) {
        self.set_on_pop_invoked_with_result(move |did_pop, _| callback(did_pop));
    }

    /// Invokes the registered callback after a pop attempt.
    pub fn invoke_pop(&self, did_pop: bool, result: Option<Value>) {
        let callback = self.state.borrow().callback.clone();
        if let Some(callback) = callback {
            callback(did_pop, result);
        }
    }

    /// Registers the controller as a pop guard in the supplied dispatcher.
    pub fn register(&self, dispatcher: &BackButtonDispatcher) -> BackRegistration {
        let can_pop = self.clone();
        let callback = self.clone();
        dispatcher.register_pop_scope(
            move || can_pop.can_pop(),
            move |attempt| callback.invoke_pop(attempt.did_pop, attempt.result),
        )
    }
}

/// Reports attempted route pops and can block the route stack before the
/// navigation fallback runs.
#[derive(Clone)]
pub struct PopScope {
    controller: PopScopeController,
    dispatcher: Option<BackButtonDispatcher>,
    child: Widget,
}

impl PopScope {
    /// Creates a pop scope around `child`.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            controller: PopScopeController::new(),
            dispatcher: None,
            child: child.into(),
        }
    }

    /// Reuses an externally owned retained controller.
    #[must_use]
    pub fn with_controller(controller: PopScopeController, child: impl Into<Widget>) -> Self {
        Self {
            controller,
            dispatcher: None,
            child: child.into(),
        }
    }

    /// Sets whether the enclosing route may pop.
    #[must_use]
    pub fn can_pop(self, can_pop: bool) -> Self {
        self.controller.set_can_pop(can_pop);
        self
    }

    /// Installs Flutter's legacy did-pop callback shape.
    #[must_use]
    pub fn on_pop_invoked(self, callback: impl Fn(bool) + 'static) -> Self {
        self.controller.set_on_pop_invoked(callback);
        self
    }

    /// Installs the result-aware pop callback.
    #[must_use]
    pub fn on_pop_invoked_with_result(
        self,
        callback: impl Fn(bool, Option<Value>) + 'static,
    ) -> Self {
        self.controller.set_on_pop_invoked_with_result(callback);
        self
    }

    /// Supplies the dispatcher used by this retained scope.  When omitted,
    /// conversion looks up the nearest ambient `BackButtonDispatcher`.
    #[must_use]
    pub fn dispatcher(mut self, dispatcher: BackButtonDispatcher) -> Self {
        self.dispatcher = Some(dispatcher);
        self
    }

    /// Returns the retained controller.
    #[must_use]
    pub fn controller(&self) -> PopScopeController {
        self.controller.clone()
    }

    /// Registers this scope manually and returns its lifecycle token.
    pub fn register(&self, dispatcher: &BackButtonDispatcher) -> BackRegistration {
        self.controller.register(dispatcher)
    }
}

impl From<PopScope> for Widget {
    fn from(value: PopScope) -> Self {
        let controller = value.controller;
        let explicit_dispatcher = value.dispatcher;
        let child = value.child;
        let registration = Rc::new(RefCell::new(None::<BackRegistration>));
        let registration_state = registration.clone();
        Widget::layout_builder(move |context, _| {
            if registration_state.borrow().is_none() {
                let dispatcher = explicit_dispatcher
                    .clone()
                    .or_else(|| context.depend_on::<BackButtonDispatcher>());
                if let Some(dispatcher) = dispatcher {
                    *registration_state.borrow_mut() = Some(controller.register(&dispatcher));
                }
            }
            child.clone()
        })
    }
}

#[derive(Default)]
struct NavigatorPopHandlerState {
    enabled: bool,
    can_pop: bool,
    on_pop: Option<ResultCallback>,
    pop_handler: Option<PopHandler>,
}

/// Retained state owner for [`NavigatorPopHandler`].
#[derive(Clone)]
pub struct NavigatorPopHandlerController {
    state: Rc<RefCell<NavigatorPopHandlerState>>,
}

impl Default for NavigatorPopHandlerController {
    fn default() -> Self {
        Self {
            state: Rc::new(RefCell::new(NavigatorPopHandlerState {
                enabled: true,
                can_pop: true,
                ..NavigatorPopHandlerState::default()
            })),
        }
    }
}

impl NavigatorPopHandlerController {
    /// Creates an enabled handler that initially claims nested-pop ownership.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether this nested handler participates in back dispatch.
    pub fn set_enabled(&self, enabled: bool) {
        self.state.borrow_mut().enabled = enabled;
    }

    /// Returns whether this handler is enabled.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.state.borrow().enabled
    }

    /// Updates whether the nested navigator currently owns a pop.
    pub fn set_can_pop(&self, can_pop: bool) {
        self.state.borrow_mut().can_pop = can_pop;
    }

    /// Returns the nested navigator's current pop ownership.
    #[must_use]
    pub fn can_pop(&self) -> bool {
        self.state.borrow().can_pop
    }

    /// Installs a callback that receives a nested navigator's pop result.
    pub fn set_on_pop_with_result(&self, callback: impl Fn(Option<Value>) + 'static) {
        self.state.borrow_mut().on_pop = Some(Rc::new(callback));
    }

    /// Installs the compatibility callback that ignores the nested result.
    pub fn set_on_pop(&self, callback: impl Fn() + 'static) {
        self.set_on_pop_with_result(move |_| callback());
    }

    /// Supplies the actual nested pop operation.
    pub fn set_pop_handler(&self, handler: impl Fn() -> PopAttempt + 'static) {
        self.state.borrow_mut().pop_handler = Some(Rc::new(handler));
    }

    /// Uses a nested widget dispatcher as the actual pop operation.
    pub fn set_nested_dispatcher(&self, dispatcher: BackButtonDispatcher) {
        self.set_pop_handler(move || pop_attempt_from_report(dispatcher.dispatch_back()));
    }

    /// Attempts the nested pop.  A handler that claims ownership consumes the
    /// platform action even when its result reports `did_pop == false`.
    #[must_use]
    pub fn handle_back(&self) -> BackHandlerResult {
        let (enabled, can_pop, handler, callback) = {
            let state = self.state.borrow();
            (
                state.enabled,
                state.can_pop,
                state.pop_handler.clone(),
                state.on_pop.clone(),
            )
        };
        if !enabled || !can_pop {
            return BackHandlerResult::unhandled();
        }
        let attempt = handler.map_or_else(PopAttempt::unhandled, |handler| handler());
        if !attempt.handled {
            return BackHandlerResult::unhandled();
        }
        if let Some(callback) = callback {
            callback(attempt.result.clone());
        }
        BackHandlerResult::from_pop(attempt)
    }

    /// Registers this nested handler in a parent dispatcher.
    pub fn register(&self, dispatcher: &BackButtonDispatcher) -> BackRegistration {
        let controller = self.clone();
        dispatcher.add_handler(move || controller.handle_back())
    }
}

/// Gives a nested navigator first refusal of platform back actions.
#[derive(Clone)]
pub struct NavigatorPopHandler {
    controller: NavigatorPopHandlerController,
    dispatcher: Option<BackButtonDispatcher>,
    child: Widget,
}

impl NavigatorPopHandler {
    /// Creates a nested navigator pop handler around `child`.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            controller: NavigatorPopHandlerController::new(),
            dispatcher: None,
            child: child.into(),
        }
    }

    /// Reuses an externally owned nested-handler controller.
    #[must_use]
    pub fn with_controller(
        controller: NavigatorPopHandlerController,
        child: impl Into<Widget>,
    ) -> Self {
        Self {
            controller,
            dispatcher: None,
            child: child.into(),
        }
    }

    /// Enables or disables nested pop ownership.
    #[must_use]
    pub fn enabled(self, enabled: bool) -> Self {
        self.controller.set_enabled(enabled);
        self
    }

    /// Sets the nested navigator's current pop state.
    #[must_use]
    pub fn can_pop(self, can_pop: bool) -> Self {
        self.controller.set_can_pop(can_pop);
        self
    }

    /// Installs the legacy callback shape.
    #[must_use]
    pub fn on_pop(self, callback: impl Fn() + 'static) -> Self {
        self.controller.set_on_pop(callback);
        self
    }

    /// Installs the result-aware nested callback.
    #[must_use]
    pub fn on_pop_with_result(self, callback: impl Fn(Option<Value>) + 'static) -> Self {
        self.controller.set_on_pop_with_result(callback);
        self
    }

    /// Installs the nested widget dispatcher that performs the actual pop.
    #[must_use]
    pub fn nested_dispatcher(self, dispatcher: BackButtonDispatcher) -> Self {
        self.controller.set_nested_dispatcher(dispatcher);
        self
    }

    /// Supplies the parent dispatcher used by this retained handler.  When
    /// omitted, conversion looks up the nearest ambient dispatcher.
    #[must_use]
    pub fn dispatcher(mut self, dispatcher: BackButtonDispatcher) -> Self {
        self.dispatcher = Some(dispatcher);
        self
    }

    /// Returns the retained nested-handler controller.
    #[must_use]
    pub fn controller(&self) -> NavigatorPopHandlerController {
        self.controller.clone()
    }

    /// Registers the handler manually and returns its lifecycle token.
    pub fn register(&self, dispatcher: &BackButtonDispatcher) -> BackRegistration {
        self.controller.register(dispatcher)
    }
}

impl From<NavigatorPopHandler> for Widget {
    fn from(value: NavigatorPopHandler) -> Self {
        let controller = value.controller;
        let explicit_dispatcher = value.dispatcher;
        let child = value.child;
        let registration = Rc::new(RefCell::new(None::<BackRegistration>));
        let registration_state = registration.clone();
        Widget::layout_builder(move |context, _| {
            if registration_state.borrow().is_none() {
                let dispatcher = explicit_dispatcher
                    .clone()
                    .or_else(|| context.depend_on::<BackButtonDispatcher>());
                if let Some(dispatcher) = dispatcher {
                    *registration_state.borrow_mut() = Some(controller.register(&dispatcher));
                }
            }
            child.clone()
        })
    }
}

/// Converts a nested dispatcher report to a pop attempt for an enclosing
/// handler.
fn pop_attempt_from_report(report: BackDispatchReport) -> PopAttempt {
    report.pop.unwrap_or_else(|| {
        if report.blocked {
            PopAttempt::blocked()
        } else if report.handled {
            PopAttempt::rejected()
        } else {
            PopAttempt::unhandled()
        }
    })
}

/// Listens for a platform/system back button at a retained widget boundary.
#[derive(Clone)]
pub struct BackButtonListener {
    callback: Option<Rc<dyn Fn() -> bool>>,
    dispatcher: Option<BackButtonDispatcher>,
    child: Widget,
}

impl BackButtonListener {
    /// Creates a listener around `child`.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            callback: None,
            dispatcher: None,
            child: child.into(),
        }
    }

    /// Installs the callback.  Returning `true` consumes the back action;
    /// returning `false` lets older/parent handlers continue.
    #[must_use]
    pub fn on_back_button_pressed(mut self, callback: impl Fn() -> bool + 'static) -> Self {
        self.callback = Some(Rc::new(callback));
        self
    }

    /// Supplies the dispatcher used by this listener.  When omitted,
    /// conversion looks up the nearest ambient dispatcher.
    #[must_use]
    pub fn dispatcher(mut self, dispatcher: BackButtonDispatcher) -> Self {
        self.dispatcher = Some(dispatcher);
        self
    }

    /// Registers the callback manually and returns its lifecycle token.
    pub fn register(&self, dispatcher: &BackButtonDispatcher) -> Option<BackRegistration> {
        self.callback
            .clone()
            .map(|callback| dispatcher.add_callback(move || callback()))
    }
}

impl From<BackButtonListener> for Widget {
    fn from(value: BackButtonListener) -> Self {
        let callback = value.callback;
        let explicit_dispatcher = value.dispatcher;
        let child = value.child;
        let registration = Rc::new(RefCell::new(None::<BackRegistration>));
        let registration_state = registration.clone();
        Widget::layout_builder(move |context, _| {
            if registration_state.borrow().is_none()
                && let (Some(callback), Some(dispatcher)) = (
                    callback.clone(),
                    explicit_dispatcher
                        .clone()
                        .or_else(|| context.depend_on::<BackButtonDispatcher>()),
                )
            {
                *registration_state.borrow_mut() =
                    Some(dispatcher.add_callback(move || callback()));
            }
            child.clone()
        })
    }
}
