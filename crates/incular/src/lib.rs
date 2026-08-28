//! Public facade for the Incular GUI framework.
//!
//! The facade will provide a convenient application-facing API while the
//! framework remains organized internally into focused crates.

pub use incular_accessibility as accessibility;
pub use incular_animation as animation;
pub use incular_assets as assets;
pub use incular_config as config;
#[cfg(feature = "controls")]
pub use incular_controls as controls;
pub use incular_core as core;
pub use incular_gestures as gestures;
pub use incular_image as image;
pub use incular_layout as layout;
pub use incular_macros as macros;
#[cfg(feature = "material")]
pub use incular_material as material;
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
    pub use incular_animation::{
        AnimationController, AnimationStatus, BoundedFrictionSimulation, ClampedSimulation, Curve,
        CurveChain, Curves, FrictionSimulation, GravitySimulation, ScrollSpringSimulation,
        Simulation, SpringDescription, SpringSimulation, SpringType, Tolerance, Tween,
        TweenSegment, TweenSequence, TweenValue,
    };
    pub use incular_config::{
        Alignment, AlignmentDirectional, Axis, AxisDirection, Brightness, Constraints,
        CrossAxisAlignment, EdgeInsets, EdgeInsetsDirectional, FlexFit, FractionalOffset,
        InputCapabilities, Locale, LocaleResolver, LocalizationCatalog, LocalizationError,
        LocalizedMessage, MainAxisAlignment, MainAxisSize, PluralCategory, PluralForms,
        RuntimeEnvironment, TextDirection, VerticalDirection, WrapAlignment, WrapCrossAlignment,
    };
    pub use incular_core::{
        ChangeImpact, Code, Color, DirtyFlags, HslColor, HsvColor, ImeEvent, InputEvent,
        Invalidation, Key, KeyState, KeyboardEvent, KeyboardKey, Lerp, LocalKey, Location,
        Modifiers, NamedKey, Offset, PointerPhase, Rect, RestorationKey, RestorationKeyError,
        RestorationScope, Size, Transform as AffineTransform, UniqueKey, ValueKey,
    };
    pub use incular_image::{
        AssetImage, DecodedImage, FileImage, ImageCache, ImageCacheDiagnostics, ImageConfiguration,
        ImageError, ImageHandle, ImageId, ImageProvider, ImageSource, MemoryImage,
    };
    pub use incular_navigation::{
        BackDispatchReport, BackDispatcher, BottomSheet, Dialog, ModalBarrier,
        NAVIGATOR_SNAPSHOT_FORMAT_VERSION, NavigationEvent, NavigationRestoreReport, Navigator,
        NavigatorObserver, NavigatorSnapshot, Overlay, OverlayEntry, Page, PageRouteBuilder,
        PopDecision, PopResult, RestorableNavigationError, RestorableRoute,
        RestorableRouteBuildError, RestorableRouteId, RestorableRouteIdError,
        RestorableRouteRegistrationError, Route, RoutePresentation, RouteRegistry, RouteResult,
        RouteScopeKey, RouteScopeKeyError, RouteSettings, RouteTransition,
    };
    #[cfg(feature = "desktop")]
    pub use incular_platform::{
        Fullscreen, WindowCommand, WindowEvent, WindowEventKind, WindowId, WindowLifecycle,
        WindowOperation, WindowOptions, WindowOptionsError,
    };
    pub use incular_rendering::{
        BlendMode, Border as RenderBorder, Brush, Canvas, ColorFilter, ColorMatrix, CornerRadii,
        Decoration, DisplayList, DropShadowEffect, Effect, EffectChain, FillRule, FilterQuality,
        GaussianBlur, GradientId, GradientStop, GradientStops, ImageSampling, LineCap, LineJoin,
        LinearGradient, Paint, PaintStyle, Path, PathBuilder, PathId, RRect, RadialGradient,
        Shader, Shadow, Stroke, SweepGradient, blend_premultiplied,
    };
    pub use incular_runtime::{
        AccessibilityDiagnostics, Application, ApplicationDiagnostics, ApplicationLifecycle,
        AsyncState, AsyncValue, BudgetStatistics, BuildContext, EditingDiagnostics,
        FileRestorationStore, FocusDiagnostics, FrameHistory, FrameRecord, FrameStatistics,
        GpuSample, InMemoryRestorationStore, LastWindowPolicy, NativeWindowCommand, PerformanceHub,
        PerformanceSnapshot, ProfilerMode, RenderFrameMetrics, Restorable, RestorableWindowFactory,
        RestorationConfig, RestorationDiagnostics, RestorationHandle, RestorationMigration,
        RestorationStore, RestorationStoreError, Runtime, RuntimeDiagnostics, RuntimeErrorReport,
        SchedulerCounters, Signal, Task, TaskFailure, TaskHandle, TaskScope, TokioHandle,
        UiDispatcher, UndoHistoryController, UndoHistoryState, WindowDiagnostics, WindowError,
        WindowHandle, WindowOpener, WindowRestorationId,
    };
    pub use incular_semantics::{
        Role as SemanticRole, SemanticAction, SemanticActionKind, SemanticNodeId, SemanticState,
        SemanticsDiagnostics, SemanticsTree,
    };
    pub use incular_text::{
        EditableText as TextEditingModel, FontFamily, FontFeature, FontStyle, FontVariation,
        FontWeight, IconData, LineHeight, RichText, StrutStyle, TextAlign, TextBaseline,
        TextDecoration, TextDecorationStyle, TextEditingController, TextEditingValue,
        TextHeightBehavior, TextLayoutOptions, TextLeadingDistribution, TextOverflow, TextRange,
        TextScaler, TextSelection, TextShadow, TextSpan, TextStyle, TextWidthBasis, WidgetSpan,
    };
    pub use incular_widgets::{
        AbsorbPointer, Action, ActionListener, ActionResult, Actions, Align, AlignTransition,
        AnimatedAlign, AnimatedBuilder, AnimatedContainer, AnimatedCrossFade,
        AnimatedDefaultTextStyle, AnimatedFractionallySizedBox, AnimatedGrid, AnimatedList,
        AnimatedModalBarrier, AnimatedOpacity, AnimatedPadding, AnimatedPhysicalModel,
        AnimatedPositioned, AnimatedPositionedDirectional, AnimatedRotation, AnimatedScale,
        AnimatedSize, AnimatedSlide, AnimatedSwitcher, AnnotatedRegion, AspectRatio,
        AutocompleteHighlightedOption, AutofillGroup, AutomaticKeepAlive, AutovalidateMode,
        BackButtonListener, Banner, Baseline, Border, BorderDirectional, BorderRadius,
        BorderRadiusDirectional, BorderSide, BorderStyle, BoxBorder, BoxDecoration, BoxFit,
        BoxShadow, BoxShape, CallbackShortcuts, Center, CheckedModeBanner, ClipOval, ClipPath,
        ClipRRect, ClipRSuperellipse, ClipRect, ColorFiltered, ColoredBox, Column, Command,
        CommandId, ConstrainedBox, ConstraintsTransformBox, Container, CustomPaint, CustomPainter,
        CustomScrollView, DecoratedBox, DecoratedBoxTransition, DecoratedSliver, DecorationImage,
        DefaultSelectionStyle, DefaultTextStyle, DefaultTextStyleTransition, Directionality,
        DismissDirection, Dismissible, DragTarget, Draggable, DraggableScrollableActuator,
        DraggableScrollableSheet, DualTransitionBuilder, EditableText, ErrorWidget, ExcludeFocus,
        ExcludeFocusTraversal, Expanded, Expansible, FadeTransition, FittedBox, Flex, Flexible,
        Focus, FocusManager, FocusNode, FocusScope, FocusScopeNode, FocusTraversalGroup,
        FocusTraversalOrder, FocusTraversalPolicy, FocusableActionDetector, Form, FormController,
        FormField, FormFieldState, FormState, FractionalTranslation, FractionallySizedBox,
        GestureDetector, GridPaper, GridView, Hero, HeroControllerScope, HeroMode, Icon, IconTheme,
        IgnorePointer, Image, ImageFiltered, ImageIcon, ImageRepeat, IndexedSemantics,
        IndexedStack, Intent, KeepAlive, KeyboardListener, KeyedSubtree, LayoutBuilder, LimitedBox,
        ListBody, ListView, ListWheelScrollView, Listener, Localizations, LogicalShortcutKey,
        LongPressDraggable, MatrixTransition, MediaQuery, MediaQueryData, MouseRegion,
        NavigationToolbar, NavigatorPopHandler, NestedScrollView, NotificationListener, Offstage,
        Opacity, OrderedTraversalPolicy, Orientation, OrientationBuilder, OverflowBar, OverflowBox,
        OverlayPortal, Padding, PageController, PageStorage, PageView, PerformanceOverlay,
        PinnedHeaderSliver, PlatformMenuBar, PopScope, Positioned, PositionedTransition,
        PrimaryScrollController, RadioGroup, Radius, RawAutocomplete, RawGestureDetector, RawImage,
        RawRadio, RawScrollbar, RawTooltip, ReadingOrderTraversalPolicy,
        RelativePositionedTransition, ReorderableDelayedDragStartListener,
        ReorderableDragStartListener, ReorderableList, RepaintBoundary, RepeatingAnimationBuilder,
        RootRestorationScope, RotatedBox, RotationTransition, Router, Row, SafeArea,
        ScaleTransition, ScrollConfiguration, ScrollController, ScrollMetrics,
        ScrollNotificationObserver, ScrollPhysics, ScrollViewKeyboardDismissBehavior,
        SelectableRegion, SelectionContainer, SelectionListener, Semantics, SemanticsDebugger,
        SensitiveContent, SensitiveContentHost, ShaderMask, ShortcutKey, ShortcutTrigger,
        Shortcuts, SingleChildScrollView, SizeTransition, SizedBox, SizedOverflowBox,
        SlideTransition, Sliver, SliverAnimatedGrid, SliverAnimatedList,
        SliverConstrainedCrossAxis, SliverCrossAxisExpanded, SliverCrossAxisGroup,
        SliverFillRemaining, SliverFillViewport, SliverFixedExtentList, SliverFloatingHeader,
        SliverGrid, SliverIgnorePointer, SliverLayoutBuilder, SliverList, SliverMainAxisGroup,
        SliverOffstage, SliverOpacity, SliverOverlapAbsorber, SliverOverlapInjector, SliverPadding,
        SliverPersistentHeader, SliverPrototypeExtentList, SliverReorderableList,
        SliverResizingHeader, SliverSafeArea, SliverToBoxAdapter, SliverVariedExtentList,
        SliverVisibility, SnapshotWidget, Spacer, Stack, Table, TableCell, TapRegion,
        TapRegionSurface, Text, TextFieldTapRegion, TickerMode, TileMode, Title, Transform,
        TreeSliver, TweenAnimationBuilder, TwoDimensionalScrollView, TwoDimensionalScrollable,
        TwoDimensionalViewport, UnconstrainedBox, UndoHistory, UnmanagedRestorationScope, View,
        ViewAnchor, Visibility, Widget, WidgetOrderTraversalPolicy, WidgetsApp, Wrap,
    };
}

/// Opt-in Controls convenience imports. Controls stay separate from the base
/// prelude so applications without the optional `controls` feature keep the
/// renderer-neutral API surface.
#[cfg(feature = "controls")]
pub mod controls_prelude {
    pub use incular_controls::prelude::*;
}

/// Opt-in Material convenience imports. Material is intentionally not part of
/// [`prelude`] so a base Widgets application cannot accidentally depend on a
/// design system. Enable the `material` feature and import this module when
/// using Material components.
#[cfg(feature = "material")]
pub mod material_prelude {
    pub use incular_material::prelude::*;
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

pub use incular_runtime::PERFORMANCE_OVERLAY_KEY;

/// Formats a snapshot into standard overlay lines (thin wrapper over
/// [`incular_runtime::PerformanceSnapshot::overlay_lines`]).
#[must_use]
pub fn overlay_lines(snapshot: &runtime::PerformanceSnapshot) -> Vec<String> {
    snapshot.overlay_lines()
}

/// Installs the repaint-contained debug performance overlay into `window`.
/// The tree must contain a placeholder keyed with
/// [`PERFORMANCE_OVERLAY_KEY`]; publishing rebuilds only that element.
#[cfg(feature = "desktop")]
pub fn install_performance_overlay(
    application: &mut runtime::Application,
    window: platform::WindowId,
) -> Result<(), widgets::internal::TreeError> {
    application.install_performance_overlay(window)
}
