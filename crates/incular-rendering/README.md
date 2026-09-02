# incular-rendering

`incular-rendering` owns Incular's renderer-neutral presentation boundary:
paths, brushes, ordered display lists, canvas recording, and retained
compositor layers. It intentionally contains no window, surface, or GPU state.

Paths are retained as `kurbo::BezPath` values: Kurbo supplies tight quadratic
and cubic bounds plus winding queries, while `incular-wgpu` translates those
same elements directly to Lyon for tessellation. `incular_core::Transform`
uses Kurbo affine composition/inversion while retaining Incular's `f32`
`Offset` and `Rect` values at the public layout boundary.

Backends such as `incular-wgpu` consume these values and remain responsible for
physical DPI conversion, resource allocation, and submission. The legacy
`incular-painting` crate re-exports this API so existing applications retain a
stable import path while new integrations should depend on this crate directly.

Display lists can also carry balanced `SurfacePartitionId` markers. They are
renderer-neutral no-ops during ordinary in-view rendering, but a multi-surface
host can detach selected retained subtrees while preserving painter order and
nested partition ownership. This is the boundary used by desktop transient
surfaces: rendering owns partition identity/command separation, while native
window creation and GPU-surface policy remain outside this crate.
