# incular-material

## Architecture and support

| Contract | Status |
| --- | --- |
| Ownership | Material presentation composed from controls and neutral widgets. |
| API class | application; re-exports and path overrides follow the [architecture contract](../../system-design/ARCHITECTURE.md). |
| Support | Curated subset; parity manifests record omissions. |

Material-style components for Incular.

Pass `SliverAppBar` as a `Sliver` to `CustomScrollView` for retained scrolling.
Explicit `expanded_height`/`collapsed_height` values describe the total header,
including bottom content. The bottom is measured first and the toolbar fills
the remaining height. Negative/non-finite heights become zero and the expanded
extent is at least the collapsed extent. Without a collapsed height the toolbar
height is used; without an expanded height both extents are equal. With neither
height set, the header measures its natural content instead.

Resizing supports all `pinned`/`floating` combinations through neutral Widgets
geometry. `stretch(true)` maps to the neutral overscroll policy for both
explicit and naturally measured headers through one retained Flex structure:
the bottom is measured first under real cross constraints (including custom
`LayoutBuilder` bottoms and wrapping text, whose actual height may differ
from any static hint) and the toolbar fills the remainder when bounded or
resolves its natural height when unbounded. No bottom size is guessed and
stretching is never silently disabled. Stretching needs bouncing physics and
a leading position; clamping and non-leading headers keep existing behavior.
Conversion to an ordinary `Widget` keeps the existing static box presentation;
stretching and snapping policies execute through the retained sliver path.

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

W7 organizes implementation by behavior owner: `theme`, `inputs`, `selection`,
`surfaces`, `lists`, `navigation`, `app_shell`, `feedback`, `buttons`, and
`menus`. The former private staging buckets `component_impl`, `p0_controls` and
`foundation` are retired. Root/prelude/components imports remain the supported
application boundary.

The W7 source-derived API ledger covers 221 Material public symbols and 1,299
accepted options, including generated builders. Public-surface drift must be
paired with execution/value semantics and regression evidence before its
snapshot is intentionally regenerated.

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
