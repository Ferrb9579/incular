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
pub use incular_semantics as semantics;
pub use incular_text as text;
pub use incular_widgets as widgets;

/// Public scrolling policies and notifications.
///
/// Retained extent indexes, scrollbar geometry, and viewport configuration
/// are implementation details of the widget/runtime boundary and are kept
/// out of the application facade.
pub mod scroll {
    pub use incular_scroll::{
        BoundaryPhysics, ClampingScrollPhysics, DragStartBehavior, ScrollController, ScrollMetrics,
        ScrollNotification, ScrollNotificationSubscription, ScrollNotificationType, ScrollPhysics,
        ScrollViewKeyboardDismissBehavior, Scrollability, SnapPhysics,
    };
}

/// Reactive values and owner-mounted asynchronous primitives. This namespace
/// keeps [`reactive::Effect`] and [`reactive::Action`] distinct from Incular's rendering and
/// widget types with the same conventional names.
pub mod reactive {
    pub use incular_runtime::{
        Action, ActionDispatchError, ActionError, ActionState, Effect, Memo,
    };
}

/// High-level application testing tools. These drive Incular's normalized
/// input/event path in-process and capture rendered application frames without
/// injecting OS input or capturing the desktop.
pub mod testing {
    pub use incular_runtime::{Screenshot, Simulation, SimulationError};
}

/// Common application-facing types for the initial native UI slice.
pub mod prelude {
    pub use incular_animation::{
        AnimationController, AnimationStatus, BoundedFrictionSimulation, ClampedSimulation, Curve,
        CurveChain, Curves, FrictionSimulation, GravitySimulation, ScrollSpringSimulation,
        Simulation, SpringDescription, SpringSimulation, SpringType, Tolerance, Tween,
        TweenSegment, TweenSequence, TweenValue,
    };
    pub use incular_config::{
        Alignment, AlignmentDirectional, ApplicationDefaults, Axis, AxisDirection, Brightness,
        Constraints, ContentSensitivity, CrossAxisAlignment, EdgeInsets, EdgeInsetsDirectional,
        FlexFit, FractionalOffset, InputCapabilities, Locale, LocaleResolver, LocalizationCatalog,
        LocalizationError, LocalizedMessage, MainAxisAlignment, MainAxisSize, PluralCategory,
        PluralForms, RuntimeEnvironment, TextDirection, TransientPresentation, TransientRole,
        TransparencyMode, VerticalDirection, WidgetDefaults, WindowSizePolicy, WrapAlignment,
        WrapCrossAlignment,
    };
    pub use incular_core::{
        BACK_POINTER_BUTTON, ChangeImpact, Code, Color, DirtyFlags, FORWARD_POINTER_BUTTON,
        HslColor, HsvColor, ImeEvent, InputEvent, Invalidation, Key, KeyState, KeyboardEvent,
        KeyboardKey, Lerp, LocalKey, Location, Modifiers, NamedKey, Offset, PRIMARY_POINTER_BUTTON,
        PointerDeviceKind, PointerPhase, Rect, RestorationKey, RestorationKeyError,
        RestorationScope, SECONDARY_POINTER_BUTTON, Size, TERTIARY_POINTER_BUTTON,
        Transform as AffineTransform, UniqueKey, ValueKey, WindowResizeDirection,
        additional_pointer_button_mask,
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
        AdvancedInputCapabilities, ApplicationServiceCapabilities, CapabilitySupport,
        ContentSensitivityBackend, ContentSensitivityCapability, ContentSensitivityNoOpReason,
        ContentSensitivityOutcome, CursorGrabMode, DataTransferCapabilities, DisplayId,
        DisplayPlacementArea, DisplayPlacementCapabilities, DisplaySnapshot, Fullscreen,
        LogicalDisplayPosition, LogicalScreenPosition, LogicalScreenRect, LogicalSizeLimits,
        LogicalSizeLimitsError, LogicalWindowPosition, LogicalWindowPositionError,
        MemoryContentSensitivityBackend, MemoryTextInputAdapter, NativeMenuCapabilities,
        NativeOperationCompletion, NativeRequestId, NoopContentSensitivityBackend,
        PhysicalDisplayPosition, PhysicalScreenPosition, PhysicalScreenRect, PlatformCapabilities,
        PlatformOperationError, PlatformOperationErrorKind, PlatformOperationResult,
        PointerMetadata, TextInputAction, TextInputAdapter, TextInputClientId, TextInputCommand,
        TextInputConfiguration, TextInputState, TextInputType, TransientSurfaceCapabilities,
        UserAttentionType, WindowCommand, WindowControlCapabilities, WindowEvent, WindowEventKind,
        WindowIcon, WindowIconError, WindowId, WindowLevel, WindowLifecycle, WindowObservedState,
        WindowOperation, WindowOptions, WindowOptionsError, WindowRequestedState,
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
        GpuSample, InMemoryRestorationStore, LastWindowPolicy, Memo,
        NativeOperationCompletionStatus, NativeOperationRequest, NativeWindowCommand,
        PerformanceHub, PerformanceSnapshot, ProfilerMode, RenderFrameMetrics, Restorable,
        RestorableWindowFactory, RestorationConfig, RestorationDiagnostics, RestorationHandle,
        RestorationMigration, RestorationStore, RestorationStoreError, Runtime, RuntimeDiagnostics,
        RuntimeErrorReport, SchedulerCounters, Signal, Task, TaskFailure, TaskHandle, TaskScope,
        TokioHandle, UiDispatcher, UndoHistoryController, UndoHistoryState,
        WindowCommandEnqueueError, WindowDiagnostics, WindowError, WindowHandle, WindowOpener,
        WindowPlacementError, WindowRestorationId,
    };
    pub use incular_semantics::{
        Role as SemanticRole, SemanticAction, SemanticActionKind, SemanticNodeId, SemanticState,
        SemanticsDiagnostics, SemanticsTree,
    };
    pub use incular_text::{
        EditableText as TextEditingModel, FontFamily, FontFeature, FontStyle, FontVariation,
        FontWeight, IconData, LineHeight, RichText, StrutStyle, TextAffinity, TextAlign,
        TextBaseline, TextCaretPosition, TextDecoration, TextDecorationStyle,
        TextEditingController, TextEditingValue, TextHeightBehavior, TextLayoutOptions,
        TextLeadingDistribution, TextOverflow, TextRange, TextScaler, TextSelection, TextShadow,
        TextSpan, TextStyle, TextWidthBasis, WidgetSpan,
    };
    pub use incular_widgets::{
        AbsorbPointer, Action, ActionInvocationPhase, ActionListener, ActionResult, Actions, Align,
        AlignTransition, AnimatedGrid, AnimatedGridController, AnimatedItem, AnimatedItemBuilder,
        AnimatedItemPhase, AnimatedList, AnimatedListController, AnimatedModalBarrier,
        AnimatedRemovedItemBuilder, ApplicationBootstrapHost, ApplicationBootstrapOptions,
        ApplicationBootstrapSpec, AspectRatio, AutocompleteHighlightedOption, AutofillGroup,
        AutomaticKeepAlive, AutovalidateMode, AuxiliaryViewError, AuxiliaryViewHandle,
        AuxiliaryViewHost, AuxiliaryViewOutcome, AuxiliaryViewRequest, BackButtonDispatcher,
        BackButtonListener, BackCallbackSubscription, BackHandlerResult, BackRegistration, Banner,
        BannerLocation, Baseline, BasicRouterDelegate, Border, BorderDirectional, BorderRadius,
        BorderRadiusDirectional, BorderSide, BorderStyle, BoxBorder, BoxDecoration, BoxFit,
        BoxShadow, BoxShape, CacheExtentStyle, CallbackShortcuts, Center, ChangeReportingBehavior,
        CheckedModeBanner, ChildVicinity, ClipOval, ClipPath, ClipRRect, ClipRSuperellipse,
        ClipRect, ClosureRouteInformationParser, ColorFiltered, ColoredBox, Column, Command,
        CommandId, ConstrainedBox, ConstraintsTransformBox, Container, CustomPaint, CustomPainter,
        CustomScrollView, DecoratedBox, DecoratedBoxTransition, DecoratedSliver, DecorationImage,
        DefaultSelectionStyle, DefaultTextStyle, DefaultTextStyleTransition, DiagonalDragBehavior,
        Directionality, DismissDirection, Dismissible, DragDropContext, DragTarget, Draggable,
        DraggableNotificationSubscription, DraggableScrollableActuator,
        DraggableScrollableController, DraggableScrollableNotification, DraggableScrollableSheet,
        DraggableScrollableState, DraggableSheetDelta, DraggableSheetExtent,
        DraggableSizeAnimation, DraggableSnap, DraggableSnapTarget, DualTransitionBuilder,
        EditableText, ErrorWidget, ExcludeFocus, ExcludeFocusTraversal, Expanded, Expansible,
        FadeTransition, FittedBox, Flex, Flexible, Focus, FocusManager, FocusNode, FocusScope,
        FocusScopeNode, FocusTraversalGroup, FocusTraversalOrder, FocusTraversalPolicy,
        FocusableActionDetector, Form, FormController, FormField, FormFieldState, FormState,
        FractionalTranslation, FractionallySizedBox, GestureDetector, GestureRecognizer,
        GestureRecognizerFactory, GestureRecognizerFactoryError,
        GestureRecognizerFactoryWithHandlers, GridPaper, GridView, Icon, IconTheme, IgnorePointer,
        Image, ImageFiltered, ImageIcon, ImageRepeat, IndexedStack, Intent, KeepAlive,
        KeepAliveHandle, KeepAliveNotification, KeepAliveRegistry, KeyboardListener, KeyedSubtree,
        LayoutBuilder, LimitedBox, ListBody, ListView, ListWheelScrollView, ListWheelViewport,
        Listener, ListenerCallbacks, Localizations, LogicalShortcutKey, LongPressDraggable,
        LookupBoundary, MatrixTransition, MediaQuery, MediaQueryData,
        MemoryRouteInformationProvider, MouseCursor, MouseRegion, MouseRegionCallbacks,
        NavigationBackButtonDispatcher, NavigationNotification, NavigationNotificationKind,
        NavigationNotificationListener, NavigationNotificationSubscription, NavigationToolbar,
        NavigatorPopHandler, NavigatorPopHandlerController, NestedScrollView,
        NoopAuxiliaryViewHost, NoopWindowChromeSink, NotificationListener, Offstage, Opacity,
        OrderedTraversalPolicy, Orientation, OrientationBuilder, OverflowBar, OverflowBox,
        OverlayPortal, Padding, PageController, PageStorage, PageStorageBucket,
        PageStorageIdentifier, PageStorageKey, PageView, PerformanceOverlay, PinnedHeaderSliver,
        PopAttempt, PopScope, PopScopeController, Positioned, PositionedTransition,
        PrimaryScrollController, RadioGroup, Radius, RawAutocomplete, RawGestureDetector, RawImage,
        RawPointerEvent, RawRadio, RawScrollbar, RawScrollbarGeometry, RawScrollbarOrientation,
        RawScrollbarStyle, RawTooltip, RawTooltipBuilder, RawTooltipController,
        RawTooltipDurations, RawTooltipTriggerMode, RawTooltipVisibility,
        ReadingOrderTraversalPolicy, RelativePositionedTransition,
        ReorderableDelayedDragStartListener, ReorderableDragStartListener, ReorderableList,
        RepaintBoundary, RootBackButtonDispatcher, RootRestorationScope, RotatedBox,
        RotationTransition, RouteInformation, RouteInformationListener, RouteInformationParser,
        RouteInformationProvider, RouteInformationReportingType, RouteInformationSubscription,
        Router, RouterConfig, RouterData, RouterDelegate, RouterDelegateListener,
        RouterDelegateSubscription, RouterError, Row, SafeArea, ScaleTransition,
        ScrollConfiguration, ScrollController, ScrollMetrics, ScrollNotification,
        ScrollNotificationObserver, ScrollNotificationSubscription, ScrollNotificationType,
        ScrollPhysics, ScrollViewKeyboardDismissBehavior, SelectableRegion, Semantics,
        SensitiveContent, SensitiveContentHost, ShortcutActivator, ShortcutKey, ShortcutTrigger,
        Shortcuts, SingleActivator, SingleChildScrollView, SizeTransition, SizedBox,
        SizedOverflowBox, SlideTransition, Sliver, SliverAnimatedGrid,
        SliverAnimatedGridController, SliverAnimatedList, SliverAnimatedListController,
        SliverConstrainedCrossAxis, SliverConstraints, SliverCrossAxisExpanded,
        SliverCrossAxisGroup, SliverFillRemaining, SliverFillViewport, SliverFixedExtentList,
        SliverFloatingHeader, SliverGrid, SliverGridDelegate, SliverLayout, SliverLayoutBuilder,
        SliverList, SliverMainAxisGroup, SliverOffstage, SliverOpacity, SliverOverlapAbsorber,
        SliverOverlapHandle, SliverOverlapInjector, SliverPadding, SliverPersistentHeader,
        SliverPrototypeExtentList, SliverReorderController, SliverReorderableList,
        SliverResizingHeader, SliverSafeArea, SliverToBoxAdapter, SliverVariedExtentList,
        SliverVisibility, SnapshotWidget, Spacer, Stack, StringRouteInformationParser, Table,
        TableCell, TapRegion, TapRegionCallbacks, TapRegionGroupId, TapRegionSurface, Text,
        TextFieldTapRegion, TextInputActionHint, TextInputTypeHint, TickerMode, TileMode, Title,
        TitleController, TitleData, TitleError, TooltipComponentBuilder, TooltipDurations,
        TooltipTriggerMode, TooltipVisibility, Transform, TreeRowAnimation, TreeSliver,
        TreeSliverController, TreeSliverIndentation, TreeSliverNode, TreeSliverNodeId,
        TwoDimensionalChildDelegate, TwoDimensionalChildLayout, TwoDimensionalConstraints,
        TwoDimensionalScrollDelta, TwoDimensionalScrollView, TwoDimensionalScrollable,
        TwoDimensionalViewport, TwoDimensionalViewportLayout, UnconstrainedBox, UndoHistory,
        UnmanagedRestorationScope, View, ViewAnchor, ViewAnchorController, ViewAnchorData,
        ViewAnchorSubscription, ViewController, ViewData, ViewEvent, ViewId, ViewLifecycle,
        ViewMetrics, ViewSubscription, Visibility, WheelChildDelegate, WheelChildLayout,
        WheelLayout, WheelMatrix, WheelProjection, Widget, WidgetOrderTraversalPolicy, WidgetsApp,
        WidgetsAppController, WidgetsAppData, WindowChromeSink, WindowDragRegion,
        WindowResizeRegion, Wrap, normalize_route_location,
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
