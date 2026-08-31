use std::fmt;

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

/// Stable identity for a route in a navigator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RouteId(pub(crate) u64);

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
/// identity used by a live navigator. `RestorableRouteId` is persisted and
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
/// closure used to build a page, the resulting widget, or a live
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
    /// route registry.
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

    pub(crate) fn removes_scope_on_pop(&self) -> bool {
        self.clear_scope_on_pop
    }

    pub(crate) fn with_registered_scope_cleanup(mut self, clear_scope_on_pop: bool) -> Self {
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

pub(crate) fn normalize_location(location: &str) -> String {
    let location = location.trim();
    if location.is_empty() || location == "/" {
        "/".to_owned()
    } else if location.starts_with('/') {
        location.trim_end_matches('/').to_owned()
    } else {
        format!("/{}", location.trim_end_matches('/'))
    }
}
