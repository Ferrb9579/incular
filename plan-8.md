# Plan 8 - Make widget structure single-source-of-truth

## Goal

Make adding or changing a widget a locally understandable operation. Preserve Incular's closed-enum, allocation-conscious retained architecture while eliminating repeated structural knowledge about `WidgetKind` and `RenderKind` across unrelated subsystems.

This is a pre-public cleanup. Do not preserve old internal representations for compatibility.

## Current problem

`WidgetKind` is interpreted independently by many places: type identification, child enumeration, owned-child extraction, equality/debugging, lowering, semantics, focus, painting, layout and reconciliation. `RenderKind` repeats part of the same configuration in another large enum.

The problem is not that the enums are large. The problem is that structural facts such as "this widget has one child", "this widget lowers to this render family", or "this widget owns no render object" are encoded repeatedly.

## Target architecture

Keep a closed internal widget representation, but make its structure authoritative in one module.

Introduce typed payload structs for non-trivial variants instead of large inline field lists where that improves clarity:

```rust
pub(crate) enum WidgetKind {
    Text(TextSpec),
    Padding(PaddingSpec),
    Flex(FlexSpec),
    Scroll(ScrollSpec),
    // ...
}
```

The payloads remain plain immutable data. They must not gain runtime ownership simply to make the enum smaller.

Centralize structural operations on `WidgetKind`:

- stable internal widget type/class;
- child shape: none / optional / single / many / dynamically materialized;
- borrowed child traversal;
- owned child draining used by stack-safe destruction;
- whether the descriptor creates a normal retained child list or owns dynamic children;
- coarse render/layout/paint/focus/semantics family classification where useful.

Behavior-specific code should dispatch on narrow families rather than reclassifying the entire widget universe.

## Do not solve this with an opaque macro DSL

A small internal macro is acceptable only if it removes mechanical duplication while expanding to ordinary readable Rust. Do not hide widget behavior, layout rules, invalidation or ownership inside a custom mini-language.

Prefer explicit typed payloads plus a small number of centralized exhaustive matches over clever code generation.

## Render representation

`RenderKind` is allowed to remain distinct because declarative configuration and retained render state are different phases. However:

- widget-to-render lowering must have one authoritative entry point;
- render variants should carry only data used after lowering;
- callback-only, child-only and build-only widget data must not leak into `RenderKind`;
- shared configuration structs may be reused when semantics are genuinely identical;
- do not maintain two independently evolving copies of the same configuration shape.

## Migration sequence

1. Inventory every exhaustive `WidgetKind` and `RenderKind` match.
2. Classify each match as structural metadata or subsystem behavior.
3. Move structural metadata into the canonical widget-kind module.
4. Introduce typed payload structs for variants with large field sets.
5. Replace duplicate child/type/family matches with canonical methods.
6. Narrow subsystem dispatch to relevant families.
7. Audit widget-to-render lowering for duplicated configuration.
8. Delete superseded helper matches immediately; no compatibility layer.

## Hard invariants

- one authoritative implementation defines the child topology of each widget kind;
- adding a child-bearing widget cannot compile without defining its structural child policy;
- declarative widgets remain immutable values/handles;
- retained mutable state remains outside declarative payloads;
- no new trait-object dispatch is introduced in layout/paint hot paths merely to reduce source-code size;
- widget-to-render lowering remains deterministic and side-effect free;
- no change to painter order, reconciliation identity or retained element ownership.

## Tests

Add structural contract tests covering every built-in widget kind:

- borrowed child count/order equals owned-drain child count/order;
- widget type/class is stable and unique where required;
- leaf widgets report no children;
- dynamic-child widgets are explicitly classified and do not expose synthetic retained children as declarative children;
- lowering produces the expected render family;
- deep and wide widget descriptors retain current allocation/performance contracts.

Prefer table-driven tests built from real constructors rather than hand-constructing internal variants.

## Acceptance criteria

- structural `WidgetKind` knowledge is centralized;
- `type_`, `children_refs`, owned-child extraction and equivalent duplicated classification code no longer maintain independent exhaustive mappings;
- large inline variant payloads are replaced where typed specs materially improve locality;
- render lowering is the only declarative-to-render conversion boundary;
- adding a normal built-in widget requires intentionally implementing its behavior, not hunting through unrelated structural switch statements;
- all superseded legacy helpers are removed.

## Validation

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p incular-widgets --all-features
cargo test -p incular-runtime --all-features
cargo test --workspace --all-features
```

## Completion report

List the old exhaustive structural matches removed, the final canonical structural API, remaining intentional subsystem matches, and before/after hot-path allocation or benchmark results where representation changed.

### Completed implementation - 2026-08-31

- Added one exhaustive `WidgetKind::structure()` classification that owns built-in widget type, lowering family and declarative child topology.
- Replaced the old allocating `children_refs() -> Vec<&Widget>` with an allocation-free `WidgetChildren` view and exact-size iterator.
- Removed the independent lowering-family classifier from `tree/rendering/lowering/mod.rs`; lowering now routes through canonical widget structure metadata.
- Removed `WidgetKind::into_owned_children()`. Stack-safe descriptor teardown now obtains direct shared child handles from the same canonical topology used by reconciliation.
- Reworked `widget_text` to traverse canonical child topology instead of maintaining another exhaustive child-shape match.
- Reworked DevTools widget naming to use canonical `WidgetType::name()` (with the intentional Row/Column presentation distinction for `Flex`) instead of another full widget-kind type map.
- Introduced typed `ButtonSpec` and `TextFieldSpec` payloads for the two largest cross-cutting inline field bags. Lowering, focus, semantics, input, DevTools and interaction now consume these coherent specs.
- Added structural contract tests for leaf, optional, single, many and dynamic child shapes, child order, lowering families and the typed Button/TextField payload classifications.

Remaining exhaustive `WidgetKind` matches are behavior-specific by design: layout/paint/semantics/input/render-layer logic still dispatches on the variants whose behavior it implements. `LayerSpec::for_widget` remains compositor-specific rather than structural metadata because layer ownership is itself renderer behavior.

No trait-object dispatch was added to layout/paint/reconciliation. Ordinary child iteration no longer allocates a temporary `Vec`; only callers that explicitly require an indexed slice materialize one locally.

Validation completed:

```text
cargo fmt --all -- --check
cargo clippy -p incular-widgets --all-targets --all-features -- -D warnings
cargo test -p incular-widgets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
