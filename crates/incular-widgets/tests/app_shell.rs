use std::{cell::RefCell, collections::HashMap, rc::Rc};

use incular_config::{RuntimeEnvironment, TransparencyMode, WindowSizePolicy};
use incular_core::{Color, Rect, RestorationBackend, RestorationKey, RestorationScope, Size};
use incular_widgets::{
    ApplicationBootstrapHost, ApplicationBootstrapSpec, AuxiliaryViewError, AuxiliaryViewHandle,
    AuxiliaryViewHost, AuxiliaryViewOutcome, AuxiliaryViewRequest, BasicRouterDelegate,
    ErrorWidget, MemoryRouteInformationProvider, MenuDispatchResult, MenuOwnerId,
    NoopPlatformMenuDelegate, PlatformMenu, PlatformMenuBar, PlatformMenuBarController,
    PlatformMenuDelegate, PlatformMenuEvent, PlatformMenuItem, PlatformMenuSnapshot,
    PlatformMenuUpdate, RootBackButtonDispatcher, Router, RouterConfig, RouterDelegate, SizedBox,
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
