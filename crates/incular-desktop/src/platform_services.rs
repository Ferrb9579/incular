use incular_config::TransientRole;
use incular_platform::{
    CapabilitySupport, NativeWindowSystem, PhysicalScreenRect, PlatformOperationResult,
};
use incular_widgets::{NoopPlatformMenuDelegate, PlatformMenuDelegate};
use std::rc::Rc;
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
