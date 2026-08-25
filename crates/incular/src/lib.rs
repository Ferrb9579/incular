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
        Invalidation, KeyState, KeyboardEvent, KeyboardKey, Lerp, Location, Modifiers, NamedKey,
        Offset, PointerPhase, Rect, RestorationKey, RestorationKeyError, RestorationScope, Size,
        Transform as AffineTransform,
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
        BlendMode, Border as RenderBorder, Brush, Canvas, ColorFilter, ColorMatrix, CornerRadii,
        Decoration, DisplayList, DropShadowEffect, Effect, EffectChain, FillRule, GaussianBlur,
        GradientId, GradientStop, GradientStops, ImageSampling, LineCap, LineJoin, LinearGradient,
        Path, PathBuilder, PathId, RRect, RadialGradient, Stroke, SweepGradient,
        blend_premultiplied,
    };
    pub use incular_runtime::{
        AccessibilityDiagnostics, Application, ApplicationDiagnostics, ApplicationLifecycle,
        AsyncValue, BudgetStatistics, BuildContext, EditingDiagnostics, FileRestorationStore,
        FocusDiagnostics, FrameHistory, FrameRecord, FrameStatistics, GpuSample,
        InMemoryRestorationStore, LastWindowPolicy, NativeWindowCommand, PerformanceHub,
        PerformanceSnapshot, ProfilerMode, RenderFrameMetrics, Restorable, RestorableWindowFactory,
        RestorationConfig, RestorationDiagnostics, RestorationHandle, RestorationMigration,
        RestorationStore, RestorationStoreError, Runtime, RuntimeDiagnostics, RuntimeErrorReport,
        SchedulerCounters, Signal, Task, TaskFailure, TaskHandle, TaskScope, TokioHandle,
        UiDispatcher, WindowDiagnostics, WindowError, WindowHandle, WindowOpener,
        WindowRestorationId,
    };
    pub use incular_semantics::{
        Role as SemanticRole, SemanticAction, SemanticActionKind, SemanticNodeId, SemanticState,
        SemanticsDiagnostics, SemanticsTree,
    };
    pub use incular_text::{
        EditableText, FontFamily, FontFeature, FontStyle, FontVariation, FontWeight, LineHeight,
        RichText, StrutStyle, TextAlign, TextBaseline, TextDecoration, TextDecorationStyle,
        TextHeightBehavior, TextLayoutOptions, TextLeadingDistribution, TextOverflow, TextScaler,
        TextShadow, TextSpan, TextStyle, TextWidthBasis, WidgetSpan,
    };
    pub use incular_widgets::{
        AbsorbPointer, ActionId, ActionListener, Actions, Align, AlignTransition, AnimatedAlign,
        AnimatedBuilder, AnimatedContainer, AnimatedCrossFade, AnimatedDefaultTextStyle,
        AnimatedFractionallySizedBox, AnimatedGrid, AnimatedList, AnimatedModalBarrier,
        AnimatedOpacity, AnimatedPadding, AnimatedPhysicalModel, AnimatedPositioned,
        AnimatedPositionedDirectional, AnimatedRotation, AnimatedScale, AnimatedSize,
        AnimatedSlide, AnimatedSwitcher, AnnotatedRegion, AspectRatio, Autocomplete,
        AutocompleteHighlightedOption, AutofillGroup, AutomaticKeepAlive, AutovalidateMode,
        BackButtonListener, Banner, Baseline, Blend, Blur, BlurController, BlurStyle, Border,
        BorderDirectional, BorderRadius, BorderRadiusDirectional, BorderSide, BorderStyle,
        BoundaryPhysics, BoxBorder, BoxDecoration, BoxFit, BoxShadow, BoxShape, Button,
        CallbackShortcuts, Center, CheckedModeBanner, ClipOval, ClipPath, ClipRRect,
        ClipRSuperellipse, ClipRect, ColorFilterController, ColorFiltered, ColorMatrixController,
        ColoredBox, Column, Command, ConstrainedBox, ConstraintsTransformBox, Container,
        CustomPaint, CustomPainter, CustomScrollView, DecoratedBox, DecoratedBoxTransition,
        DecoratedSliver, DecorationImage, DefaultTextStyle, DefaultTextStyleTransition,
        Directionality, DismissDirection, Dismissible, DragCallbacks, DragDownDetails,
        DragDropContext, DragEndDetails, DragFeedback, DragGestureDetector, DragStartBehavior,
        DragStartDetails, DragTarget, DragUpdateDetails, Draggable, DraggableScrollableActuator,
        DraggableScrollableSheet, DropShadow, DropShadowController, DualTransitionBuilder, Effects,
        ErrorWidget, ExcludeFocus, ExcludeFocusTraversal, Expanded, Expansible, FadeTransition,
        FilteringTextInputFormatter, FittedBox, Flex, Flexible, Focus, FocusManager, FocusNode,
        FocusScope, FocusTraversalGroup, FocusTraversalOrder, FocusTraversalPolicy,
        FocusableActionDetector, Form, FormField, FormFieldId, FormFieldState,
        FractionalTranslation, FractionallySizedBox, FutureBuilder, GenericFormField,
        GestureAction, GestureArena, GestureArenaEntry, GestureArenaKey, GestureArenaMember,
        GestureCallbacks, GestureDecision, GestureDetector, GestureDisposition, GestureRegion,
        GridDelegate, GridPaper, GridView, Hero, HeroControllerScope, HeroMode, Icon, IconTheme,
        IgnorePointer, Image, ImageFiltered, ImageFit, ImageIcon, ImageRepeat, IndexedSemantics,
        IndexedStack, InteractiveViewer, ItemExtentStrategy, KeepAlive, KeepAlivePolicy, Key,
        KeyboardListener, KeyedSubtree, LayoutBuilder, LengthLimitingTextInputFormatter,
        LimitedBox, ListBody, ListView, ListWheelScrollView, ListenableBuilder, Listener,
        LongPressDraggable, LongPressEndDetails, LongPressMoveUpdateDetails, LongPressStartDetails,
        MatrixTransition, MaxLengthEnforcement, MeasuredExtentIndex, MouseRegion,
        NavigationToolbar, NavigatorPopHandler, NestedScrollCoordinator, NestedScrollView,
        NotificationListener, Offstage, Opacity, OpacityController, OrderedTraversalPolicy,
        Orientation, OrientationBuilder, OverflowBar, OverflowBox, OverlayPortal, Padding,
        PageController, PageStorage, PageView, PathView, PerformanceOverlay, PhysicalModel,
        PhysicalShape, PinnedHeaderSliver, Placeholder, PlatformMenuBar, PointerCapture,
        PointerEvent, PopScope, Positioned, PositionedTransition, PreferredSize,
        PrimaryScrollController, RadioGroup, Radius, RawAutocomplete, RawGestureDetector, RawImage,
        RawRadio, RawScrollbar, RawTooltip, ReadingOrderTraversalPolicy,
        RelativePositionedTransition, ReorderableDelayedDragStartListener,
        ReorderableDragStartListener, ReorderableList, RepaintBoundary, RepaintBoundaryPolicy,
        RepeatingAnimationBuilder, RootRestorationScope, RotatedBox, RotationController,
        RotationTransition, Router, Row, SafeArea, ScaleController, ScaleEndDetails,
        ScaleGestureDetector, ScaleStartDetails, ScaleTransition, ScaleUpdateDetails,
        ScrollCacheExtent, ScrollConfiguration, ScrollController, ScrollDelta, ScrollMetrics,
        ScrollNotificationObserver, ScrollPhysics, ScrollSpringStep, ScrollView, ScrollViewConfig,
        ScrollViewKeyboardDismissBehavior, Scrollability, ScrollbarDragDiagnostics,
        ScrollbarGeometry, ScrollbarStyle, SelectableRegion, SelectableText, SelectionArea,
        SelectionAreaController, SelectionContainer, SelectionListener, SemanticIndexPolicy,
        Semantics, SemanticsDebugger, SensitiveContent, SensitiveContentHost, ShaderMask,
        Shortcuts, SingleChildScrollView, SizeTransition, SizedBox, SizedOverflowBox,
        SlideTransition, Sliver, SliverAnimatedGrid, SliverAnimatedList, SliverAppBar, SliverBox,
        SliverConstrainedCrossAxis, SliverCrossAxisExpanded, SliverCrossAxisGroup,
        SliverFillRemaining, SliverFillViewport, SliverFixedExtentList, SliverFloatingHeader,
        SliverGrid, SliverIgnorePointer, SliverLayoutBuilder, SliverList, SliverMainAxisGroup,
        SliverOffstage, SliverOpacity, SliverOverlapAbsorber, SliverOverlapInjector, SliverPadding,
        SliverPersistentHeader, SliverPrototypeExtentList, SliverReorderableList,
        SliverResizingHeader, SliverSafeArea, SliverVariedExtentList, SliverVisibility,
        SnapPhysics, SnapshotWidget, Spacer, SplitPosition, SplitView, Stack, StatefulBuilder,
        StreamBuilder, Table, TableCell, TapDownDetails, TapRegion, TapRegionSurface, TapUpDetails,
        Text, TextArea, TextEditingController, TextEditingValue, TextField, TextFieldTapRegion,
        TextFormField, TextInputFormatter, TextRange, TextSelection, TickerMode, TileMode, Title,
        Transform, Transition, TranslationController, TreeSliver, TweenAnimationBuilder,
        TwoDimensionalScrollView, TwoDimensionalScrollable, TwoDimensionalViewport,
        UnconstrainedBox, UndoHistory, UnmanagedRestorationScope, ValueListenableBuilder, Velocity,
        View, ViewAnchor, VirtualList, Visibility, Widget, WidgetOrderTraversalPolicy, WidgetsApp,
        Wrap, icons,
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
) -> Result<(), widgets::TreeError> {
    application.install_performance_overlay(window)
}
