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
#[doc(hidden)]
pub mod internal;
mod layout;
mod navigation_scopes;
mod painting_effects;
mod radio_selection;
mod recursion;
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
pub use drag_drop::{
    DismissDirection, Dismissible, DragDropContext, DragTarget, Draggable, LongPressDraggable,
};
pub use environment::{
    DefaultSelectionStyle, DefaultTextStyle, Directionality, IconTheme, Localizations, MediaQuery,
    MediaQueryData, Orientation, OrientationBuilder, PrimaryScrollController, ScrollConfiguration,
    SensitiveContent, TickerMode,
};
pub use focus_keyboard::{
    Action, ActionResult, Actions, Command, CommandId, ExcludeFocus, ExcludeFocusTraversal, Focus,
    FocusManager, FocusNode, FocusScope, FocusScopeNode, FocusScopeSubscription,
    FocusTraversalGroup, FocusTraversalOrder, FocusTraversalPolicy, FocusTraversalPolicyKind,
    Intent, KeyboardListener, LogicalShortcutKey, OrderedTraversalPolicy,
    ReadingOrderTraversalPolicy, ShortcutKey, ShortcutTrigger, Shortcuts,
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
    VerticalDirection, WrapAlignment, WrapCrossAlignment,
};
pub use incular_core::{Color, Key, LocalKey, Offset, Rect, Size, UniqueKey, ValueKey};
pub use incular_image::{AssetImage, FileImage, ImageConfiguration, ImageProvider, MemoryImage};
pub use incular_rendering::{
    BlendMode, Canvas, ColorFilter, FilterQuality, LinearGradient, Paint, PaintStyle, Path, RRect,
    RadialGradient, Shader, Shadow, SweepGradient,
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
pub use layout::{
    Align, AspectRatio, Baseline, Center, ClipOval, ClipPath, ClipRRect, ClipRect, ColoredBox,
    Column, ConstrainedBox, ConstraintsTransformBox, Container, CustomPaint, Expanded, FittedBox,
    Flex, Flexible, FractionalTranslation, FractionallySizedBox, IndexedStack, IntrinsicHeight,
    IntrinsicWidth, KeyedSubtree, LayoutBuilder, LimitedBox, NavigationToolbar, Offstage,
    OverflowBar, OverflowBox, Padding, Positioned, RepaintBoundary, RotatedBox, Row, SizedBox,
    SizedOverflowBox, Spacer, Stack, Table, TableCell, UnconstrainedBox, Visibility, Wrap,
};
pub use navigation_scopes::OverlayPortal;
pub use painting_effects::{
    Border, BorderDirectional, BorderRadius, BorderRadiusDirectional, BorderSide, BorderStyle,
    BoxBorder, BoxDecoration, BoxShadow, BoxShape, ClipRSuperellipse, CustomPainter,
    DecorationImage, GridPaper, ImageFiltered, ImageIcon, Radius, RawImage, SnapshotWidget,
    TileMode,
};
pub use radio_selection::{RadioGroup, RawRadio, SelectableRegion};
pub use safe_area::SafeArea;
pub use scrolling::{
    CustomScrollView, DecoratedSliver, GridView, ListBody, ListView, NestedScrollView,
    NotificationListener, PageController, PageView, PinnedHeaderSliver, ScrollNotificationObserver,
    Scrollable, SingleChildScrollView, Sliver, SliverAnimatedList, SliverAnimatedListController,
    SliverConstrainedCrossAxis, SliverCrossAxisExpanded, SliverCrossAxisGroup, SliverFillRemaining,
    SliverFillViewport, SliverFixedExtentList, SliverFloatingHeader, SliverGrid,
    SliverGridDelegate, SliverIgnorePointer, SliverLayout, SliverLayoutBuilder, SliverList,
    SliverMainAxisGroup, SliverOffstage, SliverOpacity, SliverOverlapAbsorber, SliverOverlapHandle,
    SliverOverlapInjector, SliverPadding, SliverPersistentHeader, SliverPrototypeExtentList,
    SliverReorderController, SliverReorderableList, SliverResizingHeader, SliverSafeArea,
    SliverToBoxAdapter, SliverVariedExtentList, SliverVisibility, Viewport,
};
pub use semantics::{BlockSemantics, ExcludeSemantics, MergeSemantics, Semantics};
pub use tree::{
    BoxFit, ColorFiltered, DecoratedBox, EditableText, FadeTransition, Icon, Image, ImageRepeat,
    Opacity, RotationTransition, ScaleTransition, SlideTransition, Text, TextFieldInputSnapshot,
    TextInputActionHint, TextInputTypeHint, Transform, Widget,
};
pub use utilities::{ErrorWidget, Expansible, PerformanceOverlay};

// Internal implementation aliases.  These names are deliberately
// `pub(crate)`: sibling crates use the documented `internal` bridge instead
// of observing retained-tree implementation types as Widgets API.
pub(crate) use gestures::GestureCallbacks;
pub(crate) use tree::{Blur, BlurController, DropShadow, ExplicitSemantics, WidgetKind};
