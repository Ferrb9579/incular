use incular_platform::{
    ApplicationBadge, ApplicationShellError, ApplicationShellFeature, NotificationAction,
    NotificationActionId, NotificationPresentation, PlatformCapabilities, TaskbarDockState,
    TaskbarProgress, TrayItemPresentation,
};
use incular_runtime::{
    ApplicationShellService, MemoryApplicationShellAdapter, NativeApplicationShellEvent,
    NativeApplicationShellOperation,
};
use incular_widgets::{MenuItemId, PlatformMenu, PlatformMenuEvent, PlatformMenuItem};
use std::sync::{Arc, Mutex, RwLock};

fn service() -> ApplicationShellService {
    ApplicationShellService::new_for_test(Arc::new(RwLock::new(PlatformCapabilities::default())))
}

#[test]
fn unsupported_shell_operations_fail_before_native_queueing() {
    let shell = ApplicationShellService::new_for_test(Arc::new(RwLock::new(
        PlatformCapabilities::unsupported(),
    )));
    assert_eq!(
        shell
            .create_tray_item(TrayItemPresentation::new(), &[])
            .err(),
        Some(ApplicationShellError::Unsupported(
            ApplicationShellFeature::TrayOrStatusItem
        ))
    );
    assert_eq!(
        shell
            .show_notification(NotificationPresentation::new("Build", "Done"))
            .err(),
        Some(ApplicationShellError::Unsupported(
            ApplicationShellFeature::Notifications
        ))
    );
    assert_eq!(
        shell
            .set_taskbar_dock_state(
                TaskbarDockState::default().progress(TaskbarProgress::normal(0.5).unwrap())
            )
            .unwrap_err(),
        ApplicationShellError::Unsupported(ApplicationShellFeature::TaskbarProgress)
    );
    assert!(shell.take_native_requests_for_test().is_empty());
}

#[test]
fn tray_lifecycle_reuses_slots_with_new_generation_and_rejects_stale_events() {
    let shell = service();
    let selected = Arc::new(Mutex::new(Vec::new()));
    let selected_copy = selected.clone();
    let menu = PlatformMenu::new(
        "App",
        vec![
            PlatformMenuItem::new("Open")
                .id("open")
                .on_selected(move || selected_copy.lock().unwrap().push("open"))
                .into(),
        ],
    );
    let first = shell
        .create_tray_item(
            TrayItemPresentation::new().tooltip("first"),
            std::slice::from_ref(&menu),
        )
        .unwrap();
    let stale_id = first.id();
    first.remove().unwrap();
    let second = shell
        .create_tray_item(TrayItemPresentation::new().tooltip("second"), &[menu])
        .unwrap();
    let activated = Arc::new(Mutex::new(0_u32));
    let activated_copy = activated.clone();
    second
        .on_activated(move || *activated_copy.lock().unwrap() += 1)
        .unwrap();
    assert_eq!(stale_id.index(), second.id().index());
    assert_ne!(stale_id.generation(), second.id().generation());

    assert!(
        !shell.handle_native_event_for_test(NativeApplicationShellEvent::TrayMenu {
            id: stale_id,
            event: PlatformMenuEvent::Selected(MenuItemId::new("open")),
        })
    );
    assert!(
        shell.handle_native_event_for_test(NativeApplicationShellEvent::TrayMenu {
            id: second.id(),
            event: PlatformMenuEvent::Selected(MenuItemId::new("open")),
        })
    );
    assert_eq!(*selected.lock().unwrap(), vec!["open"]);
    assert!(
        shell.handle_native_event_for_test(NativeApplicationShellEvent::TrayActivated {
            id: second.id(),
        })
    );
    assert_eq!(*activated.lock().unwrap(), 1);
}

#[test]
fn notification_callbacks_dispatch_only_for_live_known_actions() {
    let shell = service();
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let notification = shell
        .show_notification(
            NotificationPresentation::new("Build", "Done")
                .action(NotificationAction::new("open", "Open")),
        )
        .unwrap();
    let id = notification.id();
    let events_copy = events.clone();
    notification
        .on_action(move |action| events_copy.lock().unwrap().push(action.as_str().to_owned()))
        .unwrap();
    notification
        .update(
            NotificationPresentation::new("Build", "Updated")
                .action(NotificationAction::new("open", "Open")),
        )
        .unwrap();
    assert!(
        !shell.handle_native_event_for_test(NativeApplicationShellEvent::NotificationAction {
            id,
            action: NotificationActionId::new("unknown"),
        })
    );
    assert!(
        shell.handle_native_event_for_test(NativeApplicationShellEvent::NotificationAction {
            id,
            action: NotificationActionId::new("open"),
        })
    );
    assert_eq!(*events.lock().unwrap(), vec!["open"]);
    assert!(
        shell
            .take_native_requests_for_test()
            .iter()
            .any(|request| matches!(
                request.operation,
                NativeApplicationShellOperation::UpdateNotification { .. }
            ))
    );
}

#[test]
fn dismissed_notification_releases_identity_and_rejects_stale_callbacks() {
    let shell = service();
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let notification = shell
        .show_notification(NotificationPresentation::new("Build", "Done"))
        .unwrap();
    let stale_id = notification.id();
    let activated_events = events.clone();
    notification
        .on_activated(move || activated_events.lock().unwrap().push("activated".into()))
        .unwrap();
    let dismissed_events = events.clone();
    notification
        .on_dismissed(move || dismissed_events.lock().unwrap().push("dismissed".into()))
        .unwrap();

    assert!(shell.handle_native_event_for_test(
        NativeApplicationShellEvent::NotificationActivated { id: stale_id }
    ));
    assert!(shell.handle_native_event_for_test(
        NativeApplicationShellEvent::NotificationDismissed { id: stale_id }
    ));
    assert!(!shell.handle_native_event_for_test(
        NativeApplicationShellEvent::NotificationActivated { id: stale_id }
    ));

    let replacement = shell
        .show_notification(NotificationPresentation::new("Build", "Again"))
        .unwrap();
    assert_eq!(stale_id.index(), replacement.id().index());
    assert_ne!(stale_id.generation(), replacement.id().generation());
    assert_eq!(*events.lock().unwrap(), vec!["activated", "dismissed"]);
}

#[test]
fn dropping_notification_releases_identity_and_requests_native_cleanup() {
    let shell = service();
    let notification = shell
        .show_notification(NotificationPresentation::new("Build", "Done"))
        .unwrap();
    let stale_id = notification.id();
    let initial = shell.take_native_requests_for_test();
    assert!(matches!(
        initial.as_slice(),
        [request]
            if matches!(
                request.operation,
                NativeApplicationShellOperation::ShowNotification { id, .. } if id == stale_id
            )
    ));

    drop(notification);
    let cleanup = shell.take_native_requests_for_test();
    assert!(matches!(
        cleanup.as_slice(),
        [request]
            if matches!(
                request.operation,
                NativeApplicationShellOperation::CloseNotification { id } if id == stale_id
            )
    ));
    assert!(!shell.handle_native_event_for_test(
        NativeApplicationShellEvent::NotificationActivated { id: stale_id }
    ));

    let replacement = shell
        .show_notification(NotificationPresentation::new("Build", "Again"))
        .unwrap();
    assert_eq!(replacement.id().index(), stale_id.index());
    assert_ne!(replacement.id().generation(), stale_id.generation());
}

#[test]
fn native_dismissal_releases_identity_and_queues_backend_cleanup() {
    let shell = service();
    let notification = shell
        .show_notification(NotificationPresentation::new("Build", "Done"))
        .unwrap();
    let id = notification.id();
    shell.take_native_requests_for_test();

    assert!(
        shell.handle_native_event_for_test(NativeApplicationShellEvent::NotificationDismissed {
            id
        })
    );
    let cleanup = shell.take_native_requests_for_test();
    assert!(matches!(
        cleanup.as_slice(),
        [request]
            if matches!(
                request.operation,
                NativeApplicationShellOperation::CloseNotification { id: cleaned } if cleaned == id
            )
    ));
    assert!(
        !shell.handle_native_event_for_test(NativeApplicationShellEvent::NotificationDismissed {
            id
        })
    );
}

#[test]
fn taskbar_progress_transitions_are_queued_in_order() {
    let shell = service();
    let states = [
        TaskbarProgress::None,
        TaskbarProgress::Indeterminate,
        TaskbarProgress::normal(0.25).unwrap(),
        TaskbarProgress::paused(0.5).unwrap(),
        TaskbarProgress::error(0.75).unwrap(),
        TaskbarProgress::None,
    ];
    for progress in states {
        shell
            .set_taskbar_dock_state(TaskbarDockState::default().progress(progress))
            .unwrap();
    }
    let queued: Vec<_> = shell
        .take_native_requests_for_test()
        .into_iter()
        .map(|request| match request.operation {
            NativeApplicationShellOperation::SetTaskbarDockState(state) => state.progress,
            other => panic!("unexpected shell operation: {other:?}"),
        })
        .collect();
    assert_eq!(queued, states);
}

#[test]
fn memory_adapter_records_shell_operations_in_order() {
    let shell = service();
    let adapter = MemoryApplicationShellAdapter::default();
    let tray = shell
        .create_tray_item(TrayItemPresentation::new(), &[])
        .unwrap();
    shell
        .set_taskbar_dock_state(
            TaskbarDockState::default()
                .progress(TaskbarProgress::normal(0.5).unwrap())
                .badge(ApplicationBadge::Count(3)),
        )
        .unwrap();
    drop(tray);
    for request in shell.take_native_requests_for_test() {
        shell.complete_for_test(adapter.apply(request));
    }
    let operations = adapter.operations();
    assert!(matches!(
        operations[0],
        NativeApplicationShellOperation::CreateTray { .. }
    ));
    assert!(matches!(
        operations[1],
        NativeApplicationShellOperation::SetTaskbarDockState(_)
    ));
    assert!(matches!(
        operations[2],
        NativeApplicationShellOperation::RemoveTray { .. }
    ));
    assert_eq!(shell.take_completions().len(), 3);
}
