# incular-material

## Architecture and support

| Contract | Status |
| --- | --- |
| Ownership | Material presentation composed from controls and neutral widgets. |
| API class | application; re-exports and path overrides follow the [architecture contract](../../docs/ARCHITECTURE.md). |
| Support | Curated subset; parity manifests record omissions. |

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

Material menus, popup menus, dropdowns, context-menu anchors, and tooltips do not
own a second popup-positioning implementation. They supply Material defaults to
the Widgets-layer `TransientPlacement` policy, including RTL submenu direction,
then rely on the runtime for cross-surface dismissal/focus. `MenuAnchor` treats
its trigger child as presentation and owns the single activation surface, so a
button-styled visual child cannot create a competing nested-button hit target.
Leaf selection propagates an explicit semantic close reason through the complete
retained submenu chain.

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
