# Plan 6 - Make deep-tree lifecycle operations stack-safe by design

## Goal

Ensure that a tree depth accepted by mount/update/layout cannot later crash during teardown or another maintenance traversal because one path still uses uncontrolled native recursion.

## Current problem

The code already acknowledges deep trees:

- mounting uses an explicit work stack;
- layout/update use `stacker::maybe_grow` in selected recursive paths;
- `unmount_element` recursively descends children directly;
- other maintenance traversals also use recursion in places.

This creates inconsistent depth guarantees.

## Target policy

Classify tree traversals into two groups.

### Traversals that can be iterative

Use explicit work stacks for:

- mount/unmount;
- subtree cleanup;
- environment propagation;
- collection/inspection traversals;
- semantics collection where dependency ordering permits it;
- notification-source discovery;
- other DFS/BFS bookkeeping.

### Traversals whose algorithm naturally depends on child return values

Layout and some reconciliation/update flows may remain recursive if iterative conversion would materially damage clarity. Their stack policy must be centralized and documented rather than calling `stacker::maybe_grow` ad hoc.

## Shared traversal utilities

Introduce small internal traversal helpers where they reduce duplicated DFS mechanics, e.g. post-order work items:

```rust
enum Visit<T> {
    Enter(T),
    Exit(T),
}
```

Do not build a generic abstraction so complicated that each call site becomes harder to understand. Specialized loops are preferable for performance-critical paths.

## Unmount algorithm

Convert unmount to explicit post-order teardown so children are removed before/while parent resources are finalized according to current lifecycle semantics.

Requirements:

- unregister raw input/selection/listeners exactly once;
- cancel pointer/gesture ownership for removed nodes;
- remove compositor layers exactly once;
- remove render arena entries and element entries deterministically;
- preserve `unmounted` diagnostics/order if externally observed by tests/devtools;
- avoid keeping cloned full `Element` values for a huge subtree longer than necessary.

Use compact work records containing IDs and teardown stage rather than cloning widgets unnecessarily.

## Depth contract

Add one documented framework depth policy:

- no arbitrary low hard-coded widget depth limit unless required for cycle/recursion protection;
- pathological cycles/recursive builder behavior are handled by existing recursion diagnostics;
- ordinary finite deep trees are bounded by memory, not an accidental native call-stack limit, for lifecycle bookkeeping.

Do not conflate "deep valid tree" with recursive-build-cycle detection.

## Migration sequence

1. Inventory all recursive functions over element/render topology.
2. Mark each as iterative-required or recursion-justified.
3. Convert `unmount_element` first.
4. Convert environment/notification/other simple DFS traversals.
5. Centralize stack-growth policy for justified recursive layout/update paths.
6. Add extreme-depth lifecycle tests.
7. Remove redundant `stacker` usage where traversal is now iterative.

## Hard invariants

- a subtree is never partly detached with descendants still referencing removed parents after successful teardown;
- each lifecycle callback/cleanup occurs exactly once;
- no recursive destructor-style cascade through owned child boxes is introduced;
- deep-tree tests run without enlarging the test thread stack artificially for operations promised to be iterative;
- layout results and reconciliation identity remain unchanged.

## Tests

Construct very deep synthetic trees, with depth chosen high enough to overflow ordinary recursion on supported test platforms while remaining memory-reasonable.

Test:

- mount -> unmount;
- mount -> replace root/subtree -> teardown;
- deep environment scopes;
- deep semantics/focus traversal where applicable;
- cleanup of gestures/listeners/layers at depth;
- stale generational IDs after mass teardown;
- no retained arena/layer count leak.

Also retain recursion-cycle tests to ensure stack-safe traversal does not weaken cycle diagnostics.

## Acceptance criteria

- `unmount_element` is iterative;
- all simple topology DFS operations are either iterative or explicitly justified in code/docs;
- stack-growth policy for remaining recursive hot algorithms is centralized;
- a deep-tree lifecycle regression suite exists;
- no cleanup/resource-count regressions.

## Validation

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p incular-widgets
cargo test -p incular-runtime
cargo test --workspace --all-features
```

## Completion report

List every recursive tree traversal found, its final strategy (iterative or justified recursion), maximum tested depth and resource-leak results.

### Completed implementation

- Mount remains explicit-stack DFS; widget subtree validation is iterative.
- Unmount is explicit post-order teardown and releases input/gesture ownership,
  selection/listener state, compositor layers, render nodes, and elements once.
- Environment propagation, notification-source discovery, selectable-text
  collection, semantics collection, ordinary focus discovery, and declarative
  widget destruction use explicit work stacks. Focus traversal-group nesting
  retains structured recursion because groups produce nested ordered member
  lists; ordinary descendant walking inside those groups is iterative.
- Declarative child edges are shared with `Rc<Widget>`. Retained nodes therefore
  clone only direct descriptor edges instead of recursively duplicating complete
  descendants; unique descriptor destruction unwraps those edges iteratively.
- Callback binding is local to the element being mounted/updated. Descendant
  callbacks are bound when their own retained elements are processed.
- Layout, paint, and the remaining child-return-value reconciliation paths use
  one `with_recursive_tree_stack` policy rather than ad-hoc `stacker` calls.
- Reconciliation no longer invokes recursive full-subtree equality for non-leaf
  widgets. The unchanged fast path remains for childless descriptors, while
  retained child reconciliation walks deep trees under the centralized policy.
- Recursion diagnostics continue to detect same-node re-entry precisely. The
  numeric depth ceiling is an emergency runaway guard rather than a normal
  finite-widget-depth limit.

Depth contracts exercised on the ordinary cargo-test thread:

- 50,000 nested declarative wrappers: destruction completes iteratively.
- 4,096 retained levels: mount, focus discovery, semantics, compatible update,
  root replacement, stale-ID rejection, and arena/layer cleanup complete.
- 2,048 levels below an environment-backed LayoutBuilder: materialization,
  environment propagation, update, and relayout retain stable arena counts.
