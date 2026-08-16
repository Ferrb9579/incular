# incular-image

Renderer-neutral raster image resources for Incular. This crate owns immutable
decoded RGBA8 images, source metadata, byte/file decoding, and the
application-owned decode cache. It intentionally owns no GPU texture, painter,
or widget state.

`ImageHandle` is cheap to clone and identified by a stable `ImageId`; renderers
use that identity to retain backend resources while widgets and display lists
remain renderer independent. PNG, JPEG, and WebP decoding happens at load
time, and generated RGBA8 pixels are validated before a handle is created.
