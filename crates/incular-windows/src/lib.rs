//! Windows desktop adapter for Incular's shared desktop shell.
mod crash_reporter;

pub use incular_desktop::RunError;

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, Default)]
struct WindowsDesktopPlatformServices;

#[cfg(target_os = "windows")]
impl incular_desktop::DesktopPlatformServices for WindowsDesktopPlatformServices {
    fn work_area_support(
        &self,
        system: incular_platform::NativeWindowSystem,
    ) -> incular_platform::CapabilitySupport {
        if system == incular_platform::NativeWindowSystem::Win32 {
            incular_platform::CapabilitySupport::Supported
        } else {
            incular_platform::CapabilitySupport::Unsupported
        }
    }

    fn work_area(
        &self,
        monitor: &winit::monitor::MonitorHandle,
    ) -> Option<incular_platform::PhysicalScreenRect> {
        use windows_sys::Win32::{
            Foundation::RECT,
            Graphics::Gdi::{GetMonitorInfoW, MONITORINFO},
        };
        use winit::platform::windows::MonitorHandleExtWindows;

        let zero = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let mut info = MONITORINFO {
            cbSize: u32::try_from(std::mem::size_of::<MONITORINFO>()).ok()?,
            rcMonitor: zero,
            rcWork: zero,
            dwFlags: 0,
        };
        // SAFETY: `monitor.hmonitor()` is a live HMONITOR borrowed from Winit
        // and `info` is a correctly sized writable MONITORINFO for the duration
        // of the synchronous Win32 call.
        if unsafe { GetMonitorInfoW(monitor.hmonitor(), &mut info) } == 0 {
            return None;
        }
        let width = u32::try_from(info.rcWork.right.checked_sub(info.rcWork.left)?).ok()?;
        let height = u32::try_from(info.rcWork.bottom.checked_sub(info.rcWork.top)?).ok()?;
        Some(incular_platform::PhysicalScreenRect::new(
            info.rcWork.left,
            info.rcWork.top,
            width,
            height,
        ))
    }
}

pub fn run_application(application: incular_runtime::Application) -> Result<(), RunError> {
    let _crash_handler = crash_reporter::install();
    #[cfg(target_os = "windows")]
    {
        incular_desktop::run_application_with_services(application, WindowsDesktopPlatformServices)
    }
    #[cfg(not(target_os = "windows"))]
    incular_desktop::run_application(application)
}

pub fn run_window(
    runtime: incular_runtime::Runtime,
    on_action: impl FnMut(incular_widgets::internal::ActionId) + 'static,
) -> Result<(), RunError> {
    let _crash_handler = crash_reporter::install();
    incular_desktop::run_window(runtime, on_action)
}

#[cfg(feature = "devtools")]
pub use incular_desktop::{
    DevToolsLaunchMode, devtools_launch_mode_from, devtools_runner, devtools_ui_candidates,
};
