# Plan 11 - Unify dynamic child materialization

## Goal

Give slivers, list wheels, draggable sheets, two-dimensional viewports and future lazy/dynamic widgets one transactional retained-child reconciliation mechanism instead of copy/pasted prepare/materialize branches.

## Current problem

Advanced scrolling layout contains near-duplicate branches that:

1. ask a model for desired children;
2. map model-specific identities into keys;
3. call materialization;
4. update retained layout state.

Wheel scroll-view/viewport and two-dimensional scroll-view/viewport pairs currently repeat this flow. This is a maintenance trap.

## Target architecture

Introduce one internal dynamic-child protocol owned by `WidgetTree` reconciliation.

Use a compact desired-child value:

```rust
struct DesiredChild<K> {
    key: K,
    widget: Widget,
    semantic_index: Option<usize>,
}
```

and one transactional reconciliation operation conceptually equivalent to:

```rust
fn reconcile_dynamic_children<K>(
    &mut self,
    owner: ElementId,
    source: DynamicChildSource,
    desired: impl IntoIterator<Item = DesiredChild<K>>,
) -> Result<DynamicChildResult, TreeError>
```

The exact generic shape may differ to avoid monomorphization/API noise, but the lifecycle semantics must be single-source.

## Responsibilities of the shared reconciler

- validate all desired child descriptors before mutating retained state;
- reject duplicate dynamic keys deterministically;
- reuse compatible retained children by key/type;
- mount new children;
- unmount removed children;
- preserve stable order;
- update owner child/key bookkeeping atomically;
- synchronize render children once;
- provide contextual `TreeError::InvalidGeneratedChild` information;
- leave the previous retained child set valid on prevalidation failure.

## Model adapters

Each dynamic layout family should only be responsible for producing:

- desired child identities/widgets;
- family-specific layout metadata;
- optional semantic indices/pinning metadata.

It should not implement its own retained lifecycle algorithm.

Use typed adapters/functions for wheel, sliver, sheet and 2D families. Avoid public trait objects; this is an internal reconciliation protocol.

## Layout commit rule

Do not write a new family layout snapshot into retained feature state until dynamic-child reconciliation succeeds. This prevents layout state from describing children that were not actually committed.

## Migration sequence

1. Extract current sliver and advanced-child lifecycle semantics into tests.
2. Define common desired-child/key/result types.
3. Implement the transactional shared reconciler.
4. Migrate list-wheel families.
5. Migrate two-dimensional families.
6. Migrate draggable-sheet/scrollbar synthetic children where appropriate.
7. Reuse the mechanism for sliver materialization if semantics align; otherwise share the lifecycle core and retain sliver-specific metadata adapter.
8. Delete old `materialize_advanced_children`/parallel reconciliation paths.

## Hard invariants

- generated child validation happens before destructive mutation;
- a dynamic child key identifies at most one live child for an owner;
- failed generation never leaks elements, renders, subscriptions or layers;
- successful reconciliation preserves keyed retained identity;
- family layout snapshots and retained child sets describe the same commit;
- no family has a private copy of mount/update/unmount logic.

## Tests

Create a shared contract suite exercised by every dynamic-child family:

- unchanged set;
- append/prepend/remove/reorder;
- keyed type-compatible update;
- keyed incompatible replacement;
- duplicate-key rejection;
- invalid generated subtree rollback;
- large visible-window shifts;
- resource counts after repeated churn;
- semantic index/pinned metadata preservation where supported.

## Acceptance criteria

- one retained dynamic-child reconciliation core exists;
- wheel and 2D duplicate prepare/materialize logic is gone;
- family adapters only compute desired children and layout metadata;
- rollback/error behavior is consistent across all dynamic families;
- no legacy parallel materialization implementation remains.

## Validation

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p incular-widgets --all-features
cargo test -p incular-widgets --test tree_performance_contracts --all-features
cargo test-constrained --all-features
```

## Completion report

List every dynamic-child family migrated, duplicate lifecycle functions removed, rollback/resource-count results and performance comparisons for large viewport shifts.

## Completed implementation - 2026-08-31

### Shared reconciliation core

- Added one internal `reconcile_dynamic_children` lifecycle core with typed desired-key/widget inputs.
- The core prevalidates the complete desired set, rejects duplicate generated keys before mutation, reuses compatible keyed children, mounts replacements, unmounts removed children and preserves emitted order.
- Family adapters commit owner bookkeeping and synchronize render children only after the lifecycle operation succeeds.
- Lazy-item diagnostic accounting is an explicit adapter policy so `LayoutBuilder` does not inflate scrolling/sliver `items_*` counters.

### Migrated families

- Sliver viewports now adapt `SliverChildId` plus widget descriptors into the shared core; semantic indices and pinned metadata remain sliver-specific commit metadata.
- `ListWheelScrollView` and `ListWheelViewport` share one wheel adapter and commit the retained `WheelLayout` only after child reconciliation succeeds.
- `TwoDimensionalScrollView` and `TwoDimensionalViewport` share one 2D adapter and commit the retained viewport layout only after reconciliation succeeds.
- `DraggableScrollableSheet` uses the same advanced-child adapter for its generated child; initial mounted state/config and compatible builder/config changes are committed only after the generated child succeeds.
- `LayoutBuilder` now uses the same lifecycle core with a singleton internal key while retaining its own builder/constraint/revision policy.
- `RawScrollbar` and `DraggableScrollableActuator` were removed from the dynamic path because both already have ordinary declarative single children.

### Lifecycle cleanup

- Removed the duplicated `materialize_advanced_children` implementation.
- Replaced the sliver-specific mount/update/unmount implementation with `reconcile_sliver_children` as a metadata adapter over the shared core.
- Dynamic-child preservation during compatible owner updates is now driven by canonical `WidgetKind::structure()` / `WidgetChildren::Dynamic` metadata instead of hard-coded SliverViewport/LayoutBuilder exceptions.
- Wheel and 2D scroll-view/viewport duplicate prepare branches were collapsed into shared family helpers.

### Rollback and retention coverage

- Added a duplicate generated sliver-key regression which first commits a valid child set, then produces a duplicate ID and verifies the previous element/render/layer counts remain unchanged.
- Existing generated-subtree failure tests continue to verify rollback for LayoutBuilder, slivers and wheels.
- Existing sliver tests continue to verify keyed reuse, incompatible replacement, removals, semantic/pinned metadata and million-item cache-window/deep-jump behavior.
- Existing advanced scrolling tests continue to verify retained wheel/sheet/2D state across compatible updates.
- The 10,000-child reconciliation performance contracts remain green.

### Validation

Passed:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p incular-widgets --all-features
cargo test -p incular-widgets --test tree_performance_contracts --all-features
cargo test-constrained --all-features --quiet
git diff --check
```

Architecture search confirms no legacy `materialize_advanced_children` or `materialize_sliver_children` lifecycle entry points remain.

## Completed implementation - 2026-08-31

### Shared reconciliation core

- Added one generic internal `reconcile_dynamic_children` lifecycle in `WidgetTree`.
- The core prevalidates every generated subtree before retained mutation.
- Duplicate generated keys are rejected deterministically as contextual `TreeError::InvalidGeneratedChild` errors.
- Compatible children are reused by generated key + widget compatibility; incompatible children are replaced; removed children are unmounted after the new set is prepared.
- Stable desired ordering is committed once and render children are synchronized once by the family adapter.
- Item build/mount/reuse/unmount diagnostics are accounted by the shared core for lazy viewport families.

### Families migrated

- Sliver viewports now use `reconcile_sliver_children` as a metadata adapter over the shared core. Semantic indices, pinned IDs and delegate/controller revisions remain sliver-specific metadata only.
- List-wheel scroll view and list-wheel viewport share one wheel preparation path and the common advanced-child adapter.
- Two-dimensional scroll view and viewport share one 2D preparation path and the common advanced-child adapter.
- Draggable sheet generated child replacement uses the same advanced-child adapter.
- `LayoutBuilder` now also uses the same dynamic-child lifecycle core with a unit key, so generated-subtree validation/replacement semantics are no longer a separate implementation.
- Raw scrollbar and draggable actuator have ordinary declarative child edges and therefore no longer participate in synthetic advanced-child materialization.

### Removed duplication

- Removed the separate sliver mount/update/unmount loop.
- Removed the separate advanced-scrolling mount/update/unmount loop.
- Removed the legacy `materialize_advanced_children` path/name.
- Collapsed duplicate wheel scroll-view/viewport layout branches into `prepare_wheel_children`.
- Collapsed duplicate 2D scroll-view/viewport layout branches into `prepare_two_dimensional_children`.

### Transaction / rollback coverage

- Existing invalid generated-subtree tests for LayoutBuilder, sliver and wheel remain green and verify element/render/layer counts do not leak.
- Added `duplicate_dynamic_key_rejects_the_new_set_without_destroying_the_previous_commit`: a custom sliver first commits a valid child set, then emits a duplicate generated key on a later revision. The update is rejected before destructive mutation and element/render/layer counts remain at the previous committed baseline.
- Existing keyed reorder, incompatible replacement, million-item sliver window shifts and advanced scrolling retained-state tests remain green.

### Performance

- The shared core preserves the previous O(visible-window) reconciliation shape: one hash map of old generated keys, one prevalidation pass, one desired pass and one cleanup pass.
- The 10,000-child retained reconciliation performance contracts remain green.
- Million-item sliver deep-jump/materialization contracts remain green and continue to materialize only the destination cache window rather than scanning logical item count.
- No additional per-family lifecycle pass remains after migration.

### Validation

Passed:

```text
cargo fmt --all -- --check
cargo clippy -p incular-widgets --all-targets --all-features -- -D warnings
cargo test -p incular-widgets --all-features
cargo test -p incular-widgets --test tree_performance_contracts --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test-constrained --all-features --quiet
git diff --check
```

### Completed implementation - 2026-08-31

- Introduced one generic `reconcile_dynamic_children` lifecycle core for generated children.
- Migrated sliver viewport children, advanced scrolling children, and `LayoutBuilder` output to that core.
- Wheel scroll-view/viewport and two-dimensional scroll-view/viewport now share family adapters instead of duplicate lifecycle branches.
- Draggable sheet, raw scrollbar and actuator synthetic children use the same advanced dynamic-child adapter.
- Removed the old `materialize_advanced_children` path; `reconcile_advanced_children` is now only the advanced-key/bookkeeping adapter over the shared core.
- Shared reconciliation prevalidates every desired widget, rejects duplicate generated keys before mutation, reuses compatible keyed children, mounts replacements/new children, unmounts removed children, preserves order, and returns one committed child/key result.
- Sliver-specific semantic indices and pinned metadata remain a thin adapter layered on the common lifecycle core.
- Family layout snapshots are written only after child reconciliation succeeds.

Rollback/resource contracts:

- Added a custom sliver test that first commits a valid child set, then emits duplicate dynamic IDs and verifies the previous retained resource counts remain unchanged.
- Existing LayoutBuilder, sliver and wheel invalid-generated-subtree tests continue to verify no element/render/layer leaks on prevalidation failure.
- Existing keyed reorder/incompatible replacement contracts remain green.

Validation passed:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p incular-widgets --all-features
cargo test -p incular-widgets --test tree_error_policy --all-features
cargo test -p incular-widgets --test tree_performance_contracts --all-features
cargo test-constrained --all-features --quiet
git diff --check
```

The 10,000-child reconciliation structural/performance contracts remain green after the extraction, including unchanged-child no-mutation behavior, keyed reorder reuse, single-child updates and incompatible replacement isolation.
