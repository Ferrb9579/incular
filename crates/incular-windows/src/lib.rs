//! Windows desktop adapter for Incular's shared desktop shell.
mod crash_reporter;
#[cfg(target_os = "windows")]
mod environment;
#[cfg(target_os = "windows")]
mod platform_menus;

pub use incular_desktop::RunError;

#[cfg(target_os = "windows")]
#[derive(Clone, Default)]
struct WindowsDesktopPlatformServices {
    menus: std::rc::Rc<platform_menus::WindowsPlatformMenuDelegate>,
    environment: environment::WindowsEnvironmentState,
}

#[cfg(target_os = "windows")]
impl incular_desktop::DesktopPlatformServices for WindowsDesktopPlatformServices {
    fn system_environment_preferences(&self) -> incular_platform::SystemEnvironmentPreferences {
        self.environment.preferences()
    }

    fn application_active(&self) -> Option<bool> {
        Some(self.environment.application_active())
    }

    fn take_system_environment_change(&self) -> bool {
        self.environment.take_settings_change()
    }

    fn take_application_lifecycle_events(&self) -> Vec<incular_platform::PlatformLifecycle> {
        self.environment.take_lifecycle_events()
    }

    fn start_system_environment_watch(
        &self,
        _tokio: incular_runtime::TokioHandle,
        wake: std::sync::Arc<dyn Fn() + Send + Sync>,
    ) {
        self.environment.start_watch(wake);
    }

    fn windows_message_hook(
        &self,
    ) -> Option<Box<dyn FnMut(*const std::ffi::c_void) -> bool + 'static>> {
        Some(self.environment.message_hook())
    }

    fn external_file_drag_support(
        &self,
        system: incular_platform::NativeWindowSystem,
    ) -> incular_platform::CapabilitySupport {
        if system == incular_platform::NativeWindowSystem::Win32 {
            incular_platform::CapabilitySupport::Supported
        } else {
            incular_platform::CapabilitySupport::Unsupported
        }
    }

    fn external_drag_position(
        &self,
        system: incular_platform::NativeWindowSystem,
        window: &winit::window::Window,
    ) -> Option<winit::dpi::PhysicalPosition<f64>> {
        if system != incular_platform::NativeWindowSystem::Win32 {
            return None;
        }
        use raw_window_handle::RawWindowHandle;
        use windows_sys::Win32::{
            Foundation::POINT, Graphics::Gdi::ScreenToClient, UI::WindowsAndMessaging::GetCursorPos,
        };

        let RawWindowHandle::Win32(handle) = incular_platform::raw_window_handles(window).window
        else {
            return None;
        };
        let mut point = POINT { x: 0, y: 0 };
        // SAFETY: `point` is writable and `handle.hwnd` is the live Winit HWND.
        if unsafe { GetCursorPos(&mut point) } == 0
            || unsafe { ScreenToClient(handle.hwnd.get(), &mut point) } == 0
        {
            return None;
        }
        Some(winit::dpi::PhysicalPosition::new(
            f64::from(point.x),
            f64::from(point.y),
        ))
    }

    fn platform_menu_delegate(&self) -> std::rc::Rc<dyn incular_widgets::PlatformMenuDelegate> {
        self.menus.clone()
    }

    fn register_platform_menu_window(
        &self,
        system: incular_platform::NativeWindowSystem,
        window: &winit::window::Window,
    ) -> incular_platform::PlatformOperationResult {
        if system != incular_platform::NativeWindowSystem::Win32 {
            return Ok(());
        }
        self.menus.register_window(window)
    }

    fn unregister_platform_menu_window(
        &self,
        system: incular_platform::NativeWindowSystem,
        window: &winit::window::Window,
    ) {
        if system == incular_platform::NativeWindowSystem::Win32 {
            self.menus.unregister_window(window);
        }
    }

    fn flush_platform_menu_events(&self) {
        self.menus.flush_events();
    }

    fn wait_for_destroyed_event_after_window_drop(
        &self,
        system: incular_platform::NativeWindowSystem,
    ) -> bool {
        system == incular_platform::NativeWindowSystem::Win32
    }

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

    fn transient_support(
        &self,
        system: incular_platform::NativeWindowSystem,
        _role: incular_config::TransientRole,
    ) -> incular_platform::CapabilitySupport {
        if system == incular_platform::NativeWindowSystem::Win32 {
            incular_platform::CapabilitySupport::Supported
        } else {
            incular_platform::CapabilitySupport::Unsupported
        }
    }

    fn configure_transient_attributes(
        &self,
        system: incular_platform::NativeWindowSystem,
        parent: &winit::window::Window,
        _role: incular_config::TransientRole,
        attributes: winit::window::WindowAttributes,
    ) -> winit::window::WindowAttributes {
        if system != incular_platform::NativeWindowSystem::Win32 {
            return attributes;
        }
        use raw_window_handle::RawWindowHandle;
        use winit::platform::windows::WindowAttributesExtWindows;

        let RawWindowHandle::Win32(handle) = incular_platform::raw_window_handles(parent).window
        else {
            return attributes;
        };
        attributes
            .with_owner_window(handle.hwnd.get())
            .with_skip_taskbar(true)
            .with_active(false)
    }

    fn attach_transient(
        &self,
        system: incular_platform::NativeWindowSystem,
        parent: &winit::window::Window,
        popup: &winit::window::Window,
        _role: incular_config::TransientRole,
    ) -> incular_platform::PlatformOperationResult {
        if system != incular_platform::NativeWindowSystem::Win32 {
            return Ok(());
        }
        use raw_window_handle::RawWindowHandle;
        use windows_sys::Win32::UI::WindowsAndMessaging::{GW_OWNER, GetWindow};

        let RawWindowHandle::Win32(parent_handle) =
            incular_platform::raw_window_handles(parent).window
        else {
            return Err(incular_platform::PlatformOperationError::unavailable());
        };
        let RawWindowHandle::Win32(popup_handle) =
            incular_platform::raw_window_handles(popup).window
        else {
            return Err(incular_platform::PlatformOperationError::unavailable());
        };
        // SAFETY: both HWNDs are live Winit windows on this event-loop thread.
        // Querying GW_OWNER does not retain or mutate either handle.
        if unsafe { GetWindow(popup_handle.hwnd.get(), GW_OWNER) } != parent_handle.hwnd.get() {
            return Err(incular_platform::PlatformOperationError::with_context(
                incular_platform::PlatformOperationErrorKind::NativeFailure,
                "Win32 transient window was not created with the requested owner",
            ));
        }
        Ok(())
    }

    fn show_transient(
        &self,
        system: incular_platform::NativeWindowSystem,
        popup: &winit::window::Window,
        _role: incular_config::TransientRole,
    ) -> incular_platform::PlatformOperationResult {
        if system != incular_platform::NativeWindowSystem::Win32 {
            popup.set_visible(true);
            return Ok(());
        }
        use raw_window_handle::RawWindowHandle;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, SW_SHOWNOACTIVATE, SWP_FRAMECHANGED, SWP_NOACTIVATE,
            SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, ShowWindow,
            WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        };

        let RawWindowHandle::Win32(handle) = incular_platform::raw_window_handles(popup).window
        else {
            return Err(incular_platform::PlatformOperationError::unavailable());
        };
        let hwnd = handle.hwnd.get();
        // SAFETY: the HWND is owned by this live Winit window and every call is
        // synchronous on the window's event-loop thread. Showing first with
        // SW_SHOWNOACTIVATE avoids an activation edge; applying the persistent
        // NOACTIVATE/TOOLWINDOW styles afterwards prevents later mouse
        // activation and task-switcher participation. The desktop shell never
        // calls Winit's visibility mutator for this host after this point.
        unsafe {
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let required = isize::try_from(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW)
                .expect("Win32 extended style flags fit isize");
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, current | required);
            let flags = SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER;
            if SetWindowPos(hwnd, 0, 0, 0, 0, 0, flags) == 0 {
                return Err(incular_platform::PlatformOperationError::with_context(
                    incular_platform::PlatformOperationErrorKind::NativeFailure,
                    "Win32 could not apply transient extended-window styles",
                ));
            }
            let applied = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            if applied & required != required {
                return Err(incular_platform::PlatformOperationError::with_context(
                    incular_platform::PlatformOperationErrorKind::NativeFailure,
                    "Win32 transient window is missing WS_EX_NOACTIVATE/WS_EX_TOOLWINDOW after native show",
                ));
            }
        }
        Ok(())
    }
}

pub fn run_application(application: incular_runtime::Application) -> Result<(), RunError> {
    let _crash_handler = crash_reporter::install();
    #[cfg(target_os = "windows")]
    {
        incular_desktop::run_application_with_services(
            application,
            WindowsDesktopPlatformServices::default(),
        )
    }
    #[cfg(not(target_os = "windows"))]
    incular_desktop::run_application(application)
}

pub fn run_window(
    runtime: incular_runtime::Runtime,
    on_action: impl FnMut(incular_widgets::internal::ActionId) + 'static,
) -> Result<(), RunError> {
    let _crash_handler = crash_reporter::install();
    #[cfg(target_os = "windows")]
    {
        incular_desktop::run_window_with_services(
            runtime,
            on_action,
            WindowsDesktopPlatformServices::default(),
        )
    }
    #[cfg(not(target_os = "windows"))]
    incular_desktop::run_window(runtime, on_action)
}

#[cfg(feature = "devtools")]
pub use incular_desktop::{
    DevToolsLaunchMode, devtools_launch_mode_from, devtools_runner, devtools_ui_candidates,
};
