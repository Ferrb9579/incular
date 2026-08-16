# incular-wgpu

Owns Incular's native `wgpu` surface, retained rectangle/text/image pipelines, static
unit quad, and geometrically grown instance buffers. It consumes ordered
renderer-neutral display lists without leaking `wgpu` types upstream.

Text shaping remains in `incular-text`. This crate keys grayscale glyph masks
by font ID, glyph ID, and physical raster size, stores them in retained
page-growing `R8Unorm` atlas textures, and uploads only a missing glyph region.
The text pipeline samples coverage with straight-alpha source-over blending;
foreground color is per instance, so color-only changes reuse masks. Logical
layout stays DPI-independent while raster masks scale with the device factor.

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
