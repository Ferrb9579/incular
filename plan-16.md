# Plan 16 - Make Widget an opaque public transport type with Flutter-like concrete APIs

## Goal

Stop exposing Incular's internal widget representation as public API. Before open source, establish a deliberate Flutter-like surface where concrete widgets are the user-facing types and `Widget` is an opaque erased/transport value used to compose the tree.

## Current problem

`Widget` exposes representation fields such as `pub key` and `pub kind`, and `WidgetKind` is public. This makes internal layout/render taxonomy part of the external compatibility surface and encourages users/tests to construct or mutate framework internals directly.

## Public API direction

Users should primarily author concrete widget types:

```rust
Padding::all(16.0)
    .child(
        Text::new("Hello")
            .style(style)
    )
```

or the established Rust equivalent that preserves Flutter naming/mechanics where practical.

Concrete widgets convert into the opaque `Widget` transport type through `From`/`Into<Widget>` or a similarly ergonomic, zero-surprise mechanism.

## Internal representation

After Plan 9, `Widget` should be a small shared descriptor handle. Its node/kind fields are private. `WidgetKind` and payload specs become `pub(crate)` unless a specific type is intentionally part of a public diagnostics/introspection API.

Public consumers may inspect only stable concepts through explicit APIs, for example:

- key;
- debug/widget type name;
- semantic/debug description where intentionally supported.

They must not mutate the internal kind or access retained/render implementation details.

## Flutter parity policy

Use Flutter concepts and names when they make sense in Rust:

- concrete widget structs;
- immutable builder-style configuration;
- `BuildContext` from Plan 13;
- explicit child/children composition;
- controllers for retained external state;
- keys as identity hints.

Do not mechanically expose Flutter internals that only exist because Dart has GC/dynamic dispatch. Preserve Rust ownership and performance advantages.

## Remove legacy convenience APIs

Audit static `Widget::foo(...)` constructors, aliases and compatibility helpers.

- keep only helpers that are genuinely the preferred API;
- migrate examples/tests to concrete widgets;
- delete redundant legacy constructors instead of forwarding them forever;
- remove public internal enums/IDs that are not intended extension points.

Because there are no external users to migrate, optimize for the clean first public API.

## Extension model

Define explicitly how third-party/custom widgets are expected to work before 1.0. Do not accidentally make `WidgetKind` public merely to permit extension.

Choose an intentional mechanism such as:

- composition from built-ins for most users;
- documented custom paint/render extension points;
- a sealed/internal built-in widget pipeline with separate public custom-widget traits only if required and benchmarked.

The extension model must not force every internal built-in representation to become public.

## Migration sequence

1. Complete Plan 9 handle representation and Plan 13 BuildContext direction.
2. Inventory public widget constructors/types/exports.
3. Mark the intended first-public API surface.
4. Make `Widget` fields and `WidgetKind` internal.
5. Make concrete widget types the canonical constructors.
6. Migrate examples, tests, controls and material code.
7. Delete duplicate legacy static constructors/aliases.
8. Add rustdoc examples for primary composition patterns.

## Hard invariants

- public API cannot directly construct invalid internal `WidgetKind` states;
- immutable widget configuration remains cheap to clone/move;
- keys cannot be mutated behind a retained element's back;
- internal render/layout taxonomy can evolve without public breakage;
- public composition remains ergonomic and Flutter-familiar;
- no public trait-object extension point is added without a real use case and performance evidence.

## Tests

- compile-time/public API tests using only documented concrete widgets;
- examples contain no access to internal `WidgetKind`;
- material/controls build without privileged representation access except explicit `pub(crate)` internal boundaries;
- rustdoc snippets compile;
- visibility audit ensures internal IDs/specs are not accidentally exported.

## Acceptance criteria

- `Widget` is opaque to external crates;
- `WidgetKind` is not public API;
- concrete widget types are the canonical authoring surface;
- redundant legacy `Widget::...` constructors are removed;
- first-public extension points are intentional and documented;
- public rustdoc demonstrates Flutter-like composition without leaking internals.

## Validation

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test-constrained --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
```

## Completion report

### Public surface before -> after

- Before: `Widget` exposed its descriptor node through deref-style access, `WidgetKind` was public, sibling crates could rely on a broad `tree::*` internal re-export, and application/test code commonly authored trees with static `Widget::foo(...)` constructors.
- After: `Widget` is a pointer-sized opaque shared descriptor handle. Its node, kind, mutation helpers and pointer identity are crate-private. `WidgetKind` is `pub(crate)` and the public root exports only intentional concrete widget descriptors and stable concepts such as `key()` and `debug_type_name()`.
- `internal` now has an explicit auditable re-export list rather than `pub use crate::tree::*`. DevTools-only types are feature-gated explicitly. Controls and Material do not inspect `WidgetKind`; the one intentional behavior-specific bridge is `scroll_view_parts`, which preserves an already-built scroll viewport without exposing its representation.
- Concrete widget defaults remain canonical. In particular, `Row`/`Column` keep their Flutter-style defaults instead of inheriting the old static helpers' implicit `MainAxisSize::Min` / `CrossAxisAlignment::Start` behavior. Tests that require that legacy geometry state it explicitly.

### Removed static constructors

The redundant public/static authoring path was removed for:

`fixed_box`, `custom_paint`, `text`, `editable_text`, `editable_text_configured`, `constrained`, `limited_box`, `overflow_box`, `unconstrained`, `fractionally_sized`, `baseline`, `gesture`, `align`, `row`, `column`, `flex`, `flexible`, `wrap`, `table`, `stack`, `positioned`, `indexed_stack`, `visibility`, `aspect_ratio`, `raw_scrollbar`, `scale`, and `rotate`.

Equivalent framework lowering helpers that are still useful internally remain crate-private. `Widget::box_` remains public as the intentional low-level fixed rectangle primitive rather than a duplicate concrete-widget convenience alias.

### Custom-extension story

- Most third-party components are ordinary Rust composition: functions or application structs build public concrete descriptors and return/hold an opaque `Widget` only at tree/storage boundaries.
- Custom drawing uses the existing public `CustomPaint` / `CustomPainter` path.
- Built-in retained/layout/render taxonomy stays sealed; downstream code does not construct `WidgetKind` or payload variants.
- No public trait-object custom-render protocol was added. Such an extension point should be introduced only for a demonstrated use case with an explicit ownership/invalidation contract and performance evidence.

### Representative first-public composition

```rust
use incular_widgets::{Column, Padding, Text, Widget};

let content = Padding::all(
    16.0,
    Column::new([
        Text::new("Incular"),
        Text::new("Concrete widgets first"),
    ]),
);
let root: Widget = content.into();
```

For retained scrolling, the controller is configured on the concrete descriptor before erasure:

```rust
use incular_scroll::ScrollController;
use incular_widgets::{Column, SingleChildScrollView, Text, Widget};

let controller = ScrollController::new();
let view: Widget = SingleChildScrollView::new(Column::new([
    Text::new("One"),
    Text::new("Two"),
]))
.controller(controller)
.into();
```

### Validation result

- `cargo fmt --all -- --check` - pass.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` - pass.
- `cargo test-constrained --all-features` - pass, including the public API/boundary tests and the crate rustdoc example.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps` - pass.
- `git diff --check` - pass.
- Visibility audits found no direct `WidgetKind`, `.kind()`, or `ptr_eq()` use in `incular-widgets` integration tests/benches or in Controls/Material.
