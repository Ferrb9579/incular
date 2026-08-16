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
    pub use incular_accessibility::{
        Role as SemanticRole, SemanticAction, SemanticActionKind, SemanticNodeId, SemanticState,
        SemanticsDiagnostics, SemanticsTree,
    };
    pub use incular_assets::{
        AssetCache, AssetDiagnostics, AssetError, DecodedImage, ImageHandle, ImageId, ImageSource,
    };
    pub use incular_core::{
        Color, ImeEvent, InputEvent, KeyCode, KeyEvent, Modifiers, Offset, Size,
    };
    pub use incular_layout::{Alignment, Constraints, EdgeInsets};
    pub use incular_painting::{
        BlendMode, Border, Brush, ColorFilter, ColorMatrix, CornerRadii, Decoration,
        DropShadowEffect, Effect, EffectChain, FillRule, GaussianBlur, GradientId, GradientStop,
        GradientStops, ImageSampling, LineCap, LineJoin, LinearGradient, Path, PathBuilder, PathId,
        RRect, RadialGradient, Stroke, blend_premultiplied,
    };
    pub use incular_runtime::{
        Application, BuildContext, EditingDiagnostics, FocusDiagnostics, Runtime, Signal,
    };
    pub use incular_text::{FontFamily, FontStyle, FontWeight, TextAlign, TextStyle};
    pub use incular_widgets::{
        ActionId, Blend, Blur, BlurController, Button, ColorFilterController, ColorFiltered,
        ColorMatrixController, DecoratedBox, DropShadow, DropShadowController, Effects, Icon,
        Image, ImageFit, Key, Opacity, OpacityController, PathView, ScrollController, ScrollView,
        ScrollbarDragDiagnostics, ScrollbarGeometry, ScrollbarStyle, Text, TextArea,
        TextEditingController, TextEditingValue, TextField, TextRange, TextSelection,
        TranslationController, VirtualList, Widget, icons,
    };
}

#[cfg(feature = "desktop")]
pub use incular_platform as platform;

#[cfg(feature = "desktop")]
pub use incular_wgpu as wgpu;

#[cfg(feature = "desktop")]
pub use incular_linux as linux;

#[cfg(all(feature = "desktop", target_os = "macos"))]
pub use incular_macos as macos;
#[cfg(all(feature = "desktop", target_os = "windows"))]
pub use incular_windows as windows;

/// Runs an application on the native backend selected by the compilation target.
#[cfg(all(feature = "desktop", target_os = "linux"))]
pub fn run(application: incular_runtime::Application) -> Result<(), incular_linux::RunError> {
    incular_linux::run_application(application)
}
#[cfg(all(feature = "desktop", target_os = "windows"))]
pub fn run(application: incular_runtime::Application) -> Result<(), incular_windows::RunError> {
    incular_windows::run_application(application)
}
#[cfg(all(feature = "desktop", target_os = "macos"))]
pub fn run(application: incular_runtime::Application) -> Result<(), incular_macos::RunError> {
    incular_macos::run_application(application)
}
