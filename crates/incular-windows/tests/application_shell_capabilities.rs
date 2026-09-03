#[cfg(target_os = "windows")]
use incular_platform::{CapabilitySupport, NativeWindowSystem};

#[cfg(target_os = "windows")]
#[test]
fn windows_application_shell_capability_matrix_is_exact() {
    let services = incular_windows::application_shell_capabilities(NativeWindowSystem::Win32);
    assert_eq!(services.tray_or_status_item, CapabilitySupport::Supported);
    assert_eq!(services.notifications, CapabilitySupport::Supported);
    assert_eq!(services.notification_actions, CapabilitySupport::Supported);
    assert_eq!(services.notification_update, CapabilitySupport::Unsupported);
    assert_eq!(
        services.notification_dismiss,
        CapabilitySupport::Unsupported
    );
    assert_eq!(services.taskbar_progress, CapabilitySupport::Supported);
    assert_eq!(services.application_badge, CapabilitySupport::Unsupported);
    assert_eq!(services.taskbar_overlay_icon, CapabilitySupport::Supported);

    let services = incular_windows::application_shell_capabilities(NativeWindowSystem::Other);
    assert_eq!(services.tray_or_status_item, CapabilitySupport::Unsupported);
    assert_eq!(services.notifications, CapabilitySupport::Unsupported);
    assert_eq!(
        services.notification_actions,
        CapabilitySupport::Unsupported
    );
    assert_eq!(services.notification_update, CapabilitySupport::Unsupported);
    assert_eq!(
        services.notification_dismiss,
        CapabilitySupport::Unsupported
    );
    assert_eq!(services.taskbar_progress, CapabilitySupport::Unsupported);
    assert_eq!(services.application_badge, CapabilitySupport::Unsupported);
    assert_eq!(
        services.taskbar_overlay_icon,
        CapabilitySupport::Unsupported
    );
}
