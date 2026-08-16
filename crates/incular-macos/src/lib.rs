//! macOS desktop backend using Incular's shared winit/wgpu runner contract.
#[path = "../../incular-linux/src/lib.rs"]
mod desktop_runner;
pub use desktop_runner::{RunError, run_application, run_window};
