//! Declarative widgets and the retained element/render tree.
//!
//! The public crate root provides canonical first-class Flutter-style widget descriptors
//! while internal retained tree execution remains Rust-native.

mod animation_transitions;
#[cfg(feature = "devtools")]
#[doc(hidden)]
pub mod devtools;
#[cfg(feature = "devtools")]
pub(crate) mod devtools_props;
mod drag_drop;
mod environment;
mod focus_keyboard;
mod forms;
mod gestures;
mod hero;
mod implicit_animations;
#[doc(hidden)]
pub mod internal;
mod layout;
mod navigation_scopes;
mod painting_effects;
mod radio_selection;
mod reactive_builders;
mod safe_area;
mod scrolling;
mod selection;
mod semantics;
mod tree;
mod utilities;

// Re-export only the Flutter Widgets vocabulary.  Retained-tree IDs,
// renderer diagnostics, and Incular-only helpers stay private to this crate;
// the facade's `internal` module is the sole explicitly documented bridge for
// sibling implementation crates.
pub use animation_transitions::{
    AlignTransition, DecoratedBoxTransition, DefaultTextStyleTransition, DualTransitionBuilder,
    MatrixTransition, PositionedTransition, RelativePositionedTransition, SizeTransition,
};
pub use drag_drop::{DismissDirection, Dismissible, DragTarget, Draggable, LongPressDraggable};
pub use environment::{
    DefaultSelectionStyle, DefaultTextStyle, Directionality, IconTheme, Localizations,
    LookupBoundary, MediaQuery, MediaQueryData, Orientation, OrientationBuilder,
    PrimaryScrollController, ScrollConfiguration, SensitiveContent, SensitiveContentHost,
    TickerMode,
};
pub use focus_keyboard::{
    Action, ActionListener, ActionResult, Actions, CallbackShortcuts, Command, CommandId,
    ExcludeFocus, ExcludeFocusTraversal, Focus, FocusManager, FocusNode, FocusScope,
    FocusScopeNode, FocusScopeSubscription, FocusTraversalGroup, FocusTraversalOrder,
    FocusTraversalPolicy, FocusableActionDetector, Intent, KeyboardListener, LogicalShortcutKey,
    OrderedTraversalPolicy, ReadingOrderTraversalPolicy, ShortcutKey, ShortcutTrigger, Shortcuts,
    WidgetOrderTraversalPolicy,
};
pub use forms::{
    AutocompleteHighlightedOption, AutofillGroup, AutovalidateMode, FilteringTextInputFormatter,
    Form, FormController, FormField, FormFieldState, FormState, LengthLimitingTextInputFormatter,
    MaxLengthEnforcement, RawAutocomplete, TextInputFormatter, UndoHistory,
};
pub use gestures::{
    AbsorbPointer, GestureDetector, HitTestBehavior, IgnorePointer, Listener, MouseRegion,
    RawGestureDetector, ReorderableDelayedDragStartListener, ReorderableDragStartListener,
    ReorderableList, TapRegion, TapRegionSurface, TextFieldTapRegion,
};
pub use hero::{Hero, HeroControllerScope, HeroMode};
pub use implicit_animations::{
    AnimatedAlign, AnimatedContainer, AnimatedCrossFade, AnimatedDefaultTextStyle,
    AnimatedFractionallySizedBox, AnimatedOpacity, AnimatedPadding, AnimatedPhysicalModel,
    AnimatedPositioned, AnimatedPositionedDirectional, AnimatedRotation, AnimatedScale,
    AnimatedSize, AnimatedSlide, AnimatedSwitcher,
};
pub use incular_animation::{
    Animatable, Animation, AnimationController, AnimationStatus, Curve, Curves, Tween,
};
pub use incular_config::{
    Alignment, AlignmentDirectional, Axis, AxisDirection, Brightness, Clip, CrossAxisAlignment,
    FlexFit, FractionalOffset, MainAxisAlignment, MainAxisSize, StackFit, TextDirection,
    VerticalDirection, WrapAlignment, WrapCrossAlignment,
};
pub use incular_core::{Color, Key, LocalKey, Offset, Rect, Size, UniqueKey, ValueKey};
pub use incular_image::{AssetImage, FileImage, ImageConfiguration, ImageProvider, MemoryImage};
pub use incular_rendering::{
    BlendMode, Canvas, ColorFilter, FilterQuality, LinearGradient, Paint, PaintStyle, Path, RRect,
    RadialGradient, Shader, Shadow, SweepGradient,
};
pub use incular_scroll::{
    ClampingScrollPhysics, ScrollController, ScrollMetrics, ScrollPhysics,
    ScrollViewKeyboardDismissBehavior,
};
pub use incular_text::{
    FontFeature, FontVariation, FontWeight, IconData, RichText, StrutStyle, TextAlign,
    TextBaseline, TextEditingController, TextEditingValue, TextHeightBehavior, TextOverflow,
    TextRange, TextScaler, TextSelection, TextSpan, TextStyle, TextWidthBasis, WidgetSpan,
};
pub use layout::{
    Align, AspectRatio, Baseline, Center, ClipOval, ClipPath, ClipRRect, ClipRect, ColoredBox,
    Column, ConstrainedBox, ConstraintsTransformBox, Container, CustomPaint, Expanded, FittedBox,
    Flex, Flexible, FractionalTranslation, FractionallySizedBox, IndexedStack, IntrinsicHeight,
    IntrinsicWidth, KeyedSubtree, LayoutBuilder, LimitedBox, NavigationToolbar, Offstage,
    OverflowBar, OverflowBox, Padding, Positioned, RepaintBoundary, RotatedBox, Row, SizedBox,
    SizedOverflowBox, Spacer, Stack, Table, TableCell, UnconstrainedBox, Visibility, Wrap,
};
pub use navigation_scopes::{
    AnimatedModalBarrier, BackButtonListener, NavigatorPopHandler, OverlayPortal, PageStorage,
    PlatformMenuBar, PopScope, RootRestorationScope, Router, Title, UnmanagedRestorationScope,
    View, ViewAnchor, WidgetsApp,
};
pub use painting_effects::{
    AnnotatedRegion, Border, BorderDirectional, BorderRadius, BorderRadiusDirectional, BorderSide,
    BorderStyle, BoxBorder, BoxDecoration, BoxShadow, BoxShape, ClipRSuperellipse, CustomPainter,
    DecorationImage, GridPaper, ImageFiltered, ImageIcon, Radius, RawImage, ShaderMask,
    SnapshotWidget, TileMode,
};
pub use radio_selection::{
    RadioGroup, RawRadio, SelectableRegion, SelectionContainer, SelectionListener,
};
pub use reactive_builders::{AnimatedBuilder, RepeatingAnimationBuilder, TweenAnimationBuilder};
pub use safe_area::SafeArea;
pub use scrolling::{
    AnimatedGrid, AnimatedList, CustomScrollView, DecoratedSliver, DraggableScrollableActuator,
    DraggableScrollableSheet, GridView, ListBody, ListView, ListWheelScrollView, NestedScrollView,
    NotificationListener, PageController, PageView, PinnedHeaderSliver, RawScrollbar,
    ScrollNotificationObserver, Scrollable, SingleChildScrollView, Sliver, SliverAnimatedGrid,
    SliverAnimatedList, SliverConstrainedCrossAxis, SliverCrossAxisExpanded, SliverCrossAxisGroup,
    SliverFillRemaining, SliverFillViewport, SliverFixedExtentList, SliverFloatingHeader,
    SliverGrid, SliverIgnorePointer, SliverLayoutBuilder, SliverList, SliverMainAxisGroup,
    SliverOffstage, SliverOpacity, SliverOverlapAbsorber, SliverOverlapInjector, SliverPadding,
    SliverPersistentHeader, SliverPrototypeExtentList, SliverReorderableList, SliverResizingHeader,
    SliverSafeArea, SliverToBoxAdapter, SliverVariedExtentList, SliverVisibility, TreeSliver,
    TwoDimensionalScrollView, TwoDimensionalScrollable, TwoDimensionalViewport, Viewport,
};
pub use semantics::{BlockSemantics, ExcludeSemantics, MergeSemantics, Semantics};
pub use tree::{
    BoxFit, ColorFiltered, DecoratedBox, EditableText, FadeTransition, Icon, Image, ImageRepeat,
    Opacity, RotationTransition, ScaleTransition, SlideTransition, Text, Transform, Widget,
};
pub use utilities::{
    AutomaticKeepAlive, Banner, CheckedModeBanner, ErrorWidget, Expansible, IndexedSemantics,
    KeepAlive, PerformanceOverlay, RawTooltip, SemanticsDebugger,
};

// Internal implementation aliases.  These names are deliberately
// `pub(crate)`: sibling crates use the documented `internal` bridge instead
// of observing retained-tree implementation types as Widgets API.
pub(crate) use gestures::GestureCallbacks;
pub(crate) use tree::{
    Blur, BlurController, DropShadow, ExplicitSemantics, VirtualList, WidgetKind,
};
