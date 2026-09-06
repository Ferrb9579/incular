//! Windows desktop adapter for Incular's shared desktop shell.
#[cfg(target_os = "windows")]
mod application_shell;
mod crash_reporter;
#[cfg(target_os = "windows")]
mod environment;
#[cfg(target_os = "windows")]
mod platform_menus;
#[cfg(target_os = "windows")]
mod pointer;

pub use incular_desktop::RunError;

/// Returns the static application-shell capability matrix advertised by the
/// Windows adapter for a concrete native window system.
#[doc(hidden)]
#[must_use]
#[cfg(target_os = "windows")]
pub fn application_shell_capabilities(
    system: incular_platform::NativeWindowSystem,
) -> incular_platform::ApplicationServiceCapabilities {
    let shell = application_shell::WindowsApplicationShell::default();
    let mut capabilities = incular_platform::PlatformCapabilities::default();
    shell.refine_capabilities(system, &mut capabilities);
    capabilities.application_services
}

/// Returns the advanced-input capability matrix after the Windows facade has
/// refined the shared desktop capabilities.
#[doc(hidden)]
#[must_use]
#[cfg(target_os = "windows")]
pub fn advanced_input_capabilities(
    system: incular_platform::NativeWindowSystem,
) -> incular_platform::AdvancedInputCapabilities {
    use incular_desktop::DesktopPlatformServices as _;

    let services = WindowsDesktopPlatformServices::default();
    let mut capabilities = incular_platform::PlatformCapabilities::unsupported();
    capabilities.advanced_input = incular_desktop::desktop_advanced_input_capabilities(system);
    services.refine_advanced_input_capabilities(system, &mut capabilities);
    capabilities.advanced_input
}

#[cfg(target_os = "windows")]
#[derive(Clone, Default)]
struct WindowsDesktopPlatformServices {
    application_shell: application_shell::WindowsApplicationShell,
    menus: std::rc::Rc<platform_menus::WindowsPlatformMenuDelegate>,
    environment: environment::WindowsEnvironmentState,
    pointer: pointer::WindowsPointerState,
}

#[cfg(target_os = "windows")]
impl incular_desktop::DesktopPlatformServices for WindowsDesktopPlatformServices {
    fn application_shell(&self) -> &dyn incular_desktop::DesktopApplicationShellServices {
        self
    }
    fn refine_advanced_input_capabilities(
        &self,
        system: incular_platform::NativeWindowSystem,
        capabilities: &mut incular_platform::PlatformCapabilities,
    ) {
        capabilities.advanced_input.stylus =
            if system == incular_platform::NativeWindowSystem::Win32 {
                incular_platform::CapabilitySupport::Supported
            } else {
                incular_platform::CapabilitySupport::Unsupported
            };
    }

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
        let mut environment = self.environment.message_hook();
        let mut pointer = self.pointer.message_hook();
        Some(Box::new(move |message| {
            // Both hooks are observation-only. Evaluate both even if one ever
            // grows a consuming case so input/settings capture cannot starve.
            let pointer_handled = pointer(message);
            let environment_handled = environment(message);
            pointer_handled || environment_handled
        }))
    }

    fn take_native_pointer_sample(
        &self,
        pointer: u64,
    ) -> Option<incular_platform::NativePointerSample> {
        self.pointer.take(pointer)
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

        let RawWindowHandle::Win32(handle) =
            incular_desktop::winit_adapter::raw_window_handles(window).window
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

        let RawWindowHandle::Win32(handle) =
            incular_desktop::winit_adapter::raw_window_handles(parent).window
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
            incular_desktop::winit_adapter::raw_window_handles(parent).window
        else {
            return Err(incular_platform::PlatformOperationError::unavailable());
        };
        let RawWindowHandle::Win32(popup_handle) =
            incular_desktop::winit_adapter::raw_window_handles(popup).window
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

        let RawWindowHandle::Win32(handle) =
            incular_desktop::winit_adapter::raw_window_handles(popup).window
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

#[cfg(target_os = "windows")]
impl incular_desktop::DesktopApplicationShellServices for WindowsDesktopPlatformServices {
    fn refine_application_shell_capabilities(
        &self,
        system: incular_platform::NativeWindowSystem,
        capabilities: &mut incular_platform::PlatformCapabilities,
    ) {
        self.application_shell
            .refine_capabilities(system, capabilities);
    }
    fn start_application_shell_watch(
        &self,
        deliver: std::sync::Arc<dyn Fn(incular_runtime::NativeApplicationShellEvent) + Send + Sync>,
        _complete: std::sync::Arc<
            dyn Fn(incular_runtime::NativeApplicationShellCompletion) + Send + Sync,
        >,
    ) {
        self.application_shell.start_watch(deliver);
    }
    fn set_application_shell_notification_identity(&self, identity: Option<String>) {
        self.application_shell.set_notification_identity(identity);
    }
    fn apply_application_shell_request(
        &self,
        system: incular_platform::NativeWindowSystem,
        request: incular_runtime::NativeApplicationShellRequest,
        target_window: Option<&winit::window::Window>,
    ) -> incular_runtime::NativeApplicationShellApplyResult {
        self.application_shell
            .apply(system, request, target_window)
            .into()
    }
}
