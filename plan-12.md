# Plan 12 - Establish a real cross-platform desktop runner boundary

## Goal

Share desktop event-loop/rendering code through an intentional cross-platform module/crate instead of compiling the Linux crate's source file inside Windows and macOS packages.

The final ownership should resemble a Flutter-style shell/embedder split: shared desktop shell logic plus explicit per-platform services.

## Current problem

`incular-windows` and `incular-macos` use cross-package `#[path = "../../incular-linux/src/lib.rs"]` source inclusion. The implementation is largely cross-platform Winit code, but its physical/module ownership says Linux.

This confuses dependency tools, platform ownership and contributor expectations.

## Target crate boundary

Create an internal shared desktop crate, tentatively `incular-desktop` (or an equivalently explicit name), that owns:

- Winit application/event-loop integration;
- shared WGPU renderer setup/lifecycle;
- window-id mapping;
- redraw/frame scheduling;
- AccessKit-Winit projection plumbing common to desktop;
- common keyboard/pointer/touch/IME event translation orchestration;
- shared DevTools attachment orchestration where platform-neutral.

OS crates remain real adapters:

```text
incular-desktop     shared shell/runtime integration
incular-linux       Linux services/integration
incular-windows     Windows services/crash integration
incular-macos       macOS services/integration
```

## Platform service interface

Define a small explicit interface only for behavior that actually differs, such as:

- clipboard construction if platform-specific behavior is needed;
- content sensitivity/window flags;
- crash reporting hooks;
- native menu/titlebar/platform channel hooks;
- platform lifecycle quirks.

Do not invent abstract traits for APIs that Winit already normalizes. Use mature Winit/AccessKit/WGPU abstractions directly.

## Public entry points

Each OS crate should expose the same intentional runner facade and delegate to `incular-desktop` with its platform services. Shared implementation types remain internal unless they are part of Incular's real embedder API.

## Remove source inclusion

Production cross-crate `#[path]` inclusion must be eliminated. It must no longer be necessary to add dependency-tool exceptions because another crate secretly compiles Linux source.

## Migration sequence

1. Classify `incular-linux/src/lib.rs` code as platform-neutral or Linux-specific.
2. Create the shared desktop crate/module with platform-neutral code.
3. Define the minimum platform-service seam.
4. Move Linux-specific clipboard/window behavior into `incular-linux`.
5. Wire Windows and macOS adapters explicitly.
6. Move Windows crash reporting into the Windows platform service lifecycle.
7. Remove all cross-package `#[path]` production inclusion.
8. Remove cargo-machete exceptions that existed only for source inclusion.

## Hard invariants

- one shared Winit/WGPU desktop event-loop implementation;
- no copy/paste fork per OS;
- OS-specific behavior is owned by the corresponding OS crate;
- shared desktop code contains no misleading Linux names;
- raw-window/surface lifetime safety remains explicit and documented;
- platform adapters cannot bypass runtime lifecycle/frame ordering.

## Tests

- compile/check shared desktop crate independently;
- platform adapter construction/unit tests where host-independent;
- Windows native smoke test on Windows;
- Linux native smoke test on Linux;
- macOS native smoke test on macOS before first public release;
- event translation contract tests against platform-neutral inputs;
- accessibility adapter lifecycle tests;
- WGPU surface lifetime safety tests where practical.

## Acceptance criteria

- no Windows/macOS source inclusion of `incular-linux`;
- shared desktop logic has one intentional owner;
- platform-specific services are explicit and minimal;
- dependency manifests accurately describe code each crate compiles;
- temporary cargo-machete exceptions for the old arrangement are removed;
- public runner behavior remains platform-consistent.

## Validation

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test-constrained --all-features
cargo check -p incular-linux --all-features
cargo check -p incular-windows --all-features
cargo check -p incular-macos --all-features
cargo machete --with-metadata
```

Also perform native smoke runs on each desktop OS before public release; cross-compilation alone is not sufficient for final sign-off.

## Completion report

Document the final shared/platform split, all moved services, native platforms actually executed, source-inclusion removals and dependency-audit result.

### Completed

- Added `incular-desktop` as the single owner of the shared Winit/WGPU/AccessKit desktop shell.
- Moved the common event loop, renderer/surface lifetime, window-ID mapping, input/IME routing, clipboard integration, accessibility bridge and DevTools attachment code out of `incular-linux`.
- Converted `incular-linux` and `incular-macos` into thin facades over `incular-desktop`.
- Converted `incular-windows` into the same facade while keeping its crash-handler installation Windows-owned and ahead of entry into the shared shell.
- Moved shared DevTools launch contract tests to `incular-desktop`.
- Removed all production cross-package `#[path]` inclusion of Linux source.
- Removed cargo-machete exceptions that existed only because Windows/macOS secretly compiled Linux-owned source.
- Added crate/platform README documentation for the final ownership and raw-window/surface lifetime contract.

Validation completed successfully:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test-constrained --all-features`
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps`
- `cargo check -p incular-desktop --all-features`
- `cargo check -p incular-linux --all-features`
- `cargo check -p incular-windows --all-features`
- `cargo check -p incular-macos --all-features`
- `cargo test -p incular-desktop --all-features`
- `cargo machete --with-metadata`
- `git diff --check`
- repository search confirms no `incular-linux/src/lib.rs` source inclusion, `LinuxWake`, or `LinuxClipboard` remains.

Native execution status: this migration was validated on a Windows host, including native Windows compilation/tests. No interactive GUI smoke window was launched. Linux and macOS native smoke runs remain required before first public-release sign-off, as specified above; their adapters were compile-checked from the Windows host only.
