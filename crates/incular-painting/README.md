# incular-painting

Owns compact ordered `DisplayList` and `PaintCommand` values. It depends on
renderer-neutral core/assets types and contains no GPU state. `GlyphRun` holds
the selected font and shaped glyph positions rather than characters, so a
backend can rasterize/draw without reshaping. Render objects cache local lists;
backends must preserve command order, transforms, and rectangular clips when
lowering rectangle and glyph operations.

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
