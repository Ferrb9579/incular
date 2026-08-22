# incular-text

Renderer-independent font selection, OpenType shaping, Unicode line breaking,
metrics, baselines, and a bounded logical text-layout cache. Parley 0.7,
Fontique, and HarfRust are the single authority for discovery, fallback, bidi,
and shaping. This crate emits font handles plus positioned glyph runs; it does
not own GPU state or rasterize glyphs. Layout cache keys use metric inputs, not
foreground color or display DPI: a 16 logical-pixel paragraph remains a 16
logical-pixel paragraph at all window scales. `incular-wgpu` independently
rasterizes the resulting glyph IDs at physical DPI and caches those physical
mask variants, preserving logical layout across DPI changes.

The renderer uses each logical glyph run to request a DPI-aware physical
grayscale mask from its cached `fontdue` font. Raster cache identity is owned
by the renderer; shaping, advances, clusters, logical baselines, wrapping, and
caret geometry remain here. Scroll and compositor transforms are renderer
placement only and never alter the shaped text.

## Font resolution and fallback

`TextEngine` owns one Parley `FontContext` and reusable `LayoutContext`.
Fontique discovers system and registered application fonts; Parley resolves
generic, named, and supplied fallback families, applies coverage-aware fallback,
and shapes/bidi-reorders/runs line breaking in one layout pass. Each physical
face produces a renderer-neutral glyph run with a stable byte-plus-collection
face `FontId`. Registration advances the collection generation and invalidates
the bounded layout cache. Fontdue remains only the WGPU R8 alpha-mask
rasterizer; color-glyph formats are outside this path.

## Paragraph bounds and overflow

`Text` and `RichText` expose `soft_wrap`, `max_lines`, and `TextOverflow`
without leaking Parley layout types. The same `TextLayoutOptions` reaches the
retained widget tree. `Clip` restricts retained paint to the laid-out bounds,
`Visible` preserves the full logical layout, and `Ellipsis` shapes U+2026 with
Parley after grapheme-safe truncation, including mixed-direction text.
