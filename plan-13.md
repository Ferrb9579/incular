# Plan 13 - Replace ambient build environment state with BuildContext dependencies

## Goal

Replace thread-local, dynamically scoped build environments with an explicit retained dependency system modeled after Flutter's `BuildContext`/inherited-widget mechanism.

The public API should become more Flutter-like while the internal dependency/invalidation behavior becomes easier to reason about.

## Current problem

The tree currently uses `BUILD_ENVIRONMENT` thread-local stacks plus `Rc<dyn Any>` environment chains. `current_build_environment::<T>()` works only because a builder happens to execute inside an ambient dynamic scope on the current thread.

This hides data dependencies and prevents precise retained invalidation of consumers.

## Target model

Introduce an explicit `BuildContext` tied to a live element/build operation.

Core operations should conceptually include:

```rust
impl BuildContext<'_> {
    pub fn depend_on<T: InheritedValue>(&self) -> Option<T::Value>;
    pub fn find<T: InheritedValue>(&self) -> Option<T::Value>;
}
```

`depend_on` registers a dependency so a changed inherited value dirties only dependent elements. `find` performs a non-subscribing lookup for cases where dependency tracking is intentionally unnecessary.

Concrete API names should follow Flutter parity where useful (`Theme::of(context)`, etc.).

## Retained environment storage

Store inherited scopes in retained element data, not TLS.

A heterogeneous value still requires controlled type erasure internally. Use `TypeId`/`Any` behind a typed key/value abstraction, but keep it confined to the environment subsystem.

The important change is:

- explicit context lookup;
- retained scope ownership;
- dependency registration;
- targeted invalidation.

Do not attempt to eliminate all type erasure by creating a giant enum of every possible inherited value.

## Builder APIs

Update builder callbacks to receive `BuildContext` where they may consult inherited state. For example, LayoutBuilder should align with Flutter's conceptual shape:

```rust
Fn(BuildContext<'_>, Constraints) -> Widget
```

Higher-level widgets/material components should use context-based accessors rather than ambient globals.

Because Incular is pre-public, change signatures directly. Do not retain duplicate legacy builder APIs.

## Invalidation

Each inherited scope tracks dependent element IDs by typed key/scope generation. When a value changes:

- compare according to the value's defined update policy;
- mark only dependents whose observed value changed;
- remove stale dependency edges on rebuild/unmount;
- do not force whole-subtree environment propagation merely because an ancestor scope changed.

Provide an opt-in update predicate equivalent in spirit to Flutter's inherited notification semantics where needed.

## Migration sequence

1. Introduce `BuildContext` and typed inherited-key/value abstraction.
2. Add retained scope lookup and dependency registration.
3. Convert LayoutBuilder/environment-backed builders to explicit context.
4. Convert Material theme and other ambient APIs to `of(context)` style access.
5. Add precise dependency invalidation and cleanup.
6. Remove `BUILD_ENVIRONMENT`, `with_build_environment`, boundary TLS and ambient getters.
7. Remove old full-subtree environment propagation paths if no longer required.

## Hard invariants

- inherited lookup never depends on thread-local dynamic execution state;
- dependencies belong to live element identities and are removed on unmount/rebuild;
- context cannot outlive the build operation/borrow from which it was created;
- a changed inherited value invalidates dependents, not unrelated descendants;
- environment boundaries are represented in retained topology, not TLS sentinels;
- Material/control layers remain decoupled from `incular-widgets` concrete types through typed keys/traits.

## Tests

- nearest-scope lookup;
- nested scopes of the same and different types;
- explicit boundary behavior;
- dependency registration/removal;
- only dependent elements rebuild on a value change;
- non-subscribing lookup does not create an invalidation edge;
- reparent/update behavior;
- unmount leaves no stale dependent IDs;
- Theme-style public API parity tests.

## Acceptance criteria

- no thread-local build environment stack remains;
- builders that can consume inherited data receive `BuildContext` explicitly;
- typed inherited lookup and dependency tracking are retained-tree features;
- Material/theme APIs use context-oriented Flutter-like access;
- broad environment propagation is removed or reduced to cases with a documented need;
- old ambient environment APIs are deleted.

## Validation

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p incular-widgets --all-features
cargo test -p incular-material --all-features
cargo test -p incular-runtime --all-features
cargo test --workspace --all-features
```

## Completion report

Implemented and validated.

- Removed the build-environment TLS stack and the `BUILD_ENVIRONMENT`, `BuildEnvironmentFrame`, `InheritedEnvironment`, `compose_environment`, `with_build_environment`, `with_build_environment_boundary`, `current_build_environment`, and related ambient getter paths.
- Added a borrowed widgets-facing `BuildContext<'_>` backed by retained core dependency contexts. `depend_on`/`depend_on_shared` subscribe the current retained consumer; `find`/`find_shared` perform non-subscribing lookup.
- Each retained element owns independent build and render dependency consumers. Inherited scopes and lookup boundaries are retained topology, stale dependencies are cleared on rebuild, and both dependency consumers are removed on unmount.
- Same-type inherited value changes invalidate exact subscribers through the dependency tracker. Broad subtree context rebinding is reserved for actual inherited-topology changes such as adding/removing a boundary or changing the provided type.
- LayoutBuilder, stateful/dynamic builders, navigation scopes, controls, Material theme/component access, examples, tests, and devtools callers now receive/use explicit context. No legacy one-argument compatibility builder or ambient environment API was retained.
- `inherited_context_contracts` proves nearest-scope shadowing, boundary isolation, non-subscribing lookup, stale-edge removal, unmount cleanup, and dependent-only rebuilds. For the targeted invalidation case the dependent builder moves from 1 to 2 builds while its adjacent non-subscriber remains at 1.
- Controlled type erasure intentionally remains only inside the inherited environment transport (`TypeId` plus `Rc<dyn Any>`); application-facing lookup remains typed.
- Descriptor-pointer equality is now an O(1) reconciliation bailout, allowing stable generated children to remain untouched during an inherited-scope value update.

Validation passed: workspace/all-target compile, clippy with `-D warnings`, core/widgets/controls/material/runtime tests, full workspace all-feature tests, rustdoc with `-D warnings`, tree performance contracts, formatting, `git diff --check`, and a source search confirming the removed ambient build-environment symbols no longer exist under `crates/`.
