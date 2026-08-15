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
