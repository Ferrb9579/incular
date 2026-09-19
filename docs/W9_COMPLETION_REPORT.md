# W9 completion report: tooling and sustainable closure

Implementation date: 2026-09-19 to 2026-09-20.

Baseline commit: `3d6eff658c647ff03399db94bd57c43f76c25a1d`.

Validated implementation commit: `3cd7ac3` (`Complete W9 tooling and sustainable closure`).

W8 remained removed. W9 did not add CI, an `xtask`, a device farm, or a new
test-infrastructure project. Focused regressions and the repository's ordinary
Cargo validation were used as implementation evidence.

## Result

W9 is implemented. The five Plan 15 requirements are closed in source and all
applicable repository closure gates pass on the Windows host. Windows native
desktop regressions were also executed live. Android and iOS compile checks pass
for their installed targets. macOS source ownership is fixed, but native AppKit
runtime execution was not possible on this Windows host; the attempted
`x86_64-apple-darwin` check was blocked before Incular code by `psm` because no
Apple-target C compiler was available. That limitation is compile/runtime
evidence, not an unresolved W9 source defect.

## Plan 15 disposition

### 1. Bound DevTools requests, wakes, sessions, models, and discovery

Completed.

- Target command admission is bounded to 256 queued commands and 4 MiB of
  admitted command payload. Telemetry is bounded to 1,024 queued events and
  16 MiB of admitted payload. Response payload ownership is bounded to 32 MiB.
- Incoming target request frames are capped at 64 KiB. Target WebSocket sends
  have a five-second timeout.
- The UI client admits at most 128 queued requests, 2 MiB of queued/in-flight
  request payload, 64 simultaneous in-flight requests, and 64 KiB per request.
  Requests time out after ten seconds.
- The UI accepts target messages up to 16 MiB. Request IDs use checked
  allocation; an exhausted ID space is terminal rather than wrapping.
- Target command completion is session-owned and one-shot. Cancelling or
  disconnecting one wait cannot consume another request's reply.
- Native event-loop wakeup is coalesced. DevTools work advances without an
  application redraw and the UI model subscription uses one lifecycle-owned
  `tokio::sync::Notify` wait rather than rebuild-spawned blocking receivers.
- A transport disconnect does not automatically replay work. Retry is explicit,
  re-runs discovery for the original PID, authenticates against the newly
  discovered record, starts a fresh request-ID session, and abandons old queued
  work. In-flight mutations are reported as uncertain because the target may
  already have applied them.
- Response payload kind and relevant request identity are correlated before
  model mutation. Request-level protocol errors no longer mark the transport as
  disconnected.
- Discovery scans inspect at most 128 records, each at most 64 KiB, and retain
  at most 32 warnings. Malformed, oversized, unreadable, and invalid records are
  reported without printing authentication tokens.
- The target-side discovery lease owns exact registration/removal. On platforms
  where process liveness cannot be proven portably, the UI no longer pretends a
  stale record is known-dead and does not delete another process's record
  without proof.

Client/model retention limits are explicit:

| Model surface | Limit |
| --- | ---: |
| Frame history | 300 frames |
| Deep trace events | 200,000 events total |
| Tree nodes | 131,072 nodes |
| Tree payload | 32 MiB |
| Frame-arrival entries | 512 |
| Details payload | 4 MiB |
| Signal-list payload | 4 MiB |
| Subscriber payload | 2 MiB |
| Target-info payload | 2 MiB |
| Console retained entries | 500 |
| Console aggregate payload | 4 MiB |
| Single console entry | 8 KiB |
| Target tree snapshot producer | 20,000 nodes |
| Target diagnostic overlay producer | 10,000 nodes |
| Deep-trace producer per frame | 4,096 events |

Target truncation and dropped telemetry stay visible. The UI's existing
100,001-row virtualization regression remains supported: the model limit is
above that fixture instead of imposing a misleading 100,000-row ceiling.

### 2. Separate diagnostic counters from identity allocation

Completed.

Generational arenas, window slots, accessibility native IDs, image/rendering
resource IDs, compositor generations, application-shell resource generations,
reactive/task identities, DevTools request IDs, and other correctness-sensitive
allocators now use checked allocation or permanently retire an exhausted slot.
No stale handle can become valid again through generation wrap.

Correctness-sensitive revision epochs used for rebuild/cache invalidation were
also changed from wrapping/saturating arithmetic to checked progression so an
old revision cannot silently alias a new state. Pure diagnostics and visual
bookkeeping counters remain allowed to wrap where identity/equality is not part
of their contract; examples include cumulative diagnostic counters and repaint
rainbow bookkeeping.

Focused exhaustion/reentrancy evidence includes the arena/native stale-ID tests,
the existing resource-generation tests, and the new
`crates/incular-gestures/tests/focus_scope_transitions.rs` coverage.

### 3. Audit facade features, bridges, aliases, painting, and macros

Completed.

- Facade `devtools` forwarding uses weak optional dependency features so enabling
  portable instrumentation does not implicitly activate desktop backends.
- The standalone DevTools UI enables the exact facade features it owns.
- Eight isolated facade configurations passed: no features; controls; material;
  devtools; desktop; desktop+devtools; defaults; and all features.
- `incular-painting` remains the exact pure `pub use incular_rendering::*;`
  compatibility shim. The architecture contract rejects extra implementation in
  that crate.
- `incular-macros` remains an intentional empty procedural-macro scaffold. Its
  README/support statement now says so instead of claiming an API that does not
  exist.
- `specs/architecture.json` schema 2 classifies all 32 workspace packages with
  owner, API class, support statement, and executable evidence. The root harness
  and standalone DevTools tool are included rather than silently excluded.

Externally observable changes are recorded in
[W9 API migrations](W9_API_MIGRATIONS.md).

### 4. Reconcile parity assertions and executable evidence

Completed.

Parity validation now maps historical vocabulary to reviewed semantic outcomes:
supported, Rust-equivalent, internal, omitted, or deferred. Supported and
Rust-equivalent rows require concrete Incular members and named executable test
evidence; deferred/omitted rows require explicit rationale and decisions instead
of being counted as implemented.

The current ledgers retain their actual denominators rather than a blanket
"complete Flutter parity" claim:

| Ledger | Total | Status distribution |
| --- | ---: | --- |
| `flutter_member_parity.jsonl` | 436 | 436 supported |
| `flutter_api_parity.jsonl` | 331 | 263 implemented, 11 merged, 3 internal, 53 deferred, 1 skipped |
| `widget_parity.jsonl` | 103 | 75 implemented, 18 merged, 3 internal, 5 deferred, 2 skipped |
| `base_ui_component_parity.jsonl` | 34 | 19 adapted, 2 merged, 13 deferred |
| `flutter_widgets_3471_parity.jsonl` | 1,307 | 269 Rustified, 109 merged, 28 deferred-platform, 765 deferred-renderer, 121 skipped-Dart-mechanic, 15 skipped-deprecated |
| `flutter_material_3471_parity.jsonl` | 551 | 111 Rustified, 35 merged, 4 internal, 374 deferred, 27 skipped-legacy |

Architecture allow-lists were not regenerated to accept accidental dependency
edges. The architecture and parity policy tests remain the enforcement points.

### 5. MSRV, targets/features, dependencies, documentation, and build policy

Completed for applicable host/tooling coverage.

- Locked Rust 1.88 all-feature workspace check passes.
- Stable default and all-feature workspace checks pass.
- Default and all-feature constrained workspace tests pass.
- Strict workspace/all-target/all-feature Clippy passes with `-D warnings`.
- Warning-denied workspace rustdoc passes.
- `cargo-deny` blocking advisory/license/source/bans checks pass. It still shows
  nonblocking review warnings for duplicate transitive versions and an allowed
  BSD-2-Clause license that is not currently encountered.
- `cargo-machete --with-metadata` reports no unused dependencies.
- Android `aarch64-linux-android`, iOS `aarch64-apple-ios`, and Windows
  `x86_64-pc-windows-msvc` affected target checks pass.
- The macOS `x86_64-apple-darwin` attempt cannot get through dependency build on
  this Windows machine because `psm` cannot find an Apple-target C compiler.
  No AppKit runtime claim is made from that failed foreign-host attempt.
- `warn(missing_docs)` is now enforced on the DevTools target and macros
  boundaries. Painting deliberately does not add a crate attribute because its
  architecture contract requires the source to remain an exact pure re-export;
  warning-denied rustdoc still covers it in workspace documentation builds.

Build policy was measured before changing configuration. The host reported
49,086,705,664 bytes of RAM, 32 logical CPUs, and an existing `target/` tree of
297,781,824,102 bytes. A warm jobs=4 check measured 1.363 s and a subsequent
jobs=12 check 0.503 s, but the ordering made that comparison cache-biased and
therefore insufficient evidence to raise concurrency. The repository now has
one conservative policy: `jobs = 4`, `test-constrained` uses four jobs, and
incremental compilation is explicitly disabled to agree with the workspace
profiles and `CARGO_INCREMENTAL=0` policy.

## Incorporated historical plan findings

- **Plan 17 focus prerequisite:** resolved. Focus-scope state commits before
  callbacks; callbacks may inspect or change focus without borrow panics, and an
  older outer operation does not overwrite the reentrant decision.
- **Plan 18 / Plan 20 still-applicable tooling findings:** incorporated into the
  DevTools ownership, bounded work, identity, facade, and parity slices above.
  The deleted historical plan files were not restored merely to mark them done.
- **Plan 21 native observer ownership:** resolved in source. macOS environment
  notification callbacks no longer strongly retain the owner of their removal
  tokens; token teardown is explicit/idempotent. Native AppKit execution still
  requires a macOS host and is not claimed here.

## Native and integration evidence

On Windows, the live native regression targets were rerun with
`INCULAR_DESKTOP_LIVE_TESTS=1` and exited successfully:

- `application_shell`
- `display_placement`
- `platform_menu`
- `system_environment`
- `transient_surface`

The standalone DevTools process smoke used the real binaries from the validated
build. `counter.exe` was launched with the target agent enabled; the real
`incular-devtools.exe` was then launched with that target PID. Both processes
remained alive through the smoke interval and were terminated cleanly afterward.
Interactive UI actions such as clicking Retry are not claimed from that hidden
process smoke; their behavior is covered by focused transport/model/UI tests.

## Closure commands

The final concise closure rerun stored full output under the ignored
`target/w9-closure/` directory and reported every gate PASS with overall exit
code 0:

```text
cargo check --workspace --locked
cargo check --workspace --all-features --locked
cargo test-constrained --locked
cargo test-constrained --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo +1.88.0 check --workspace --all-features --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
cargo deny check -W unmaintained
cargo machete --with-metadata
```

`cargo fmt --all -- --check` cannot execute on this Windows checkout because the
expanded rustfmt command exceeds Windows' command-line length limit (OS error
206). The equivalent ordinary Cargo formatter was therefore run package by
package across every package from `cargo metadata`; every
`cargo fmt -p <package> -- --check` passed. No custom formatter wrapper was added
to the repository.

Additional focused evidence passed for DevTools UI/target/protocol, gestures
focus transitions, architecture ownership, parity policy/manifests, facade
feature isolation, Android/iOS/Windows target compilation, documentation linting,
and the Windows live native regressions described above.

## Remaining limitations

There is no unresolved W9 source defect known at closure. The remaining evidence
limitation is platform-host-specific: the macOS observer path has source and
ownership review but not AppKit runtime execution in this Windows session. Its
exact remaining evidence requirement is to run the macOS lifecycle/native tests
on a macOS host with the Apple toolchain/SDK available.

W8 remains removed, and no W9 completion claim depends on recreating it.
