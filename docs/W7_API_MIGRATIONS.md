# W7 API migrations

W7 completed the controls/Material presentation-ownership cleanup without a
public facade rewrite. Most application code keeps the same imports through
`incular_material`, `incular_material::prelude`, and
`incular_material::components`.

## ControlTheme palette storage

`ControlTheme` now has one authoritative semantic color store:
`ControlTheme::colors`.

The former independently writable `palette` field was removed because two
stored copies could diverge. Existing palette vocabulary remains available:

- `PaletteTokens` remains a type alias for the semantic color tokens.
- Read palette-compatible values with `theme.palette()`.
- Replace palette-compatible values with `theme.with_palette(tokens)`.
- Code that intentionally mutates individual tokens should mutate
  `theme.colors`.

`with_palette` no longer rebuilds typography. Explicit typography therefore
survives color-token replacement.

## Material implementation ownership

The historical private implementation buckets `component_impl`, `p0_controls`,
and `foundation` were retired. They were not supported application import
paths. Their implementations now live with their owners:

- `theme/` — Material state properties, component themes and `ThemeData`.
- `inputs/` — decoration plus Material text/form fields.
- `selection/` — checkbox, radio, switch and slider adapters.
- `surfaces/` — Material/ink/card/divider/avatar surface policy.
- `lists/` — list tiles and chips.
- `navigation/` — navigation bars and tabs.
- `app_shell/` — application shell, AppBar/Scaffold and drawers/rails.
- `feedback/` — dialogs, snackbars/tooltips and progress indicators.
- `buttons/` and `menus/` retain their family ownership.

The root Material facade, prelude and complete `components` export surface stay
the compatibility boundary. Source contributors should use the owner modules
above rather than recreating generic staging buckets.

## Behavioral compatibility notes

Several settings that previously compiled without reaching execution now have
real retained behavior. This is a correctness change rather than a call-site
migration: custom control visuals retain the same action/focus/semantics owner;
slider and range-slider drags use press-origin displacement; `TabBarView` uses
measured viewport geometry; `MenuAnchor` executes its clip/constraint/root
overlay/animation policies; progress indeterminate motion is retained; and
`AnimatedTheme` honors duration and retargets from its presented value.

W7 does not claim native-platform verification.
