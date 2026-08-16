# incular-assets

Asset identity, loading, caching, and resource lifecycle foundations for
Incular. It will cover images, fonts, icons, shaders, and other application
resources without owning a particular renderer.

Raster images use immutable cheap-clone `ImageHandle`s. A handle owns a stable
`ImageId`, source metadata, and decoded straight-alpha RGBA8 pixels; it never
owns a GPU object. PNG, JPEG, and WebP decode synchronously at load time.
