# incular-controls

## Architecture and support

| Contract | Status |
| --- | --- |
| Ownership | Themed Incular controls and visual slots over neutral widgets. |
| API class | application; re-exports and path overrides follow the [architecture contract](../../docs/ARCHITECTURE.md). |
| Support | Available themed controls; shared behavior consolidates in F. |

Platform-neutral, styled, ready-to-use desktop UI controls and design tokens for the Incular framework.

## Overview

`incular-controls` provides a clean, modern, and accessible control palette built on top of `incular-widgets` primitives without introducing coupling or opinionated styling into the core framework.

### Architecture Separation
- `incular-widgets`: Layout primitives, raw gesture recognizers, semantic roles, unstyled rendering mechanics.
- `incular-controls`: Styled buttons, inputs, toggles, sliders, scrollbars, cards, dividers, and cohesive light/dark `ControlTheme` design tokens.
- `incular-material` is the Flutter-shaped application layer built on these
  controls. `incular-cupertino` and `incular-fluent` remain optional future
  sibling design-system packages.

## Quick Start

```rust
use incular::prelude::*;
use incular_controls::prelude::*;

fn build_ui() -> Widget {
    ControlThemeScope::new(ControlTheme::dark(), Column::new([
        Button::new("Standard Button").on_click(|| println!("Clicked")),
        PrimaryButton::new("Primary Action").on_click(|| println!("Primary")),
        TextField::new(TextEditingController::new()).placeholder("Type here..."),
        Checkbox::new(true).on_changed(|v| println!("Checked: {v}")),
    ]).spacing(12.0))
    .into()
}
```

`From<T> for Widget` is deferred: controls read the nearest
`ControlThemeScope` when retained builders materialize, so nested light/dark
scopes work without threading a theme through every constructor. Compound
parts live in the `checkbox`, `switch`, `radio`, `field`, `tabs`, `menu`,
`dialog`, `popover`, `tooltip`, `select`, and `overlay` modules. The repository
README and this crate README are the durable architecture overview.

`context_menu::Trigger` is a secondary-button trigger built directly on the
Widgets `Listener` metadata path. It receives the original `RawPointerEvent`
(including local position and button chord) and never converts a right click
into a primary tap. Menu open state and overlay presentation remain owned by the
normal popup/menu model rather than by a second pointer recognizer.
