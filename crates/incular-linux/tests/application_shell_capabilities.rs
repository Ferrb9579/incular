#[cfg(target_os = "linux")]
use incular_platform::{CapabilitySupport, NativeWindowSystem};

#[cfg(target_os = "linux")]
fn assert_linux_shell_support(system: NativeWindowSystem) {
    let services = incular_linux::application_shell_capabilities(system);
    assert_eq!(services.tray_or_status_item, CapabilitySupport::Supported);
    assert_eq!(services.notifications, CapabilitySupport::Supported);
    assert_eq!(services.notification_actions, CapabilitySupport::Supported);
    assert_eq!(services.notification_update, CapabilitySupport::Supported);
    assert_eq!(services.notification_dismiss, CapabilitySupport::Supported);
    assert_eq!(services.taskbar_progress, CapabilitySupport::Unsupported);
    assert_eq!(services.application_badge, CapabilitySupport::Unsupported);
    assert_eq!(
        services.taskbar_overlay_icon,
        CapabilitySupport::Unsupported
    );
}

#[cfg(target_os = "linux")]
#[test]
fn linux_application_shell_capability_matrix_is_exact() {
    assert_linux_shell_support(NativeWindowSystem::X11);
    assert_linux_shell_support(NativeWindowSystem::Wayland);

    let services = incular_linux::application_shell_capabilities(NativeWindowSystem::Other);
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
