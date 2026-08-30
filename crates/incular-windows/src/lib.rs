//! Windows desktop backend using Incular's shared winit/wgpu runner contract.
mod crash_reporter;
#[path = "../../incular-linux/src/lib.rs"]
mod desktop_runner;

pub use desktop_runner::RunError;

pub fn run_application(application: incular_runtime::Application) -> Result<(), RunError> {
    let _crash_handler = crash_reporter::install();
    desktop_runner::run_application(application)
}

pub fn run_window(
    runtime: incular_runtime::Runtime,
    on_action: impl FnMut(incular_widgets::internal::ActionId) + 'static,
) -> Result<(), RunError> {
    let _crash_handler = crash_reporter::install();
    desktop_runner::run_window(runtime, on_action)
}

#[cfg(feature = "devtools")]
pub use desktop_runner::devtools_runner;
