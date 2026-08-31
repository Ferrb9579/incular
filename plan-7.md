# Plan 7 - Establish exceptional open-source quality gates

## Goal

Make code quality mechanically enforceable before open sourcing Incular. A clean local tree and every protected branch/PR must pass the same reproducible gates; "we usually run clippy" is not sufficient.

This plan includes fixing the current `fmt` and `clippy -D warnings` failures, but the main deliverable is preventing recurrence.

## Required baseline

The repository should have zero known formatting/lint/test debt before publishing.

Immediate cleanup:

- `cargo fmt --all -- --check` must pass;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` must pass;
- remove or justify existing warning noise from test/example inclusion patterns;
- audit `#[allow(...)]` attributes and keep only narrowly scoped, documented exceptions;
- all workspace tests must pass from a clean checkout.

## CI matrix

Add GitHub Actions workflows with separate, readable jobs rather than one opaque mega-job.

Minimum PR gates:

1. formatting;
2. clippy with `-D warnings`;
3. workspace tests;
4. documentation build with warnings denied;
5. feature-matrix checks for meaningful optional features;
6. Linux build/test;
7. Windows build/test;
8. macOS build/test where runners support required dependencies;
9. Android/iOS compile checks where practical and stable.

Use caching carefully, but never cache generated state in a way that can hide missing dependencies or stale generated files.

## Toolchain policy

Commit a `rust-toolchain.toml` or clearly document the supported stable toolchain/MSRV policy.

Choose one of:

- latest stable with an explicitly updated MSRV; or
- fixed MSRV plus latest stable CI jobs.

Do not accidentally claim an MSRV that CI does not test.

## Additional quality tooling

Add only tools that enforce meaningful policy:

- `cargo deny` for advisories, licenses, bans and duplicate-dependency policy;
- `cargo machete` or equivalent for unused dependencies if it works reliably with the workspace;
- `cargo semver-checks` once public APIs are being released/versioned;
- Miri for crates containing meaningful unsafe code or unsafe-sensitive abstractions;
- `cargo llvm-cov` for visibility into coverage, initially informational unless a justified threshold is established.

Do not chase arbitrary coverage percentages. Critical algorithms need deliberate tests, not line-count theater.

## Unsafe policy

Create a repository policy for unsafe Rust.

Requirements for every production unsafe block:

- narrow scope;
- preceding `// SAFETY:` comment describing the exact invariant;
- no avoidable unsafe used only for micro-optimization without benchmark evidence;
- targeted tests around the abstraction boundary;
- Miri where the code path is compatible;
- unsafe is hidden behind a safe API wherever possible.

Consider `#![deny(unsafe_op_in_unsafe_fn)]` workspace-wide if compatible with the supported compiler.

Do not globally `forbid(unsafe_code)` because platform/GPU integration legitimately needs unsafe boundaries.

## Lint policy

Define workspace lint configuration in root `Cargo.toml` where supported so crates do not drift.

Use:

- Rust warnings denied in CI;
- Clippy warnings denied in CI;
- selected pedantic/restriction lints only when they encode actual project policy;
- narrow local allows with comments when a lint is intentionally violated.

Do not enable hundreds of noisy lints and then scatter `#[allow]` everywhere.

## Documentation quality

For public crates/APIs:

- enable `#![warn(missing_docs)]` or a staged equivalent once current public surface is documented;
- document safety, ownership, invalidation and threading contracts;
- include runnable examples for primary public APIs;
- ensure rustdoc builds with `-D warnings`;
- avoid exposing internal implementation types merely to avoid writing facade APIs.

Before the first public release, audit every `pub`/`pub(crate)` boundary for intent.

## Test taxonomy

Keep tests intentionally layered:

- unit tests for local algorithms;
- property tests for reconciliation/geometry invariants;
- integration tests for public APIs;
- performance contract tests for retained behavior;
- platform smoke tests;
- examples compiled/tested as user-facing documentation;
- regression tests named after behavior, not task numbers.

Avoid giant test files becoming their own maintenance problem; split by subsystem/contract.

## Performance gates

Do not put noisy wall-clock microbenchmarks directly in required PR CI.

Instead:

- keep deterministic operation-count/performance-contract tests in CI;
- run Criterion benchmarks on demand or scheduled dedicated runners;
- keep benchmark baselines for reconciliation, layout, text, semantics and renderer hot paths;
- require benchmark evidence for architectural changes expected to affect hot paths.

## Dependency and supply-chain policy

For open source:

- document license policy;
- run advisory checks;
- review large/native dependencies explicitly;
- avoid vendored binary blobs unless legally/technically necessary and documented;
- keep lockfile policy appropriate for the workspace/release model;
- use mature crates instead of reimplementing solved infrastructure, while still auditing dependency quality.

## Generated/spec files

The repo contains parity/spec/generated metadata. Define which files are canonical sources and which are generated.

For generated checked-in files:

- one documented regeneration command;
- CI verifies regeneration produces no diff;
- generated files include a clear header where format permits;
- no hand-editing generated outputs.

## Contributor experience

Add one canonical local validation command/script, for example:

```text
cargo xtask ci
```

or an equivalent workspace script that runs the same core gates as CI.

Do not require contributors to memorize a dozen inconsistent commands.

The command must fail fast enough to be useful but print the exact underlying failing command.

## Migration sequence

1. Fix current fmt/clippy failures and warning noise.
2. Define root lint/toolchain policy.
3. Add core Linux CI gates.
4. Add Windows/macOS/platform compile matrix.
5. Add rustdoc and feature-matrix checks.
6. Add `cargo deny` and unsafe/Miri jobs.
7. Add canonical local CI command.
8. Add generated-file consistency check.
9. Add public API/missing-doc staging before first release.
10. Document branch protection requirements.

## Hard invariants

- protected branches cannot merge code failing fmt, clippy, tests or docs;
- CI runs from a clean checkout;
- no blanket lint suppression to make CI green;
- no ignored failing tests without a linked, documented rationale;
- generated-file drift is detectable;
- unsafe code is reviewable and justified;
- release tags are created only from a fully green commit.

## Acceptance criteria

- clean checkout passes canonical local validation;
- GitHub CI enforces equivalent core gates;
- current fmt/clippy failures are gone;
- platform matrix is documented and functioning;
- unsafe policy is documented and audited;
- dependency/license/advisory policy is automated;
- public rustdoc builds warning-free;
- contributors have one clear validation entry point.

## Validation

At minimum the canonical command must cover equivalents of:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
cargo deny check
```

Platform-specific and Miri jobs may run separately.

## Completion report

Report every required CI job, supported toolchain/MSRV, platform coverage, remaining intentional lint allows, unsafe-site audit results and the exact canonical local validation command.

## Completion report - 2026-08-31

Plan 7 is complete with one explicit owner-directed scope change: **no GitHub CI
workflows and no `xtask` wrapper are included**. Quality gates remain direct,
documented Cargo commands rather than repository automation.

### Toolchain and MSRV

- Normal development: latest stable via `rust-toolchain.toml` with minimal
  profile, `rustfmt`, and Clippy.
- Declared/proven MSRV: **Rust 1.88.0**.
- Rust 1.87.0 was tested and rejected because the locked transitive
  `ar_archive_writer 0.5.1` graph uses syntax unavailable there.
- `cargo +1.88.0 check --workspace --all-features` passes.

### Quality gates validated directly

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
cargo test -p incular-widgets --test tree_performance_contracts --all-features
cargo +1.88.0 check --workspace --all-features
cargo deny check -W unmaintained
cargo machete --with-metadata
```

All commands above pass. `cargo-deny` intentionally reports transitive duplicate
versions for review and reports RUSTSEC-2026-0192 (`ttf-parser` unmaintained) at
warning level; advisories, bans, licenses, and sources all finish OK and there is
currently no safe upstream upgrade path through `winit`.

Facade feature checks also pass for no-default-features, `controls`, `material`,
and all-features configurations.

### Platform coverage

- Windows host workspace/all-feature validation: passed.
- `aarch64-linux-android`: `incular-android` compile check passed locally.
- `aarch64-apple-ios`: `incular-ios` compile check passed locally.
- Linux/macOS native runtime execution was not performed on this Windows host.

### Dependency hygiene

- Added `deny.toml` for advisory, license, ban, wildcard, registry, and git-source
  policy.
- `cargo machete --with-metadata` reports no unused dependencies.
- Real unused manifest dependencies found during the audit were removed.
- macOS/Windows shared-runner dependencies that cargo-machete cannot discover
  through the cross-package `#[path]` source inclusion have narrow documented
  machete exceptions rather than workspace-wide suppression.

### Unsafe audit

Production code contains exactly three real unsafe blocks:

1. WGPU temporary surface creation for adapter selection.
2. WGPU retained renderer surface creation.
3. Windows crash-event handle creation.

All three have nearby `SAFETY:` comments explaining the lifetime/handle
invariants. Workspace `unsafe_op_in_unsafe_fn = "deny"` remains enabled.

### Lint cleanup and intentional allows

The workspace passes all-target/all-feature Clippy with `-D warnings`.
Remaining production allows are narrow and fall into these deliberate groups:

- facade/module re-export `unused_imports` required by public API organization;
- renderer/compositor `too_many_arguments` on explicit hot-path state plumbing;
- Flutter/API parity naming (`non_camel_case_types`, `non_upper_case_globals`);
- a small number of retained diagnostics/compatibility declarations marked
  `dead_code`;
- documented complex callback types and the intentional rich-text enum size.

No blanket crate-wide warning suppression was added.

### Documentation and performance

- Warning-free workspace rustdoc passes.
- Existing stale intra-doc links were repaired instead of weakening the lint.
- Criterion benchmark targets were audited and documented; deterministic
  retained-performance contracts remain the stable regression gate.
- `missing_docs` remains staged rather than globally enabled while the broad
  pre-1.0 parity API is still being documented.

### Contributor/release policy

`docs/QUALITY.md` and `CONTRIBUTING.md` now document the direct validation
commands, MSRV policy, unsafe policy, supply-chain policy, test taxonomy,
benchmark policy, and generated/spec-file policy. There is intentionally no CI
or custom task runner per the repository owner's direction.
