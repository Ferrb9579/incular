# incular-widgets

Widget contracts, the widget tree, built-in components, layout integration,
build context, lifecycle, and future state handling for Incular. Components
such as buttons, text fields, lists, and containers belong in this crate.

`Image` uses decoded intrinsic dimensions for layout and supports explicit
width/height plus `Fill`, `Contain`, `Cover`, `None`, and `ScaleDown` fits.
It stores a shared asset handle, not bytes or GPU resources.
# incular-widgets

Owns declarative built-in `Widget` descriptions and the persistent
`WidgetTree`. Elements and render objects reside in separate generational
arenas. Updating an element reconciles only its direct children: matching
prefix/suffix are reused, and a keyed lookup is created only for a changed
middle range that contains keys. Layout runs constraints down and sizes up;
paint caches are regenerated only for paint-dirty render objects.

This crate depends on core, layout, painting, and the platform-neutral
`incular-gestures` recognizers; it owns neither scheduling, native input loops,
nor GPU resources. `GestureRegion` is the retained-tree adapter, while pointer
event values and recognition state are owned by `incular-gestures`.

## Vector painting

`PathView::new(path).fill(brush).stroke(brush, stroke)` is the public immutable
path widget. Its default size is the path's intrinsic local bounds; `.size(...)`
requests a layout size without rewriting path vertices. `Icon` wraps a shared
immutable path and routes through the same renderer; `icons::{check, close,
plus, chevron_right}` provide small reusable examples. `DecoratedBox` paints an
analytic RRect background and optional border behind its child, including
linear/radial `Brush` gradients. It deliberately provides no child clipping
promise until true rounded clipping arrives in Phase 9.1C.

The current built-ins include a target-oriented `button` with an `ActionId`.
Runtime resolves it through persistent render hit testing; widgets never see
raw OS events.

## Focused editable text

`TextEditingController` owns a UTF-8 buffer, base/extent selection and active
IME preedit independently of a rebuilt `TextField` description. Edits use
extended grapheme cluster boundaries (ICU4X `icu_segmenter` compiled data), while public
selection offsets remain valid UTF-8 byte offsets for direct Rust slicing.
`TextField` is intentionally single-line: Enter calls `on_submit` and never
inserts a newline. `TextArea` shares the same controller but shapes a wrapped
paragraph in a fixed viewport; Enter and Shift+Enter replace the selection with
a hard newline, ArrowUp/Down and Home/End use shaped-line geometry, and the
caret keeps itself vertically visible. Selection painting is line-by-line.
Neither control implements bidi visual cursor movement yet.

## Read-only selection

`SelectableText` has no mutable text buffer, caret, or IME route. Wrap one or
more labels in `SelectionArea` to select across them with pointer drag or
Shift+Arrow and copy with Ctrl/Cmd+C. Its coordinator derives caret positions
and selection rectangles from the cached Parley-backed layout, including
wrapped, mixed-font, and bidirectional text; selection changes only repaint
the highlight and never reshape or rerasterize glyphs. Keep the optional
`SelectionAreaController` when application code needs the copied selection.

## Opt-in editor and form restoration

`TextEditingController::restored(scope, key)` creates an empty restored editor;
alternatively, create a controller with an application default and call
`bind_restoration(scope, key)`. A valid saved committed buffer and base/extent
selection replaces the default. IME preedit is transient composition state and
is never serialized. `Form::register_with_restoration` and
`FormField::bind_restoration` apply the same binding to a field's editor while
leaving validation errors derived and non-persistent. Stable scopes and keys
come from the runtime restoration API; ordinary controllers remain ephemeral.

`ScrollView::vertical` retains a viewport clip and content translation using a
persistent `ScrollController`; wheel changes do not rebuild, relayout, or
repaint unchanged content. `TranslationController` similarly drives a
compositor transform and supports runtime-ticked animation. Hit testing applies
the same scroll/translation coordinate changes as painting.

Scrollable viewports draw a logical-pixel overlay `ScrollbarStyle` by default.
The proportional vertical thumb reads the shared controller's content/viewport
extents, captures pointer drags, and track clicks page by one viewport.
`VirtualList` uses the same path, so a thumb jump computes its destination
offset directly without materializing intermediate rows.

## Lazy fixed-extent viewports

`ScrollView` remains the eager choice for ordinary, arbitrary child trees.
`VirtualList::fixed_extent(item_count, item_extent, builder)` is the lazy
vertical alternative for large indexed data. Its builder runs only as an item
enters the viewport plus a bounded 240 logical-pixel cache before and after it;
it never expands `0..item_count` into Widget values. `VirtualList::builder`
uses a 48 logical-pixel default extent, while
`fixed_extent_with_controller` lets application code retain and `jump_to` a
`ScrollController`.

The fixed path computes content extent as checked/saturating
`item_count * item_extent`, then derives an exclusive range with direct
division: `floor((offset-cache)/extent)..ceil((offset+viewport+cache)/extent)`.
Only intersecting rows are mounted. A row at exactly the cache edge is omitted;
any intersecting row is retained. Existing indices in the next range keep their
Elements, RenderObjects, local pictures, text layouts and layers. Departing
indices unmount (there is deliberately no unsafe state recycling), which drops
their callbacks and reactive subscriptions through the runtime's normal
generational lifetime path.

## Lazy variable-extent viewports

`VirtualList::variable_extent` and `ListView::variable_extent` use that same
retained viewport for rows whose height is known only after layout. They begin
with an estimate and update a shared `MeasuredExtentIndex` as cached rows are
laid out. Offset/index lookup and range selection remain bounded, so jumping
near row 900,000 of one million items does not construct the preceding rows.

For mutable data, keep a `MeasuredExtentIndex` and use
`variable_extent_with_index`. Its insertion, removal, move, and invalidation
operations update the logical mapping while the viewport rebuilds only its
visible cache. Measurements before the visible anchor compensate the scroll
offset to avoid a content jump.

The virtual viewport owns an outer layout layer, local clip, and inner
`-scroll_offset` content transform, just like `ScrollView`. A scroll inside the
same materialized range updates only that retained transform. Crossing a cache
boundary mounts/unmounts only the changed edge rows and leaves retained rows
unchanged. Runtime diagnostics expose logical count, range, viewport/cache
sizes, and live Element/RenderObject/PictureLayer counts for debugging.

Fixed-extent virtualization is implemented. Variable measured extents,
estimated extent caches, grids, sticky headers, and keep-alive policies remain
future work. Future semantics can expose logical child count and materialized
item indices without creating semantic nodes for every logical row.

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
