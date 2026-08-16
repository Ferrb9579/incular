//! Public facade for the Incular GUI framework.
//!
//! The facade will provide a convenient application-facing API while the
//! framework remains organized internally into focused crates.

pub use incular_accessibility as accessibility;
pub use incular_animation as animation;
pub use incular_assets as assets;
pub use incular_config as config;
pub use incular_core as core;
pub use incular_gestures as gestures;
pub use incular_image as image;
pub use incular_layout as layout;
pub use incular_macros as macros;
pub use incular_navigation as navigation;
pub use incular_painting as painting;
pub use incular_rendering as rendering;
pub use incular_runtime as runtime;
pub use incular_scroll as scroll;
pub use incular_semantics as semantics;
pub use incular_text as text;
pub use incular_widgets as widgets;

/// Common application-facing types for the initial native UI slice.
pub mod prelude {
    pub use incular_config::{
        Alignment, AlignmentDirectional, Axis, AxisDirection, Constraints, CrossAxisAlignment,
        EdgeInsets, FlexFit, MainAxisAlignment, MainAxisSize, TextDirection, VerticalDirection,
        WrapAlignment, WrapCrossAlignment,
    };
    pub use incular_core::{
        Color, ImeEvent, InputEvent, KeyCode, KeyEvent, Modifiers, Offset, PointerPhase, Size,
    };
    pub use incular_image::{
        DecodedImage, ImageCache, ImageCacheDiagnostics, ImageError, ImageHandle, ImageId,
        ImageSource,
    };
    pub use incular_navigation::{
        BottomSheet, Dialog, ModalBarrier, Navigator, Overlay, OverlayEntry, Page, Route,
        RouteRegistry, RouteTransition,
    };
    pub use incular_rendering::{
        BlendMode, Border, Brush, Canvas, ColorFilter, ColorMatrix, CornerRadii, Decoration,
        DisplayList, DropShadowEffect, Effect, EffectChain, FillRule, GaussianBlur, GradientId,
        GradientStop, GradientStops, ImageSampling, LineCap, LineJoin, LinearGradient, Path,
        PathBuilder, PathId, RRect, RadialGradient, Stroke, blend_premultiplied,
    };
    pub use incular_runtime::{
        Application, BuildContext, EditingDiagnostics, FocusDiagnostics, Runtime, Signal,
    };
    pub use incular_semantics::{
        Role as SemanticRole, SemanticAction, SemanticActionKind, SemanticNodeId, SemanticState,
        SemanticsDiagnostics, SemanticsTree,
    };
    pub use incular_text::{
        EditableText, FontFamily, FontStyle, FontWeight, RichText, TextAlign, TextScaler, TextSpan,
        TextStyle, WidgetSpan,
    };
    pub use incular_widgets::{
        ActionId, Align, AspectRatio, Baseline, Blend, Blur, BlurController, Button, Center,
        ColorFilterController, ColorFiltered, ColorMatrixController, Column, ConstrainedBox,
        CustomPaint, CustomScrollView, DecoratedBox, DragCallbacks, DragEndDetails,
        DragGestureDetector, DragUpdateDetails, DropShadow, DropShadowController, Effects,
        FadeTransition, Flex, FocusNode, FractionallySizedBox, GestureCallbacks, GestureDetector,
        GestureRegion, GridView, Icon, Image, ImageFit, ImageRepeat, Key, ListView, Offstage,
        Opacity, OpacityController, Padding, PageController, PageView, PathView, PointerEvent,
        RepaintBoundary, Row, ScaleGestureDetector, ScaleUpdateDetails, ScrollController,
        ScrollView, ScrollbarDragDiagnostics, ScrollbarGeometry, ScrollbarStyle, Shortcuts,
        SizedBox, SlideTransition, Sliver, SliverAppBar, SliverBox, SliverGrid, SliverList,
        SliverPadding, SliverPersistentHeader, Stack, Table, Text, TextArea, TextEditingController,
        TextEditingValue, TextField, TextRange, TextSelection, Transition, TranslationController,
        UnconstrainedBox, VirtualList, Visibility, Widget, Wrap, icons,
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
