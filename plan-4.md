# Plan 4 - Define a strict error, invariant and panic policy

## Goal

Make failures caused by application-authored widgets return structured framework errors/diagnostics instead of panicking, while keeping true internal invariants cheap and explicit.

Do not blindly replace every `expect()` with `Result`. Hot-path internal invariants and recoverable user errors are different categories and should be modeled differently.

## Failure categories

Define and document three categories.

### 1. User/configuration errors

Examples:

- duplicate sibling keys;
- invalid lazily generated child configuration;
- malformed constraints or invalid public constructor input where validation is possible;
- operations against an unknown/stale public ID.

These must be represented by a structured error or framework diagnostic and must not call `panic!` internally.

### 2. Internal invariant violations

Examples:

- a render child listed by a live element has vanished during a phase that owns the tree exclusively;
- a freshly inserted arena entry cannot be read back;
- an internally balanced stack becomes unbalanced.

These indicate framework bugs. They may use a central invariant mechanism that provides rich context in debug/development builds. Avoid dozens of vague `expect("live")` strings.

### 3. Explicit API contract panics

Rust APIs analogous to Flutter's `Foo::of()` may intentionally panic when the caller chooses a strict accessor instead of `maybe_of()`. Such panics must be documented on the public method and must not be used as an internal error transport mechanism.

## Error types

Expand `TreeError` into a structured hierarchy with source context. Use `thiserror` if the workspace already accepts it or add it deliberately; do not hand-maintain repetitive Display implementations without reason.

Conceptual structure:

```rust
pub enum TreeError {
    MissingElement(ElementId),
    DuplicateKey { key: Key, parent: Option<ElementId> },
    InvalidGeneratedChild {
        owner: ElementId,
        child: GeneratedChildIdentity,
        source: Box<TreeError>,
    },
    InvalidWidgetConfiguration { widget: &'static str, reason: String },
}
```

Avoid storing huge widgets in errors. Include stable IDs, widget class/name and concise diagnostic context.

## Internal invariant helper

Create one small internal helper/macro only if it materially improves diagnostics, for example:

```rust
framework_invariant!(condition, "render child must be live", id = id, phase = phase);
```

Requirements:

- no allocation on the successful path;
- debug message contains IDs and current frame phase where available;
- use is reserved for impossible internal states;
- it does not swallow or convert user errors into framework bugs.

Simple `expect()` is still acceptable for locally proven facts such as `NonZero` created immediately above, but generic `expect("live")` should be reduced where a richer invariant helper gives actionable context.

## Lazy materialization

Change sliver, advanced-scrolling and layout-builder child materialization to return `Result` through the owning refresh/layout path instead of panicking on mount/update errors.

This likely requires making selected internal layout/materialization functions fallible. Keep the fallible boundary as narrow as possible rather than converting every numeric layout helper into `Result`.

Preferred flow:

```text
generated widget
  -> validate/reconcile
  -> TreeError with generated-child context
  -> frame/runtime boundary
  -> application-facing error/diagnostic
```

No `catch_unwind` as routine control flow.

## Runtime boundary

Define one clear boundary where a frame error is surfaced. It should:

- preserve the original structured cause;
- add frame/window/phase context;
- avoid leaving partially mutated topology where possible;
- ensure a failed child mount does not leak compositor layers or subscriptions.

For mutations that can fail after partial allocation, use explicit rollback guards or validate before committing. Do not rely on process abort to keep state consistent.

## Migration sequence

1. Inventory production `panic!`, `unwrap()`, and `expect()` uses by category.
2. Document the panic/invariant policy in contributor documentation.
3. Expand `TreeError` and frame/runtime error context.
4. Convert sliver/advanced child panic paths to structured errors.
5. Convert other application-triggerable panics.
6. Improve high-value internal invariant diagnostics.
7. Add a lint/review check preventing new undocumented production panics.

## Hard invariants

- application-authored invalid widget trees do not panic inside framework internals;
- framework invariant failures are never silently converted to ordinary user errors;
- errors preserve causal context;
- no routine `catch_unwind`;
- failed mount/update does not leak retained elements/renders/layers/listeners;
- hot successful layout/paint paths do not allocate merely for error bookkeeping.

## Tests

- duplicate keys in eager children return `TreeError`;
- duplicate/invalid keys from sliver builders return contextual `TreeError`, not panic;
- advanced scrolling generated-child failure returns an error;
- layout-builder generated-child failure returns an error;
- stale IDs return `MissingElement`;
- intentionally strict public `of()` APIs retain documented panic behavior;
- failure-injection tests verify arena/layer/listener counts return to their pre-operation state.

Where practical, use `catch_unwind` only in tests to assert that user-error cases no longer panic.

## Acceptance criteria

- no `panic!` remains as transport for a recoverable tree/materialization error;
- production panic sites are classified and documented;
- lazy materialization is fallible without leaking partial state;
- errors include enough context to debug an open-source user's issue report;
- no meaningful hot-path regression.

## Validation

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
rg -n "panic!|unwrap\(|expect\(" crates -g "*.rs"
```

## Completion report

Provide counts of production panic/unwrap/expect sites before and after, grouped by category, and list any intentional public contract panics that remain.

### Completed audit

- Baseline `incular-widgets/src`: 180 `panic!`/`unwrap`/`expect` call sites.
- Final `incular-widgets/src`: 175 total: 2 direct `panic!`, 173 `expect`, 0 direct `unwrap`.
- Recoverable generated-child panic/expect transport removed from sliver, advanced-scrolling, and `LayoutBuilder` materialization.
- Remaining direct panics are intentional: the recursion guard reports an internal framework invariant, and `SelectionListenerNotifier::selection()` is the strict documented API paired with `try_selection()`.
- Remaining `expect` sites are internal retained-tree/arena invariants and are not used to transport application-authored configuration failures.
