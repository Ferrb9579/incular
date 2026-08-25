# Flutter package-boundary audit

This audit records the public names that were easy to put in the wrong Incular
crate. It is intentionally separate from the generated API inventories so a
future parity pass does not reintroduce invented core widgets.

## Core `widgets` layer

`incular-widgets` contains renderer-neutral primitives and retained behavior:

| Flutter concept | Incular public name | Notes |
| --- | --- | --- |
| `EditableText` | `incular_widgets::EditableText` | Core editor; call `.multiline(true)` for multiline editing. |
| gesture/action composition | `incular_widgets::GestureDetector` | Renderer-neutral interaction primitive. Flutter has no generic core `Button`; styled button families belong to Material or another design system. |
| `SelectableRegion` and selection containers | `incular_widgets::{SelectableRegion, SelectionContainer, SelectionListener}` | Selection mechanics stay core. |
| `Form`, `FormField`, `RawAutocomplete` | `incular_widgets` | Core form state and builder primitives. |

There is deliberately no core `Button`, `TextField`, `TextArea`,
`TextFormField`, `Autocomplete`, `SelectableText`, or `SelectionArea`
descriptor. `TextArea` is not a Flutter widget; use a Material `TextField` with
`max_lines`/`multiline`.

## Material layer

`incular-material` owns the Flutter Material vocabulary and composes the core
primitives with the existing themed control implementation:

| Flutter Material concept | Incular public name |
| --- | --- |
| `ElevatedButton` | `incular_material::ElevatedButton` |
| `FilledButton` | `incular_material::FilledButton` |
| `OutlinedButton` | `incular_material::OutlinedButton` |
| `TextButton` | `incular_material::TextButton` |
| `MaterialButton` | `incular_material::MaterialButton` |
| `RawMaterialButton` | `incular_material::RawMaterialButton` |
| `TextField` | `incular_material::TextField` |
| `TextFormField` | `incular_material::TextFormField` |
| `SelectableText` | `incular_material::SelectableText` |
| `SelectionArea` | `incular_material::SelectionArea` |
| `Autocomplete<T>` | `incular_material::Autocomplete<T>` |

The crate also exposes the themed component modules (dialogs, menus, drawers,
popups, sliders, tabs, progress, scroll areas, toggles, toolbars, and feedback)
under the Material package boundary. The facade enables this layer with the
`material` feature, which is part of the default desktop feature set.

## Intentional Incular extensions

These names are useful framework features, but are not claimed as one-to-one
Flutter widget classes: `SplitView`, `VirtualList`, `PathView`, retained
painting effects (`Blur`, `DropShadow`, `Blend`), and the platform/devtools
diagnostic surfaces. Their parity entries describe the closest Flutter
protocol or explicitly mark the extension.

## Follow-up audit points

The package split does not hide behavior gaps. In particular, several
environment wrappers still need runtime propagation tests (`Directionality`,
`DefaultTextStyle`, `IconTheme`, `ScrollConfiguration`,
`PrimaryScrollController`, and `TickerMode`), and `TextFormField` validation
callbacks should eventually be connected to the retained `Form` model rather
than only stored on the descriptor. `TextField::max_lines` currently selects
the retained primitive's multiline mode; line-count caps and overflow scrolling
are a separate follow-up. These are implementation follow-ups, not reasons to
put Material names back into the core widget crate.
