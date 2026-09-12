use incular_core::{Color, Size};
use incular_navigation::*;
use incular_widgets::internal::{OpacityController, TranslationController};
use incular_widgets::{GestureDetector, Text, Widget};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

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
    root.attach_child(&child).unwrap();
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
fn back_navigator() -> Navigator {
    let navigator = Navigator::new();
    navigator.push_page(Page::new("base", page()));
    navigator.push_page(Page::new("top", page()));
    navigator
}

#[test]
fn attach_child_rejects_self_attachment_unchanged() {
    let root = BackDispatcher::new(back_navigator());
    let child = BackDispatcher::new(back_navigator());
    root.attach_child(&child).unwrap();
    root.set_active_child(Some(&child));
    assert_eq!(
        root.attach_child(&root),
        Err(BackAttachError::SelfAttachment)
    );
    // Rejection leaves the topology untouched: dispatch still routes to
    // the attached active child.
    let report = root.dispatch_back();
    assert!(report.handled);
    assert_eq!(report.depth, 1);
}

#[test]
fn attach_child_rejects_two_node_and_longer_cycles() {
    let first = BackDispatcher::new(back_navigator());
    let second = BackDispatcher::new(back_navigator());
    first.attach_child(&second).unwrap();
    assert_eq!(second.attach_child(&first), Err(BackAttachError::Cycle));
    // The rejected parent still dispatches its own stack at depth zero.
    let report = second.dispatch_back();
    assert!(report.handled);
    assert_eq!(report.depth, 0);

    let third = BackDispatcher::new(back_navigator());
    second.attach_child(&third).unwrap();
    assert_eq!(third.attach_child(&first), Err(BackAttachError::Cycle));
    // A diamond is acyclic and stays permitted.
    assert!(first.attach_child(&third).is_ok());
    first.set_active_child(Some(&second));
    second.set_active_child(Some(&third));
    let report = first.dispatch_back();
    assert!(report.handled);
    assert_eq!(report.depth, 2);
}

#[test]
fn duplicate_attachment_is_idempotent_and_detachable() {
    let root = BackDispatcher::new(back_navigator());
    let child = BackDispatcher::new(back_navigator());
    assert!(root.attach_child(&child).is_ok());
    assert!(root.attach_child(&child).is_ok());
    root.set_active_child(Some(&child));
    let report = root.dispatch_back();
    assert_eq!(report.depth, 1);
    // One detach removes the single stored edge.
    root.detach_child(&child);
    let report = root.dispatch_back();
    assert_eq!(report.depth, 0);
    // Reattachment after detachment dispatches again.
    child.navigator().push_page(Page::new("top", page()));
    root.attach_child(&child).unwrap();
    root.set_active_child(Some(&child));
    let report = root.dispatch_back();
    assert_eq!(report.depth, 1);
}

#[test]
fn dead_children_do_not_break_attach_or_dispatch() {
    let root = BackDispatcher::new(back_navigator());
    {
        let ephemeral = BackDispatcher::new(back_navigator());
        root.attach_child(&ephemeral).unwrap();
    }
    // The dead weak entry is skipped: attaching and dispatching through a
    // live child still work.
    let live = BackDispatcher::new(back_navigator());
    root.attach_child(&live).unwrap();
    root.set_active_child(Some(&live));
    let report = root.dispatch_back();
    assert!(report.handled);
    assert_eq!(report.depth, 1);
}

#[test]
fn deep_active_chain_dispatches_once_at_depth() {
    const DEPTH: usize = 50;
    let dispatchers: Vec<BackDispatcher> = (0..=DEPTH)
        .map(|_| BackDispatcher::new(back_navigator()))
        .collect();
    for pair in dispatchers.windows(2) {
        pair[0].attach_child(&pair[1]).unwrap();
        pair[0].set_active_child(Some(&pair[1]));
    }
    // An acyclic chain always terminates, visiting each level once.
    let report = dispatchers[0].dispatch_back();
    assert!(report.handled);
    assert!(!report.blocked);
    assert_eq!(report.depth, DEPTH);
}

#[test]
fn inactive_branch_never_handles_back() {
    let root = BackDispatcher::new(back_navigator());
    let active = BackDispatcher::new(back_navigator());
    let idle = BackDispatcher::new(back_navigator());
    root.attach_child(&active).unwrap();
    root.attach_child(&idle).unwrap();
    root.set_active_child(Some(&active));
    let report = root.dispatch_back();
    assert_eq!(report.depth, 1);
    assert_eq!(active.navigator().current().unwrap().name, "base");
    assert_eq!(idle.navigator().current().unwrap().name, "top");
    assert_eq!(root.navigator().current().unwrap().name, "top");
}

#[test]
fn declarative_pages_preserve_keyed_route_identity() {
    let home = PageKey::new("home").unwrap();
    let navigator = Navigator::new();
    navigator
        .set_pages([
            Page::new("home", page()).key(home.clone()),
            Page::new("settings", page()).key(PageKey::new("settings").unwrap()),
        ])
        .unwrap();
    let id = navigator.routes()[0].id;
    navigator
        .set_pages([Page::new("home", page()).key(home)])
        .unwrap();
    assert_eq!(navigator.routes()[0].id, id);
}

#[test]
fn unkeyed_pages_mount_anew_on_every_reconciliation() {
    let navigator = Navigator::new();
    navigator
        .set_pages([Page::new("home", page()), Page::new("settings", page())])
        .unwrap();
    let ids: Vec<RouteId> = navigator.routes().iter().map(|route| route.id).collect();
    navigator
        .set_pages([Page::new("home", page()), Page::new("settings", page())])
        .unwrap();
    let fresh: Vec<RouteId> = navigator.routes().iter().map(|route| route.id).collect();
    assert_ne!(ids, fresh);
}

#[test]
fn keyed_reorder_preserves_route_lifetimes() {
    let key = |name: &str| PageKey::new(name).unwrap();
    let navigator = Navigator::new();
    navigator
        .set_pages([
            Page::new("home", page()).key(key("home")),
            Page::new("search", page()).key(key("search")),
            Page::new("settings", page()).key(key("settings")),
        ])
        .unwrap();
    let ids: Vec<RouteId> = navigator.routes().iter().map(|route| route.id).collect();
    navigator
        .set_pages([
            Page::new("settings", page()).key(key("settings")),
            Page::new("home", page()).key(key("home")),
            Page::new("search", page()).key(key("search")),
        ])
        .unwrap();
    let reordered: Vec<RouteId> = navigator.routes().iter().map(|route| route.id).collect();
    assert_eq!(reordered, vec![ids[2], ids[0], ids[1]]);
    assert_eq!(
        navigator.current().unwrap().id,
        ids[1],
        "the active route follows its key, not its position"
    );
}

#[test]
fn repeated_names_with_different_keys_coexist_stably() {
    let navigator = Navigator::new();
    navigator
        .set_pages([
            Page::new("detail", page()).key(PageKey::new("detail-a").unwrap()),
            Page::new("detail", page()).key(PageKey::new("detail-b").unwrap()),
        ])
        .unwrap();
    let ids: Vec<RouteId> = navigator.routes().iter().map(|route| route.id).collect();
    assert_ne!(ids[0], ids[1]);
    // Reconciling the same keys in the same order keeps both lifetimes,
    // matched by key rather than by name position.
    navigator
        .set_pages([
            Page::new("detail", page()).key(PageKey::new("detail-a").unwrap()),
            Page::new("detail", page()).key(PageKey::new("detail-b").unwrap()),
        ])
        .unwrap();
    let stable: Vec<RouteId> = navigator.routes().iter().map(|route| route.id).collect();
    assert_eq!(stable, ids);
}

#[test]
fn duplicate_page_keys_reject_without_mutation_or_events() {
    let navigator = Navigator::new();
    navigator
        .set_pages([Page::new("home", page()).key(PageKey::new("home").unwrap())])
        .unwrap();
    let before = navigator.routes();
    let revision = navigator.revision();
    let events = Rc::new(RefCell::new(Vec::new()));
    let _observer = navigator.observe({
        let events = Rc::clone(&events);
        move |_| events.borrow_mut().push(())
    });
    let result = navigator.set_pages([
        Page::new("home", page()).key(PageKey::new("home").unwrap()),
        Page::new("settings", page()).key(PageKey::new("home").unwrap()),
    ]);
    assert_eq!(result.unwrap_err().key(), &PageKey::new("home").unwrap());
    assert_eq!(
        navigator
            .routes()
            .iter()
            .map(|route| route.id)
            .collect::<Vec<_>>(),
        before.iter().map(|route| route.id).collect::<Vec<_>>()
    );
    assert_eq!(navigator.revision(), revision);
    assert!(events.borrow().is_empty());
}

#[test]
fn removed_and_reinserted_keys_start_new_lifetimes() {
    let home = PageKey::new("home").unwrap();
    let navigator = Navigator::new();
    navigator
        .set_pages([
            Page::new("home", page()).key(home.clone()),
            Page::new("settings", page()).key(PageKey::new("settings").unwrap()),
        ])
        .unwrap();
    let id = navigator.routes()[0].id;
    navigator
        .set_pages([Page::new("settings", page()).key(PageKey::new("settings").unwrap())])
        .unwrap();
    assert_eq!(navigator.routes().len(), 1);
    navigator
        .set_pages([
            Page::new("settings", page()).key(PageKey::new("settings").unwrap()),
            Page::new("home", page()).key(home),
        ])
        .unwrap();
    let reinserted = navigator.routes()[1].id;
    assert_ne!(
        reinserted, id,
        "reinsertion mounts a new lifetime rather than resurrecting the removed route"
    );
}

#[test]
fn set_pages_iterators_may_inspect_and_mutate_first() {
    // The iterator drains fully before any state is borrowed, so reading
    // the navigator inside it cannot panic and mutations complete first.
    let navigator = Navigator::new();
    navigator.push_page(Page::new("seed", page()));
    let seen = navigator.set_pages((0..2).map(|index| {
        let _ = navigator.routes();
        Page::new(format!("page-{index}"), page()).key(PageKey::new(format!("k-{index}")).unwrap())
    }));
    assert!(seen.is_ok());
    assert_eq!(navigator.routes().len(), 2);

    // A mutation performed by the iterator lands first; reconciliation
    // then matches the materialized pages against the mutated stack.
    let navigator = Navigator::new();
    navigator.push_page(Page::new("seed", page()).key(PageKey::new("seed").unwrap()));
    let seed_id = navigator.routes()[0].id;
    navigator
        .set_pages(
            [
                Page::new("seed", page()).key(PageKey::new("seed").unwrap()),
                Page::new("extra", page()).key(PageKey::new("extra").unwrap()),
            ]
            .into_iter()
            .inspect(|declared| {
                if declared.name == "extra" {
                    navigator.push_page(Page::new("unrelated", page()));
                }
            }),
        )
        .unwrap();
    // The pushed route is dropped by the reconciliation (it was not in the
    // materialized page list) while the keyed seed lifetime survives.
    let names: Vec<String> = navigator
        .routes()
        .iter()
        .map(|route| route.name.clone())
        .collect();
    assert_eq!(names, ["seed", "extra"]);
    assert_eq!(navigator.routes()[0].id, seed_id);
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
fn mixed_stacks_keep_each_entry_metadata_associated() {
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
            RestorableRoute::new(home, json!({ "name": "home" })).state(json!({ "tab": 1 })),
        )
        .unwrap();
    navigator.push_page(Page::new("transient-dialog-route", page()));
    // An ordinary top entry carries no metadata of its own.
    assert!(navigator.current_restorable_route().is_none());
    assert!(!navigator.set_current_restorable_state(json!({ "tab": 2 })));

    registry
        .navigate_restorable(
            &navigator,
            RestorableRoute::new(detail, json!({ "name": "detail" }))
                .state(json!({ "document": 42 })),
        )
        .unwrap();
    assert_eq!(
        navigator.current_restorable_route().unwrap().state,
        json!({ "document": 42 })
    );
    assert!(navigator.set_current_restorable_state(json!({ "document": 43 })));

    // Removing the top restorable entry exposes the ordinary middle entry
    // without disturbing the bottom entry's stored state.
    assert_eq!(navigator.pop().unwrap().name, "detail");
    assert!(navigator.current_restorable_route().is_none());
    assert_eq!(navigator.pop().unwrap().name, "transient-dialog-route");
    assert_eq!(
        navigator.current_restorable_route().unwrap().state,
        json!({ "tab": 1 })
    );

    // The snapshot prefix stops at the first ordinary route, so only the
    // bottom entry persists.
    navigator.push_page(Page::new("another-transient", page()));
    let snapshot = navigator.restoration_snapshot();
    assert_eq!(snapshot.routes.len(), 1);
    assert_eq!(snapshot.routes[0].state, json!({ "tab": 1 }));
    assert_eq!(snapshot.active_route, Some(0));
}

#[test]
fn replace_discards_the_replaced_entry_metadata_and_cleans_its_scope() {
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
    let previous = navigator
        .replace(Route::new("settings", page()))
        .expect("replace succeeds");
    assert_eq!(previous.name, "document");
    assert_eq!(&*removed.borrow(), &["document-42"]);
    // The replacement entry is ordinary: no metadata survives the swap.
    assert!(navigator.current_restorable_route().is_none());
    assert!(!navigator.set_current_restorable_state(json!({ "x": 1 })));
    // An empty snapshot reports no active route instead of underflowing.
    let snapshot = navigator.restoration_snapshot();
    assert!(snapshot.routes.is_empty());
    assert_eq!(snapshot.active_route, None);
}

#[test]
fn restored_entries_carry_metadata_into_later_snapshots() {
    let registry = RouteRegistry::new();
    let home = registry
        .register_restorable("/home", restorable_page)
        .unwrap();
    let detail = registry
        .register_restorable("/detail", restorable_page)
        .unwrap();
    let source = Navigator::new();
    registry
        .navigate_restorable(
            &source,
            RestorableRoute::new(home, json!({ "name": "home" })).state(json!({ "tab": 3 })),
        )
        .unwrap();
    registry
        .navigate_restorable(
            &source,
            RestorableRoute::new(detail, json!({ "name": "detail" }))
                .state(json!({ "document": 7 })),
        )
        .unwrap();
    let snapshot = source.restoration_snapshot();

    let restored = Navigator::new();
    let report = registry.restore_navigator(&restored, &snapshot);
    assert_eq!(report.restored_routes, 2);
    // Each restored entry kept its own metadata by position.
    assert_eq!(restored.routes()[0].name, "home");
    assert_eq!(
        restored.current_restorable_route().unwrap().state,
        json!({
            "document": 7
        })
    );
    let round_trip = restored.restoration_snapshot();
    assert_eq!(round_trip.routes.len(), 2);
    assert_eq!(round_trip.routes[0].state, json!({ "tab": 3 }));
    assert_eq!(round_trip.routes[1].state, json!({ "document": 7 }));
    assert_eq!(round_trip.active_route, Some(1));
}

fn event_log() -> (Rc<RefCell<Vec<String>>>, impl Fn(NavigationEvent) + Clone) {
    let events = Rc::new(RefCell::new(Vec::<String>::new()));
    let sink = {
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
            NavigationEvent::ActiveRouteChanged { previous, current } => {
                events.borrow_mut().push(format!(
                    "active:{}:{}",
                    previous.map_or_else(|| "none".to_owned(), |route| route.name),
                    current.map_or_else(|| "none".to_owned(), |route| route.name),
                ));
            }
        }
    };
    (events, sink)
}

#[test]
fn declarative_removal_drops_scopes_without_bridge_exactly_once() {
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
    // Declarative removal drops the entry without the pop-only bridge,
    // and repeating the reconciliation cleans up nothing further.
    navigator.set_pages([]).unwrap();
    assert!(navigator.routes().is_empty());
    assert!(removed.borrow().is_empty());
    navigator.set_pages([]).unwrap();
    assert!(removed.borrow().is_empty());
}

#[test]
fn keyed_reorder_preserves_lifetime_metadata_and_skips_cleanup() {
    fn keyed_page(key: &str, arguments: &Value) -> Result<Page, RestorableRouteBuildError> {
        let name = arguments
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| RestorableRouteBuildError::invalid_arguments("missing name"))?;
        Ok(
            Page::new(name, Widget::box_(Size::new(1., 1.), Color::WHITE))
                .key(PageKey::new(key).unwrap()),
        )
    }
    let registry = RouteRegistry::new();
    let home = registry
        .register_restorable_with_scope_cleanup("/home", true, |arguments| {
            keyed_page("home", arguments)
        })
        .unwrap();
    let detail = registry
        .register_restorable_with_scope_cleanup("/detail", true, |arguments| {
            keyed_page("detail", arguments)
        })
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
            RestorableRoute::new(home, json!({ "name": "home" })).state(json!({ "tab": 1 })),
        )
        .unwrap();
    registry
        .navigate_restorable(
            &navigator,
            RestorableRoute::new(detail, json!({ "name": "detail" }))
                .state(json!({ "document": 2 })),
        )
        .unwrap();
    let ids: Vec<RouteId> = navigator.routes().iter().map(|route| route.id).collect();

    // Reordering by key keeps both lifetimes and their metadata, with no
    // scope cleanup: nothing was permanently popped.
    navigator
        .set_pages([
            Page::new("detail", page()).key(PageKey::new("detail").unwrap()),
            Page::new("home", page()).key(PageKey::new("home").unwrap()),
        ])
        .unwrap();
    let reordered: Vec<RouteId> = navigator.routes().iter().map(|route| route.id).collect();
    assert_eq!(reordered, vec![ids[1], ids[0]]);
    assert!(removed.borrow().is_empty());
    let snapshot = navigator.restoration_snapshot();
    assert_eq!(snapshot.routes.len(), 2);
    assert_eq!(snapshot.routes[0].state, json!({ "document": 2 }));
    assert_eq!(snapshot.routes[1].state, json!({ "tab": 1 }));
    assert_eq!(snapshot.active_route, Some(1));
}

#[test]
fn reentrant_observer_push_during_pop_keeps_commit_order() {
    let navigator = Navigator::new();
    navigator.push_page(Page::new("home", page()));
    navigator.push_page(Page::new("settings", page()));
    let (events, sink) = event_log();
    let _observer = navigator.observe({
        let navigator = navigator.clone();
        move |event| {
            sink(event.clone());
            if matches!(event, NavigationEvent::Popped { .. }) {
                navigator.push_page(Page::new("interrupted", page()));
            }
        }
    });
    assert!(navigator.pop().is_some());
    // The outer pop's events describe its own commit; the nested push
    // dispatches fully inside the outer Popped delivery. Nothing panics
    // and the final stack holds the nested route on the committed base.
    assert_eq!(
        *events.borrow(),
        [
            "pop:settings",
            "push:interrupted",
            "active:home:interrupted",
            "active:settings:home",
        ]
        .map(str::to_owned)
    );
    assert_eq!(navigator.current().unwrap().name, "interrupted");
}

#[test]
fn cleanup_callback_may_navigate_without_panic() {
    let registry = RouteRegistry::new();
    let document = registry
        .register_restorable_with_scope_cleanup("/document", true, restorable_page)
        .unwrap();
    let navigator = Navigator::new();
    let (events, sink) = event_log();
    let _observer = navigator.observe(sink);
    let log = Rc::new(RefCell::new(Vec::new()));
    navigator.set_route_scope_cleanup({
        let log = Rc::clone(&log);
        let navigator = navigator.clone();
        move |key| {
            log.borrow_mut().push(format!("cleanup:{}", key));
            navigator.push_page(Page::new("followup", page()));
        }
    });
    registry
        .navigate_restorable(
            &navigator,
            RestorableRoute::new(document, json!({ "name": "document" }))
                .scope_key(RouteScopeKey::new("document-42").unwrap()),
        )
        .unwrap();
    events.borrow_mut().clear();
    navigator.pop();
    // Cleanup runs before observer events; its nested push dispatches
    // fully before the outer pop's events continue. The pop already
    // committed, so the nested push observes the emptied stack.
    assert_eq!(&*log.borrow(), &["cleanup:document-42"]);
    assert_eq!(
        *events.borrow(),
        [
            "push:followup",
            "active:none:followup",
            "pop:document",
            "active:document:none",
        ]
        .map(str::to_owned)
    );
    assert_eq!(navigator.current().unwrap().name, "followup");
}

#[test]
fn active_events_match_committed_transitions() {
    let home = PageKey::new("home").unwrap();
    let navigator = Navigator::new();
    let (events, sink) = event_log();
    let _observer = navigator.observe(sink);
    navigator.push_page(Page::new("home", page()).key(home.clone()));
    navigator.push_page(Page::new("details", page()));
    navigator.replace(Route::new("settings", page()));
    navigator.pop();
    // Same-top reconciliation emits no active event; emptying the stack
    // reports the transition to none.
    navigator
        .set_pages([Page::new("home", page()).key(home)])
        .unwrap();
    navigator.set_pages([]).unwrap();
    assert_eq!(
        *events.borrow(),
        [
            "push:home",
            "active:none:home",
            "push:details",
            "active:home:details",
            "replace:details:settings",
            "active:details:settings",
            "pop:settings",
            "active:settings:home",
            "active:home:none",
        ]
        .map(str::to_owned)
    );
}

#[test]
fn rejected_operations_produce_no_effects() {
    let navigator = Navigator::new();
    navigator.push_page(Page::new("home", page()).key(PageKey::new("home").unwrap()));
    navigator.push_page(Page::new("editor", page()));
    let (events, sink) = event_log();
    let _observer = navigator.observe(sink);
    let log = Rc::new(RefCell::new(Vec::new()));
    navigator.set_route_scope_cleanup({
        let log = Rc::clone(&log);
        move |key| log.borrow_mut().push(key.to_string())
    });
    let revision = navigator.revision();

    // Denied pop: no mutation, no events, no cleanup.
    navigator.set_pop_guard(|route| {
        if route.name == "editor" {
            PopDecision::Deny
        } else {
            PopDecision::Allow
        }
    });
    assert!(matches!(navigator.maybe_pop(), PopResult::Blocked));
    assert_eq!(navigator.current().unwrap().name, "editor");
    assert_eq!(navigator.revision(), revision);
    assert!(events.borrow().is_empty());
    assert!(log.borrow().is_empty());

    // Denied replace: same guarantees.
    navigator.clear_pop_guard();
    navigator.set_pop_guard(|_| PopDecision::Deny);
    assert!(navigator.replace(Route::new("other", page())).is_none());
    assert_eq!(navigator.current().unwrap().name, "editor");
    assert_eq!(navigator.revision(), revision);
    assert!(events.borrow().is_empty());
    assert!(log.borrow().is_empty());
    navigator.clear_pop_guard();

    // Duplicate declarative keys: rejected before mutation or events.
    let duplicate = navigator.set_pages([
        Page::new("home", page()).key(PageKey::new("home").unwrap()),
        Page::new("editor", page()).key(PageKey::new("home").unwrap()),
    ]);
    assert!(duplicate.is_err());
    assert_eq!(navigator.current().unwrap().name, "editor");
    assert_eq!(navigator.revision(), revision);
    assert!(events.borrow().is_empty());
    assert!(log.borrow().is_empty());

    // Empty pop on a drained navigator: no events either.
    navigator.pop();
    navigator.pop();
    events.borrow_mut().clear();
    assert!(navigator.pop().is_none());
    assert!(matches!(navigator.maybe_pop(), PopResult::Empty));
    assert!(events.borrow().is_empty());
}

#[test]
fn same_key_rename_updates_name_and_keeps_metadata() {
    fn keyed_page(key: &str, arguments: &Value) -> Result<Page, RestorableRouteBuildError> {
        let name = arguments
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| RestorableRouteBuildError::invalid_arguments("missing name"))?;
        Ok(Page::new(name, page()).key(PageKey::new(key).unwrap()))
    }
    let registry = RouteRegistry::new();
    let detail = registry
        .register_restorable("/detail", |arguments| keyed_page("detail", arguments))
        .unwrap();
    let navigator = Navigator::new();
    registry
        .navigate_restorable(
            &navigator,
            RestorableRoute::new(detail, json!({ "name": "detail" }))
                .state(json!({ "document": 7 })),
        )
        .unwrap();
    let id = navigator.routes()[0].id;

    // A renamed page with the same key updates the name and its settings
    // mirror while preserving the route ID and restoration metadata.
    navigator
        .set_pages([Page::new("detail-renamed", page()).key(PageKey::new("detail").unwrap())])
        .unwrap();
    let routes = navigator.routes();
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].id, id);
    assert_eq!(routes[0].name, "detail-renamed");
    assert_eq!(routes[0].settings.name(), "detail-renamed");
    assert!(routes[0].presentation.is_opaque());
    assert_eq!(
        navigator.current_restorable_route().unwrap().state,
        json!({ "document": 7 })
    );
    let snapshot = navigator.restoration_snapshot();
    assert_eq!(snapshot.routes.len(), 1);
    assert_eq!(snapshot.routes[0].state, json!({ "document": 7 }));
}

#[test]
fn duplicate_live_key_claims_keep_the_topmost_entry() {
    // Imperative pushes perform no identity validation, so two live
    // entries may claim one key. Reconciliation keeps the topmost
    // (presented) claim deterministically instead of the first.
    let navigator = Navigator::new();
    let first = navigator.push_page(Page::new("detail", page()).key(PageKey::new("k").unwrap()));
    let second =
        navigator.push_page(Page::new("detail-again", page()).key(PageKey::new("k").unwrap()));
    assert_ne!(first, second);
    navigator
        .set_pages([Page::new("detail", page()).key(PageKey::new("k").unwrap())])
        .unwrap();
    let routes = navigator.routes();
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].id, second);
    assert_eq!(routes[0].name, "detail");
}

#[test]
fn large_keyed_reorder_commits_once_with_exact_permutation() {
    let navigator = Navigator::new();
    let count = 200usize;
    navigator
        .set_pages((0..count).map(|index| {
            Page::new(format!("page-{index}"), page())
                .key(PageKey::new(format!("k-{index}")).unwrap())
        }))
        .unwrap();
    let ids: Vec<RouteId> = navigator.routes().iter().map(|route| route.id).collect();
    let revision = navigator.revision();
    // Reverse the whole stack: one commit, every lifetime preserved at
    // its mirrored position.
    navigator
        .set_pages((0..count).rev().map(|index| {
            Page::new(format!("page-{index}"), page())
                .key(PageKey::new(format!("k-{index}")).unwrap())
        }))
        .unwrap();
    let reordered: Vec<RouteId> = navigator.routes().iter().map(|route| route.id).collect();
    let expected: Vec<RouteId> = ids.iter().rev().copied().collect();
    assert_eq!(reordered, expected);
    assert_eq!(navigator.revision(), revision.wrapping_add(1));
    assert_eq!(navigator.current().unwrap().id, ids[0]);
}

/// Application-owned value whose destruction observes the navigator.
///
/// The probe must be the final owner: tests relinquish every cloned
/// route, page, and widget so dropping the retired entry actually runs
/// this destructor instead of merely decrementing a shared count.
struct DropProbe {
    navigator: Navigator,
    log: Rc<RefCell<Vec<String>>>,
}
impl Drop for DropProbe {
    fn drop(&mut self) {
        let count = self.navigator.routes().len();
        self.log
            .borrow_mut()
            .push(format!("dropped:routes={count}"));
        self.navigator.push_page(Page::new("drop-child", page()));
    }
}

fn probed_child(navigator: &Navigator, log: &Rc<RefCell<Vec<String>>>) -> Widget {
    let probe = DropProbe {
        navigator: navigator.clone(),
        log: Rc::clone(log),
    };
    Widget::from(GestureDetector::new(page()).on_tap(move || {
        let _ = &probe;
    }))
}

#[test]
fn retired_route_widgets_drop_outside_borrows_with_reentrant_push() {
    let navigator = Navigator::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    navigator.push_page(Page::new("doomed", probed_child(&navigator, &log)));
    // Reconciling away the probed route retires its widget; destruction
    // runs after the borrow ends, observes the committed stack, and its
    // reentrant push survives the outer reconciliation.
    navigator.set_pages([Page::new("other", page())]).unwrap();
    assert_eq!(&*log.borrow(), &["dropped:routes=1"]);
    assert_eq!(
        navigator
            .routes()
            .iter()
            .map(|route| route.name.clone())
            .collect::<Vec<_>>(),
        ["other", "drop-child"]
    );
}

#[test]
fn restoration_replacement_drops_the_old_stack_outside_borrows() {
    let registry = RouteRegistry::new();
    registry.register("/seed", || Page::new("seed", page()));
    let navigator = Navigator::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    navigator.push_page(Page::new("doomed", probed_child(&navigator, &log)));
    // A whole-stack restore retires the probed route the same way:
    // destruction observes the committed restored stack.
    let home = registry
        .register_restorable("/home", restorable_page)
        .unwrap();
    let snapshot = NavigatorSnapshot {
        format_version: NAVIGATOR_SNAPSHOT_FORMAT_VERSION,
        routes: vec![RestorableRoute::new(home, json!({ "name": "home" }))],
        active_route: Some(0),
    };
    let report = registry.restore_navigator(&navigator, &snapshot);
    assert_eq!(report.restored_routes, 1);
    assert_eq!(&*log.borrow(), &["dropped:routes=1"]);
    assert_eq!(
        navigator
            .routes()
            .iter()
            .map(|route| route.name.clone())
            .collect::<Vec<_>>(),
        ["home", "drop-child"]
    );
}

#[test]
fn keyed_reorder_retires_only_the_replaced_child_outside_borrows() {
    let navigator = Navigator::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let key = PageKey::new("probed").unwrap();
    navigator.push_page(Page::new("placeholder", page()));
    // Mount the probed child under a key via reconciliation.
    navigator
        .set_pages([
            Page::new("placeholder", page()).key(PageKey::new("other").unwrap()),
            Page {
                name: "probed".to_owned(),
                child: probed_child(&navigator, &log),
                key: Some(key.clone()),
            },
        ])
        .unwrap();
    assert!(log.borrow().is_empty());
    let ids: Vec<RouteId> = navigator.routes().iter().map(|route| route.id).collect();
    // Reordering retains both entries: the only destructor is the
    // replaced child description, which drops exactly once — after the
    // borrow ends, observing the committed stack — while both lifetimes
    // survive. (Commit snapshots share the child until events dispatch,
    // so the drop lands after dispatch rather than during reconcile.)
    navigator
        .set_pages([
            Page {
                name: "probed".to_owned(),
                child: page(),
                key: Some(key),
            },
            Page::new("placeholder", page()).key(PageKey::new("other").unwrap()),
        ])
        .unwrap();
    assert_eq!(&*log.borrow(), &["dropped:routes=2"]);
    let reordered: Vec<RouteId> = navigator.routes().iter().map(|route| route.id).collect();
    assert_eq!(&reordered[..2], &[ids[1], ids[0]]);
    // The destructor's reentrant push survives the outer reconciliation.
    assert_eq!(
        navigator
            .routes()
            .iter()
            .map(|route| route.name.clone())
            .collect::<Vec<_>>(),
        ["probed", "placeholder", "drop-child"]
    );
}

#[test]
fn replaced_guard_drops_outside_borrows_with_reentrant_push() {
    let navigator = Navigator::new();
    navigator.push_page(Page::new("base", page()));
    let log = Rc::new(RefCell::new(Vec::new()));
    let probe = DropProbe {
        navigator: navigator.clone(),
        log: Rc::clone(&log),
    };
    navigator.set_pop_guard(move |_| {
        let _ = &probe;
        PopDecision::Allow
    });
    // Replacing the guard retires the probe closure outside the borrow:
    // destruction observes the committed stack and pushes through.
    navigator.set_pop_guard(|_| PopDecision::Allow);
    assert_eq!(&*log.borrow(), &["dropped:routes=1"]);
    assert_eq!(
        navigator
            .routes()
            .iter()
            .map(|route| route.name.clone())
            .collect::<Vec<_>>(),
        ["base", "drop-child"]
    );
    navigator.clear_pop_guard();
}

#[test]
fn replaced_cleanup_bridge_drops_outside_borrows() {
    let navigator = Navigator::new();
    navigator.push_page(Page::new("base", page()));
    let log = Rc::new(RefCell::new(Vec::new()));
    let probe = DropProbe {
        navigator: navigator.clone(),
        log: Rc::clone(&log),
    };
    navigator.set_route_scope_cleanup(move |_| {
        let _ = &probe;
    });
    // Bridge replacement retires the old closure the same way; the probe
    // observes the committed stack and its reentrant push survives.
    navigator.set_route_scope_cleanup(|_| {});
    assert_eq!(&*log.borrow(), &["dropped:routes=1"]);
    assert_eq!(navigator.current().unwrap().name, "drop-child");
    navigator.clear_route_scope_cleanup();
}

#[test]
fn commit_effects_deliver_identical_order_to_all_observers() {
    let navigator = Navigator::new();
    let first = Rc::new(RefCell::new(Vec::<String>::new()));
    let second = Rc::new(RefCell::new(Vec::<String>::new()));
    let sink = |events: Rc<RefCell<Vec<String>>>| {
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
            NavigationEvent::ActiveRouteChanged { previous, current } => {
                events.borrow_mut().push(format!(
                    "active:{}:{}",
                    previous.map_or_else(|| "none".to_owned(), |route| route.name),
                    current.map_or_else(|| "none".to_owned(), |route| route.name),
                ));
            }
        }
    };
    let _first = navigator.observe(sink(Rc::clone(&first)));
    let _second = navigator.observe(sink(Rc::clone(&second)));

    navigator.push_page(Page::new("home", page()).key(PageKey::new("home").unwrap()));
    navigator.push_page(Page::new("details", page()).key(PageKey::new("details").unwrap()));
    navigator.replace(Route::new("settings", page()));
    navigator.pop();
    // Declarative top change reports through the same shared path, while
    // same-top reconciliation emits nothing; emptying the stack reports
    // the transition to none.
    navigator
        .set_pages([Page::new("home", page()).key(PageKey::new("home").unwrap())])
        .unwrap();
    navigator.set_pages([]).unwrap();
    let expected = [
        "push:home",
        "active:none:home",
        "push:details",
        "active:home:details",
        "replace:details:settings",
        "active:details:settings",
        "pop:settings",
        "active:settings:home",
        "active:home:none",
    ]
    .map(str::to_owned);
    assert_eq!(*first.borrow(), expected);
    assert_eq!(*second.borrow(), expected);
}

#[test]
fn restoration_replacement_notifies_both_observers_once() {
    let registry = RouteRegistry::new();
    let home = registry
        .register_restorable("/home", restorable_page)
        .unwrap();
    let detail = registry
        .register_restorable("/detail", restorable_page)
        .unwrap();
    let navigator = Navigator::new();
    navigator.push_page(Page::new("seed", page()));
    let first = Rc::new(RefCell::new((0usize, 0usize)));
    let second = Rc::new(RefCell::new((0usize, 0usize)));
    let counter = |counts: Rc<RefCell<(usize, usize)>>| {
        move |event| {
            let mut counts = counts.borrow_mut();
            match event {
                NavigationEvent::ActiveRouteChanged { .. } => counts.0 += 1,
                _ => counts.1 += 1,
            }
        }
    };
    let _first = navigator.observe(counter(Rc::clone(&first)));
    let _second = navigator.observe(counter(Rc::clone(&second)));
    let snapshot = NavigatorSnapshot {
        format_version: NAVIGATOR_SNAPSHOT_FORMAT_VERSION,
        routes: vec![
            RestorableRoute::new(home, json!({ "name": "home" })),
            RestorableRoute::new(detail, json!({ "name": "detail" })),
        ],
        active_route: Some(1),
    };
    let report = registry.restore_navigator(&navigator, &snapshot);
    assert_eq!(report.restored_routes, 2);
    // Whole-stack replacement reports exactly one active transition to
    // each observer, and no push events for the restored routes.
    assert_eq!(*first.borrow(), (1, 0));
    assert_eq!(*second.borrow(), (1, 0));
    assert_eq!(navigator.current().unwrap().name, "detail");
}

#[test]
fn reentrant_cleanup_orders_before_observer_events_for_both() {
    let registry = RouteRegistry::new();
    let document = registry
        .register_restorable_with_scope_cleanup("/document", true, restorable_page)
        .unwrap();
    let navigator = Navigator::new();
    navigator.push_page(Page::new("home", page()));
    registry
        .navigate_restorable(
            &navigator,
            RestorableRoute::new(document, json!({ "name": "document" }))
                .scope_key(RouteScopeKey::new("document-42").unwrap()),
        )
        .unwrap();
    let order = Rc::new(RefCell::new(Vec::<String>::new()));
    let first = Rc::new(RefCell::new(Vec::<String>::new()));
    let second = Rc::new(RefCell::new(Vec::<String>::new()));
    let sink = |events: Rc<RefCell<Vec<String>>>| {
        move |event| match event {
            NavigationEvent::Pushed { route } => {
                events.borrow_mut().push(format!("push:{}", route.name));
            }
            NavigationEvent::Popped { route } => {
                events.borrow_mut().push(format!("pop:{}", route.name));
            }
            NavigationEvent::ActiveRouteChanged { previous, current } => {
                events.borrow_mut().push(format!(
                    "active:{}:{}",
                    previous.map_or_else(|| "none".to_owned(), |route| route.name),
                    current.map_or_else(|| "none".to_owned(), |route| route.name),
                ));
            }
            _ => {}
        }
    };
    let _first = navigator.observe(sink(Rc::clone(&first)));
    let _second = navigator.observe(sink(Rc::clone(&second)));
    navigator.set_route_scope_cleanup({
        let order = Rc::clone(&order);
        let navigator = navigator.clone();
        move |key| {
            order.borrow_mut().push(format!("cleanup:{}", key));
            navigator.push_page(Page::new("followup", page()));
        }
    });
    navigator.pop();
    // The reentrant cleanup push dispatches fully to both observers
    // before the outer pop's events continue, in identical order.
    let nested = ["push:followup", "active:home:followup"].map(str::to_owned);
    let outer = ["pop:document", "active:document:home"].map(str::to_owned);
    let expected: Vec<String> = nested.into_iter().chain(outer).collect();
    assert_eq!(&*order.borrow(), &["cleanup:document-42"]);
    assert_eq!(*first.borrow(), expected);
    assert_eq!(*second.borrow(), expected);
    assert_eq!(navigator.current().unwrap().name, "followup");
}

#[test]
fn non_top_child_replacement_retires_outside_borrows() {
    // The lower route's replaced child has no top-snapshot protection:
    // it must retire through the explicit collection, dropping after the
    // borrow ends with no panic, committed-state observation, and a
    // surviving reentrant push.
    let navigator = Navigator::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let low = PageKey::new("low").unwrap();
    let top = PageKey::new("top").unwrap();
    navigator.push_page(Page::new("lower", probed_child(&navigator, &log)).key(low.clone()));
    navigator.push_page(Page::new("top", page()).key(top.clone()));
    let ids: Vec<RouteId> = navigator.routes().iter().map(|route| route.id).collect();
    navigator
        .set_pages([
            Page {
                name: "lower".to_owned(),
                child: page(),
                key: Some(low),
            },
            Page::new("top", page()).key(top),
        ])
        .unwrap();
    assert_eq!(&*log.borrow(), &["dropped:routes=2"]);
    let routes = navigator.routes();
    assert_eq!(
        routes.iter().map(|route| route.id).collect::<Vec<_>>(),
        vec![ids[0], ids[1], routes[2].id]
    );
    assert_eq!(
        routes
            .iter()
            .map(|route| route.name.clone())
            .collect::<Vec<_>>(),
        ["lower", "top", "drop-child"]
    );
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
    navigator.set_pages([]).unwrap();
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

fn lifetime_counter(lifetime: &RouteLifetime) -> (RouteLifetimeSubscription, Rc<RefCell<usize>>) {
    let ended = Rc::new(RefCell::new(0usize));
    let ended_for_callback = ended.clone();
    let subscription = lifetime.on_ended(move || {
        *ended_for_callback.borrow_mut() += 1;
    });
    (subscription, ended)
}

#[test]
fn route_lifetime_ends_exactly_once_on_pop() {
    let navigator = Navigator::new();
    let first = navigator.push(Page::new("first", page()).into());
    let second = navigator.push(Page::new("second", page()).into());
    let first_lifetime = navigator
        .lifetime_of(first)
        .expect("mounted route has a lifetime");
    let second_lifetime = navigator
        .lifetime_of(second)
        .expect("mounted route has a lifetime");
    assert!(first_lifetime.is_live());
    assert!(second_lifetime.is_live());
    let (_first_sub, first_ended) = lifetime_counter(&first_lifetime);
    let (_second_sub, second_ended) = lifetime_counter(&second_lifetime);

    navigator.pop();
    assert!(!second_lifetime.is_live());
    assert_eq!(*second_ended.borrow(), 1);
    assert!(first_lifetime.is_live());
    assert_eq!(*first_ended.borrow(), 0);
    assert!(navigator.lifetime_of(second).is_none());

    navigator.pop();
    assert!(!first_lifetime.is_live());
    assert_eq!(*first_ended.borrow(), 1);
    // Popping an empty navigator ends nothing further.
    assert!(navigator.pop().is_none());
    assert_eq!(*first_ended.borrow(), 1);
    assert_eq!(*second_ended.borrow(), 1);
}

#[test]
fn route_lifetime_survives_keyed_reorder_and_retained_update() {
    let key = |name: &str| PageKey::new(name).unwrap();
    let navigator = Navigator::new();
    navigator
        .set_pages([
            Page::new("home", page()).key(key("home")),
            Page::new("search", page()).key(key("search")),
        ])
        .unwrap();
    let home_id = navigator.routes()[0].id;
    let search_id = navigator.routes()[1].id;
    let home_lifetime = navigator.lifetime_of(home_id).expect("home mounted");
    let search_lifetime = navigator.lifetime_of(search_id).expect("search mounted");
    let (_home_sub, home_ended) = lifetime_counter(&home_lifetime);
    let (_search_sub, search_ended) = lifetime_counter(&search_lifetime);

    // Reorder plus a retained rename/child update: identities — and their
    // lifetimes — move with the entries instead of ending.
    navigator
        .set_pages([
            Page::new("search!", page()).key(key("search")),
            Page::new("home", page()).key(key("home")),
        ])
        .unwrap();
    assert_eq!(navigator.routes()[0].id, search_id);
    assert_eq!(navigator.routes()[1].id, home_id);
    assert!(home_lifetime.is_live());
    assert!(search_lifetime.is_live());
    assert_eq!(*home_ended.borrow(), 0);
    assert_eq!(*search_ended.borrow(), 0);
    // The pre-reorder handles still track the entries: removing by key ends
    // exactly the removed lifetime.
    navigator
        .set_pages([Page::new("home", page()).key(key("home"))])
        .unwrap();
    assert!(!search_lifetime.is_live());
    assert_eq!(*search_ended.borrow(), 1);
    assert!(home_lifetime.is_live());
}

#[test]
fn route_lifetime_ends_on_replace_and_declarative_removal() {
    let key = |name: &str| PageKey::new(name).unwrap();
    let navigator = Navigator::new();
    navigator.push(Page::new("first", page()).into());
    let replaced_id = navigator.current().expect("top route").id;
    let replaced_lifetime = navigator.lifetime_of(replaced_id).expect("mounted");
    let (_replaced_sub, replaced_ended) = lifetime_counter(&replaced_lifetime);

    navigator.replace(Route::new("second", page()));
    assert!(!replaced_lifetime.is_live());
    assert_eq!(*replaced_ended.borrow(), 1);
    let current_lifetime = navigator
        .lifetime_of(navigator.current().expect("top route").id)
        .expect("replacement mounted");
    assert!(current_lifetime.is_live());

    // Declarative removal ends the lifetime through the same retirement
    // path, while persisted restoration data stays retained: the scope
    // bridge (pop-only by design) must not run for reconciliation.
    let scope_cleanups = Rc::new(RefCell::new(0usize));
    let scope_cleanups_for_bridge = scope_cleanups.clone();
    navigator.set_route_scope_cleanup(move |_| {
        *scope_cleanups_for_bridge.borrow_mut() += 1;
    });
    navigator
        .set_pages([Page::new("gone", page()).key(key("gone"))])
        .unwrap();
    assert!(!current_lifetime.is_live());
    assert_eq!(*scope_cleanups.borrow(), 0);
}

#[test]
fn route_lifetime_ends_on_restored_stack_replacement() {
    let registry = RouteRegistry::new();
    let home = registry
        .register_restorable("/home", restorable_page)
        .unwrap();
    let navigator = Navigator::new();
    navigator.push(Page::new("transient", page()).into());
    let transient_id = navigator.current().expect("top route").id;
    let transient_lifetime = navigator.lifetime_of(transient_id).expect("mounted");
    let (_transient_sub, transient_ended) = lifetime_counter(&transient_lifetime);

    let snapshot = NavigatorSnapshot {
        format_version: NAVIGATOR_SNAPSHOT_FORMAT_VERSION,
        active_route: Some(0),
        routes: vec![RestorableRoute::new(home, json!({ "name": "home" }))],
    };
    let report = registry.restore_navigator(&navigator, &snapshot);
    assert_eq!(report.restored_routes, 1);
    assert!(!transient_lifetime.is_live());
    assert_eq!(*transient_ended.borrow(), 1);
}

#[test]
fn route_lifetime_ends_on_fallback_replacement() {
    let registry = RouteRegistry::new();
    let navigator = Navigator::new();
    navigator.push(Page::new("stale", page()).into());
    let stale_id = navigator.current().expect("top route").id;
    let stale_lifetime = navigator.lifetime_of(stale_id).expect("mounted");
    let (_stale_sub, stale_ended) = lifetime_counter(&stale_lifetime);

    let missing = NavigatorSnapshot {
        format_version: NAVIGATOR_SNAPSHOT_FORMAT_VERSION,
        active_route: None,
        routes: Vec::new(),
    };
    let report =
        registry.restore_navigator_or(&navigator, &missing, || Page::new("fallback", page()));
    assert!(report.used_fallback);
    assert!(!stale_lifetime.is_live());
    assert_eq!(*stale_ended.borrow(), 1);
    assert!(navigator.current().expect("fallback mounted").name == "fallback");
}

#[test]
fn duplicate_page_key_leaves_lifetimes_untouched() {
    let key = |name: &str| PageKey::new(name).unwrap();
    let navigator = Navigator::new();
    navigator
        .set_pages([Page::new("home", page()).key(key("home"))])
        .unwrap();
    let home_id = navigator.routes()[0].id;
    let home_lifetime = navigator.lifetime_of(home_id).expect("mounted");
    let (_home_sub, home_ended) = lifetime_counter(&home_lifetime);

    let result = navigator.set_pages([
        Page::new("home", page()).key(key("home")),
        Page::new("clash", page()).key(key("home")),
    ]);
    assert!(result.is_err());
    assert!(home_lifetime.is_live());
    assert_eq!(*home_ended.borrow(), 0);
    assert_eq!(navigator.routes().len(), 1);
}

#[test]
fn reentrant_lifetime_callback_removes_safely() {
    let navigator = Navigator::new();
    navigator.push(Page::new("bottom", page()).into());
    let middle = navigator.push(Page::new("middle", page()).into());
    let top = navigator.push(Page::new("top", page()).into());
    let middle_lifetime = navigator.lifetime_of(middle).expect("mounted");
    let top_lifetime = navigator.lifetime_of(top).expect("mounted");
    let order = Rc::new(RefCell::new(Vec::new()));
    let order_for_top = order.clone();
    let order_for_middle = order.clone();
    let reentrant = navigator.clone();
    let (_top_sub, top_ended) = lifetime_counter(&top_lifetime);
    let (_middle_sub, middle_ended) = lifetime_counter(&middle_lifetime);
    // The top lifetime's own callback pops the middle reentrantly: nested
    // effects dispatch first, each lifetime ends exactly once, and no
    // borrow is live across either callback.
    let _top_reentrant_sub = top_lifetime.on_ended(move || {
        order_for_top.borrow_mut().push("top");
        let _ = reentrant.pop();
    });
    let _middle_order_sub = middle_lifetime.on_ended(move || {
        order_for_middle.borrow_mut().push("middle");
    });

    navigator.pop();
    assert_eq!(*order.borrow(), ["top", "middle"]);
    assert_eq!(*top_ended.borrow(), 1);
    assert_eq!(*middle_ended.borrow(), 1);
    assert!(!middle_lifetime.is_live());
    assert_eq!(navigator.routes().len(), 1);
}

#[test]
fn navigator_disposal_ends_mounted_lifetimes() {
    let ended = Rc::new(RefCell::new(0usize));
    let lifetimes = {
        let navigator = Navigator::new();
        navigator.push(Page::new("first", page()).into());
        navigator.push(Page::new("second", page()).into());
        let lifetimes: Vec<RouteLifetime> = navigator
            .routes()
            .iter()
            .map(|route| navigator.lifetime_of(route.id).expect("mounted"))
            .collect();
        // Subscriptions keep only their callbacks, never the navigator:
        // dropping every clone below must still dispose the state.
        let mut subscriptions = Vec::new();
        for lifetime in &lifetimes {
            let ended_for_callback = ended.clone();
            subscriptions.push(lifetime.on_ended(move || {
                *ended_for_callback.borrow_mut() += 1;
            }));
        }
        std::mem::forget(subscriptions);
        assert!(lifetimes.iter().all(|lifetime| lifetime.is_live()));
        lifetimes
    };
    // The navigator (and its single clone scope) is gone: disposal ended
    // both lifetimes exactly once through the explicit disposal policy.
    assert_eq!(*ended.borrow(), 2);
    assert!(lifetimes.iter().all(|lifetime| !lifetime.is_live()));
}

#[test]
fn event_route_snapshot_does_not_extend_mounted_lifetime() {
    let navigator = Navigator::new();
    navigator.push(Page::new("first", page()).into());
    let top = navigator.push(Page::new("top", page()).into());
    let top_lifetime = navigator.lifetime_of(top).expect("mounted");
    let (_top_sub, top_ended) = lifetime_counter(&top_lifetime);
    let popped = navigator.pop().expect("popped route");
    assert_eq!(popped.id, top);

    // The popped `Route` snapshot stays alive here, and the navigator goes
    // away below; neither keeps the already-ended lifetime alive or
    // re-triggers it.
    drop(navigator);
    assert_eq!(*top_ended.borrow(), 1);
    assert!(!top_lifetime.is_live());
    drop(popped);
}

#[test]
fn panicking_lifetime_callback_cannot_spare_siblings() {
    let key = |name: &str| PageKey::new(name).unwrap();
    let navigator = Navigator::new();
    navigator
        .set_pages([
            Page::new("a", page()).key(key("a")),
            Page::new("b", page()).key(key("b")),
            Page::new("c", page()).key(key("c")),
        ])
        .unwrap();
    let lifetimes: Vec<RouteLifetime> = navigator
        .routes()
        .iter()
        .map(|route| navigator.lifetime_of(route.id).expect("mounted"))
        .collect();
    let ended = Rc::new(RefCell::new(vec![0_usize; 3]));
    let mut subscriptions = Vec::new();
    for (index, lifetime) in lifetimes.iter().enumerate() {
        let ended = ended.clone();
        if index == 0 {
            // Retired order follows previous stack order, so this panic
            // belongs to the first removed route.
            subscriptions.push(lifetime.on_ended(move || {
                ended.borrow_mut()[index] += 1;
                panic!("sibling removal must not spare later lifetimes");
            }));
        } else {
            subscriptions.push(lifetime.on_ended(move || {
                ended.borrow_mut()[index] += 1;
            }));
        }
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        navigator
            .set_pages([Page::new("gone", page()).key(key("gone"))])
            .unwrap();
    }));
    assert!(
        result.is_err(),
        "the first panic resumes after delivery, not instead of it"
    );
    // Terminal marks committed before any delivery: every removed route is
    // terminal and every callback ran exactly once, despite the panic.
    assert!(lifetimes.iter().all(|lifetime| !lifetime.is_live()));
    assert_eq!(*ended.borrow(), [1, 1, 1]);
    drop(subscriptions);
}

#[test]
fn panicking_observer_does_not_veto_later_observers() {
    let navigator = Navigator::new();
    navigator.push_page(Page::new("a", page()));
    let id = navigator.routes()[0].id;
    let lifetime = navigator.lifetime_of(id).expect("mounted");
    let second_got = Rc::new(RefCell::new(0_usize));
    let second_got_for_callback = second_got.clone();
    let _first = navigator.observe(|_| panic!("observer veto attempt"));
    let _second = navigator.observe(move |_| {
        *second_got_for_callback.borrow_mut() += 1;
    });
    let result = catch_unwind(AssertUnwindSafe(|| {
        navigator.pop();
    }));
    assert!(result.is_err());
    // Both events (Popped, ActiveRouteChanged) still reach the later
    // observer, and the removed lifetime still ends.
    assert_eq!(*second_got.borrow(), 2);
    assert!(!lifetime.is_live());
}

#[test]
fn disposal_during_unwind_swallows_and_marks() {
    struct DropNavigator(Option<Navigator>);
    impl Drop for DropNavigator {
        fn drop(&mut self) {
            drop(self.0.take());
        }
    }
    let navigator = Navigator::new();
    navigator.push_page(Page::new("a", page()));
    let id = navigator.routes()[0].id;
    let lifetime = navigator.lifetime_of(id).expect("mounted");
    let _subscription = lifetime.on_ended(|| panic!("callback panic during unwind"));
    let guard = DropNavigator(Some(navigator));
    let result = catch_unwind(AssertUnwindSafe(move || {
        let _guard = guard;
        panic!("boom");
    }));
    let error = result.expect_err("the outer panic propagates");
    assert_eq!(
        error.downcast_ref::<&str>(),
        Some(&"boom"),
        "the disposal panic is swallowed, never replacing the unwind"
    );
    assert!(!lifetime.is_live());
}

#[test]
fn normal_disposal_resumes_callback_panic() {
    let navigator = Navigator::new();
    navigator.push_page(Page::new("a", page()));
    let id = navigator.routes()[0].id;
    let lifetime = navigator.lifetime_of(id).expect("mounted");
    let _subscription = lifetime.on_ended(|| panic!("disposal callback"));
    let result = catch_unwind(AssertUnwindSafe(move || {
        drop(navigator);
    }));
    let error = result.expect_err("disposal resumes the callback panic");
    assert_eq!(error.downcast_ref::<&str>(), Some(&"disposal callback"));
    assert!(!lifetime.is_live());
}

#[test]
fn late_lifetime_subscription_runs_immediately() {
    let navigator = Navigator::new();
    let id = navigator.push(Page::new("only", page()).into());
    let lifetime = navigator.lifetime_of(id).expect("mounted");
    navigator.pop();
    assert!(!lifetime.is_live());

    let ended = Rc::new(RefCell::new(0usize));
    let ended_for_callback = ended.clone();
    let _subscription = lifetime.on_ended(move || {
        *ended_for_callback.borrow_mut() += 1;
    });
    assert_eq!(*ended.borrow(), 1);
}
