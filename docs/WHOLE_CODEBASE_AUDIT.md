# Whole-codebase audit — current baseline

Baseline: `42befb7`, 2026-09-06. Implementation is unchanged by this audit.
Execution plan: [plan-15.md](../plan-15.md). Full file inventory:
[CODEBASE_INVENTORY.json](CODEBASE_INVENTORY.json).

## Scope and evidence

The inventory covers all 30 crates and the DevTools application: 411 production
Rust files, 153,815 lines, plus crate tests/benchmarks and 193 supporting files
under examples, root tests, scripts, Cargo configuration and specifications.
Counts describe scope, not quality. Review used targeted source reads of public
boundaries, retained transfer/execution, ownership, mutation, caches, callbacks,
native adapters, tests and historical plan acceptance criteria. Every crate has
a disposition below. This is a whole-workspace architectural/source audit, not
an assertion that every line or exported option has been exhaustively tested.

Evidence labels: **confirmed** means the current source establishes the stated
behavior; **debt** means an architectural cost with a demonstrated location;
**verification** means a risk requiring a regression or native execution before
claiming a defect. P1 is user-visible correctness or unbounded resource behavior;
P2 is architectural/lifecycle debt; P3 is hardening. New defects below were not
reproduced by running new tests in this planning pass.

## Preserve completed work

Plan 14 A–E and its first F slice are committed. Preserve owned GPU surface
targets; complete retained visibility policy; explicit clipboard failure; shared
native request admission/completion; checkbox mixed semantics; guarded navigator
pop callbacks; one lower-level dependency engine; scoped subscriptions; one
desktop event loop; Winit-free platform/runtime/widgets boundaries; private
validated Constraints; and shared core invalidation flags. These are not open
findings merely because the historical audit still describes the old baseline.

In particular, `WidgetStatesController` now tracks a `DependencySource`
(`crates/incular-material/src/foundation/state.rs:307`), and animation, editing
and forms provide scoped observers. Do not replace these again or describe the
old lack of observation as still current. Preserve signals and immutable widget
descriptions; no Stateless/Stateful split or wholesale trait-object rewrite.

## Current findings

### R01 — Public Material options still do nothing — P1, confirmed

`SliverAppBar::build` discards pinned/floating/snap/stretch and returns a fixed
app bar (`crates/incular-material/src/component_impl/app_shell.rs:344`). Scaffold
discards resize-to-avoid-inset and both extend-body options (`:549`). Card retains
border ordering and semantic-container options as compatibility configuration
without behavior (`component_impl/surfaces.rs:256`). Checkbox ignores `is_error`
(`p0_controls/checkbox_radio.rs:189`). ListTile accepts autofocus but discards it
(`component_impl/list_items.rs:382`). An application can compile and set these
options without receiving the requested behavior or an unsupported result.

Fix each through its existing retained layout/scroll/focus/semantics mechanism,
or explicitly deprecate/remove it with migration guidance. Test one changed
option at a time at execution, not just equality or builder construction.
**False-positive exclusion:** ListTile's minimum leading width and title gap
are passed to `tile_row_configured` at lines 331–332; their appearance in the
later discard tuple is redundant code, not evidence that those options fail.

### R02 — Form validation stops after the first invalid field — P1, confirmed

`crates/incular-widgets/src/forms.rs:170` promises every live field but uses
short-circuiting `.all(...)`; `save` repeats it at `:193`. Later validators do
not run, so their errors can remain absent or stale. Validate all fields into
one result before notifying; save callbacks must still run only if all pass.
Test several invalid fields, previously invalid fields becoming valid, and
all registration orders. Keep callback invocation outside mutable borrows.

### R03 — Length formatter ignores enforcement and can split text units — P1, confirmed

`forms.rs:730` stores `MaxLengthEnforcement`; `format_edit_update` at `:753`
never reads it. It always truncates with `chars().take`, including composition,
and returns old selection/new composing ranges without rebasing them to the
truncated string. A combining sequence or emoji cluster can be split; returned
ranges can exceed the output. Define Incular's grapheme/composition contract,
honor every mode, and normalize selection/composition. Test combining marks,
ZWJ emoji, UTF-8 boundaries and composing commit/cancel through runtime editing.

### R04 — Shared GPU resource lifetime defeats local eviction — P1, confirmed

`crates/incular-wgpu/src/resources.rs:175` retains strong image/gradient maps;
insertion occurs at `:437`/`:496`. Per-renderer eviction at
`renderer/resources.rs:193`/`:209` only removes local entries. The shared maps
have no matching reclamation path. Identity maps also grow independently.
Introduce device-owned budgets, liveness and retirement accounting across
windows and submitted work. Measure actual shared resident bytes separately
from local cache evictions; test churn while another window keeps a resource live.

### R05 — CPU image cache retains encoded and decoded data without a budget — P1, confirmed

`crates/incular-image/src/lib.rs:359` uses `HashMap<Vec<u8>, ImageHandle>`.
`load_bytes` copies the complete encoded input even for lookup; the public cache
has no clear/eviction/budget operation. Bound residency, avoid full-buffer lookup
allocation, and retain collision-safe identity. Live ImageHandles must remain
valid after cache eviction. Text's layout cache already has an entry limit
(`crates/incular-text/src/engine.rs:334`); do not inaccurately call every cache
unbounded. Audit its byte cost and dynamic font lifetime separately.

### R06 — GPU validation failure is treated as a normal skipped frame — P1, confirmed

`crates/incular-wgpu/src/renderer/frame.rs:25` combines Validation, Timeout and
Occluded into `Ok(None)`. Callers cannot distinguish a validation failure from
benign non-presentation through this result. Preserve valid suboptimal-frame
handling and loss recovery, but introduce typed skip/failure reasons, counters
and a defined application recovery policy. Test synthetic acquisition outcomes;
exercise actual device/surface failures where the backend supports injection.

### R07 — Declarative navigation bypasses the imperative transaction — P1/P2, confirmed

`NavigatorState` has parallel route/restoration vectors (`navigator.rs:83`).
`set_pages` at `:313` matches by route name, rebuilds both vectors and increments
a revision without the imperative observer/cleanup dispatch. Duplicate names
have ambiguous identity; removed restorable entries need explicit cleanup.
Use one RouteEntry and explicit page identity, with one mutation transaction for
imperative/declarative changes. Preserve the already-fixed guarded-pop reentrancy
contract. Test duplicate identity rejection, repeated route names, reorder,
observer parity and permanent removal cleanup.

### R08 — Retained primitive policy is still spread across many dispatch sites — P2, debt

`crates/incular-widgets/src/tree/specs.rs`, `tree/widget/structure.rs` (2,204
lines), lowering, render updates, layout, painting, input and semantics must
agree. `internal.rs:76` onward still re-exports broad modules. R01/R03 show why
compile-only coverage does not establish a complete property contract. Finish
[RETAINED_PROPERTIES.md](RETAINED_PROPERTIES.md), then regroup one primitive
family at a time. Shared invalidation bits are already fixed; remaining work is
complete property-to-phase decisions and geometry consumers, not another bitset.

### R09 — Scroll and animation interruption ownership needs one contract — P2, debt

ScrollController owns one set of viewport extents and an `activity_active` bit
(`incular-scroll/src/controller.rs:49`); activity start/end notifications are
separate (`activity.rs:3`). Nested handoff lives in `coordinator.rs`, while
retained widgets own motion and viewport attachment. Animation's state spans
status/start time/repeat (`incular-animation/src/controller.rs:28`). These are
not proof that every boolean is wrong. Specify attachment cardinality and
transition rules first, then use an activity enum where states are exclusive.
Verify jump-during-drive, drag takeover, reduced motion, detach and nested handoff
with one controllable clock and exactly-once interruption/completion outcomes.

### R10 — Tool transport has unbounded queues and discarded failures — P2, confirmed

`tools/incular-devtools/src/transport.rs:22` creates unbounded request/update
channels; send errors are discarded at `:17`. Repeated update notifications can
accumulate behind a slow UI. Target session failures are ignored at
`crates/incular-devtools/src/session.rs:126`; discovery writes also discard
errors. Bound/coalesce wakeups, backpressure commands and report disconnects and
persistence failures. Preserve token authentication, bounded target command
admission and bounded runtime frame history; those mechanisms already exist.

### R11 — Compatibility ledgers can overstate behavior — P2, confirmed

The SliverAppBar rows in `specs/P0_MATERIAL_3471.jsonl` and
`flutter_material_3471_parity.jsonl` combine `RUSTIFIED_IMPLEMENTED`, deferred
status and generic surface/stress evidence while R01 remains. Reconcile each
claim to a concrete behavioral test or an explicit deliberate omission. Do not
turn Flutter completeness into the product requirement. Incular's Rust contract
is authoritative; keep deviations documented and testable.

### R12 — Native and mobile confidence must remain platform-specific — P2, verification

Windows/macOS/Linux inject OS services into the shared desktop host. Target cfg
means a Windows workspace build does not type-check or exercise every macOS/X11/
Wayland body. Native menu/transient/shell FFI needs thread, owner, generation and
drop-order review on each host. Android and iOS currently expose semantic
projection adapters (`incular-android/src/lib.rs:16`, `incular-ios/src/lib.rs:15`),
not full mobile application hosts. Preserve that explicit support level; don't
invent a new mobile implementation as architecture cleanup.

### R13 — Identity wraparound has no uniform exhaustion policy — P3, verification

Core arena removal wraps generation (`incular-core/src/arena.rs:104`), runtime
window slots do likewise (`window_state.rs:287`), and accessibility/native IDs
wrap (`incular-accessibility/src/lib.rs:675`). This is not a reproduced practical
collision. Distinguish harmless diagnostic counter wrapping from identities
used to reject stale callbacks; retire exhausted slots or use checked allocation
and test with deliberately small counters. Avoid replacing ordinary booleans
or counters where no invariant requires a richer type.

### R14 — Large runtime owners and callback boundaries remain audit targets — P2, debt

Runtime `application.rs` is 2,668 lines and `frame.rs` 2,008, even after the host
split. Split by lifecycle/scheduling/input ownership only after mapping shared
invariants; moving methods into files with unrestricted parent access is not
an architectural improvement. Forms validator/save callbacks run while callback
RefCells are borrowed (`forms.rs:384`, `:198`): add reentrancy regressions and
snapshot callback/state before invoking user code where mutation is permitted.
This is a source risk requiring tests, not a claim that every callback panics.

## Crate-by-crate disposition

Paths below are relative to `crates/` unless prefixed `tools/`. Entry points and
hotspots were reviewed by targeted reads; the inventory records all remaining
files for implementation-time option/ownership tracing. W identifiers refer to
plan 15. A preserve disposition deliberately avoids gratuitous rewrites.

| Crate | Evidence / scope | Disposition and verification |
| --- | --- | --- |
| incular | `src/lib.rs`, Cargo features, facade tests | W9: preserve feature-controlled facade; verify minimal/controls/Material/devtools surfaces and retire only unused bridges |
| incular-core | arena, geometry, context, reactivity, widget bridge | W3/W4/W9: preserve dependency engine and invalidation; identity exhaustion tests; retire unused legacy widget transport after consumer audit |
| incular-config | constraints, insets, alignment, environment policies | W1/W3: Constraints completed; distinguish arbitrary snapshots from invariant values; constructor/builder validation inventory |
| incular-layout | algorithms, descriptors, config re-exports | W3: preserve pure algorithms/type identity; property-test tight/unbounded/directional geometry and algorithm/retained parity |
| incular-animation | controller, curves, physics/tweens | W5: preserve RAII observation; define interruption, bounds changes, repeat and reduced-motion semantics |
| incular-assets | FontId and shared FontHandle bytes | W2/W9: preserve small owner; validate identity/face-index contract with text/GPU caches |
| incular-image | decode, handles, ImageCache | W2: R05 budgets, collision-safe cache identity, large/invalid input and decode-error contracts |
| incular-text | engine cache, editing/controller/history | W1/W6: Unicode/IME/selection contracts; font/cache byte accounting; preserve paragraph shaping reuse |
| incular-scroll | controller, activity, coordinator, extent index, physics | W5: attachment/activity ownership and differential extent-index tests; preserve bounded deep jumps |
| incular-gestures | arena, recognizers, focus, keyboard | W6: cancellation, nested arbitration, focus removal, keyboard scope cleanup; distinguish simultaneous recognition from lifecycle flags |
| incular-semantics | tree updates, IDs, checked state | W3/W6: preserve mixed/absent distinction; test semantic geometry, graph removal and actions |
| incular-accessibility | AccessKit/mobile projection and action translation | W6/W8: stale native actions, stable IDs, parent/popup projection, adapter teardown; exhaustive role/state mapping |
| incular-widgets | descriptors/structure/lowering/phases, forms and advanced widgets | W1/W3/W5/W6: property ledger, R02/R03, narrow bridges, executable behavior for all exported options |
| incular-controls | button/checkbox/selection, theme/styles, visual slots | W3/W7: one behavior owner; equivalent default/custom visuals for focus, keyboard and semantics |
| incular-material | component_impl, foundation, p0_controls, app shell | W1/W7: remove silent no-ops; resolve tokens consistently; wrapper-only interaction; reconcile builder invariants |
| incular-navigation | navigator, route registry, restoration, presentation | W4: R07 route entry/identity/transactions; one delivery/cleanup contract |
| incular-rendering | paths, display lists, layers/effects/compositor | W2/W3: preserve GPU independence; balanced command scopes, layer liveness and geometry property tests |
| incular-painting | pure re-export lib | W9: preserve current compatibility decision/type identity; no second renderer model |
| incular-wgpu | shared resources, surfaces, frame acquisition, lowering, pipelines | W2: R04/R06; bounded residency, error outcomes, owned surfaces, backend format/alpha/effect tests |
| incular-platform | portable operations/capabilities, events, IDs and domains | W8/W9: preserve Winit-free contracts; executable support matrix and exhaustive typed outcome mappings |
| incular-runtime | request registry/channel, tasks, application/frame, restoration/profiling | W4/W5/W9: preserve completed lifecycle machinery; smaller owners, cancellation/reentrancy and restore-error contracts |
| incular-desktop | DesktopHost, window owner, input, transients, presentation/services | W8: preserve one runner; native integration and shared-GPU teardown tests; no new global host state |
| incular-windows | input/environment/menu/shell/native facade | W8: HWND/menu callback lifetime and Windows acceptance matrix; existing live evidence preserved |
| incular-macos | native facade, menu/shell/environment | W8: AppKit thread/object lifetime, child windows, menu generations and host-native validation |
| incular-linux | native facade, environment/shell | W8: separate X11/Wayland/portal capability outcomes; owner close and bus/service failures |
| incular-android | semantic adapter | W8: test portable mapping/target compile; preserve partial support declaration |
| incular-ios | semantic adapter | W8: test portable mapping/target compile; preserve partial support declaration |
| incular-devtools-protocol | versioned messages, handshake and token checks | W9: compatibility/size limits, malformed input and stable wire outcomes |
| incular-devtools | session, discovery, UI command bridge | W9: R10 backpressure/disconnect diagnostics; preserve authentication and opt-in behavior |
| incular-macros | empty scaffold lib | W9: explicitly scaffold/defer or retire through migration; do not implement macros just to fill the crate |
| tools/incular-devtools | transport, model, inspector and views | W9: bounded queues/models, reconnect, stale responses and nonblocking inspector refresh |

## Repository and historical plan coverage

| Area | Required follow-through |
| --- | --- |
| Examples | Migrate API changes; use public preludes; retain behavior assertions, not just construction smoke tests |
| Root/crate tests | Preserve 1,112-test baseline; add focused failing regressions for R01–R07; tests belong only under tests/ |
| Benchmarks/scripts | Preserve operation-count gates; measure byte residency and change fan-out; benchmark watchdog currently uses Linux /proc, so document/provide Windows equivalent if needed |
| Cargo/toolchain/dependencies | Verify locked MSRV separately from floating stable, meaningful feature combinations, cargo-deny/machete and target checks; no new CI/xtask framework by default |
| Specifications/docs | Reconcile ledger claims; update API migrations/accepted decisions/support in lockstep, not retroactively loosening allow-lists |
| Plans 00–03 | Campaign contracts, typed outcomes, window identity/resize/display metrics and truthful Wayland placement |
| Plans 04–06 | Pointer chords/capture/cancel, popup ownership/placement/focus/semantics across native surfaces |
| Plans 07–09 | Menu generations/shortcut once-only dispatch; typed transfer negotiation; asynchronous dialogs and parent-close cancellation |
| Plans 10–13 | Native environment/reduced motion, activation/deep-link once-only delivery, shell-resource lifetime and normalized advanced pointer metadata |
| Plan 14 | Keep committed A–E/F-first-slice fixes; use plan 15 as the expanded remaining-work schedule |

## Validation evidence and limits

Previous implementation validation at this unchanged code baseline passed both
workspace configurations (1,112 tests each), strict all-target/all-feature
Clippy, formatting and warning-denied rustdoc. This audit does not relabel those
as newly run tests. Only documentation/inventory changes are planned here.
Current-pass checks and their results are recorded in plan 15. No native Linux,
macOS, Android or iOS behavior was exercised in this audit; no MSRV or new
advisory scan is claimed. A green existing suite is not proof against the newly
identified uncovered behavior.
