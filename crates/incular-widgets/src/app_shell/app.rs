//! Application bootstrap and the `WidgetsApp` route/localization shell.
//!
//! `WidgetsApp` owns the policy that Flutter normally installs around the
//! root navigator: platform initial-route defaults, named-route expansion,
//! restoration, back dispatch, locale selection, directionality, media
//! metrics, and title updates.  The final hand-off to `incular-runtime` is an
//! explicit [`ApplicationBootstrapHost`] implementation so this crate keeps
//! its dependency direction intact.

use super::{
    BackCallbackSubscription, CheckedModeBanner, Directionality, ErrorWidget, Localizations,
    MediaQuery, MemoryRouteInformationProvider, NavigationNotification, NavigationNotificationKind,
    NavigationNotificationListener, NavigationNotificationSubscription, RootBackButtonDispatcher,
    RouteInformation, RouteInformationProvider, RouteInformationReportingType, Router, RouterError,
    SizedBox, Title, TitleController, Widget, WindowChromeSink,
};
use incular_config::{
    ApplicationDefaults, Locale, LocaleResolver, LocalizationCatalog, RuntimeEnvironment,
};
use incular_core::{Color, RestorationKey, RestorationScope, Size};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    fmt,
    rc::{Rc, Weak},
};

type HomeBuilder = Rc<dyn Fn() -> Widget>;
type RouteBuilder = Rc<dyn Fn() -> Widget>;
type GenerateRoute = Rc<dyn Fn(&RouteInformation) -> Option<Widget>>;
type UnknownRoute = Rc<dyn Fn(&RouteInformation) -> Widget>;
type GeneratedTitle = Rc<dyn Fn(&Locale) -> String>;

/// Portable window options used by application bootstrap adapters.
#[derive(Clone, Debug, PartialEq)]
pub struct ApplicationBootstrapOptions {
    pub title: String,
    pub initial_size: Size,
    pub resizable: bool,
    pub visible: bool,
    pub decorations: bool,
    pub transparent: bool,
    pub maximized: bool,
}

impl Default for ApplicationBootstrapOptions {
    fn default() -> Self {
        let defaults = ApplicationDefaults::DEFAULT;
        Self {
            title: defaults.window_title.to_owned(),
            initial_size: defaults.initial_window_size,
            resizable: defaults.resizable,
            visible: defaults.visible,
            decorations: defaults.decorations,
            transparent: defaults.transparent,
            maximized: defaults.maximized,
        }
    }
}

/// Complete root specification passed to a runtime bootstrap adapter.
#[derive(Clone, Debug)]
pub struct ApplicationBootstrapSpec {
    pub options: ApplicationBootstrapOptions,
    pub environment: RuntimeEnvironment,
    pub root: Widget,
    pub restoration_scope_id: Option<String>,
}

/// Runtime-owned application creation boundary.
///
/// The runtime adapter should map [`ApplicationBootstrapOptions`] to
/// `incular-platform::WindowOptions`, install `root` in its first
/// `BuildContext`, and use `restoration_scope_id` when creating a restorable
/// application.  Keeping this trait here avoids a widgets/runtime dependency
/// cycle while making bootstrap behavior testable.
pub trait ApplicationBootstrapHost {
    type Handle;
    type Error;

    fn create_application(
        &self,
        specification: ApplicationBootstrapSpec,
    ) -> Result<Self::Handle, Self::Error>;
}

/// A route controller used by the navigator-shaped `WidgetsApp` constructor.
#[derive(Clone)]
pub struct WidgetsAppController {
    inner: Rc<RefCell<WidgetsAppState>>,
}

impl fmt::Debug for WidgetsAppController {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.inner.borrow();
        formatter
            .debug_struct("WidgetsAppController")
            .field("current_route", &state.stack.last())
            .field("stack_depth", &state.stack.len())
            .field("route_count", &state.routes.len())
            .finish()
    }
}

struct WidgetsAppState {
    provider: MemoryRouteInformationProvider,
    provider_subscription: Option<super::RouteInformationSubscription>,
    root_back_dispatcher: RootBackButtonDispatcher,
    back_subscription: Option<BackCallbackSubscription>,
    home: Option<HomeBuilder>,
    routes: BTreeMap<String, RouteBuilder>,
    generate_route: Option<GenerateRoute>,
    unknown_route: Option<UnknownRoute>,
    initial_route: Option<String>,
    stack: Vec<RouteInformation>,
    initialized: bool,
    revision: Rc<Cell<u64>>,
    listeners: Vec<Weak<dyn Fn(NavigationNotification)>>,
    restoration: Option<AppRestoration>,
}

#[derive(Clone)]
struct AppRestoration {
    scope: RestorationScope,
    key: RestorationKey,
}

impl WidgetsAppController {
    /// Creates a controller using the platform root route as its default.
    #[must_use]
    pub fn new() -> Self {
        Self::with_provider(MemoryRouteInformationProvider::root())
    }

    /// Creates a controller with a retained platform route provider.
    #[must_use]
    pub fn with_provider(provider: MemoryRouteInformationProvider) -> Self {
        let inner = Rc::new(RefCell::new(WidgetsAppState {
            provider: provider.clone(),
            provider_subscription: None,
            root_back_dispatcher: RootBackButtonDispatcher::new(),
            back_subscription: None,
            home: None,
            routes: BTreeMap::new(),
            generate_route: None,
            unknown_route: None,
            initial_route: None,
            stack: Vec::new(),
            initialized: false,
            revision: Rc::new(Cell::new(0)),
            listeners: Vec::new(),
            restoration: None,
        }));

        let weak = Rc::downgrade(&inner);
        let provider_subscription = provider.subscribe(Rc::new(move |information| {
            if let Some(inner) = weak.upgrade() {
                WidgetsAppController::receive_platform_route(&inner, information);
            }
        }));
        let weak = Rc::downgrade(&inner);
        let back_subscription = inner.borrow().root_back_dispatcher.add_callback(move || {
            weak.upgrade()
                .is_some_and(|inner| WidgetsAppController::pop_inner(&inner))
        });
        {
            let mut state = inner.borrow_mut();
            state.provider_subscription = Some(provider_subscription);
            state.back_subscription = Some(back_subscription);
        }
        Self { inner }
    }

    /// Returns the provider used for external deep-link updates.
    #[must_use]
    pub fn route_information_provider(&self) -> MemoryRouteInformationProvider {
        self.inner.borrow().provider.clone()
    }

    /// Installs a home builder for the `/` route.
    pub fn set_home(&self, builder: impl Fn() -> Widget + 'static) {
        self.inner.borrow_mut().home = Some(Rc::new(builder));
        self.invalidate();
    }

    /// Installs a fixed home widget while retaining its route state.
    pub fn set_home_widget(&self, widget: Widget) {
        self.set_home(move || widget.clone());
    }

    /// Registers or replaces one named route.
    pub fn register_route(
        &self,
        location: impl AsRef<str>,
        builder: impl Fn() -> Widget + 'static,
    ) {
        self.inner
            .borrow_mut()
            .routes
            .insert(route_path(location.as_ref()), Rc::new(builder));
        self.invalidate();
    }

    /// Alias matching Flutter's `routes` vocabulary.
    pub fn route(&self, location: impl AsRef<str>, builder: impl Fn() -> Widget + 'static) {
        self.register_route(location, builder);
    }

    /// Installs a route generator consulted after the table.
    pub fn on_generate_route(
        &self,
        builder: impl Fn(&RouteInformation) -> Option<Widget> + 'static,
    ) {
        self.inner.borrow_mut().generate_route = Some(Rc::new(builder));
        self.invalidate();
    }

    /// Installs the final unknown-route fallback.
    pub fn on_unknown_route(&self, builder: impl Fn(&RouteInformation) -> Widget + 'static) {
        self.inner.borrow_mut().unknown_route = Some(Rc::new(builder));
        self.invalidate();
    }

    /// Sets Flutter's platform/deep-link initial route override.
    pub fn set_initial_route(&self, location: impl Into<String>) {
        self.inner.borrow_mut().initial_route = Some(location.into());
        self.reset_initialization();
    }

    /// Enables restoration of the route stack at a stable runtime scope/key.
    pub fn set_restoration_scope(&self, scope: RestorationScope, key: RestorationKey) {
        self.inner.borrow_mut().restoration = Some(AppRestoration { scope, key });
        self.reset_initialization();
    }

    /// Subscribes to route and back notifications.
    pub fn on_navigation_notification(
        &self,
        listener: impl Fn(NavigationNotification) + 'static,
    ) -> NavigationNotificationSubscription {
        let listener: NavigationNotificationListener = Rc::new(listener);
        self.inner
            .borrow_mut()
            .listeners
            .push(Rc::downgrade(&listener));
        NavigationNotificationSubscription {
            _listener: Some(listener),
        }
    }

    /// Returns the current route, initializing the platform route on demand.
    #[must_use]
    pub fn current_route(&self) -> RouteInformation {
        Self::ensure_initialized(&self.inner);
        self.inner
            .borrow()
            .stack
            .last()
            .cloned()
            .unwrap_or_default()
    }

    /// Returns a copy of the current route stack.
    #[must_use]
    pub fn route_stack(&self) -> Vec<RouteInformation> {
        Self::ensure_initialized(&self.inner);
        self.inner.borrow().stack.clone()
    }

    /// Returns whether a system back request can pop this app controller.
    #[must_use]
    pub fn can_pop(&self) -> bool {
        Self::ensure_initialized(&self.inner);
        self.inner.borrow().stack.len() > 1
    }

    /// Pushes a route and reports it to the retained provider.
    pub fn push_route(&self, location: impl Into<RouteInformation>) -> Result<(), RouterError> {
        Self::ensure_initialized(&self.inner);
        Self::push_inner(&self.inner, location.into(), false)
    }

    /// Replaces the current route and reports it without growing the stack.
    pub fn replace_route(&self, location: impl Into<RouteInformation>) -> Result<(), RouterError> {
        Self::ensure_initialized(&self.inner);
        Self::replace_inner(&self.inner, location.into(), false)
    }

    /// Applies a platform route as a deep link, using Flutter's prefix-stack
    /// behavior and falling back to `/` when the final route is unavailable.
    pub fn set_route_information(
        &self,
        information: impl Into<RouteInformation>,
    ) -> Result<(), RouterError> {
        Self::ensure_initialized(&self.inner);
        Self::replace_stack(&self.inner, information.into(), true)
    }

    /// Dispatches a native system back event through the root dispatcher.
    #[must_use]
    pub fn dispatch_back(&self) -> bool {
        Self::ensure_initialized(&self.inner);
        self.inner.borrow().root_back_dispatcher.dispatch_back()
    }

    /// Builds the currently selected named-route subtree.
    #[must_use]
    pub fn build_current_widget(&self) -> Widget {
        Self::ensure_initialized(&self.inner);
        let information = self.current_route();
        self.widget_for(&information).unwrap_or_else(|| {
            ErrorWidget::with_message(format!("No route found for {}", information.location()))
                .into()
        })
    }

    /// Builds a retained route widget whose local revision changes only when
    /// navigation state changes.
    #[must_use]
    pub fn into_widget(self) -> Widget {
        let revision = self.inner.borrow().revision.clone();
        let retained = self.clone();
        Widget::stateful_layout_builder(revision, move |_| {
            let information = retained.current_route();
            Widget::environment_scope(
                WidgetsAppData {
                    current_route: information,
                    can_pop: retained.can_pop(),
                    revision: retained.inner.borrow().revision.get(),
                },
                retained.build_current_widget(),
            )
        })
    }

    /// Alias for [`Self::into_widget`].
    #[must_use]
    pub fn widget(self) -> Widget {
        self.into_widget()
    }

    fn widget_for(&self, information: &RouteInformation) -> Option<Widget> {
        let (home, route, generator) = {
            let state = self.inner.borrow();
            (
                state.home.clone(),
                state
                    .routes
                    .get(&route_path(information.location()))
                    .cloned(),
                state.generate_route.clone(),
            )
        };
        if route_path(information.location()) == "/" {
            if let Some(home) = home {
                return Some(home());
            }
        }
        if let Some(route) = route {
            return Some(route());
        }
        generator.and_then(|generator| generator(information))
    }

    fn route_exists(inner: &Rc<RefCell<WidgetsAppState>>, information: &RouteInformation) -> bool {
        let controller = Self {
            inner: inner.clone(),
        };
        controller.widget_for(information).is_some()
    }

    fn ensure_initialized(inner: &Rc<RefCell<WidgetsAppState>>) {
        if inner.borrow().initialized {
            return;
        }
        let (restoration, provider, initial_route) = {
            let state = inner.borrow();
            (
                state.restoration.clone(),
                state.provider.clone(),
                state.initial_route.clone(),
            )
        };
        if let Some(restoration) = restoration {
            if let Some(value) = restoration.scope.get_json(&restoration.key) {
                if let Some(array) = value.as_array() {
                    let restored = array
                        .iter()
                        .filter_map(|value| RouteInformation::from_json(value).ok())
                        .collect::<Vec<_>>();
                    if !restored.is_empty() {
                        let mut state = inner.borrow_mut();
                        state.stack = restored;
                        state.initialized = true;
                        state.revision.set(state.revision.get().wrapping_add(1));
                        return;
                    }
                }
            }
        }
        let target = initial_route
            .map(RouteInformation::new)
            .unwrap_or_else(|| provider.value());
        let stack = Self::initial_stack(inner, target);
        {
            let mut state = inner.borrow_mut();
            state.stack = stack;
            state.initialized = true;
            state.revision.set(state.revision.get().wrapping_add(1));
        }
        Self::persist(inner);
    }

    fn initial_stack(
        inner: &Rc<RefCell<WidgetsAppState>>,
        target: RouteInformation,
    ) -> Vec<RouteInformation> {
        let path = route_path(target.location());
        if path == "/" {
            return vec![RouteInformation::new("/")];
        }
        let prefixes = route_prefixes(&path);
        if !Self::route_exists(inner, &target) {
            return vec![RouteInformation::new("/")];
        }
        let mut stack = vec![RouteInformation::new("/")];
        for prefix in prefixes {
            let candidate = if prefix == path {
                target.clone()
            } else {
                RouteInformation::new(prefix)
            };
            if Self::route_exists(inner, &candidate) {
                stack.push(candidate);
            }
        }
        stack
    }

    fn receive_platform_route(inner: &Rc<RefCell<WidgetsAppState>>, information: RouteInformation) {
        if !inner.borrow().initialized {
            return;
        }
        let _ = Self::replace_stack(inner, information, true);
    }

    fn push_inner(
        inner: &Rc<RefCell<WidgetsAppState>>,
        information: RouteInformation,
        platform: bool,
    ) -> Result<(), RouterError> {
        if !Self::route_exists(inner, &information) {
            let has_unknown = inner.borrow().unknown_route.is_some();
            if !has_unknown {
                return Err(RouterError::Delegate(format!(
                    "no route found for {}",
                    information.location()
                )));
            }
        }
        {
            let mut state = inner.borrow_mut();
            state.stack.push(information.clone());
            state.revision.set(state.revision.get().wrapping_add(1));
        }
        if platform {
            let _ = inner
                .borrow()
                .provider
                .router_reports_new_route_information(
                    information.clone(),
                    RouteInformationReportingType::Navigate,
                );
        }
        Self::persist(inner);
        Self::notify(
            inner,
            information,
            NavigationNotificationKind::RouteInformationChanged,
        );
        Ok(())
    }

    fn replace_inner(
        inner: &Rc<RefCell<WidgetsAppState>>,
        information: RouteInformation,
        platform: bool,
    ) -> Result<(), RouterError> {
        if !Self::route_exists(inner, &information) && inner.borrow().unknown_route.is_none() {
            return Err(RouterError::Delegate(format!(
                "no route found for {}",
                information.location()
            )));
        }
        {
            let mut state = inner.borrow_mut();
            if state.stack.is_empty() {
                state.stack.push(information.clone());
            } else {
                let last = state.stack.len() - 1;
                state.stack[last] = information.clone();
            }
            state.revision.set(state.revision.get().wrapping_add(1));
        }
        if platform {
            let _ = inner
                .borrow()
                .provider
                .router_reports_new_route_information(
                    information.clone(),
                    RouteInformationReportingType::Neglect,
                );
        }
        Self::persist(inner);
        Self::notify(
            inner,
            information,
            NavigationNotificationKind::RouteInformationChanged,
        );
        Ok(())
    }

    fn replace_stack(
        inner: &Rc<RefCell<WidgetsAppState>>,
        information: RouteInformation,
        platform: bool,
    ) -> Result<(), RouterError> {
        let stack = Self::initial_stack(inner, information.clone());
        {
            let mut state = inner.borrow_mut();
            state.stack = stack;
            state.revision.set(state.revision.get().wrapping_add(1));
        }
        if platform {
            let _ = inner
                .borrow()
                .provider
                .router_reports_new_route_information(
                    information.clone(),
                    RouteInformationReportingType::Neglect,
                );
        }
        Self::persist(inner);
        Self::notify(
            inner,
            inner.borrow().stack.last().cloned().unwrap_or_default(),
            NavigationNotificationKind::RouteInformationChanged,
        );
        Ok(())
    }

    fn pop_inner(inner: &Rc<RefCell<WidgetsAppState>>) -> bool {
        let popped = {
            let mut state = inner.borrow_mut();
            if state.stack.len() <= 1 {
                return false;
            }
            state.stack.pop();
            state.revision.set(state.revision.get().wrapping_add(1));
            state.stack.last().cloned().unwrap_or_default()
        };
        let _ = inner
            .borrow()
            .provider
            .router_reports_new_route_information(
                popped.clone(),
                RouteInformationReportingType::Neglect,
            );
        Self::persist(inner);
        Self::notify(inner, popped, NavigationNotificationKind::BackHandled);
        true
    }

    fn persist(inner: &Rc<RefCell<WidgetsAppState>>) {
        let (restoration, stack) = {
            let state = inner.borrow();
            (state.restoration.clone(), state.stack.clone())
        };
        if let Some(restoration) = restoration {
            let value = stack
                .iter()
                .map(RouteInformation::to_json)
                .collect::<Vec<_>>();
            restoration.scope.set_json(&restoration.key, value.into());
        }
    }

    fn notify(
        inner: &Rc<RefCell<WidgetsAppState>>,
        information: RouteInformation,
        kind: NavigationNotificationKind,
    ) {
        let (listeners, can_pop) = {
            let mut state = inner.borrow_mut();
            state
                .listeners
                .retain(|listener| listener.strong_count() > 0);
            (
                state
                    .listeners
                    .iter()
                    .filter_map(Weak::upgrade)
                    .collect::<Vec<_>>(),
                state.stack.len() > 1,
            )
        };
        let notification = NavigationNotification {
            kind,
            route_information: Some(information),
            can_handle_pop: can_pop,
            error: None,
        };
        for listener in listeners {
            listener(notification.clone());
        }
    }

    fn invalidate(&self) {
        let revision = self.inner.borrow().revision.clone();
        revision.set(revision.get().wrapping_add(1));
    }

    fn reset_initialization(&self) {
        {
            let mut state = self.inner.borrow_mut();
            state.initialized = false;
            state.stack.clear();
        }
        self.invalidate();
    }
}

impl Default for WidgetsAppController {
    fn default() -> Self {
        Self::new()
    }
}

/// Ambient app-shell route state.
#[derive(Clone, Debug, PartialEq)]
pub struct WidgetsAppData {
    pub current_route: RouteInformation,
    pub can_pop: bool,
    pub revision: u64,
}

/// Application-level localization selection and root-widget policy.
#[derive(Clone)]
pub struct WidgetsApp {
    controller: WidgetsAppController,
    environment: RuntimeEnvironment,
    title: String,
    generated_title: Option<GeneratedTitle>,
    title_controller: TitleController,
    title_sink: Option<Rc<dyn WindowChromeSink>>,
    color: Color,
    supported_locales: Vec<Locale>,
    catalog: Option<Rc<dyn LocalizationCatalog>>,
    builder: Option<Rc<dyn Fn(Widget) -> Widget>>,
    router_root: Option<Widget>,
    show_checked_mode_banner: bool,
    restoration_scope_id: Option<String>,
}

impl fmt::Debug for WidgetsApp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WidgetsApp")
            .field("title", &self.title)
            .field("supported_locales", &self.supported_locales)
            .field("has_catalog", &self.catalog.is_some())
            .field("has_router_root", &self.router_root.is_some())
            .finish()
    }
}

impl WidgetsApp {
    /// Creates the navigator-shaped app with Flutter's stable bootstrap
    /// defaults and a home widget at `/`.
    #[must_use]
    pub fn new(home: impl Into<Widget>) -> Self {
        let defaults = ApplicationDefaults::DEFAULT;
        let controller = WidgetsAppController::new();
        controller.set_home_widget(home.into());
        Self {
            controller,
            environment: RuntimeEnvironment::default(),
            title: defaults.window_title.to_owned(),
            generated_title: None,
            title_controller: TitleController::new(defaults.window_title, Color::BLACK),
            title_sink: None,
            color: Color::BLACK,
            supported_locales: Vec::new(),
            catalog: None,
            builder: None,
            router_root: None,
            show_checked_mode_banner: false,
            restoration_scope_id: None,
        }
    }

    /// Creates an app shell whose root is supplied by a [`Router`].
    #[must_use]
    pub fn router<T: 'static>(router: Router<T>) -> Self {
        let mut app = Self::new(SizedBox::shrink());
        app.router_root = Some(router.into_widget());
        app
    }

    /// Creates an empty app whose route table can be filled incrementally.
    #[must_use]
    pub fn empty() -> Self {
        Self::new(SizedBox::shrink())
    }

    #[must_use]
    pub fn controller(&self) -> WidgetsAppController {
        self.controller.clone()
    }

    /// Replaces the app's home route.
    #[must_use]
    pub fn home(self, home: impl Into<Widget>) -> Self {
        self.controller.set_home_widget(home.into());
        self
    }

    /// Adds one named route to the app.
    #[must_use]
    pub fn route(self, location: impl AsRef<str>, builder: impl Fn() -> Widget + 'static) -> Self {
        self.controller.register_route(location, builder);
        self
    }

    /// Adds a fixed named route widget.
    #[must_use]
    pub fn route_widget(self, location: impl AsRef<str>, widget: Widget) -> Self {
        self.controller
            .register_route(location, move || widget.clone());
        self
    }

    #[must_use]
    pub fn initial_route(self, location: impl Into<String>) -> Self {
        self.controller.set_initial_route(location);
        self
    }

    #[must_use]
    pub fn on_generate_route(
        self,
        builder: impl Fn(&RouteInformation) -> Option<Widget> + 'static,
    ) -> Self {
        self.controller.on_generate_route(builder);
        self
    }

    #[must_use]
    pub fn on_unknown_route(self, builder: impl Fn(&RouteInformation) -> Widget + 'static) -> Self {
        self.controller.on_unknown_route(builder);
        self
    }

    /// Sets the app title used unless `on_generate_title` is supplied.
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        let _ = self.title_controller.set_title(self.title.clone());
        self
    }

    /// Generates a locale-dependent title on every root rebuild.
    #[must_use]
    pub fn on_generate_title(mut self, builder: impl Fn(&Locale) -> String + 'static) -> Self {
        self.generated_title = Some(Rc::new(builder));
        self
    }

    #[must_use]
    pub fn title_controller(mut self, controller: TitleController) -> Self {
        self.title_controller = controller;
        self
    }

    #[must_use]
    pub fn title_sink(mut self, sink: Rc<dyn WindowChromeSink>) -> Self {
        self.title_sink = Some(sink);
        self
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        let _ = self.title_controller.set_color(color);
        self
    }

    #[must_use]
    pub fn environment(mut self, environment: RuntimeEnvironment) -> Self {
        self.environment = environment.normalized();
        self
    }

    #[must_use]
    pub fn supported_locales(mut self, locales: impl IntoIterator<Item = Locale>) -> Self {
        self.supported_locales = locales.into_iter().collect();
        self
    }

    #[must_use]
    pub fn localization_catalog(mut self, catalog: impl LocalizationCatalog + 'static) -> Self {
        self.catalog = Some(Rc::new(catalog));
        self
    }

    /// Accepts an already type-erased application catalog.
    #[must_use]
    pub fn shared_localization_catalog(mut self, catalog: Rc<dyn LocalizationCatalog>) -> Self {
        self.catalog = Some(catalog);
        self
    }

    /// Wraps the selected root just inside the app shell.
    #[must_use]
    pub fn builder(mut self, builder: impl Fn(Widget) -> Widget + 'static) -> Self {
        self.builder = Some(Rc::new(builder));
        self
    }

    /// Uses an independently built router root, equivalent to
    /// `WidgetsApp.router` in Flutter.
    #[must_use]
    pub fn router_widget(mut self, router: impl Into<Widget>) -> Self {
        self.router_root = Some(router.into());
        self
    }

    #[must_use]
    pub fn checked_mode_banner(mut self, enabled: bool) -> Self {
        self.show_checked_mode_banner = enabled;
        self
    }

    #[must_use]
    pub fn restoration_scope_id(mut self, id: impl Into<String>) -> Self {
        self.restoration_scope_id = Some(id.into());
        self
    }

    /// Builds the complete root with an explicit runtime environment.
    #[must_use]
    pub fn build_with_environment(&self, environment: RuntimeEnvironment) -> Widget {
        let environment = environment.normalized();
        let locale = resolve_app_locale(&environment, &self.supported_locales);
        let title = locale
            .as_ref()
            .and_then(|locale| self.generated_title.as_ref().map(|builder| builder(locale)))
            .unwrap_or_else(|| self.title.clone());
        let _ = self.title_controller.set(&title, self.color);
        if let Some(sink) = self.title_sink.clone() {
            self.title_controller.bind_sink(sink);
        }

        let mut child = self
            .router_root
            .clone()
            .unwrap_or_else(|| self.controller.clone().into_widget());
        if let Some(builder) = self.builder.as_ref() {
            child = builder(child);
        }
        if let Some(catalog) = self.catalog.as_ref() {
            if let Some(locale) = locale.clone() {
                child = Localizations::new(locale.clone(), SharedCatalog(catalog.clone()), child)
                    .into();
            }
        }
        let direction = locale
            .as_ref()
            .map_or(environment.text_direction, LocaleResolver::text_direction);
        child = Directionality::new(direction, child).into();
        child = MediaQuery::new(environment.clone(), child).into();
        child = Title::simple(title, child)
            .with_controller(self.title_controller.clone())
            .into_widget();
        if self.show_checked_mode_banner {
            child = CheckedModeBanner::new(child).into();
        }
        child
    }

    /// Builds using the app's retained environment.
    #[must_use]
    pub fn into_widget(self) -> Widget {
        self.build_with_environment(self.environment.clone())
    }

    /// Alias for [`Self::into_widget`].
    #[must_use]
    pub fn widget(self) -> Widget {
        self.into_widget()
    }

    /// Produces the spec consumed by a runtime bootstrap adapter.
    #[must_use]
    pub fn bootstrap_spec(&self) -> ApplicationBootstrapSpec {
        let mut environment = self.environment.clone().normalized();
        let options = ApplicationBootstrapOptions::default();
        if environment.viewport == Size::ZERO {
            environment.viewport = options.initial_size;
        }
        let root = self.build_with_environment(environment.clone());
        let options = ApplicationBootstrapOptions {
            title: self.title_controller.title(),
            ..options
        };
        ApplicationBootstrapSpec {
            options,
            environment: environment.clone().normalized(),
            root,
            restoration_scope_id: self.restoration_scope_id.clone(),
        }
    }

    /// Hands the root and real bootstrap defaults to a runtime-owned host.
    pub fn bootstrap<H: ApplicationBootstrapHost>(&self, host: &H) -> Result<H::Handle, H::Error> {
        host.create_application(self.bootstrap_spec())
    }
}

impl From<WidgetsApp> for Widget {
    fn from(value: WidgetsApp) -> Self {
        value.into_widget()
    }
}

fn route_path(location: &str) -> String {
    let normalized = super::normalize_route_location(location);
    normalized
        .char_indices()
        .find(|(_, character)| matches!(character, '?' | '#'))
        .map_or_else(
            || normalized.clone(),
            |(index, _)| normalized[..index].to_owned(),
        )
}

fn route_prefixes(path: &str) -> Vec<String> {
    let segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    (1..=segments.len())
        .map(|count| format!("/{}", segments[..count].join("/")))
        .collect()
}

fn resolve_app_locale(environment: &RuntimeEnvironment, supported: &[Locale]) -> Option<Locale> {
    if supported.is_empty() {
        return environment.primary_locale().cloned();
    }
    LocaleResolver::resolve(&environment.locales, supported)
}

struct SharedCatalog(Rc<dyn LocalizationCatalog>);

impl LocalizationCatalog for SharedCatalog {
    fn supported_locales(&self) -> &[Locale] {
        self.0.supported_locales()
    }

    fn message(&self, locale: &Locale, key: &str) -> Option<&str> {
        self.0.message(locale, key)
    }
}
