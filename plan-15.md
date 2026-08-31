# Plan 15 - Make layout natively fallible and remove the error side channel

## Goal

Make the layout API tell the truth in its type system. Generated-child/configuration failures discovered during layout must propagate normally with `Result`, not be stored in `pending_tree_error` and recovered after a void recursive pipeline returns.

Incullar is pre-public, so remove the legacy swallowing API rather than deprecating it indefinitely.

## Current problem

`WidgetTree::layout()` returns `()` and calls `try_layout()`, discarding the returned error. Internally, some layout helpers cannot return `TreeError`, so they record failures in `pending_tree_error`; `try_layout()` checks that side channel after layout completes.

This makes fallibility non-local and allows work to continue after the first failure unless each call site manually checks a boolean helper.

## Target API

The canonical API should simply be fallible:

```rust
pub fn layout(&mut self, constraints: Constraints) -> Result<(), TreeError>
```

There should be no duplicate `try_layout` naming once no infallible `layout` exists.

Runtime/frame code uses `?`/typed conversion directly.

## Propagate Result through layout

Refactor layout operations that can materialize generated children or otherwise encounter application-authored configuration errors to return `Result`.

Typical shapes:

```rust
fn layout_render(...) -> Result<Size, TreeError>;
fn prepare_dynamic_children(...) -> Result<(), TreeError>;
fn layout_child(...) -> Result<..., TreeError>;
```

Pure layout algorithms that cannot fail may remain infallible. Do not mechanically wrap every arithmetic helper in `Result`.

## Transaction boundaries

When a layout-time builder/dynamic child fails:

- return immediately from the affected layout transaction;
- preserve the last valid retained child set as defined by Plan 11;
- do not commit a new layout snapshot that depends on failed children;
- leave dirty flags in a state that permits a later corrected rebuild/layout;
- runtime receives the exact contextual `TreeError`.

## Remove legacy state

Delete:

- infallible compatibility `layout()`;
- `try_layout()` duplicate naming;
- `pending_tree_error`;
- `last_tree_error` if it exists only for compatibility/error polling;
- `record_tree_error` / `record_tree_result` boolean side-channel helpers.

If DevTools needs frame failure history, store it in DevTools/runtime diagnostics explicitly, not as control flow inside `WidgetTree`.

## Migration sequence

1. Identify every layout path that can currently call `record_tree_error`.
2. Make the nearest operation return `Result`.
3. Propagate `Result` upward through recursive layout entry points.
4. Update runtime/frame APIs.
5. Remove pending/last tree-error fields and helpers.
6. Rename the final fallible entry point to `layout`.
7. Update tests/examples directly; no legacy wrapper.

## Hard invariants

- no application-authored layout error is swallowed;
- first failure returns through normal control flow;
- pure geometry/layout helpers remain simple and infallible where appropriate;
- failed dynamic generation does not partially commit retained lifecycle state;
- runtime frame error reporting retains source/context;
- corrected subsequent layout can recover without reconstructing the whole runtime.

## Tests

- LayoutBuilder invalid child returns `Err` directly from `layout`;
- sliver/wheel/2D invalid generated child returns its contextual error;
- no side-channel state needs to be inspected after failure;
- resource counts remain unchanged on failed materialization;
- correct widget update followed by layout succeeds after a previous failure;
- runtime `run_frame` surfaces the same error chain.

## Acceptance criteria

- canonical `WidgetTree::layout` returns `Result<(), TreeError>`;
- no `try_layout` compatibility split remains;
- pending/last tree error control-flow fields are gone;
- layout-time generated-child failures use `?` propagation;
- no boolean `record_tree_result` pattern remains;
- docs/examples use only the final fallible API.

## Validation

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p incular-widgets --all-features
cargo test -p incular-runtime --all-features
cargo test --workspace --all-features
```

## Completion report

List every removed side-channel API/field, the final fallible call chain and regression tests proving rollback plus recovery after a failed frame.

### Completed

- `WidgetTree::layout(Constraints)` is now the single canonical entry point and returns `Result<(), TreeError>`.
- Removed the compatibility/error-polling control flow completely: `try_layout`, `pending_tree_error`, `last_tree_error`, `take_last_tree_error`, `record_tree_error`, and `record_tree_result` no longer exist.
- The fallible chain is now direct: `layout -> layout_render -> layout_render_inner -> layout_kind -> layout_{core,container,text,scrolling,effect,sliver}_kind`, with generated-child preparation/reconciliation using `?` at the point of failure.
- Wheel, draggable-sheet, two-dimensional, sliver, and `LayoutBuilder` materialization now return immediately on reconciliation/configuration failure. Pure geometry helpers remain infallible.
- `update_compositor` is also fallible because pinned-sliver compositor updates can intentionally perform same-frame retained relayout; its layout error is propagated instead of discarded. Runtime `run_frame` uses `?` for both layout and compositor update.
- Tests, examples, integration surfaces, and benchmarks were migrated to consume the final API explicitly; expected-success callers use `expect` rather than silently discarding the `Result`.

Regression coverage:

- `LayoutBuilder` failure after a previously valid generated subtree preserves element/render/layer counts, then a corrected descriptor layouts successfully.
- Sliver duplicate generated children preserve the previous committed child/resource set and recover on a corrected subsequent layout.
- Wheel and two-dimensional generated-child failures return contextual `InvalidGeneratedChild` errors without resource growth.
- Direct `layout` failure requires no side-channel inspection and can be retried successfully.
- Runtime `run_frame` surfaces the same contextual `LayoutBuilder` error chain.

Validation completed successfully:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p incular-widgets --all-features
cargo test -p incular-runtime --all-features
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
git diff --check
```
