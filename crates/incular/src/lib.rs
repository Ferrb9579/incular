//! Public facade for the Incular GUI framework.
//!
//! The facade will provide a convenient application-facing API while the
//! framework remains organized internally into focused crates.

pub use incular_accessibility as accessibility;
pub use incular_animation as animation;
pub use incular_assets as assets;
pub use incular_core as core;
pub use incular_layout as layout;
pub use incular_macros as macros;
pub use incular_painting as painting;
pub use incular_runtime as runtime;
pub use incular_text as text;
pub use incular_widgets as widgets;

#[cfg(feature = "desktop")]
pub use incular_platform as platform;

#[cfg(feature = "desktop")]
pub use incular_wgpu as wgpu;
