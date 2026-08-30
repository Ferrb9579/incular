# Contributing to Incular

Thanks for helping improve Incular. The project is experimental, so small,
focused changes with clear tests are especially valuable.

## Before opening a pull request

1. Explain the user-visible behavior or invariant the change addresses.
2. Add or update a focused unit/integration test for regressions.
3. Update an example simulation when the change affects application behavior.
4. Update the relevant crate README or `docs/` note when an ownership boundary
   or public API changes.
5. Run the workspace checks from the repository root:

   ```text
   cargo fmt --all -- --check
   cargo check --workspace --all-targets
   cargo test-constrained
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   ```

   `cargo test-constrained` preserves Cargo's normal build parallelism while
   running the stock test harness on one test thread.

   For documentation changes, also run `cargo doc --workspace --no-deps` with
   `RUSTDOCFLAGS=-D warnings`.

Keep renderer-independent APIs out of `incular-wgpu`, keep platform-specific
code in its platform crate, and avoid dependency cycles. Prefer explicit error
handling at public boundaries and document safety invariants around `unsafe`
code.

## Commit and review guidance

Use a descriptive commit subject, keep unrelated formatting or refactors out
of a feature change, and call out platform-specific validation that you could
not run locally. Reviewers should be able to reproduce the reported behavior
from the tests, examples, or a short reproduction case.
