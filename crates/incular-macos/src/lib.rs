//! macOS desktop adapter for Incular's shared desktop shell.

pub use incular_desktop::{RunError, run_application, run_window};

#[cfg(feature = "devtools")]
pub use incular_desktop::{
    DevToolsLaunchMode, devtools_launch_mode_from, devtools_runner, devtools_ui_candidates,
};
