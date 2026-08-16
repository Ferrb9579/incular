# incular-painting

Owns compact ordered `DisplayList` and `PaintCommand` values. It depends on
renderer-neutral core/assets types and contains no GPU state. `GlyphRun` holds
the selected font and shaped glyph positions rather than characters, so a
backend can rasterize/draw without reshaping. Render objects cache local lists;
backends must preserve command order, transforms, and rectangular clips when
lowering rectangle and glyph operations.

`PaintCommand::Image` carries an `ImageHandle`, a pixel-space source rectangle,
and a local logical destination rectangle. It follows normal painter order,
transforms, and clipping without exposing GPU resources.

Phase 4 adds `LayerTree`: stable generational `Picture`, `Transform`, and
`ClipRect` layers. Pictures retain `Arc<DisplayList>` payloads, transforms and
clips change independently, and flattening produces only the transient ordered
submission stream. Bounds-driven rectangular culling is conservative.

## Coordinate spaces and transform composition

Pictures contain local logical commands only. Parent-relative layout placement
and compositor-only movement are transform layers, accumulated by `flatten` to
world logical coordinates exactly once. Clip rectangles are transformed into
that same world space before culling. `flattened_pictures()` provides the local
bounds, world bounds, and active clip observed during the latest flattening for
CPU regression tests; physical DPI conversion belongs exclusively to a backend.

## Retained opacity isolation

`LayerTree` also retains an `Opacity` isolation layer. It owns only a
normalized alpha value and a stable content generation; it never owns a GPU
texture or render pass. Flattening emits `PushOpacity`/`PopOpacity` around the
child stream together with conservative subtree bounds and a generation. A
backend can therefore render the child stream into a tight transparent target
and composite that result once, while an alpha-only change remains a
compositor update. A clip inside the layer is part of the cached child stream;
a clip above it remains active when the group quad is composited.

## Gaussian effects

`GaussianBlur` and `DropShadowEffect` are renderer-neutral descriptions. Their
sigma values are logical pixels and are normalized (`< 0` and non-finite values
become zero); the backend multiplies them by the active DPI scale before
building a kernel. `LayerTree` emits `PushBlur`/`PushDropShadow` and a matching
`PopEffect`, while the source subtree's generation remains independent of its
effect parameters. This lets a compositor update sigma, shadow offset, or
shadow color without repainting the child display list. A zero-sigma blur is an
identity and does not require an isolation target.

Bounds use a finite three-sigma support: a blur expands a source by
`3 * sigma_x/y`, and a shadow unions the source with the offset, expanded source
rectangle. These are conservative logical bounds; physical target dimensions
are rounded outward by the backend. Effects do not change hit testing or
semantics geometry, and they contain no renderer or GPU resources.

Effect layers preserve command order and nesting. Clips inside an effect are
rendered into its isolated source; clips above the effect remain active when
the final result is composited. The source is therefore still an ordinary
retained display-list subtree rather than a special box-only shadow primitive.

## Color filters, blend modes, and effect chains

`ColorFilter` is a renderer-neutral 4×5 matrix over normalized straight RGBA.
`apply` uses zero RGB for transparent input, clamps/sanitizes all results, and
zeros RGB when the output alpha is zero. The GPU converts retained
premultiplied pixels to this straight representation, applies the matrix, and
premultiplies the result again. `identity`, `grayscale`, `sepia`, `brightness`,
`contrast`, `saturate`, `invert`, and `opacity` cover common adjustments.
`ColorFilter::then` composes `B(A(x))` as `B * A`, including the affine bias.

`EffectChain` is an ordered list of single-input stages. `optimized` fuses only
adjacent color matrices; blur remains a separate spatial stage, so
`color → blur → color` cannot be reordered or collapsed. Drop shadows remain a
separate multi-output layer because they emit both a shadow and the original
source. Pure color and blend stages do not expand visual bounds.

`BlendMode` defines Porter–Duff and separable artistic modes over normalized
premultiplied pixels. The public `blend_premultiplied` reference follows the
alpha-aware equation used by the GPU and is safe for zero-alpha inputs. A
blend-mode property belongs to the final composite state: it does not change
the source generation or invalidate upstream filter pixels.
