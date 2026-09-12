//! Route information, router delegates, and application back dispatch.
//!
//! This module is the renderer-independent counterpart of Flutter's
//! `Router` family.  It deliberately keeps route parsing synchronous: Incular
//! builds widgets synchronously, so an asynchronous platform/parser can feed
//! a later [`RouteInformationProvider::set_value`] update after completing its
//! own work.  The retained delegate/provider subscriptions, restoration
//! snapshot, reporting policy, and back transaction are still owned here.

use super::Widget;
use incular_core::{RestorationKey, RestorationScope};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    fmt,
    rc::{Rc, Weak},
};

type RouteParser<T> = Rc<dyn Fn(&RouteInformation) -> Result<T, RouterError>>;
type RouteRestorer<T> = Rc<dyn Fn(&T) -> Result<RouteInformation, RouterError>>;
type SetRoutePath<T> = Rc<dyn Fn(T) -> Result<(), RouterError>>;
type PopRoute = Rc<dyn Fn() -> bool>;
type NavigationNotificationListeners = Rc<RefCell<Vec<Weak<dyn Fn(NavigationNotification)>>>>;

/// Normalizes a named route while retaining its query and fragment suffix.
///
/// Flutter's default route provider treats a route as a URI location but
/// application route tables conventionally use a leading slash.  Keeping the
/// suffix byte-for-byte makes deep links and browser-like providers lossless.
#[must_use]
pub fn normalize_route_location(location: &str) -> String {
    let location = location.trim();
    if location.is_empty() {
        return "/".to_owned();
    }
    let split = location
        .char_indices()
        .find(|(_, character)| matches!(character, '?' | '#'))
        .map(|(index, _)| index)
        .unwrap_or(location.len());
    let (path, suffix) = location.split_at(split);
    let path = if path.is_empty() {
        "/".to_owned()
    } else if path.starts_with('/') {
        path.to_owned()
    } else {
        format!("/{path}")
    };
    format!("{path}{suffix}")
}

/// A location delivered to or reported by a router.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteInformation {
    location: String,
    state: Option<Value>,
}

impl RouteInformation {
    /// Creates route information from a URI-like location.
    #[must_use]
    pub fn new(location: impl AsRef<str>) -> Self {
        Self {
            location: normalize_route_location(location.as_ref()),
            state: None,
        }
    }

    /// Creates route information with a serializable platform state value.
    #[must_use]
    pub fn with_state(location: impl AsRef<str>, state: Option<Value>) -> Self {
        Self {
            location: normalize_route_location(location.as_ref()),
            state,
        }
    }

    /// Returns the normalized route location.
    #[must_use]
    pub fn location(&self) -> &str {
        &self.location
    }

    /// `uri` is an explicit alias for callers that model Flutter's URI API.
    #[must_use]
    pub fn uri(&self) -> &str {
        self.location()
    }

    /// Returns the optional platform state payload.
    #[must_use]
    pub fn state(&self) -> Option<&Value> {
        self.state.as_ref()
    }

    /// Returns a copy with a different optional state payload.
    #[must_use]
    pub fn map_state(mut self, state: Option<Value>) -> Self {
        self.state = state;
        self
    }

    /// Encodes route information for a [`RestorationScope`].
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "location": self.location,
            "state": self.state,
        })
    }

    /// Decodes route information written by [`Self::to_json`].
    pub fn from_json(value: &Value) -> Result<Self, RouterError> {
        let object = value
            .as_object()
            .ok_or_else(|| RouterError::Restoration("route information is not an object".into()))?;
        let location = object
            .get("location")
            .and_then(Value::as_str)
            .ok_or_else(|| RouterError::Restoration("route information has no location".into()))?;
        Ok(Self::with_state(
            location,
            object
                .get("state")
                .cloned()
                .filter(|state| !state.is_null()),
        ))
    }
}

impl Default for RouteInformation {
    fn default() -> Self {
        Self::new("/")
    }
}

impl From<&str> for RouteInformation {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for RouteInformation {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// How a router report should affect platform history.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RouteInformationReportingType {
    /// Do not report this change to the platform.
    #[default]
    None,
    /// Replace the current platform entry without adding history.
    Neglect,
    /// Add a new platform history entry.
    Navigate,
}

/// Error returned by route parsing, delegate updates, or restoration data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouterError {
    /// A provider or platform rejected a route report.
    Provider(String),
    /// A parser rejected route information.
    Parse(String),
    /// A delegate rejected a parsed route configuration.
    Delegate(String),
    /// A restoration value was malformed or unavailable.
    Restoration(String),
    /// A router configuration violated the provider/parser pairing contract.
    InvalidConfiguration(String),
}

impl RouterError {
    #[must_use]
    pub fn message(message: impl Into<String>) -> Self {
        Self::Delegate(message.into())
    }
}

impl fmt::Display for RouterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provider(message) => write!(formatter, "route provider: {message}"),
            Self::Parse(message) => write!(formatter, "route parser: {message}"),
            Self::Delegate(message) => write!(formatter, "router delegate: {message}"),
            Self::Restoration(message) => write!(formatter, "router restoration: {message}"),
            Self::InvalidConfiguration(message) => {
                write!(formatter, "invalid router configuration: {message}")
            }
        }
    }
}

impl std::error::Error for RouterError {}

/// A subscription that keeps one route-provider listener alive.
pub struct RouteInformationSubscription {
    _listener: Option<RouteInformationListener>,
}

impl fmt::Debug for RouteInformationSubscription {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RouteInformationSubscription(..)")
    }
}

/// A retained route-information callback.
pub type RouteInformationListener = Rc<dyn Fn(RouteInformation)>;

/// The platform route-information boundary used by [`Router`].
pub trait RouteInformationProvider: 'static {
    /// Returns the current platform route.
    fn value(&self) -> RouteInformation;

    /// Publishes a platform-originated route change into this provider.
    ///
    /// Native deep links and application activations use this path; it is
    /// deliberately distinct from [`Self::router_reports_new_route_information`],
    /// which reports a route chosen by the application back to platform history.
    fn set_platform_route_information(&self, information: RouteInformation);

    /// Subscribes to platform-originated route changes.
    fn subscribe(&self, listener: RouteInformationListener) -> RouteInformationSubscription;

    /// Reports a delegate-originated route change to platform history.
    fn router_reports_new_route_information(
        &self,
        information: RouteInformation,
        reporting: RouteInformationReportingType,
    ) -> Result<(), RouterError>;
}

#[derive(Default)]
struct MemoryProviderState {
    value: RouteInformation,
    listeners: Vec<Weak<dyn Fn(RouteInformation)>>,
    reports: Vec<(RouteInformation, RouteInformationReportingType)>,
}

/// A deterministic in-memory provider for desktop adapters and tests.
#[derive(Clone)]
pub struct MemoryRouteInformationProvider {
    state: Rc<RefCell<MemoryProviderState>>,
}

impl fmt::Debug for MemoryRouteInformationProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MemoryRouteInformationProvider")
            .field("value", &self.value())
            .field("report_count", &self.report_count())
            .finish()
    }
}

impl MemoryRouteInformationProvider {
    /// Creates a provider with the supplied initial location.
    #[must_use]
    pub fn new(initial: impl Into<RouteInformation>) -> Self {
        Self {
            state: Rc::new(RefCell::new(MemoryProviderState {
                value: initial.into(),
                ..MemoryProviderState::default()
            })),
        }
    }

    /// Creates a provider with the platform's conventional root route.
    #[must_use]
    pub fn root() -> Self {
        Self::new(RouteInformation::default())
    }

    /// Publishes a platform-originated route and notifies listeners.
    pub fn set_value(&self, value: impl Into<RouteInformation>) {
        let value = value.into();
        let listeners = {
            let mut state = self.state.borrow_mut();
            state.value = value.clone();
            state
                .listeners
                .retain(|listener| listener.strong_count() > 0);
            state
                .listeners
                .iter()
                .filter_map(Weak::upgrade)
                .collect::<Vec<_>>()
        };
        for listener in listeners {
            listener(value.clone());
        }
    }

    /// Convenience alias matching the platform provider vocabulary.
    pub fn push(&self, value: impl Into<RouteInformation>) {
        self.set_value(value);
    }

    /// Returns all reports made by a router delegate.
    #[must_use]
    pub fn reports(&self) -> Vec<(RouteInformation, RouteInformationReportingType)> {
        self.state.borrow().reports.clone()
    }

    /// Returns the number of reports made by a router delegate.
    #[must_use]
    pub fn report_count(&self) -> usize {
        self.state.borrow().reports.len()
    }

    /// Clears the report log without changing the current route.
    pub fn clear_reports(&self) {
        self.state.borrow_mut().reports.clear();
    }
}

impl Default for MemoryRouteInformationProvider {
    fn default() -> Self {
        Self::root()
    }
}

impl RouteInformationProvider for MemoryRouteInformationProvider {
    fn value(&self) -> RouteInformation {
        self.state.borrow().value.clone()
    }

    fn set_platform_route_information(&self, information: RouteInformation) {
        self.set_value(information);
    }

    fn subscribe(&self, listener: RouteInformationListener) -> RouteInformationSubscription {
        self.state
            .borrow_mut()
            .listeners
            .push(Rc::downgrade(&listener));
        RouteInformationSubscription {
            _listener: Some(listener),
        }
    }

    fn router_reports_new_route_information(
        &self,
        information: RouteInformation,
        reporting: RouteInformationReportingType,
    ) -> Result<(), RouterError> {
        if reporting != RouteInformationReportingType::None {
            let mut state = self.state.borrow_mut();
            state.value = information.clone();
            state.reports.push((information, reporting));
        }
        Ok(())
    }
}

/// Parses and restores a typed router configuration.
pub trait RouteInformationParser<T>: 'static {
    /// Parses platform route information into delegate configuration.
    fn parse_route_information(&self, information: &RouteInformation) -> Result<T, RouterError>;

    /// Converts the delegate's current configuration back into route information.
    fn restore_route_information(&self, configuration: &T)
    -> Result<RouteInformation, RouterError>;
}

/// A closure-backed route parser for application-owned route types.
pub struct ClosureRouteInformationParser<T> {
    parse: RouteParser<T>,
    restore: RouteRestorer<T>,
}

impl<T> Clone for ClosureRouteInformationParser<T> {
    fn clone(&self) -> Self {
        Self {
            parse: self.parse.clone(),
            restore: self.restore.clone(),
        }
    }
}

impl<T> ClosureRouteInformationParser<T> {
    /// Creates a parser from parse and restore closures.
    #[must_use]
    pub fn new(
        parse: impl Fn(&RouteInformation) -> Result<T, RouterError> + 'static,
        restore: impl Fn(&T) -> Result<RouteInformation, RouterError> + 'static,
    ) -> Self {
        Self {
            parse: Rc::new(parse),
            restore: Rc::new(restore),
        }
    }
}

impl<T: 'static> RouteInformationParser<T> for ClosureRouteInformationParser<T> {
    fn parse_route_information(&self, information: &RouteInformation) -> Result<T, RouterError> {
        (self.parse)(information)
    }

    fn restore_route_information(
        &self,
        configuration: &T,
    ) -> Result<RouteInformation, RouterError> {
        (self.restore)(configuration)
    }
}

/// A parser for applications whose route configuration is just a location.
#[derive(Clone, Copy, Debug, Default)]
pub struct StringRouteInformationParser;

impl RouteInformationParser<String> for StringRouteInformationParser {
    fn parse_route_information(
        &self,
        information: &RouteInformation,
    ) -> Result<String, RouterError> {
        Ok(information.location().to_owned())
    }

    fn restore_route_information(
        &self,
        configuration: &String,
    ) -> Result<RouteInformation, RouterError> {
        Ok(RouteInformation::new(configuration))
    }
}

/// A subscription that keeps one delegate listener alive.
pub struct RouterDelegateSubscription {
    _listener: Option<RouterDelegateListener>,
}

impl fmt::Debug for RouterDelegateSubscription {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RouterDelegateSubscription(..)")
    }
}

/// A callback fired after a router delegate's current configuration changes.
pub type RouterDelegateListener = Rc<dyn Fn()>;

/// The retained route delegate consumed by [`Router`].
pub trait RouterDelegate<T>: 'static {
    /// Builds the current route subtree.
    fn build(&self) -> Widget;

    /// Returns the state which should be reflected in platform history.
    fn current_configuration(&self) -> Option<T>;

    /// Applies a newly parsed route.
    fn set_new_route_path(&self, configuration: T) -> Result<(), RouterError>;

    /// Applies the platform's initial route.  The default matches Flutter's
    /// delegate contract and forwards to [`Self::set_new_route_path`].
    fn set_initial_route_path(&self, configuration: T) -> Result<(), RouterError> {
        self.set_new_route_path(configuration)
    }

    /// Applies restored state.  The default forwards to the new-route hook.
    fn set_restored_route_path(&self, configuration: T) -> Result<(), RouterError> {
        self.set_new_route_path(configuration)
    }

    /// Handles one system back request.  `true` means the delegate consumed it.
    fn pop_route(&self) -> bool;

    /// Observes delegate state changes.
    fn subscribe(&self, listener: RouterDelegateListener) -> RouterDelegateSubscription;
}

struct BasicDelegateState<T> {
    configuration: Option<T>,
    /// Accepted-commit counter.  Each committed application bumps it, so an
    /// outer application can detect a nested commit and must not overwrite it.
    revision: u64,
    listeners: Vec<Weak<dyn Fn()>>,
}

/// A small retained delegate useful for app shells that own their route state
/// in a controller rather than implementing a bespoke delegate type.
#[derive(Clone)]
pub struct BasicRouterDelegate<T: Clone + 'static> {
    state: Rc<RefCell<BasicDelegateState<T>>>,
    builder: Rc<dyn Fn() -> Widget>,
    set_path: Rc<RefCell<Option<SetRoutePath<T>>>>,
    pop: Rc<RefCell<Option<PopRoute>>>,
}

impl<T: Clone + 'static> fmt::Debug for BasicRouterDelegate<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BasicRouterDelegate")
            .field(
                "configuration",
                &self.state.borrow().configuration.is_some(),
            )
            .finish_non_exhaustive()
    }
}

impl<T: Clone + 'static> BasicRouterDelegate<T> {
    /// Creates a delegate whose widget is rebuilt from `builder`.
    #[must_use]
    pub fn new(builder: impl Fn() -> Widget + 'static) -> Self {
        Self {
            state: Rc::new(RefCell::new(BasicDelegateState {
                configuration: None,
                revision: 0,
                listeners: Vec::new(),
            })),
            builder: Rc::new(builder),
            set_path: Rc::new(RefCell::new(None)),
            pop: Rc::new(RefCell::new(None)),
        }
    }

    /// Seeds the current configuration before the router is built.
    pub fn set_configuration(&self, configuration: T) {
        let mut state = self.state.borrow_mut();
        state.configuration = Some(configuration);
        state.revision = state.revision.wrapping_add(1);
        drop(state);
        self.notify();
    }

    /// Installs application logic invoked for new and initial route paths.
    pub fn on_set_new_route_path(&self, callback: impl Fn(T) -> Result<(), RouterError> + 'static) {
        // Retire the replaced callback outside the registration borrow: its
        // destructor is application code and may reenter registration.
        let previous = self.set_path.borrow_mut().replace(Rc::new(callback));
        drop(previous);
    }

    /// Installs the system-back callback.
    pub fn on_pop_route(&self, callback: impl Fn() -> bool + 'static) {
        // Retire the replaced callback outside the registration borrow: its
        // destructor is application code and may reenter registration.
        let previous = self.pop.borrow_mut().replace(Rc::new(callback));
        drop(previous);
    }

    /// Notifies the router that the delegate's current configuration changed.
    pub fn notify(&self) {
        let listeners = {
            let mut state = self.state.borrow_mut();
            state
                .listeners
                .retain(|listener| listener.strong_count() > 0);
            state
                .listeners
                .iter()
                .filter_map(Weak::upgrade)
                .collect::<Vec<_>>()
        };
        for listener in listeners {
            listener();
        }
    }

    /// Applies one route, atomically from the caller's perspective.
    ///
    /// The callback handle is cloned before invocation so no registration
    /// borrow spans application code, and the accepted revision is snapshotted
    /// so a nested application is detected below.  A rejection leaves the
    /// previously accepted configuration untouched and emits nothing.  When a
    /// nested application committed meanwhile, the outer acceptance is
    /// superseded: it still reports success (the callback accepted the route)
    /// but must not overwrite the newer configuration — and there is no
    /// rollback, which would erase the reentrant change.
    fn apply(&self, configuration: T) -> Result<(), RouterError> {
        let callback = self.set_path.borrow().as_ref().cloned();
        let revision = self.state.borrow().revision;
        if let Some(callback) = callback {
            callback(configuration.clone())?;
        }
        {
            let mut state = self.state.borrow_mut();
            if state.revision != revision {
                return Ok(());
            }
            state.configuration = Some(configuration);
            state.revision = state.revision.wrapping_add(1);
        }
        self.notify();
        Ok(())
    }
}

impl<T: Clone + 'static> RouterDelegate<T> for BasicRouterDelegate<T> {
    fn build(&self) -> Widget {
        (self.builder)()
    }

    fn current_configuration(&self) -> Option<T> {
        self.state.borrow().configuration.clone()
    }

    fn set_new_route_path(&self, configuration: T) -> Result<(), RouterError> {
        self.apply(configuration)
    }

    fn pop_route(&self) -> bool {
        // Clone the handle first: the callback is application code and may
        // replace the registration while it runs.
        let callback = self.pop.borrow().as_ref().cloned();
        callback.is_some_and(|callback| callback())
    }

    fn subscribe(&self, listener: RouterDelegateListener) -> RouterDelegateSubscription {
        self.state
            .borrow_mut()
            .listeners
            .push(Rc::downgrade(&listener));
        RouterDelegateSubscription {
            _listener: Some(listener),
        }
    }
}

#[derive(Default)]
struct BackDispatcherState {
    next_id: u64,
    callbacks: BTreeMap<u64, Weak<dyn Fn() -> bool>>,
}

/// A retained, priority-ordered back dispatcher.
#[derive(Clone, Default)]
pub struct BackButtonDispatcher {
    state: Rc<RefCell<BackDispatcherState>>,
}

impl fmt::Debug for BackButtonDispatcher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackButtonDispatcher")
            .field("callback_count", &self.state.borrow().callbacks.len())
            .finish()
    }
}

impl BackButtonDispatcher {
    /// Adds a callback.  Newer callbacks get first refusal, matching nested
    /// Flutter back-dispatcher priority.
    pub fn add_callback(&self, callback: impl Fn() -> bool + 'static) -> BackCallbackSubscription {
        let callback: Rc<dyn Fn() -> bool> = Rc::new(callback);
        let id = {
            let mut state = self.state.borrow_mut();
            state.next_id = state.next_id.wrapping_add(1).max(1);
            let id = state.next_id;
            state.callbacks.insert(id, Rc::downgrade(&callback));
            id
        };
        BackCallbackSubscription {
            dispatcher: Rc::downgrade(&self.state),
            id,
            _callback: callback,
        }
    }

    /// Gives each live callback a chance to handle the back request.
    pub fn dispatch_back(&self) -> bool {
        let callbacks = {
            let mut state = self.state.borrow_mut();
            state
                .callbacks
                .retain(|_, callback| callback.strong_count() > 0);
            state
                .callbacks
                .iter()
                .rev()
                .filter_map(|(_, callback)| Weak::upgrade(callback))
                .collect::<Vec<_>>()
        };
        callbacks.into_iter().any(|callback| callback())
    }
}

/// Keeps a callback registered with a [`BackButtonDispatcher`].
pub struct BackCallbackSubscription {
    dispatcher: Weak<RefCell<BackDispatcherState>>,
    id: u64,
    _callback: Rc<dyn Fn() -> bool>,
}

impl fmt::Debug for BackCallbackSubscription {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackCallbackSubscription")
            .field("id", &self.id)
            .finish()
    }
}

impl Drop for BackCallbackSubscription {
    fn drop(&mut self) {
        if let Some(dispatcher) = self.dispatcher.upgrade() {
            dispatcher.borrow_mut().callbacks.remove(&self.id);
        }
    }
}

/// The root back dispatcher used by an application bootstrap.
#[derive(Clone, Default)]
pub struct RootBackButtonDispatcher {
    dispatcher: BackButtonDispatcher,
}

impl fmt::Debug for RootBackButtonDispatcher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RootBackButtonDispatcher")
            .field("dispatcher", &self.dispatcher)
            .finish()
    }
}

impl RootBackButtonDispatcher {
    /// Creates an empty root dispatcher.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a root-level callback.
    pub fn add_callback(&self, callback: impl Fn() -> bool + 'static) -> BackCallbackSubscription {
        self.dispatcher.add_callback(callback)
    }

    /// Dispatches a native back/pop-route event.
    pub fn dispatch_back(&self) -> bool {
        self.dispatcher.dispatch_back()
    }

    /// Returns the shared dispatcher used by nested routers.
    #[must_use]
    pub fn dispatcher(&self) -> BackButtonDispatcher {
        self.dispatcher.clone()
    }
}

/// Why a [`NavigationNotification`] was emitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavigationNotificationKind {
    RouteInformationChanged,
    DelegateChanged,
    Restored,
    BackHandled,
    BackUnhandled,
    ParseFailed,
}

/// A router event suitable for an application-level notification hook.
#[derive(Clone, Debug, PartialEq)]
pub struct NavigationNotification {
    pub kind: NavigationNotificationKind,
    pub route_information: Option<RouteInformation>,
    pub can_handle_pop: bool,
    pub error: Option<RouterError>,
}

/// A subscription that keeps one navigation listener alive.
pub struct NavigationNotificationSubscription {
    pub(super) _listener: Option<NavigationNotificationListener>,
}

impl fmt::Debug for NavigationNotificationSubscription {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("NavigationNotificationSubscription(..)")
    }
}

/// Application callback for router notifications.
pub type NavigationNotificationListener = Rc<dyn Fn(NavigationNotification)>;

/// A validated collection of router integration objects.
pub struct RouterConfig<T> {
    route_information_provider: Option<Rc<dyn RouteInformationProvider>>,
    route_information_parser: Option<Rc<dyn RouteInformationParser<T>>>,
    router_delegate: Rc<dyn RouterDelegate<T>>,
    back_button_dispatcher: Option<RootBackButtonDispatcher>,
}

impl<T> Clone for RouterConfig<T> {
    fn clone(&self) -> Self {
        Self {
            route_information_provider: self.route_information_provider.clone(),
            route_information_parser: self.route_information_parser.clone(),
            router_delegate: self.router_delegate.clone(),
            back_button_dispatcher: self.back_button_dispatcher.clone(),
        }
    }
}

impl<T> fmt::Debug for RouterConfig<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RouterConfig")
            .field("has_provider", &self.route_information_provider.is_some())
            .field("has_parser", &self.route_information_parser.is_some())
            .field(
                "has_back_dispatcher",
                &self.back_button_dispatcher.is_some(),
            )
            .finish()
    }
}

impl<T: 'static> RouterConfig<T> {
    /// Creates a delegate-only router configuration.
    #[must_use]
    pub fn new(delegate: impl RouterDelegate<T>) -> Self {
        Self {
            route_information_provider: None,
            route_information_parser: None,
            router_delegate: Rc::new(delegate),
            back_button_dispatcher: None,
        }
    }

    /// Creates a configuration with the provider/parser pair required for
    /// platform deep links and route reporting.
    pub fn with_provider_parser(
        delegate: impl RouterDelegate<T>,
        provider: impl RouteInformationProvider,
        parser: impl RouteInformationParser<T>,
    ) -> Self {
        Self {
            route_information_provider: Some(Rc::new(provider)),
            route_information_parser: Some(Rc::new(parser)),
            router_delegate: Rc::new(delegate),
            back_button_dispatcher: None,
        }
    }

    /// Fallible constructor for callers holding trait objects.
    pub fn try_from_parts(
        delegate: Rc<dyn RouterDelegate<T>>,
        provider: Option<Rc<dyn RouteInformationProvider>>,
        parser: Option<Rc<dyn RouteInformationParser<T>>>,
    ) -> Result<Self, RouterError> {
        if provider.is_some() != parser.is_some() {
            return Err(RouterError::InvalidConfiguration(
                "route information provider and parser must be supplied together".into(),
            ));
        }
        Ok(Self {
            route_information_provider: provider,
            route_information_parser: parser,
            router_delegate: delegate,
            back_button_dispatcher: None,
        })
    }

    /// Adds the root back dispatcher.
    #[must_use]
    pub fn with_back_button_dispatcher(mut self, dispatcher: RootBackButtonDispatcher) -> Self {
        self.back_button_dispatcher = Some(dispatcher);
        self
    }

    #[must_use]
    pub fn route_information_provider(&self) -> Option<Rc<dyn RouteInformationProvider>> {
        self.route_information_provider.clone()
    }

    #[must_use]
    pub fn route_information_parser(&self) -> Option<Rc<dyn RouteInformationParser<T>>> {
        self.route_information_parser.clone()
    }

    #[must_use]
    pub fn router_delegate(&self) -> Rc<dyn RouterDelegate<T>> {
        self.router_delegate.clone()
    }

    #[must_use]
    pub fn back_button_dispatcher(&self) -> Option<RootBackButtonDispatcher> {
        self.back_button_dispatcher.clone()
    }
}

#[derive(Clone)]
struct RouterRestoration {
    scope: RestorationScope,
    key: RestorationKey,
}

/// A declarative router descriptor that owns its retained subscriptions after
/// conversion to a [`Widget`].
pub struct Router<T> {
    config: RouterConfig<T>,
    restoration: Option<RouterRestoration>,
    listeners: NavigationNotificationListeners,
}

impl<T> Clone for Router<T> {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            restoration: self.restoration.clone(),
            listeners: self.listeners.clone(),
        }
    }
}

impl<T> fmt::Debug for Router<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Router")
            .field("config", &self.config)
            .field("restorable", &self.restoration.is_some())
            .finish()
    }
}

impl<T: 'static> Router<T> {
    /// Creates a delegate-only router.
    #[must_use]
    pub fn new(delegate: impl RouterDelegate<T>) -> Self {
        Self::from_config(RouterConfig::new(delegate))
    }

    /// Creates a router with platform route information and parsing.
    #[must_use]
    pub fn with_provider_parser(
        delegate: impl RouterDelegate<T>,
        provider: impl RouteInformationProvider,
        parser: impl RouteInformationParser<T>,
    ) -> Self {
        Self::from_config(RouterConfig::with_provider_parser(
            delegate, provider, parser,
        ))
    }

    /// Creates a router from a validated configuration.
    #[must_use]
    pub fn from_config(config: RouterConfig<T>) -> Self {
        Self {
            config,
            restoration: None,
            listeners: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Enables route-information restoration at a stable scope/key.
    #[must_use]
    pub fn restoration_scope(mut self, scope: RestorationScope, key: RestorationKey) -> Self {
        self.restoration = Some(RouterRestoration { scope, key });
        self
    }

    /// Subscribes to route, delegate, parse, and back notifications.
    pub fn on_navigation_notification(
        &self,
        listener: impl Fn(NavigationNotification) + 'static,
    ) -> NavigationNotificationSubscription {
        let listener: NavigationNotificationListener = Rc::new(listener);
        self.listeners.borrow_mut().push(Rc::downgrade(&listener));
        NavigationNotificationSubscription {
            _listener: Some(listener),
        }
    }

    /// Returns the configured route provider, if one is installed.
    #[must_use]
    pub fn route_information_provider(&self) -> Option<Rc<dyn RouteInformationProvider>> {
        self.config.route_information_provider()
    }

    /// Dispatches a back event through the configured root dispatcher.
    #[must_use]
    pub fn dispatch_back(&self) -> bool {
        self.config
            .back_button_dispatcher()
            .is_some_and(|dispatcher| dispatcher.dispatch_back())
    }

    /// Builds a retained router widget.
    #[must_use]
    pub fn into_widget(self) -> Widget {
        let runtime = RouterRuntime::new(self);
        let revision = runtime.borrow().revision.clone();
        let retained = runtime.clone();
        Widget::stateful_layout_builder(revision, move |_, _| {
            RouterRuntime::build_widget(&retained)
        })
    }

    /// Alias for [`Self::into_widget`] useful in an application root builder.
    #[must_use]
    pub fn widget(self) -> Widget {
        self.into_widget()
    }
}

impl<T: 'static> From<Router<T>> for Widget {
    fn from(value: Router<T>) -> Self {
        value.into_widget()
    }
}

#[derive(Clone, Copy)]
enum RouteApplyKind {
    Initial,
    Restored,
    New,
}

struct RouterRuntime<T: 'static> {
    provider: Option<Rc<dyn RouteInformationProvider>>,
    parser: Option<Rc<dyn RouteInformationParser<T>>>,
    delegate: Rc<dyn RouterDelegate<T>>,
    back_dispatcher: RootBackButtonDispatcher,
    restoration: Option<RouterRestoration>,
    listeners: NavigationNotificationListeners,
    current: Option<RouteInformation>,
    revision: Rc<Cell<u64>>,
    /// Committed-transaction counter.  An operation snapshots it before
    /// invoking application code and may commit only while it is unchanged:
    /// a newer commit preempts older operations.  Failed operations never
    /// bump it, so they preempt nothing.
    transaction: u64,
    subscriptions: Vec<SubscriptionKeepAlive>,
}

enum SubscriptionKeepAlive {
    Provider(RouteInformationSubscription),
    Delegate(RouterDelegateSubscription),
    Back(BackCallbackSubscription),
}

impl SubscriptionKeepAlive {
    fn touch(&self) {
        match self {
            Self::Provider(subscription) => {
                let _ = subscription;
            }
            Self::Delegate(subscription) => {
                let _ = subscription;
            }
            Self::Back(subscription) => {
                let _ = subscription;
            }
        }
    }
}

impl<T: 'static> RouterRuntime<T> {
    fn new(router: Router<T>) -> Rc<RefCell<Self>> {
        let config = router.config;
        let back_dispatcher = config.back_button_dispatcher.clone().unwrap_or_default();
        let runtime = Rc::new(RefCell::new(Self {
            provider: config.route_information_provider,
            parser: config.route_information_parser,
            delegate: config.router_delegate,
            back_dispatcher,
            restoration: router.restoration,
            listeners: router.listeners,
            current: None,
            revision: Rc::new(Cell::new(0)),
            transaction: 0,
            subscriptions: Vec::new(),
        }));

        let weak_runtime = Rc::downgrade(&runtime);
        let provider_subscription = runtime.borrow().provider.as_ref().map(|provider| {
            let weak_runtime = weak_runtime.clone();
            provider.subscribe(Rc::new(move |information| {
                if let Some(runtime) = weak_runtime.upgrade() {
                    Self::receive_route_information(&runtime, information, RouteApplyKind::New);
                }
            }))
        });

        let weak_runtime = Rc::downgrade(&runtime);
        let delegate_subscription = {
            let delegate = runtime.borrow().delegate.clone();
            delegate.subscribe(Rc::new(move || {
                if let Some(runtime) = weak_runtime.upgrade() {
                    Self::delegate_changed(&runtime);
                }
            }))
        };

        let weak_runtime = Rc::downgrade(&runtime);
        let back_subscription = runtime.borrow().back_dispatcher.add_callback(move || {
            weak_runtime
                .upgrade()
                .is_some_and(|runtime| Self::handle_back(&runtime))
        });

        {
            let mut state = runtime.borrow_mut();
            if let Some(subscription) = provider_subscription {
                state
                    .subscriptions
                    .push(SubscriptionKeepAlive::Provider(subscription));
            }
            state
                .subscriptions
                .push(SubscriptionKeepAlive::Delegate(delegate_subscription));
            state
                .subscriptions
                .push(SubscriptionKeepAlive::Back(back_subscription));
        }
        Self::initialize(&runtime);
        runtime
    }

    fn initialize(runtime: &Rc<RefCell<Self>>) {
        let (restoration, provider, parser) = {
            let state = runtime.borrow();
            (
                state.restoration.clone(),
                state.provider.clone(),
                state.parser.clone(),
            )
        };

        if let Some(restoration) = restoration
            && let Some(value) = restoration.scope.get_json(&restoration.key)
        {
            match RouteInformation::from_json(&value) {
                Ok(information) => {
                    if Self::apply_route(runtime, information, RouteApplyKind::Restored).is_ok() {
                        return;
                    }
                }
                // A malformed persisted value is reported rather than
                // silently skipped, then falls through to the provider
                // fallback below: partial restoration, not all-or-nothing.
                Err(error) => {
                    Self::notify(
                        runtime,
                        NavigationNotification {
                            kind: NavigationNotificationKind::ParseFailed,
                            route_information: None,
                            can_handle_pop: false,
                            error: Some(error),
                        },
                    );
                }
            }
        }

        // The restoration attempt above — including its failure observers —
        // may have installed a route reentrantly; the provider fallback must
        // not overwrite an accepted commit.
        let installed = runtime.borrow().current.is_some();
        if !installed && let (Some(provider), Some(_parser)) = (provider, parser) {
            let information = provider.value();
            // A reentrant commit during the provider read owns the route; the
            // fallback must not overwrite it with a stale read.
            if runtime.borrow().current.is_some() {
                return;
            }
            if Self::apply_route(runtime, information, RouteApplyKind::Initial).is_ok() {
                return;
            }
        }

        let revision = runtime.borrow().revision.clone();
        revision.set(revision.get().wrapping_add(1));
    }

    fn receive_route_information(
        runtime: &Rc<RefCell<Self>>,
        information: RouteInformation,
        kind: RouteApplyKind,
    ) {
        // Every failure inside `apply_route` already notified exactly once;
        // adding another notification here would double-report.
        let _ = Self::apply_route(runtime, information, kind);
    }

    /// Applies one route attempt with exactly-once failure reporting.
    ///
    /// Every failed attempt notifies from inside this function, so
    /// [`Self::receive_route_information`] never adds another notification:
    ///
    /// | Failure source           | Typed result                  | Notification (one per attempt)              |
    /// |--------------------------|-------------------------------|-----------------------------------------------|
    /// | Missing parser           | `Err(InvalidConfiguration)`   | none: unreachable — [`RouterConfig::try_from_parts`] rejects an unpaired provider/parser at construction |
    /// | Parser rejection         | `Err(Parse(..))`              | `ParseFailed` with the route information      |
    /// | Delegate rejection       | `Err(Delegate(..))`           | `ParseFailed` with the route information; the router commits nothing |
    /// | Malformed persisted data | `Err(Restoration(..))`        | `ParseFailed` with no route information; the provider fallback is a separate attempt |
    /// | Superseded transaction   | `Ok(())` with no commit       | none: not a failure — the winning transaction's commit notification is the single record |
    ///
    /// A rejection notifies without committing: no success notification
    /// follows a failed attempt, and the accepted route, delegate
    /// configuration, and persisted scope all survive until a later attempt
    /// commits.  Delegate configuration and router state are distinct layers:
    /// the delegate owns its atomicity (`BasicRouterDelegate` preserves the
    /// previously accepted configuration), while current route and persisted
    /// scope commit only through [`Self::commit_route`].
    fn apply_route(
        runtime: &Rc<RefCell<Self>>,
        information: RouteInformation,
        kind: RouteApplyKind,
    ) -> Result<(), RouterError> {
        // Snapshot handles without bumping the transaction: only a commit
        // owns the next number, so failed operations preempt nothing.
        let (parser, delegate, transaction) = {
            let state = runtime.borrow();
            (
                state.parser.clone(),
                state.delegate.clone(),
                state.transaction,
            )
        };
        let parser = parser.ok_or_else(|| {
            RouterError::InvalidConfiguration("a route provider requires a route parser".into())
        })?;
        // The parser is application code and may deliver another route
        // reentrantly; that nested commit owns the route state afterwards.
        let configuration = match parser.parse_route_information(&information) {
            Ok(configuration) => configuration,
            Err(error) => {
                Self::notify(
                    runtime,
                    NavigationNotification {
                        kind: NavigationNotificationKind::ParseFailed,
                        route_information: Some(information),
                        can_handle_pop: false,
                        error: Some(error.clone()),
                    },
                );
                return Err(error);
            }
        };
        if Self::is_superseded(runtime, transaction) {
            // A nested transaction committed while parsing: the older
            // operation reports success (a route is installed) but commits
            // nothing over the newer transaction.
            return Ok(());
        }
        // The delegate hooks are application code with the same reentrancy
        // contract: a nested commit preempts this operation's commit below.
        let result = match kind {
            RouteApplyKind::Initial => delegate.set_initial_route_path(configuration),
            RouteApplyKind::Restored => delegate.set_restored_route_path(configuration),
            RouteApplyKind::New => delegate.set_new_route_path(configuration),
        };
        if let Err(error) = result {
            Self::notify(
                runtime,
                NavigationNotification {
                    kind: NavigationNotificationKind::ParseFailed,
                    route_information: Some(information),
                    can_handle_pop: false,
                    error: Some(error.clone()),
                },
            );
            return Err(error);
        }
        if Self::is_superseded(runtime, transaction) {
            return Ok(());
        }
        Self::commit_route(
            runtime,
            information,
            if matches!(kind, RouteApplyKind::Restored) {
                NavigationNotificationKind::Restored
            } else {
                NavigationNotificationKind::RouteInformationChanged
            },
            None,
            transaction,
        );
        Ok(())
    }

    /// Reports whether a newer transaction committed since `transaction` was
    /// snapshotted, revoking the snapshotting operation's right to commit.
    fn is_superseded(runtime: &Rc<RefCell<Self>>, transaction: u64) -> bool {
        runtime.borrow().transaction != transaction
    }

    fn commit_route(
        runtime: &Rc<RefCell<Self>>,
        information: RouteInformation,
        kind: NavigationNotificationKind,
        error: Option<RouterError>,
        transaction: u64,
    ) {
        let (restoration, delegate, revision) = {
            let state = runtime.borrow();
            (
                state.restoration.clone(),
                state.delegate.clone(),
                state.revision.clone(),
            )
        };
        // A delegate query is application code and may reenter the router,
        // so it runs outside the borrow and the commit right is rechecked.
        let can_handle_pop = delegate.current_configuration().is_some();
        {
            let mut state = runtime.borrow_mut();
            if state.transaction != transaction {
                return;
            }
            state.transaction = state.transaction.wrapping_add(1);
            state.current = Some(information.clone());
        }
        if let Some(restoration) = restoration {
            restoration
                .scope
                .set_json(&restoration.key, information.to_json());
        }
        revision.set(revision.get().wrapping_add(1));
        Self::notify(
            runtime,
            NavigationNotification {
                kind,
                route_information: Some(information),
                can_handle_pop,
                error,
            },
        );
    }

    fn delegate_changed(runtime: &Rc<RefCell<Self>>) {
        // The delegate query is application code and may reenter the router,
        // so it runs without any runtime borrow held; a nested commit meanwhile
        // makes this reaction stale before it even starts.
        let (delegate, transaction) = {
            let state = runtime.borrow();
            (state.delegate.clone(), state.transaction)
        };
        let configuration = delegate.current_configuration();
        if Self::is_superseded(runtime, transaction) {
            return;
        }
        let (parser, provider, restoration, revision) = {
            let state = runtime.borrow();
            (
                state.parser.clone(),
                state.provider.clone(),
                state.restoration.clone(),
                state.revision.clone(),
            )
        };
        let Some(configuration) = configuration else {
            revision.set(revision.get().wrapping_add(1));
            return;
        };
        let Some(parser) = parser else {
            revision.set(revision.get().wrapping_add(1));
            Self::notify(
                runtime,
                NavigationNotification {
                    kind: NavigationNotificationKind::DelegateChanged,
                    route_information: None,
                    can_handle_pop: true,
                    error: None,
                },
            );
            return;
        };
        match parser.restore_route_information(&configuration) {
            Ok(information) => {
                // The restorer is application code: a nested commit meanwhile
                // owns the route state and this reaction commits nothing.
                if Self::is_superseded(runtime, transaction) {
                    return;
                }
                if let Some(provider) = provider {
                    let _ = provider.router_reports_new_route_information(
                        information.clone(),
                        RouteInformationReportingType::Navigate,
                    );
                }
                // A custom provider report may also reenter the router, so
                // the commit right is rechecked before writing.
                if Self::is_superseded(runtime, transaction) {
                    return;
                }
                if let Some(restoration) = restoration {
                    restoration
                        .scope
                        .set_json(&restoration.key, information.to_json());
                }
                {
                    // This reaction never claims a transaction number: inside
                    // an in-flight application the outer commit owns the
                    // number, and standalone there is no contender.  Either
                    // way the newest commit wins by time order.
                    let mut state = runtime.borrow_mut();
                    state.current = Some(information.clone());
                }
                revision.set(revision.get().wrapping_add(1));
                Self::notify(
                    runtime,
                    NavigationNotification {
                        kind: NavigationNotificationKind::DelegateChanged,
                        route_information: Some(information),
                        can_handle_pop: true,
                        error: None,
                    },
                );
            }
            Err(error) => {
                revision.set(revision.get().wrapping_add(1));
                Self::notify(
                    runtime,
                    NavigationNotification {
                        kind: NavigationNotificationKind::DelegateChanged,
                        route_information: None,
                        can_handle_pop: true,
                        error: Some(error),
                    },
                );
            }
        }
    }

    fn handle_back(runtime: &Rc<RefCell<Self>>) -> bool {
        let delegate = runtime.borrow().delegate.clone();
        let handled = delegate.pop_route();
        let can_handle_pop = delegate.current_configuration().is_some();
        let information = runtime.borrow().current.clone();
        let revision = runtime.borrow().revision.clone();
        revision.set(revision.get().wrapping_add(1));
        Self::notify(
            runtime,
            NavigationNotification {
                kind: if handled {
                    NavigationNotificationKind::BackHandled
                } else {
                    NavigationNotificationKind::BackUnhandled
                },
                route_information: information,
                can_handle_pop,
                error: None,
            },
        );
        handled
    }

    fn notify(runtime: &Rc<RefCell<Self>>, notification: NavigationNotification) {
        let listeners = {
            let listeners = runtime.borrow().listeners.clone();
            let mut listeners = listeners.borrow_mut();
            listeners.retain(|listener| listener.strong_count() > 0);
            listeners
                .iter()
                .filter_map(Weak::upgrade)
                .collect::<Vec<_>>()
        };
        for listener in listeners {
            listener(notification.clone());
        }
    }

    fn build_widget(runtime: &Rc<RefCell<Self>>) -> Widget {
        let (delegate, current_route, revision) = {
            let state = runtime.borrow();
            for subscription in &state.subscriptions {
                subscription.touch();
            }
            (
                state.delegate.clone(),
                state.current.clone(),
                state.revision.get(),
            )
        };
        // Delegate queries are application code and run outside the borrow.
        let data = RouterData {
            current_route,
            can_pop: delegate.current_configuration().is_some(),
            revision,
        };
        Widget::environment_scope(data, delegate.build())
    }
}

/// Ambient router state installed above a delegate's current route subtree.
#[derive(Clone, Debug, PartialEq)]
pub struct RouterData {
    pub current_route: Option<RouteInformation>,
    pub can_pop: bool,
    pub revision: u64,
}

impl RouterData {
    /// Returns the current location or `/` when no provider has initialized.
    #[must_use]
    pub fn location(&self) -> &str {
        self.current_route
            .as_ref()
            .map_or("/", RouteInformation::location)
    }
}
