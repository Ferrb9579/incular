# incular-wgpu

Owns Incular's native `wgpu` surface, retained rectangle/text/image pipelines, static
unit quad, and geometrically grown instance buffers. It consumes ordered
renderer-neutral display lists without leaking `wgpu` types upstream.

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
lookup textures (implemented as `256×1` textures), keyed by immutable
`GradientId` and evicted after 600 unused submitted frames. The lookup is
premultiplied linear RGB; shaders unpremultiply for Incular's straight-alpha
surface blend. It is shared by analytic RRects and cached path meshes, so a
paint-only gradient change never retessellates geometry. The fixed lookup is a
deterministic resampling policy for arbitrarily many normalized stops; every
input stop contributes to the LUT.

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
