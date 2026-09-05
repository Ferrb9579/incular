# Incular repository guidance

## Crate responsibilities

Follow [the architecture contract](docs/ARCHITECTURE.md), its API classes and
[accepted decisions](docs/ARCHITECTURE_DECISIONS.md). Historical parity plans do
not override these boundaries. The tested dependency allow-list and complete
support matrix are in `specs/architecture.json`.

- `incular`: Feature-controlled public facade and application preludes.
- `incular-accessibility`: AccessKit and mobile semantic projections and action translation.
- `incular-android`: Android semantic adapter for a native host.
- `incular-animation`: Curves, interpolation, tweens and animation physics.
- `incular-assets`: Font identities and shared font bytes.
- `incular-config`: Shared constraints, alignment, insets, localization and application policies.
- `incular-controls`: Themed Incular controls and visual slots over neutral widgets.
- `incular-core`: Geometry, identity, input, interpolation and lower-level context values.
- `incular-desktop`: Shared Winit host, WGPU frame coordination and native service integration.
- `incular-devtools`: Opt-in diagnostics transport and runtime command bridge.
- `incular-devtools-protocol`: Versioned diagnostics messages shared with tooling.
- `incular-gestures`: Pointer recognition, gesture arbitration, keyboard and focus mechanisms.
- `incular-image`: Raster image identities, decoding and CPU image caches.
- `incular-ios`: iOS semantic adapter for a native host.
- `incular-layout`: Layout algorithms over measured sizes; config values are exact re-exports.
- `incular-linux`: Linux desktop entry and native services over the shared desktop host.
- `incular-macos`: macOS desktop entry and native services over the shared desktop host.
- `incular-macros`: Procedural macros for framework composition and builders.
- `incular-material`: Material presentation composed from controls and neutral widgets.
- `incular-navigation`: Application route stacks, routing, restoration and route composition.
- `incular-painting`: Pure compatibility re-export of incular-rendering.
- `incular-platform`: Portable native IDs, events, capabilities, commands, metrics and errors.
- `incular-rendering`: Renderer-neutral commands, paths, canvas and retained compositor layers.
- `incular-runtime`: Application lifecycle, scheduling, reactive dispatch, tasks and frame coordination.
- `incular-scroll`: Scroll controllers, physics, metrics, extent indexing and geometry.
- `incular-semantics`: Platform-neutral semantic nodes, roles, labels, state and actions.
- `incular-text`: Font selection, shaping, metrics, text layout and editing values.
- `incular-wgpu`: Owned native surfaces, GPU resources and execution of rendering commands.
- `incular-widgets`: Neutral widget composition and retained build/layout/paint/input/semantics.
- `incular-windows`: Windows desktop entry and native services over the shared desktop host.

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
