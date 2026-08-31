# Plan 9 - Make declarative widget ownership intrinsically stack-safe

## Goal

Keep declarative widget cloning cheap and deep-tree destruction stack-safe without requiring `Widget` itself to know how to manually dismantle every `WidgetKind` variant.

The final representation should make ownership obvious and place stack-safety in one narrow abstraction.

## Current problem

`Widget` currently owns a large enum whose child edges are `Rc<Widget>`. Its custom `Drop` replaces the kind with a dummy leaf and uses `WidgetKind::into_owned_children()` plus `Rc::try_unwrap` to iteratively dismantle unique deep chains.

This is correct, but destruction correctness depends on a variant-wide child extraction match and on a dummy replacement value. It couples a fundamental memory-safety/performance property to widget taxonomy.

## Target representation

Make `Widget` a small opaque shared descriptor handle:

```rust
pub struct Widget {
    node: Option<Rc<WidgetNode>>,
}

struct WidgetNode {
    key: Option<Key>,
    kind: WidgetKind,
    semantics: SemanticProperties,
}
```

Child fields inside widget payloads should store `Widget` handles, not raw `Rc<Widget>` edges. Cloning a child therefore clones one small handle.

`Widget` owns the stack-safe release policy. On final ownership, it unwraps `WidgetNode`, drains child `Widget` handles through the canonical structural API from Plan 8, and iteratively releases uniquely owned descendants.

The implementation may use `Option<Rc<_>>`, `ManuallyDrop`, or another safe representation after measurement, but it must avoid unsafe code unless there is a demonstrated reason and a reviewed safety invariant.

## Desired properties

- `Widget::clone()` is O(1) and shallow;
- dropping a 50k+ unary descriptor chain does not recurse on the native stack;
- dropping shared subtrees decrements references without traversing nodes still owned elsewhere;
- there is no dummy `WidgetKind` sentinel used purely to make Drop work;
- child ownership uses one abstraction rather than raw `Rc<Widget>` throughout the enum;
- consuming a widget does not require special escape hatches such as representation-specific `into_kind` workarounds.

## Ownership API

Provide narrow internal operations such as:

- `Widget::node()` / descriptor access by borrow;
- `Widget::ptr_eq()` for identity-of-description optimizations where appropriate;
- `WidgetNode::children()` through Plan 8 structural metadata;
- `WidgetNode::drain_children()` only for final destruction/consumption.

Do not expose reference-count internals publicly.

## Equality

Do not reintroduce recursive full-subtree equality as a reconciliation fast path.

Separate:

- descriptor handle identity (`ptr_eq`) for exact shared descriptions;
- local widget configuration equality for compatible retained update decisions;
- explicit structural recursion only in tests/debug utilities that are stack-safe.

## Migration sequence

1. Land Plan 8 canonical child topology first.
2. Introduce `WidgetNode` and the small `Widget` handle.
3. Convert child payload fields from `Rc<Widget>` to `Widget`.
4. Port constructors/conversions without public compatibility shims.
5. Move iterative final-release logic into the handle implementation.
6. Remove the existing dummy-kind custom Drop path and raw child `Rc`s.
7. Remove obsolete `into_kind`/ownership helpers that only existed for the old representation.
8. Measure size, clone cost, mount cost and deep destruction.

## Hard invariants

- no declarative child chain can cause unbounded native recursive destruction;
- shallow clone cost is independent of subtree size;
- shared descriptors remain immutable;
- retained identity still belongs to `Element`, never to a `Widget` handle;
- pointer identity is never substituted for keyed/type reconciliation semantics;
- no reference cycle is introduced by widget descriptors.

## Tests

- 100,000-deep unary descriptor create/clone/drop on an ordinary test thread;
- shared subtree dropped from several parents without premature destruction;
- wide tree clone allocation contract;
- mount/update behavior identical for cloned versus independently constructed equivalent descriptions;
- no recursive equality path on deep non-leaf trees;
- Miri for the ownership abstraction if compatible with the toolchain.

## Acceptance criteria

- `Widget` is a small opaque descriptor handle;
- raw `Rc<Widget>` child fields are gone;
- stack-safe release is implemented once at the descriptor ownership boundary;
- the old sentinel-replacement Drop technique is removed;
- deep descriptor tests pass without enlarged thread stacks;
- measured mount/update performance does not regress materially.

## Validation

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p incular-widgets --all-features
cargo test -p incular-widgets --test tree_performance_contracts --all-features
cargo test --workspace --all-features
```

## Completion report

Report `size_of::<Widget>()`, clone/destruction behavior, maximum tested depth, allocation/performance measurements and all old ownership helpers removed.
