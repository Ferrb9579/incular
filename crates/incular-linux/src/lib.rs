//! Linux desktop adapter for Incular's shared desktop shell.

#[cfg(target_os = "linux")]
mod environment;

pub use incular_desktop::RunError;

#[cfg(target_os = "linux")]
struct LinuxDesktopPlatformServices {
    xlib: Option<x11_dl::xlib::Xlib>,
    environment: environment::LinuxEnvironmentState,
}

#[cfg(target_os = "linux")]
impl Default for LinuxDesktopPlatformServices {
    fn default() -> Self {
        Self {
            xlib: x11_dl::xlib::Xlib::open().ok(),
            environment: environment::LinuxEnvironmentState::default(),
        }
    }
}

#[cfg(target_os = "linux")]
impl incular_desktop::DesktopPlatformServices for LinuxDesktopPlatformServices {
    fn system_environment_preferences(&self) -> incular_platform::SystemEnvironmentPreferences {
        self.environment.preferences()
    }

    fn start_system_environment_watch(
        &self,
        tokio: incular_runtime::TokioHandle,
        wake: std::sync::Arc<dyn Fn() + Send + Sync>,
    ) {
        self.environment.start_watch(tokio, wake);
    }

    fn take_system_environment_change(&self) -> bool {
        self.environment.take_change()
    }

    fn external_file_drag_support(
        &self,
        system: incular_platform::NativeWindowSystem,
    ) -> incular_platform::CapabilitySupport {
        if system == incular_platform::NativeWindowSystem::X11 && self.xlib.is_some() {
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
        if system != incular_platform::NativeWindowSystem::X11 {
            return None;
        }
        use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
        let xlib = self.xlib.as_ref()?;
        let handles = incular_platform::raw_window_handles(window);
        let RawWindowHandle::Xlib(window_handle) = handles.window else {
            return None;
        };
        let Some(RawDisplayHandle::Xlib(display_handle)) = handles.display else {
            return None;
        };
        let display = display_handle
            .display?
            .as_ptr()
            .cast::<x11_dl::xlib::Display>();
        let mut root = 0;
        let mut child = 0;
        let mut root_x = 0;
        let mut root_y = 0;
        let mut window_x = 0;
        let mut window_y = 0;
        let mut mask = 0;
        // SAFETY: the Xlib display/window are borrowed from live Winit handles,
        // all out-pointers reference stack storage, and XQueryPointer is
        // synchronous on the event-loop thread.
        let success = unsafe {
            (xlib.XQueryPointer)(
                display,
                window_handle.window,
                &mut root,
                &mut child,
                &mut root_x,
                &mut root_y,
                &mut window_x,
                &mut window_y,
                &mut mask,
            )
        };
        (success != 0)
            .then(|| winit::dpi::PhysicalPosition::new(f64::from(window_x), f64::from(window_y)))
    }
}

pub fn run_application(application: incular_runtime::Application) -> Result<(), RunError> {
    #[cfg(target_os = "linux")]
    {
        incular_desktop::run_application_with_services(
            application,
            LinuxDesktopPlatformServices::default(),
        )
    }
    #[cfg(not(target_os = "linux"))]
    incular_desktop::run_application(application)
}

pub fn run_window(
    runtime: incular_runtime::Runtime,
    on_action: impl FnMut(incular_widgets::internal::ActionId) + 'static,
) -> Result<(), RunError> {
    #[cfg(target_os = "linux")]
    {
        incular_desktop::run_window_with_services(
            runtime,
            on_action,
            LinuxDesktopPlatformServices::default(),
        )
    }
    #[cfg(not(target_os = "linux"))]
    incular_desktop::run_window(runtime, on_action)
}

#[cfg(feature = "devtools")]
pub use incular_desktop::{
    DevToolsLaunchMode, devtools_launch_mode_from, devtools_runner, devtools_ui_candidates,
};
