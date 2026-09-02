//! macOS desktop adapter for Incular's shared desktop shell.

pub use incular_desktop::{RunError, run_window};

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Default)]
struct MacosDesktopPlatformServices;

#[cfg(target_os = "macos")]
impl incular_desktop::DesktopPlatformServices for MacosDesktopPlatformServices {
    fn work_area_support(
        &self,
        system: incular_platform::NativeWindowSystem,
    ) -> incular_platform::CapabilitySupport {
        if system == incular_platform::NativeWindowSystem::AppKit {
            incular_platform::CapabilitySupport::Supported
        } else {
            incular_platform::CapabilitySupport::Unsupported
        }
    }

    fn work_area(
        &self,
        monitor: &winit::monitor::MonitorHandle,
    ) -> Option<incular_platform::PhysicalScreenRect> {
        use winit::platform::macos::MonitorHandleExtMacOS;

        let screen = monitor.ns_screen()?.cast::<objc2_app_kit::NSScreen>();
        // SAFETY: Winit returns a borrowed pointer to the live NSScreen for this
        // monitor. DesktopPlatformServices is queried from the native event-loop
        // thread, satisfying NSScreen's main-thread affinity, and the reference
        // does not outlive this synchronous query.
        let screen = unsafe { &*screen };
        let frame = screen.frame();
        let visible = screen.visibleFrame();
        let scale = monitor.scale_factor();
        if !scale.is_finite() || scale <= 0.0 {
            return None;
        }

        let frame_max_x = frame.origin.x + frame.size.width;
        let frame_max_y = frame.origin.y + frame.size.height;
        let visible_max_x = visible.origin.x + visible.size.width;
        let visible_max_y = visible.origin.y + visible.size.height;
        let left = ((visible.origin.x - frame.origin.x).max(0.0) * scale).round();
        let right = ((frame_max_x - visible_max_x).max(0.0) * scale).round();
        let bottom = ((visible.origin.y - frame.origin.y).max(0.0) * scale).round();
        let top = ((frame_max_y - visible_max_y).max(0.0) * scale).round();
        let to_i64 = |value: f64| {
            (value.is_finite() && value >= 0.0 && value <= i64::MAX as f64).then_some(value as i64)
        };
        let left = to_i64(left)?;
        let right = to_i64(right)?;
        let bottom = to_i64(bottom)?;
        let top = to_i64(top)?;

        let bounds_position = monitor.position();
        let bounds_size = monitor.size();
        let width = i64::from(bounds_size.width)
            .checked_sub(left)?
            .checked_sub(right)?;
        let height = i64::from(bounds_size.height)
            .checked_sub(top)?
            .checked_sub(bottom)?;
        Some(incular_platform::PhysicalScreenRect::new(
            i32::try_from(i64::from(bounds_position.x).checked_add(left)?).ok()?,
            i32::try_from(i64::from(bounds_position.y).checked_add(top)?).ok()?,
            u32::try_from(width).ok()?,
            u32::try_from(height).ok()?,
        ))
    }

    fn transient_support(
        &self,
        system: incular_platform::NativeWindowSystem,
        _role: incular_config::TransientRole,
    ) -> incular_platform::CapabilitySupport {
        if system == incular_platform::NativeWindowSystem::AppKit {
            incular_platform::CapabilitySupport::Supported
        } else {
            incular_platform::CapabilitySupport::Unsupported
        }
    }

    fn configure_transient_attributes(
        &self,
        _system: incular_platform::NativeWindowSystem,
        _parent: &winit::window::Window,
        _role: incular_config::TransientRole,
        attributes: winit::window::WindowAttributes,
    ) -> winit::window::WindowAttributes {
        attributes.with_active(false)
    }

    fn attach_transient(
        &self,
        system: incular_platform::NativeWindowSystem,
        parent: &winit::window::Window,
        popup: &winit::window::Window,
        _role: incular_config::TransientRole,
    ) -> incular_platform::PlatformOperationResult {
        if system != incular_platform::NativeWindowSystem::AppKit {
            return Ok(());
        }
        let Some(parent_window) = appkit_window(parent) else {
            return Err(incular_platform::PlatformOperationError::unavailable());
        };
        let Some(popup_window) = appkit_window(popup) else {
            return Err(incular_platform::PlatformOperationError::unavailable());
        };
        // SAFETY: both NSWindows are retained by their live Winit windows and
        // this call runs on AppKit's event-loop thread. The child relationship
        // is removed explicitly before the popup Winit window is dropped.
        unsafe {
            parent_window.addChildWindow_ordered(
                &popup_window,
                objc2_app_kit::NSWindowOrderingMode::NSWindowAbove,
            );
        }
        Ok(())
    }

    fn detach_transient(
        &self,
        system: incular_platform::NativeWindowSystem,
        parent: &winit::window::Window,
        popup: &winit::window::Window,
        _role: incular_config::TransientRole,
    ) {
        if system != incular_platform::NativeWindowSystem::AppKit {
            return;
        }
        if let (Some(parent_window), Some(popup_window)) =
            (appkit_window(parent), appkit_window(popup))
        {
            // SAFETY: same live-window/event-thread invariant as attachment.
            unsafe { parent_window.removeChildWindow(&popup_window) };
        }
    }
}

#[cfg(target_os = "macos")]
fn appkit_window(
    window: &winit::window::Window,
) -> Option<objc2::rc::Retained<objc2_app_kit::NSWindow>> {
    use raw_window_handle::RawWindowHandle;

    let RawWindowHandle::AppKit(handle) = incular_platform::raw_window_handles(window).window
    else {
        return None;
    };
    // SAFETY: raw-window-handle documents this as the live NSView owned by the
    // Winit window. We borrow it only long enough to ask AppKit for its retained
    // containing NSWindow.
    let view = unsafe { handle.ns_view.cast::<objc2_app_kit::NSView>().as_ref() };
    view.window()
}

pub fn run_application(application: incular_runtime::Application) -> Result<(), RunError> {
    #[cfg(target_os = "macos")]
    {
        incular_desktop::run_application_with_services(application, MacosDesktopPlatformServices)
    }
    #[cfg(not(target_os = "macos"))]
    incular_desktop::run_application(application)
}

#[cfg(feature = "devtools")]
pub use incular_desktop::{
    DevToolsLaunchMode, devtools_launch_mode_from, devtools_runner, devtools_ui_candidates,
};
