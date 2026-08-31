# Contributing to Incular

Thanks for helping improve Incular. The project is experimental, so small,
focused changes with clear tests are especially valuable.

## Before opening a pull request

1. Explain the user-visible behavior or invariant the change addresses.
2. Add or update a focused unit/integration test for regressions.
3. Update an example simulation when the change affects application behavior.
4. Update the relevant crate README or `docs/` note when an ownership boundary
   or public API changes.
5. Install the repository's two optional dependency-audit tools when working on
   dependency, release, or open-source readiness changes:

   ```text
   cargo install cargo-deny --locked
   cargo install cargo-machete --locked
   ```

6. Run the relevant repository checks from the repository root:

   ```text
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo test-constrained --all-features
   cargo doc --workspace --all-features --no-deps
   ```

   Run the documentation command with `RUSTDOCFLAGS=-D warnings`. For dependency
   policy changes, also run `cargo deny check -W unmaintained` and
   `cargo machete --with-metadata`.

   See [`docs/QUALITY.md`](docs/QUALITY.md) for the supported toolchain/MSRV,
   unsafe policy, dependency policy, and validation matrix.

Keep renderer-independent APIs out of `incular-wgpu`, keep platform-specific
code in its platform crate, and avoid dependency cycles. Prefer explicit error
handling at public boundaries and document safety invariants around `unsafe`
code.

## Error and panic policy

Incular distinguishes three failure classes. Application-authored configuration
errors (duplicate keys, invalid generated children, stale public IDs, and other
validated input) must return a structured error or framework diagnostic; they
must never use `panic!`, `unwrap`, or `expect` as error transport. Validate
before mutating retained topology whenever practical, especially for lazy
builders that can allocate elements, compositor layers, or subscriptions.

Framework invariant failures are bugs in Incular itself. A locally proven
`expect` is acceptable on an impossible arena/state transition when the message
identifies the invariant; do not turn these into ordinary user errors. Hot
successful paths must not allocate merely to prepare invariant diagnostics.

Public APIs may intentionally panic only when the panic is part of the API
contract (for example a strict `Foo::of()`/getter paired with a fallible
`maybe_of()`/`try_*` form). Such methods must document the panic/misuse contract.
New production panic sites should be rejected in review unless they clearly fit
one of these invariant or explicit-contract categories.

## Test placement

All test code belongs in a `tests/` directory. Put crate tests in
`crates/<crate>/tests/`, workspace integration tests in the repository-root
`tests/`, and example tests in `examples/<example>/tests/`. Production `src/`
files and example entry-point directories must not contain inline test modules,
test helpers, or test-only implementations.

## Commit and review guidance

Use a descriptive commit subject, keep unrelated formatting or refactors out
of a feature change, and call out platform-specific validation that you could
not run locally. Reviewers should be able to reproduce the reported behavior
from the tests, examples, or a short reproduction case.
