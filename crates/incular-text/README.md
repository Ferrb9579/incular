# incular-text

Renderer-independent system-font selection, OpenType shaping, Unicode line
breaking, metrics, baselines, and a bounded logical text-layout cache. It emits
font handles plus positioned glyph runs; it does not own GPU state or rasterize
glyphs. Layout cache keys use metric inputs, not foreground color or display
DPI. `incular-wgpu` rasterizes the resulting glyph IDs at physical DPI.
