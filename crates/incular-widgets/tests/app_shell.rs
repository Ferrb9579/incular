use std::{cell::RefCell, collections::HashMap, rc::Rc};

use incular_config::{RuntimeEnvironment, TransparencyMode, WindowSizePolicy};
use incular_core::{Color, Rect, RestorationBackend, RestorationKey, RestorationScope, Size};
use incular_widgets::{
    ApplicationBootstrapHost, ApplicationBootstrapSpec, AuxiliaryViewError, AuxiliaryViewHandle,
    AuxiliaryViewHost, AuxiliaryViewOutcome, AuxiliaryViewRequest, BasicRouterDelegate,
    ClosureRouteInformationParser, ErrorWidget, MemoryRouteInformationProvider, MenuDispatchResult,
    MenuOwnerId, NavigationNotification, NavigationNotificationKind, NoopPlatformMenuDelegate,
    PlatformMenu, PlatformMenuBar, PlatformMenuBarController, PlatformMenuDelegate,
    PlatformMenuEvent, PlatformMenuItem, PlatformMenuSnapshot, PlatformMenuUpdate,
    RootBackButtonDispatcher, RouteInformation, RouteInformationProvider, Router, RouterConfig,
    RouterDelegate, RouterDelegateListener, RouterDelegateSubscription, RouterError, SizedBox,
    StringRouteInformationParser, TitleController, TitleError, ViewAnchorController,
    ViewController, ViewId, ViewLifecycle, ViewMetrics, Widget, WidgetsApp, WindowChromeSink,
};

#[derive(Default)]
struct RestorationMemory {
    values: RefCell<HashMap<Vec<RestorationKey>, serde_json::Value>>,
}

impl RestorationBackend for RestorationMemory {
    fn read_value(&self, path: &[RestorationKey]) -> Option<serde_json::Value> {
        self.values.borrow().get(path).cloned()
    }

    fn write_value(&self, path: &[RestorationKey], value: serde_json::Value) {
        self.values.borrow_mut().insert(path.to_vec(), value);
    }

    fn remove_value(&self, path: &[RestorationKey]) {
        self.values.borrow_mut().remove(path);
    }
}

#[test]
fn router_parses_reports_restores_and_dispatches_back() {
    let provider = MemoryRouteInformationProvider::new("/initial");
    let delegate = BasicRouterDelegate::new(|| SizedBox::shrink().into());
    let back_count = Rc::new(RefCell::new(0_u32));
    let back_count_for_delegate = back_count.clone();
    delegate.on_pop_route(move || {
        *back_count_for_delegate.borrow_mut() += 1;
        true
    });
    let dispatcher = RootBackButtonDispatcher::new();
    let config = RouterConfig::with_provider_parser(
        delegate.clone(),
        provider.clone(),
        StringRouteInformationParser,
    )
    .with_back_button_dispatcher(dispatcher.clone());
    let restoration_backend = Rc::new(RestorationMemory::default());
    let restoration_key = RestorationKey::new("router").expect("valid key");
    let router = Router::from_config(config).restoration_scope(
        RestorationScope::root(restoration_backend.clone()),
        restoration_key,
    );
    let event_count = Rc::new(RefCell::new(0_u32));
    let event_count_for_listener = event_count.clone();
    let _events = router.on_navigation_notification(move |_| {
        *event_count_for_listener.borrow_mut() += 1;
    });
    let _widget: Widget = router.into_widget();
    assert_eq!(
        delegate.current_configuration().as_deref(),
        Some("/initial")
    );

    provider.set_value("next");
    assert_eq!(delegate.current_configuration().as_deref(), Some("/next"));
    assert!(*event_count.borrow() >= 1);
    assert!(dispatcher.dispatch_back());
    assert_eq!(*back_count.borrow(), 1);
    assert!(
        restoration_backend
            .values
            .borrow()
            .values()
            .next()
            .is_some()
    );
}

#[test]
fn malformed_restored_route_notifies_and_falls_back_to_provider() {
    // A malformed persisted route is reported through the typed
    // notification channel instead of vanishing, while restoration stays
    // partial: the provider fallback still applies and later routes
    // recover normally.
    let restoration_backend = Rc::new(RestorationMemory::default());
    let scope = RestorationScope::root(restoration_backend.clone());
    let restoration_key = RestorationKey::new("router").expect("valid key");
    scope.set_json(&restoration_key, serde_json::json!({"location": 42}));
    let provider = MemoryRouteInformationProvider::new("/fallback");
    let delegate = BasicRouterDelegate::new(|| SizedBox::shrink().into());
    let config = RouterConfig::with_provider_parser(
        delegate.clone(),
        provider.clone(),
        StringRouteInformationParser,
    );
    let notifications = Rc::new(RefCell::new(Vec::new()));
    let notifications_for_listener = notifications.clone();
    let router =
        Router::from_config(config).restoration_scope(scope.clone(), restoration_key.clone());
    let _events = router.on_navigation_notification(move |notification| {
        notifications_for_listener.borrow_mut().push(notification);
    });
    let _widget: Widget = router.into_widget();

    // Exactly one failure notification, carrying the restoration error
    // with no route information attached.
    let failures: Vec<_> = notifications
        .borrow()
        .iter()
        .filter(|notification| notification.kind == NavigationNotificationKind::ParseFailed)
        .cloned()
        .collect();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].route_information, None);
    assert!(!failures[0].can_handle_pop);
    assert!(matches!(
        failures[0].error,
        Some(RouterError::Restoration(_))
    ));
    // The delegate and persisted scope follow the provider fallback (the
    // corrupt value heals forward instead of failing every launch), and
    // the next valid route applies and recovers normally.
    assert_eq!(
        delegate.current_configuration().as_deref(),
        Some("/fallback")
    );
    assert_eq!(
        scope.get_json(&restoration_key),
        Some(RouteInformation::new("/fallback").to_json())
    );
    provider.set_value("/next");
    assert_eq!(delegate.current_configuration().as_deref(), Some("/next"));
    assert!(
        notifications.borrow().iter().all(|notification| {
            notification.kind != NavigationNotificationKind::ParseFailed
                || notification.route_information.is_none()
        }),
        "only the malformed restore reports without route information"
    );
}

#[test]
fn rejected_delegate_route_notifies_without_committing() {
    // A builder failure reaches observers as a typed ParseFailed while
    // the router commits nothing: the next valid route still recovers
    // normally through the same path.
    let provider = MemoryRouteInformationProvider::new("/initial");
    let delegate = BasicRouterDelegate::new(|| SizedBox::shrink().into());
    delegate.on_set_new_route_path(|configuration: String| {
        (configuration != "/reject")
            .then_some(())
            .ok_or_else(|| RouterError::message("route not served"))
    });
    let config = RouterConfig::with_provider_parser(
        delegate.clone(),
        provider.clone(),
        StringRouteInformationParser,
    );
    let notifications = Rc::new(RefCell::new(Vec::new()));
    let notifications_for_listener = notifications.clone();
    let router = Router::from_config(config);
    let _events = router.on_navigation_notification(move |notification| {
        notifications_for_listener.borrow_mut().push(notification);
    });
    let _widget: Widget = router.into_widget();
    provider.set_value("/reject");
    let failures: Vec<_> = notifications
        .borrow()
        .iter()
        .filter(|notification| notification.kind == NavigationNotificationKind::ParseFailed)
        .cloned()
        .collect();
    assert_eq!(failures.len(), 1);
    assert_eq!(
        failures[0]
            .route_information
            .as_ref()
            .map(|info| info.location()),
        Some("/reject")
    );
    assert!(matches!(failures[0].error, Some(RouterError::Delegate(_))));
    provider.set_value("/next");
    assert_eq!(delegate.current_configuration().as_deref(), Some("/next"));
}

#[test]
fn widgets_app_expands_deep_links_and_bootstraps_defaults() {
    let app = WidgetsApp::empty()
        .route_widget("/", SizedBox::shrink().into())
        .route_widget("/a", SizedBox::shrink().into())
        .route_widget("/a/b", SizedBox::shrink().into())
        .initial_route("/a/b");
    let controller = app.controller();
    let _root: Widget = app.clone().into_widget();
    let stack = controller.route_stack();
    assert_eq!(
        stack
            .iter()
            .map(|route| route.location())
            .collect::<Vec<_>>(),
        ["/", "/a", "/a/b"]
    );

    struct TestHost;
    impl ApplicationBootstrapHost for TestHost {
        type Handle = (String, Size, WindowSizePolicy, TransparencyMode, Color);
        type Error = std::convert::Infallible;

        fn create_application(
            &self,
            specification: ApplicationBootstrapSpec,
        ) -> Result<Self::Handle, Self::Error> {
            Ok((
                specification.options.title,
                specification.options.initial_size,
                specification.options.size_policy,
                specification.options.transparency_mode,
                specification.options.background_color,
            ))
        }
    }

    let (title, size, size_policy, transparency_mode, background_color) =
        app.bootstrap(&TestHost).expect("bootstrap succeeds");
    assert_eq!(title, "Incular");
    assert_eq!(size, Size::new(800.0, 600.0));
    assert_eq!(size_policy, WindowSizePolicy::Viewport);
    assert_eq!(transparency_mode, TransparencyMode::Opaque);
    assert_eq!(background_color, Color::BLACK);
}

#[derive(Default)]
struct ChromeState {
    titles: RefCell<Vec<String>>,
    colors: RefCell<Vec<Color>>,
}

impl WindowChromeSink for ChromeState {
    fn set_title(&self, title: &str) -> bool {
        self.titles.borrow_mut().push(title.to_owned());
        true
    }

    fn set_application_color(&self, color: Color) -> bool {
        self.colors.borrow_mut().push(color);
        true
    }
}

#[test]
fn title_retains_updates_and_rejects_transparency() {
    let controller = TitleController::new("First", Color::WHITE);
    let sink = Rc::new(ChromeState::default());
    controller.bind_sink(sink.clone());
    assert!(controller.set_title("Second"));
    assert_eq!(sink.titles.borrow().as_slice(), ["First", "Second"]);
    assert_eq!(controller.set_color(Color::rgba(1, 2, 3, 255)), Ok(true));
    assert_eq!(sink.colors.borrow().len(), 2);
    assert_eq!(
        controller.set_color(Color::rgba(1, 2, 3, 0)),
        Err(TitleError::TransparentColor)
    );
}

#[derive(Default)]
struct AuxiliaryState {
    created: RefCell<Vec<AuxiliaryViewRequest>>,
    updated: RefCell<Vec<AuxiliaryViewRequest>>,
    disposed: RefCell<Vec<AuxiliaryViewHandle>>,
}

impl AuxiliaryViewHost for AuxiliaryState {
    fn create(
        &self,
        request: AuxiliaryViewRequest,
    ) -> Result<AuxiliaryViewHandle, AuxiliaryViewError> {
        self.created.borrow_mut().push(request.clone());
        Ok(AuxiliaryViewHandle {
            view_id: request.view_id,
            token: 1,
        })
    }

    fn update(
        &self,
        _handle: AuxiliaryViewHandle,
        request: AuxiliaryViewRequest,
    ) -> Result<(), AuxiliaryViewError> {
        self.updated.borrow_mut().push(request);
        Ok(())
    }

    fn dispose(&self, handle: AuxiliaryViewHandle) {
        self.disposed.borrow_mut().push(handle);
    }
}

#[derive(Default)]
struct PlatformMenuState {
    owner: RefCell<Option<MenuOwnerId>>,
    snapshot: RefCell<Option<PlatformMenuSnapshot>>,
    event_handler: RefCell<Option<PlatformMenuHandler>>,
    reject_updates: RefCell<bool>,
}

type PlatformMenuHandler = Rc<dyn Fn(PlatformMenuEvent)>;

impl PlatformMenuState {
    fn emit(&self, event: PlatformMenuEvent) {
        let handler = self.event_handler.borrow().clone();
        if let Some(handler) = handler {
            handler(event);
        }
    }
}

impl PlatformMenuDelegate for PlatformMenuState {
    fn acquire(&self, owner: MenuOwnerId) -> PlatformMenuUpdate {
        let mut current = self.owner.borrow_mut();
        match *current {
            None => {
                *current = Some(owner);
                PlatformMenuUpdate::Applied
            }
            Some(active) if active == owner => PlatformMenuUpdate::Applied,
            Some(_) => PlatformMenuUpdate::RejectedOwnedByOther,
        }
    }

    fn set_menus(&self, owner: MenuOwnerId, snapshot: PlatformMenuSnapshot) -> PlatformMenuUpdate {
        if *self.owner.borrow() != Some(owner) {
            return PlatformMenuUpdate::RejectedOwnedByOther;
        }
        if *self.reject_updates.borrow() {
            return PlatformMenuUpdate::RejectedByPlatform;
        }
        let mut current = self.snapshot.borrow_mut();
        if current.as_ref() == Some(&snapshot) {
            return PlatformMenuUpdate::Unchanged;
        }
        *current = Some(snapshot);
        PlatformMenuUpdate::Applied
    }

    fn clear_menus(&self, owner: MenuOwnerId) -> PlatformMenuUpdate {
        if *self.owner.borrow() != Some(owner) {
            return PlatformMenuUpdate::RejectedOwnedByOther;
        }
        self.snapshot.borrow_mut().take();
        PlatformMenuUpdate::Applied
    }

    fn release(&self, owner: MenuOwnerId) -> PlatformMenuUpdate {
        let mut current = self.owner.borrow_mut();
        if *current != Some(owner) {
            return PlatformMenuUpdate::RejectedOwnedByOther;
        }
        *current = None;
        PlatformMenuUpdate::Applied
    }

    fn set_event_handler(
        &self,
        owner: MenuOwnerId,
        handler: Option<Rc<dyn Fn(PlatformMenuEvent)>>,
    ) -> PlatformMenuUpdate {
        if *self.owner.borrow() != Some(owner) {
            return PlatformMenuUpdate::RejectedOwnedByOther;
        }
        *self.event_handler.borrow_mut() = handler;
        PlatformMenuUpdate::Applied
    }
}

#[test]
fn platform_menu_delegate_owns_commands_and_noop_is_explicit() {
    let selected = Rc::new(RefCell::new(0_u32));
    let selected_for_item = selected.clone();
    let menu = PlatformMenu::with_items(
        "File",
        vec![
            PlatformMenuItem::new("Open")
                .id("open")
                .on_selected(move || *selected_for_item.borrow_mut() += 1),
        ],
    );
    let delegate = Rc::new(PlatformMenuState::default());
    let controller = PlatformMenuBarController::new(delegate.clone());
    assert_eq!(controller.install(&[menu]), Ok(PlatformMenuUpdate::Applied));
    assert_eq!(
        controller.dispatch(&"open".into()),
        MenuDispatchResult::Handled
    );
    assert_eq!(*selected.borrow(), 1);
    delegate.emit(PlatformMenuEvent::Selected("open".into()));
    assert_eq!(*selected.borrow(), 2);
    assert_eq!(*delegate.owner.borrow(), Some(controller.owner()));
    assert!(matches!(controller.detach(), PlatformMenuUpdate::Applied));
    assert!(delegate.owner.borrow().is_none());
    delegate.emit(PlatformMenuEvent::Selected("open".into()));
    assert_eq!(
        *selected.borrow(),
        2,
        "detach removes the native event sink"
    );

    let unsupported = PlatformMenuBarController::new(Rc::new(NoopPlatformMenuDelegate));
    assert_eq!(
        unsupported.install(&[]),
        Ok(PlatformMenuUpdate::NoOpUnsupported)
    );
}

#[test]
fn platform_menu_ownership_hands_off_after_active_owner_detaches() {
    let delegate = Rc::new(PlatformMenuState::default());
    let first = PlatformMenuBarController::new(delegate.clone());
    let second = PlatformMenuBarController::new(delegate.clone());
    let first_menu = PlatformMenu::with_items(
        "File",
        [PlatformMenuItem::new("Open")
            .id("first.open")
            .on_selected(|| {})],
    );
    let second_menu = PlatformMenu::with_items(
        "File",
        [PlatformMenuItem::new("Open")
            .id("second.open")
            .on_selected(|| {})],
    );

    assert_eq!(
        first.install(&[first_menu]),
        Ok(PlatformMenuUpdate::Applied)
    );
    assert_eq!(
        second.install(std::slice::from_ref(&second_menu)),
        Ok(PlatformMenuUpdate::RejectedOwnedByOther)
    );
    assert_eq!(*delegate.owner.borrow(), Some(first.owner()));

    assert_eq!(first.detach(), PlatformMenuUpdate::Applied);
    assert_eq!(
        second.install(&[second_menu]),
        Ok(PlatformMenuUpdate::Applied)
    );
    assert_eq!(*delegate.owner.borrow(), Some(second.owner()));
}

#[test]
fn platform_menu_snapshot_distinguishes_empty_menu_from_command_item() {
    let delegate = Rc::new(PlatformMenuState::default());
    let controller = PlatformMenuBarController::new(delegate.clone());
    let menus = [
        PlatformMenu::new("Empty", Vec::<incular_widgets::PlatformMenuEntry>::new()).id("empty"),
        PlatformMenu::with_items(
            "Actions",
            [PlatformMenuItem::new("Run").id("run").on_selected(|| {})],
        )
        .id("actions"),
    ];
    assert_eq!(controller.install(&menus), Ok(PlatformMenuUpdate::Applied));
    let snapshot = delegate.snapshot.borrow();
    let snapshot = snapshot.as_ref().expect("native snapshot");
    assert!(!snapshot.menus[0].selectable);
    assert!(snapshot.menus[0].children.is_empty());
    assert!(!snapshot.menus[1].selectable);
    assert!(snapshot.menus[1].children[0].selectable);
}

#[test]
fn native_menu_callback_may_detach_controller_reentrantly() {
    let delegate = Rc::new(PlatformMenuState::default());
    let controller = PlatformMenuBarController::new(delegate.clone());
    let retained_controller = Rc::new(RefCell::new(Some(controller.clone())));
    let retained_for_callback = retained_controller.clone();
    let menu = PlatformMenu::with_items(
        "File",
        [PlatformMenuItem::new("Close Menu")
            .id("close-menu")
            .on_selected(move || {
                let controller = retained_for_callback.borrow().as_ref().unwrap().clone();
                let _ = controller.detach();
            })],
    );
    assert_eq!(controller.install(&[menu]), Ok(PlatformMenuUpdate::Applied));

    delegate.emit(PlatformMenuEvent::Selected("close-menu".into()));
    assert!(delegate.owner.borrow().is_none());
    retained_controller.borrow_mut().take();
}

#[test]
fn default_platform_menu_bar_is_late_bound_from_the_retained_tree() {
    let selected = Rc::new(RefCell::new(0_u32));
    let selected_for_item = selected.clone();
    let root: Widget = PlatformMenuBar::new(
        [PlatformMenu::with_items(
            "File",
            [PlatformMenuItem::new("Open")
                .id("open")
                .on_selected(move || *selected_for_item.borrow_mut() += 1)],
        )],
        SizedBox::shrink(),
    )
    .into();
    let mut tree = incular_widgets::internal::WidgetTree::new();
    tree.mount(root).expect("mount late-bound platform menu");
    let bindings = tree.platform_menu_bindings();
    assert_eq!(bindings.len(), 1);

    let delegate = Rc::new(PlatformMenuState::default());
    assert_eq!(
        bindings[0].connect(delegate.clone()),
        Ok(PlatformMenuUpdate::Applied)
    );
    delegate.emit(PlatformMenuEvent::Selected("open".into()));
    assert_eq!(*selected.borrow(), 1);
}

#[test]
fn platform_menu_bar_update_persists_model_before_native_binding() {
    let mut menu_bar = PlatformMenuBar::new(
        [PlatformMenu::with_items(
            "File",
            [PlatformMenuItem::new("Open").id("open").on_selected(|| {})],
        )],
        SizedBox::shrink(),
    );
    assert_eq!(
        menu_bar.update([PlatformMenu::with_items(
            "Document",
            [PlatformMenuItem::new("Save").id("save").on_selected(|| {})],
        )]),
        Ok(PlatformMenuUpdate::NoOpUnsupported)
    );

    let mut tree = incular_widgets::internal::WidgetTree::new();
    tree.mount(menu_bar.into())
        .expect("mount updated platform menu");
    let delegate = Rc::new(PlatformMenuState::default());
    let binding = tree.platform_menu_bindings().remove(0);
    assert_eq!(
        binding.connect(delegate.clone()),
        Ok(PlatformMenuUpdate::Applied)
    );
    let snapshot = delegate.snapshot.borrow();
    let snapshot = snapshot.as_ref().expect("updated menu installed");
    assert_eq!(snapshot.menus[0].label, "Document");
    assert_eq!(snapshot.menus[0].children[0].id.as_str(), "save");
}

#[test]
fn explicit_platform_menu_delegate_is_retained_and_reconciled() {
    let first_delegate = Rc::new(PlatformMenuState::default());
    let root: Widget = PlatformMenuBar::with_delegate(
        [PlatformMenu::with_items(
            "File",
            [PlatformMenuItem::new("Open").id("open").on_selected(|| {})],
        )],
        SizedBox::shrink(),
        first_delegate.clone(),
    )
    .into();
    let mut tree = incular_widgets::internal::WidgetTree::new();
    let root = tree.mount(root).expect("mount explicit platform menu");
    let owner = first_delegate
        .owner
        .borrow()
        .expect("explicit delegate acquires retained owner on mount");
    assert_eq!(
        first_delegate.snapshot.borrow().as_ref().unwrap().menus[0].children[0].label,
        "Open"
    );
    assert!(
        tree.platform_menu_bindings().is_empty(),
        "explicit delegates must not be rebound by the desktop native delegate"
    );

    tree.update(
        root,
        PlatformMenuBar::with_delegate(
            [PlatformMenu::with_items(
                "File",
                [PlatformMenuItem::new("Opened")
                    .id("open")
                    .on_selected(|| {})],
            )],
            SizedBox::shrink(),
            first_delegate.clone(),
        )
        .into(),
    )
    .expect("reconcile explicit platform menu");
    assert_eq!(*first_delegate.owner.borrow(), Some(owner));
    assert_eq!(
        first_delegate.snapshot.borrow().as_ref().unwrap().menus[0].children[0].label,
        "Opened"
    );

    let second_delegate = Rc::new(PlatformMenuState::default());
    tree.update(
        root,
        PlatformMenuBar::with_delegate(
            [PlatformMenu::with_items(
                "File",
                [PlatformMenuItem::new("Save").id("save").on_selected(|| {})],
            )],
            SizedBox::shrink(),
            second_delegate.clone(),
        )
        .into(),
    )
    .expect("switch explicit platform menu delegate");
    assert!(first_delegate.owner.borrow().is_none());
    assert_eq!(*second_delegate.owner.borrow(), Some(owner));
    assert_eq!(
        second_delegate.snapshot.borrow().as_ref().unwrap().menus[0].children[0].label,
        "Save"
    );
}

#[test]
fn platform_menu_duplicate_id_rejection_preserves_installed_model() {
    let delegate = Rc::new(PlatformMenuState::default());
    let controller = PlatformMenuBarController::new(delegate.clone());
    let original = PlatformMenu::with_items(
        "File",
        [PlatformMenuItem::new("Open").id("open").on_selected(|| {})],
    )
    .id("file");
    assert_eq!(
        controller.install(&[original]),
        Ok(PlatformMenuUpdate::Applied)
    );

    let duplicate = PlatformMenu::with_items(
        "Document",
        [
            PlatformMenuItem::new("Save")
                .id("duplicate")
                .on_selected(|| {}),
            PlatformMenuItem::new("Save As")
                .id("duplicate")
                .on_selected(|| {}),
        ],
    )
    .id("document");
    assert!(matches!(
        controller.install(&[duplicate]),
        Err(incular_widgets::PlatformMenuBuildError::DuplicateId(id)) if id.as_str() == "duplicate"
    ));
    let snapshot = delegate.snapshot.borrow();
    let snapshot = snapshot.as_ref().expect("previous menu remains installed");
    assert_eq!(snapshot.menus[0].label, "File");
    assert_eq!(snapshot.menus[0].children[0].id.as_str(), "open");
}

#[test]
fn rejected_platform_menu_replacement_keeps_installed_callback_generation() {
    let old_calls = Rc::new(RefCell::new(0_u32));
    let old_calls_for_item = old_calls.clone();
    let delegate = Rc::new(PlatformMenuState::default());
    let controller = PlatformMenuBarController::new(delegate.clone());
    let original = PlatformMenu::with_items(
        "File",
        [PlatformMenuItem::new("Open")
            .id("open")
            .on_selected(move || *old_calls_for_item.borrow_mut() += 1)],
    );
    assert_eq!(
        controller.install(&[original]),
        Ok(PlatformMenuUpdate::Applied)
    );

    *delegate.reject_updates.borrow_mut() = true;
    let new_calls = Rc::new(RefCell::new(0_u32));
    let new_calls_for_item = new_calls.clone();
    let replacement = PlatformMenu::with_items(
        "Document",
        [PlatformMenuItem::new("Open New")
            .id("open")
            .on_selected(move || *new_calls_for_item.borrow_mut() += 1)],
    );
    assert_eq!(
        controller.install(&[replacement]),
        Ok(PlatformMenuUpdate::RejectedByPlatform)
    );

    delegate.emit(PlatformMenuEvent::Selected("open".into()));
    assert_eq!(*old_calls.borrow(), 1);
    assert_eq!(*new_calls.borrow(), 0);
}

#[test]
fn unchanged_platform_menu_snapshot_updates_callback_without_native_rebuild() {
    let old_calls = Rc::new(RefCell::new(0_u32));
    let old_calls_for_item = old_calls.clone();
    let delegate = Rc::new(PlatformMenuState::default());
    let controller = PlatformMenuBarController::new(delegate.clone());
    let original = PlatformMenu::with_items(
        "File",
        [PlatformMenuItem::new("Open")
            .id("open")
            .on_selected(move || *old_calls_for_item.borrow_mut() += 1)],
    );
    assert_eq!(
        controller.install(&[original]),
        Ok(PlatformMenuUpdate::Applied)
    );

    let new_calls = Rc::new(RefCell::new(0_u32));
    let new_calls_for_item = new_calls.clone();
    let same_snapshot = PlatformMenu::with_items(
        "File",
        [PlatformMenuItem::new("Open")
            .id("open")
            .on_selected(move || *new_calls_for_item.borrow_mut() += 1)],
    );
    assert_eq!(
        controller.install(&[same_snapshot]),
        Ok(PlatformMenuUpdate::Unchanged)
    );

    delegate.emit(PlatformMenuEvent::Selected("open".into()));
    assert_eq!(*old_calls.borrow(), 0);
    assert_eq!(*new_calls.borrow(), 1);
}

#[test]
fn retained_platform_menu_rebuild_preserves_owner_and_updates_model() {
    let old_calls = Rc::new(RefCell::new(0_u32));
    let old_calls_for_item = old_calls.clone();
    let root_widget: Widget = PlatformMenuBar::new(
        [PlatformMenu::with_items(
            "File",
            [PlatformMenuItem::new("Open")
                .id("open")
                .on_selected(move || *old_calls_for_item.borrow_mut() += 1)],
        )],
        SizedBox::shrink(),
    )
    .into();
    let mut tree = incular_widgets::internal::WidgetTree::new();
    let root = tree
        .mount(root_widget)
        .expect("mount retained platform menu");
    let delegate = Rc::new(PlatformMenuState::default());
    let first = tree.platform_menu_bindings();
    assert_eq!(first.len(), 1);
    let owner = first[0].owner();
    assert_eq!(
        first[0].connect(delegate.clone()),
        Ok(PlatformMenuUpdate::Applied)
    );

    let new_calls = Rc::new(RefCell::new(0_u32));
    let new_calls_for_item = new_calls.clone();
    let rebuilt: Widget = PlatformMenuBar::new(
        [PlatformMenu::with_items(
            "Document",
            [PlatformMenuItem::new("Open New")
                .id("open")
                .on_selected(move || *new_calls_for_item.borrow_mut() += 1)],
        )],
        SizedBox::shrink(),
    )
    .into();
    tree.update(root, rebuilt)
        .expect("rebuild retained platform menu");

    let second = tree.platform_menu_bindings();
    assert_eq!(second.len(), 1);
    assert_eq!(
        second[0].owner(),
        owner,
        "retained element owns menu identity"
    );
    assert_eq!(
        delegate
            .snapshot
            .borrow()
            .as_ref()
            .expect("reconciliation synchronizes bound native snapshot")
            .menus[0]
            .label,
        "Document"
    );
    assert_eq!(
        second[0].connect(delegate.clone()),
        Ok(PlatformMenuUpdate::Unchanged)
    );
    assert_eq!(
        delegate
            .snapshot
            .borrow()
            .as_ref()
            .expect("updated native snapshot")
            .menus[0]
            .label,
        "Document"
    );
    delegate.emit(PlatformMenuEvent::Selected("open".into()));
    assert_eq!(*old_calls.borrow(), 0);
    assert_eq!(*new_calls.borrow(), 1);
}

#[test]
fn view_controller_and_anchor_own_metrics_lifecycle_and_auxiliary_disposal() {
    let controller = ViewController::new(
        ViewId::new(7),
        ViewMetrics::new(1600, 1200, 2.0),
        RuntimeEnvironment::default(),
    );
    assert_eq!(
        controller.data().environment.viewport,
        Size::new(800.0, 600.0)
    );
    assert!(controller.set_lifecycle(ViewLifecycle::Visible));
    assert!(controller.update_metrics(ViewMetrics::new(1000, 800, 2.0)));
    assert_eq!(
        controller.data().environment.viewport,
        Size::new(500.0, 400.0)
    );

    let host = Rc::new(AuxiliaryState::default());
    let anchor = ViewAnchorController::with_host(host.clone());
    let request = AuxiliaryViewRequest {
        view_id: controller.id(),
        child: ErrorWidget::new().into(),
        metrics: controller.data().metrics,
        environment: controller.data().environment,
        anchor: Rect::from_origin_size(incular_core::Offset::ZERO, Size::new(40.0, 20.0)),
        title: Some("Auxiliary".to_owned()),
    };
    assert!(matches!(
        anchor.sync(request.clone()),
        Ok(AuxiliaryViewOutcome::Created(_))
    ));
    assert!(matches!(
        anchor.sync(request),
        Ok(AuxiliaryViewOutcome::Unchanged(_))
    ));
    assert_eq!(host.created.borrow().len(), 1);
    assert!(anchor.detach());
    assert_eq!(host.disposed.borrow().len(), 1);
}

fn basic_delegate_with_counter() -> (BasicRouterDelegate<String>, Rc<RefCell<usize>>) {
    let delegate = BasicRouterDelegate::new(|| SizedBox::shrink().into());
    let notifications = Rc::new(RefCell::new(0usize));
    let notifications_for_listener = notifications.clone();
    let subscription = delegate.subscribe(Rc::new(move || {
        *notifications_for_listener.borrow_mut() += 1;
    }));
    // The subscription only keeps the listener alive while it is retained;
    // leak it so delegate notifications keep flowing for the test body.
    std::mem::forget(subscription);
    (delegate, notifications)
}

#[test]
fn basic_delegate_rejection_preserves_accepted_configuration() {
    let (delegate, notifications) = basic_delegate_with_counter();
    delegate.on_set_new_route_path(|configuration: String| {
        (configuration != "/reject")
            .then_some(())
            .ok_or_else(|| RouterError::message("route not served"))
    });
    assert!(delegate.set_new_route_path("/accepted".to_owned()).is_ok());
    assert_eq!(
        delegate.current_configuration().as_deref(),
        Some("/accepted")
    );
    assert_eq!(*notifications.borrow(), 1);
    assert!(delegate.set_new_route_path("/reject".to_owned()).is_err());
    // Rejection is atomic: the previously accepted configuration survives
    // and no success notification is emitted.
    assert_eq!(
        delegate.current_configuration().as_deref(),
        Some("/accepted")
    );
    assert_eq!(*notifications.borrow(), 1);
    // The delegate still accepts later routes after a rejection.
    assert!(delegate.set_new_route_path("/next".to_owned()).is_ok());
    assert_eq!(delegate.current_configuration().as_deref(), Some("/next"));
    assert_eq!(*notifications.borrow(), 2);
}

#[test]
fn basic_delegate_callback_may_replace_itself() {
    let (delegate, notifications) = basic_delegate_with_counter();
    let delegate_for_callback = delegate.clone();
    delegate.on_set_new_route_path(move |configuration: String| {
        if configuration == "/swap" {
            delegate_for_callback.on_set_new_route_path(|_: String| Ok(()));
        }
        Ok(())
    });
    // The old code held the registration borrow across the callback, so this
    // reentrant replacement panicked with `already borrowed`.
    assert!(delegate.set_new_route_path("/swap".to_owned()).is_ok());
    assert_eq!(delegate.current_configuration().as_deref(), Some("/swap"));
    assert_eq!(*notifications.borrow(), 1);
    assert!(delegate.set_new_route_path("/later".to_owned()).is_ok());
    assert_eq!(delegate.current_configuration().as_deref(), Some("/later"));
    assert_eq!(*notifications.borrow(), 2);
}

#[test]
fn basic_delegate_nested_success_wins_over_outer_application() {
    let (delegate, notifications) = basic_delegate_with_counter();
    let delegate_for_callback = delegate.clone();
    delegate.on_set_new_route_path(move |configuration: String| {
        if configuration == "/outer" {
            delegate_for_callback
                .set_new_route_path("/inner".to_owned())
                .expect("nested application succeeds");
        }
        Ok(())
    });
    assert!(delegate.set_new_route_path("/outer".to_owned()).is_ok());
    // The nested commit is newer, so the outer acceptance must not overwrite
    // it — and only the nested commit notifies.
    assert_eq!(delegate.current_configuration().as_deref(), Some("/inner"));
    assert_eq!(*notifications.borrow(), 1);
}

#[test]
fn basic_delegate_nested_rejection_leaves_outer_application_intact() {
    let (delegate, notifications) = basic_delegate_with_counter();
    let delegate_for_callback = delegate.clone();
    delegate.on_set_new_route_path(move |configuration: String| {
        if configuration == "/outer" {
            // The nested application fails; ignoring its error must not roll
            // back anything because the nested attempt committed nothing.
            let _ = delegate_for_callback.set_new_route_path("/bad".to_owned());
            return Ok(());
        }
        Err(RouterError::message("route not served"))
    });
    assert!(delegate.set_new_route_path("/outer".to_owned()).is_ok());
    assert_eq!(delegate.current_configuration().as_deref(), Some("/outer"));
    assert_eq!(*notifications.borrow(), 1);
}

#[test]
fn basic_delegate_pop_callback_may_replace_itself() {
    let (delegate, _) = basic_delegate_with_counter();
    let delegate_for_callback = delegate.clone();
    let second_called = Rc::new(RefCell::new(false));
    let second_called_for_callback = second_called.clone();
    delegate.on_pop_route(move || {
        let second_called_for_callback = second_called_for_callback.clone();
        delegate_for_callback.on_pop_route(move || {
            *second_called_for_callback.borrow_mut() = true;
            true
        });
        true
    });
    assert!(delegate.pop_route());
    assert!(!*second_called.borrow());
    // The replacement installed by the first callback serves the next pop.
    assert!(delegate.pop_route());
    assert!(*second_called.borrow());
}

#[test]
fn basic_delegate_callback_replacement_retires_outside_the_borrow() {
    struct ReentrantGuard {
        delegate: BasicRouterDelegate<String>,
    }
    impl Drop for ReentrantGuard {
        fn drop(&mut self) {
            // Runs while the replaced callback is retired: registration must
            // already be released or this reentrant install panics.
            self.delegate
                .on_set_new_route_path(|_: String| Err(RouterError::message("reentrant")));
        }
    }

    let (delegate, notifications) = basic_delegate_with_counter();
    let guard = Rc::new(ReentrantGuard {
        delegate: delegate.clone(),
    });
    delegate.on_set_new_route_path({
        let guard = guard.clone();
        move |_: String| {
            let _ = &guard;
            Ok(())
        }
    });
    drop(guard);
    delegate.on_set_new_route_path(|_: String| Ok(()));
    // The destructor ran after the replacement was stored, so its install is
    // the one that sticks — deterministically, last writer wins.
    assert!(delegate.set_new_route_path("/probe".to_owned()).is_err());
    assert_eq!(delegate.current_configuration(), None);
    assert_eq!(*notifications.borrow(), 0);
}

type NavigationLog = Rc<RefCell<Vec<NavigationNotification>>>;

fn navigation_log() -> NavigationLog {
    Rc::new(RefCell::new(Vec::new()))
}

fn notification_summary(notifications: &[NavigationNotification]) -> Vec<(String, Option<String>)> {
    notifications
        .iter()
        .map(|notification| {
            (
                format!("{:?}", notification.kind),
                notification
                    .route_information
                    .as_ref()
                    .map(|information| information.location().to_owned()),
            )
        })
        .collect()
}

#[test]
fn router_parser_reentrancy_cannot_commit_over_nested_transaction() {
    // The parser delivers another route while the outer parse is in flight;
    // the nested commit owns the route state and the outer operation commits
    // nothing over it — exactly one commit sequence, for the nested route.
    let provider = MemoryRouteInformationProvider::new("/initial");
    let provider_for_parser = provider.clone();
    let parser = ClosureRouteInformationParser::new(
        move |information: &RouteInformation| {
            if information.location() == "/outer" {
                provider_for_parser.set_value("/inner");
            }
            Ok(information.location().to_owned())
        },
        |configuration: &String| Ok(RouteInformation::new(configuration)),
    );
    let delegate = BasicRouterDelegate::new(|| SizedBox::shrink().into());
    delegate.on_set_new_route_path(|_: String| Ok(()));
    let router = Router::with_provider_parser(delegate.clone(), provider.clone(), parser);
    let log = navigation_log();
    let notifications = log.clone();
    let _events = router.on_navigation_notification(move |notification| {
        notifications.borrow_mut().push(notification);
    });
    let _widget: Widget = router.into_widget();
    log.borrow_mut().clear();

    provider.set_value("/outer");

    assert_eq!(delegate.current_configuration().as_deref(), Some("/inner"));
    assert_eq!(
        notification_summary(&log.borrow()),
        [
            ("DelegateChanged".to_owned(), Some("/inner".to_owned())),
            (
                "RouteInformationChanged".to_owned(),
                Some("/inner".to_owned())
            ),
        ]
    );
}

#[test]
fn router_delegate_reentrancy_cannot_commit_over_nested_transaction() {
    // Same ownership rule through the delegate hook, with restoration
    // configured: persisted state reflects only the accepted nested commit.
    let restoration_backend = Rc::new(RestorationMemory::default());
    let scope = RestorationScope::root(restoration_backend.clone());
    let restoration_key = RestorationKey::new("router").expect("valid key");
    let provider = MemoryRouteInformationProvider::new("/initial");
    let provider_for_callback = provider.clone();
    let delegate = BasicRouterDelegate::new(|| SizedBox::shrink().into());
    delegate.on_set_new_route_path(move |configuration: String| {
        if configuration == "/outer" {
            provider_for_callback.set_value("/inner");
        }
        Ok(())
    });
    let config = RouterConfig::with_provider_parser(
        delegate.clone(),
        provider.clone(),
        StringRouteInformationParser,
    );
    let router =
        Router::from_config(config).restoration_scope(scope.clone(), restoration_key.clone());
    let log = navigation_log();
    let notifications = log.clone();
    let _events = router.on_navigation_notification(move |notification| {
        notifications.borrow_mut().push(notification);
    });
    let _widget: Widget = router.into_widget();
    log.borrow_mut().clear();

    provider.set_value("/outer");

    assert_eq!(delegate.current_configuration().as_deref(), Some("/inner"));
    assert_eq!(
        scope.get_json(&restoration_key),
        Some(RouteInformation::new("/inner").to_json())
    );
    assert_eq!(
        notification_summary(&log.borrow()),
        [
            ("DelegateChanged".to_owned(), Some("/inner".to_owned())),
            (
                "RouteInformationChanged".to_owned(),
                Some("/inner".to_owned())
            ),
        ]
    );
}

#[test]
fn router_delegate_query_reentrancy_does_not_borrow_panic() {
    use std::cell::Cell;

    struct IntrudingDelegate {
        inner: BasicRouterDelegate<String>,
        provider: MemoryRouteInformationProvider,
        intruded: Cell<bool>,
    }
    impl RouterDelegate<String> for IntrudingDelegate {
        fn build(&self) -> Widget {
            self.inner.build()
        }
        fn current_configuration(&self) -> Option<String> {
            if !self.intruded.get() {
                // Fires once: a delegate query is application code and may
                // deliver another route while the router holds no borrow.
                self.intruded.set(true);
                self.provider.set_value("/intruder");
            }
            self.inner.current_configuration()
        }
        fn set_new_route_path(&self, configuration: String) -> Result<(), RouterError> {
            self.inner.set_new_route_path(configuration)
        }
        fn pop_route(&self) -> bool {
            false
        }
        fn subscribe(&self, listener: RouterDelegateListener) -> RouterDelegateSubscription {
            self.inner.subscribe(listener)
        }
    }

    let provider = MemoryRouteInformationProvider::new("/initial");
    let delegate = IntrudingDelegate {
        inner: BasicRouterDelegate::new(|| SizedBox::shrink().into()),
        provider: provider.clone(),
        intruded: Cell::new(false),
    };
    delegate.inner.on_set_new_route_path(|_: String| Ok(()));
    let router =
        Router::with_provider_parser(delegate, provider.clone(), StringRouteInformationParser);
    let log = navigation_log();
    let notifications = log.clone();
    let _events = router.on_navigation_notification(move |notification| {
        notifications.borrow_mut().push(notification);
    });
    // The intrusion fires during initialization; the nested transaction owns
    // the route and no runtime borrow is live across the delegate query.
    let _widget: Widget = router.into_widget();

    assert_eq!(provider.value().location(), "/intruder");
    assert!(
        log.borrow().iter().all(|notification| notification
            .route_information
            .as_ref()
            .is_none_or(|information| { information.location() != "/initial" })),
        "the preempted initial route commits nothing: {log:?}"
    );
    assert!(
        log.borrow().iter().any(|notification| {
            notification.kind == NavigationNotificationKind::RouteInformationChanged
                && notification
                    .route_information
                    .as_ref()
                    .is_some_and(|information| information.location() == "/intruder")
        }),
        "the nested transaction commits: {log:?}"
    );
}

#[test]
fn router_restoration_failure_observers_can_claim_the_route() {
    // A failure observer delivers another route before the provider fallback
    // runs; the fallback must not overwrite the accepted claim.
    let restoration_backend = Rc::new(RestorationMemory::default());
    let scope = RestorationScope::root(restoration_backend.clone());
    let restoration_key = RestorationKey::new("router").expect("valid key");
    scope.set_json(&restoration_key, serde_json::json!({"location": 42}));
    let provider = MemoryRouteInformationProvider::new("/fallback");
    let provider_for_observer = provider.clone();
    let delegate = BasicRouterDelegate::new(|| SizedBox::shrink().into());
    delegate.on_set_new_route_path(|_: String| Ok(()));
    let config = RouterConfig::with_provider_parser(
        delegate.clone(),
        provider.clone(),
        StringRouteInformationParser,
    );
    let router =
        Router::from_config(config).restoration_scope(scope.clone(), restoration_key.clone());
    let log = navigation_log();
    let notifications = log.clone();
    let _events = router.on_navigation_notification(move |notification| {
        let claimed = notification.kind == NavigationNotificationKind::ParseFailed;
        notifications.borrow_mut().push(notification);
        if claimed {
            provider_for_observer.set_value("/claimed");
        }
    });
    let _widget: Widget = router.into_widget();

    assert_eq!(
        delegate.current_configuration().as_deref(),
        Some("/claimed")
    );
    assert_eq!(
        scope.get_json(&restoration_key),
        Some(RouteInformation::new("/claimed").to_json())
    );
    assert_eq!(
        notification_summary(&log.borrow()),
        [
            ("ParseFailed".to_owned(), None),
            ("DelegateChanged".to_owned(), Some("/claimed".to_owned())),
            (
                "RouteInformationChanged".to_owned(),
                Some("/claimed".to_owned())
            ),
        ]
    );
}

#[test]
fn router_reports_each_failed_attempt_once_then_recovers() {
    // Restoration and provider fallback are separate attempts: each failure
    // notifies exactly once, nothing commits, and the next valid route
    // recovers normally.
    let restoration_backend = Rc::new(RestorationMemory::default());
    let scope = RestorationScope::root(restoration_backend.clone());
    let restoration_key = RestorationKey::new("router").expect("valid key");
    scope.set_json(
        &restoration_key,
        RouteInformation::new("/restore").to_json(),
    );
    let provider = MemoryRouteInformationProvider::new("/initial");
    let delegate = BasicRouterDelegate::new(|| SizedBox::shrink().into());
    delegate.on_set_new_route_path(|configuration: String| {
        (configuration == "/next")
            .then_some(())
            .ok_or_else(|| RouterError::message("route not served"))
    });
    let config = RouterConfig::with_provider_parser(
        delegate.clone(),
        provider.clone(),
        StringRouteInformationParser,
    );
    let router =
        Router::from_config(config).restoration_scope(scope.clone(), restoration_key.clone());
    let log = navigation_log();
    let notifications = log.clone();
    let _events = router.on_navigation_notification(move |notification| {
        notifications.borrow_mut().push(notification);
    });
    let _widget: Widget = router.into_widget();

    // One failure notification per attempt, no success notification, and the
    // accepted layers are untouched: no delegate configuration, and the
    // persisted scope still holds the seed no attempt committed.
    assert_eq!(
        notification_summary(&log.borrow()),
        [
            ("ParseFailed".to_owned(), Some("/restore".to_owned())),
            ("ParseFailed".to_owned(), Some("/initial".to_owned())),
        ]
    );
    assert_eq!(delegate.current_configuration(), None);
    assert_eq!(
        scope.get_json(&restoration_key),
        Some(RouteInformation::new("/restore").to_json())
    );

    provider.set_value("/next");
    assert_eq!(delegate.current_configuration().as_deref(), Some("/next"));
    assert_eq!(
        scope.get_json(&restoration_key),
        Some(RouteInformation::new("/next").to_json())
    );
    assert_eq!(
        notification_summary(&log.borrow())[2..],
        [
            ("DelegateChanged".to_owned(), Some("/next".to_owned())),
            (
                "RouteInformationChanged".to_owned(),
                Some("/next".to_owned())
            ),
        ]
    );
}

#[test]
fn router_live_rejection_notifies_exactly_once() {
    // A live platform route rejected by the delegate produces exactly one
    // notification total: `receive_route_information` adds nothing on top of
    // the `ParseFailed` that `apply_route` already emitted.
    let restoration_backend = Rc::new(RestorationMemory::default());
    let scope = RestorationScope::root(restoration_backend.clone());
    let restoration_key = RestorationKey::new("router").expect("valid key");
    let provider = MemoryRouteInformationProvider::new("/initial");
    let delegate = BasicRouterDelegate::new(|| SizedBox::shrink().into());
    delegate.on_set_new_route_path(|configuration: String| {
        (configuration != "/reject")
            .then_some(())
            .ok_or_else(|| RouterError::message("route not served"))
    });
    let config = RouterConfig::with_provider_parser(
        delegate.clone(),
        provider.clone(),
        StringRouteInformationParser,
    );
    let router =
        Router::from_config(config).restoration_scope(scope.clone(), restoration_key.clone());
    let log = navigation_log();
    let notifications = log.clone();
    let _events = router.on_navigation_notification(move |notification| {
        notifications.borrow_mut().push(notification);
    });
    let _widget: Widget = router.into_widget();
    log.borrow_mut().clear();

    provider.set_value("/reject");

    assert_eq!(
        notification_summary(&log.borrow()),
        [("ParseFailed".to_owned(), Some("/reject".to_owned()))]
    );
    assert!(matches!(
        log.borrow()[0].error,
        Some(RouterError::Delegate(_))
    ));
    // The previously accepted delegate configuration and persisted scope
    // survive the rejection.
    assert_eq!(
        delegate.current_configuration().as_deref(),
        Some("/initial")
    );
    assert_eq!(
        scope.get_json(&restoration_key),
        Some(RouteInformation::new("/initial").to_json())
    );
}

#[test]
fn router_set_configuration_during_parse_is_adopted() {
    // A parser callback applies an independent configuration while the outer
    // route is in flight.  Delegate configuration, router state, persisted
    // scope, and notifications must agree on the newer change — the older
    // application commits nothing over it.
    let restoration_backend = Rc::new(RestorationMemory::default());
    let scope = RestorationScope::root(restoration_backend.clone());
    let restoration_key = RestorationKey::new("router").expect("valid key");
    let provider = MemoryRouteInformationProvider::new("/initial");
    let delegate = BasicRouterDelegate::new(|| SizedBox::shrink().into());
    let delegate_for_parser = delegate.clone();
    let parser = ClosureRouteInformationParser::new(
        move |information: &RouteInformation| {
            if information.location() == "/outer" {
                delegate_for_parser.set_configuration("/inner".to_owned());
            }
            Ok(information.location().to_owned())
        },
        |configuration: &String| Ok(RouteInformation::new(configuration)),
    );
    delegate.on_set_new_route_path(|_: String| Ok(()));
    let config = RouterConfig::with_provider_parser(delegate.clone(), provider.clone(), parser);
    let router =
        Router::from_config(config).restoration_scope(scope.clone(), restoration_key.clone());
    let log = navigation_log();
    let notifications = log.clone();
    let _events = router.on_navigation_notification(move |notification| {
        notifications.borrow_mut().push(notification);
    });
    let _widget: Widget = router.into_widget();
    log.borrow_mut().clear();

    provider.set_value("/outer");

    assert_eq!(delegate.current_configuration().as_deref(), Some("/inner"));
    assert_eq!(
        scope.get_json(&restoration_key),
        Some(RouteInformation::new("/inner").to_json())
    );
    assert_eq!(
        notification_summary(&log.borrow()),
        [("DelegateChanged".to_owned(), Some("/inner".to_owned())),]
    );

    provider.set_value("/next");
    assert_eq!(delegate.current_configuration().as_deref(), Some("/next"));
    assert_eq!(
        scope.get_json(&restoration_key),
        Some(RouteInformation::new("/next").to_json())
    );
}

#[test]
fn router_set_configuration_during_delegate_callback_is_adopted() {
    // Same agreement through the delegate hook: the callback-triggered
    // configuration wins, and the outer application neither overwrites it
    // nor emits a second commit.
    let restoration_backend = Rc::new(RestorationMemory::default());
    let scope = RestorationScope::root(restoration_backend.clone());
    let restoration_key = RestorationKey::new("router").expect("valid key");
    let provider = MemoryRouteInformationProvider::new("/initial");
    let delegate = BasicRouterDelegate::new(|| SizedBox::shrink().into());
    let delegate_for_callback = delegate.clone();
    delegate.on_set_new_route_path(move |configuration: String| {
        if configuration == "/outer" {
            delegate_for_callback.set_configuration("/inner".to_owned());
        }
        Ok(())
    });
    let config = RouterConfig::with_provider_parser(
        delegate.clone(),
        provider.clone(),
        StringRouteInformationParser,
    );
    let router =
        Router::from_config(config).restoration_scope(scope.clone(), restoration_key.clone());
    let log = navigation_log();
    let notifications = log.clone();
    let _events = router.on_navigation_notification(move |notification| {
        notifications.borrow_mut().push(notification);
    });
    let _widget: Widget = router.into_widget();
    log.borrow_mut().clear();

    provider.set_value("/outer");

    assert_eq!(delegate.current_configuration().as_deref(), Some("/inner"));
    assert_eq!(
        scope.get_json(&restoration_key),
        Some(RouteInformation::new("/inner").to_json())
    );
    assert_eq!(
        notification_summary(&log.borrow()),
        [("DelegateChanged".to_owned(), Some("/inner".to_owned())),]
    );
}

#[test]
fn router_standalone_delegate_change_still_commits() {
    // Guard against over-deferral: with no application open, a delegate
    // change commits router state directly.
    let restoration_backend = Rc::new(RestorationMemory::default());
    let scope = RestorationScope::root(restoration_backend.clone());
    let restoration_key = RestorationKey::new("router").expect("valid key");
    let provider = MemoryRouteInformationProvider::new("/initial");
    let delegate = BasicRouterDelegate::new(|| SizedBox::shrink().into());
    delegate.on_set_new_route_path(|_: String| Ok(()));
    let config = RouterConfig::with_provider_parser(
        delegate.clone(),
        provider.clone(),
        StringRouteInformationParser,
    );
    let router =
        Router::from_config(config).restoration_scope(scope.clone(), restoration_key.clone());
    let log = navigation_log();
    let notifications = log.clone();
    let _events = router.on_navigation_notification(move |notification| {
        notifications.borrow_mut().push(notification);
    });
    let _widget: Widget = router.into_widget();
    log.borrow_mut().clear();

    delegate.set_configuration("/direct".to_owned());

    assert_eq!(
        scope.get_json(&restoration_key),
        Some(RouteInformation::new("/direct").to_json())
    );
    assert_eq!(
        notification_summary(&log.borrow()),
        [("DelegateChanged".to_owned(), Some("/direct".to_owned())),]
    );
}

#[test]
fn router_nested_rejection_preserves_staged_reaction() {
    // A staged independent change survives a nested failure in the same
    // window: the failed attempt reports once and the staged route — not
    // the outer one — is adopted.
    let restoration_backend = Rc::new(RestorationMemory::default());
    let scope = RestorationScope::root(restoration_backend.clone());
    let restoration_key = RestorationKey::new("router").expect("valid key");
    let provider = MemoryRouteInformationProvider::new("/initial");
    let delegate = BasicRouterDelegate::new(|| SizedBox::shrink().into());
    let delegate_for_parser = delegate.clone();
    let provider_for_parser = provider.clone();
    let parser = ClosureRouteInformationParser::new(
        move |information: &RouteInformation| {
            if information.location() == "/outer" {
                delegate_for_parser.set_configuration("/staged".to_owned());
                provider_for_parser.set_value("/bad");
            }
            Ok(information.location().to_owned())
        },
        |configuration: &String| Ok(RouteInformation::new(configuration)),
    );
    delegate.on_set_new_route_path(|configuration: String| {
        (configuration != "/bad")
            .then_some(())
            .ok_or_else(|| RouterError::message("route not served"))
    });
    let config = RouterConfig::with_provider_parser(delegate.clone(), provider.clone(), parser);
    let router =
        Router::from_config(config).restoration_scope(scope.clone(), restoration_key.clone());
    let log = navigation_log();
    let notifications = log.clone();
    let _events = router.on_navigation_notification(move |notification| {
        notifications.borrow_mut().push(notification);
    });
    let _widget: Widget = router.into_widget();
    log.borrow_mut().clear();

    provider.set_value("/outer");

    assert_eq!(delegate.current_configuration().as_deref(), Some("/staged"));
    assert_eq!(
        scope.get_json(&restoration_key),
        Some(RouteInformation::new("/staged").to_json())
    );
    assert_eq!(
        notification_summary(&log.borrow()),
        [
            ("DelegateChanged".to_owned(), Some("/staged".to_owned())),
            ("ParseFailed".to_owned(), Some("/bad".to_owned())),
        ]
    );
}

#[test]
fn router_restoration_intrusion_is_adopted_without_fallback() {
    // A delegate change staged during the restoration application is adopted
    // as the initial route; the provider fallback must not overwrite it.
    let restoration_backend = Rc::new(RestorationMemory::default());
    let scope = RestorationScope::root(restoration_backend.clone());
    let restoration_key = RestorationKey::new("router").expect("valid key");
    scope.set_json(
        &restoration_key,
        RouteInformation::new("/restore").to_json(),
    );
    let provider = MemoryRouteInformationProvider::new("/fallback");
    let delegate = BasicRouterDelegate::new(|| SizedBox::shrink().into());
    let delegate_for_callback = delegate.clone();
    delegate.on_set_new_route_path(move |configuration: String| {
        if configuration == "/restore" {
            delegate_for_callback.set_configuration("/intruder".to_owned());
        }
        Ok(())
    });
    let config = RouterConfig::with_provider_parser(
        delegate.clone(),
        provider.clone(),
        StringRouteInformationParser,
    );
    let router =
        Router::from_config(config).restoration_scope(scope.clone(), restoration_key.clone());
    let log = navigation_log();
    let notifications = log.clone();
    let _events = router.on_navigation_notification(move |notification| {
        notifications.borrow_mut().push(notification);
    });
    let _widget: Widget = router.into_widget();

    assert_eq!(
        delegate.current_configuration().as_deref(),
        Some("/intruder")
    );
    assert_eq!(
        scope.get_json(&restoration_key),
        Some(RouteInformation::new("/intruder").to_json())
    );
    assert_eq!(
        notification_summary(&log.borrow()),
        [("DelegateChanged".to_owned(), Some("/intruder".to_owned())),]
    );
}

#[test]
fn basic_delegate_configuration_destructor_may_reenter() {
    use std::cell::Cell;

    #[derive(Clone)]
    struct ReentrantConfig {
        name: String,
        delegate: Option<BasicRouterDelegate<ReentrantConfig>>,
        armed: Rc<Cell<bool>>,
    }
    impl Drop for ReentrantConfig {
        fn drop(&mut self) {
            // Runs while the replaced configuration retires: the state
            // borrow must already be released or this reentrant application
            // panics.
            if self.armed.take()
                && let Some(delegate) = self.delegate.clone()
            {
                delegate.set_configuration(ReentrantConfig {
                    name: "reentrant".to_owned(),
                    delegate: None,
                    armed: Rc::new(Cell::new(false)),
                });
            }
        }
    }

    let delegate = BasicRouterDelegate::new(|| SizedBox::shrink().into());
    delegate.on_set_new_route_path(|_: ReentrantConfig| Ok(()));
    delegate.set_configuration(ReentrantConfig {
        name: "first".to_owned(),
        delegate: Some(delegate.clone()),
        armed: Rc::new(Cell::new(true)),
    });
    delegate
        .set_new_route_path(ReentrantConfig {
            name: "second".to_owned(),
            delegate: None,
            armed: Rc::new(Cell::new(false)),
        })
        .expect("application succeeds");

    // The retired configuration's destructor applied a newer one, which the
    // superseded outer application left intact.
    assert_eq!(
        delegate
            .current_configuration()
            .as_ref()
            .map(|config| config.name.as_str()),
        Some("reentrant")
    );
}

#[test]
fn router_config_rejects_an_unpaired_provider_and_parser() {
    // The missing-parser error path is unreachable at runtime because the
    // pairing contract is enforced at construction: no notification path is
    // needed for a configuration that cannot be built.
    let delegate = BasicRouterDelegate::new(|| SizedBox::shrink().into());
    let provider = MemoryRouteInformationProvider::new("/initial");
    assert!(matches!(
        RouterConfig::try_from_parts(Rc::new(delegate.clone()), Some(Rc::new(provider)), None,),
        Err(RouterError::InvalidConfiguration(_))
    ));
    assert!(matches!(
        RouterConfig::try_from_parts(
            Rc::new(delegate),
            None,
            Some(Rc::new(StringRouteInformationParser)),
        ),
        Err(RouterError::InvalidConfiguration(_))
    ));
}
