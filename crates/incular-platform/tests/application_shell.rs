use incular_platform::{
    ApplicationBadge, ApplicationShellFeature, NotificationAction, NotificationPresentation,
    PlatformCapabilities, TaskbarDockState, TaskbarProgress, TrayItemId, TrayItemPresentation,
    WindowIcon, WindowId,
};

#[test]
fn shell_resource_ids_are_generational() {
    let first = TrayItemId::from_parts(2, 7);
    let replacement = TrayItemId::from_parts(2, 8);
    assert_eq!(first.index(), replacement.index());
    assert_ne!(first, replacement);
    assert_eq!(first.generation(), 7);
}

#[test]
fn shell_presentations_keep_owned_icon_pixels_and_actions() {
    let icon = WindowIcon::from_rgba(vec![255, 0, 0, 255], 1, 1).unwrap();
    let tray = TrayItemPresentation::new()
        .title("Incular")
        .tooltip("Running")
        .visible(false)
        .icon(icon.clone());
    assert_eq!(tray.icon.as_ref().unwrap().rgba(), &[255, 0, 0, 255]);
    assert!(!tray.visible);

    let notification = NotificationPresentation::new("Build", "Complete")
        .icon(icon)
        .action(NotificationAction::new("open", "Open"));
    assert_eq!(notification.actions[0].id.as_str(), "open");
}

#[test]
fn taskbar_progress_rejects_invalid_fractions_and_preserves_state() {
    assert!(TaskbarProgress::normal(f64::NAN).is_err());
    assert!(TaskbarProgress::paused(-0.1).is_err());
    assert!(TaskbarProgress::error(1.1).is_err());

    let state = TaskbarDockState::for_window(WindowId::from_parts(1, 3))
        .progress(TaskbarProgress::normal(0.42).unwrap())
        .badge(ApplicationBadge::Count(9));
    assert_eq!(state.progress.fraction(), Some(0.42));
    assert_eq!(state.badge, ApplicationBadge::Count(9));
}

#[test]
fn unsupported_host_reports_every_shell_facility_as_unsupported() {
    let capabilities = PlatformCapabilities::unsupported().application_services;
    assert!(!capabilities.tray_or_status_item.is_supported());
    assert!(!capabilities.notifications.is_supported());
    assert!(!capabilities.notification_actions.is_supported());
    assert!(!capabilities.notification_update.is_supported());
    assert!(!capabilities.notification_dismiss.is_supported());
    assert!(!capabilities.taskbar_progress.is_supported());
    assert!(!capabilities.application_badge.is_supported());
    assert!(!capabilities.taskbar_overlay_icon.is_supported());

    let feature = ApplicationShellFeature::TaskbarProgress;
    assert_eq!(format!("{feature:?}"), "TaskbarProgress");
}
