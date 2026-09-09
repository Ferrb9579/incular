# Code-quality audit: callback and transport ownership

Baseline: `f068993`, Windows, 2026-09-09. Initial working tree clean.
This is a repository-wide architectural review with targeted source tracing,
not an exhaustive line-by-line or native-platform certification. Historical
audits remain historical; this report does not reopen their fixed defects.

## Preserve

- Core `reactivity.rs`: one dependency engine, owned subscription edges,
  notifications outside registry borrows, immediate removal during dispatch.
- Runtime `request_channel.rs` and `NATIVE_REQUESTS.md`: serialized admission
  versus shutdown, checked request identity, completion-once, cancellation and
  wake/destructor calls outside locks. Do not invent another executor.
- Retained widget identity, shared invalidation, renderer-neutral commands and
  the property/geometry ledgers. Large coherent owners are not automatically debt.
- Owned window surface targets and W2 resource reclamation. Current
  `incular-wgpu/src/renderer/frame.rs:12-63` has an authoritative acquisition
  classifier and reports validation failures instead of pretending to skip.
- Guarded navigator pop/replace already revalidate candidate and revision.
- Painting is an intentional exact re-export, not a second renderer.

## Findings at the baseline

P0 denotes correctness, not automatically UB/security severity. Source-confirmed
findings below need failing regressions before implementation is credited.

| ID | Category | Evidence and consequence | Plan |
| --- | --- | --- | --- |
| Q01 | P0 correctness | `incular-gestures/src/focus.rs:282-382`: highlight setters invoke observers through `set_mode_locked` while the TLS RefCell is mutably borrowed. An observer reading `mode`, changing strategy, or registering another observer panics. `FocusBehavior` carries this into application highlight callbacks. | 16 |
| Q02 | P0 correctness | `focus.rs:945-1028`: FocusScopeNode holds its manager RefMut while FocusManager emits FocusNode callbacks. A listener reading `scope.focused()` conflicts; later state writes can also overwrite callback-driven transitions. | 17 |
| Q03 | P0 correctness / P1 ownership | `incular-devtools/src/session.rs:232-249` starts a blocking receive inside a cancellable select branch. A losing branch leaves a receiver task able to consume an undeliverable reply. `incular-desktop/src/devtools_runner.rs:310-312` uses blocking SyncSender::send on the UI thread. Replies are shared across sessions rather than owned by their originating requests. | 18 |
| Q04 | P1 ownership / P0 reentrancy | `incular-navigation/src/navigator.rs:83-90,154-170,313-345`: parallel route/restoration vectors; push and active-change notifications are separate; reentrant push can publish active B then obsolete active A. set_pages evaluates a user iterator after taking the stack out under RefMut, permitting borrow panic and partial loss on unwind. | 19 |
| Q05 | P3 API / P0 termination | `navigator.rs:533-547,590-603`: attach_child only rejects self. A two-dispatcher active cycle recurses without termination. This is application-triggerable invalid topology, not a demonstrated memory-safety flaw. | 19 |
| Q06 | P4 memory / P1 backpressure | `tools/incular-devtools/src/transport.rs:21-23,345`: unbounded command and unit-wakeup queues; send failures discarded. Slow UI consumes redundant notifications while the shared model already contains the update. | 20 |
| Q07 | Verification: native lifetime | `incular-macos/src/environment.rs:29-47,88-105,114-132,147-194`: owner stores observer tokens, observer blocks capture that owner strongly, removal occurs only in owner Drop. Verify actual native retain graph, callback delivery thread and teardown; no macOS execution or UB claim here. | 21 |
| Q08 | P5 documentation | Widgets README still describes a pending core Signal migration; WGPU README and plan-15 header contain stale W2 status despite completed implementation entries. These are discoverability costs, not fresh implementation failures. | 21 |

Navigation's declarative removal deliberately preserves persisted scope data
(`incular-navigation/README.md:52-59`, existing cleanup regression). Do not
silently change that into permanent deletion while unifying transactions.

## Coverage, risks and remaining work

Reviewed architecture/API/dependency contracts, all crate responsibilities,
historical inventories and current plan progress; traced shared reactivity,
native requests, focus, navigation, target/client DevTools transport, renderer
acquisition and sampled native observer ownership. Crate READMEs were checked
for relevant support and ownership claims. This is not a new exhaustive unsafe
audit, an MSRV run, or proof of all public property behavior.

Current tests strongly cover ordinary focus/navigation behavior and guarded-pop
reentry, but lack notification-time read/mutate/unsubscribe tests and actual
DevTools request/session lifetime tests. Token-format tests are not transport
correctness tests. Add behavioral regressions, not source-spelling assertions.

Plan 15 W3/W5/W7 remains the authority for unfinished property ledgers,
scroll/animation attachment and presentation-owner work. Wide descriptor-to-phase
fan-out remains a maintainability risk; this pass establishes no new geometry
or scroll performance defect. Preserve operation-count contracts and investigate
with measured evidence rather than splitting files or speculative caching.

Native FFI and mobile support remain host-specific. Windows checks cannot prove
AppKit/X11/Wayland behavior. No new dependency, unsafe rewrite, compatibility
deletion or ABI policy is justified by this audit alone.

## Ordered execution

Plans 16-21 specialize existing W6/W9/W4/W8 work without replacing plan 15.
Each plan is independently complete only after reproduction, migration, focused
tests, ordinary workspace validation, diff review and its own commit.

Baseline validation: `cargo fmt --all -- --check` and `cargo check --workspace`
passed on Windows. Further validation belongs to each plan's completion record.

## Implementation progress

Q01 is fixed in plan 16: initial regressions reproduced eight failures; all eleven
expanded highlight tests now pass. The default and all-feature workspace runs
each report 1,429 passed, zero failed and zero ignored. Formatting, workspace
check, strict Clippy and warning-denied rustdoc also pass on Windows. See plan 16
for callback ordering, lifetime, unwind and performance-contract evidence.

Plans 17-21 are recorded, not implemented by this slice. Q02-Q06 remain source
findings awaiting their focused reproductions/fixes; Q07 remains native lifetime
verification work. This completion does not close plan 15 W6/W9 or certify
open-source readiness across all hosts and feature/MSRV combinations.
