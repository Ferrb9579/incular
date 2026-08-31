# Plan 1 - Replace the monolithic RenderObject state bag

## Goal

Replace the current `RenderObject` representation with a small common node shell plus strongly typed per-render-kind state. A render node must only store state that is valid for that kind of render object.

This is a structural refactor, not a behavior rewrite. Public widget behavior, layout results, painting, hit testing, semantics, retained identity, and performance characteristics must remain equivalent unless a separately documented bug is fixed.

## Current problem

`crates/incular-widgets/src/tree.rs` currently stores unrelated state in every `RenderObject`, including text revisions, text scroll offsets, scrollbar state, draggable-sheet state, button state, focus state, cached display lists, and many optional compositor layer IDs.

Problems:

- invalid combinations are representable;
- every render object pays conceptual and often memory cost for unrelated features;
- fields are mutated from many distant modules;
- adding a feature usually adds another field to all render objects;
- invariants live in comments and `expect("live")` calls instead of the type system;
- it is difficult to understand what state belongs to one render kind.

## Target architecture

Keep one arena-backed retained node identity, but split common topology/geometry from type-specific behavior/state.

```rust
pub(crate) struct RenderNode {
    parent: Option<RenderObjectId>,
    children: Vec<RenderObjectId>,
    geometry: RenderGeometry,
    dirty: DirtyFlags,
    object: RenderObjectKind,
}

pub(crate) struct RenderGeometry {
    size: Size,
    offset: Offset,
    constraints: Option<Constraints>,
    baseline: Option<f32>,
}

pub(crate) enum RenderObjectKind {
    Box(RenderBox),
    Text(RenderText),
    TextField(RenderTextField),
    Flex(RenderFlex),
    Scroll(RenderScroll),
    SliverViewport(RenderSliverViewport),
    Effect(RenderEffect),
    // ...
}
```

The exact names may change, but the ownership rule must not: generic node data lives in the node shell; feature-specific mutable state lives in the corresponding render payload.

Examples:

- `RenderTextField` owns controller revision tracking, local text scroll offsets, caret/selection render state.
- `RenderScroll`/`RenderSliverViewport` own overlay-scrollbar hover/drag state in addition to their viewport configuration.
- `RenderRawScrollbar` owns the retained `RawScrollbar` model/geometry cache for the explicit scrollbar widget.
- `RenderButton` owns pressed/hover/focus visual state.
- `RenderDraggableSheet` owns `DraggableScrollableState`.
- ordinary layout nodes do not contain any of those fields.

Do not replace the current model with a large `Box<dyn Any>` payload or pervasive downcasting. The replacement must be statically exhaustive and inspectable.

## Module structure

Create a dedicated render-object area under `incular-widgets`, for example:

```text
src/render_object/
  mod.rs
  node.rs
  leaf.rs
  text.rs
  editing.rs
  flex.rs
  stack.rs
  scrolling.rs
  sliver.rs
  effects.rs
  semantics.rs
```

Module boundaries should follow behavior/state ownership, not merely split a large file by line count.

`tree.rs` should retain `WidgetTree` orchestration and IDs but should no longer define the complete render-object implementation surface.

## Migration sequence

1. Introduce `RenderGeometry` and move `size`, `offset`, `constraints`, and `baseline` into it with no behavior change.
2. Introduce typed payload structs while preserving the existing `RenderKind` enum shape temporarily.
3. Move text-only runtime fields into `RenderText`/`RenderTextField`.
4. Move scrolling-only runtime fields into the corresponding scrolling payloads.
5. Move button/focus visual state into the relevant interactive payloads.
6. Move effect/compositor-specific state as described by Plan 3.
7. Rename the final enum to `RenderObjectKind` or another unambiguous name once it owns both configuration and local retained state.
8. Remove every legacy cross-kind field from the common node.

Migrate one behavioral family at a time and keep tests green after each family. Do not perform a flag-day rewrite.

## Hard invariants

- `RenderObjectId` remains generational and invalid immediately after removal.
- parent/child topology has exactly one owner: the render tree.
- layout geometry is common node state; feature state is not.
- a render payload may not directly mutate another render node's private payload.
- cross-node operations go through narrow `WidgetTree`/render-tree helpers.
- no `Any`, stringly typed state maps, or untyped property bags.
- no duplicated source of truth between common node fields and payload fields.
- no regressions to compositor-only update paths.
- no increase in asymptotic reconciliation/layout complexity.

## API and ergonomics

Add narrow typed accessors only where a subsystem genuinely needs them, e.g. `as_text_field_mut()` inside the render-object module. Do not expose payload internals publicly to avoid recreating global coupling through accessor methods.

Prefer methods on the owning payload for local operations:

```rust
impl RenderTextField {
    fn refresh_revisions(&mut self) -> TextInvalidation { ... }
}
```

The method should return a small semantic result such as `TextInvalidation`, not directly reach into unrelated tree systems.

## Tests

Add structural tests in addition to preserving all existing behavior tests:

- each render kind constructs only its expected payload;
- replacement between compatible widget configurations preserves local retained state that should survive;
- incompatible replacement creates fresh local state;
- text-field state does not survive replacement by plain text;
- scrollbar state cannot exist on a non-scrollbar render payload;
- generational IDs remain stale after removal/reuse;
- layout/paint diagnostics remain unchanged for representative trees.

Add a memory-size regression test or benchmark for representative `RenderNode` payloads. Do not assert one exact compiler-dependent byte count; assert meaningful upper bounds or compare before/after benchmark output.

## Performance validation

Run existing reconciliation/layout/semantics benchmarks and add a render-node construction/update benchmark if needed.

The refactor must not regress the main retained hot paths beyond normal benchmark noise. Any measurable regression must be explained and approved rather than hidden.

## Acceptance criteria

- common `RenderNode` contains only topology, geometry, dirty state, and one typed render payload;
- text, editing, scrolling, interaction and effect runtime state no longer exist as unrelated optional fields on every node;
- no downcast-based architecture was introduced;
- existing behavior/performance contract tests pass;
- no new `#[allow(...)]` is added to bypass design problems;
- relevant code is documented around ownership/invariants, not implementation trivia.

## Validation

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo bench -p incular-widgets --bench layout
cargo bench -p incular-widgets --bench reconciliation
```

## Completion report

Report:

- old vs new render-node shape;
- which fields moved to which payloads;
- remaining cross-kind state, if any, and why it is truly common;
- test results;
- benchmark comparison;
- any public API changes.
