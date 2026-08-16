# incular-text

Renderer-independent system-font selection, OpenType shaping, Unicode line
breaking, metrics, baselines, and a bounded logical text-layout cache. It emits
font handles plus positioned glyph runs; it does not own GPU state or rasterize
glyphs. Layout cache keys use metric inputs, not foreground color or display
DPI: a 16 logical-pixel paragraph remains a 16 logical-pixel paragraph at all
window scales. `incular-wgpu` independently rasterizes the resulting glyph IDs
at physical DPI and caches those physical mask variants, preserving logical
layout across DPI changes.

The renderer uses each logical glyph run to request a DPI-aware physical
grayscale mask from its cached `fontdue` font. Raster cache identity is owned
by the renderer; shaping, advances, clusters, logical baselines, wrapping, and
caret geometry remain here. Scroll and compositor transforms are renderer
placement only and never alter the shaped text.

## Font resolution and fallback

`TextEngine` owns one `fontdb` database for system discovery and immutable application registrations. `SystemUi` deliberately tries common UI sans families (`Noto Sans`, `Cantarell`, `Inter`, `DejaVu Sans`, `Liberation Sans`) before `fontdb`'s generic sans mapping. A named family remains first priority, followed by application fallback families, then a cached script-indexed system search.

Resolution operates on Unicode grapheme clusters before Rustybuzz shaping. Combining marks, variation selectors, and ZWJ sequences stay in one coverage decision; Common/Inherited punctuation inherits surrounding script context. The selected physical face shapes its own run, so its advances are authoritative. Lines contain multiple `GlyphRun`s with one shared baseline; flattened geometry remains available for caret, hit testing, and selection. Font bytes plus collection face index form the stable `FontId`; font registration advances the database generation and invalidates resolution/layout caches. Color-only glyph formats remain a future R8 boundary.
