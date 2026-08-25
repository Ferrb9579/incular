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
