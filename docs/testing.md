# Local testing and release evidence

The root is a virtual workspace. Workspace/architecture tests live in
`crates/incular/tests/`; crate tests live under their own `tests/`, and example
simulation helpers under `examples/<example>/tests/`. Registry archives contain
library sources and selected self-contained examples, not repository-policy tests.

GitHub Actions intentionally runs **build checks only**. Run tests locally:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo test-constrained --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
python scripts/check-features.py
cargo deny check -W unmaintained
cargo machete --with-metadata
```

Build rustdoc with `RUSTDOCFLAGS=-D warnings`:

```powershell
$env:RUSTDOCFLAGS='-D warnings'
cargo doc --workspace --all-features --no-deps --locked
```

On POSIX shells use `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked`.
Doctests run with the local test suite. Do not claim a successful suite if it was
interrupted, only compiled, or skipped native cases.

## Native scenarios

Several desktop executables skip unless `INCULAR_DESKTOP_LIVE_TESTS=1` is set.
Enable this only in an interactive test desktop; it opens native windows.

```powershell
$env:INCULAR_DESKTOP_LIVE_TESTS='1'
cargo test -p incular-desktop --test in_place_resize --test two_window_resource_churn --test window_control --locked
Remove-Item Env:INCULAR_DESKTOP_LIVE_TESTS
```

Use focused scenarios on each native OS. Record commit, OS/architecture,
GPU/driver/backend, display protocol, test command, pass/fail/skipped status,
and observed limits. Include DPI/resize, multi-window teardown, IME and Unicode,
keyboard focus, pointer/touch, clipboard/dialogs, access through the OS screen
reader, surface failure, async cancellation and resource churn. Source parity or
cross compilation does not substitute for this evidence.

## Security regressions

```text
cargo test -p incular-devtools --test discovery --test transport_security --locked
cargo test -p incular-image --test cache_policy --locked
```

The transport regressions use loopback sockets, not GUI input. Discovery tests
reserve a unique port and remove their own records. They never rewrite global
environment variables or remove another application's discovery records.

See [the release runbook](releasing.md) for archive, registry consumer, secret
scan, MSRV, and docs.rs checks. Full local validation is required before upload.
