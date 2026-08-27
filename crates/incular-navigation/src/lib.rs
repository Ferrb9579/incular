//! Stack navigation, deep-link routing, route transitions, and overlays.
//!
//! The subsystem depends one-way on widget descriptions and retained
//! transition layers. It owns no widget tree or renderer state.

use std::{
    cell::RefCell,
    collections::HashMap,
    fmt,
    rc::Rc,
    time::{Duration, Instant},
};

use incular_core::Color;
use incular_widgets::internal::{OpacityController, TranslationController};
use incular_widgets::{FadeTransition, SlideTransition, Widget};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

/// Stable identity for a route in a [`Navigator`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RouteId(u64);

/// Version of the standalone navigator persistence payload.
///
/// This is deliberately independent from the framework-level restoration
/// format version. A runtime restoration store should retain this payload as a
/// value below the navigator's stable restoration scope.
pub const NAVIGATOR_SNAPSHOT_FORMAT_VERSION: u32 = 1;

/// An application-defined, stable identifier for a route registered for
/// restoration.
///
/// This is not [`RouteId`]: `RouteId` is a session-local, generational
/// identity used by a live [`Navigator`]. `RestorableRouteId` is persisted and
/// therefore must remain stable across application versions which support the
/// route. It is normalized using the same rules as a registry location.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct RestorableRouteId(String);
impl RestorableRouteId {
    /// Creates a stable route identifier.
    ///
    /// Empty identifiers and identifiers containing control characters are
    /// rejected so a persisted route cannot be confused with an absent value
    /// or rendered ambiguously in diagnostics.
    pub fn new(value: impl AsRef<str>) -> Result<Self, RestorableRouteIdError> {
        let value = value.as_ref();
        if value.trim().is_empty() {
            return Err(RestorableRouteIdError::Empty);
        }
        if value.chars().any(char::is_control) {
            return Err(RestorableRouteIdError::ContainsControlCharacter);
        }
        Ok(Self(normalize_location(value)))
    }

    /// Returns the normalized application-defined route identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl AsRef<str> for RestorableRouteId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl fmt::Display for RestorableRouteId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
impl<'de> Deserialize<'de> for RestorableRouteId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Why a [`RestorableRouteId`] was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestorableRouteIdError {
    /// The identifier was empty or whitespace only.
    Empty,
    /// The identifier contains a control character.
    ContainsControlCharacter,
}
impl fmt::Display for RestorableRouteIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("a restorable route ID must not be empty"),
            Self::ContainsControlCharacter => {
                formatter.write_str("a restorable route ID must not contain control characters")
            }
        }
    }
}
impl std::error::Error for RestorableRouteIdError {}

/// A stable, single-segment key for a route-specific restoration scope.
///
/// Route scope keys are optional. Supply one for dynamic route instances that
/// need independent state, for example `document-42`. The key intentionally
/// cannot contain path separators: callers may safely combine it with their
/// navigator scope using structured restoration paths rather than ambiguous
/// string concatenation.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct RouteScopeKey(String);
impl RouteScopeKey {
    pub fn new(value: impl AsRef<str>) -> Result<Self, RouteScopeKeyError> {
        let value = value.as_ref();
        if value.trim().is_empty() {
            return Err(RouteScopeKeyError::Empty);
        }
        if value.contains(['/', '\\']) {
            return Err(RouteScopeKeyError::ContainsPathSeparator);
        }
        if value.chars().any(char::is_control) {
            return Err(RouteScopeKeyError::ContainsControlCharacter);
        }
        Ok(Self(value.to_owned()))
    }

    /// Returns the stable key without exposing any live navigator identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl AsRef<str> for RouteScopeKey {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl fmt::Display for RouteScopeKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
impl<'de> Deserialize<'de> for RouteScopeKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Why a [`RouteScopeKey`] was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteScopeKeyError {
    /// The key was empty or whitespace only.
    Empty,
    /// Scope keys model exactly one restoration-path segment.
    ContainsPathSeparator,
    /// The key contains a control character.
    ContainsControlCharacter,
}
impl fmt::Display for RouteScopeKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("a route scope key must not be empty"),
            Self::ContainsPathSeparator => {
                formatter.write_str("a route scope key must not contain path separators")
            }
            Self::ContainsControlCharacter => {
                formatter.write_str("a route scope key must not contain control characters")
            }
        }
    }
}
impl std::error::Error for RouteScopeKeyError {}

/// Serializable state for one restorable navigator entry.
///
/// It contains only declarative data. In particular it never contains the
/// closure used to build a page, the resulting [`Widget`], or a live
/// [`RouteId`]. The registered route builder receives `arguments`; `state` is
/// application-owned serializable route state that can be placed below the
/// route's restoration scope.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RestorableRoute {
    /// Registered stable route identifier.
    pub route_id: RestorableRouteId,
    /// Serializable arguments used to rebuild the route.
    #[serde(default, skip_serializing_if = "is_json_null")]
    pub arguments: Value,
    /// Serializable application-owned route state.
    #[serde(default, skip_serializing_if = "is_json_null")]
    pub state: Value,
    /// Optional stable scope key for this particular route instance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_key: Option<RouteScopeKey>,
    /// Whether an explicit permanent pop should remove `scope_key` state.
    ///
    /// This lifecycle policy is deliberately not serialized. The currently
    /// registered application policy is reapplied during restoration.
    #[serde(skip)]
    clear_scope_on_pop: bool,
}
impl RestorableRoute {
    /// Creates serializable state for a route registered in a
    /// [`RouteRegistry`].
    #[must_use]
    pub fn new(route_id: RestorableRouteId, arguments: Value) -> Self {
        Self {
            route_id,
            arguments,
            state: Value::Null,
            scope_key: None,
            clear_scope_on_pop: false,
        }
    }

    /// Attaches application-owned, serializable route state.
    #[must_use]
    pub fn state(mut self, state: Value) -> Self {
        self.state = state;
        self
    }

    /// Assigns a stable scope key to this route instance.
    #[must_use]
    pub fn scope_key(mut self, key: RouteScopeKey) -> Self {
        self.scope_key = Some(key);
        self
    }

    fn removes_scope_on_pop(&self) -> bool {
        self.clear_scope_on_pop
    }

    fn with_registered_scope_cleanup(mut self, clear_scope_on_pop: bool) -> Self {
        self.clear_scope_on_pop = clear_scope_on_pop;
        self
    }
}

fn is_json_null(value: &Value) -> bool {
    value.is_null()
}

/// A versioned, serde-compatible navigator persistence payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NavigatorSnapshot {
    /// Version for this navigator payload, independent of the application
    /// restoration schema and framework snapshot format.
    pub format_version: u32,
    /// Persistent stack order from root to active route.
    #[serde(default)]
    pub routes: Vec<RestorableRoute>,
    /// Index into `routes` of the active route. A stack navigator normally
    /// writes the final route here; it is retained explicitly so a future
    /// navigator presentation model can evolve without changing the payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_route: Option<usize>,
}
impl Default for NavigatorSnapshot {
    fn default() -> Self {
        Self {
            format_version: NAVIGATOR_SNAPSHOT_FORMAT_VERSION,
            routes: Vec::new(),
            active_route: None,
        }
    }
}

/// Errors returned by restorable page builders when route arguments cannot be
/// safely rebuilt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RestorableRouteBuildError {
    /// The serialized arguments are not valid for the currently registered
    /// route definition.
    InvalidArguments(String),
    /// The application declined to build the route for another safe reason.
    BuildFailed(String),
}
impl RestorableRouteBuildError {
    #[must_use]
    pub fn invalid_arguments(message: impl Into<String>) -> Self {
        Self::InvalidArguments(message.into())
    }

    #[must_use]
    pub fn build_failed(message: impl Into<String>) -> Self {
        Self::BuildFailed(message.into())
    }
}
impl fmt::Display for RestorableRouteBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArguments(message) => {
                write!(formatter, "invalid route arguments: {message}")
            }
            Self::BuildFailed(message) => write!(formatter, "could not build route: {message}"),
        }
    }
}
impl std::error::Error for RestorableRouteBuildError {}

/// Failure while registering a restorable route definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RestorableRouteRegistrationError {
    /// The supplied stable route identifier was invalid.
    InvalidRouteId(RestorableRouteIdError),
    /// A route with the same normalized stable identifier is already present.
    DuplicateRouteId(RestorableRouteId),
}
impl fmt::Display for RestorableRouteRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRouteId(error) => error.fmt(formatter),
            Self::DuplicateRouteId(id) => write!(
                formatter,
                "a restorable route is already registered for {id}"
            ),
        }
    }
}
impl std::error::Error for RestorableRouteRegistrationError {}

/// Failure while a caller tries to push a restorable route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RestorableNavigationError {
    /// No restorable route builder is registered for the identifier.
    UnknownRoute(RestorableRouteId),
    /// The registered builder rejected the serializable arguments.
    BuildFailed(RestorableRouteBuildError),
}
impl fmt::Display for RestorableNavigationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownRoute(id) => {
                write!(formatter, "no restorable route is registered for {id}")
            }
            Self::BuildFailed(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for RestorableNavigationError {}

/// Outcome from reconstructing a navigator stack from a [`NavigatorSnapshot`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NavigationRestoreReport {
    /// Number of registered routes reconstructed into the live navigator.
    pub restored_routes: usize,
    /// Number of routes discarded because their ID, arguments, or active index
    /// were no longer valid. Restoration always retains the valid prefix.
    pub invalid_routes: usize,
    /// The serialized navigator payload used a future/unsupported format.
    pub unsupported_format: bool,
    /// The supplied fallback page was installed because no valid saved root
    /// route could be reconstructed.
    pub used_fallback: bool,
}

/// A reusable description of a navigable screen.
///
/// Unlike [`Route`], a page has no runtime identity, allowing the same page
/// description to be placed in multiple navigators.
#[derive(Clone)]
pub struct Page {
    pub name: String,
    pub child: Widget,
}

/// Stable, serializable metadata associated with a route.
///
/// Flutter exposes route arguments as an untyped `Object?`. Incular keeps the
/// persisted representation explicit JSON and lets applications recover a
/// concrete Rust type at the boundary with [`RouteSettings::arguments`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RouteSettings {
    name: String,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    arguments: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    restoration_scope: Option<RouteScopeKey>,
}

impl RouteSettings {
    /// Creates settings with no untyped arguments.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            arguments: Value::Null,
            restoration_scope: None,
        }
    }

    /// Sets serializable route arguments. The value remains JSON only at the
    /// persistence boundary; callers can use [`Self::arguments`] to decode it
    /// into their application enum/struct.
    pub fn with_arguments<T: Serialize>(
        mut self,
        arguments: &T,
    ) -> Result<Self, serde_json::Error> {
        self.arguments = serde_json::to_value(arguments)?;
        Ok(self)
    }

    /// Attaches a stable restoration scope to this route.
    #[must_use]
    pub fn restoration_scope(mut self, scope: RouteScopeKey) -> Self {
        self.restoration_scope = Some(scope);
        self
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn raw_arguments(&self) -> &Value {
        &self.arguments
    }

    /// Decodes the route arguments into an application-owned type.
    pub fn arguments<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.arguments.clone())
    }

    #[must_use]
    pub fn restoration_scope_key(&self) -> Option<&RouteScopeKey> {
        self.restoration_scope.as_ref()
    }
}

/// Rust-native route presentation composition.
///
/// These variants replace Flutter's `Route`/`ModalRoute`/`PopupRoute` class
/// hierarchy. Core owns the mechanics and Material can supply appearance.
#[derive(Clone)]
pub enum RoutePresentation {
    /// A normal page in the navigator's content area.
    Page {
        opaque: bool,
        maintain_state: bool,
        fullscreen_dialog: bool,
    },
    /// A non-fullscreen overlay presentation.
    Popup {
        barrier: Option<ModalBarrier>,
        maintain_state: bool,
    },
    /// A modal presentation that isolates background input.
    Modal {
        barrier: ModalBarrier,
        focus_trap: bool,
    },
    /// A route backed by one or more existing overlay entries.
    Overlay { entries: Vec<OverlayEntry> },
}

impl Default for RoutePresentation {
    fn default() -> Self {
        Self::Page {
            opaque: true,
            maintain_state: true,
            fullscreen_dialog: false,
        }
    }
}

impl RoutePresentation {
    #[must_use]
    pub const fn page() -> Self {
        Self::Page {
            opaque: true,
            maintain_state: true,
            fullscreen_dialog: false,
        }
    }

    #[must_use]
    pub fn popup(barrier: Option<ModalBarrier>) -> Self {
        Self::Popup {
            barrier,
            maintain_state: true,
        }
    }

    #[must_use]
    pub fn modal(barrier: ModalBarrier) -> Self {
        Self::Modal {
            barrier,
            focus_trap: true,
        }
    }

    #[must_use]
    pub fn overlay(entries: impl IntoIterator<Item = OverlayEntry>) -> Self {
        Self::Overlay {
            entries: entries.into_iter().collect(),
        }
    }

    #[must_use]
    pub const fn is_opaque(&self) -> bool {
        matches!(self, Self::Page { opaque: true, .. })
    }

    #[must_use]
    pub const fn blocks_background_input(&self) -> bool {
        matches!(self, Self::Modal { .. })
    }
}

/// A typed, retained completion channel for a route result.
///
/// The navigator remains synchronous and renderer-independent; applications
/// retain this handle and complete it from their route action. No `Any` or
/// untyped Dart `Object?` is needed.
#[derive(Clone)]
pub struct RouteResult<T> {
    state: Rc<RefCell<Option<T>>>,
}

impl<T> Default for RouteResult<T> {
    fn default() -> Self {
        Self {
            state: Rc::new(RefCell::new(None)),
        }
    }
}

impl<T> RouteResult<T> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Completes the result once. Repeated completion is rejected.
    pub fn complete(&self, value: T) -> Result<(), T> {
        let mut slot = self.state.borrow_mut();
        if slot.is_some() {
            return Err(value);
        }
        *slot = Some(value);
        Ok(())
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.state.borrow().is_some()
    }

    #[must_use]
    pub fn take(&self) -> Option<T> {
        self.state.borrow_mut().take()
    }
}
impl Page {
    #[must_use]
    pub fn new(name: impl Into<String>, child: impl Into<Widget>) -> Self {
        Self {
            name: name.into(),
            child: child.into(),
        }
    }
}

/// A declarative route: an application name and the widget it displays.
#[derive(Clone)]
pub struct Route {
    pub id: RouteId,
    pub name: String,
    pub child: Widget,
    pub transition: RouteTransition,
    pub settings: RouteSettings,
    pub presentation: RoutePresentation,
}

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
    /// A registered guard declined the mutation.
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
impl Route {
    #[must_use]
    pub fn new(name: impl Into<String>, child: impl Into<Widget>) -> Self {
        let name = name.into();
        Self {
            id: RouteId(0),
            settings: RouteSettings::new(name.clone()),
            name,
            child: child.into(),
            transition: RouteTransition::None,
            presentation: RoutePresentation::default(),
        }
    }
    /// Replaces the route metadata and keeps the legacy `name` field in sync.
    #[must_use]
    pub fn settings(mut self, settings: RouteSettings) -> Self {
        self.name = settings.name.clone();
        self.settings = settings;
        self
    }

    #[must_use]
    pub fn presentation(mut self, presentation: RoutePresentation) -> Self {
        self.presentation = presentation;
        self
    }

    #[must_use]
    pub fn popup(self, barrier: Option<ModalBarrier>) -> Self {
        self.presentation(RoutePresentation::popup(barrier))
    }

    #[must_use]
    pub fn modal(self, barrier: ModalBarrier) -> Self {
        self.presentation(RoutePresentation::modal(barrier))
    }

    #[must_use]
    pub fn dialog(self) -> Self {
        self.modal(ModalBarrier::default())
    }

    #[must_use]
    pub fn overlay(self, entries: impl IntoIterator<Item = OverlayEntry>) -> Self {
        self.presentation(RoutePresentation::overlay(entries))
    }
    #[must_use]
    pub fn transition(mut self, transition: RouteTransition) -> Self {
        self.transition = transition;
        self
    }
    /// Returns the retained presentation with its transition layer applied.
    #[must_use]
    pub fn presented_child(&self) -> Widget {
        self.transition.apply(self.child.clone())
    }
}

/// Ergonomic builder for a page route with a custom transition.
#[derive(Clone)]
pub struct PageRouteBuilder {
    route: Route,
}

impl PageRouteBuilder {
    #[must_use]
    pub fn new(name: impl Into<String>, child: impl Into<Widget>) -> Self {
        Self {
            route: Route::new(name, child),
        }
    }

    #[must_use]
    pub fn settings(mut self, settings: RouteSettings) -> Self {
        self.route = self.route.settings(settings);
        self
    }

    #[must_use]
    pub fn transition(mut self, transition: RouteTransition) -> Self {
        self.route = self.route.transition(transition);
        self
    }

    #[must_use]
    pub fn build(self) -> Route {
        self.route
    }
}
impl From<Page> for Route {
    fn from(page: Page) -> Self {
        Self::new(page.name, page.child)
    }
}

/// Route-level presentation transitions backed by retained controller layers.
#[derive(Clone)]
pub enum RouteTransition {
    None,
    Fade(OpacityController),
    Slide(TranslationController),
    FadeSlide {
        opacity: OpacityController,
        translation: TranslationController,
    },
}
impl RouteTransition {
    /// Creates a neutral compositor fade-in without exposing the retained
    /// opacity controller. The route owns the animation and the runtime ticks
    /// it along with the rest of the retained tree.
    #[must_use]
    pub fn fade_in(duration: Duration, now: Instant) -> Self {
        let opacity = OpacityController::new();
        opacity.set_opacity(0.);
        opacity.animate_to(1., duration, now);
        Self::Fade(opacity)
    }

    #[must_use]
    pub fn apply(&self, child: Widget) -> Widget {
        match self {
            Self::None => child,
            Self::Fade(controller) => FadeTransition::new(controller.clone(), child).into(),
            Self::Slide(controller) => SlideTransition::new(controller.clone(), child).into(),
            Self::FadeSlide {
                opacity,
                translation,
            } => FadeTransition::new(
                opacity.clone(),
                SlideTransition::new(translation.clone(), child),
            )
            .into(),
        }
    }
}

/// Maps external locations to declarative [`Page`] builders without coupling
/// navigation to URL, file-association, or platform APIs.
type PageBuilder = Rc<dyn Fn() -> Page>;
type RouteBuilders = Rc<RefCell<HashMap<String, PageBuilder>>>;
type RestorableRouteBuilder = Rc<dyn Fn(&Value) -> Result<Page, RestorableRouteBuildError>>;
type RestorableRouteBuilders = Rc<RefCell<HashMap<RestorableRouteId, RestorableRouteDefinition>>>;

#[derive(Clone)]
struct RestorableRouteDefinition {
    builder: RestorableRouteBuilder,
    clear_scope_on_pop: bool,
}

#[derive(Clone, Default)]
pub struct RouteRegistry {
    builders: RouteBuilders,
    restorable_builders: RestorableRouteBuilders,
}
impl RouteRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&self, location: impl Into<String>, builder: impl Fn() -> Page + 'static) {
        let location = location.into();
        self.builders
            .borrow_mut()
            .insert(normalize_location(&location), Rc::new(builder));
    }
    #[must_use]
    pub fn resolve(&self, location: &str) -> Option<Page> {
        self.builders
            .borrow()
            .get(&normalize_location(location))
            .map(|builder| builder())
    }
    pub fn navigate(&self, navigator: &Navigator, location: &str) -> Option<RouteId> {
        self.resolve(location).map(|page| navigator.push_page(page))
    }

    /// Registers a route builder that may be written into a
    /// [`NavigatorSnapshot`].
    ///
    /// The closure remains live application code and is never serialized. It
    /// is called only after a persisted route ID is resolved at startup, where
    /// it must validate the JSON arguments before producing a normal [`Page`].
    /// A duplicate stable ID is rejected rather than silently replacing a
    /// restored route with a different page.
    pub fn register_restorable(
        &self,
        route_id: impl AsRef<str>,
        builder: impl Fn(&Value) -> Result<Page, RestorableRouteBuildError> + 'static,
    ) -> Result<RestorableRouteId, RestorableRouteRegistrationError> {
        self.register_restorable_with_scope_cleanup(route_id, false, builder)
    }

    /// Registers a restorable route whose stable scope key should be removed
    /// after an explicit permanent [`Navigator::pop`].
    ///
    /// This lifecycle policy applies only when a scope key was supplied while
    /// navigating. Declarative page reconciliation never invokes it, avoiding
    /// accidental deletion when a route is temporarily absent from a build.
    pub fn register_restorable_with_scope_cleanup(
        &self,
        route_id: impl AsRef<str>,
        clear_scope_on_pop: bool,
        builder: impl Fn(&Value) -> Result<Page, RestorableRouteBuildError> + 'static,
    ) -> Result<RestorableRouteId, RestorableRouteRegistrationError> {
        let route_id = RestorableRouteId::new(route_id)
            .map_err(RestorableRouteRegistrationError::InvalidRouteId)?;
        let mut builders = self.restorable_builders.borrow_mut();
        if builders.contains_key(&route_id) {
            return Err(RestorableRouteRegistrationError::DuplicateRouteId(route_id));
        }
        builders.insert(
            route_id.clone(),
            RestorableRouteDefinition {
                builder: Rc::new(builder),
                clear_scope_on_pop,
            },
        );
        Ok(route_id)
    }

    /// Returns whether a stable route ID is currently registered for safe
    /// restoration.
    #[must_use]
    pub fn is_restorable_registered(&self, route_id: &RestorableRouteId) -> bool {
        self.restorable_builders.borrow().contains_key(route_id)
    }

    /// Builds and pushes a registered restorable route.
    ///
    /// The arguments and optional route state are JSON values so the returned
    /// stack snapshot is self-contained and does not capture this builder
    /// closure or its widget tree.
    pub fn navigate_restorable(
        &self,
        navigator: &Navigator,
        route: RestorableRoute,
    ) -> Result<RouteId, RestorableNavigationError> {
        let definition = self
            .restorable_builders
            .borrow()
            .get(&route.route_id)
            .cloned()
            .ok_or_else(|| RestorableNavigationError::UnknownRoute(route.route_id.clone()))?;
        let page = (definition.builder)(&route.arguments)
            .map_err(RestorableNavigationError::BuildFailed)?;
        Ok(navigator.push_registered_restorable(
            page,
            route.with_registered_scope_cleanup(definition.clear_scope_on_pop),
        ))
    }

    /// Restores the longest valid prefix of `snapshot` into `navigator`.
    ///
    /// An unknown route or invalid arguments truncates the saved stack at that
    /// entry. If its root cannot be rebuilt, the current navigator remains
    /// untouched so an application-provided initial route stays valid. Use
    /// [`Self::restore_navigator_or`] when the navigator may start empty and a
    /// fallback root must be installed explicitly.
    pub fn restore_navigator(
        &self,
        navigator: &Navigator,
        snapshot: &NavigatorSnapshot,
    ) -> NavigationRestoreReport {
        if snapshot.format_version != NAVIGATOR_SNAPSHOT_FORMAT_VERSION {
            return NavigationRestoreReport {
                unsupported_format: true,
                invalid_routes: snapshot.routes.len(),
                ..NavigationRestoreReport::default()
            };
        }

        let requested_len = match snapshot.active_route {
            Some(active_route) if active_route < snapshot.routes.len() => active_route + 1,
            Some(_) => {
                return NavigationRestoreReport {
                    invalid_routes: snapshot.routes.len(),
                    ..NavigationRestoreReport::default()
                };
            }
            None => snapshot.routes.len(),
        };
        let mut restored = Vec::with_capacity(requested_len);
        let mut invalid_routes = snapshot.routes.len().saturating_sub(requested_len);
        for (index, route) in snapshot.routes.iter().take(requested_len).enumerate() {
            let Some(definition) = self
                .restorable_builders
                .borrow()
                .get(&route.route_id)
                .cloned()
            else {
                invalid_routes += requested_len - index;
                break;
            };
            let Ok(page) = (definition.builder)(&route.arguments) else {
                invalid_routes += requested_len - index;
                break;
            };
            restored.push((
                page,
                route
                    .clone()
                    .with_registered_scope_cleanup(definition.clear_scope_on_pop),
            ));
        }

        let restored_routes = restored.len();
        if restored_routes != 0 {
            navigator.replace_with_restored_routes(restored);
        }
        NavigationRestoreReport {
            restored_routes,
            invalid_routes,
            ..NavigationRestoreReport::default()
        }
    }

    /// Restores a navigator and installs `fallback` when no saved root can be
    /// rebuilt. This is the recommended startup API for a navigator that has
    /// not already been initialized with a valid root page.
    pub fn restore_navigator_or(
        &self,
        navigator: &Navigator,
        snapshot: &NavigatorSnapshot,
        fallback: impl FnOnce() -> Page,
    ) -> NavigationRestoreReport {
        let mut report = self.restore_navigator(navigator, snapshot);
        if report.restored_routes == 0 {
            navigator.replace_with_fallback(fallback());
            report.used_fallback = true;
        }
        report
    }
}

fn normalize_location(location: &str) -> String {
    let location = location.trim();
    if location.is_empty() || location == "/" {
        "/".to_owned()
    } else if location.starts_with('/') {
        location.trim_end_matches('/').to_owned()
    } else {
        format!("/{}", location.trim_end_matches('/'))
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

    fn push_registered_restorable(&self, page: Page, route: RestorableRoute) -> RouteId {
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

    fn replace_with_restored_routes(&self, routes: Vec<(Page, RestorableRoute)>) {
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

    fn replace_with_fallback(&self, page: Page) {
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
    /// pushed through [`RouteRegistry::navigate_restorable`].
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
    /// empty navigator from a route that deliberately blocked navigation.
    pub fn maybe_pop(&self) -> PopResult {
        let (candidate, guard) = {
            let state = self.state.borrow();
            (state.routes.last().cloned(), state.pop_guard.clone())
        };
        let Some(candidate) = candidate else {
            return PopResult::Empty;
        };
        if guard.is_some_and(|guard| guard(&candidate) == PopDecision::Deny) {
            return PopResult::Blocked;
        }
        let (route, cleanup) = {
            let mut state = self.state.borrow_mut();
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
    pub fn replace(&self, mut route: Route) -> Option<Route> {
        let (candidate, guard) = {
            let state = self.state.borrow();
            (state.routes.last().cloned(), state.pop_guard.clone())
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
    /// `true` when a pop guard consumed the action without mutating a stack.
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

/// Modal overlay configuration. The barrier is input-blocking by default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModalBarrier {
    pub color: Color,
    pub dismissible: bool,
    pub label: Option<String>,
}
impl Default for ModalBarrier {
    fn default() -> Self {
        Self {
            color: Color::rgba(0, 0, 0, 128),
            dismissible: true,
            label: None,
        }
    }
}

/// An entry in painter-ordered overlay state.
#[derive(Clone)]
pub struct OverlayEntry {
    pub child: Widget,
    pub barrier: Option<ModalBarrier>,
}
impl OverlayEntry {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            barrier: None,
        }
    }
    #[must_use]
    pub fn modal(mut self, barrier: ModalBarrier) -> Self {
        self.barrier = Some(barrier);
        self
    }

    #[must_use]
    pub fn blocks_background_input(&self) -> bool {
        self.barrier.is_some()
    }

    #[must_use]
    pub fn dismissible(&self) -> bool {
        self.barrier
            .as_ref()
            .is_none_or(|barrier| barrier.dismissible)
    }
}

/// A modal dialog presentation owned by application overlay state.
#[derive(Clone)]
pub struct Dialog {
    pub child: Widget,
    pub barrier: ModalBarrier,
}
impl Dialog {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            barrier: ModalBarrier::default(),
        }
    }
    #[must_use]
    pub fn barrier(mut self, barrier: ModalBarrier) -> Self {
        self.barrier = barrier;
        self
    }
    #[must_use]
    pub fn into_entry(self) -> OverlayEntry {
        OverlayEntry::new(self.child).modal(self.barrier)
    }
}

/// A modal bottom-sheet presentation. Positioning remains a widget concern.
#[derive(Clone)]
pub struct BottomSheet {
    pub child: Widget,
    pub barrier: ModalBarrier,
}
impl BottomSheet {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            barrier: ModalBarrier::default(),
        }
    }
    #[must_use]
    pub fn barrier(mut self, barrier: ModalBarrier) -> Self {
        self.barrier = barrier;
        self
    }
    #[must_use]
    pub fn into_entry(self) -> OverlayEntry {
        OverlayEntry::new(self.child).modal(self.barrier)
    }
}

/// Cloneable painter-ordered overlay state.
#[derive(Clone, Default)]
pub struct Overlay {
    entries: Rc<RefCell<Vec<OverlayEntry>>>,
}
impl Overlay {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn insert(&self, entry: OverlayEntry) {
        self.entries.borrow_mut().push(entry);
    }
    pub fn show_dialog(&self, dialog: Dialog) {
        self.insert(dialog.into_entry());
    }
    pub fn show_bottom_sheet(&self, sheet: BottomSheet) {
        self.insert(sheet.into_entry());
    }
    pub fn remove_top(&self) -> Option<OverlayEntry> {
        self.entries.borrow_mut().pop()
    }

    /// Removes the top entry only when it has a dismissible barrier.
    pub fn dismiss_top(&self) -> Option<OverlayEntry> {
        let dismissible = self
            .entries
            .borrow()
            .last()
            .is_some_and(OverlayEntry::dismissible);
        dismissible.then(|| self.remove_top()).flatten()
    }

    /// Whether the current overlay stack must consume pointer/keyboard input
    /// before it reaches the route underneath it.
    #[must_use]
    pub fn blocks_background_input(&self) -> bool {
        self.entries
            .borrow()
            .iter()
            .rev()
            .any(OverlayEntry::blocks_background_input)
    }
    #[must_use]
    pub fn entries(&self) -> Vec<OverlayEntry> {
        self.entries.borrow().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use incular_core::Size;
    use serde_json::json;
    use std::{cell::RefCell, rc::Rc};

    fn page() -> Widget {
        Widget::fixed_box(Size::new(1., 1.), Color::WHITE)
    }

    #[test]
    fn route_settings_round_trip_typed_arguments_and_scope() {
        #[derive(Debug, PartialEq, Serialize, Deserialize)]
        struct Args {
            document: u64,
        }
        let scope = RouteScopeKey::new("document-7").unwrap();
        let settings = RouteSettings::new("/editor")
            .with_arguments(&Args { document: 7 })
            .unwrap()
            .restoration_scope(scope.clone());
        assert_eq!(settings.name(), "/editor");
        assert_eq!(settings.arguments::<Args>().unwrap(), Args { document: 7 });
        assert_eq!(settings.restoration_scope_key(), Some(&scope));
    }

    #[test]
    fn composable_route_presentations_and_typed_result_are_retained() {
        let route = PageRouteBuilder::new("settings", page())
            .settings(RouteSettings::new("settings"))
            .build()
            .modal(ModalBarrier {
                dismissible: false,
                ..ModalBarrier::default()
            });
        assert!(route.presentation.blocks_background_input());
        assert!(!route.presentation.is_opaque());

        let result = RouteResult::<bool>::new();
        assert!(!result.is_complete());
        assert!(result.complete(true).is_ok());
        assert_eq!(result.complete(false), Err(false));
        assert_eq!(result.take(), Some(true));
    }

    #[test]
    fn modal_overlay_blocks_input_and_respects_dismissibility() {
        let overlay = Overlay::new();
        overlay.insert(OverlayEntry::new(page()));
        assert!(!overlay.blocks_background_input());
        overlay.insert(OverlayEntry::new(page()).modal(ModalBarrier {
            dismissible: false,
            ..ModalBarrier::default()
        }));
        assert!(overlay.blocks_background_input());
        assert!(overlay.dismiss_top().is_none());
        assert!(overlay.remove_top().is_some());
        assert!(!overlay.blocks_background_input());
    }
    #[test]
    fn navigator_is_a_lifo_stack() {
        let navigator = Navigator::new();
        navigator.push(Route::new("home", page()));
        navigator.push(Route::new("details", page()));
        assert_eq!(navigator.current().unwrap().name, "details");
        assert_eq!(navigator.pop().unwrap().name, "details");
    }
    #[test]
    fn observers_receive_stack_and_active_route_lifecycle() {
        let navigator = Navigator::new();
        let events = Rc::new(RefCell::new(Vec::<String>::new()));
        let observer = navigator.observe({
            let events = Rc::clone(&events);
            move |event| match event {
                NavigationEvent::Pushed { route } => {
                    events.borrow_mut().push(format!("push:{}", route.name));
                }
                NavigationEvent::Popped { route } => {
                    events.borrow_mut().push(format!("pop:{}", route.name));
                }
                NavigationEvent::Replaced { previous, route } => events
                    .borrow_mut()
                    .push(format!("replace:{}:{}", previous.name, route.name)),
                NavigationEvent::ActiveRouteChanged { current, .. } => {
                    events.borrow_mut().push(format!(
                        "active:{}",
                        current.map_or_else(|| "none".to_owned(), |route| route.name)
                    ))
                }
            }
        });
        navigator.push_page(Page::new("home", page()));
        navigator.push_page(Page::new("details", page()));
        navigator.replace(Route::new("settings", page()));
        navigator.pop();
        assert_eq!(
            *events.borrow(),
            [
                "push:home",
                "active:home",
                "push:details",
                "active:details",
                "replace:details:settings",
                "active:settings",
                "pop:settings",
                "active:home",
            ]
            .map(str::to_owned)
        );
        drop(observer);
        navigator.push_page(Page::new("ignored", page()));
        assert_eq!(events.borrow().len(), 8);
    }
    #[test]
    fn pop_guard_blocks_without_mutating_a_route_stack() {
        let navigator = Navigator::new();
        navigator.push_page(Page::new("home", page()));
        navigator.push_page(Page::new("editor", page()));
        navigator.set_pop_guard(|route| {
            if route.name == "editor" {
                PopDecision::Deny
            } else {
                PopDecision::Allow
            }
        });
        assert!(matches!(navigator.maybe_pop(), PopResult::Blocked));
        assert_eq!(navigator.current().unwrap().name, "editor");
        navigator.clear_pop_guard();
        assert!(matches!(navigator.maybe_pop(), PopResult::Popped(_)));
        assert_eq!(navigator.current().unwrap().name, "home");
    }
    #[test]
    fn deepest_active_nested_dispatcher_handles_back_once() {
        let root_navigator = Navigator::new();
        root_navigator.push_page(Page::new("home", page()));
        root_navigator.push_page(Page::new("settings", page()));
        let child_navigator = Navigator::new();
        child_navigator.push_page(Page::new("overview", page()));
        child_navigator.push_page(Page::new("detail", page()));
        let root = BackDispatcher::new(root_navigator.clone());
        let child = BackDispatcher::new(child_navigator.clone());
        root.attach_child(&child);
        root.set_active_child(Some(&child));

        let first = root.dispatch_back();
        assert_eq!(first.depth, 1);
        assert!(first.handled);
        assert_eq!(child_navigator.current().unwrap().name, "overview");
        assert_eq!(root_navigator.current().unwrap().name, "settings");

        let second = root.dispatch_back();
        assert_eq!(second.depth, 0);
        assert!(second.handled);
        assert_eq!(root_navigator.current().unwrap().name, "home");

        let third = root.dispatch_back();
        assert!(!third.handled);
        assert_eq!(third.depth, 0);
    }
    #[test]
    fn declarative_pages_preserve_named_route_identity() {
        let navigator = Navigator::new();
        navigator.set_pages([Page::new("home", page()), Page::new("settings", page())]);
        let id = navigator.routes()[0].id;
        navigator.set_pages([Page::new("home", page())]);
        assert_eq!(navigator.routes()[0].id, id);
    }
    #[test]
    fn registry_resolves_a_normalized_location() {
        let registry = RouteRegistry::new();
        registry.register("/settings", || Page::new("settings", page()));
        let navigator = Navigator::new();
        assert!(registry.navigate(&navigator, "settings/").is_some());
    }
    #[test]
    fn transitions_wrap_route_children_in_retained_layers() {
        let route = Route::new("details", page()).transition(RouteTransition::FadeSlide {
            opacity: OpacityController::new(),
            translation: TranslationController::new(),
        });
        let _: Widget = route.presented_child();
    }
    #[test]
    fn dialogs_and_bottom_sheets_are_modal_entries() {
        let overlay = Overlay::new();
        overlay.show_dialog(Dialog::new(page()));
        overlay.show_bottom_sheet(BottomSheet::new(page()).barrier(ModalBarrier {
            dismissible: false,
            ..ModalBarrier::default()
        }));
        let entries = overlay.entries();
        assert!(entries[0].barrier.as_ref().unwrap().dismissible);
        assert!(!entries[1].barrier.as_ref().unwrap().dismissible);
    }

    fn restorable_page(arguments: &Value) -> Result<Page, RestorableRouteBuildError> {
        let name = arguments
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| RestorableRouteBuildError::invalid_arguments("missing name"))?;
        Ok(Page::new(name, page()))
    }

    #[test]
    fn restorable_stack_round_trips_as_declarative_data() {
        let registry = RouteRegistry::new();
        let home = registry
            .register_restorable("/home", restorable_page)
            .unwrap();
        let detail = registry
            .register_restorable("/detail", restorable_page)
            .unwrap();
        let navigator = Navigator::new();
        registry
            .navigate_restorable(
                &navigator,
                RestorableRoute::new(home, json!({ "name": "home" }))
                    .state(json!({ "tab": "recent" })),
            )
            .unwrap();
        registry
            .navigate_restorable(
                &navigator,
                RestorableRoute::new(detail, json!({ "name": "detail" }))
                    .state(json!({ "document": 42 })),
            )
            .unwrap();
        assert!(navigator.set_current_restorable_state(json!({ "document": 43 })));

        let snapshot = navigator.restoration_snapshot();
        assert_eq!(snapshot.active_route, Some(1));
        let serialized = serde_json::to_value(&snapshot).unwrap();
        assert!(serialized.get("child").is_none());
        assert_eq!(serialized["routes"][1]["route_id"], "/detail");
        let snapshot: NavigatorSnapshot = serde_json::from_value(serialized).unwrap();

        let restored = Navigator::new();
        let report = registry.restore_navigator(&restored, &snapshot);
        assert_eq!(report.restored_routes, 2);
        assert_eq!(report.invalid_routes, 0);
        assert_eq!(
            restored
                .routes()
                .into_iter()
                .map(|route| route.name)
                .collect::<Vec<_>>(),
            ["home", "detail"]
        );
        assert_eq!(
            restored.current_restorable_route().unwrap().state,
            json!({ "document": 43 })
        );
    }

    #[test]
    fn restoration_truncates_invalid_tail_and_keeps_a_safe_root() {
        let registry = RouteRegistry::new();
        let home = registry
            .register_restorable("/home", restorable_page)
            .unwrap();
        let snapshot = NavigatorSnapshot {
            format_version: NAVIGATOR_SNAPSHOT_FORMAT_VERSION,
            routes: vec![
                RestorableRoute::new(home, json!({ "name": "home" })),
                RestorableRoute::new(
                    RestorableRouteId::new("/removed-after-update").unwrap(),
                    json!({ "name": "removed" }),
                ),
            ],
            active_route: Some(1),
        };
        let navigator = Navigator::new();
        let report =
            registry.restore_navigator_or(&navigator, &snapshot, || Page::new("fallback", page()));
        assert_eq!(report.restored_routes, 1);
        assert_eq!(report.invalid_routes, 1);
        assert!(!report.used_fallback);
        assert_eq!(navigator.current().unwrap().name, "home");

        let missing_root = NavigatorSnapshot {
            routes: vec![RestorableRoute::new(
                RestorableRouteId::new("/gone").unwrap(),
                json!({ "name": "gone" }),
            )],
            active_route: Some(0),
            ..NavigatorSnapshot::default()
        };
        let report = registry
            .restore_navigator_or(&navigator, &missing_root, || Page::new("fallback", page()));
        assert_eq!(report.restored_routes, 0);
        assert_eq!(report.invalid_routes, 1);
        assert!(report.used_fallback);
        assert_eq!(navigator.current().unwrap().name, "fallback");
    }

    #[test]
    fn snapshot_excludes_transient_routes_and_their_suffix() {
        let registry = RouteRegistry::new();
        let home = registry
            .register_restorable("/home", restorable_page)
            .unwrap();
        let detail = registry
            .register_restorable("/detail", restorable_page)
            .unwrap();
        let navigator = Navigator::new();
        registry
            .navigate_restorable(
                &navigator,
                RestorableRoute::new(home, json!({ "name": "home" })),
            )
            .unwrap();
        navigator.push_page(Page::new("transient-dialog-route", page()));
        registry
            .navigate_restorable(
                &navigator,
                RestorableRoute::new(detail, json!({ "name": "detail" })),
            )
            .unwrap();

        let snapshot = navigator.restoration_snapshot();
        assert_eq!(snapshot.routes.len(), 1);
        assert_eq!(snapshot.routes[0].route_id.as_str(), "/home");
        assert_eq!(snapshot.active_route, Some(0));
    }

    #[test]
    fn registered_pop_can_clean_up_only_a_stable_route_scope() {
        let registry = RouteRegistry::new();
        let document = registry
            .register_restorable_with_scope_cleanup("/document", true, restorable_page)
            .unwrap();
        let navigator = Navigator::new();
        let removed = Rc::new(RefCell::new(Vec::new()));
        navigator.set_route_scope_cleanup({
            let removed = Rc::clone(&removed);
            move |key| removed.borrow_mut().push(key.to_string())
        });
        registry
            .navigate_restorable(
                &navigator,
                RestorableRoute::new(document, json!({ "name": "document" }))
                    .scope_key(RouteScopeKey::new("document-42").unwrap()),
            )
            .unwrap();
        navigator.pop();
        assert_eq!(&*removed.borrow(), &["document-42"]);

        registry
            .navigate_restorable(
                &navigator,
                RestorableRoute::new(
                    RestorableRouteId::new("/document").unwrap(),
                    json!({ "name": "document" }),
                )
                .scope_key(RouteScopeKey::new("temporary-document").unwrap()),
            )
            .unwrap();
        navigator.set_pages([]);
        assert_eq!(&*removed.borrow(), &["document-42"]);
    }

    #[test]
    fn restorable_registry_rejects_duplicate_normalized_ids() {
        let registry = RouteRegistry::new();
        registry
            .register_restorable("settings/", restorable_page)
            .unwrap();
        let duplicate = registry.register_restorable("/settings", restorable_page);
        assert!(matches!(
            duplicate,
            Err(RestorableRouteRegistrationError::DuplicateRouteId(id)) if id.as_str() == "/settings"
        ));
    }

    #[test]
    fn future_navigator_snapshot_falls_back_without_panic() {
        let navigator = Navigator::new();
        let report = RouteRegistry::new().restore_navigator_or(
            &navigator,
            &NavigatorSnapshot {
                format_version: NAVIGATOR_SNAPSHOT_FORMAT_VERSION + 1,
                ..NavigatorSnapshot::default()
            },
            || Page::new("fallback", page()),
        );
        assert!(report.unsupported_format);
        assert!(report.used_fallback);
        assert_eq!(navigator.current().unwrap().name, "fallback");
    }

    #[test]
    fn malformed_persisted_keys_are_rejected_by_serde() {
        let malformed = r#"{
            "format_version": 1,
            "routes": [{
                "route_id": "/bad\u0000route",
                "arguments": null,
                "state": null
            }]
        }"#;
        assert!(serde_json::from_str::<NavigatorSnapshot>(malformed).is_err());
    }
}
