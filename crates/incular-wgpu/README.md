# incular-wgpu

## Architecture and support

| Contract | Status |
| --- | --- |
| Ownership | Owned native surfaces, GPU resources and execution of rendering commands. |
| API class | backend; re-exports and path overrides follow the [architecture contract](../../docs/ARCHITECTURE.md). |
| Support | Available WGPU backend; shared reclamation and device recovery improve in H. |

`SharedGpuContext` owns an application's one `wgpu` instance, adapter, device,
queue, immutable target-format pipeline bundles, shared image/gradient
textures, and glyph atlas storage. Each `WgpuRenderer` owns one `WindowGpuState`: its native
surface/configuration, physical presentation state, stencil attachment,
dynamic instance buffers, and retained compositor/effect caches. This lets
multiple desktop windows share one GPU device without leaking Winit types
through Incular's public API.

`WgpuRenderer::new` remains the single-window convenience constructor.
Multi-window platform code creates one `SharedGpuContext` from the first
window's owned `WindowSurfaceTarget`, then calls
`SharedGpuContext::create_renderer` (or `WgpuRenderer::new_with_shared`) for
each later window. A zero-sized surface is deliberately not configured or
presented. Surface loss and resize are window-local; device loss is a future
shared-context generation boundary.

Every renderer is created with an explicit `TransparencyMode` plus an independent
scene `background_color`. WGPU's
`CompositeAlphaMode::Auto` is never used as transparency policy because it can
resolve only to opaque/inherited compositing. Incular's source-over scene
accumulation stores premultiplied RGB, so transparent surfaces prefer
`PreMultiplied` and present directly. A surface exposing only `PostMultiplied`
is supported through a final premultiplied-to-straight presentation pass. If a
backend exposes neither transparent alpha mode, renderer initialization fails
instead of silently presenting the requested transparent window as opaque
black.

The renderer clears only the root scene to `background_color`; retained effect
and isolation targets remain transparent. Destination-reading blend modes see
that background as the initial root destination. When a non-transparent
background is configured, destination promotion uses the full view so the
background is represented exactly once across the complete presented frame.

The first window's transparency contract also participates in shared-adapter
selection. Incular preserves WGPU's preferred compatible adapter when it can
satisfy the requested surface alpha; otherwise it searches the instance's
enabled adapters for a compatible one, respecting `WGPU_POWER_PREF` instead of
inventing a framework-specific GPU preference. Later windows stay on that one
shared device and fail explicitly when their native surface cannot satisfy a
requested transparent contract.

Surface readback has one backend-independent representation: `CapturedFrame`
is top-to-bottom straight-alpha RGBA8. BGRA surfaces are reordered, and
premultiplied readback is unpremultiplied in linear light before sRGB encoding.
This keeps simulation screenshots consistent whether native presentation uses
opaque, premultiplied, or postmultiplied alpha.

`SharedGpuDiagnostics` and `WindowGpuPresentation` expose native-free
ownership diagnostics for headless tests. In particular, image and matching
DPI-specific glyph resource identities are context-wide, while presentation
generation and compositor caches remain per window.
Pipelines and gradient bind groups are cached by target format, so windows
with an unusual surface format receive a compatible variant without rebuilding
the common-format path every frame.

Text shaping remains in `incular-text`. Layout positions, advances, line
metrics, and font sizes are logical pixels. This crate turns each logical font
size into a DPI-specific physical raster request (`logical_size × scale`,
rounded to a physical raster size), keys grayscale masks by font ID, glyph ID,
and physical size, then stores them in retained page-growing
`R8Unorm` atlas textures.
Changing a window from 1x to 2x therefore requests new 2x masks without
reshaping the logical paragraph; movement and scroll offsets are deliberately
not cache-key inputs.

## Glyph rasterization

Production masks are generated solely by `fontdue` 0.9: a simple,
platform-independent rasterizer whose stable grayscale masks are visually
verified in Incular's current atlas pipeline. A `Fontdue` object is parsed once
per `FontId`; glyph-cache misses reuse that parsed object. The cache identity is
exactly `(FontId, glyph ID, rounded physical raster size)`. There is no backend
switch, hinting policy, supersampling, downsampling, or fractional-raster phase.
Fractional layout and compositor placement remain normal GPU quad placement, so
they never create extra masks. `GlyphAtlas::debug_glyph` reports the real
logical size, scale, physical raster size, bitmap dimensions, bearing, and atlas
allocation.

Text fallback does not change the atlas topology. Each font-specific logical run carries its own stable `FontId` (font bytes plus OpenType collection face index). Fontdue receives that collection index through `FontSettings`, so a shaped TTC/OTC face is rasterized as the same face. Mixed-script paragraphs safely share R8 atlas pages: warm Latin masks remain warm while a fallback run uploads only its own glyphs. Color glyph tables are a future non-R8 boundary and are not interpreted as alpha masks.

Normal glyphs use retained 1024×1024 atlas pages. A bitmap allocation occupying
at least one quarter of a normal page receives a dedicated oversize page, so a
display glyph cannot fragment the UI-text pool. `GlyphAtlas::memory` reports
normal/oversize page counts, bytes, content area, and allocated area. Pages are
retained for the renderer lifetime today; an 8 MiB CPU bitmap safety limit and
1024 ppem request limit reject unsupported giant requests instead of silently
downsampling them. GPU page dimensions remain 1024; a future larger-page policy
must query the adapter limit first.

Each allocation has a one-physical-pixel zero-coverage border. The visible
content rectangle excludes that border; its UVs name the texel-edge bounds of
the content rectangle, so a physical-size quad samples its own texel centers
without cropping the first or last coverage texel. The atlas uses linear
minification/magnification filtering and clamp-to-edge addressing. This keeps
grayscale antialiasing smooth at fractional placement while the zero border
prevents adjacent glyph coverage from bleeding. The text pipeline samples that
coverage as `text_alpha × coverage` and uses straight-alpha source-over
blending; coverage is never treated as sRGB color or thresholded.
`coverage_histogram` is available for development checks against accidental
binary masks.

Rectangle and glyph batches are consecutive display-list segments, preserving
painter order. Atlas page or rectangular scissor changes split text batches;
there is no one-draw-per-glyph path. Diagnostics expose atlas, cache, pipeline,
and buffer-growth counters.

Raster images stay renderer-neutral until a `PaintCommand::Image` reaches this
crate. The command carries an immutable `ImageHandle`, pixel-space source crop,
logical destination rectangle, and `Linear`/`Nearest` sampling choice. On first
use the renderer keys a retained `Rgba8UnormSrgb` texture cache by `ImageId`,
uploads the asset layer's already-decoded straight-alpha RGBA8 bytes with
`Queue::write_texture`, and retains its view plus lazily created sampler bind
groups. Later frames reuse that texture without decoding or upload. Entries
unused for 600 submitted frames are evicted; all GPU objects remain here.
Uploaded textures are additionally retained in the device-owned shared
image cache (budgeted, LRU-evicted across windows), and each window reports
per-frame use back to it in one batch while dropping locally superseded
entries; idle windows release stale entries through host event-loop
maintenance or on disposal.

Image batches are only merged when adjacent commands use the same image,
sampler, and rectangular clip, preserving rectangle/image/text painter order.
The image shader maps source pixels to normalized UVs and samples a static
unit-quad instance. `Rgba8UnormSrgb` samples are linearized by the GPU and the
surface format is the platform-selected `wgpu` surface format; image fragments
are straight-alpha and use normal source-alpha blending, consistent with text
and solid colors. Logical destinations and clips are transformed/scaled by the
same compositor/DPI path as other paint commands.

Paths use a retained two-stage cache owned by this crate. An immutable
`PathId` plus fill rule, or stroke width/cap/join/miter-limit, keys local Lyon
tessellation. The resulting local `[f32; 2]` vertices and u32 GPU indices are
uploaded once into retained vertex/index buffers; translation and solid paint
color are per-draw instance data, so moving or recoloring an icon does not
retessellate or reupload it. GPU path meshes unused for 600 submitted frames
are evicted while the CPU mesh remains available for a later reupload. Ordered
path batches preserve interleaving with rectangles, rounded rectangles, images,
and text, and honor the current rectangular scissor clip.

Path fill and stroke are implemented through Lyon and the GPU path pipeline;
quadratic and cubic verbs, fill rules, caps, joins, and miter limits are passed
to Lyon. The current edge treatment is Lyon tessellation without MSAA.

Normalized gradient stops are cached as retained 256-sample `Rgba8Unorm` 1D
lookup textures (implemented as `256×1` textures), keyed by stops identity
plus surface format and evicted after 600 unused submitted frames. The lookup is
premultiplied linear RGB; shaders unpremultiply for Incular's straight-alpha
surface blend. It is shared by analytic RRects and cached path meshes, so a
paint-only gradient change never retessellates geometry. The fixed lookup is a
deterministic resampling policy for arbitrarily many normalized stops; every
input stop contributes to the LUT. Like images, uploaded lookups are
additionally retained in the device-owned shared gradient cache (budgeted,
LRU-evicted across windows, generations distinguishing re-uploads), with the
same per-frame use reporting and idle reclamation.

## Non-rectangular clips

`ClipRect` stays a logical scissor intersection. `ClipRRect` and `ClipPath`
remain in the ordered draw stream as stencil mask push/pop operations. The
retained `Depth24PlusStencil8` target is cleared to zero per frame and is
recreated only for a physical surface resize. A non-rectangular push draws its
mask where the current stencil value equals depth `d`, incrementing coverage to
`d + 1`; content compares equal to its active depth; the same retained mask is
drawn with decrement on pop. This makes nested rounded/path clips an exact
intersection while restoring sibling and post-clip visibility. Clip masks also
honor the current rectangular scissor. RRects use the analytic normalized-radius
shader; paths reuse their existing fill-rule Lyon/GPU mesh cache.

## Retained offscreen group opacity

`PaintCommand::PushOpacity`/`PopOpacity` is lowered as an ordered isolation
boundary, not as per-descendant alpha. For a partial alpha the child commands
are rendered in painter order into a transparent target, then a dedicated
compositor quad samples that target and applies the group alpha exactly once.
Alpha `0` and `1` use visual fast paths; layout, hit testing, focus, and
semantics remain independent of opacity.

The offscreen color target uses the configured surface format so all existing
rectangle, text, image, gradient, path, and stencil pipelines remain compatible.
Primitive shaders write straight source colors with ordinary source-over
blending; consequently the transparent target stores premultiplied composited
RGB plus alpha. The compositor multiplies both sampled RGB and alpha by the
group alpha and uses premultiplied source-over blending. This avoids applying
alpha twice and keeps antialiased text, transparent images, and gradient edges
free of dark fringes.

Targets are tight conservative physical bounds: logical bounds are converted
with outward floor/ceil rounding and retain their logical origin for the final
quad. Internal `ClipRect` scissor coordinates are translated into the target;
`ClipRRect` and `ClipPath` use a target-local `Depth24PlusStencil8` attachment.
Ancestor clips are carried on the final quad instead of being baked into the
cached pixels, so clip-inside and clip-outside opacity have distinct, correct
semantics. Nested opacity groups render their inner target before the outer
target, with separate passes that never sample an active attachment.

Persistent entries are keyed by stable opacity layer, subtree content
generation, physical width/height, exact scale factor, target format, and
device generation. Alpha and ancestor translation are composite-only and reuse
the entry. A 64 MiB byte budget uses
least-recently-used eviction; a bounded 16 MiB exact-size target pool keys
physical width/height, color format, and stencil requirement while recycling
detached color/stencil attachments for future effects. Surface resize alone
does not invalidate an entry when its physical bounds remain unchanged, while
device recreation is represented by a new device generation. `GpuCounters`
reports cache hits/misses, rerenders, target creation/reuse/eviction, cached
bytes and peak bytes, render passes, composite draws, fast paths, and maximum
nested depth.

## Gaussian blur and drop shadows

Effect lowering keeps two retained stages: an isolated source texture keyed by
subtree generation, source dimensions, scale, format, and device generation;
and a filtered-result texture keyed by that source generation, physical X/Y
sigma, expanded dimensions, scale, and device generation. A sigma update thus
reuses the source but reruns only the filter passes. Translation, shadow offset,
and shadow color remain outside the expensive source/blur keys. Zero sigma
bypasses filter allocation and passes.

The blur is a separable GPU Gaussian over premultiplied source pixels. CPU
coefficients use `exp(-x²/(2σ²))`, a normalized symmetric `ceil(3σ)` support,
and a quantized physical-sigma kernel cache; one stable pipeline family receives
the weights through a uniform buffer. The shader explicitly treats samples
outside the source as transparent, so clamp-to-edge filtering cannot smear an
opaque edge. Normal blurs use horizontal then vertical passes. Physical sigma
above 16 selects a documented power-of-two downsample/blur/upsample path until
the low-resolution sigma is at most 16; this keeps the shader loop bounded at
97 taps while retaining full-resolution three-sigma bounds. The multi-scale
path is an efficient approximation, not a claim of exact full-resolution
Gaussian equivalence.

`DropShadow` uses the same isolated source and blurred result, reads only its
alpha, then colorizes at composite time with
`a = color.a * mask` and `rgb_premultiplied = color.rgb * a`. The shadow quad is
offset and drawn before the normal source quad, so changing offset or color
does not rerun the blur. Internal clips are already present in the source
texture; ancestor clips are applied to each final quad. Nested effects retain
their ordered boundaries rather than algebraically flattening them.

Effect targets use the existing exact-size target pool. Source and filtered
entries participate in the same 64 MiB retained offscreen budget, while pooled
temporary attachments remain capped at 16 MiB. LRU budget eviction drops only
GPU results; a later frame lazily rerenders the source or filter. Effect-specific
counters expose source hits/misses/rerenders, blur passes and kernel reuse,
large-blur resampling, shadow composites, pool activity, evictions, and cached
bytes/peaks.

`WgpuRenderer::effect_debug_tree` prints one text-only line per effect with
sigma/offset, source and mask cache state, and physical bounds; it is safe to
surface in diagnostics because it never exposes a GPU pointer.

## Color matrices and effect-chain caching

Color-matrix stages use one stable fullscreen GPU pipeline. The source texture
is sampled as premultiplied RGBA, unpremultiplied only when alpha is above an
epsilon, evaluated with the retained 4×5 matrix in straight RGBA, clamped to
finite normalized values, and premultiplied for the destination. The CPU and
GPU use the same row-major matrix layout and affine bias. Adjacent matrix
stages can be fused by `EffectChain`; a matrix on either side of a blur remains
an explicit pass. `GpuCounters` reports stage hits/misses/rerenders, matrix
passes, fusions, and retained effect bytes.

Source entries are keyed by subtree generation and physical source geometry.
Filtered entries additionally key the input generation, matrix (or sigma),
scale, format, and device generation. Thus a matrix-only change reruns the
matrix stage but not the source; changing a blur reruns the blur and every
downstream stage; ancestor translation changes only the composite quad.

## Blend modes and destination promotion

Porter–Duff `SrcOver`, `Src`, `DstOver`, `SrcIn`, `DstIn`, `SrcOut`, `DstOut`,
`SrcAtop`, `DstAtop`, `Xor`, and `Plus` use fixed-function premultiplied
attachment blending. `Multiply`, `Screen`, `Overlay`, `Darken`, `Lighten`,
`ColorDodge`, `ColorBurn`, `HardLight`, `SoftLight`, `Difference`, and
`Exclusion` use the alpha-aware artistic equation and a destination-sampling
shader. The shader samples straight RGB only for the blend function and writes
premultiplied output, matching `incular_painting::blend_premultiplied`.

The presentation surface is never sampled while it is being rendered. When an
artistic blend appears, the affected scene/composition scope is promoted to two
sampleable ping-pong targets (sparse scenes use tight physical bounds rather
than the complete window). Painter-ordered segments render into the current
target; before a destination read, the current color is copied to the
alternate target, and the blend pass samples current while writing alternate.
Nested source scopes use the same graph and copy their final target into the
retained source texture. `full_frame_intermediate_passes` stays zero for an
ordinary SrcOver-only frame; target creation/reuse and destination-read draws
are exposed in the blend counters.

Clips and stencil masks are replayed per segment, so a promotion does not
change clip or painter-order semantics. Intermediate color targets include
copy usages for the explicit ping-pong copies; their live and peak bytes are
reported separately from the source/effect cache budget. `effect_debug_tree`
reports matrix stage warmth and whether each blend uses the fixed-function or
destination-read path.
## Native window ownership

Renderer constructors accept `WindowSurfaceTarget::new(Arc::clone(&window))`
instead of detached `RawWindowHandles`. The target owns the window and its
display-handle provider. WGPU's safe surface API retains that owner, and the
renderer keeps a target for recreating lost surfaces. Ownership also covers
pending asynchronous initialization; cancelling initialization releases its
references. Native surface creation must still run on the platform's required
thread (the main thread for Metal).

`SharedGpuContext::new` needs the target only during adapter selection; the
shared GPU context does not keep the first window alive afterward. Each
renderer owns its own target. Unsupported surface configuration returns
`RendererError::SurfaceConfigurationUnsupported` instead of panicking.
