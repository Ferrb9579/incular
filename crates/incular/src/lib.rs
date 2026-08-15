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

/// Common application-facing types for the initial native UI slice.
pub mod prelude {
    pub use incular_core::{Color, Offset, Size};
    pub use incular_layout::{Alignment, Constraints, EdgeInsets};
    pub use incular_runtime::{Application, BuildContext, Runtime, Signal};
    pub use incular_text::{FontFamily, FontStyle, FontWeight, TextAlign, TextStyle};
    pub use incular_widgets::{
        ActionId, Button, Key, ScrollController, ScrollView, Text, TranslationController, Widget,
    };
}

#[cfg(feature = "desktop")]
pub use incular_platform as platform;

#[cfg(feature = "desktop")]
pub use incular_wgpu as wgpu;

#[cfg(feature = "desktop")]
pub use incular_linux as linux;
