# Incular Flutter Widget & Public API Parity Specification

This document details the public widget architecture and API parity between **Flutter** and **Incular** established in Task 17.

---

## 1. Architectural Principles

Incular maintains a clear separation between **public API ergonomics** and **internal retained engine architecture**:

1. **Flutter-like Public API**:
   - Dedicated first-class widget descriptors (`Row::new`, `Column::new`, `Container::new`, `ListView::builder`, `GestureDetector::new`, `Semantics::new`, `SafeArea::new`, `Form::new`).
   - Fluent builders for optional parameters (`.alignment(...)`, `.padding(...)`, `.color(...)`, `.main_axis_alignment(...)`).
   - Consistent child passing (`impl Into<Widget>` for single-child and `impl IntoIterator<Item = impl Into<Widget>>` / `children![]` macro for multi-child).
   - No permanent dual API (`Widget::*` factories are legacy/internal-only; first-class structs are canonical).

2. **Rust-Native Internal Architecture**:
   - **No Dart-style class inheritance**: Descriptors are lightweight value types implementing `Into<Widget>` / `IntoWidget`.
   - **Retained Engine**: Immutable widget trees compile into retained `Element` trees, retained `RenderObject` graphs, and composited `Layer` trees.
   - **Zero Layout Duplication**: Layout widgets delegate directly to algorithms in `incular-layout` (`layout_flex`, `layout_wrap`, `layout_stack`, `layout_align`, `layout_table`).
   - **No Deep Cloning**: Widget tree reconciliation preserves identity and structural hashing guarantees.

---

## 2. Canonical Construction Conventions

| Pattern | Incular Canonical Syntax | Flutter Equivalent | Notes |
|---|---|---|---|
| **Single Child** | `Padding::new(EdgeInsets::all(8.0), child)` | `Padding(padding: EdgeInsets.all(8.0), child: child)` | Accepts `impl Into<Widget>` |
| **Multi Child** | `Row::new([child1, child2])` or `Row::new(children![child1, child2])` | `Row(children: [child1, child2])` | Accepts `IntoIterator<Item = impl Into<Widget>>` |
| **Lazy Builder** | `ListView::builder(item_count, |index| ...)` | `ListView.builder(itemCount: ..., itemBuilder: ...)` | Retained virtualized list allocation |
| **Separated List** | `ListView::separated(count, item_fn, sep_fn)` | `ListView.separated(...)` | Interleaved separator elements |
| **Fixed Sizing** | `SizedBox::from_size(size).child(child)` or `SizedBox::new().width(w).height(h).child(c)` | `SizedBox(width: w, height: h, child: c)` | Dimension modifiers |
| **Color Wrapper** | `ColoredBox::new(color, child)` | `ColoredBox(color: color, child: child)` | Color painting wrapper around child |
| **Composite Box** | `Container::new().padding(p).color(c).child(child)` | `Container(padding: p, color: c, child: child)` | Composes `Padding`, `ColoredBox`, `DecoratedBox`, `Align`, `Transform` |
| **Gestures** | `GestureDetector::new(child).on_tap(...)` | `GestureDetector(onTap: ..., child: child)` | Retained gesture arena dispatcher |
| **Semantics** | `Semantics::new(child).label("...").role(SemanticRole::Button)` | `Semantics(label: "...", child: child)` | AccessKit semantic bridge |
| **Forms** | `Form::new(child).autovalidate_mode(mode)` | `Form(child: child, autovalidateMode: ...)` | Declarative form scope |

---

## 3. Audited API Parity Manifest (93 Authorized Entries)

All 93 widgets in `specs/flutter_api_parity.jsonl` are fully implemented, canonical, and tested:

### 3.1 Flex, Stack, Wrap & Table Layout (11)
- `Row`: Horizontal flex container (`MainAxisAlignment`, `CrossAxisAlignment`, `MainAxisSize`, `spacing`, `TextDirection`, `VerticalDirection`).
- `Column`: Vertical flex container (`MainAxisAlignment`, `CrossAxisAlignment`, `MainAxisSize`, `spacing`, `TextDirection`, `VerticalDirection`).
- `Flex`: Direction-configurable flex container (`Axis::Horizontal` / `Axis::Vertical`).
- `Expanded`: Flex child forcing tight allocation with configurable `flex` factor (defaults to 1).
- `Flexible`: Flex child allowing loose/tight allocation with configurable `flex` and `FlexFit`.
- `Spacer`: Empty expandable flex element consuming remaining main-axis space.
- `Stack`: Layered overlay container (`alignment`, `fit: StackFit`, `clip_behavior: Clip`, `text_direction`).
- `Positioned`: Stack parent data specifying `left`, `top`, `right`, `bottom`, `width`, `height`.
- `IndexedStack`: Stack displaying only one child at `index` while keeping retained state for siblings.
- `Wrap`: Flowing multi-run layout (`direction`, `alignment`, `spacing`, `run_alignment`, `run_spacing`, `cross_axis_alignment`).
- `Table`: Measured row-major grid with max-content cell measurement and uniform row/column distribution.

### 3.2 Containers, Dimensions & Alignment (11)
- `Container`: Convenience composer widget combining padding, margins, decoration, sizing, constraints, alignment, and transforms without duplicate layout passes.
- `SizedBox`: Box with fixed `width` / `height` constraints, `SizedBox::from_size(size)`, `SizedBox::square(dimension)`, `SizedBox::shrink()`, and `SizedBox::expand()`.
- `ColoredBox`: Retained solid color painting wrapper over child.
- `Padding`: Inset container applying `EdgeInsets` around child.
- `Align`: Positional container placing child according to `Alignment` with optional `width_factor` and `height_factor`.
- `Center`: Convenience centering wrapper (`Align::center(child)`).
- `ConstrainedBox`: Container imposing additional tight/loose `Constraints` on child.
- `LimitedBox`: Container providing upper bound limits on child when incoming constraints are unbounded.
- `OverflowBox`: Container allowing child to overflow parent layout bounds with custom min/max dimensions.
- `UnconstrainedBox`: Container allowing child to render at its natural unconstrained size within parent bounds.
- `FractionallySizedBox`: Container sizing child to a fraction of available parent size (`width_factor`, `height_factor`, `alignment`).

### 3.3 Proportions & Custom Painting (8)
- `AspectRatio`: Container enforcing fixed aspect ratio proportional sizing.
- `Baseline`: Container aligning child along horizontal baseline offset.
- `FittedBox`: Content scaling container applying `ImageFit` (`Contain`, `Cover`, `Fill`, `FitWidth`, `FitHeight`, `None`) and `Alignment`.
- `Visibility`: Toggles child layout, rendering, hit-testing, and semantics with optional maintain flags (`maintain_size`, `maintain_animation`, `maintain_state`).
- `Offstage`: Hides child subtree from painting and hit testing while keeping retained state.
- `LayoutBuilder`: Deferred builder receiving parent layout constraints before computing child subtree.
- `RepaintBoundary`: Retained compositor boundary isolating display list picture recording from parent/child changes.
- `CustomPaint`: Low-level canvas rendering delegate for custom drawing and painting commands.

### 3.4 Clipping (4)
- `ClipRect`: Clips child painting to rectangular bounds with `Clip` policy.
- `ClipRRect`: Clips child painting to rounded rectangle with `CornerRadii` and `Clip` policy.
- `ClipOval`: Clips child painting to inscribed axis-aligned oval/circle.
- `ClipPath`: Clips child painting to arbitrary kurbo/vector vector path geometry.

### 3.5 Scrolling & Slivers (11)
- `SingleChildScrollView`: Viewport enabling scrolling over a single child widget.
- `ListView`: Scrollable linear list supporting `new([children])`, `builder(count, item_builder)`, `separated(count, item_builder, sep_builder)`, `fixed_extent(...)`, and `variable_extent(...)`.
- `GridView`: Scrollable two-dimensional grid supporting `count(cross_axis_count, [children])` and `builder(count, cross_axis_count, row_extent, item_builder)`.
- `PageView`: Swipeable/scrollable multi-page viewport with snapping and controller support.
- `CustomScrollView`: Viewport coordinating ordered slivers with shared scrolling geometry.
- `SliverList`: Linear sliver delegating item building to viewport scroll bounds.
- `SliverGrid`: Grid sliver arranging items with cross-axis count and row extents.
- `SliverPadding`: Sliver applying padding around descendant slivers.
- `SliverPersistentHeader`: Pinned or floating persistent header sliver remaining fixed during scrolling.
- `SliverAppBar`: Framework-neutral app bar sliver with pinned and expanded height support.
- `SliverToBoxAdapter` (`SliverBox`): Bridges normal box widgets into the sliver protocol.

### 3.6 Gestures & Interaction (5)
- `GestureDetector`: Retained gesture recognizer region with callbacks for `on_tap`, `on_double_tap`, `on_long_press`, `on_pan_*`, `on_scale_*`, `on_horizontal_drag_*`, `on_vertical_drag_*`, and hit-test behavior.
- `MouseRegion`: Hover and mouse cursor tracking region (`on_enter`, `on_exit`, `on_hover`, `cursor`).
- `IgnorePointer`: Transparently allows pointer events to pass through this subtree to stacked targets behind it.
- `AbsorbPointer`: Consumes and blocks pointer events from reaching either its child or stacked targets behind it.
- `Dismissible`: Swipe-to-dismiss gesture container supporting `DismissDirection` and completion callbacks.

### 3.7 Platform, Safe Area & Forms (5)
- `SafeArea`: Insets child to avoid hardware cutouts, status bars, notches, and software home indicators.
- `Form`: Declarative form container managing form validation state and auto-validation modes.
- `TextFormField`: Form field integrating text editing controller with custom validation and error rendering.
- `Focus`: Retained focus node widget handling focus gaining, losing, and key event routing.
- `FocusScope`: Focus scope boundary managing subtree focus traversal and active focused nodes.
- `KeyboardListener`: Key event listener dispatching raw key presses and key releases.

### 3.8 Animation & Effects (12)
- `AnimatedContainer`: Implicitly animated container transitioning layout, styling, and color changes over a duration and curve.
- `AnimatedOpacity`: Implicitly animated opacity container smoothly fading child transparency.
- `AnimatedPadding`: Implicitly animated padding container interpolating edge insets.
- `AnimatedAlign`: Implicitly animated alignment container interpolating child positions.
- `AnimatedPositioned`: Implicitly animated stack positioning container interpolating box bounds.
- `FadeTransition`: Explicit animation widget driven by controller progress for alpha transitions.
- `ScaleTransition`: Explicit animation widget driven by controller progress for scale transitions.
- `RotationTransition`: Explicit animation widget driven by controller progress for 2D angle rotations.
- `SlideTransition`: Explicit animation widget driven by controller progress for offset translation.
- `SizeTransition`: Explicit animation widget expanding/collapsing dimensions during transitions.
- `Hero`: Shared-element transition container tag for cross-page route animations.
- `BackdropFilter`: Compositor filter applying Gaussian blur and color effects to background layers underneath child.

### 3.9 Semantics & Accessibility (4)
- `Semantics`: Exposes explicit accessibility roles, labels, hints, values, and actions to AccessKit native bridges.
- `MergeSemantics`: Merges descendant accessibility properties into a single combined native node.
- `ExcludeSemantics`: Excludes child subtree from native accessibility hierarchy (decorative visuals).
- `BlockSemantics`: Blocks accessibility exploration of ancestor and background sibling nodes (modal dialogs).

### 3.10 Fundamental Core Widgets (10)
- `Text`: Basic styled text display widget.
- `RichText`: Multi-span rich formatted text display widget with per-span styles.
- `Image`: Image rendering widget supporting asset, memory, and raw pixel sources with `ImageFit`.
- `Icon`: Icon display widget with font glyph mapping and color tinting.
- `Button`: Interactive button widget with press, hover, exit callbacks, and compositional child content.
- `TextField`: Single-line text input widget with controller, caret, selection, and IME integration.
- `TextArea`: Multi-line text input widget with scrolling, wrapping, caret navigation, and IME integration.
- `DecoratedBox`: Visual container applying background colors, gradients, borders, and corner radii.
- `Transform`: Affine transform container applying 2D translation, rotation, scaling, and skewing matrices.
- `Opacity`: Alpha blending container applying fractional transparency to child layer.

---

## 4. Verification Suite & Validation Commands

All workspace crates and tests conform to the Incular validation suite:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Parity manifest consistency is continuously asserted by `tests/parity_manifest.rs`, ensuring zero unknown or unresolved entries in `specs/flutter_api_parity.jsonl`.
