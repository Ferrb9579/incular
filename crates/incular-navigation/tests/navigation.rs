use incular_core::{Color, Size};
use incular_navigation::*;
use incular_widgets::internal::{OpacityController, TranslationController};
use incular_widgets::{Text, Widget};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

fn page() -> Widget {
    Widget::box_(Size::new(1., 1.), Color::WHITE)
}

#[test]
fn guarded_pop_does_not_remove_a_route_pushed_by_the_guard() {
    let navigator = Navigator::new();
    navigator.push_page(Page::new("candidate", page()));
    let reentrant = navigator.clone();
    navigator.set_pop_guard(move |_| {
        reentrant.push_page(Page::new("unapproved", page()));
        PopDecision::Allow
    });
    assert!(matches!(navigator.maybe_pop(), PopResult::Blocked));
    assert_eq!(navigator.current().unwrap().name, "unapproved");
    assert_eq!(navigator.routes().len(), 2);
}

#[test]
fn guarded_replace_handles_a_guard_removing_the_candidate() {
    let navigator = Navigator::new();
    navigator.push_page(Page::new("candidate", page()));
    let reentrant = navigator.clone();
    navigator.set_pop_guard(move |_| {
        reentrant.clear_pop_guard();
        let _ = reentrant.pop();
        PopDecision::Allow
    });
    assert!(
        navigator
            .replace(Route::new("replacement", page()))
            .is_none()
    );
    assert!(navigator.current().is_none());
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
fn declarative_navigation_builders_accept_generic_widgets() {
    let page = Page::builder()
        .name("home")
        .child(Text::new("Home"))
        .build();
    let entry = OverlayEntry::builder().child(Text::new("Overlay")).build();
    let dialog = Dialog::builder().child(Text::new("Dialog")).build();
    let sheet = BottomSheet::builder()
        .child(Text::new("Bottom sheet"))
        .build();
    let barrier = ModalBarrier::builder().build();
    let route = Route::builder()
        .name("route")
        .child(Text::new("Route"))
        .build();
    let page_route = PageRouteBuilder::builder()
        .name("settings")
        .child(Text::new("Settings"))
        .presentation(RoutePresentation::popup(None))
        .build()
        .settings(RouteSettings::new("renamed"))
        .build();

    assert_eq!(page.name, "home");
    assert!(!entry.blocks_background_input());
    assert!(dialog.barrier.dismissible);
    assert!(sheet.barrier.dismissible);
    assert_eq!(barrier.color, Color::rgba(0, 0, 0, 128));
    assert_eq!(route.settings.name(), "route");
    assert_eq!(page_route.name, "renamed");
    assert_eq!(page_route.settings.name(), "renamed");
    assert!(!page_route.presentation.is_opaque());
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
            RestorableRoute::new(home, json!({ "name": "home" })).state(json!({ "tab": "recent" })),
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
    let report =
        registry.restore_navigator_or(&navigator, &missing_root, || Page::new("fallback", page()));
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
