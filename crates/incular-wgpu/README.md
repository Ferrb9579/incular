# incular-wgpu

Owns Incular's native `wgpu` surface, retained rectangle/text pipelines, static
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
