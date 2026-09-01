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
