# incular-rendering

`incular-rendering` owns Incular's renderer-neutral presentation boundary:
paths, brushes, ordered display lists, canvas recording, and retained
compositor layers. It intentionally contains no window, surface, or GPU state.

Backends such as `incular-wgpu` consume these values and remain responsible for
physical DPI conversion, resource allocation, and submission. The legacy
`incular-painting` crate re-exports this API so existing applications retain a
stable import path while new integrations should depend on this crate directly.
