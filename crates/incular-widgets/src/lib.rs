//! Declarative widgets and the retained element/render tree.
//!
//! The public crate root provides canonical first-class Flutter-style widget descriptors
//! while internal retained tree execution remains Rust-native.
//!
//! Author UI with concrete widget types and erase to [`Widget`] only at composition or
//! framework boundaries:
//!
//! ```
//! use incular_widgets::{Column, Padding, Text, Widget};
//!
//! let screen: Widget = Padding::all(
//!     16.0,
//!     Column::new([
//!         Widget::from(Text::new("Title")),
//!         Widget::from(Text::new("Body")),
//!     ]),
//! )
//! .into();
//!
//! assert_eq!(screen.debug_type_name(), "Padding");
//! ```
//!
//! `Widget` is intentionally an opaque transport value. Application-specific widgets should
//! normally be ordinary Rust components/functions that compose these public descriptors.
//! Custom drawing uses [`CustomPaint`] / [`CustomPainter`]. The retained built-in taxonomy is
//! sealed; Incular does not expose `WidgetKind` or a public trait-object render protocol.

mod advanced_scrolling;
mod advanced_slivers;
mod animation_transitions;
mod app_shell;
mod compositing;
#[cfg(feature = "devtools")]
#[doc(hidden)]
pub mod devtools;
#[cfg(feature = "devtools")]
pub(crate) mod devtools_props;
mod drag_drop;
mod environment;
pub mod extensions;
mod external_drop;
mod focus_keyboard;
mod forms;
mod gestures;
mod indexed_semantics;
#[doc(hidden)]
pub mod internal;
mod layout;
mod navigation;
mod navigation_scopes;
mod painting_effects;
mod platform_widgets;
mod radio_selection;
mod raw_input;
mod raw_tooltip;
mod recursion;
mod render_object;
mod safe_area;
mod scrolling;
mod selection;
mod selection_container;
mod selection_listener;
mod semantics;
mod semantics_debugger;
mod transient;
mod tree;
mod utilities;

// Re-export only the Flutter Widgets vocabulary.  Retained-tree IDs,
// renderer diagnostics, and Incular-only helpers stay private to this crate;
// the facade's `internal` module is the sole explicitly documented bridge for
// sibling implementation crates.
pub use advanced_scrolling::{
    CacheExtentStyle, ChangeReportingBehavior, ChildVicinity, DiagonalDragBehavior,
    DraggableNotificationSubscription, DraggableScrollableActuator, DraggableScrollableController,
    DraggableScrollableNotification, DraggableScrollableSheet, DraggableScrollableState,
    DraggableSheetDelta, DraggableSheetExtent, DraggableSizeAnimation, DraggableSnap,
    DraggableSnapTarget, FixedExtentScrollController, ListWheelScrollView, ListWheelViewport,
    RawScrollbar, RawScrollbarGeometry, RawScrollbarOrientation, RawScrollbarStyle,
    TwoDimensionalChildDelegate, TwoDimensionalChildLayout, TwoDimensionalConstraints,
    TwoDimensionalScrollDelta, TwoDimensionalScrollView, TwoDimensionalScrollable,
    TwoDimensionalViewport, TwoDimensionalViewportLayout, WheelChildDelegate, WheelChildLayout,
    WheelLayout, WheelMatrix, WheelProjection,
};
pub use advanced_slivers::{
    AnimatedGrid, AnimatedGridController, AnimatedItem, AnimatedItemBuilder, AnimatedItemPhase,
    AnimatedList, AnimatedListController, AnimatedRemovedItemBuilder, AutomaticKeepAlive,
    KeepAlive, KeepAliveHandle, KeepAliveNotification, KeepAliveRegistry, SliverAnimatedGrid,
    SliverAnimatedGridController, TreeRowAnimation, TreeSliver, TreeSliverController,
    TreeSliverIndentation, TreeSliverNode, TreeSliverNodeId,
};
pub use animation_transitions::{
    AlignTransition, DecoratedBoxTransition, DefaultTextStyleTransition, DualTransitionBuilder,
    MatrixTransition, PositionedTransition, RelativePositionedTransition, SizeTransition,
};
pub use app_shell::{
    ApplicationBootstrapHost, ApplicationBootstrapOptions, ApplicationBootstrapSpec,
    AuxiliaryViewError, AuxiliaryViewHandle, AuxiliaryViewHost, AuxiliaryViewOutcome,
    AuxiliaryViewRequest, BackButtonDispatcher, BackCallbackSubscription, BasicRouterDelegate,
    ClosureRouteInformationParser, MemoryRouteInformationProvider, NavigationNotification,
    NavigationNotificationKind, NavigationNotificationListener, NavigationNotificationSubscription,
    NoopAuxiliaryViewHost, NoopWindowChromeSink, RootBackButtonDispatcher, RouteInformation,
    RouteInformationListener, RouteInformationParser, RouteInformationProvider,
    RouteInformationReportingType, RouteInformationSubscription, Router, RouterConfig, RouterData,
    RouterDelegate, RouterDelegateListener, RouterDelegateSubscription, RouterError,
    StringRouteInformationParser, Title, TitleController, TitleData, TitleError, View, ViewAnchor,
    ViewAnchorController, ViewAnchorData, ViewAnchorSubscription, ViewController, ViewData,
    ViewEvent, ViewId, ViewLifecycle, ViewMetrics, ViewSubscription, WidgetsApp,
    WidgetsAppController, WidgetsAppData, WindowChromeSink, WindowDragRegion, WindowResizeRegion,
    normalize_route_location,
};
pub use compositing::{
    AnnotatedRegion, BackdropFilter, CompositedTransformFollower, CompositedTransformTarget,
    ShaderCallback, ShaderMask,
};
pub use drag_drop::{
    DismissDirection, Dismissible, DragDropContext, DragTarget, Draggable, LongPressDraggable,
};
pub use environment::{
    ContentSensitivity, DefaultSelectionStyle, DefaultTextStyle, Directionality, IconTheme,
    Localizations, LookupBoundary, MediaQuery, MediaQueryData, Orientation, OrientationBuilder,
    PrimaryScrollController, ScrollConfiguration, SensitiveContent, SensitiveContentHost,
    TickerMode,
};
pub use focus_keyboard::{
    Action, ActionInvocationPhase, ActionListener, ActionResult, Actions, CallbackShortcuts,
    Command, CommandId, ExcludeFocus, ExcludeFocusTraversal, Focus, FocusManager, FocusNode,
    FocusScope, FocusScopeNode, FocusScopeSubscription, FocusTraversalGroup, FocusTraversalOrder,
    FocusTraversalPolicy, FocusTraversalPolicyKind, FocusableActionDetector, Intent,
    KeyboardListener, LogicalShortcutKey, OrderedTraversalPolicy, ReadingOrderTraversalPolicy,
    ShortcutActivator, ShortcutKey, ShortcutTrigger, Shortcuts, SingleActivator,
    WidgetOrderTraversalPolicy,
};
pub use forms::{
    AutocompleteHighlightedOption, AutofillGroup, AutovalidateMode, FilteringTextInputFormatter,
    Form, FormController, FormField, FormFieldState, FormState, LengthLimitingTextInputFormatter,
    MaxLengthEnforcement, RawAutocomplete, TextInputFormatter, UndoHistory,
};
pub use gestures::{AbsorbPointer, GestureDetector, HitTestBehavior, IgnorePointer};
pub use incular_animation::{
    Animatable, Animation, AnimationController, AnimationStatus, Curve, Curves, Tween,
};
pub use incular_config::{
    Alignment, AlignmentDirectional, Axis, AxisDirection, Brightness, Clip, CrossAxisAlignment,
    FlexFit, FractionalOffset, MainAxisAlignment, MainAxisSize, StackFit, TextDirection,
    TransientPresentation, TransientRole, VerticalDirection, WrapAlignment, WrapCrossAlignment,
};
pub use incular_core::{Color, Key, LocalKey, Offset, Rect, Size, UniqueKey, ValueKey};
pub use incular_image::{AssetImage, FileImage, ImageConfiguration, ImageProvider, MemoryImage};
pub use incular_rendering::{
    Annotation, BlendMode, Canvas, ColorFilter, FilterQuality, LayerAnchor, LayerLink,
    LinearGradient, Paint, PaintStyle, Path, RRect, RadialGradient, Shader, Shadow, SweepGradient,
};
pub use incular_scroll::{
    ClampingScrollPhysics, ScrollController, ScrollMetrics, ScrollNotification,
    ScrollNotificationSubscription, ScrollNotificationType, ScrollPhysics,
    ScrollViewKeyboardDismissBehavior, SliverConstraints,
};
pub use incular_text::{
    FontFeature, FontVariation, FontWeight, IconData, RichText, StrutStyle, TextAlign,
    TextBaseline, TextEditingController, TextEditingValue, TextHeightBehavior, TextOverflow,
    TextRange, TextScaler, TextSelection, TextSpan, TextStyle, TextWidthBasis, WidgetSpan,
};
pub use indexed_semantics::IndexedSemantics;
pub use layout::{
    Align, AspectRatio, Baseline, Center, ClipOval, ClipPath, ClipRRect, ClipRect, ColoredBox,
    Column, ConstrainedBox, ConstraintsTransformBox, Container, CustomPaint, Expanded, FittedBox,
    Flex, Flexible, FractionalTranslation, FractionallySizedBox, IndexedStack, IntrinsicHeight,
    IntrinsicWidth, KeyedSubtree, LayoutBuilder, LimitedBox, NavigationToolbar, Offstage,
    OverflowBar, OverflowBox, Padding, Positioned, RepaintBoundary, RotatedBox, Row, SizedBox,
    SizedOverflowBox, Spacer, Stack, Table, TableCell, UnconstrainedBox, Visibility, Wrap,
};
/// The navigation-scope dispatcher is kept under an explicit name so a future
/// app-shell `BackButtonDispatcher` can coexist in this facade without a
/// duplicate public binding.
pub use navigation::BackButtonDispatcher as NavigationBackButtonDispatcher;
pub use navigation::{
    AnimatedModalBarrier, BackButtonListener, BackDispatchReport, BackHandlerResult,
    BackRegistration, NavigatorPopHandler, NavigatorPopHandlerController, PageStorage,
    PageStorageBucket, PageStorageIdentifier, PageStorageKey, PopAttempt, PopScope,
    PopScopeController, RootRestorationScope, UnmanagedRestorationScope,
    current_page_storage_bucket, current_restoration_scope,
};
pub use navigation_scopes::OverlayPortal;
pub use painting_effects::{
    Border, BorderDirectional, BorderRadius, BorderRadiusDirectional, BorderSide, BorderStyle,
    BoxBorder, BoxDecoration, BoxShadow, BoxShape, ClipRSuperellipse, CustomPainter,
    DecorationImage, GridPaper, ImageFiltered, ImageIcon, Radius, RawImage, SnapshotWidget,
    TileMode,
};
pub use platform_widgets::{
    MenuDispatchResult, MenuItemId, MenuOwnerId, NoopPlatformMenuDelegate, PlatformMenu,
    PlatformMenuBar, PlatformMenuBarController, PlatformMenuBuildError, PlatformMenuDelegate,
    PlatformMenuEntry, PlatformMenuEvent, PlatformMenuItem, PlatformMenuItemGroup,
    PlatformMenuShortcut, PlatformMenuSnapshot, PlatformMenuSnapshotNode, PlatformMenuUpdate,
    ShortcutModifiers,
};
pub use radio_selection::{RadioGroup, RawRadio, SelectableRegion};
pub use raw_input::{
    ErasedGestureRecognizerFactory, GestureRecognizer, GestureRecognizerFactory,
    GestureRecognizerFactoryError, GestureRecognizerFactoryWithHandlers, Listener,
    ListenerCallbacks, MouseCursor, MouseRegion, MouseRegionCallbacks, PointerDeviceKind,
    RawGestureDetector, RawPointerEvent, TapRegion, TapRegionCallbacks, TapRegionGroupId,
    TapRegionSurface, TextFieldTapRegion,
};
pub use raw_tooltip::{
    RawTooltip, RawTooltipBuilder, RawTooltipController, RawTooltipDurations,
    RawTooltipTriggerMode, RawTooltipVisibility, TooltipComponentBuilder, TooltipDurations,
    TooltipTriggerMode, TooltipVisibility,
};
pub use safe_area::SafeArea;
pub use scrolling::{
    CustomScrollView, DecoratedSliver, GridView, ListBody, ListView, NestedScrollView,
    NotificationListener, PageController, PageView, PinnedHeaderSliver,
    ReorderableDelayedDragStartListener, ReorderableDragStartListener, ReorderableList,
    ScrollNotificationObserver, Scrollable, SingleChildScrollView, Sliver, SliverAnimatedList,
    SliverAnimatedListController, SliverConstrainedCrossAxis, SliverCrossAxisExpanded,
    SliverCrossAxisGroup, SliverFillRemaining, SliverFillViewport, SliverFixedExtentList,
    SliverFloatingHeader, SliverGrid, SliverGridDelegate, SliverHeaderOverscrollBehavior,
    SliverHeaderScrollBehavior, SliverIgnorePointer, SliverLayout, SliverLayoutBuilder, SliverList,
    SliverMainAxisGroup, SliverNaturalHeader, SliverOffstage, SliverOpacity, SliverOverlapAbsorber,
    SliverOverlapHandle, SliverOverlapInjector, SliverPadding, SliverPersistentHeader,
    SliverPrototypeExtentList, SliverReorderController, SliverReorderableList,
    SliverResizingHeader, SliverSafeArea, SliverToBoxAdapter, SliverVariedExtentList,
    SliverVisibility, Viewport,
};
pub use selection::{
    SelectableChildPolicy, SelectedContent, SelectedContentRange, SelectionAreaController,
    SelectionContainerDelegate, SelectionDetails, SelectionGeometry, SelectionHandleType,
    SelectionListenerNotifier, SelectionPoint, SelectionStatus,
};
pub use selection_container::SelectionContainer;
pub use selection_listener::SelectionListener;
pub use semantics::{BlockSemantics, ExcludeSemantics, MergeSemantics, Semantics};
pub use semantics_debugger::{DEFAULT_SEMANTICS_DEBUGGER_NODE_LIMIT, SemanticsDebugger};
pub use transient::{
    TransientAlignment, TransientDismissPolicy, TransientDismissReason, TransientPlacement,
    TransientPlacementInput, TransientPlacementMode, TransientPlacementResult, TransientSide,
    TransientSurfaceId, TransientSurfaceSnapshot, place_transient,
};
pub use tree::{
    BoxFit, BuildContext, ColorFiltered, DecoratedBox, EditableText, FadeTransition, Icon, Image,
    ImageRepeat, Opacity, RotationTransition, ScaleTransition, SlideTransition, Text,
    TextFieldInputSnapshot, TextInputActionHint, TextInputTypeHint, Transform, Widget,
};
pub use utilities::{
    Banner, BannerLocation, CheckedModeBanner, ErrorWidget, Expansible, PerformanceOverlay,
};

// Internal implementation aliases.  These names are deliberately
// `pub(crate)`: sibling crates use the documented `internal` bridge instead
// of observing retained-tree implementation types as Widgets API.
pub(crate) use gestures::GestureCallbacks;
pub(crate) use tree::{Blur, BlurController, DropShadow, ExplicitSemantics, WidgetKind};
