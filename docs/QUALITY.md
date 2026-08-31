# Quality and release policy

Incular keeps quality policy explicit and repository-local. There is no custom
CI wrapper or `xtask`; contributors use normal Cargo commands so every check is
transparent and independently runnable.

## Toolchain and MSRV

Normal development uses the latest stable Rust selected by
`rust-toolchain.toml`, with `rustfmt` and Clippy installed. The workspace's
declared minimum supported Rust version is **1.88**. The MSRV must not be raised
without first proving `cargo +<version> check --workspace --all-features` on the
locked dependency graph and then updating `workspace.package.rust-version`.

## Required local validation

- formatting (`cargo fmt --all -- --check`);
- all-target/all-feature Clippy with `-D warnings`;
- workspace tests (`cargo test --workspace --all-features`);
- warning-free public rustdoc;
- meaningful feature combinations for the facade crate;
- deterministic retained-performance contracts;
- Android and iOS target compile checks;
- dependency advisory/license/source policy through `cargo-deny`;
- unused dependency detection through `cargo-machete`;
- production unsafe-site review.

## Unsafe Rust

Unsafe Rust is allowed only at platform/GPU boundaries that cannot be expressed
through a safe API. Every production unsafe block or unsafe function must:

1. be as small as practical;
2. have a nearby `// SAFETY:` comment stating the exact lifetime/aliasing/FFI
   invariant relied on;
3. remain behind a safe public abstraction where possible;
4. have focused tests for the surrounding safe contract;
5. satisfy workspace `unsafe_op_in_unsafe_fn = "deny"`.

Native WGPU surface creation and Windows crash-handler registration cannot
execute meaningfully under Miri; those boundaries are covered by explicit
safety documentation, compiler lints, and targeted tests. The current
production unsafe inventory is intentionally small and every site carries a
nearby `SAFETY:` justification.

Current production unsafe boundaries are intentionally limited to WGPU raw
window-handle surface creation and Windows crash-handler registration.

## Dependencies and licenses

The project is Apache-2.0. `deny.toml` is the executable allow-list for accepted
dependency licenses and rejects unknown registries/git sources and wildcard
dependency versions. Multiple dependency versions are reported for review but
are not categorically forbidden because platform/graphics stacks can require
them. Security and yanked-package advisories are blocking. The `unmaintained`
category is warning-level because the current `winit` dependency transitively
includes `ttf-parser` under RUSTSEC-2026-0192 and no safe upgrade is available;
the warning remains visible on every supply-chain run and should be removed as
soon as the upstream dependency graph permits it.

Large native dependencies and any future vendored binary artifacts require an
explicit review note describing why the dependency is necessary and how it is
updated. Prefer mature ecosystem crates over reimplementing solved
infrastructure.

## Documentation and public API

Rustdoc warnings are denied during release/readiness validation. `missing_docs`
is not yet enabled globally:
the pre-1.0 Flutter-parity surface is intentionally broad, and globally allowing
the lint would provide no value. New public APIs should be documented now; the
repository will enable `warn(missing_docs)` crate-by-crate as existing public
surfaces reach documentation parity. `cargo semver-checks` becomes a release
gate once a published baseline version exists; running it before that point
would compare against no meaningful public release.

## Tests and performance

Tests are layered as unit, integration/public-API, property/invariant,
platform, example, and deterministic performance-contract tests. Normal quality
checks should not gate on noisy wall-clock benchmark thresholds. Architecture
changes that are expected to affect hot paths should include benchmark evidence.

Criterion benchmarks cover reconciliation, layout, semantics, widget equality,
runtime signals/restoration, variable scrolling, text layout, and path
tessellation. They remain informational: deterministic operation-count and
behavioral performance contracts are the stable regression gates. Miri and
coverage are useful targeted/manual tools rather than repository automation.

## Generated and specification files

There are currently no checked-in generated source outputs. Files under
`specs/` and parity manifests are canonical source/test inputs, not generated
artifacts, and workspace tests validate their contracts. If generated outputs
are added later, the generator command and a clean-diff verification step must
be documented in the same change; checked-in generated files must not be
hand-edited.

## Validation command set

The open-source readiness baseline is the following direct command set:

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

Feature and platform checks are run separately where relevant. The mobile facade
crates are expected to compile for `aarch64-linux-android` and
`aarch64-apple-ios`.
