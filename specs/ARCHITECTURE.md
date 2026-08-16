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
to painter-order-preserving rectangle, glyph, and image instance batches and
renders them.

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
{painting, core, platform, assets} -> wgpu; {platform, runtime, wgpu} -> linux; all
public-facing crates -> incular`.

## Declarative application model

## Platform Architecture

`incular-platform` owns logical/physical metrics, normalized pointer, wheel,
keyboard and IME conversion, clipboard traits, and opaque GPU handles. Native
window ownership remains target-selected in `incular-linux`, `incular-windows`,
and `incular-macos`; they share the portable winit/wgpu runner contract rather
than forking Runtime or Widget behavior. `incular::run(Application)` selects
the backend at compile time. Android and iOS retain separate lifecycle crates:
their future surface recreation, safe-area, touch, and soft-keyboard behavior
will feed the same normalized contract rather than desktop-only widgets.

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
keyed by font, glyph ID, and rounded physical raster size.
Each stable atlas page maps to one
retained `R8Unorm` GPU texture; a cache miss rasterizes once and uploads only
that glyph's padded subregion. The padding is one zero-coverage physical texel
on every side, and each physical-size quad maps UVs to the content rectangle's
texel edges. Linear atlas filtering therefore preserves grayscale coverage at
fractional placement without sampling neighboring glyphs. Glyphs use a static
quad, retained growable instance buffer, per-instance UV/color data, and a
coverage shader with straight-alpha source-over blending. A DPI change requests
new physical masks while retaining shaped layout; color-only and translation
changes reuse atlas masks.

The production `fontdue` grayscale backend rasterizes at the rounded physical
size and never changes shaped advances or logical layout. Scroll and compositor
translation are never cache inputs. Glyphs consuming at least a quarter of a 1024×1024 page
go to dedicated oversize pages, keeping the normal UI atlas intact. The atlas
reports normal/oversize bytes and area, rejects requests above 1024 ppem or an
8 MiB bitmap safety limit, and leaves future larger-page selection dependent on
the adapter's texture-dimension limit.

The backend lowers display lists into consecutive compatible rectangle and
glyph batches, never a global "all rectangles then all text" pass. It splits
glyph work by atlas page or clip while keeping command order, and applies the
same rectangular clip stack to both pipelines as physical-pixel scissor state.
Renderer diagnostics separately record glyph cache/raster/upload/page work,
atlas texture creations, pipeline creations, and both instance-buffer growth
counts.

## Raster image pipeline

`incular-assets` decodes filesystem, embedded, memory, or generated immutable
images once into shared straight-alpha RGBA8 `DecodedImage` data. Every
`ImageHandle` has a stable `ImageId`; widgets use intrinsic pixel dimensions
for layout and calculate `Fill`, `Contain`, `Cover`, `None`, and `ScaleDown`
into a pixel-space source crop plus logical destination rectangle. Painting
keeps that data renderer-independent in `PaintCommand::Image`, including a
neutral `ImageSampling::{Linear, Nearest}` option.

`incular-wgpu` depends on `incular-assets` (never the reverse) to read that
already-decoded immutable data on a cache miss. It retains `ImageId ->
GpuImage` textures, views, and sampler bind groups entirely inside the GPU
crate. First use creates a `Rgba8UnormSrgb` texture and writes the exact RGBA8
rows with `Queue::write_texture`; later use reuses it with no decode/upload.
The cached image shader uses a static quad and per-instance destination NDC
rectangle plus source UV rectangle. Normal source-alpha blending is used for
straight-alpha image fragments. Image textures are sRGB; samples are decoded
to linear by the GPU and the platform-selected surface format receives the
same linear color convention used by solid colors and text.

Image work is only batched across consecutive commands with the same image,
sampling mode, and clip, so it interleaves correctly with rectangles and
glyphs. The existing transform and logical-to-physical DPI conversion affect
only the quad instance; source pixels and retained GPU texture never move or
rescale. A small last-used-frame policy evicts image textures unused for 600
submitted frames.

## Retained compositor, scrolling, and animation

`incular-painting::LayerTree` is a renderer-independent generational arena of
`Picture`, `Transform`, axis-aligned `ClipRect`, and `Opacity` layers. A picture
owns an `Arc<DisplayList>` and conservative local bounds. Render objects create
stable transform, picture, and (when requested) opacity layers at mount;
unmount removes their compositor state. Scroll views additionally retain a
viewport clip and a content transform. An opacity layer stores only normalized
alpha, subtree bounds, and a content generation; GPU targets remain owned by
`incular-wgpu`.

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
while an animation remains active. Opacity animation follows the same
compositor-only path as translation: once an isolated target is warm, alpha
ticks do not rebuild, relayout, repaint, or rerasterize its child.

### Retained offscreen group opacity

Partial group opacity is isolated in `incular-wgpu`: the ordered child stream
renders into a transparent, tight physical target and a compositor quad applies
group alpha once. The target stores premultiplied composited RGB plus alpha;
the compositor scales both by group alpha and uses premultiplied source-over
blending. Outward physical floor/ceil rounding and a retained origin prevent
fractional-edge cropping. Clips inside the group use target-local scissor and
stencil attachments; ancestor clips remain active on the final quad.

Opacity cache identity is stable layer ID + subtree content generation +
physical width/height + exact scale factor + target format + device generation.
Alpha and ancestor translation do not invalidate pixels. A bounded byte budget evicts least-recently-used entries,
and a separate bounded target pool keys exact-size color/format/stencil targets
for reuse.
Surface resize alone does not invalidate a target whose physical bounds remain
valid; device recreation starts a new generation. Opacity is visual only and
does not alter hit testing, focus, pointer/keyboard routing, or semantics.

Blur and shadows build on the same retained source/effect cache. Color matrices
are ordered straight-RGBA stages with adjacent-matrix fusion only; blend modes
are final composite state and never change source generations. Porter–Duff
modes use fixed-function premultiplied blending, while artistic modes promote
only their composition scope to sampleable ping-pong targets so a destination is
never read from the texture currently being written. Ordinary SrcOver frames
retain the direct presentation fast path. Backdrop effects, inner shadows,
custom filters, and further advanced blend modes remain future work on this
offscreen foundation.

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

## Accessibility semantics

Accessibility is a retained, renderer-independent tree owned beside the
Element/RenderObject arenas. `SemanticNodeId` is generational and is mapped to
the persistent Element that contributes it; compatible reconciliation preserves
the mapping, while unmount removes both node and action target. Semantics are
derived from widget/control state, never from display lists, glyphs, or GPU
commands. Padding, alignment, clipping, and transforms merge their children
into the nearest meaningful node. `Widget::accessibility_label`,
`accessibility_description`, and `exclude_semantics` let applications override
or suppress this contribution.

The frame order is BUILD, LAYOUT, retained COMPOSITE, semantics synchronization,
PAINT, then platform delivery. Bounds use the same world-coordinate traversal
as hit testing, including ancestor scroll and translation transforms. Offscreen
eager scroll descendants remain logical semantic descendants but retain their
actual (possibly clipped/off-viewport) world bounds. A VirtualList contributes
one List node plus only its mounted rows; row metadata includes logical index
and set size, so a million logical rows never creates a million Element or
semantic nodes. Future adapters can request an offscreen item by index, scroll
the existing controller, materialize it, and then focus its new semantic node.

Semantic actions are owned requests (`Focus`, `Activate`, `SetText`,
`SetSelection`, and scroll actions) resolved through IDs by Runtime. They use
the same focus owner, editing controller, callbacks, and ScrollController as
keyboard/pointer input. Text selection uses the editor's UTF-8 byte offsets;
IME preedit is not reported as committed semantic text. Caret blink changes no
semantic state. The visual overlay scrollbar is intentionally not an additional
semantic control: its ScrollView/List exposes the scrolling actions.

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

## Vector paint and gradients

`incular-painting` owns one renderer-neutral `Brush` model: `Solid`,
`LinearGradient`, and `RadialGradient`. `GradientStops` clamps finite offsets to
`0..=1`, stable-sorts them, makes empty input transparent, and duplicates a
single stop. The immutable normalized stop set has a stable `GradientId`.
Gradient geometry is local logical space: linear paint uses projection onto
`end - start`; radial paint uses distance from `center / radius`; degenerate
axes/radii select the final stop deterministically. Duplicate offsets are
right-continuous, enabling hard transitions.

The wgpu backend retains a `256×1` premultiplied-linear lookup texture per
`GradientId` (600-frame stale eviction). This deterministic resampling accepts
any number of normalized stops, with all stops contributing; it is paint data,
not geometry. Both analytic RRects and cached Lyon paths bind the same resource.
Path tessellation and GPU mesh keys contain only `PathId` and fill/stroke
geometry, never Brush, so gradient changes and compositor translation do not
retessellate a path.

Applications use `PathView`, `Icon`, and `DecoratedBox` from
`incular-widgets`; these produce ordinary paint commands through the retained
widget tree. At the renderer boundary, rectangular clips remain scissors while
rounded-rectangle and path clips remain explicit ordered stencil transitions.
The stencil stack is depth-counted (maximum 255 nested non-rectangular clips),
cleared for every target frame, and combines with the active rectangular
scissor. Path masks share the fill-rule keyed retained Lyon/GPU mesh cache;
rounded masks share the analytic RRect radius convention.

## Font fallback

`incular-text` owns the renderer-neutral `fontdb` database and resolves `SystemUi`, generic, named, and application fallback families before shaping. Coverage is checked for whole grapheme clusters and cached by style/fallback chain/database generation; a one-time per-script candidate index avoids a full installed-font scan for every character. Each resolved face shapes its own Rustybuzz run, preserving its advances and bidi direction. A logical line carries several `GlyphRun`s on one baseline plus flattened caret/hit-test geometry. `FontId` incorporates collection-face identity, so `incular-wgpu` safely keys all fallback glyph masks in the shared Fontdue/R8 atlas. Color glyph formats remain explicitly outside the current alpha-mask path.

## Text coverage quality

`incular-wgpu` rasterizes each shaped glyph with a parsed, cached Fontdue font
at `logical size × window scale`, rounded to its physical raster size. The
production key is `(FontId, glyph ID, physical raster size)`. Fontdue returns
one target-resolution grayscale mask which enters the retained `R8Unorm` atlas;
there is no hinting policy, supersampling, downsampling, or phase cache. The
final bitmap is limited to 8 MiB. Scroll and compositor movement remain absent
from cache identity and never cause raster work.

## Gaussian effects and retained filter resources

Phase 10.1 adds renderer-neutral `GaussianBlur` and `DropShadowEffect` layers.
Sigma is a logical-pixel value normalized at the painting/widget boundary and
converted to physical pixels only by `incular-wgpu`. A layer's source
generation excludes its own sigma, shadow offset, and shadow color, so retained
compositor updates do not mark the child for build, layout, or paint. Effects
remain ordinary ordered boundaries: source commands, including internal
rectangular/rounded/path clips, are isolated first, then the result is filtered
and composited through the current ancestor clip. Effect expansion is visual
only; hit testing and semantics continue to use the child's normal geometry.

The backend separates source isolation from filtered results. Source entries
are keyed by subtree generation, source bounds/dimensions, DPI, format, and
device generation. Blur entries add physical X/Y sigma, expanded dimensions,
and the selected downsample factor. A sigma change invalidates only the latter;
ancestor translation changes only the final quad. A shadow reuses the same
blurred source-alpha representation when source and sigma are unchanged, then
colorizes and offsets it during the composite draw before the original source.

Filtering is a true separable Gaussian over premultiplied pixels with a
normalized symmetric three-sigma kernel generated on the CPU and uploaded via
one stable GPU pipeline. Samples outside the source are transparent rather than
clamped opaque edge pixels. For physical sigma above 16, repeated power-of-two
downsampling keeps the low-resolution sigma in the bounded direct range,
followed by horizontal/vertical filtering and an upsample. This is explicitly
an efficient multi-scale approximation; full-resolution effect bounds still
use outward-rounded three-sigma support.

Effect and source targets share Phase 10's 64 MiB retained offscreen budget and
16 MiB exact-size transient pool. LRU eviction removes only GPU results and
recomputes them lazily. `GpuCounters` and compositor debug-tree entries expose
source/filter cache state, physical bounds, pass counts, kernel reuse,
downsample/upsample activity, shadow composites, and effect memory usage.
