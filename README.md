# Incular

Incular is an experimental Rust GUI library inspired by Flutter and built
around `wgpu`.

The repository is organized as a layered Cargo workspace:

- `incular-core` contains platform-independent foundation types.
- `incular-layout` contains renderer-independent layout primitives and algorithms.
- `incular-text` contains text layout and typography foundations.
- `incular-assets` contains asset identity, loading, and resource lifecycle foundations.
- `incular-animation` contains renderer-independent animation primitives.
- `incular-accessibility` contains accessibility semantics and tree foundations.
- `incular-macros` will contain procedural macros for Incular APIs.
- `incular-runtime` contains application lifecycle and scheduling foundations.
- `incular-painting` defines renderer-neutral painting concepts.
- `incular-widgets` will provide the widget, layout, and built-in component model.
- `incular-platform` provides shared platform abstractions.
- `incular-android` provides Android integration.
- `incular-ios` provides iOS integration.
- `incular-linux` provides Linux integration.
- `incular-macos` provides macOS integration.
- `incular-windows` provides Windows integration.
- `incular-wgpu` provides the `wgpu` rendering backend.
- `incular` is the public facade crate.

The project is currently at the workspace-scaffolding stage. Runtime behavior,
widgets, layout, and rendering APIs will be developed incrementally.
