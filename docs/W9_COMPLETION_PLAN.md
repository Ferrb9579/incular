# W9 completion plan: tooling and sustainable closure

Status: planned, not implemented by this document.

Planning date: 2026-09-19. Source baseline: `08a827e`; final reviewed repository
HEAD: `3d6eff6`, which commits the user's plan/documentation cleanup, including
the removal of completed plans and W8. The intervening commit changed plans and
documentation, not production Rust or Cargo manifests. Re-read the working tree
before implementation; the baseline is a reference, not permission to reset it.

This is the execution plan for the whole W9 section of [Plan 15](../plan-15.md),
not just the DevTools transport fix. It incorporates the still-applicable work
in [Plan 18](../plan-18.md), [Plan 20](../plan-20.md), and
[Plan 21](../plan-21.md), together with the specific focus-scope prerequisite
in [Plan 17](../plan-17.md).

## 1. Scope and completion contract

W8 remains removed. Do not recreate a cross-platform live-testing campaign,
device farm, CI workflow, xtask, or another test-infrastructure project under
W9. Focused regressions and the repository's ordinary Cargo checks still
accompany the implementation changes they protect.

Implement the work in this document in order, migrate its consumers, run the
applicable checks, and record actual results. Do not stop after replacing one
channel or updating the status ledgers. Do not label a source fix as native
runtime verification, or a compile-only fixture as behavioral evidence.

| Plan 15 requirement | Execution sections | Required result |
| --- | --- | --- |
| Bound DevTools requests, wakes, sessions, models, and discovery | 4-7 | Nonblocking UI boundary, session-owned completions, bounded work/data, explicit failures and reconnect behavior |
| Separate diagnostic counters from identities | 8 | No stale identity resurrection or unchecked identity truncation; exhaustion exercised without billions of operations |
| Audit facade features, bridges, aliases, painting, and macros | 9 | Intentional feature graph and API ownership; migrations completed; painting retained; macros honestly scaffolded |
| Reconcile parity assertions and evidence | 10 | Consistent supported/equivalent/omitted/deferred outcomes without weakening ownership or coverage |
| Locked MSRV, features/targets, dependencies, documentation, build policy | 11-12 | Recorded compiler/dependency/feature evidence, useful documentation linting, and measured rather than speculative build-policy changes |
| Exit statement: every crate has owner, API class, support, evidence | 9-13 | All workspace packages classified, current claims backed by named evidence, no unresolved W9 defect hidden by a status change |

Existing omissions in the pinned Flutter inventories are not a mandate to port
all omitted Flutter APIs. Conversely, an implemented W9 path cannot be marked
deferred merely to avoid fixing it. Historical findings must be checked against
current code before being reopened; W7's completed behavior and migrations stay
intact.

## 2. Source-confirmed starting points

These observations came from reading the current code, not from newly executed
failing tests. Reproduce behavior where applicable before the corresponding fix.

| Area | Current source | Planning consequence |
| --- | --- | --- |
| Focus callback ownership | `crates/incular-gestures/src/focus.rs`: `FocusScopeNode::request_focus`, `clear_focus`, `focus_next`, and `unregister` hold the manager borrow while node notifications run | Plan 17's narrow prerequisite is still relevant; complete commit-before-notify transitions first |
| Target replies | `crates/incular-devtools/src/session.rs`: a fresh `spawn_blocking` receive participates in each `select!`; `commands.rs` and `lib.rs` expose the shared reply queue | A cancelled wait must not leave an independent receiver consuming another session's reply |
| UI blocking and idle progress | `crates/incular-desktop/src/devtools_runner.rs`: unbounded `drain` and blocking `reply_sender.send`; `lib.rs` drains only from `redraw_window` | Use nonblocking completion, an event-loop work budget, and a wake path that works without redraw |
| Target shutdown | `crates/incular-devtools/src/session.rs`: polling shutdown flag, unbounded WebSocket upgrade wait, unchecked write failures | Own cancellation through accept, handshake, dispatch, writes, and teardown |
| Client ownership | `tools/incular-devtools/src/transport.rs`: unbounded request/update channels, ignored sends, detached worker, 20 ms request polling, wrapping IDs | Bounded typed admission, event-driven worker, owned shutdown, request correlation |
| Client presentation | `tools/incular-devtools/src/views.rs`: rebuild-mounted blocking update wait; model errors are also used as connection errors | Install one lifecycle-owned update subscription; separate connection, request, and data-quality state |
| Model limits | `tools/incular-devtools/src/inspector.rs` and `performance.rs`: some history counts are bounded, other maps/strings/arrival lists and graph walks are not | Keep existing useful limits; add aggregate budgets, lifecycle pruning, and terminating graph traversal |
| Producer limits | `crates/incular-widgets/src/devtools.rs`: snapshots cap at 20,000 nodes and overlays at 10,000 | Preserve bounded target collection; show truncation instead of claiming automatic full-tree retrieval |
| Existing large-model contract | `tools/incular-devtools/tests/ui.rs`: 100,001 rows before a removal, 100,000 afterward | Do not accidentally break this fixture with a 100,000-total-node cap |
| Discovery failures | Both DevTools `discovery.rs` / client `session.rs` suppress useful filesystem errors and use port-based discovery files | Return bounded discovery reports; make registration and removal ownership explicit |
| Identity reuse | `core/arena.rs`, `runtime/window_state.rs`, both accessibility projections, and multiple resource/revision allocators | Retire exhausted slots and distinguish identity/invalidation epochs from harmless telemetry counts |
| Feature graph | `crates/incular/Cargo.toml`: `devtools` strongly activates optional native dependencies; examples have no feature prerequisites | Separate portable instrumentation from native hosting and verify features independently |
| Ownership inventory | `tests/architecture_contract.rs` checks unclassified packages under `/crates/`, but the UI lives under `/tools/` | Classify the tool and root harness, not just the 30 framework crates |
| Scaffold truthfulness | `incular-macros/src/lib.rs` exports no macros; its README and architecture support string say available | Keep the scaffold, correct the claim, do not invent macros to satisfy documentation |
| Parity evidence | Multiple manifest schemas and substring parsers; `tests/support/parity_status.rs` does not require executable evidence for implemented rows | Normalize meaning and validate evidence without deleting inventory rows or fabricating blanket failures |
| Native lifetime | `incular-macos/src/environment.rs`: observer blocks capture `Rc<Inner>` and `Inner` owns their tokens; removal depends on `Inner::drop` | Break the ownership cycle and establish callback/thread teardown explicitly |
| Build policy | `.cargo/config.toml`: `incremental = true` conflicts with its comment, environment setting, and disabled profile settings; alias uses 12 jobs while default is 4 | Establish effective behavior and measurements before changing the concurrency/incremental policy |

The old blanket parity requirements have already been relaxed in parts of the
repository. W9 must fix the remaining inconsistencies and weak evidence, not
pretend every historical assertion is still present.

## 3. Implementation discipline and initial baseline

1. Read `AGENTS.md`, the architecture decisions, the current W9 section, this
   document, and the working-tree diff. Preserve all unrelated edits and deleted
   plans. Never use reset, checkout, clean, or broad staging to simplify the job.
2. Record HEAD, modified paths, toolchain versions, available RAM/disk, and
   installed target toolchains. Check for existing builds before starting more.
   The planning host has Windows Rust 1.98.0 and Rust 1.88.0 installed; that is not
   proof that the locked graph builds on either, or that installed foreign
   target libraries provide their SDKs/linkers or a runnable native host.
3. Capture focused baseline outcomes for each slice before changing its owner.
   Distinguish a reproduced defect, source-confirmed debt, and an unverified
   historical finding. Existing successful W7 evidence is not a new W9 run.
4. Keep production logic in the authoritative crate. All tests, fixtures,
   allocator test adapters, and protocol peers live under `tests/`. No test
   modules, `#[cfg(test)]` helpers, or test-only accessors in `src/`.
5. Keep the existing application Tokio runtime for the target. The standalone
   DevTools client may retain its one service runtime; do not create another
   executor per request, reply, or widget rebuild.
6. Update implementation, callers, error handling, focused tests, and migration
   notes as one slice. Make commits only from reviewed W9 changes after checks;
   never include the user's unrelated cleanup merely because it is already dirty.

The proposed new filenames and type names below are implementation targets,
not assertions that those files or types already exist. Existing modules may be
split by responsibility, but do not introduce a generic framework or new crate
just to move the same state around.

## 4. Slice A: finish the focus-scope prerequisite

Owners: `incular-gestures`; migrate only affected widget/runtime consumers.
Primary files: `crates/incular-gestures/src/focus.rs`, existing focus consumers,
and new `crates/incular-gestures/tests/focus_scope_transitions.rs`.

### Change

Separate selection commitment from notification delivery. Add an internal
transition result containing the changed nodes and committed revision. Manager
operations used by `FocusScopeNode` must update membership/selection silently,
then return the notifications to deliver after all manager, parent, and
restoration-state borrows have been released.

Commit the new exclusive selection and restoration bookkeeping before invoking
the first callback. Do not write an older selection or `last_focused` value
after callbacks have been allowed to make another focus request. Notification
delivery must respect a newer committed transition rather than replaying stale
state. Use the existing core subscription mechanism where it removes the
duplicate scope observer implementation; dropping a subscriber during delivery
must prevent its later invocation.

Apply the same rule to request, clear, unregister, traversal, nested-scope
handoff, and restoration. Preserve public standalone `FocusNode` behavior and
the accepted traversal rules. This does not reopen all W6 work or promise safe
reentrancy through an arbitrary user-held external `RefCell<FocusManager>`.

### Acceptance

Callbacks can read their scope, read the parent scope, request another node,
unregister a node, and drop another subscription without a borrow panic or an
older operation overwriting the final decision. Observers see committed
exclusive selection, not a transient half-updated scope. Cover clear, restore,
forward/reverse traversal, nested scopes, and standalone nodes. Run the existing
focus-highlight and affected W6/W7 regressions as well.

Exit: Plan 17's relevant defect is resolved by behavior, not just by moving one
`drop(manager)` statement.

## 5. Slice B: session-owned target commands and completion

Owners and migration surface:

| File | Required change |
| --- | --- |
| `crates/incular-devtools/src/commands.rs` | Replace shared reply addressing with a one-use completion capability attached to each admitted command |
| `crates/incular-devtools/src/lib.rs` | Bounded command pump, typed startup/queue outcomes, stop notification and owned agent lifecycle; remove `reply_sender` |
| `crates/incular-devtools/src/session.rs` | Session-local pending receivers, bounded writer ownership, protocol admission, cancellation and teardown |
| `crates/incular-desktop/src/devtools_runner.rs` | Bounded UI dispatch, stale/abandoned command checks, immediate nonblocking completion |
| `crates/incular-desktop/src/lib.rs` | Dedicated/coalesced DevTools wake handling independent of redraw; startup failure reporting and orderly shutdown |
| `crates/incular-devtools/tests/` and `crates/incular-desktop/tests/` | Deterministic real-channel/session tests and host-pump behavior tests |

### 5.1 Command ownership

Use the existing Tokio bounded channels and `oneshot`. The intended boundary is:

```rust
// Shape of the new internal contract, not a second public request protocol.
struct UiCommand {
    body: RequestMethod,
    completion: CommandCompletion,
}

struct CommandCompletion {
    sender: tokio::sync::oneshot::Sender<Result<ResponsePayload, ErrorCode>>,
}
```

The originating session retains the request ID and receiver together. The UI
never selects a reply recipient by a global numeric ID. A completion consumes
its sender and cannot block the UI. A dropped command/handler closes its
receiver and becomes an explicit terminal outcome for a still-live session.

Maintain pending receivers in a session-owned collection such as
`FuturesUnordered`, constructed outside the `select!` branches. No blocking
receiver task per iteration, shared reply receiver, detached receive, or global
reply routing table remains.

Cancellation contract: work whose completion/session is already abandoned when
the UI begins it is skipped. Cancellation racing after execution begins cannot
undo an already committed application effect. A late result can only reach its
original receiver, never a replacement connection reusing the request number.
Do not promise network-level exactly-once delivery across disconnects.

### 5.2 Admission and fairness

Bound queued commands, per-session in-flight commands, completed-but-unsent
results, and their aggregate payload bytes separately. A permit remains charged
until its result is written or terminally discarded, not merely until its UI
handler returns. Otherwise a slow reader can move the same unbounded growth
from the command queue into a result collection.

Return distinct internal `Full`, `Closed`, `Cancelled`, and invalid-request
outcomes. Queue rejection must not mutate application state. Reject request ID
zero after the handshake and duplicate IDs while outstanding; handle peer kind,
message shape, and supported protocol version deliberately.

Use one session-owned writer loop with bounded response admission and bounded,
loss-tolerant telemetry admission. Its task is supervised and joined/cancelled
with the session, not detached. Responses are not silently dropped to make room
for telemetry. A permanently stalled writer closes the session after its write
deadline and releases all retained work. Ensure telemetry cannot indefinitely
starve commands, completion, or shutdown, and command floods cannot monopolize
the native event loop.

### 5.3 Event-loop integration

Replace unbounded `DevToolsState::drain` with a count-budgeted drain that reports
whether work remains. Enqueueing the first outstanding command schedules one
DevTools event-loop wake. The event handler services the pump without requiring
`RedrawRequested`, including when the target is idle, occluded, or minimized.
Actual UI mutations retain their normal invalidation/redraw behavior; an
inspection query alone must not force continuous painting.

After a partial drain, schedule one continuation wake. Acknowledge/reset the
pending-wake state with a recheck so enqueueing during the drain cannot lose a
wake. Do not drain once per window and accidentally multiply the per-turn
budget by the number of windows.

### 5.4 Startup, stop, and reconnect

Make agent startup return a typed error instead of collapsing bind, entropy,
discovery, and runtime failures into `None`. Optional DevTools failure must not
abort an otherwise runnable application: the desktop owner records a useful,
redacted error and continues without the agent.

Use an owned stop notification that wakes every asynchronous stage. Bound and
make cancellable TCP accept, WebSocket upgrade, authenticated Hello, idle receive,
response writing, and close. Keep loopback binding, OS-random authentication,
and authentication before command processing. Never print the token or a raw
discovery record containing it.

On disconnect, drop all session receivers, stop the writer, release byte/count
permits, clear its subscriptions, and wake the UI for session cleanup. A new
session receives fresh identity and initial state. Debug overlays, hover,
selection, recording/subscriptions, and temporary property overrides need an
explicit owner policy: release session-owned presentation and restore still-live
temporary overrides through their normal reset path; never undo committed
signal edits or resurrect removed widgets. Cleanup validates window/node
generations and is budgeted like other UI work.

The target continues accepting a later client without retaining abandoned
command/reply workers. Agent stop and drop are idempotent. Provide an explicit
awaitable shutdown completion for owning hosts/tests; destructors signal stop
without blocking the UI or requiring an async runtime to be created in `Drop`.

### 5.5 Wire compatibility

Keep the existing v1/v2 wire vocabulary for this fix wherever it can represent
the outcome. Local Rust `Full`/`Disconnected` types do not justify adding an
unknown serialized enum variant to old peers. Map overload to an existing error
code with the existing descriptive error payload; normalize outer response
errors and `ResponsePayload::Error` at the client boundary.

Verify compatibility with serialized fixtures, not only a version-number check.
If implementation proves a new wire shape is necessary, introduce an explicit
negotiated version and legacy encoding path together; never merely bump the
constant while claiming all earlier variants remain decodable. This plan does
not require a new wire version or tree-pagination protocol.

### Acceptance

Exercise telemetry-versus-reply readiness, many simultaneous completions, full
admission, duplicate IDs, a dropped handler, cancelled queued mutation,
disconnect after execution, reconnect with the same numeric IDs, and an idle
target. Stop during accept, incomplete upgrade, Hello, idle receive, and stalled
write. Assert finite queue/permit counts, a bounded UI turn, terminal completion
or abandonment for every admitted request, and zero cross-session delivery.
Use coordination/barriers rather than timing sleeps to produce the races.

## 6. Slice C: bounded, owned DevTools client

Primary files: `tools/incular-devtools/src/transport.rs`, `session.rs`, `lib.rs`,
`views.rs`, and every request-producing module under `views/`.
Add focused integration tests under `tools/incular-devtools/tests/`.

### 6.1 Bridge and worker

Replace `ClientBridge::send -> ()` with a typed, nonblocking admission result,
returning a local request ticket on acceptance and an actionable error on full,
disconnected, oversized, or shutting-down admission. Migrate every call site;
no ignored result, hidden retry loop, or optimistic success on rejected edits.

Use a bounded request channel consumed asynchronously instead of a 20 ms poll
that drains everything. A single worker owns connect/Hello/read/write and all
pending correlation. Track its termination and retain the join handle in an
explicit service owner. Worker captures must not strongly own the same bridge
whose final drop is supposed to stop it.

Closing the client or dropping its last external owner closes admission, wakes
the worker, cancels pending I/O and requests, and permits deterministic join.
Thread/runtime startup failures are values displayed or returned by the caller,
not `expect` panics. UI callbacks never wait for worker join.

### 6.2 Request correlation and stale results

Each pending request records local ticket, wire ID, method/expected response
kind, session epoch, relevant window/node/signal identity, selection revision,
and deadline. Insert correlation before the response can be read. Use checked
request allocation; reserve ID zero for Hello, and terminate/re-establish a
session rather than wrap into an outstanding ID.

Only a matching response retires a pending entry. Reject or diagnose unknown,
duplicate, mismatched, old-session, and timed-out responses. Tree/details/signal
subscriber responses can update a selection only when their recorded context
still matches. A late target-info refresh must not reset a valid active window
to the first window. Closed windows and replaced node generations invalidate
their pending reads and cached details.

Distinguish read cancellation from mutation uncertainty. A timeout/disconnect
after sending a mutation means its effect may have occurred; show that state
and offer a fresh read. Do not automatically replay edits, reset overrides,
recording changes, or other mutations.

### 6.3 Connection and recovery policy

Represent at least `Discovering`, `Connecting`, `Authenticating`, `Connected`,
`Disconnected`, and `Stopping`, with bounded diagnostic text. A request returning
`Unsupported`, `UnknownId`, validation failure, or overload leaves an otherwise
healthy transport connected. Fatal authentication/version/framing/I/O failure
transitions the connection and completes all pending work appropriately.

Choose explicit user-triggered Retry/Reconnect for W9. It performs fresh bounded
discovery and token validation, cancels the old session, resets session-owned
state, and requests a fresh baseline. Do not add an unlimited automatic reconnect
loop. Empty target window lists are valid state, not a reason to recursively
issue `GetTargetInfo` until a window appears.

### 6.4 Coalesced model wakes

Keep the latest model state as data and a capacity-one notification as a wake,
or use the existing runtime wake facility with equivalent semantics. A full
wake slot means a wake is already pending, not a transport error. Install one
owner-mounted async subscription instead of creating blocking receive work
from widget rebuilds. Cancelling the UI owner cancels that subscription.

Publish a revision with model changes. The consumer acknowledges a revision,
rechecks after clearing its wake, and schedules at most one more notification
when newer data arrived. Prove enqueue/ack races cannot leave fresh state asleep.
The small UI-thread signal can remain; its revision must not silently alias.

Hover/highlight reads may be coalesced by key to the latest intent before send.
Never coalesce distinct mutations or change their order. Rejected UI actions
retain their previous committed state and display a bounded request error.

### Acceptance

Full/closed admission is visible; every existing view handles it. Flooded model
updates keep at most one queued wake and the final state is observed. Dropping
the last bridge stops the worker without a busy loop. A fake target reorders
responses, closes mid-write, reports an application error, delays a selection
read, and accepts a fresh connection with reused IDs. Verify correct ownership,
bounded pending state, no replay, and distinct request/connection outcomes.

## 7. Slice D: end-to-end data limits, discovery, and consistency

Owners: protocol value validation in `incular-devtools-protocol`; target
collection/queues in `incular-devtools`, `incular-widgets`, and
`incular-desktop`; model/discovery/presentation in `incular-devtools-ui`.

### 7.1 Explicit budget policy

Introduce named, documented limits and count/byte accounting. These are proposed
starting values to validate with existing representative fixtures, not measured
performance claims. Record final values and any evidence-based adjustment in
the completion report. Small capacities are injectable through genuine service
configuration in tests; do not need thousands of requests to test overflow.

| Resource | Starting limit/policy |
| --- | --- |
| Target command queue | Preserve 256 entries; add a 4 MiB aggregate owned-payload budget |
| Client queued requests | 128 entries and 2 MiB owned payload |
| In-flight commands per session | 64, including results awaiting writer admission/delivery |
| Target response payloads awaiting delivery | 32 MiB aggregate, charged until written/discarded |
| Target telemetry queue | Preserve 1,024-entry ceiling; add 16 MiB aggregate budget and explicit dropped accounting |
| Incoming request/Hello | 64 KiB message limit plus bounded recursive values |
| Incoming target message | 16 MiB message limit; bounded decoded structures and parser depth |
| Retained client data | 64 MiB accounted dynamic payload across model/history/cache categories; publish counters, not an exact process-RSS claim |
| Target tree/overlay collection | Existing 20,000 snapshot nodes and 10,000 overlay nodes; also bound traversal work |
| Client tree | At least 100,001-node fixture supported; start at 131,072 nodes with bounded edges/strings and total bytes |
| Existing histories | Preserve 300 frames, 500 console entries, and 200,000 deep-trace events; add byte limits and lifecycle pruning |
| Frame arrival samples | Bound count as well as the existing time window; start at 512 per retained window |
| Discovery | At most 128 considered records per scan, at most 64 KiB per record, bounded diagnostics |
| UI command service turn | Start at 32 commands, with at most one continuation wake |

Bounded counts alone are insufficient when one item owns a large string, child
list, nested `DebugValue`, or snapshot. Validate sizes before expensive model
updates and cap WebSocket buffers before JSON decoding. For target-owned data,
stop collection before building an oversized allocation; serializing a huge
value and rejecting it afterward is not a sufficient memory bound.

Trace collection, node-detail properties, signal lists/subscribers, windows,
discovery errors, console text, and graph edges all participate in accounting.
Keep one accepted oversized-work outcome: explicit rejection/truncation, not
silent data loss that looks complete. The existing count limits remain useful
even where aggregate bytes trigger eviction sooner.

### 7.2 Target collection and session cleanup

Budget traversal by visited retained elements as well as serialized nodes.
The existing recursive snapshot walker can traverse many hidden/internal nodes
without increasing output count; replace such walks with bounded iterative
work where necessary. Deep but valid retained trees must not overflow the call
stack. Preserve read-only inspection and feature-off cost expectations.

Prune per-window/node caches, subscriptions, phase flashes, highlights, property
overrides, and generation mappings when their owner closes or disappears. Check
cached window mappings against the current registry rather than trusting an
old `window_map` entry. Bound total subscribed windows and retained per-window
snapshots. Overflow is explicit; it must not allocate another unlimited map.

Correct comments that promise automatic scrolling/pagination beyond the snapshot
limit: the current protocol does not establish that contract. W9 preserves the
bounded snapshot and exposes incompleteness; it does not invent unsupported
pagination or remove the existing large-client-model fixture.

### 7.3 Dropped telemetry and tree revisions

A target may drop diagnostic telemetry, but a lost tree delta must not silently
become the client's new authoritative baseline. Do not advance the producer's
last-successfully-enqueued tree baseline when enqueue fails. Mark the
subscription dirty and send/request a fresh bounded snapshot when capacity
returns. On dropped-telemetry notification, the client can coalesce one
baseline refresh for its current tree; never generate one retry per dropped
event.

Track last-applied revision per window and session. Older data cannot overwrite
newer data. A revision gap alone is not necessarily a lost delta because frame
numbers need not be contiguous tree revisions; use the actual baseline/drop
contract. Preserve `truncated` state through model and UI. A partial snapshot
must tolerate explicitly omitted child nodes, while cycles, duplicate IDs, and
contradictory ownership are invalid input.

### 7.4 Client model and traversal

Bound and prune nodes, roots, expansion sets, selected details, window-indexed
histories, signals/subscribers, arrival samples, console payloads, recordings,
and cached view computations. Switching/closing a window removes data whose
retention policy no longer permits it. Disconnect invalidates selection and
pending requests rather than leaving an editable stale target.

Validate graph structure with bounded iterative traversal and visited sets.
Tree row building, parent reveal/search, and flamegraph parent traversal must
terminate on cycles and malformed indices. Keep partial/truncated data visibly
distinct from malformed data. Do not enforce parent/child symmetry in a way
that rejects the protocol's documented partial snapshots without a migration.

Preserve compact row indexing and virtualization. Build expensive search/rank/
flamegraph results once per relevant revision, outside long-held shared-model
locks, and publish immutable bounded results. Do not deep-clone the entire
100,001-node model on every repaint or move unlimited computation into a new
worker queue. Per-turn work is bounded and obsolete calculations can be skipped.

### 7.5 Discovery and sensitive state

Make registration return a result and retain a registration lease containing its
exact identity. Use the per-user directory, restrictive supported permissions,
bounded reads, and deliberate temporary-file/rename handling. Lease drop removes
only the matching record; an old agent must not delete a newer registration
that reused its port. Keep authentication tokens out of `Debug`, errors, and
captured diagnostics.

Discovery returns valid sessions plus bounded warnings distinguishing absent
directory, permission failure, malformed record, stale candidate, and connection
failure. Do not delete another owner's file merely because it was unreadable.
Do not claim process liveness on platforms where the current implementation
always returns true. The selected candidate is validated through the actual
bounded authenticated connection; no port scanning or authentication bypass.

Share pure record parsing/validation where useful without adding the target's
runtime dependency to the UI merely for discovery. Filesystem policy belongs
outside the wire-only protocol crate. Retain a small amount of justified host
glue rather than introducing a new general-purpose discovery subsystem.

### Acceptance

Test large/deep/partial/malformed trees, cyclic flamegraph parents, count and byte
overflow, window churn, repeated recordings, observer/model teardown, delayed
wake acknowledgement, lost telemetry followed by resynchronization, and
discovery permission/malformed/port-reuse cases. Assert bounded operation and
retention counts, not machine-dependent frame-time thresholds. Preserve the
100,001-row virtualization fixture with its real production path.

## 8. Slice E: identity and revision exhaustion

### 8.1 Inventory by meaning, not by spelling

Search all production crates/tools for `wrapping_add`, atomic `fetch_add`,
unchecked integer narrowing, local next-ID counters, and revisions used as
cache/subscription keys. Classify each result as diagnostic count, generational
slot, unique identity, or invalidation/session epoch. Record the inventory in
the completion report with each disposition; a grep hit is not automatically a
bug, and a field named `diagnostics` is not proof it is never used for identity.

Mandatory starting owners:

| Owner | Starting code | Policy |
| --- | --- | --- |
| Core arena | `crates/incular-core/src/arena.rs` | Retire exhausted generations, checked index conversion, mutation-safe allocation |
| Window registry | `crates/incular-runtime/src/window_state.rs` | Explicit retired slots excluded from reserve; preserve temporary take/restore reservations |
| Core keys/context | `crates/incular-core/src/key.rs`, `context.rs` | Checked unique allocation and non-aliasing dependency revisions |
| Existing reactive identity | `crates/incular-core/src/reactivity.rs` | Reuse its checked-allocation precedent; do not regress it to wrapping |
| Resource handles | `crates/incular-image/src/lib.rs`, rendering `paths.rs`, `gradients.rs` | Never wrap image/path/gradient IDs into still-observable resources |
| Retained cache generations | `crates/incular-rendering/src/compositor.rs` | Treat generations as invalidation identities, not harmless frame counters |
| Accessibility | `crates/incular-accessibility/src/lib.rs`, both projections | No native-ID recycling into stale callbacks; preserve reserved host IDs |
| DevTools | Target tree revisions, client request IDs and session/selection revisions | Never accept old work as current after exhaustion or reconnection |
| Caller-owned font identity | `crates/incular-assets/src/lib.rs` and actual font producers | Audit producer/cache contract; `FontId` itself currently has no internal allocator |

Extend this inventory to matching runtime/task/navigation/platform/controller
identities discovered during implementation. Do not leave another allocator
unsafe solely because it was not named by the old audit.

### 8.2 Generational storage

In `Arena::remove`, after removing generation `u32::MAX`, retire the slot instead
of wrapping and returning it to the free list. Earlier generations increment
normally. Empty retired slots are never returned by lookup or iteration. Check
new-slot index representability before changing length/free-list state.

`WindowRegistry::reserve` scans empty slots, so merely omitting a free-list push
is insufficient there: add explicit retirement state and exclude it. Preserve
reserved-but-temporarily-taken windows, failed creation cleanup, and stale
command rejection. Index checks precede mutation. Keep public index/generation
widths and wire encoding unless a separate justified migration is required.

### 8.3 Monotonic IDs and failure policy

Factor a small checked identity primitive at an already-appropriate ownership
boundary, reusing the existing core approach where dependencies allow it. Do
not make every crate depend on runtime or introduce a new allocator crate.
An infallible resource constructor may fail explicitly before publication on
unrepresentable exhaustion; document that exceptional contract and preserve
state when unwinding is supported. Do not replace wrapping with saturation:
saturation would repeatedly return the same identity.

At native callback boundaries, do not allow a new panic to unwind through FFI.
Reserve/check the needed ID range before publishing a projection update and
return a typed projection failure on exhaustion. Migrate its native callers;
reject further action dispatch from a terminally invalid projection and report
the failure. Never clear and restart IDs while stale native callbacks could
still refer to the old projection. Reset/activation paths must preserve the
allocator's no-reuse lifetime contract.

Revisions used to suppress work or correlate caches must fail or transition
through an explicit invalidation policy before aliasing. Document diagnostic
counts separately: wrapping or saturation is acceptable only where comparison,
drop reporting, or correctness does not depend on monotonicity. Audit dropped
telemetry arithmetic as well as the increment itself.

### 8.4 Exhaustion evidence

Exercise the same production allocation/retirement algorithm with a small
counter width or a legitimate bounded allocator configuration. Tests and their
adapters stay under `tests/`; do not create a second toy algorithm that passes
while the actual arena still wraps, or add test-only mutation APIs to `src/`.
Also exercise real public arena/window/resource consumers and their wiring.

Required cases: last representable allocation, next-allocation failure, removal
at maximum generation, no retired-slot reuse, unchanged length/maps on failure,
stale lookup/action/request after reuse attempts, reset/activation lifetime, and
concurrent unique allocation where the production allocator is atomic.

Exit: every identity/invalidation hit has a non-aliasing policy and evidence;
ordinary diagnostic increments are not mechanically rewritten as identities.

## 9. Slice F: facade, package ownership, and compatibility

Primary files: facade `Cargo.toml`/`src/lib.rs`, affected example declarations and
imports, `specs/architecture.json`, `AGENTS.md`, crate/tool READMEs,
`docs/ARCHITECTURE.md`, `docs/API_MIGRATIONS.md`, architecture/facade/API tests.

### 9.1 Feature policy

Keep current default application features unless an observed defect requires a
documented change. Make `devtools` add portable instrumentation without
implicitly enabling optional desktop backends: use weak dependency-feature
forwarding such as `incular-linux?/devtools` for optional native dependencies,
with native agent/launch integration active when `desktop` is also enabled.
Give the standalone UI explicit features it actually requires rather than
depending accidentally on the facade's defaults.

Check and document these independent facade configurations: no default features;
controls; material; devtools; desktop; desktop+devtools; default; all features.
The root test package already enables Material, so one workspace-wide build
cannot prove a minimal facade configuration. Run package-isolated graph checks
and small isolated application fixtures where compile-fail boundaries require
them. Mark examples/tests with accurate `required-features` or split their
feature-specific assertions; do not hide a real feature bug by disabling the
whole test family.

Inspect the transitive feature graph as well as successful compilation: portable
runtime/widgets and the portable facade must not acquire Winit/native runners
through a newly eager optional dependency. Controls must not require Material,
and Material convenience imports remain outside the base prelude.

### 9.2 Public surfaces and bridges

Audit public modules, re-exports, generated builders, backend handles, and
`internal`/`devtools` bridges against namespace inheritance and API overrides.
Classify before removing. A public re-export does not automatically make a
bridge application API, and `#[doc(hidden)]` is not a privacy boundary.

Migrate application/tool examples from internal names when a supported public
equivalent already exists. Remove obsolete core widget transport or duplicate
wrappers only after finding every consumer and replacing its actual capability;
do not remove distinct `BuildContext` lifetimes merely because names match.
Preserve useful type-identical aliases and distinct reactive/rendering names.
Every deliberate public break gets old/new import or call examples and migrated
repository consumers; do not leave a compatibility stub with changed semantics.

Keep `incular-painting` as the accepted pure `incular-rendering` re-export and
retain the type-identity guard. No new implementation belongs in that crate.
Keep `incular-macros` explicitly scaffolded: correct its support statements and
publication/usage documentation, without inventing derive/builder macros for
this workstream. Any optional-dependency cleanup for the empty scaffold must
have consumer evidence and a documented compatibility decision.

### 9.3 Exhaustive workspace classification

Extend the architecture inventory/test deliberately so it includes the
DevTools UI and the root test harness as well as framework crates. Store or
resolve their actual package paths rather than assuming every package lives in
`crates/<name>`. Add the missing tool support README.

Every package needs a single owner, API class, support statement, and evidence
location. Every externally exposed override needs a real owner. Validate the
entire workspace package set, optional/target dependencies, and the existing
external boundaries. Review the exact tool edges being added; never regenerate
allowed dependencies from whatever Cargo happens to report and call it an
architecture review.

### Acceptance

Independent feature fixtures compile or fail as intended; dependency graphs
remain layered; examples use the intended public APIs; painting is type
identical; macros cannot be mistaken for implemented composition support; and
an unclassified package under `tools/` fails the ownership guard just as one
under `crates/` does. Existing W7 public-surface coverage stays enabled.

## 10. Slice G: truthful parity and executable evidence

Primary files: `tests/support/parity_status.rs`, `tests/parity_policy.rs`,
`parity_manifest.rs`, `member_parity.rs`, Widgets/Material manifest tests,
affected `specs/*parity*` inventories and summary/projection files, and the
current migration/support documentation. Reuse existing W7 evidence rather than
creating a competing W9 widget ledger.

### 10.1 Normalize meaning without erasing history

Use the accepted semantic outcomes: implemented/supported, rustified equivalent,
merged equivalent, internal, omitted, and deferred. Historical source inventories
may retain their pinned spelling through an explicit typed adapter, but their
meaning must be unambiguous. Keep port policy, inventory coverage, implementation
outcome, and host-validation status separate; `RUSTIFIED` as a porting choice is
not by itself proof that behavior is implemented.

Replace hand-written JSON substring parsing with real JSON parsing and shared
validation in the test-support layer. Preserve the pinned Flutter version,
source commit, symbol identity, and full canonical inventory. Do not fetch a
new Flutter release or regenerate an architecture allow-list to make W9 pass.
Do not lower minimum coverage counts or remove inconvenient canonical rows.

Cross-check canonical rows, P0 projections, member mappings, public paths,
summaries, and W7 evidence by stable identity. Frozen P0 scope need not expand
merely because a later stream implemented an out-of-P0 component. A legitimate
scope distinction must be explicit rather than mistaken for contradictory
implementation status.

### 10.2 Evidence requirements

| Outcome | Required record and check |
| --- | --- |
| Implemented/supported | Actual owner/public path, supported feature/target scope, named behavior evidence, and compile evidence where public composition is claimed |
| Rustified/merged | Authoritative implementation, specific semantic difference, rationale, and executable evidence of the promised equivalent behavior |
| Internal | Owning subsystem, why it is not application API, and boundary/behavior evidence appropriate to that claim |
| Omitted | Explicit decision, reason, migration/alternative where callers are affected, and a boundary check preventing false exposure or a no-op stub |
| Deferred | Owner, prerequisite, reason, acceptance condition, and an executable current unsupported/boundary check where applicable; future acceptance is labelled future, not passed |

An evidence field containing the word `test`, a deleted plan, an unrelated
example, or another self-referential manifest is not enough. Resolve paths,
named test targets/functions, feature conditions, and referenced decisions.
Keep behavior tests responsible for semantics; source parsing only establishes
that the cited evidence exists, not that the claim is true. Record execution
results separately from evidence references.

### 10.3 Reconciliation

Review conflicting rows against actual implementation and recent migration
records. Preserve demonstrated support; correct overclaims; fix an actual
in-scope behavior regression instead of relabelling it away. Derive summaries
from the reviewed semantic outcomes and retain a clearly scoped denominator.
Report inventory coverage and supported behavior separately. Remove blanket
completion language only where it actually remains misleading.

### Acceptance

Add negative fixtures for unknown owner, missing behavior evidence, nonexistent
test, stale public path, policy/implementation mismatch, contradictory
projection, duplicate identity, missing decision, and unsupported claims counted
as implemented. All current manifest families use the reviewed rules. No W7
option coverage, omission rationale, or pinned symbol is lost during migration.

## 11. Slice H: native observer ownership and current documentation

### 11.1 macOS environment observer lifetime

Owners: `crates/incular-macos/src/environment.rs` and `lib.rs`; affected desktop
watcher lifecycle only if an explicit stop hook is required. This is Plan 21's
source ownership fix, not a reinstated W8 platform campaign.

Separate callback state from registration ownership. The host/service retains a
registration lease; callback blocks do not strongly retain the object that owns
their removal tokens. The lease removes each token through the exact center
that registered it, on its required thread, before state becomes unreachable.
Weak captures alone are not a complete teardown policy unless token ownership
and removal order are also explicit.

Verify the pinned Objective-C binding signatures and Apple's notification
retention/delivery guarantees while implementing. The current registrations
pass no operation queue; do not assume all notifications therefore execute on
the UI thread. Deliver through a verified main-thread mechanism or keep callback
payloads thread-safe and marshal them through the existing wake path. Never
add unsafe `Send`/`Sync` to the existing `Rc`/`RefCell` state as a shortcut.

Make repeated start/stop/drop idempotent. A queued callback after stop must see
inactive/expired state and do nothing. Preserve the correct Foundation versus
NSWorkspace notification center, lifecycle ordering, and existing locale /
accessibility preference behavior. Scope any audit of sibling observers to the
same demonstrated ownership/thread pattern rather than redesigning all native
services.

Add target-gated focused lifecycle tests under `crates/incular-macos/tests/`.
Prove state/lease release and callback suppression where the host is available;
do not count a Windows cfg-elided compile as AppKit execution. Record foreign
SDK/linker and native-host limitations precisely after actually attempting the
applicable checks.

### 11.2 Documentation and lint adoption

Reconcile crate README support tables, module headers, architecture decisions,
API migrations, and current usage docs with the implemented source. Pay
particular attention to macros, reactivity ownership, native support, DevTools
startup/limits/truncation, and GPU/native metrics that currently report a
placeholder value. Unsupported measurement must be described as unavailable,
not as a measured zero; migrate an exposed value contract deliberately where
necessary.

Audit documents that refer to deleted plans and replace live normative
references with current source/decisions/evidence. Preserve dated audit reports
as historical records; append dispositions or clarify status instead of
rewriting their original baseline into an invented past success.

Adopt useful missing-documentation linting in reviewed crate-sized waves:
small core/config/assets/protocol surfaces, the changed DevTools and backend
boundaries, resource/runtime owners, then widget/control/Material/facade
surfaces. For every wave, document exported items with semantics, ownership,
failure/normalization, feature constraints, and examples where helpful, and run
warning-denied rustdoc before enabling its gate. Resolve intentionally private
helper visibility rather than adding meaningless one-line boilerplate or broad
`allow(missing_docs)` shields. Record coverage per crate so gradual adoption
does not masquerade as a workspace-wide lint already enforced.

Exit: every package has current root support documentation and its W9-changed
public API documented; lint coverage has actually expanded and its remaining
scope is explicit. This requirement does not permit undocumented new APIs or
defer current W9 implementation merely because historical documentation is large.

## 12. Slice I: locked build, dependencies, features, and resource policy

### 12.1 Compiler and dependency closure

Keep the declared Rust 1.88 MSRV unless a reviewed unavoidable dependency/API
change proves it cannot be retained. Run the locked graph with the actual 1.88
toolchain; a `rust-version` field and successful Rust 1.98 build are not evidence.
Fix avoidable post-MSRV API usage or select a justified compatible dependency
before considering a version increase. Do not unlock/update the entire graph.

Run cargo-deny with the current advisory database and inspect actual license,
source, advisory, and duplicate-version results. The documented historical
unmaintained-dependency warning must be rechecked rather than assumed current.
Run cargo-machete with metadata, review proc-macro/generated/cfg uses, and remove
only genuinely unused dependencies. Any exception needs owner, rationale,
affected versions, and review condition; never add blanket ignores to get green.

### 12.2 Target and feature matrix

Run the independent facade matrix from section 9 and native package checks for
the platform paths actually changed. `--all-targets` covers Cargo targets such as
examples/tests; it does not mean every operating-system target. Use an explicit
`--target` when checking another target and record whether its code was built,
linked, or executed.

On this planning host, Windows-native and portable checks are locally runnable
subject to installed build tools. Linux/macOS/iOS/Android target libraries are
not a substitute for their native SDK/toolchain/host. Attempt relevant supported
cross-checks without claiming execution; use available proper hosts when present.
Do not provision a fleet or recreate W8. No new browser/web target is in scope.

### 12.3 Build configuration

Measure representative no-change and edited-crate checks with the current
effective configuration before tuning. Record elapsed/build-stage timing,
available memory, compiler/linker pressure, and target-directory size using
ordinary Cargo timing output and host tools. Avoid destructive clean builds or
multiple concurrent full graphs on a constrained machine.

Establish precedence among profile settings, `.cargo/config.toml`, and
`CARGO_INCREMENTAL`. Reconcile the contradictory incremental setting/comments
without silently changing the intended disabled policy. Review the 4-job
workspace default versus the 12-job test alias with measurements; select and
document one deliberate bounded execution policy. Do not change the counts,
enable incremental, or raise optimization simply on intuition.

Retain direct Cargo commands, existing aliases where useful, and operation-count
performance gates. Do not add CI, xtask, a separate command orchestrator, or
fragile wall-clock pass/fail benchmarks. Re-run representative measurements
after an actual configuration change and keep only justified improvements.

### 12.4 Required command families

Run commands sequentially with the reviewed job budget. Save command, exit
status, toolchain, features/target, and relevant output in the completion report.
Commands below describe implementation validation; none is claimed passed by
this planning document.

```text
cargo fmt --all -- --check
cargo check --workspace --locked
cargo check --workspace --all-features --locked
cargo test-constrained --locked
cargo test-constrained --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo +1.88.0 check --workspace --all-features --locked
cargo deny check -W unmaintained
cargo machete --with-metadata
```

Run rustdoc with `RUSTDOCFLAGS=-D warnings` using the host shell's proper
environment syntax, restoring the previous value afterward:

```text
cargo doc --workspace --all-features --no-deps --locked
```

Focused existing command anchors, extended with the new slice-specific tests:

```text
cargo test -p incular-gestures --locked
cargo test -p incular-devtools-protocol -p incular-devtools --locked
cargo test -p incular-devtools-ui --locked
cargo test -p incular-core -p incular-accessibility --locked
cargo test -p incular-workspace-tests --test architecture_contract --locked
cargo test -p incular-workspace-tests --test parity_policy --locked
cargo test -p incular-workspace-tests --test facade_surface --locked
cargo test -p incular-widgets --test tree_performance_contracts --all-features --locked
cargo check -p incular --lib --no-default-features --locked
cargo check -p incular --lib --no-default-features --features controls --locked
cargo check -p incular --lib --no-default-features --features material --locked
cargo check -p incular --lib --no-default-features --features devtools --locked
cargo check -p incular --lib --no-default-features --features desktop --locked
cargo check -p incular --lib --no-default-features --features desktop,devtools --locked
```

Also run affected runtime/window, rendering/image, native callback, manifest,
API compile-fail, test-placement, W6/W7, and feature-gated DevTools regressions.
Build and launch the ordinary DevTools UI against an ordinary example on the
available Windows host to check connect, inspect, rejected edit, disconnect,
retry, and clean close; this is a narrow W9 integration check, not a replacement
multi-environment certification effort.

## 13. Final migration and closure record

Create `docs/W9_API_MIGRATIONS.md` for actual externally observable changes and
`docs/W9_COMPLETION_REPORT.md` for executed results. Link both from the current
documentation index when they exist. Update `docs/API_MIGRATIONS.md`, Plan 15,
and the older retained plans only to the extent their specific work is completed.
Do not delete the user's remaining plans or manufacture completion history.

The report must contain:

1. Baseline/final commits and the exact W9 changes, separate from user edits.
2. A requirement-by-requirement disposition for all five Plan 15 bullets and
   Plans 17/18/20/21, including any historical item already fixed before W9.
3. Final queue, byte, history, wake, and work limits with overflow semantics and
   focused evidence; no memory/performance claims based only on proposed values.
4. Identity inventory and exhaustion outcomes; facade/package/feature decisions;
   parity status totals with explicit denominators; dependency/MSRV results;
   documentation-lint coverage and build-policy measurements.
5. Exact checks run and results, including failures, skipped features, unavailable
   tools/SDKs, and native execution not performed. Keep source implementation,
   compile verification, and native runtime evidence separate.
6. Completed migration examples and remaining intentional scope limitations.

### Final acceptance checklist

- [ ] Focus scope transitions commit before callbacks and pass reentrancy cases.
- [ ] The shared blocking reply queue and per-select blocking receivers are gone.
- [ ] Target command admission, pending results, writer work, and UI turns are bounded.
- [ ] Commands and session cleanup make progress without forcing application redraws.
- [ ] Shutdown interrupts every connection stage and no old session can receive new work.
- [ ] Client requests are correlated; all send failures have an explicit UI/API outcome.
- [ ] Final client-owner drop terminates the worker and update subscription.
- [ ] Wake coalescing cannot lose the newest state or allocate an unlimited unit queue.
- [ ] Discovery/authentication failures are useful and never reveal session tokens.
- [ ] Model/history/graph traversal has count, byte, lifetime, and work bounds.
- [ ] Truncation and telemetry loss remain visible and tree recovery is bounded.
- [ ] Identity exhaustion cannot resurrect a stale handle, native action, or request.
- [ ] Facade features and bridge migrations are independently verified.
- [ ] Every workspace package has ownership, API class, support, and evidence.
- [ ] Painting remains the accepted pure re-export; macros remain an honest scaffold.
- [ ] Parity records agree semantically and carry relevant executable evidence.
- [ ] Native observer ownership is fixed; actual host validation is reported honestly.
- [ ] New/changed APIs are documented and documentation lint coverage has expanded.
- [ ] Locked MSRV/dependency/feature checks and ordinary closure gates are recorded.
- [ ] Any build-policy change follows measurements and stays resource bounded.
- [ ] W8, CI, and xtask have not been reintroduced.
- [ ] No W9 implementation defect is hidden by relabelling it omitted or deferred.

Mark W9 implemented only after the implementation checklist and applicable
closure checks are satisfied. Where native host evidence is unavailable, state
the exact remaining evidence requirement prominently; do not call that path
fully runtime-verified. A report that fixes only the transport or only the
ledgers is not W9 completion.

## 14. Library contracts checked during planning

The design uses the existing libraries rather than a new messaging framework.
Tokio documents that one-shot send does not wait, that a successful send does
not guarantee eventual receipt, and that an already-started blocking task cannot
be aborted. Its `select!` documentation makes losing-branch cancellation
explicit. These are why reply ownership must survive selection and why the
completion contract distinguishes a committed effect from delivered data.

Primary references:

- [Tokio one-shot sender](https://docs.rs/tokio/latest/tokio/sync/oneshot/struct.Sender.html)
- [Tokio blocking task semantics](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html)
- [Tokio selection and cancellation safety](https://docs.rs/tokio/latest/tokio/macro.select.html)
- [Cargo feature reference](https://doc.rust-lang.org/cargo/reference/features.html)

The pinned binding/native notification contract still needs implementation-time
verification on the macOS slice; the ownership observation above is a source
finding, not a claimed executed AppKit lifetime test.

## 15. Planning-pass validation

At reviewed HEAD `3d6eff6`, this pass created only this document and added its
links to `plan-15.md` and `docs/README.md`. `git diff --check` passed; the new
document had no trailing whitespace or missing local Markdown link targets.
All nine execution slices, the full W9 traceability table, closure checklist,
and explicit exclusion of W8 were checked. Production Rust, Cargo manifests,
and the lockfile were not changed. No build, unit-test, or native-runtime result
is claimed from this planning-only pass.
