use std::{cell::RefCell, collections::HashMap, rc::Rc};

use serde_json::Value;

use super::route_data::normalize_location;
use super::{
    NAVIGATOR_SNAPSHOT_FORMAT_VERSION, NavigationRestoreReport, Navigator, NavigatorSnapshot, Page,
    RestorableNavigationError, RestorableRoute, RestorableRouteBuildError, RestorableRouteId,
    RestorableRouteRegistrationError, RouteId,
};

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
