use std::{cell::RefCell, collections::HashMap, rc::Rc};

use incular_config::RuntimeEnvironment;
use incular_core::{Color, Rect, RestorationBackend, RestorationKey, RestorationScope, Size};
use incular_widgets::{
    CheckedModeBanner, Directionality, ErrorWidget, Localizations, MediaQuery, SizedBox,
    Visibility, Widget,
};

#[path = "../src/app_shell/app.rs"]
mod app;
#[path = "../src/platform_widgets/menu.rs"]
// Source-inclusion parity harnesses compile the whole production module but
// deliberately exercise only the behavior under test.
#[allow(dead_code)]
mod menu;
#[path = "../src/app_shell/router.rs"]
#[allow(dead_code)]
mod router;
#[path = "../src/app_shell/title.rs"]
#[allow(dead_code)]
mod title;
#[path = "../src/app_shell/view.rs"]
mod view;

use app::{ApplicationBootstrapHost, WidgetsApp};
use menu::{
    MemoryPlatformMenuDelegate, MenuDispatchResult, PlatformMenu, PlatformMenuBarController,
    PlatformMenuItem, PlatformMenuUpdate,
};
use router::*;
use title::*;
use view::{
    AuxiliaryViewError, AuxiliaryViewHandle, AuxiliaryViewHost, AuxiliaryViewOutcome,
    AuxiliaryViewRequest, ViewAnchorController, ViewController, ViewId, ViewLifecycle, ViewMetrics,
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
        type Handle = (String, Size);
        type Error = std::convert::Infallible;

        fn create_application(
            &self,
            specification: app::ApplicationBootstrapSpec,
        ) -> Result<Self::Handle, Self::Error> {
            Ok((
                specification.options.title,
                specification.options.initial_size,
            ))
        }
    }

    let (title, size) = app.bootstrap(&TestHost).expect("bootstrap succeeds");
    assert_eq!(title, "Incular");
    assert_eq!(size, Size::new(800.0, 600.0));
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
        Err(title::TitleError::TransparentColor)
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
    let delegate = MemoryPlatformMenuDelegate::new();
    let controller = PlatformMenuBarController::new(Rc::new(delegate.clone()));
    assert_eq!(controller.install(&[menu]), Ok(PlatformMenuUpdate::Applied));
    assert_eq!(
        controller.dispatch(&"open".into()),
        MenuDispatchResult::Handled
    );
    assert_eq!(*selected.borrow(), 1);
    assert_eq!(delegate.owner(), Some(controller.owner()));
    assert!(matches!(controller.detach(), PlatformMenuUpdate::Applied));
    assert!(delegate.owner().is_none());

    let unsupported = PlatformMenuBarController::new(Rc::new(menu::NoopPlatformMenuDelegate));
    assert_eq!(
        unsupported.install(&[]),
        Ok(PlatformMenuUpdate::NoOpUnsupported)
    );
}
