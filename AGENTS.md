# Incular repository guidance

## Crate responsibilities

- `incular-core`: Shared primitives such as geometry, colors, IDs, events, and errors.
- `incular-layout`: Constraints, measurement, positioning, alignment, and layout algorithms.
- `incular-assets`: Loading, caching, and lifecycle management for images, fonts, icons, and shaders.
- `incular-painting`: Renderer-independent drawing commands, paths, clipping, transforms, and display lists.
- `incular-text`: Font selection, shaping, glyphs, metrics, wrapping, and text layout.
- `incular-animation`: Durations, curves, tweens, interpolation, and animation values.
- `incular-widgets`: Widget tree, composition, state, lifecycle, events, layout, painting, and built-in widgets.
- `incular-accessibility`: Semantic trees, roles, labels, focus, actions, and screen-reader integration.
- `incular-runtime`: Application lifecycle, scheduling, rebuilding, input dispatch, animation ticking, and frame coordination.
- `incular-platform`: Shared window, event-loop, input, clipboard, DPI, and native-handle abstractions.
- `incular-android`: Android activity lifecycle, input, surfaces, system UI, back handling, and native resources.
- `incular-ios`: iOS lifecycle, touch and gesture input, safe areas, system UI, accessibility, and native resources.
- `incular-linux`: Linux windowing, input, display scaling, clipboard, accessibility, and native resources.
- `incular-macos`: macOS lifecycle, windows, menus, input, display scaling, accessibility, and native resources.
- `incular-windows`: Windows lifecycle, input, display scaling, menus, accessibility, and native resources.
- `incular-wgpu`: GPU initialization and rendering of painting commands using `wgpu`.
- `incular-macros`: Procedural macros for reducing framework boilerplate.
- `incular`: Public facade that re-exports the framework API and manages features.

## Structure rules

- The workspace is organized under `crates/*`.
- Each crate should contain `Cargo.toml`, `README.md`, and `src/lib.rs`.
- Keep built-in widgets in `incular-widgets`; do not create a separate crate for them.
- Keep platform-specific APIs in their matching native crate.
- There is no web target or web crate planned.
- Keep renderer-independent APIs separate from `incular-wgpu`.
- Avoid introducing dependency cycles between crates.
- All test code must live in a `tests/` directory. Use `crates/<crate>/tests/`
  for crate tests, the repository-root `tests/` for workspace integration
  tests, and `examples/<example>/tests/` for example tests. Do not add
  `#[cfg(test)]` modules, test helpers, or test-only implementations under
  `src/` or directly beside an example's `main.rs`.

## Validation

Run these checks after workspace or crate changes:

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test-constrained
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
