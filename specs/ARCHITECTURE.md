# Incular architecture (implemented slice)

```
Widget values -> local reconciliation -> Element arena -> Render arena
                                                |             |
                                                +-- IDs -------+-- layout / cached paint
                                                                  |
                                                          DisplayList -> incular-wgpu surface/pipeline
```

`Widget` values are immutable descriptions. `WidgetTree` owns separate safe
generational arenas for Elements and RenderObjects, so a stale ID can never
refer to a replacement node. Elements record their parent, children, widget
description and render ID. Render objects record parent/children, constraints,
geometry, dirty phases, and a cached local display list.

An update reconciles only one element's children. Compatible nodes have equal
widget type and equal optional local key. The reconciler matches compatible
prefix/suffix first, then allocates a keyed lookup only if the changed middle
contains keyed children. Removed subtrees are unmounted once, invalidating IDs.

Dirty flags distinguish BUILD, LAYOUT, PAINT, COMPOSITE, and SEMANTICS.
Structural/geometry changes propagate layout to render ancestors; paint-only
changes stay at the node. The runtime coalesces element updates and executes
BUILD, LAYOUT, then PAINT during `run_frame`. `Signal<T>` reads inside
registered element builders record that Element as a dependency; changed values
queue only those Elements for a future frame. Painting regenerates only dirty
node caches, then composes an ordered display list. `incular-wgpu` lowers this
to painter-order-preserving rectangle instance batches and renders them.

On Linux, `incular-linux` owns the `winit` application handler/window. It
normalizes physical pointer coordinates to logical coordinates in
`incular-platform`, invokes persistent-render hit testing in `incular-runtime`,
then passes action IDs to application handlers. `incular-wgpu` owns the
instance, surface, adapter/device/queue, static unit-rectangle mesh, retained
pipeline, and geometrically-grown instance buffer. Layout uses logical pixels;
the renderer applies scale while converting instances to physical/NDC space.

Current dependency DAG: `core -> {layout, painting, animation, assets,
platform}; {core, layout, painting} -> widgets; widgets -> accessibility;
{widgets, layout, painting, core, accessibility, animation, assets} -> runtime;
{painting, core, platform} -> wgpu; {platform, runtime, wgpu} -> linux; all
public-facing crates -> incular`.

## Declarative application model

`Application::new` owns a root `FnMut(&mut BuildContext) -> Widget` and mounts
it on the persistent root Element. Application code does not look up Elements,
register builders, allocate action IDs, or schedule frames. A scoped build
collector records Signal dependencies; reads outside builds are plain snapshots.
`Signal<T>` is intentionally single-threaded and must not be shared between
independent applications.

Buttons carry callbacks until mount, where the runtime assigns opaque action
IDs. It removes unmounted IDs, and requires pointer down/up on the same Button
before dispatch. Hover and pressed visual state are paint-only changes.

## Text pipeline

`incular-assets` owns `FontHandle` bytes and `FontId`; `incular-text` discovers
system fonts with `fontdb`, shapes UTF-8/OpenType text using `rustybuzz`, and
uses Unicode break opportunities for logical-pixel lines. Its bounded 256-entry
cache keys only metric inputs, not foreground color. Text layouts expose
baseline/line metrics and contiguous glyph arrays through renderer-neutral
`PaintCommand::GlyphRun` values.

The GPU boundary applies the existing single DPI scale factor to glyph raster
size without changing logical layout. `incular-wgpu` owns a retained glyph atlas
keyed by font, glyph ID, and physical size. Each stable atlas page maps to one
retained `R8Unorm` GPU texture; a cache miss rasterizes once and uploads only
that glyph's padded subregion. Glyphs use a static quad, retained growable
instance buffer, per-instance UV/color data, and a coverage shader with
straight-alpha source-over blending. Color-only text changes reuse atlas masks.

The backend lowers display lists into consecutive compatible rectangle and
glyph batches, never a global "all rectangles then all text" pass. It splits
glyph work by atlas page or clip while keeping command order, and applies the
same rectangular clip stack to both pipelines as physical-pixel scissor state.
Renderer diagnostics separately record glyph cache/raster/upload/page work,
atlas texture creations, pipeline creations, and both instance-buffer growth
counts.

## Retained compositor, scrolling, and animation

`incular-painting::LayerTree` is a renderer-independent generational arena of
`Picture`, `Transform`, and axis-aligned `ClipRect` layers. A picture owns an
`Arc<DisplayList>` and conservative local bounds. Render objects create stable
transform and picture layers at mount; unmount removes their compositor state.
Scroll views additionally retain a viewport clip and a content transform.

Scene submission flattens this retained tree to the existing ordered
display-list format, accumulating translations and intersecting clips. It
conservatively culls a picture wholly outside the active rectangular clip. The
flattened sequence remains painter-ordered, so the GPU backend continues direct
rectangle/glyph drawing and scissor clipping without one texture per layer.

Invalidation is phase-specific: content changes can request BUILD/LAYOUT/PAINT,
but `ScrollController` and `TranslationController` changes update only retained
transform state and COMPOSITE. The current CPU flattening emits a transient
linear submission sequence each frame, but does not rebuild the retained layer
graph or repaint cached pictures.

`ScrollView::vertical(controller, child)` lays out its child loose/unbounded in
the vertical direction, clips it to a parent-constrained viewport, and clamps
the persistent offset to `0..content_extent-viewport_extent`. Normalized wheel
events carry logical-pixel deltas. Hit testing applies the inverse scroll or
translation displacement, keeping interactive targets at their visible places.

`incular-animation` owns controller values, curves and interpolation but no
timer. Runtime supplies monotonic timestamps and requests another frame only
while an animation remains active. Group opacity, affine clipping, fling
physics, and virtualization remain future work.

## Lazy viewports and fixed-extent virtualization

`ScrollView` is intentionally eager: it accepts an arbitrary already-mounted
child tree and uses a retained clip/content transform for inexpensive movement.
`VirtualList` is the indexed lazy viewport protocol implemented for large,
fixed-height vertical datasets. Its declarative configuration owns item count,
fixed logical item extent, a persistent `ScrollController`, bounded cache
extent, and an `Fn(index) -> Widget` item factory. Application code never sees
Element, render, layer, or materialization IDs.

The lazy viewport gets its logical scroll offset and constrained viewport
extent during layout. Fixed extent gives O(1) content geometry:
`content_extent = saturating(item_count * item_extent)` and item `i` is placed
directly at `i * item_extent` (integer multiplication before conversion to the
framework's f32 logical coordinate). Its materialized range is the exclusive
range `floor((offset-cache)/extent)..ceil((offset+viewport+cache)/extent)`,
clamped to the logical count. The 240 logical-pixel default cache prevents
per-pixel mount churn and is bounded, so steady-state Elements, RenderObjects,
PictureLayers, text layouts, handlers and traversal cost are O(visible +
cache), not O(total item count). Extremely enormous f32 logical extents
saturate rather than wrap; practical coordinate precision remains bounded by
f32.

The viewport maps retained children by logical index. An index that remains in
the range retains its existing subtree and keyed reconciliation semantics; an
exiting index is unmounted and a new index is mounted. No Element with
application state is reused for a different logical item. The runtime drains
lazy unmounts in the same frame, removing callback handlers and Signal
subscriptions. The same stable item identity model leaves a future explicit,
bounded keep-alive policy possible without making offscreen retention default.

Changing scroll offset without changing this range is compositor-only: no
builder, mount, layout or paint work occurs, and the inner content transform is
updated. A boundary crossing dirties the viewport only long enough to add/drop
the small changed row set; retained rows keep their geometry, local display
lists, shaped text, and glyph-atlas entries. Fixed-extent list hit testing uses
the existing inverse scroll coordinate path and therefore considers only
materialized rows at their visible locations. Native semantics can later expose
the list's logical child count plus materialized index/position metadata without
forcing one million semantic nodes.

Variable-height virtualization, estimated/remembered extents, lazy grids and
sliver-style composition are deliberately future work; Phase 5 implements the
production fixed-extent path only.

## Editing correction and shared scrollbars

Native editing commands are normalized as pressed `KeyEvent`s. The Linux/winit
adapter recognizes physical Backspace/Delete/navigation keys and forwards only
printable key text separately, preventing a Backspace control payload from
being reinserted after the runtime command. Repeated pressed events are OS
repeat and each invokes the grapheme-safe controller operation exactly once.
IME preedit remains visual-only until commit.

`TextField` is single-line and submits on Enter. `TextArea` is the explicit
multiline control; it uses the normal text layout cache with a bounded width,
hard newlines plus soft wrapping, line-based caret hit testing/navigation, and
line-by-line selection rectangles. Its internal vertical offset is paint-local
and changes without reshaping an unchanged paragraph.

`ScrollController` owns offset, viewport extent, content extent and maximum
extent for both `ScrollView` and `VirtualList`. Their framework-rendered
vertical overlay scrollbar uses `raw_thumb = track * viewport/content`, clamped
to the configured minimum size. With `M = max(content - viewport, 0)` and
`travel = max(track - actual_thumb, 0)`, the authoritative inverse mappings
are `thumb_top = track_top + (offset / M) * travel` (when `M > 0`) and
`offset = clamp(thumb_top - track_top, 0, travel) / travel * M` (when
`travel > 0`). This deliberately uses the actual, possibly minimum-clamped
thumb extent. Painting, hit testing, and dragging share that geometry.

A thumb press captures the pointer until up/cancel and records a fixed logical
`grab_offset = pointer_y - thumb_top`; moves use `pointer_y - grab_offset` and
clamp only the resulting thumb position. Thus a boundary never makes the drag
sticky and reversing direction during the same gesture works immediately.
Scrollbar geometry is in logical coordinates and is attached to the viewport,
not its scrolling content transform. Track clicks remain a separate paging
path. The common
convention is positive normalized Y increasing offset (content upward): winit
`LineDelta` maps to 40 logical px per line, while `PixelDelta` is DPI-converted
without rounding or inversion. Retained list content remains compositor-only
inside a stable materialization range; only the scrollbar overlay repaints as
its thumb changes.

### Coordinate Spaces and Transform Composition

Every `RenderObject` paints its cached `PictureLayer` in **local logical
coordinates**: rectangle origins, shaped glyph offsets, and `GlyphRun::origin`
are never rewritten into parent or world coordinates. A child render object has
a separately stored parent-relative **layout offset**. Its outer retained
`Transform` layer carries exactly that static offset, and the layer tree
accumulates those outer transforms to produce a world-logical transform at
flattening time.

Compositor-only motion is a distinct inner transform. A scroll view is
`layout placement -> viewport-local ClipRect -> (0, -scroll) content transform
-> normal child tree`; a translated widget is `layout placement -> animated
transform -> normal child tree`. Thus the final transform is the sum of every
ancestor placement plus any scroll/animation displacement, with every layout
offset applied exactly once. Clips are expressed in the local coordinates of
their clip layer, converted to world logical coordinates while flattening, and
are not moved by the scroll-content transform they contain.

The `wgpu` backend receives that final logical placement and performs only the
last logical-to-physical conversion for the surface scale factor. It does not
apply layout translations or DPI scaling inside the retained tree. Hit testing
uses the equivalent inverse scroll/translation path, so a target's visual and
interactive positions agree. `LayerTree::flattened_pictures` exposes local
bounds, world bounds, and active clips for CPU-side coordinate diagnostics.
