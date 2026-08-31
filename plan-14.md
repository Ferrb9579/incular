# Plan 14 - Centralize retained-tree invariants and corruption diagnostics

## Goal

Make impossible internal states fail through one high-quality invariant layer with actionable diagnostics instead of hundreds of scattered `expect("present")`-style assumptions.

Do not turn genuine internal corruption into recoverable application errors. Distinguish boundary errors from framework invariant violations precisely.

## Current problem

Core tree/layout/reconciliation code contains many local `expect()` calls asserting that elements, renders and typed feature states remain live and mutually consistent. Most are legitimate invariants, but their messages and context vary and the knowledge is distributed.

## Error policy

Keep two categories:

### Recoverable boundary/configuration errors

Examples: stale caller-provided ID, duplicate key, invalid generated child, invalid widget configuration. These return `TreeError`/typed `Result`.

### Internal invariant violations

Examples: a live element references a missing render, parent/child topology disagrees, a `RenderKind::TextField` lacks text-field feature state. These indicate an Incular bug and should fail loudly with rich diagnostics.

Do not silently convert the second category into `Option`/`Result` plumbing that callers cannot meaningfully recover from.

## Invariant access layer

Add narrow internal accessors on `WidgetTree`/retained state, such as conceptually:

- `element_live(id)` / mutable counterpart;
- `render_live(id)` / mutable counterpart;
- `render_for_live_element(id)`;
- typed feature-state accessors that verify `RenderKind` compatibility;
- parent/child relationship helpers.

On violation, route through one diagnostic function that reports as much available context as possible:

- frame phase;
- element/render IDs;
- widget/render type;
- parent ID;
- current recursion path where available;
- short invariant description.

Avoid a macro unless it materially improves call-site context without obscuring control flow.

## Debug verification

Add an expensive debug/test-only `WidgetTree::verify_invariants()` that checks global retained consistency:

- root validity;
- element parent/child symmetry;
- render parent/child symmetry;
- element-to-render one-to-one ownership;
- no live render owned by a dead element unless explicitly documented;
- feature state matches `RenderKind` class;
- compositor layer ownership/references are valid;
- semantic IDs refer to live elements;
- gesture/pointer captures do not reference dead elements;
- dynamic child bookkeeping matches retained children.

This verifier is not a release-frame hot-path feature. Use it aggressively in tests and optionally DevTools/debug assertions at lifecycle boundaries.

## Migration sequence

1. Classify production `expect`/panic sites in tree/runtime/rendering code.
2. Keep true external/config errors as typed `Result`.
3. Introduce invariant accessors/diagnostic failure path.
4. Replace generic live-arena/feature-state expects in core lifecycle code.
5. Implement global debug invariant verification.
6. Call verification in targeted mutation-heavy tests.
7. Audit remaining expects; document those intentionally outside the invariant layer.

## Hard invariants

- recoverable user errors never panic;
- impossible internal corruption never gets silently ignored;
- invariant panic messages contain enough identity/phase context to debug the bug;
- invariant helpers do not allocate materially on successful hot paths;
- release builds do not run O(n) global verification per frame;
- debug verification itself never mutates retained state.

## Tests

- normal mount/update/layout/paint/unmount passes global verification;
- dynamic-child churn passes after every commit;
- deep-tree replacement passes verification;
- test-only controlled corruptions produce the expected diagnostic category/message;
- stale external IDs still return `TreeError` rather than panic;
- feature-state mismatch detection is covered.

## Acceptance criteria

- generic `expect("present")`/`expect("mounted")` usage in core retained lifecycle code is substantially eliminated;
- live element/render/feature access goes through intentional invariant helpers;
- global retained consistency verifier exists for tests/debugging;
- recoverable versus invariant failure policy is documented and mechanically reflected in APIs;
- no blanket panic-catching or error-swallowing workaround is introduced.

## Validation

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p incular-widgets --all-features
cargo test -p incular-runtime --all-features
cargo test-constrained --all-features
```

## Completion report

Report expect/panic counts before and after in the core retained paths, the invariant categories implemented, verifier coverage and representative diagnostic output.

### Completed

- Added centralized retained-tree invariant access through live element/render helpers, element-to-render ownership checks, and typed feature-state accessors.
- Added `WidgetTree::verify_invariants()` as a read-only diagnostic pass; it is not called from release-frame hot paths.
- Implemented invariant categories: `Identity`, `Topology`, `Ownership`, `FeatureState`, `Compositor`, `Semantics`, `Interaction`, and `DynamicChildren`.
- Verifier coverage includes root validity, element/render parent-child symmetry, one-to-one render ownership, feature-kind compatibility, compositor layer references/root ownership, semantic references, pointer/gesture/drag/selection references, inherited dependency references, and dynamic-child/sliver bookkeeping.
- Added verification to lifecycle, generated-child churn, and 4096-depth replacement/lifecycle tests. Controlled corruption tests cover feature-state mismatch and ownership diagnostics; stale external IDs continue to return `TreeError`.
- Generic live/feature `expect` audit: **155 -> 8**. Total `expect`/`panic!` sites in the retained-tree paths: **179 -> 23**.
- The 8 remaining generic sites are intentionally narrow: three widget-descriptor handle assertions, two nested retained viewport initialization assertions, two interaction-local state assertions, and one centralized invariant helper assertion after a prior live-render check.

Representative controlled-corruption diagnostic:

```text
Incular retained-tree invariant violation [Ownership]: test controlled render corruption; phase=idle; element=Some(ElementId(ArenaId(0, 0))) widget=Box parent=None owned_render=RenderObjectId(ArenaId(0, 0)); render=Some(RenderObjectId(ArenaId(0, 0))) ; path=[]
```

Validation completed successfully:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p incular-widgets
cargo test -p incular-runtime
cargo test-constrained --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
git diff --check
```
