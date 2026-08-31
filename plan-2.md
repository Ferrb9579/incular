# Plan 2 - Replace giant render-phase switchboards with modular render behavior

## Goal

Stop making every widget/render feature participate in giant centralized `match` statements across layout, paint, hit testing, semantics, update logic, debug naming, and widget-to-render conversion.

Keep `WidgetTree` as the coordinator. The problem is not centralized orchestration; the problem is that individual render behaviors are spread across unrelated global switchboards.

## Current problem

`WidgetKind` and `RenderKind` are interpreted repeatedly in files such as:

- `tree/widget.rs`
- `tree/rendering.rs`
- `tree/layout.rs`
- `tree/painting.rs`
- `tree/interaction.rs`
- `tree/semantics.rs`
- `tree/reconciliation.rs`

Adding a primitive can require touching many long matches. Missing one location can compile if a wildcard arm exists, which makes feature completeness harder to audit.

## Target architecture

Keep statically known render kinds, but move behavior next to the render payload that owns it.

Use narrow internal traits or equivalent per-phase interfaces:

```rust
trait LayoutBehavior {
    fn layout(
        &mut self,
        ctx: &mut LayoutContext<'_>,
        id: RenderObjectId,
        constraints: Constraints,
    ) -> LayoutResult;
}

trait PaintBehavior {
    fn paint(&self, ctx: &mut PaintContext<'_>, id: RenderObjectId);
}

trait HitTestBehavior {
    fn hit_test(&self, ctx: &HitTestContext<'_>, id: RenderObjectId, point: Offset) -> HitTestResult;
}
```

The implementation may use enum delegation rather than trait objects if that integrates better with arena borrowing and benchmarks. The essential requirement is that `RenderFlex` layout logic lives with `RenderFlex`, text behavior lives with text, scrolling behavior lives with scrolling, etc.

Do not introduce runtime polymorphism purely for aesthetic similarity to Flutter. Prefer a Rust-native design with compile-time exhaustiveness and predictable memory/layout behavior.

## Dispatch design

Use one of these acceptable patterns, chosen after a small benchmark/prototype:

1. **Typed enum delegation** - `RenderObjectKind` has one short delegating match per phase and each arm immediately calls a specialized implementation.
2. **Generated enum delegation** - a local macro defines the variant registry once and generates boilerplate delegation, debug names and classification helpers.
3. **Trait-object behavior** only if benchmarks and borrow ergonomics show it is cleaner without requiring `Any`, unsafe aliasing, or take/restore hacks.

Preferred default: typed enum delegation.

The central dispatch should look like a table of contents, not contain the algorithm itself.

## Widget -> render configuration

Replace the single large `render_kind(widget, environment)` conversion with per-widget/per-family constructors.

Examples:

```rust
impl RenderText {
    fn from_widget(text: &TextWidgetData, env: &BuildEnvironment) -> Self;
    fn update_from_widget(&mut self, text: &TextWidgetData, env: &BuildEnvironment)
        -> RenderInvalidation;
}
```

Where practical, split giant `WidgetKind` variant payloads into named data structs so conversion functions do not destructure dozens of fields repeatedly.

Do not duplicate conversion logic between mount and update. Each render family must have one canonical configuration/update path.

## Invalidation contract

Replace scattered helper predicates such as "text paint only", "effect composite only", etc. with an explicit update result:

```rust
bitflags! {
    struct RenderInvalidation: u8 {
        const LAYOUT = 1 << 0;
        const PAINT = 1 << 1;
        const COMPOSITE = 1 << 2;
        const SEMANTICS = 1 << 3;
    }
}
```

Each payload compares and applies its own configuration, returning the minimum required invalidation. `WidgetTree` remains responsible for scheduling/propagation.

This creates one authoritative answer to "what does changing this property invalidate?"

## Module boundaries

Behavior should be grouped by render object family, not phase-only mega-files.

An acceptable end state is:

```text
render_object/
  text.rs       # state + update + layout + paint + hit test + semantics hooks
  flex.rs
  stack.rs
  scrolling.rs
  effects.rs
  image.rs
  editing.rs
```

Some reusable phase algorithms can remain in `layout/`, `painting/`, etc., but they must be generic helpers rather than global knowledge of every render variant.

## Migration sequence

1. Introduce `RenderInvalidation` while retaining old behavior.
2. Move one low-risk family (`Padding`, `Align`, `Constrained`) to delegated layout/update behavior.
3. Move text/image leaf behavior.
4. Move flex/stack/table layout behavior.
5. Move transform/effect behavior.
6. Move scrolling/sliver behavior last because it has the most retained/lazy state.
7. Remove obsolete global helper predicates and giant algorithmic match arms.
8. Remove wildcard arms where exhaustiveness is semantically important.

## Hard invariants

- no render family owns scheduling; it only reports invalidation;
- no render family mutates global tree topology except through explicit context methods;
- no hidden callbacks from paint into build/layout;
- phase ordering remains BUILD -> LAYOUT -> PAINT -> COMPOSITE/SEMANTICS according to existing runtime rules;
- cached layout and display-list reuse semantics are preserved;
- enum delegation must remain exhaustive;
- do not solve borrow-checker problems with unsafe aliasing.

## Tests

- unit-test `update_from_widget` invalidation for representative properties;
- assert opacity/effect/transform-only changes remain compositor-only where currently supported;
- assert text style changes distinguish layout-affecting vs paint-only cases correctly;
- preserve keyed reconciliation behavior;
- preserve lazy sliver materialization behavior;
- add tests that every render variant has a debug name/classification and phase implementation where required.

If using a registry macro, add compile-time coverage so a new variant cannot silently omit required behavior.

## Acceptance criteria

- phase files no longer contain large algorithms for dozens of unrelated render kinds;
- adding a render primitive primarily changes its own module plus one explicit registry/enum location;
- mount and update share canonical render configuration logic;
- invalidation is returned explicitly by render behavior rather than inferred in multiple places;
- no `Any`/downcasting or unsafe dispatch was added;
- benchmarks remain within noise.

## Validation

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo bench -p incular-widgets --bench reconciliation
cargo bench -p incular-widgets --bench layout
```

## Completion report

Report the old and new extension path for one representative render primitive, remaining centralized matches and why they are intentionally centralized, invalidation tests, and benchmark comparison.
