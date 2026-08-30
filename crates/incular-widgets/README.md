# incular-widgets

## Frame recursion diagnostics

Build, layout, paint, semantics, and compositor traversal track their active
retained paths. In debug and DevTools builds, re-entering the same node in one
phase panics early with the triggering input, constraints, widget path, last
external call, and a backtrace. All builds retain a configurable depth guard.
Reports are written to the platform crash-report directory; override it with
`INCULAR_CRASH_REPORT_DIR` and override the fallback depth with
`INCULAR_RECURSION_LIMIT`.

Legitimate deep trees use segmented stack growth at recursive traversal
boundaries, so a finite hierarchy is not mistaken for a cycle and does not
exhaust the native UI-thread stack.

This crate implements the deployable, curated subset of Flutter 3.47.1
`widgets.dart` vocabulary:
declarative widget descriptors, retained reconciliation, renderer-neutral
layout/composition, editing, scrolling, focus, gestures, navigation, and
semantics. Design-system components live in sibling packages. The crate root
exports only widgets whose behavior is implemented and tested; incomplete
Flutter compatibility surfaces remain internal until their contracts are
implemented. Material `ElevatedButton`/`TextField`/`TextFormField` and
`RawMaterialButton` belong to `incular-material`.

The public root is curated rather than glob-re-exported. Retained IDs, render
objects, action surfaces, lazy sliver helpers, and other implementation-only
types are available to sibling framework crates through the hidden
`incular_widgets::internal` bridge. Applications should use the root Widgets
surface or `incular::prelude::*`, never that bridge.

## Signals and Tokio replace Dart builders

`Signal<T>` is the sole general reactive primitive. Reading a signal while an
application build runs records that dependency, so a later `set`/`update`
rebuilds the affected retained subtree. The Widgets crate deliberately does
not export Flutter's `Listenable`, `ValueListenable`, `ListenableBuilder`,
`ValueListenableBuilder`, or `StatefulBuilder` compatibility wrappers.

Asynchronous work belongs to the runtime's Tokio task scopes. Store its
observable state in a `Signal<AsyncState<T, E>>`, start work with
`BuildContext::spawn_into`/`TaskScope`, and render the signal normally. The
Widgets crate likewise does not export `FutureBuilder`, `StreamBuilder`,
`AsyncSnapshot`, or `ConnectionState`; those Dart snapshot lifecycles are not
part of Incular's Rust API. `AsyncValue<T>` is the runtime-task alias using
`TaskFailure`; application code can use the generic `AsyncState<T, E>` when it
needs a domain-specific error type. Both provide the small
`Idle`/`Loading`/`Ready`/`Error` state model needed by ordinary applications.

`Image` uses decoded intrinsic dimensions for layout and supports explicit
width/height plus `Fill`, `Contain`, `Cover`, `None`, and `ScaleDown` fits.
It stores a shared asset handle, not bytes or GPU resources.
Owns declarative built-in `Widget` descriptions and the persistent
`WidgetTree`. Elements and render objects reside in separate generational
arenas. Updating an element reconciles only its direct children: matching
prefix/suffix are reused, and a keyed lookup is created only for a changed
middle range that contains keys. Layout runs constraints down and sizes up;
paint caches are regenerated only for paint-dirty render objects.

This crate depends on core, layout, painting, and the platform-neutral
`incular-gestures` recognizers; it owns neither scheduling, native input loops,
nor GPU resources. `GestureDetector` is the retained-tree adapter, while pointer
event values and recognition state are owned by `incular-gestures`.

## Vector painting

`DecoratedBox`, `CustomPaint`, `Icon`, `ImageIcon`, and the decoration/value
types re-exported by this crate are the renderer-neutral painting vocabulary
from Flutter's Widgets library. Incular-specific path helpers, effect
controllers, action surfaces, and icon catalogs are implementation services;
they are available only through the hidden `incular_widgets::internal` bridge
for sibling framework crates and are not part of the application Widgets API.

## Focused editable text

`EditableText` is the renderer-neutral Widgets primitive. Its retained editing
state is kept by the runtime and the text-domain controller; callers should
import the authoritative controller from `incular-text` (or use the facade)
rather than depending on the Widgets crate's hidden retained-tree bridge.
Material `TextField` adds themed chrome and exposes the Flutter-style
`max_lines`/`multiline` API. Selection painting is line-by-line, and both
controls use the retained Parley caret stops for visual bidi cursor movement.
Wrap an editor in `UndoHistory` to configure its runtime undo/redo capacity.

## Read-only selection

`SelectableRegion` is the retained selection primitive; it has no mutable text
buffer, caret, or IME route. The Material package provides `SelectableText` and
`SelectionArea` descriptors that wrap it. Their coordinator derives caret
positions and selection rectangles from the cached Parley-backed layout,
including wrapped, mixed-font, and bidirectional text; selection changes only
repaint the highlight and never reshape or rerasterize glyphs. Keep the
optional `SelectionAreaController` when application code needs the copied
selection.

## Opt-in editor and form restoration

Editor restoration is coordinated by the runtime restoration API. A saved
committed buffer and base/extent selection replace the application default;
IME preedit is transient composition state and is never serialized.
`Form::register_with_restoration` and `FormField::bind_restoration` apply the
same binding to a field's editor while leaving validation errors derived and
non-persistent. Stable scopes and keys come from the runtime restoration API;
ordinary controllers remain ephemeral.

`ScrollView::vertical` retains a viewport clip and content translation using a
persistent `ScrollController`; wheel changes do not rebuild, relayout, or
repaint unchanged content. Hit testing applies the same scroll/translation
coordinate changes as painting. Incular's controller-backed viewport helpers
remain internal implementation details while the public API follows Flutter's
`ScrollView`, `ListView`, `GridView`, and sliver vocabulary.

Scrollbar presentation is owned by the controls layer. Widget viewports expose
the canonical controller, metrics, notifications, and sliver protocol; they do
not silently add a scrollbar or a renderer-specific style. The retained
`SliverViewport` keeps materialized rows bounded, so a thumb jump computes its
destination offset without materializing intermediate rows. The
same viewport is used by `ListView`, `GridView`, `PageView`, and
`CustomScrollView`; applications do not need a separate lazy-list primitive.

## Lazy sliver viewports

`SingleChildScrollView`/`ScrollView` remains the eager choice for ordinary,
arbitrary child trees. `ListView.builder`, `SliverList.builder`,
`SliverFixedExtentList`, `SliverGrid`, and `SliverFillViewport` are the native
lazy vocabulary for large indexed data. Their retained `SliverViewport`
implementation builds only the visible range plus a bounded cache and never
expands `0..item_count` into Widget values.

The fixed path computes content extent as checked/saturating
`item_count * item_extent`, then derives an exclusive range with direct
division: `floor((offset-cache)/extent)..ceil((offset+viewport+cache)/extent)`.
Only intersecting rows are mounted. A row at exactly the cache edge is omitted;
any intersecting row is retained. Existing indices in the next range keep their
Elements, RenderObjects, local pictures, text layouts and layers. Departing
indices unmount (there is deliberately no unsafe state recycling), which drops
their callbacks and reactive subscriptions through the runtime's normal
generational lifetime path.

## Lazy variable-extent slivers

`SliverList` uses the same retained viewport for rows whose height is known
only after layout. It starts with an estimate, records exact extents as cached
children are laid out, and compensates the visible anchor when rows above it
change size. Offset/index lookup and range selection remain bounded, so
jumping near row 900,000 of one million items does not construct the preceding
rows. `SliverVariedExtentList` provides Flutter-shaped item extent builders
when the application has a deterministic extent function.

The sliver viewport owns an outer layout layer, local clip, and inner
`-scroll_offset` content transform, just like `ScrollView`. A scroll inside the
same materialized range updates only that retained transform. Crossing a cache
boundary mounts/unmounts only the changed edge rows and leaves retained rows
unchanged. Runtime diagnostics expose logical count, range, viewport/cache
sizes, and live Element/RenderObject/PictureLayer counts for debugging.

Fixed- and variable-extent materialization are implementation details behind
the public ListView/sliver surface. Semantics expose logical child counts and
materialized item indices without creating one semantic node for every logical
row.

## Retained layout closure

`LimitedBox` supplies maxima only when its incoming axis is unbounded, while
`OverflowBox` gives its child independent optional min/max constraints but
continues reporting the parent-constrained size. `Flexible`, `Expanded`, and
`Spacer` allocate proportional shares of a bounded `Row`/`Column` main axis;
the child layout records remain ordinary retained children.

`Positioned` resolves edge pairs or explicit dimensions against a `Stack`.
`IndexedStack` retains and lays out every child but attaches only the selected
layer branch, hit-test branch, and semantic branch. `LayoutBuilder` rebuilds
and reconciles one retained child only when its incoming `Constraints` change.

## Coordinate spaces and transform composition

Visual render objects cache local picture commands. Each object has an outer
retained transform for its parent-derived layout offset. Scroll and translation
widgets place a second, inner transform below that placement: it holds only
`-scroll_offset` or animation displacement. This keeps normal layout, retained
picture reuse, clipping, and hit testing in the same coordinate model without
turning compositor updates into repaint work.

`Transform::new(AffineTransform::rotation(...), child)` applies a general
Kurbo-backed affine transform after layout; convenience constructors cover
translation, scale, rotation, and skew, and `.origin(...)` selects its local
pivot. `FittedBox` measures its child naturally and applies its `ImageFit` and
`Alignment` as the same retained affine layer. Pointer coordinates are mapped
through the inverse affine transform and semantics use transformed bounds.
`ScaleTransition` and `RotationTransition` are controller-driven compositor
updates, so animation keeps child layout and picture caches warm.

## Group opacity

`Opacity::new(0.5, child)` adds a retained isolation boundary. The child is
painted normally and the GPU compositor applies the group alpha once to the
final offscreen result, so overlapping descendants do not receive alpha
independently. Alpha is normalized to `0..=1`; non-finite values become zero.
Opacity does not imply `IgnorePointer` or hidden semantics: a transparent
button remains hit-testable and represented in the semantic tree.

`IgnorePointer` and `AbsorbPointer` make that input policy explicit without
changing painting or accessibility. Ignore removes its entire subtree from hit
testing so a painted sibling behind it can receive the pointer; absorb returns
its own retained boundary and never descends to normal child interaction.
`WidgetTree` exposes a window-local `PointerCapture` token for active gesture
streams. It is retained-safe when native OS capture is unavailable and is
released on up, cancellation, or unmount; it is never a cross-window feature.

`DragDropContext<T>` scopes typed local payloads for `Draggable<T>` and
`DragTarget<T>`. The retained arena drives start, enter, leave, update, drop,
and cancellation; `feedback()` returns an overlay-ready widget snapshot while
a drag is active. Cross-window payload transfer and lazy-list reordering are
intentionally not implied by this local contract.

For high-frequency fades, keep an `OpacityController` outside the declarative
builder and use `Opacity::controlled(controller, child)` (or
`Widget::controlled_opacity`). Controller ticks update only the retained
compositor property. Once the isolated child is warm, changing alpha does not
rebuild, relayout, repaint, rerasterize text, upload images, or retessellate
paths.

## Gaussian blur and subtree shadows

`Blur::new(sigma, child)` and `DropShadow::new(offset, sigma, color, child)`
are retained compositor widgets. `Blur::asymmetric` and
`DropShadow::asymmetric` accept separate X/Y sigmas. Sigma is in logical
pixels, uses a finite three-sigma visual support, and is normalized at the
public boundary. The child remains the hit-test and semantic target; visual
filter expansion never enlarges its interaction bounds.

For high-frequency updates, keep a `BlurController` or
`DropShadowController` outside the declarative builder and use the corresponding
`controlled` constructor. Sigma animation recomputes only the cached GPU
filter result. Shadow offset and color changes are composite-only while the
source and sigma remain unchanged, so the isolated source and Gaussian mask
stay warm. `DropShadow` is defined from the child's isolated alpha and is drawn
behind the original subtree; it is not a rectangle inferred from widget bounds.

## Color filters, blend modes, and ordered effects

`ColorFiltered::new(ColorFilter::grayscale(1.), child)` adds a retained
straight-RGBA matrix stage. `Blend::new(BlendMode::Multiply, child)` adds a
retained final-composite mode. `Effects::new(child)` is a deliberately small
builder: calling `.color_filter(...).blur(...).opacity(...)` wraps the current
child at each step, so the call order is the execution order. It does not
flatten across a blur or shadow.

For animated matrices, keep a `ColorFilterController` outside the rebuilt
description and use `ColorFiltered::controlled`. Controller ticks update only
the compositor/filter layer (`BUILD = 0`, `LAYOUT = 0`, `PAINT = 0`); the source
picture and any unchanged upstream stage remain warm. Blend modes are discrete
and intentionally have no animation controller.

The widget tree gives each color filter and blend a stable layer ID. Updating a
matrix or blend mode changes that retained property without changing the
source generation. A downstream stage observes its input generation, so a
change before a blur invalidates that blur while a change after it does not.
Visual filter bounds do not change hit testing, focus, or semantics geometry.
