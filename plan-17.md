# Plan 17 - Reorganize source by subsystem without fragmenting ownership

## Goal

Make the codebase easy for an open-source contributor to navigate while preserving `WidgetTree` as the deliberate central owner of retained UI state.

This is source/module architecture cleanup, not a request to split `WidgetTree` into a collection of manager objects.

## Current problem

Several files carry too many unrelated implementation concerns: `tree.rs`, `tree/widget.rs`, desktop runner files and large test aggregations. Some integration tests compile source files directly with `#[path = "../src/..." ]`, creating secondary compilation contexts and tooling/lint awkwardness.

Large files are not automatically bad. The issue is weak locality: a contributor looking for widget ownership, structural metadata, constructors or layout dispatch may need to navigate thousands of lines of unrelated code.

## Module principles

- organize by stable subsystem/responsibility, not arbitrary line-count limits;
- keep state ownership centralized even when `impl WidgetTree` methods live in many modules;
- facade modules expose the intended API; implementation modules stay private;
- avoid cyclical "utils" modules containing unrelated helpers;
- no source file should be included into another crate/test using path tricks when a normal module/public-test boundary can express the relationship.

## Target widgets/tree layout

Exact names may adjust during implementation, but converge toward something like:

```text
tree/
  mod.rs                 WidgetTree state + shared IDs/errors/policy
  element.rs             Element retained representation
  reconciliation/
    mod.rs
    mount.rs
    update.rs
    dynamic_children.rs
  layout/
    mod.rs
    behavior/...
  painting/
    mod.rs
    behavior/...
  interaction/...
  semantics.rs
  focus.rs
  environment.rs         BuildContext/inherited state from Plan 13
  invariants.rs          Plan 14 invariant access/verification

widget/
  mod.rs                 opaque Widget facade
  node.rs                descriptor ownership from Plan 9
  kind.rs                canonical structure from Plan 8
  specs/                  typed built-in payload groups
  constructors/           concrete conversions where useful
```

Do not create one file per tiny widget purely for aesthetics. Group stable families (layout, interaction, painting/effects, scrolling, semantics).

## Runtime/renderer organization

Apply the same principle to other giant modules:

- separate application lifecycle, frame scheduling, task completion and window state where currently mixed;
- keep WGPU resource/pipeline/render-plan ownership explicit;
- use Plan 12 for desktop shell/platform separation rather than file shuffling alone.

Only split where it improves ownership/locality. Do not introduce forwarding layers whose only job is moving calls between files.

## Test architecture

Remove integration-test `#[path = "../src/..." ]` inclusion patterns.

Use:

- unit tests next to private implementation when private access is required;
- integration tests through public APIs/contracts;
- `pub(crate)` test support modules behind `cfg(test)` where a shared fixture is genuinely needed;
- subsystem-focused test files with behavior names rather than task numbers/dates.

Do not make production internals public just to satisfy integration tests.

## Documentation/navigation

Each major module should have a short module-level doc explaining:

- what it owns;
- what it explicitly does not own;
- important lifecycle/invalidation invariants;
- adjacent subsystem boundaries.

Keep these architectural notes concise and accurate; avoid comments that narrate obvious code.

## Migration sequence

1. Land Plans 8-16 first where they materially change boundaries.
2. Produce a responsibility map of the remaining large modules.
3. Move code by stable subsystem with no behavioral changes per move where possible.
4. Fix imports/visibility rather than creating re-export mazes.
5. Remove source-inclusion tests and production `#[path]` workarounds not already removed by Plan 12.
6. Split giant test aggregations by contract.
7. Add/update module docs and architecture documentation.
8. Run full formatting/lint/docs/dependency audits to catch boundary mistakes.

## Hard invariants

- `WidgetTree` remains the central retained-state owner;
- module splitting does not duplicate state or introduce manager synchronization;
- no circular dependency is solved by widening public visibility unnecessarily;
- no source file is compiled in multiple crates as an architectural shortcut;
- hot-path code does not gain dynamic dispatch/heap allocation because of module organization;
- public API paths remain deliberate and minimal after Plan 16.

## Acceptance criteria

- `tree.rs`/widget implementation responsibilities are separated into coherent modules;
- no arbitrary file-size target drives design;
- no cross-crate production `#[path]` source inclusion remains;
- integration tests no longer compile private source files directly;
- major modules document ownership/boundaries;
- no new manager-object architecture fragments `WidgetTree`;
- full dependency and visibility audits pass.

## Validation

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
cargo machete --with-metadata
```

## Completion report

Completed.

Old-to-new module map:

- `tree.rs` remains the central `WidgetTree` retained-state owner. Immutable descriptor types moved to `tree/descriptors.rs`, built-in payloads and the private `WidgetKind` taxonomy moved to `tree/specs.rs`, and build/inherited context moved to `tree/context.rs`. The root file is now 991 lines instead of roughly 2,879.
- `tree/widget.rs` was replaced by the private `tree/widget/` subsystem: `mod.rs` is the opaque `Widget` facade, `constructors.rs` owns concrete conversions/constructors, `geometry.rs` owns widget-structure geometry helpers, and `structure.rs` owns the exhaustive structural classification/traversal contract.
- `reorderable.rs` moved under `scrolling/reorderable.rs`, alongside the sliver and scrolling implementation it adapts.
- Material's implementation-only `components.rs`/`components/` moved to `component_impl.rs`/`component_impl/`. The public `components` facade remains unchanged. Buttons, foundation and P0 controls now use ordinary Rust submodule declarations instead of `#[path]` workarounds.
- The runtime integration-test aggregator now loads normal `runtime/*` submodules and no longer duplicates the 52 tests already owned by those focused files. `runtime.rs` fell from roughly 2,602 to 963 lines.
- Task/date-based widget test filenames were replaced with behavior names (`action_focus`, `advanced_scrolling`, `advanced_slivers`, `app_shell`, `navigation_scopes`, `reorderable`).

Source-inclusion cleanup:

- Integration tests no longer compile widget implementation files directly with `#[path = "../src/..."]`; the advanced scrolling/sliver, app-shell/platform-menu and raw-tooltip tests exercise normal crate APIs/contracts.
- No `#[path]` attribute remains anywhere under `crates/`.
- Platform-menu tests live under `tests/` and use the public delegate contract; no test backend or test module is compiled from production `src/` code.
- Existing example scenario harnesses under `examples/` still use their purpose-built example/test inclusion mechanism; they do not compile private crate production sources and are outside this crate-module migration.

Visibility/API changes:

- `WidgetTree`, `WidgetKind`, and retained implementation modules were not made public to make the split compile.
- The canonical Flutter platform-menu family is deliberately exported from `incular-widgets`; the temporary test-only memory delegate/command-log API was removed instead of widening production visibility for tests.
- Material's existing public `components` facade is preserved while its implementation subtree stays private.
- Source-level boundary tests were updated to inspect subsystem directories rather than hard-code the former monolithic filenames, preserving the same privacy/API invariants across future file moves.

Largest remaining modules are intentional responsibility boundaries rather than line-count leftovers:

- `tree/widget/structure.rs` (~2,192 lines): one sealed exhaustive widget structural contract. Splitting the taxonomy/matching/traversal tables would reduce locality and make exhaustiveness harder to audit.
- `scrolling/sliver_descriptors.rs` (~1,917): the cohesive public sliver descriptor family.
- `incular-runtime/src/frame.rs` (~1,693) and `application.rs` (~1,646): frame orchestration and application lifecycle respectively; both already have single ownership concerns and were not replaced with forwarding managers.
- `incular-rendering/src/compositor.rs` (~1,595): retained compositor/layer ownership remains intentionally co-located.
- `scrolling/layout.rs` (~1,506): one performance-sensitive scrolling/sliver layout dispatch boundary.

`WidgetTree` still exclusively owns mounted identity and mutable retained layout/paint/interaction state. The reorganization introduced no manager objects, duplicated retained state, synchronization layer, hot-path dynamic dispatch, or organization-driven allocation.

Final validation:

```text
cargo fmt --all -- --check                                      pass
cargo clippy --workspace --all-targets --all-features -- -D warnings  pass
cargo test --workspace --all-features                           pass
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps  pass
cargo machete --with-metadata                                   pass (no unused dependencies)
git diff --check                                                pass
```
