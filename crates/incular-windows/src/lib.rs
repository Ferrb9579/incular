//! Windows desktop adapter for Incular's shared desktop shell.
mod crash_reporter;

pub use incular_desktop::RunError;

pub fn run_application(application: incular_runtime::Application) -> Result<(), RunError> {
    let _crash_handler = crash_reporter::install();
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
