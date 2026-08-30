# incular-material

Material-style components for Incular.

The `incular-widgets` crate contains renderer-neutral Flutter `widgets`-layer
primitives. This crate contains Material-layer controls such as concrete
button variants, `TextField`, `TextFormField`, selection controls,
autocomplete, dialogs, menus, navigation, overlays, progress, tabs, and
feedback surfaces.

The implementation currently reuses the platform-neutral styled controls from
`incular-controls`; the public package boundary is intentional so applications
can depend on a Material vocabulary without making the core widget crate
pretend that Flutter has a generic `Button` or `TextArea` widget.
Flutter's low-level Material entry point is available here as
`RawMaterialButton`; new code should prefer a concrete Material button variant.
Multiline editing uses `TextField::max_lines`/`TextField::multiline`, just as it
does in Flutter.

Use `incular_material::prelude` for the common application surface or
`incular_material::components` when importing the complete component
vocabulary in one place. The module re-exports dialogs, menus, drawers,
popups, sliders, tabs, progress, scroll areas, toggles, toolbars, and feedback
alongside the Flutter-named text and button components.

## Flutter 3.47.1 parity audit

The pinned export graph and evidence live in
`specs/flutter_material_3471_parity.jsonl`. Task 25's generated P0 projection
is `specs/P0_MATERIAL_3471.jsonl`, with counts in
`specs/P0_MATERIAL_3471_SUMMARY.json`; regenerate both with
`python3 tools/generate_material_p0.py`. The generated status fields keep
deferred platform, localization, and renderer-specific behavior explicit
instead of presenting a type-only port as complete.

For a visual smoke test of the public API run under the repository's low-memory
serial build policy:

```text
ulimit -v 8388608
export CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 RUST_TEST_THREADS=1
cargo run -p incular --example material_gallery --features material
cargo run -p incular --example material_workbench --features material
```

That gallery exercises the theme scope, button families, chips, list tiles,
navigation, progress indicators, Material surfaces, menus, dialogs, tabs,
feedback, and decorated text input. `examples/material_workbench/main.rs` is the
Material public-API application smoke test; `examples/workbench/main.rs` remains
the base-Widgets workbench.
