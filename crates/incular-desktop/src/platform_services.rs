use incular_config::TransientRole;
use incular_platform::{
    ApplicationActivation, ApplicationShellError, ApplicationShellFeature, CapabilitySupport,
    NativePointerSample, NativeWindowSystem, PhysicalScreenRect, PlatformCapabilities,
    PlatformLifecycle, PlatformOperationResult, SystemEnvironmentPreferences,
};
use incular_runtime::{
    NativeApplicationShellApplyResult, NativeApplicationShellCompletion,
    NativeApplicationShellEvent, NativeApplicationShellOperation, NativeApplicationShellRequest,
    TokioHandle,
};
use incular_widgets::{NoopPlatformMenuDelegate, PlatformMenuDelegate};
use std::{rc::Rc, sync::Arc};
use winit::{
    dpi::PhysicalPosition,
    monitor::MonitorHandle,
    window::{Window, WindowAttributes},
};

/// Narrow seam for desktop semantics Winit intentionally does not expose as a
/// portable contract.
///
/// The shared shell still owns window/event/GPU lifecycle. OS facade crates use
/// this trait only for information or native relationship semantics that would
/// otherwise require leaking HWND/NSWindow/X11 details into `incular-desktop`.
pub trait DesktopPlatformServices {
    /// Refines advanced-input capabilities owned by an OS facade rather than
    /// Winit itself (for example Win32 pen metadata captured from WM_POINTER).
    fn refine_advanced_input_capabilities(
        &self,
        _system: NativeWindowSystem,
        _capabilities: &mut PlatformCapabilities,
    ) {
    }

    /// Complete application-scoped OS preference snapshot. Unsupported fields
    /// remain `None`; the desktop environment provider resets those fields to
    /// stable defaults instead of carrying stale values forward.
    fn system_environment_preferences(&self) -> SystemEnvironmentPreferences {
        SystemEnvironmentPreferences::default()
    }

    /// Native foreground/application-active state when the OS exposes one.
    /// `None` means unsupported and must not be replaced by a focus heuristic.
    fn application_active(&self) -> Option<bool> {
        None
    }

    /// Starts event-driven observation for preferences whose OS API is not
    /// naturally surfaced by Winit. Implementations update their snapshot and
    /// invoke `wake` only after a semantic preference change.
    fn start_system_environment_watch(
        &self,
        _tokio: TokioHandle,
        _wake: Arc<dyn Fn() + Send + Sync>,
    ) {
    }

    /// Drains one coalesced native settings-change notification. A `true`
    /// result means the shell should acquire one new complete preference
    /// snapshot. The snapshot itself is deliberately separate so native
    /// callbacks never mutate Incular runtime state across an FFI stack.
    fn take_system_environment_change(&self) -> bool {
        false
    }

    /// Drains genuine application lifecycle/activity transitions captured by
    /// the OS facade. Per-window focus is intentionally not accepted here.
    fn take_application_lifecycle_events(&self) -> Vec<PlatformLifecycle> {
        Vec::new()
    }

    /// Installs delivery for native application activations which do not
    /// belong to any Winit window input stream (for example AppKit open-URL,
    /// open-file, and reopen callbacks). Implementations must invoke `deliver`
    /// only with normalized, native-handle-free values; the desktop shell then
    /// re-enters runtime state through its Winit user-event queue.
    fn start_application_activation_watch(
        &self,
        _deliver: Arc<dyn Fn(ApplicationActivation) + Send + Sync>,
    ) {
    }

    /// Returns the application-scoped tray, notification and taskbar backend.
    /// The default backend reports these operations as unsupported.
    fn application_shell(&self) -> &dyn DesktopApplicationShellServices {
        &DefaultDesktopPlatformServices
    }

    /// Win32 exposes several relevant preference/lifecycle notifications only
    /// on the application message queue. The facade may install a Winit message
    /// hook that records those events into its own thread-safe provider state.
    /// The shared shell still owns the event loop and never interprets Win32
    /// payloads itself.
    #[cfg(target_os = "windows")]
    fn windows_message_hook(
        &self,
    ) -> Option<Box<dyn FnMut(*const std::ffi::c_void) -> bool + 'static>> {
        None
    }

    /// Drains native metadata captured for one Winit pointer contact. This is
    /// intentionally a pull on the event-loop thread: native message hooks only
    /// record POD data and never mutate runtime/widget state across FFI.
    fn take_native_pointer_sample(&self, _pointer: u64) -> Option<NativePointerSample> {
        None
    }

    /// Whether this facade can pair Winit's file-transfer payload events with a
    /// trustworthy current client-space pointer position for hit testing.
    fn external_file_drag_support(&self, _system: NativeWindowSystem) -> CapabilitySupport {
        CapabilitySupport::Unsupported
    }

    /// Current client-space pointer position in physical pixels while a native
    /// external drag is active. Winit's file events do not carry coordinates,
    /// so supported facades must query their native window system directly.
    fn external_drag_position(
        &self,
        _system: NativeWindowSystem,
        _window: &Window,
    ) -> Option<PhysicalPosition<f64>> {
        None
    }

    /// Returns the application-scoped native menu delegate. The shared desktop
    /// shell captures this once for the runner lifetime and binds retained
    /// `PlatformMenuBar` owners to it as windows mount/rebuild.
    fn platform_menu_delegate(&self) -> Rc<dyn PlatformMenuDelegate> {
        Rc::new(NoopPlatformMenuDelegate)
    }

    /// Registers one native top-level window with the application-menu
    /// backend. Windows may need an HMENU per HWND; macOS ignores this because
    /// its main menu is application-global.
    fn register_platform_menu_window(
        &self,
        _system: NativeWindowSystem,
        _window: &Window,
    ) -> PlatformOperationResult {
        Ok(())
    }

    fn unregister_platform_menu_window(&self, _system: NativeWindowSystem, _window: &Window) {}

    /// Drains native menu events captured in OS callbacks. Implementations
    /// queue rather than execute Rust callbacks across an FFI stack frame.
    fn flush_platform_menu_events(&self) {}

    /// Whether dropping a Winit window only schedules native destruction and
    /// therefore requires observing `WindowEvent::Destroyed` before the event
    /// loop may terminate. This is a native lifecycle contract, not a timing
    /// heuristic: Win32 Winit posts an internal destroy message from `Drop`.
    fn wait_for_destroyed_event_after_window_drop(&self, _system: NativeWindowSystem) -> bool {
        false
    }

    fn work_area_support(&self, _system: NativeWindowSystem) -> CapabilitySupport {
        CapabilitySupport::Unsupported
    }

    fn work_area(&self, _monitor: &MonitorHandle) -> Option<PhysicalScreenRect> {
        None
    }

    fn transient_support(
        &self,
        system: NativeWindowSystem,
        _role: TransientRole,
    ) -> CapabilitySupport {
        #[cfg(target_os = "linux")]
        if system == NativeWindowSystem::X11 {
            return CapabilitySupport::Supported;
        }
        let _ = system;
        CapabilitySupport::Unsupported
    }

    fn configure_transient_attributes(
        &self,
        system: NativeWindowSystem,
        _parent: &Window,
        role: TransientRole,
        attributes: WindowAttributes,
    ) -> WindowAttributes {
        #[cfg(target_os = "linux")]
        if system == NativeWindowSystem::X11 {
            use winit::platform::x11::{WindowAttributesExtX11, WindowType};

            let window_type = match role {
                TransientRole::Menu => WindowType::DropdownMenu,
                TransientRole::ContextMenu | TransientRole::Popover => WindowType::PopupMenu,
                TransientRole::ComboBox => WindowType::Combo,
                TransientRole::Tooltip => WindowType::Tooltip,
            };
            return attributes
                .with_override_redirect(true)
                .with_x11_window_type(vec![window_type]);
        }
        let _ = (system, role);
        attributes
    }

    fn attach_transient(
        &self,
        _system: NativeWindowSystem,
        _parent: &Window,
        _popup: &Window,
        _role: TransientRole,
    ) -> PlatformOperationResult {
        Ok(())
    }

    /// Makes an already-created transient visible using the platform's
    /// non-activating popup semantics when available. The shared desktop shell
    /// deliberately creates hosts hidden so native relationship/style setup is
    /// complete before the first visible frame.
    fn show_transient(
        &self,
        _system: NativeWindowSystem,
        popup: &Window,
        _role: TransientRole,
    ) -> PlatformOperationResult {
        popup.set_visible(true);
        Ok(())
    }

    fn detach_transient(
        &self,
        _system: NativeWindowSystem,
        _parent: &Window,
        _popup: &Window,
        _role: TransientRole,
    ) {
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultDesktopPlatformServices;

impl DesktopPlatformServices for DefaultDesktopPlatformServices {}

/// Application-scoped native resources, independent of window/input services.
/// Methods run on the desktop event-loop thread. Implementations own native
/// resource teardown and deliver normalized callbacks through the supplied sinks.
pub trait DesktopApplicationShellServices {
    /// Publishes application-shell capabilities owned by the OS facade.
    ///
    /// The shared desktop runner only coordinates requests. Native tray/status
    /// items, notifications, and taskbar/Dock resources stay in the platform
    /// crate so their lifetime and thread-affinity rules do not leak into
    /// `incular-desktop`.
    fn refine_application_shell_capabilities(
        &self,
        _system: NativeWindowSystem,
        capabilities: &mut PlatformCapabilities,
    ) {
        let unsupported = CapabilitySupport::Unsupported;
        let services = &mut capabilities.application_services;
        services.tray_or_status_item = unsupported;
        services.notifications = unsupported;
        services.notification_actions = unsupported;
        services.notification_update = unsupported;
        services.notification_dismiss = unsupported;
        services.taskbar_progress = unsupported;
        services.application_badge = unsupported;
        services.taskbar_overlay_icon = unsupported;
    }

    /// Installs the normalized native-event delivery path for application-shell
    /// resources. OS callbacks must never call runtime-owned callbacks directly;
    /// they emit stable IDs here and the desktop event loop performs dispatch.
    fn start_application_shell_watch(
        &self,
        _deliver: Arc<dyn Fn(NativeApplicationShellEvent) + Send + Sync>,
        _complete: Arc<dyn Fn(NativeApplicationShellCompletion) + Send + Sync>,
    ) {
    }

    /// Supplies any installed application identity required by the platform's
    /// notification system (for example a Windows AppUserModelID).
    fn set_application_shell_notification_identity(&self, _identity: Option<String>) {}

    /// Applies one application-shell request on the native event-loop thread.
    ///
    /// OS implementations own all native resources created by this operation.
    /// The default implementation is deliberately strict so a bare
    /// `incular-desktop` runner cannot silently fake a platform integration.
    fn apply_application_shell_request(
        &self,
        _system: NativeWindowSystem,
        request: NativeApplicationShellRequest,
        _target_window: Option<&Window>,
    ) -> NativeApplicationShellApplyResult {
        let feature = match request.operation {
            NativeApplicationShellOperation::CreateTray { .. }
            | NativeApplicationShellOperation::UpdateTray { .. }
            | NativeApplicationShellOperation::RemoveTray { .. } => {
                ApplicationShellFeature::TrayOrStatusItem
            }
            NativeApplicationShellOperation::ShowNotification { .. } => {
                ApplicationShellFeature::Notifications
            }
            NativeApplicationShellOperation::UpdateNotification { .. } => {
                ApplicationShellFeature::NotificationUpdate
            }
            NativeApplicationShellOperation::CloseNotification { .. } => {
                ApplicationShellFeature::NotificationDismiss
            }
            NativeApplicationShellOperation::SetTaskbarDockState(state) => {
                if state.overlay_icon.is_some() {
                    ApplicationShellFeature::TaskbarOverlayIcon
                } else if state.badge != incular_platform::ApplicationBadge::None {
                    ApplicationShellFeature::ApplicationBadge
                } else {
                    ApplicationShellFeature::TaskbarProgress
                }
            }
        };
        NativeApplicationShellApplyResult::Completed(Err(ApplicationShellError::Unsupported(
            feature,
        )))
    }
}

impl DesktopApplicationShellServices for DefaultDesktopPlatformServices {}
