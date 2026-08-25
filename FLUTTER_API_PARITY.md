# Incular Flutter Widget & Public API Parity Specification
This document details the public widget architecture and API parity between
**Flutter Core** (`package:flutter/widgets.dart`), **Flutter Material**
(`package:flutter/material.dart`), and **Incular**. Core primitives and
Material components are intentionally tracked separately.
Every canonical Flutter core widget is explicitly tracked in [`specs/flutter_api_parity.jsonl`](file:///home/fanisus/CODEBASE/incular/specs/flutter_api_parity.jsonl) with an authoritative status (`IMPLEMENTED`, `MERGED`, `DEFERRED`, `SKIPPED`, `INTERNAL`). No silently omitted names.
---
## 1. Parity Status Overview
Total Flutter APIs audited: **336**
- **IMPLEMENTED**: 318
- **MERGED**: 11
- **INTERNAL**: 3
- **DEFERRED**: 3
- **SKIPPED**: 1

---
## 2. Architectural Principles
1. **Flutter-like Public Ergonomics**:
   - Dedicated first-class widget structs (`Row::new`, `Column::new`, `Container::new`, `AnimatedContainer::new`, `GestureDetector::new`, `ListView::builder`, `SliverList::new`, `CustomScrollView::new`).
   - Fluent builders for optional properties (`.alignment(...)`, `.padding(...)`, `.color(...)`, `.curve(...)`, `.duration(...)`).
   - Uniform child passing via `impl Into<Widget>` and `IntoIterator<Item = impl Into<Widget>>` / `children![]` macro.
   - First-class prelude exports for all public widget types.

2. **Rust-Native Retained Engine**:
   - **Value-type widget descriptions**: Lightweight descriptors without deep cloning.
   - **Retained tree reconciliation**: Compile into retained `Element` arena nodes, `RenderObject` layout records, and compositor `Layer` trees.
   - **Reactive state**: Fine-grained reactive `Signal` and `Memo` tracking without manual boilerplate.
   - **Renderer independence**: Drawing commands compile to agnostic display lists rendered via `wgpu` or platform backends.

---
## 3. Audited API Parity by Category

### Core & Framework Primitives (10)

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`Widget`** | Widget | `IMPLEMENTED` | `incular::prelude::Widget` | Erased retained widget description value implemented as a Rust enum/struct with Element and RenderObject lifecycle. |
| **`StatelessWidget / StatefulWidget`** | Widget with closures and Signal state | `MERGED` | `incular::prelude::{Widget,Signal}` | Rust function components and reactive Signals replace Dart class-based State inheritance. |
| **`BuildContext`** | BuildContext | `IMPLEMENTED` | `incular::runtime::BuildContext` | Provides ambient context, reactive signal tracking, and environment snapshots. |
| **`Key / ValueKey / UniqueKey`** | Key / ValueKey / UniqueKey | `IMPLEMENTED` | `incular::prelude::Key` | Stable local reconciliation identity with integer and string tagging. |
| **`GlobalKey`** | None | `SKIPPED` | *none* | Deliberately skipped to preserve pure local tree reconciliation without global element addressability. |
| **`Element / BuildOwner / BuildScope`** | retained element arena and runtime build scope | `INTERNAL` | *none* | Internal scheduling and arena IDs are not application API. |
| **`RenderObject / RenderBox / RenderSliver`** | retained render records and lazy viewport protocol | `INTERNAL` | *none* | Renderer implementation details remain hidden. |
| **`Layer subclasses`** | LayerTree and compositor layers | `INTERNAL` | *none* | Compositor nodes are renderer internals, not Flutter compatibility types. |
| **`InheritedWidget`** | Environment widgets and Signal scope | `MERGED` | `incular::prelude::Signal` | Merged into explicit environment widget descriptors and reactive Signals. |
| **`Notification`** | NotificationListener and bubbling protocol | `IMPLEMENTED` | `incular::prelude::NotificationListener` | Bubbles contextual events up through the ancestor tree hierarchy. |

### Layout, Box Sizing & Composition (43)

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`Row`** | Row | `IMPLEMENTED` | `incular::prelude::Row` | Horizontal flex layout with fluent builders for MainAxisAlignment, CrossAxisAlignment, MainAxisSize, spacing, TextDirection, and VerticalDirection. |
| **`Column`** | Column | `IMPLEMENTED` | `incular::prelude::Column` | Vertical flex layout with fluent builders for MainAxisAlignment, CrossAxisAlignment, MainAxisSize, spacing, TextDirection, and VerticalDirection. |
| **`Flex`** | Flex | `IMPLEMENTED` | `incular::prelude::Flex` | Direction-configurable flex layout container. |
| **`Flexible`** | Flexible | `IMPLEMENTED` | `incular::prelude::Flexible` | Allocates proportional share of flex main-axis with FlexFit::Loose or FlexFit::Tight. |
| **`Expanded`** | Expanded | `IMPLEMENTED` | `incular::prelude::Expanded` | Tight flexible child wrapper with configurable flex factor. |
| **`Spacer`** | Spacer | `IMPLEMENTED` | `incular::prelude::Spacer` | Expanded empty space on a flex axis. |
| **`Stack`** | Stack | `IMPLEMENTED` | `incular::prelude::Stack` | Overlays children in paint order with Alignment, StackFit (Loose, Expand, Passthrough), and Clip behavior. |
| **`Positioned`** | Positioned | `IMPLEMENTED` | `incular::prelude::Positioned` | Pins child to Stack edges (left, top, right, bottom) or explicit dimensions with Positioned::fill convenience. |
| **`PositionedDirectional`** | PositionedDirectional | `IMPLEMENTED` | `incular::prelude::PositionedDirectional` | Positions child relative to ambient text direction (start/end). |
| **`IndexedStack`** | IndexedStack | `IMPLEMENTED` | `incular::prelude::IndexedStack` | Maintains all children laid out while painting, hit testing, and exposing semantics for only the active index. |
| **`Wrap`** | Wrap | `IMPLEMENTED` | `incular::prelude::Wrap` | Multi-run packing layout supporting direction, spacing, run_spacing, alignment, run_alignment, and cross_axis_alignment. |
| **`Table`** | Table | `IMPLEMENTED` | `incular::prelude::Table` | Row-major max-content table layout with column and row spacing. |
| **`TableCell`** | TableCell | `IMPLEMENTED` | `incular::prelude::TableCell` | Specifies vertical alignment for a cell inside a Table. |
| **`Container`** | Container | `IMPLEMENTED` | `incular::prelude::Container` | Comprehensive composition wrapper combining margin, transform, alignment, constraints, decoration, padding, clipping, and child. |
| **`Padding`** | Padding | `IMPLEMENTED` | `incular::prelude::Padding` | Insets around a child with Padding::new, Padding::all, Padding::symmetric, Padding::only, and Padding::zero. |
| **`Align`** | Align | `IMPLEMENTED` | `incular::prelude::Align` | Positions a child with Alignment, width_factor, and height_factor. |
| **`Center`** | Center | `IMPLEMENTED` | `incular::prelude::Center` | Centers a child inside available box with optional width and height factors. |
| **`SizedBox`** | SizedBox | `IMPLEMENTED` | `incular::prelude::SizedBox` | Flexible dimension box with width, height, square, from_size, shrink, expand, and empty constructors. |
| **`SizedOverflowBox`** | SizedOverflowBox | `IMPLEMENTED` | `incular::prelude::SizedOverflowBox` | Imposes specific size on parent while allowing child to overflow with alignment. |
| **`ColoredBox`** | ColoredBox | `IMPLEMENTED` | `incular::prelude::ColoredBox` | Paints a solid background color behind an arbitrary child widget. |
| **`ConstrainedBox`** | ConstrainedBox | `IMPLEMENTED` | `incular::prelude::ConstrainedBox` | Imposes additional layout constraints on a child. |
| **`UnconstrainedBox`** | UnconstrainedBox | `IMPLEMENTED` | `incular::prelude::UnconstrainedBox` | Allows child to render at its natural unconstrained size with alignment and axis constraint removal. |
| **`LimitedBox`** | LimitedBox | `IMPLEMENTED` | `incular::prelude::LimitedBox` | Applies maximum dimensions only when incoming axes are unbounded. |
| **`OverflowBox`** | OverflowBox | `IMPLEMENTED` | `incular::prelude::OverflowBox` | Provides independent min/max constraints to child while parent observes parent bounds. |
| **`FractionallySizedBox`** | FractionallySizedBox | `IMPLEMENTED` | `incular::prelude::FractionallySizedBox` | Sizes child proportionally to parent available constraints with width_factor and height_factor. |
| **`FractionalTranslation`** | FractionalTranslation | `IMPLEMENTED` | `incular::prelude::FractionalTranslation` | Translates child by a fraction of its own measured layout size. |
| **`AspectRatio`** | AspectRatio | `IMPLEMENTED` | `incular::prelude::AspectRatio` | Enforces a strict width-to-height aspect ratio against incoming parent constraints. |
| **`FittedBox`** | FittedBox | `IMPLEMENTED` | `incular::prelude::FittedBox` | Scales and positions child within parent bounds using BoxFit and Alignment. |
| **`Baseline`** | Baseline | `IMPLEMENTED` | `incular::prelude::Baseline` | Positions child such that its text baseline aligns to a specific vertical offset. |
| **`IgnoreBaseline`** | Baseline fallback | `MERGED` | `incular::prelude::Baseline` | Baseline metrics automatically fallback cleanly to natural box bottom when baseline is absent. |
| **`IntrinsicWidth`** | IntrinsicWidth | `IMPLEMENTED` | `incular::prelude::IntrinsicWidth` | Sizes child to its intrinsic natural width with optional step_width and step_height. |
| **`IntrinsicHeight`** | IntrinsicHeight | `IMPLEMENTED` | `incular::prelude::IntrinsicHeight` | Sizes child to its intrinsic natural height. |
| **`ConstraintsTransformBox`** | ConstraintsTransformBox | `IMPLEMENTED` | `incular::prelude::ConstraintsTransformBox` | Transforms incoming layout constraints before passing them to its child. |
| **`RotatedBox`** | RotatedBox | `IMPLEMENTED` | `incular::prelude::RotatedBox` | Rotates child by integral quarter turns (90 degree increments) affecting both layout dimensions and paint. |
| **`KeyedSubtree`** | KeyedSubtree | `IMPLEMENTED` | `incular::prelude::KeyedSubtree` | Attaches a Key to an existing widget subtree. |
| **`Placeholder`** | Placeholder | `IMPLEMENTED` | `incular::prelude::Placeholder` | Diagnostic placeholder box drawing fallback dimensions. |
| **`PreferredSize`** | PreferredSize | `IMPLEMENTED` | `incular::prelude::PreferredSize` | Announces preferred size metadata to ancestor app bars and flexible toolbars. |
| **`OverflowBar`** | OverflowBar | `IMPLEMENTED` | `incular::prelude::OverflowBar` | Lays children horizontally in a row, overflowing into a column when horizontal space is exceeded. |
| **`NavigationToolbar`** | NavigationToolbar | `IMPLEMENTED` | `incular::prelude::NavigationToolbar` | Positions leading, middle, and trailing widgets with middle alignment rules. |
| **`InteractiveViewer`** | InteractiveViewer | `IMPLEMENTED` | `incular::prelude::InteractiveViewer` | Supports 2D pan and zoom gestures over an arbitrary child subtree. |
| **`SafeArea`** | SafeArea | `IMPLEMENTED` | `incular::prelude::SafeArea` | Insets child by screen display cutouts and platform hardware intrusions. |
| **`Visibility`** | Visibility | `IMPLEMENTED` | `incular::prelude::Visibility` | Controls child visibility with maintain_state, maintain_animation, maintain_size, and maintain_semantics. |
| **`Offstage`** | Offstage | `IMPLEMENTED` | `incular::prelude::Offstage` | Lays child out without painting it or hit testing it when offstage. |

### Environment, Ambient Configuration & Theming (11)

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`Directionality`** | Directionality | `IMPLEMENTED` | `incular::prelude::Directionality` | Establishes ambient TextDirection (Ltr / Rtl) for descendant layout and text resolution. |
| **`DefaultTextStyle`** | DefaultTextStyle | `IMPLEMENTED` | `incular::prelude::DefaultTextStyle` | Injects ambient default TextStyle inherited by descendant Text widgets. |
| **`IconTheme`** | IconTheme | `IMPLEMENTED` | `incular::prelude::IconTheme` | Provides default color, size, and opacity styling for descendant Icon widgets. |
| **`OrientationBuilder`** | OrientationBuilder | `IMPLEMENTED` | `incular::prelude::OrientationBuilder` | Rebuilds subtree dynamically on Portrait vs Landscape orientation changes. |
| **`ScrollConfiguration`** | ScrollConfiguration | `IMPLEMENTED` | `incular::prelude::ScrollConfiguration` | Controls default scroll physics and overscroll glow/spring behaviors for descendant scroll views. |
| **`PrimaryScrollController`** | PrimaryScrollController | `IMPLEMENTED` | `incular::prelude::PrimaryScrollController` | Associates a default primary ScrollController with the subtree. |
| **`TickerMode`** | TickerMode | `IMPLEMENTED` | `incular::prelude::TickerMode` | Enables or disables animation ticking for inactive/offstage subtrees. |
| **`SensitiveContent`** | SensitiveContent | `IMPLEMENTED` | `incular::prelude::SensitiveContent` | Protects sensitive content from screen recording, window capture, and diagnostics. |
| **`SensitiveContentHost`** | SensitiveContentHost | `IMPLEMENTED` | `incular::prelude::SensitiveContentHost` | Boundary host coordinating sensitive content obscuration. |
| **`LookupBoundary`** | LookupBoundary | `IMPLEMENTED` | `incular::prelude::LookupBoundary` | Isolates inherited widget and theme lookups across module boundaries. |
| **`SharedAppData`** | Signal storage and ambient context | `MERGED` | `incular::prelude::Signal` | Merged into Incular's reactive Signal and environment state storage. |

### Painting, Effects, Clipping & Masking (21)

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`DecoratedBox`** | DecoratedBox | `IMPLEMENTED` | `incular::prelude::DecoratedBox` | Paints background, gradient, border, and shadows behind or in front of child. |
| **`CustomPaint`** | CustomPaint | `IMPLEMENTED` | `incular::prelude::CustomPaint` | Draws direct 2D vector commands on painter / foreground painter canvas layers. |
| **`Opacity`** | Opacity | `IMPLEMENTED` | `incular::prelude::Opacity` | Composites child subtree with alpha opacity factor. |
| **`ColorFiltered`** | ColorFiltered | `IMPLEMENTED` | `incular::prelude::ColorFiltered` | Applies blend mode or matrix color filter to child raster output. |
| **`RawImage`** | RawImage | `IMPLEMENTED` | `incular::prelude::RawImage` | Renders a raw raster image handle directly without asset pipeline caching. |
| **`ImageIcon`** | ImageIcon | `IMPLEMENTED` | `incular::prelude::ImageIcon` | Displays an icon from an image raster handle with icon sizing and tinting. |
| **`ImageFiltered`** | ImageFiltered | `IMPLEMENTED` | `incular::prelude::ImageFiltered` | Filters its child widget subtree with blur or color image filter. |
| **`ShaderMask`** | ShaderMask | `IMPLEMENTED` | `incular::prelude::ShaderMask` | Applies a shader / gradient mask over its child widget. |
| **`BackdropFilter`** | No public backdrop sampling widget | `DEFERRED` | *none* | Prerequisite: GPU backdrop sampling pass and intermediate compositor framebuffer copy. |
| **`AnnotatedRegion`** | AnnotatedRegion | `IMPLEMENTED` | `incular::prelude::AnnotatedRegion` | Injects system UI / status bar annotations into the tree hierarchy. |
| **`SnapshotWidget`** | SnapshotWidget | `IMPLEMENTED` | `incular::prelude::SnapshotWidget` | Renders its child into a retained raster snapshot for performance optimization. |
| **`ClipRect`** | ClipRect | `IMPLEMENTED` | `incular::prelude::ClipRect` | Clips child to its rectangular bounds. |
| **`ClipRRect`** | ClipRRect | `IMPLEMENTED` | `incular::prelude::ClipRRect` | Clips child with rounded corner radii. |
| **`ClipOval`** | ClipOval | `IMPLEMENTED` | `incular::prelude::ClipOval` | Clips child using an inscribed ellipse / circle. |
| **`ClipPath`** | ClipPath | `IMPLEMENTED` | `incular::prelude::ClipPath` | Clips child using an arbitrary 2D Bezier path. |
| **`ClipRSuperellipse`** | ClipRSuperellipse | `IMPLEMENTED` | `incular::prelude::ClipRSuperellipse` | Clips child using a rounded superellipse (squircle) shape. |
| **`PhysicalModel`** | PhysicalModel | `IMPLEMENTED` | `incular::prelude::PhysicalModel` | Physical layer model with elevation, shadow, and corner radius. |
| **`PhysicalShape`** | PhysicalShape | `IMPLEMENTED` | `incular::prelude::PhysicalShape` | Physical layer widget with custom path shape and elevation. |
| **`GridPaper`** | GridPaper | `IMPLEMENTED` | `incular::prelude::GridPaper` | Draws an engineering grid paper over its background. |
| **`Transform`** | Transform | `IMPLEMENTED` | `incular::prelude::Transform` | Applies affine translation, rotation, scale, and skew transformations with inverse pointer mapping. |
| **`RepaintBoundary`** | RepaintBoundary | `IMPLEMENTED` | `incular::prelude::RepaintBoundary` | Isolates painting display list caching to avoid invalidating ancestor rendering. |

### Animation Transitions (Explicit) (12)

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`FadeTransition`** | FadeTransition | `IMPLEMENTED` | `incular::prelude::FadeTransition` | Animates opacity of a child with an animation controller. |
| **`ScaleTransition`** | ScaleTransition | `IMPLEMENTED` | `incular::prelude::ScaleTransition` | Animates scale transformation of a child. |
| **`RotationTransition`** | RotationTransition | `IMPLEMENTED` | `incular::prelude::RotationTransition` | Animates rotation of a child around an anchor pivot. |
| **`SlideTransition`** | SlideTransition | `IMPLEMENTED` | `incular::prelude::SlideTransition` | Animates translation offset of a child. |
| **`AlignTransition`** | AlignTransition | `IMPLEMENTED` | `incular::prelude::AlignTransition` | Animates alignment position of child within parent bounds. |
| **`SizeTransition`** | SizeTransition | `IMPLEMENTED` | `incular::prelude::SizeTransition` | Animates size factor along horizontal or vertical axis. |
| **`PositionedTransition`** | PositionedTransition | `IMPLEMENTED` | `incular::prelude::PositionedTransition` | Animates Positioned rect child inside a Stack. |
| **`RelativePositionedTransition`** | RelativePositionedTransition | `IMPLEMENTED` | `incular::prelude::RelativePositionedTransition` | Animates relative positioned rect in a Stack. |
| **`DecoratedBoxTransition`** | DecoratedBoxTransition | `IMPLEMENTED` | `incular::prelude::DecoratedBoxTransition` | Animates Decoration of a DecoratedBox. |
| **`DefaultTextStyleTransition`** | DefaultTextStyleTransition | `IMPLEMENTED` | `incular::prelude::DefaultTextStyleTransition` | Animates default text styling across children. |
| **`MatrixTransition`** | MatrixTransition | `IMPLEMENTED` | `incular::prelude::MatrixTransition` | Animates an arbitrary 2D affine transformation matrix. |
| **`DualTransitionBuilder`** | DualTransitionBuilder | `IMPLEMENTED` | `incular::prelude::DualTransitionBuilder` | Composes forward and reverse animation transitions. |

### Implicit Animations (Animated*) (15)

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`AnimatedContainer`** | AnimatedContainer | `IMPLEMENTED` | `incular::prelude::AnimatedContainer` | Implicitly animates container size, color, padding, margin, and alignment changes. |
| **`AnimatedAlign`** | AnimatedAlign | `IMPLEMENTED` | `incular::prelude::AnimatedAlign` | Implicitly animates child alignment changes. |
| **`AnimatedPadding`** | AnimatedPadding | `IMPLEMENTED` | `incular::prelude::AnimatedPadding` | Implicitly animates edge insets changes. |
| **`AnimatedOpacity`** | AnimatedOpacity | `IMPLEMENTED` | `incular::prelude::AnimatedOpacity` | Implicitly animates opacity factor changes. |
| **`AnimatedPositioned`** | AnimatedPositioned | `IMPLEMENTED` | `incular::prelude::AnimatedPositioned` | Implicitly animates Positioned edge insets and sizes in a Stack. |
| **`AnimatedPositionedDirectional`** | AnimatedPositionedDirectional | `IMPLEMENTED` | `incular::prelude::AnimatedPositionedDirectional` | Implicitly animates directional positioned bounds (start/end). |
| **`AnimatedFractionallySizedBox`** | AnimatedFractionallySizedBox | `IMPLEMENTED` | `incular::prelude::AnimatedFractionallySizedBox` | Implicitly animates width and height fractional scale factors. |
| **`AnimatedRotation`** | AnimatedRotation | `IMPLEMENTED` | `incular::prelude::AnimatedRotation` | Implicitly animates turns rotation (1 turn = 360 degrees). |
| **`AnimatedScale`** | AnimatedScale | `IMPLEMENTED` | `incular::prelude::AnimatedScale` | Implicitly animates scale factor changes. |
| **`AnimatedSlide`** | AnimatedSlide | `IMPLEMENTED` | `incular::prelude::AnimatedSlide` | Implicitly animates translation offset shifts. |
| **`AnimatedSize`** | AnimatedSize | `IMPLEMENTED` | `incular::prelude::AnimatedSize` | Implicitly animates bounds when child changes layout size. |
| **`AnimatedDefaultTextStyle`** | AnimatedDefaultTextStyle | `IMPLEMENTED` | `incular::prelude::AnimatedDefaultTextStyle` | Implicitly animates text style property updates across descendants. |
| **`AnimatedPhysicalModel`** | AnimatedPhysicalModel | `IMPLEMENTED` | `incular::prelude::AnimatedPhysicalModel` | Implicitly animates elevation and color of physical model. |
| **`AnimatedCrossFade`** | AnimatedCrossFade | `IMPLEMENTED` | `incular::prelude::AnimatedCrossFade` | Implicitly cross-fades between two children. |
| **`AnimatedSwitcher`** | AnimatedSwitcher | `IMPLEMENTED` | `incular::prelude::AnimatedSwitcher` | Transitions smoothly when child identity changes. |

### Reactive Builders & State (9)

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`AnimatedBuilder`** | AnimatedBuilder | `IMPLEMENTED` | `incular::prelude::AnimatedBuilder` | Rebuilds subtree when an animation controller notifies. |
| **`ListenableBuilder`** | ListenableBuilder | `IMPLEMENTED` | `incular::prelude::ListenableBuilder` | General-purpose reactive builder rebuilding on listenable state changes. |
| **`ValueListenableBuilder`** | ValueListenableBuilder | `IMPLEMENTED` | `incular::prelude::ValueListenableBuilder` | Typed value builder reacting to typed state value changes. |
| **`FutureBuilder`** | FutureBuilder | `IMPLEMENTED` | `incular::prelude::FutureBuilder` | Asynchronous builder reacting to Future and async task snapshots. |
| **`StreamBuilder`** | StreamBuilder | `IMPLEMENTED` | `incular::prelude::StreamBuilder` | Asynchronous builder reacting to Stream and channel snapshots. |
| **`StatefulBuilder`** | StatefulBuilder | `IMPLEMENTED` | `incular::prelude::StatefulBuilder` | Provides an inline set_state closure to trigger local subtree rebuilds. |
| **`TweenAnimationBuilder`** | TweenAnimationBuilder | `IMPLEMENTED` | `incular::prelude::TweenAnimationBuilder` | Implicit tween interpolation builder without manual controller boilerplate. |
| **`RepeatingAnimationBuilder`** | RepeatingAnimationBuilder | `IMPLEMENTED` | `incular::prelude::RepeatingAnimationBuilder` | Continuously ticking repeating animation builder. |
| **`LayoutBuilder`** | LayoutBuilder | `IMPLEMENTED` | `incular::prelude::LayoutBuilder` | Defers child tree creation until incoming parent layout constraints are resolved. |

### Shared-Element Transitions & Hero (3)

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`Hero`** | Hero | `IMPLEMENTED` | `incular::prelude::Hero` | Marks child widget as candidate for shared-element route animations. |
| **`HeroMode`** | HeroMode | `IMPLEMENTED` | `incular::prelude::HeroMode` | Enables or disables Hero transitions for its subtree. |
| **`HeroControllerScope`** | HeroControllerScope | `IMPLEMENTED` | `incular::prelude::HeroControllerScope` | Scopes a hero animation controller. |

### Scrolling Coordinators & Viewports (20)

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`SingleChildScrollView`** | SingleChildScrollView | `IMPLEMENTED` | `incular::prelude::SingleChildScrollView` | Single-child scrollable viewport with scroll controller, physics, axis direction, and padding. |
| **`Scrollable`** | Scrollable | `IMPLEMENTED` | `incular::prelude::Scrollable` | Underlying scroll gesture and viewport coordinator. |
| **`NestedScrollView`** | NestedScrollView | `IMPLEMENTED` | `incular::prelude::NestedScrollView` | Coordinates outer and inner scroll views with sliver headers. |
| **`Viewport`** | Viewport | `IMPLEMENTED` | `incular::prelude::Viewport` | A viewport bounding visible slivers. |
| **`ShrinkWrappingViewport`** | ShrinkWrappingViewport | `IMPLEMENTED` | `incular::prelude::ShrinkWrappingViewport` | A shrink-wrapping viewport for constrained scroll containers. |
| **`RawScrollbar`** | RawScrollbar | `IMPLEMENTED` | `incular::prelude::RawScrollbar` | A configurable scrollbar widget. |
| **`ListBody`** | ListBody | `IMPLEMENTED` | `incular::prelude::ListBody` | A simple sequential layout along the main axis. |
| **`ListWheelScrollView`** | ListWheelScrollView | `IMPLEMENTED` | `incular::prelude::ListWheelScrollView` | A 3D cylindrical rotating wheel scroll list. |
| **`ListWheelViewport`** | ListWheelScrollView | `MERGED` | `incular::prelude::ListWheelScrollView` | Merged into high-level ListWheelScrollView. |
| **`DraggableScrollableSheet`** | DraggableScrollableSheet | `IMPLEMENTED` | `incular::prelude::DraggableScrollableSheet` | Draggable scrollable bottom sheet with min, initial, and max extents. |
| **`DraggableScrollableActuator`** | DraggableScrollableActuator | `IMPLEMENTED` | `incular::prelude::DraggableScrollableActuator` | Notifies and controls sheet extent of ancestor DraggableScrollableSheet. |
| **`NotificationListener`** | NotificationListener | `IMPLEMENTED` | `incular::prelude::NotificationListener` | Listens for notifications bubbling up the widget tree. |
| **`ScrollNotificationObserver`** | ScrollNotificationObserver | `IMPLEMENTED` | `incular::prelude::ScrollNotificationObserver` | Observes scroll notifications. |
| **`TwoDimensionalScrollable`** | TwoDimensionalScrollable | `IMPLEMENTED` | `incular::prelude::TwoDimensionalScrollable` | Bidirectional (2D) scrollable coordinator. |
| **`TwoDimensionalScrollView`** | TwoDimensionalScrollView | `IMPLEMENTED` | `incular::prelude::TwoDimensionalScrollView` | Bidirectional (2D) scrolling view. |
| **`TwoDimensionalViewport`** | TwoDimensionalViewport | `IMPLEMENTED` | `incular::prelude::TwoDimensionalViewport` | Bidirectional (2D) viewport. |
| **`ListView`** | ListView | `IMPLEMENTED` | `incular::prelude::ListView` | Linear scrollable list with builder, separated, and fixed extent constructors. |
| **`GridView`** | GridView | `IMPLEMENTED` | `incular::prelude::GridView` | 2D scrollable grid with fixed cross-axis count and max cross-axis extent. |
| **`PageView`** | PageView | `IMPLEMENTED` | `incular::prelude::PageView` | Paging scrollable view with page snapping and PageController. |
| **`CustomScrollView`** | CustomScrollView | `IMPLEMENTED` | `incular::prelude::CustomScrollView` | Unified sliver scroll container coordinating multiple sliver elements. |

### Slivers (Extents, Groups, Headers & Scrolling) (35)

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`SliverList`** | SliverList | `IMPLEMENTED` | `incular::prelude::SliverList` | Sliver list with fixed extent and variable extent items. |
| **`SliverGrid`** | SliverGrid | `IMPLEMENTED` | `incular::prelude::SliverGrid` | Sliver 2D grid with cross-axis count and spacing. |
| **`SliverPadding`** | SliverPadding | `IMPLEMENTED` | `incular::prelude::SliverPadding` | Insets padding around child sliver. |
| **`SliverAppBar`** | SliverAppBar | `IMPLEMENTED` | `incular::prelude::SliverAppBar` | Expandable and pinnable sliver app bar header. |
| **`SliverPersistentHeader`** | SliverPersistentHeader | `IMPLEMENTED` | `incular::prelude::SliverPersistentHeader` | Pinned or floating persistent header sliver. |
| **`SliverToBoxAdapter`** | SliverToBoxAdapter | `IMPLEMENTED` | `incular::prelude::SliverToBoxAdapter` | Adapts an arbitrary box widget into a sliver viewport. |
| **`SliverFixedExtentList`** | SliverFixedExtentList | `IMPLEMENTED` | `incular::prelude::SliverFixedExtentList` | Sliver list with fixed item extents. |
| **`SliverVariedExtentList`** | SliverVariedExtentList | `IMPLEMENTED` | `incular::prelude::SliverVariedExtentList` | Sliver list with variable item extents. |
| **`SliverPrototypeExtentList`** | SliverPrototypeExtentList | `IMPLEMENTED` | `incular::prelude::SliverPrototypeExtentList` | Sliver list taking extent from a prototype widget. |
| **`SliverFillRemaining`** | SliverFillRemaining | `IMPLEMENTED` | `incular::prelude::SliverFillRemaining` | Sliver filling remaining viewport space. |
| **`SliverFillViewport`** | SliverFillViewport | `IMPLEMENTED` | `incular::prelude::SliverFillViewport` | Sliver with children each filling the entire viewport. |
| **`SliverLayoutBuilder`** | SliverLayoutBuilder | `IMPLEMENTED` | `incular::prelude::SliverLayoutBuilder` | Sliver builder receiving scroll constraints. |
| **`SliverMainAxisGroup`** | SliverMainAxisGroup | `IMPLEMENTED` | `incular::prelude::SliverMainAxisGroup` | Groups multiple slivers along the main axis. |
| **`SliverCrossAxisGroup`** | SliverCrossAxisGroup | `IMPLEMENTED` | `incular::prelude::SliverCrossAxisGroup` | Groups multiple slivers across the cross axis. |
| **`SliverCrossAxisExpanded`** | SliverCrossAxisExpanded | `IMPLEMENTED` | `incular::prelude::SliverCrossAxisExpanded` | Expands a sliver across cross-axis group space. |
| **`SliverConstrainedCrossAxis`** | SliverConstrainedCrossAxis | `IMPLEMENTED` | `incular::prelude::SliverConstrainedCrossAxis` | Constrains the cross-axis dimension of a sliver. |
| **`DecoratedSliver`** | DecoratedSliver | `IMPLEMENTED` | `incular::prelude::DecoratedSliver` | Paints decoration behind a sliver. |
| **`SliverOpacity`** | SliverOpacity | `IMPLEMENTED` | `incular::prelude::SliverOpacity` | Sliver opacity wrapper. |
| **`SliverAnimatedOpacity`** | SliverOpacity | `MERGED` | `incular::prelude::SliverOpacity` | Merged into SliverOpacity. |
| **`SliverFadeTransition`** | SliverOpacity | `MERGED` | `incular::prelude::SliverOpacity` | Merged into SliverOpacity. |
| **`SliverOffstage`** | SliverOffstage | `IMPLEMENTED` | `incular::prelude::SliverOffstage` | Sliver offstage wrapper. |
| **`SliverIgnorePointer`** | SliverIgnorePointer | `IMPLEMENTED` | `incular::prelude::SliverIgnorePointer` | Sliver ignore pointer wrapper. |
| **`SliverSafeArea`** | SliverSafeArea | `IMPLEMENTED` | `incular::prelude::SliverSafeArea` | Sliver safe area insets wrapper. |
| **`SliverVisibility`** | SliverVisibility | `IMPLEMENTED` | `incular::prelude::SliverVisibility` | Sliver visibility wrapper. |
| **`PinnedHeaderSliver`** | PinnedHeaderSliver | `IMPLEMENTED` | `incular::prelude::PinnedHeaderSliver` | Pinned leading header sliver. |
| **`SliverFloatingHeader`** | SliverFloatingHeader | `IMPLEMENTED` | `incular::prelude::SliverFloatingHeader` | Floating header sliver. |
| **`SliverResizingHeader`** | SliverResizingHeader | `IMPLEMENTED` | `incular::prelude::SliverResizingHeader` | Resizing header sliver with min/max extent bounds. |
| **`SliverOverlapAbsorber`** | SliverOverlapAbsorber | `IMPLEMENTED` | `incular::prelude::SliverOverlapAbsorber` | Sliver overlap absorber for nested scroll view coordinators. |
| **`SliverOverlapInjector`** | SliverOverlapInjector | `IMPLEMENTED` | `incular::prelude::SliverOverlapInjector` | Sliver overlap injector for nested scroll view coordinators. |
| **`SliverReorderableList`** | SliverReorderableList | `IMPLEMENTED` | `incular::prelude::SliverReorderableList` | Reorderable sliver list. |
| **`TreeSliver`** | TreeSliver | `IMPLEMENTED` | `incular::prelude::TreeSliver` | Hierarchical tree sliver. |
| **`AnimatedList`** | AnimatedList | `IMPLEMENTED` | `incular::prelude::AnimatedList` | Animated list container. |
| **`AnimatedGrid`** | AnimatedGrid | `IMPLEMENTED` | `incular::prelude::AnimatedGrid` | Animated grid container. |
| **`SliverAnimatedList`** | SliverAnimatedList | `IMPLEMENTED` | `incular::prelude::SliverAnimatedList` | Animated list sliver. |
| **`SliverAnimatedGrid`** | SliverAnimatedGrid | `IMPLEMENTED` | `incular::prelude::SliverAnimatedGrid` | Animated grid sliver. |

### Gestures, Pointer Routing & Drag-Drop (16)

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`GestureDetector`** | GestureDetector | `IMPLEMENTED` | `incular::prelude::GestureDetector` | First-class gesture arena detector with on_tap, double_tap, long_press, pan, and scale callbacks. |
| **`Listener`** | Listener | `IMPLEMENTED` | `incular::prelude::Listener` | Calls raw pointer event callbacks (down, move, up, cancel) directly. |
| **`RawGestureDetector`** | RawGestureDetector | `IMPLEMENTED` | `incular::prelude::RawGestureDetector` | Creates a widget detecting gestures with custom gesture recognizers. |
| **`MouseRegion`** | MouseRegion | `IMPLEMENTED` | `incular::prelude::MouseRegion` | Tracks mouse enter, hover motion, and exit within bounding geometry. |
| **`IgnorePointer`** | IgnorePointer | `IMPLEMENTED` | `incular::prelude::IgnorePointer` | Makes subtree completely transparent to hit testing. |
| **`AbsorbPointer`** | AbsorbPointer | `IMPLEMENTED` | `incular::prelude::AbsorbPointer` | Absorbs pointer events preventing descendants and background siblings from receiving them. |
| **`TapRegion`** | TapRegion | `IMPLEMENTED` | `incular::prelude::TapRegion` | Detects taps inside and outside of its boundary for group coordination. |
| **`TapRegionSurface`** | TapRegionSurface | `IMPLEMENTED` | `incular::prelude::TapRegionSurface` | A surface coordinating tap region groups. |
| **`TextFieldTapRegion`** | TextFieldTapRegion | `IMPLEMENTED` | `incular::prelude::TextFieldTapRegion` | Tap region tailored for text field unfocus behavior. |
| **`Draggable`** | Draggable | `IMPLEMENTED` | `incular::prelude::Draggable` | Typed draggable payload widget with feedback and on_start/on_end callbacks. |
| **`LongPressDraggable`** | LongPressDraggable | `IMPLEMENTED` | `incular::prelude::LongPressDraggable` | Draggable widget initiating drag operations exclusively on long-press. |
| **`DragTarget`** | DragTarget | `IMPLEMENTED` | `incular::prelude::DragTarget` | Typed drop target receiving payloads with will_accept and on_accept callbacks. |
| **`Dismissible`** | Dismissible | `IMPLEMENTED` | `incular::prelude::Dismissible` | One-shot dismiss interaction invoking callbacks when drag threshold is crossed. |
| **`ReorderableList`** | ReorderableList | `IMPLEMENTED` | `incular::prelude::ReorderableList` | Reorderable list supporting item dragging and position reordering. |
| **`ReorderableDragStartListener`** | ReorderableDragStartListener | `IMPLEMENTED` | `incular::prelude::ReorderableDragStartListener` | Drag start listener for reorderable list items. |
| **`ReorderableDelayedDragStartListener`** | ReorderableDelayedDragStartListener | `IMPLEMENTED` | `incular::prelude::ReorderableDelayedDragStartListener` | Delayed drag start listener for reorderable list items (touch/long-press). |

### Focus, Keyboard & Shortcuts (13)

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`Focus`** | Focus | `IMPLEMENTED` | `incular::prelude::Focus` | Manages keyboard focus and autofocus for a subtree. |
| **`FocusScope`** | FocusScope | `IMPLEMENTED` | `incular::prelude::FocusScope` | Establishes a scoped focus tree traversal domain. |
| **`KeyboardListener`** | KeyboardListener | `IMPLEMENTED` | `incular::prelude::KeyboardListener` | Listens for raw keyboard events routed from platform input. |
| **`Shortcuts`** | Shortcuts | `IMPLEMENTED` | `incular::prelude::Shortcuts` | Maps physical keyboard key combinations to logical Intent/Action commands. |
| **`Actions`** | Actions | `IMPLEMENTED` | `incular::prelude::Actions` | Dispatches and handles intent commands. |
| **`ActionListener`** | ActionListener | `IMPLEMENTED` | `incular::prelude::ActionListener` | Listens for action invocations within its subtree. |
| **`CallbackShortcuts`** | CallbackShortcuts | `IMPLEMENTED` | `incular::prelude::CallbackShortcuts` | Defines key combination shortcut callbacks directly. |
| **`FocusableActionDetector`** | FocusableActionDetector | `IMPLEMENTED` | `incular::prelude::FocusableActionDetector` | Combines focus management, shortcut handling, action execution, and mouse region tracking. |
| **`FocusTraversalGroup`** | FocusTraversalGroup | `IMPLEMENTED` | `incular::prelude::FocusTraversalGroup` | Establishes a focus traversal policy group for its descendants. |
| **`FocusTraversalOrder`** | FocusTraversalOrder | `IMPLEMENTED` | `incular::prelude::FocusTraversalOrder` | Customizes the focus traversal ordering of a widget. |
| **`ExcludeFocus`** | ExcludeFocus | `IMPLEMENTED` | `incular::prelude::ExcludeFocus` | Excludes a subtree from receiving focus. |
| **`ExcludeFocusTraversal`** | ExcludeFocusTraversal | `IMPLEMENTED` | `incular::prelude::ExcludeFocusTraversal` | Excludes a subtree from tab traversal while still allowing direct programmatic focus. |
| **`ShortcutRegistrar`** | Shortcuts | `MERGED` | `incular::prelude::Shortcuts` | Merged into Shortcuts registry. |

### Selection & Radio (7)

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`SelectionArea`** | SelectionArea | `IMPLEMENTED` | `incular::material::SelectionArea` | Material selection wrapper over the core SelectionAreaController. |
| **`SelectableText`** | SelectableText | `IMPLEMENTED` | `incular::material::SelectableText` | Material selectable read-only text wrapper over the core selection primitive. |
| **`SelectableRegion`** | SelectableRegion | `IMPLEMENTED` | `incular::prelude::SelectableRegion` | An area that supports pointer-based text selection. |
| **`SelectionContainer`** | SelectionContainer | `IMPLEMENTED` | `incular::prelude::SelectionContainer` | A container that hosts and manages selectable content. |
| **`SelectionListener`** | SelectionListener | `IMPLEMENTED` | `incular::prelude::SelectionListener` | Listens for selection geometry and state changes. |
| **`RadioGroup`** | RadioGroup | `IMPLEMENTED` | `incular::prelude::RadioGroup` | Coordinates mutually exclusive selection for a group of radio buttons. |
| **`RawRadio`** | RawRadio | `IMPLEMENTED` | `incular::prelude::RawRadio` | A raw radio button control. |

### Forms & Text Input (10)

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`Form`** | Form | `IMPLEMENTED` | `incular::prelude::Form` | Form container coordinating field registration, validation, saving, and reset. |
| **`TextFormField`** | TextFormField | `IMPLEMENTED` | `incular::material::TextFormField` | Material convenience field composed with the core Form/FormField model. |
| **`FormField`** | GenericFormField | `IMPLEMENTED` | `incular::prelude::GenericFormField` | Generic FormField widget managing arbitrary typed form state and validation. |
| **`EditableText`** | EditableText | `IMPLEMENTED` | `incular::prelude::EditableText` | Renderer-neutral retained editor with cursor, IME, selection, and multiline mode. |
| **`TextField`** | TextField | `IMPLEMENTED` | `incular::material::TextField` | Material text input wrapper; multiline editing uses `max_lines` rather than a separate TextArea class. |
| **`Autocomplete`** | Autocomplete | `IMPLEMENTED` | `incular::material::Autocomplete` | Filters options based on text input query with keyboard selection. |
| **`RawAutocomplete`** | RawAutocomplete | `IMPLEMENTED` | `incular::prelude::RawAutocomplete` | Core autocomplete widget coordinating text input with options view. |
| **`AutocompleteHighlightedOption`** | AutocompleteHighlightedOption | `IMPLEMENTED` | `incular::prelude::AutocompleteHighlightedOption` | Highlights currently focused option in autocomplete view. |
| **`AutofillGroup`** | AutofillGroup | `IMPLEMENTED` | `incular::prelude::AutofillGroup` | Coordinates autofill context across text input descendants. |
| **`UndoHistory`** | UndoHistory | `IMPLEMENTED` | `incular::prelude::UndoHistory` | Manages undo and redo history for editable text input. |

### Navigation, Overlays, Restoration & Multi-View (16)

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`Navigator`** | Navigator / NavigatorState | `IMPLEMENTED` | `incular::navigation::Navigator` | Imperative and declarative route stack with push, pop, replace, and transition delegates. |
| **`Router`** | Router | `IMPLEMENTED` | `incular::prelude::Router` | Declarative router managing top-level page routes. |
| **`PopScope`** | PopScope | `IMPLEMENTED` | `incular::prelude::PopScope` | Intercepts back navigation and page pop requests. |
| **`NavigatorPopHandler`** | NavigatorPopHandler | `IMPLEMENTED` | `incular::prelude::NavigatorPopHandler` | Handles navigator pop gestures and requests. |
| **`BackButtonListener`** | BackButtonListener | `IMPLEMENTED` | `incular::prelude::BackButtonListener` | Listens for platform hardware / system back button presses. |
| **`Overlay`** | Overlay | `IMPLEMENTED` | `incular::navigation::Overlay` | Global floating stack managing overlays, popups, and dropdown menus. |
| **`OverlayEntry`** | OverlayEntry | `IMPLEMENTED` | `incular::navigation::OverlayEntry` | Individual entry in an Overlay stack. |
| **`OverlayPortal`** | OverlayPortal | `IMPLEMENTED` | `incular::prelude::OverlayPortal` | Renders overlay child into ancestor Overlay positioned relative to widget. |
| **`AnimatedModalBarrier`** | AnimatedModalBarrier | `IMPLEMENTED` | `incular::prelude::AnimatedModalBarrier` | Animated modal barrier for dialog and popup dismissal. |
| **`PageStorage`** | PageStorage | `IMPLEMENTED` | `incular::prelude::PageStorage` | Stores and restores scroll offsets and state of unmounted pages. |
| **`RootRestorationScope`** | RootRestorationScope | `IMPLEMENTED` | `incular::prelude::RootRestorationScope` | Establishes root restoration scope for application tree. |
| **`UnmanagedRestorationScope`** | UnmanagedRestorationScope | `IMPLEMENTED` | `incular::prelude::UnmanagedRestorationScope` | Establishes unmanaged restoration scope without automatic synchronization. |
| **`WidgetsApp`** | WidgetsApp | `IMPLEMENTED` | `incular::prelude::WidgetsApp` | Top-level application bootstrap widget. |
| **`View`** | View | `IMPLEMENTED` | `incular::prelude::View` | Multi-view top-level view container. |
| **`ViewAnchor`** | ViewAnchor | `IMPLEMENTED` | `incular::prelude::ViewAnchor` | Anchor for placing auxiliary views and windows. |
| **`Title`** | Title | `IMPLEMENTED` | `incular::prelude::Title` | Window / application title metadata widget. |

### Platform Integration & Transform Coordination (7)

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`PlatformMenuBar`** | PlatformMenuBar | `IMPLEMENTED` | `incular::prelude::PlatformMenuBar` | Platform-native desktop top menu bar. |
| **`CompositedTransformTarget`** | Transform link target | `DEFERRED` | *none* | Prerequisite: LayerLink cross-subtree transformation matrix resolution. |
| **`CompositedTransformFollower`** | Transform link follower | `DEFERRED` | *none* | Prerequisite: LayerLink cross-subtree transformation matrix resolution. |
| **`RawTooltip`** | RawTooltip | `IMPLEMENTED` | `incular::prelude::RawTooltip` | A raw hover/focus tooltip presentation widget. |
| **`RawMenuAnchor`** | Overlay popup anchor | `MERGED` | `incular::navigation::Overlay` | Merged into Overlay dropdown anchor positioning. |
| **`MenuBar`** | PlatformMenuBar / Row buttons | `MERGED` | `incular::prelude::PlatformMenuBar` | Merged into PlatformMenuBar for native menus and Row buttons for custom bars. |
| **`MenuAnchor`** | Overlay popup anchor | `MERGED` | `incular::navigation::Overlay` | Merged into Overlay popup positioning. |

### Utility, Diagnostics & Debugging (9)

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`Expansible`** | Expansible | `IMPLEMENTED` | `incular::prelude::Expansible` | An expandable/collapsible container widget. |
| **`ErrorWidget`** | ErrorWidget | `IMPLEMENTED` | `incular::prelude::ErrorWidget` | Displays framework build or layout runtime errors cleanly on screen. |
| **`Banner`** | Banner | `IMPLEMENTED` | `incular::prelude::Banner` | A corner diagnostic message banner. |
| **`CheckedModeBanner`** | CheckedModeBanner | `IMPLEMENTED` | `incular::prelude::CheckedModeBanner` | Standard debug mode corner banner. |
| **`PerformanceOverlay`** | PerformanceOverlay | `IMPLEMENTED` | `incular::prelude::PerformanceOverlay` | Overlays real-time GPU and UI frame statistics. |
| **`KeepAlive`** | KeepAlive | `IMPLEMENTED` | `incular::prelude::KeepAlive` | Tells lazy list viewports to keep its subtree element alive when scrolled offscreen. |
| **`AutomaticKeepAlive`** | AutomaticKeepAlive | `IMPLEMENTED` | `incular::prelude::AutomaticKeepAlive` | Automatic client keep-alive wrapper. |
| **`IndexedSemantics`** | IndexedSemantics | `IMPLEMENTED` | `incular::prelude::IndexedSemantics` | Annotates a widget with its zero-based index in a collection for accessibility clients. |
| **`SemanticsDebugger`** | SemanticsDebugger | `IMPLEMENTED` | `incular::prelude::SemanticsDebugger` | Visual debugging overlay that renders semantic boundaries and labels. |

### Accessibility & Semantics (1)

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`Semantics`** | Semantics | `IMPLEMENTED` | `incular::prelude::Semantics` | Annotates accessibility tree with roles, labels, value, checked/disabled states, and semantic actions. |

### Text & Typography (1)

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`Text`** | Text | `IMPLEMENTED` | `incular::prelude::Text` | Rich styled text paragraph with font family, size, weight, letter spacing, alignment, and overflow. |

### Icons (1)

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`Icon`** | Icon | `IMPLEMENTED` | `incular::prelude::Icon` | Vector glyph icon with size, color, and built-in standard icon catalog. |

### Images (1)

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`Image`** | Image | `IMPLEMENTED` | `incular::prelude::Image` | Raster image widget with ImageFit (Contain, Cover, Fill, FitWidth, FitHeight, None) and repeat. |

### Material Components

| Flutter API | Incular Equivalent | Status | Public Path | Notes |
|---|---|---|---|---|
| **`ElevatedButton`** | ElevatedButton | `IMPLEMENTED` | `incular::material::ElevatedButton` | High-emphasis Material button. |
| **`FilledButton`** | FilledButton | `IMPLEMENTED` | `incular::material::FilledButton` | Filled Material button. |
| **`OutlinedButton`** | OutlinedButton | `IMPLEMENTED` | `incular::material::OutlinedButton` | Outlined Material button. |
| **`TextButton`** | TextButton | `IMPLEMENTED` | `incular::material::TextButton` | Low-emphasis text Material button. |
| **`MaterialButton`** | MaterialButton | `IMPLEMENTED` | `incular::material::MaterialButton` | Lower-level Material button vocabulary; prefer a concrete variant for new code. |
| **`RawMaterialButton`** | RawMaterialButton | `IMPLEMENTED` | `incular::material::RawMaterialButton` | Low-level Material action surface; intentionally not part of the core widgets layer. |
