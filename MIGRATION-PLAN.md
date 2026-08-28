# Incular Typed-Builder Architecture Migration Plan

## Objective

Migrate Incular’s public declarative widget APIs toward one consistent architecture based on:

- `TypedBuilder`
- explicit `Default`
- `new() -> Self::default()`
- optional properties using `Option<T>`
- composable children represented by `Widget`
- existing `impl From<Component> for Widget` lowering
- canonical Incular types instead of duplicate placeholder types

The pasted example is an API pattern, not a request to duplicate its supporting types.

## Non-negotiable child rule

Generic child fields must use `Widget`, never a component-specific type such as `Card`.

```rust
#[derive(Clone, Debug, TypedBuilder)]
pub struct Container {
    #[builder(default, setter(strip_option, into))]
    pub child: Option<Widget>,

    #[builder(default, setter(strip_option))]
    pub width: Option<f32>,

    #[builder(default, setter(strip_option))]
    pub height: Option<f32>,
}
```

These must all be valid:

```rust
Container::builder()
    .child(Text::new("Hello"))
    .build();

Container::builder()
    .child(Card::new(Text::new("Card")))
    .build();

Container::with_child(PrimaryButton::new("Save"));
```

For struct literals, explicit conversion is required:

```rust
Container {
    child: Some(Text::new("Hello").into()),
    ..Default::default()
}
```

Rust cannot implicitly convert `Option<Card>` into `Option<Widget>`, so update existing struct-literal call sites accordingly.

## Canonical type mapping

Do not create duplicate versions of these types from the pasted example.

| Example type | Incular type |
|---|---|
| `Widget` | `incular_widgets::Widget` |
| `EdgeInsets` | `incular_config::EdgeInsets` |
| `Alignment` | `incular_config::Alignment` |
| `Color` | `incular_core::Color` |
| `BoxConstraints` | `incular_config::Constraints` |
| `ClipBehavior` | `incular_config::Clip` |
| `Decoration` | Existing `BoxDecoration`, `Brush`, `Border`, and radius types |
| `Card` | A concrete widget that converts into `Widget` |

## Standard component shape

For each public declarative widget:

```rust
use typed_builder::TypedBuilder;

#[derive(Clone, Debug, TypedBuilder)]
pub struct Component {
    #[builder(default, setter(strip_option, into))]
    pub child: Option<Widget>,

    #[builder(default, setter(strip_option))]
    pub width: Option<f32>,

    #[builder(default)]
    pub children: Vec<Widget>,

    #[builder(default = true)]
    pub enabled: bool,
}

impl Default for Component {
    fn default() -> Self {
        Self {
            child: None,
            width: None,
            children: Vec::new(),
            enabled: true,
        }
    }
}

impl Component {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self {
            child: Some(child.into()),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }
}

impl From<Component> for Widget {
    fn from(value: Component) -> Self {
        // Lower into existing Incular primitives.
    }
}
```

## Builder attribute rules

Use these conventions consistently:

```rust
#[builder(default, setter(strip_option))]
field: Option<T>,
```

For values that should accept `Into<T>`:

```rust
#[builder(default, setter(strip_option, into))]
child: Option<Widget>,
```

For ordinary defaultable values:

```rust
#[builder(default)]
field: Vec<Widget>,
```

For non-defaulted required values, omit `default`:

```rust
#[builder(setter(into))]
child: Widget,
```

For booleans with non-zero defaults:

```rust
#[builder(default = true)]
enabled: bool,
```

Do not use `derive(Default)` when the component has semantic defaults that differ from Rust defaults. Keep a manual `Default` implementation as the authoritative definition.

## API compatibility rules

Preserve existing APIs during migration:

- Keep `new()`.
- Keep `with_child(...)`.
- Keep existing fluent setters.
- Add `builder()` through `TypedBuilder`.
- Preserve existing `From<Component> for Widget`.
- Preserve existing layout and paint behavior.
- Do not expose runtime state, controllers, caches, or internal callbacks as public configuration fields.
- Public plain-data fields may be made `pub` where that matches the requested architecture and does not violate invariants.
- Do not remove old constructors until a separate compatibility decision is made.

A field should remain private when it contains:

- mutable runtime state
- `Rc`/`Arc` callback storage
- internal IDs
- cache handles
- renderer objects
- lifecycle state
- invariants that cannot be maintained through public struct literals

## Migration phases

### Phase 1: Foundation

Files:

- `Cargo.toml`
- `crates/incular-widgets/Cargo.toml`
- `crates/incular-controls/Cargo.toml`
- `crates/incular-material/Cargo.toml`
- other crates that use `TypedBuilder`

Tasks:

1. Add a workspace-level `typed-builder` dependency compatible with the project MSRV.
2. Add it only to crates that derive `TypedBuilder`.
3. Document the conventions above.
4. Do not add typed-builder to runtime-only crates unnecessarily.
5. Confirm no dependency cycles are introduced.

### Phase 2: Container pilot

Primary file:

- `crates/incular-widgets/src/layout/container.rs`

Tasks:

1. Convert `Container` to the standard architecture.
2. Change the child storage type to:

   ```rust
   pub child: Option<Widget>
   ```

3. Add `TypedBuilder`.
4. Preserve existing fields and lowering order:
   - child
   - alignment
   - padding
   - decoration
   - constraints
   - margin
   - transform
   - clipping
5. Keep `Container::new()`, `empty()`, `with_child()`, and fluent setters.
6. Ensure builder setters accept `Text`, `Card`, buttons, and arbitrary custom widgets.
7. Do not add fields such as `is_anti_alias` unless Incular already supports their behavior.

Tests to add or update:

- `Container::new()` equals `Container::default()`.
- Empty builder produces the same defaults.
- `.child(Text::new(...))` compiles.
- `.child(Card::new(...))` compiles.
- Struct literal requires `.into()` for the child.
- Width, height, alignment, padding, margin, decoration, and clipping still lower correctly.
- Layout remains content-sized when no explicit size is provided.

### Phase 3: Core layout widgets

Files:

- `crates/incular-widgets/src/layout/basic.rs`
- `crates/incular-widgets/src/layout/flex.rs`
- `crates/incular-widgets/src/layout/stack.rs`
- `crates/incular-widgets/src/layout/table.rs`
- `crates/incular-widgets/src/layout/wrap.rs`

Migrate compositional widgets such as:

- `Padding`
- `Align`
- `Center`
- `SizedBox`
- `ColoredBox`
- `ConstrainedBox`
- `LimitedBox`
- `OverflowBox`
- `UnconstrainedBox`
- `FractionallySizedBox`
- `AspectRatio`
- `FittedBox`
- `Visibility`
- `Offstage`
- `ClipRect`
- `ClipRRect`
- `ClipOval`
- `IntrinsicWidth`
- `IntrinsicHeight`
- `RotatedBox`
- `KeyedSubtree`
- `Row`
- `Column`
- `Flex`
- `Stack`
- `Positioned`
- `IndexedStack`
- `Wrap`
- `Table`

Rules:

- A single generic child is `Widget` or `Option<Widget>`.
- Multiple children are stored as `Vec<Widget>`.
- Builder inputs should accept `impl Into<Widget>` where practical.
- Preserve existing required-child semantics. Do not make a required child optional just to simplify the builder.
- Keep layout-builder closures and runtime callbacks private if they cannot safely implement `Default`.

### Phase 4: Controls

Files include:

- `crates/incular-controls/src/button.rs`
- `containers.rs`
- `field.rs`
- `text_input.rs`
- `checkbox.rs`
- `switch.rs`
- `radio.rs`
- `slider.rs`
- `select.rs`
- `combobox.rs`
- `tabs.rs`
- `tooltip.rs`
- `popover.rs`
- `dialog.rs`
- `menu.rs`
- `scroll_area.rs`

Tasks:

1. Convert public control descriptors to the standard builder shape.
2. Use `Widget` for generic children and content.
3. Use `Vec<Widget>` for action/content collections.
4. Keep callback fields optional and default to `None`.
5. Preserve state handling and controller behavior.
6. Keep button visual invariants:
   - primary buttons do not receive an accidental default border
   - ordinary buttons shrink-wrap their content
   - explicit fixed sizes still work
   - focus rings remain separate from decorative borders
7. Add builder compile tests for each major control family.

### Phase 5: Material components

Files:

- `crates/incular-material/src/components.rs`
- `buttons.rs`
- `feedback.rs`
- `menus.rs`
- `p0_controls.rs`
- `app_shell.rs`
- `foundation.rs`

Tasks:

1. Migrate public Material component descriptors.
2. Convert generic child fields to `Widget`.
3. Preserve Material-specific defaults and theme resolution.
4. Keep `impl From<MaterialComponent> for Widget`.
5. Preserve shared heap-backed theme construction.
6. Do not move large `ThemeData` values back onto the native stack.
7. Migrate `ThemeData` last. If a generated builder becomes too large or harms compile/runtime behavior, use a dedicated patch builder while keeping the same field/default conventions.
8. Keep Material buttons separate from neutral controls where their sizing semantics intentionally differ.

### Phase 6: Navigation and overlays

Primary file:

- `crates/incular-navigation/src/lib.rs`

Review and migrate:

- `Page`
- `Route`
- `PageRouteBuilder`
- `OverlayEntry`
- `Dialog`
- `BottomSheet`
- route transition descriptors

Rules:

- Route/page child content must be `Widget`.
- Route names and required fields remain required builder fields.
- Callback and transition closures remain optional or private.
- Restoration IDs and navigation state must not be confused with declarative widget configuration.

### Phase 7: Examples and public facade

Update:

- `examples/*.rs`
- `crates/incular/src/lib.rs`
- public prelude modules
- API contract tests

Tasks:

1. Update examples to demonstrate both:
   - `Component::new()` plus fluent setters
   - `Component::builder()`
2. Replace component-specific child assumptions with generic widgets.
3. Add at least one example showing a `Container` containing:
   - `Text`
   - a control button
   - a Material component
4. Keep the counter example using the compact primary button behavior.
5. Do not introduce duplicate example-only `Card`, `Color`, `Alignment`, or `Widget` types.

## What must not be migrated

Do not blindly derive `TypedBuilder` for:

- `Widget`
- `WidgetKind`
- `Element`
- retained tree records
- render commands
- animation controllers
- signals
- focus managers
- gesture arenas
- restoration stores
- runtime/window state
- GPU resources
- internal cache structures
- enums that are not configuration structs

These are implementation/runtime types, not declarative component descriptors.

## Required validation after every migration batch

Run targeted checks first:

```text
cargo fmt --all -- --check
cargo check -p <changed-crate>
cargo test -p <changed-crate>
cargo clippy -p <changed-crate> --all-targets --all-features -- -D warnings
```

After all phases:

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo build -p incular --examples
```

Also run the native example smoke pass and capture screenshots, with at least five seconds of startup time before capturing the counter and representative galleries.

## Definition of done

The migration is complete when:

- Generic container children are stored as `Option<Widget>`.
- Builders accept arbitrary widgets through `Into<Widget>`.
- Public declarative components consistently provide:
  - `TypedBuilder`
  - `Default`
  - `new()`
  - existing compatibility setters
  - `From<Component> for Widget`, where applicable
- Defaults are explicit and tested.
- No duplicate supporting types were introduced.
- Existing layout, paint, interaction, theme, and accessibility behavior remains intact.
- The counter button remains compact and free of the unwanted border/ring.
- All workspace validation commands pass.
- All registered examples compile, launch, and produce screenshots.
- The sub-agent reports migrated files, intentionally deferred types, compatibility changes, and any remaining exceptions.