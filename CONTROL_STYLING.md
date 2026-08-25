# Styling controls

Use `ControlThemeScope` at the application or feature boundary:

```rust
ControlThemeScope::new(
    ControlTheme::light(),
    Button::new("Save"),
)
```

Nested scopes shadow only their descendants. A control should not call
`ControlTheme::default()` while being converted into a `Widget`.

State-aware values use the shared resolver:

```rust
let style = ButtonStyle::new().background_states(
    StateColor::new(theme.colors.surface_variant)
        .hovered(theme.colors.surface_elevated)
        .pressed(theme.colors.surface_active)
);
```

Keep behavior and appearance separate. Use `ControlState` for interaction
flags and `StateValue`/`StateTable` for reusable state resolution; use
component theme tokens for geometry and semantic palette roles.

