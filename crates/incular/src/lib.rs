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
        Alignment, AlignmentDirectional, Axis, AxisDirection, Brightness, Constraints,
        CrossAxisAlignment, EdgeInsets, FlexFit, InputCapabilities, Locale, LocaleResolver,
        LocalizationCatalog, LocalizationError, LocalizedMessage, MainAxisAlignment, MainAxisSize,
        PluralCategory, PluralForms, RuntimeEnvironment, TextDirection, VerticalDirection,
        WrapAlignment, WrapCrossAlignment,
    };
    pub use incular_core::{
        Code, Color, HslColor, HsvColor, ImeEvent, InputEvent, KeyState, KeyboardEvent,
        KeyboardKey, Location, Modifiers, NamedKey, Offset, PointerPhase, RestorationKey,
        RestorationKeyError, RestorationScope, Size, Transform as AffineTransform,
    };
    pub use incular_image::{
        DecodedImage, ImageCache, ImageCacheDiagnostics, ImageError, ImageHandle, ImageId,
        ImageSource,
    };
    pub use incular_navigation::{
        BackDispatchReport, BackDispatcher, BottomSheet, Dialog, ModalBarrier,
        NAVIGATOR_SNAPSHOT_FORMAT_VERSION, NavigationEvent, NavigationRestoreReport, Navigator,
        NavigatorObserver, NavigatorSnapshot, Overlay, OverlayEntry, Page, PopDecision, PopResult,
        RestorableNavigationError, RestorableRoute, RestorableRouteBuildError, RestorableRouteId,
        RestorableRouteIdError, RestorableRouteRegistrationError, Route, RouteRegistry,
        RouteScopeKey, RouteScopeKeyError, RouteTransition,
    };
    #[cfg(feature = "desktop")]
    pub use incular_platform::{
        Fullscreen, WindowCommand, WindowEvent, WindowEventKind, WindowId, WindowLifecycle,
        WindowOperation, WindowOptions, WindowOptionsError,
    };
    pub use incular_rendering::{
        BlendMode, Border, Brush, Canvas, ColorFilter, ColorMatrix, CornerRadii, Decoration,
        DisplayList, DropShadowEffect, Effect, EffectChain, FillRule, GaussianBlur, GradientId,
        GradientStop, GradientStops, ImageSampling, LineCap, LineJoin, LinearGradient, Path,
        PathBuilder, PathId, RRect, RadialGradient, Stroke, SweepGradient, blend_premultiplied,
    };
    pub use incular_runtime::{
        AccessibilityDiagnostics, Application, ApplicationDiagnostics, ApplicationLifecycle,
        AsyncValue, BuildContext, EditingDiagnostics, FileRestorationStore, FocusDiagnostics,
        InMemoryRestorationStore, LastWindowPolicy, NativeWindowCommand, Restorable,
        RestorableWindowFactory, RestorationConfig, RestorationDiagnostics, RestorationHandle,
        RestorationMigration, RestorationStore, RestorationStoreError, Runtime, RuntimeDiagnostics,
        RuntimeErrorReport, Signal, Task, TaskFailure, TaskHandle, TaskScope, TokioHandle,
        UiDispatcher, WindowDiagnostics, WindowError, WindowHandle, WindowOpener,
        WindowRestorationId,
    };
    pub use incular_semantics::{
        Role as SemanticRole, SemanticAction, SemanticActionKind, SemanticNodeId, SemanticState,
        SemanticsDiagnostics, SemanticsTree,
    };
    pub use incular_text::{
        EditableText, FontFamily, FontStyle, FontWeight, RichText, TextAlign, TextLayoutOptions,
        TextOverflow, TextScaler, TextSpan, TextStyle, WidgetSpan,
    };
    pub use incular_widgets::{
        AbsorbPointer, ActionId, Actions, Align, AspectRatio, Autocomplete, AutovalidateMode,
        Baseline, Blend, Blur, BlurController, BoundaryPhysics, Button, Center,
        ColorFilterController, ColorFiltered, ColorMatrixController, ColoredBox, Column, Command,
        ConstrainedBox, CustomPaint, CustomScrollView, DecoratedBox, DismissDirection, Dismissible,
        DragCallbacks, DragDropContext, DragEndDetails, DragFeedback, DragGestureDetector,
        DragTarget, DragUpdateDetails, Draggable, DropShadow, DropShadowController, Effects,
        Expanded, FadeTransition, FittedBox, Flex, Flexible, FocusManager, FocusNode, Form,
        FormField, FormFieldId, FractionallySizedBox, GestureAction, GestureArena,
        GestureArenaEntry, GestureArenaKey, GestureArenaMember, GestureCallbacks, GestureDecision,
        GestureDetector, GestureDisposition, GestureRegion, GridView, Icon, IgnorePointer, Image,
        ImageFit, ImageRepeat, IndexedStack, Key, LayoutBuilder, LimitedBox, ListView,
        MeasuredExtentIndex, MouseRegion, NestedScrollCoordinator, Offstage, Opacity,
        OpacityController, OverflowBox, Padding, PageController, PageView, PathView,
        PointerCapture, PointerEvent, Positioned, RepaintBoundary, RotationController,
        RotationTransition, Row, SafeArea, ScaleController, ScaleGestureDetector, ScaleTransition,
        ScaleUpdateDetails, ScrollController, ScrollDelta, ScrollPhysics, ScrollSpringStep,
        ScrollView, Scrollability, ScrollbarDragDiagnostics, ScrollbarGeometry, ScrollbarStyle,
        SelectableText, SelectionArea, SelectionAreaController, Semantics, Shortcuts, SizedBox,
        SlideTransition, Sliver, SliverAppBar, SliverBox, SliverGrid, SliverList, SliverPadding,
        SliverPersistentHeader, SnapPhysics, Spacer, Stack, Table, Text, TextArea,
        TextEditingController, TextEditingValue, TextField, TextRange, TextSelection, Transform,
        Transition, TranslationController, UnconstrainedBox, VirtualList, Visibility, Widget, Wrap,
        icons,
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
