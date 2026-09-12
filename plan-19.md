# Plan 19 - Navigation transactions and valid back topology

Status: in progress. Audit Q04/Q05; specializes plan 15 W4. Depends on plan 18.

Completed (navigator stack; BackDispatcher topology untouched): one private
`RouteEntry` collection replaces the parallel routes/restorable_routes vectors,
with metadata association pinned through mixed stacks, replacement, removal,
and restoration; explicit `PageKey` identity with typed `DuplicatePageKey`
rejection before mutation, atomic keyed reconciliation (iterator drains before
any borrow; unkeyed pages mount anew), same-key rename preserving unrelated
metadata, topmost-wins for duplicate live claims, and transaction-local
indexed lookup; one shared commit/effect path (cleanups before observer events,
both after borrows end) for push, pop, replace, set_pages, and restoration;
retirement of unmatched entries, restored stacks, and replaced callback
registrations outside borrows, plus an explicit retired-children collection
for keyed child replacement (top-route snapshots alone protect only the top
child; non-top replaced children previously dropped under the borrow), with
destructor regressions proving no RefCell panic, committed-state observation,
reentrant survival, and exact-once release. Remaining drops under borrows
hold inert data only (ids, strings, JSON values, weak refs). Events are
historical nested delivery in commit order, pinned with two observers plus
reentrant cleanup across all operations. Guarded-pop candidate/revision checks
are preserved. Evidence:
`crates/incular-navigation/tests/navigation.rs` (observer sequences, reentrant
observer/cleanup navigation, keyed reorder, repeated names, duplicate rejection
without mutation or events, retained IDs, removal/reinsertion, iterator
mutation, metadata preservation, permanent cleanup, active-transition
consistency, destructor reentrancy, non-top child retirement, back-topology
cycles/duplicates/dead children/deep chains, sheet FIFO drain with panic
recovery, activation queued-before-install/replacement/closure/reentrancy/
equal-payload delivery).

Remaining: runtime ownership mapping; focus restoration and route-scoped
task ownership.

Deep-link normal-operation contract closed: queued-before-install,
replacement, disposal, reentrancy, identical URLs, unmapped URLs, and
rejected parses are pinned end-to-end (activation service, bridge,
provider, live router delegate) with no deduplication and no second
delivery engine. Restoration and builder failures report through typed
`ParseFailed` notifications: malformed persisted routes notify (with no
route information attached) and fall back to the provider, and rejected
delegate routes notify without the router committing — each pinned with
recovery. Failure reporting is exactly-once per attempt from inside
`apply_route` (error-path table on that function): parser and delegate
rejections each emit one `ParseFailed` carrying the route information,
restoration and provider-fallback attempts report independently, and
`receive_route_information` deliberately adds nothing — its `let _`
discards only the result, never a report. The earlier "swallowed
platform failures" claim was wrong and is retracted. Two state layers
stay distinct: the delegate owns its configuration atomicity
(`BasicRouterDelegate` preserves the previously accepted configuration
and emits nothing on rejection; custom delegates own their own
atomicity), while router state (current route, persisted scope) commits
only through `commit_route`. The missing-parser path is unreachable at
runtime — `RouterConfig::try_from_parts` rejects an unpaired
provider/parser at construction, pinned by test — and a superseded
transaction reports `Ok` with no commit and no notification by design
(the winning transaction's commit notification is the single record).

Done since: acyclic back attachment with typed rejection before mutation
(acyclic-graph topology with reachability check; self, two-node, and longer
cycles rejected; diamonds stay permitted; duplicates idempotent;
detach/reattach, dead children, deep chains, and inactive branches pinned).
Retired values drop outside borrows, including non-top replaced children via
an explicit collection. Sheet notifications drain FIFO through a VecDeque with
guard-owned release. Activation delivery audited production-path by
production-path (queued-before-install, replacement, reentrancy, closure,
repeated equal payloads): the service queue is the only queue, delivery is
exactly-once per live listener with no dedup, and a dispatch guard releases
the drain on listener panic without losing the queue.

Route-lifetime ownership decision (traced to code; interface work remains,
so W4 stays open — see exact work below, not a placeholder):

Guarantees that exist today:
- Focus is a per-window slot (`runtime/frame.rs`). An unmounted focused
  element clears focus to `None` with text-capture release in the same
  frame (`clear_focus_if_unmounted`) — no stale focus, but no fallback and
  no per-route save/restore. Autofocus resolves once at window mount only;
  later route changes never re-trigger it. `FocusScopeNode` keeps
  scope-local `last_focused` with save/restore, but nothing binds a scope
  to a route. `RoutePresentation::Modal.focus_trap` is declared
  (`navigation/presentation.rs`) and never read — dead until enforced or
  removed.
- Tasks have app/window/element scopes only (`runtime/tasks.rs`; zero
  route references). Element unmount cancels that element's owner scope in
  the frame drain (`frame.rs` build/layout drains), so element-owned work
  inside a removed route subtree is cancelled through rebuild —
  emergently, not by route lifetime. Work spawned in an app/window scope
  for a route, or detached from element ownership, survives route removal
  by design.
- Window close fails per-window simulation/file-dialog/native requests,
  cancels the window scope, and clears focus; app-scoped global shortcuts
  are untouched. Shutdown fails all dialogs/shortcuts/native requests,
  disposes every window, and cancels the application scope with a bounded
  Tokio stop (`application.rs`, `tasks.rs`).
- The navigator issues no per-route lifetime token. Removal runs cleanups
  (restoration-scope deletion, pop-only by design) plus observer events;
  declarative reconciliation retains persisted scope data, which must not
  be read as lifecycle cleanup.

Ownership that belongs to mounted route lifetime but is missing: (a) focus
save/restore across push/pop — which element was focused per route, and
the fallback policy when it is gone; (b) cancellation of
route-associated work that outlives its elements or never had an element
owner. Route-scoped focus/task APIs are therefore still planned, not
supported: no route-keyed API exists in runtime, desktop, or widgets.

Minimal interface for the remainder (exact implementation work; no code
yet because the only route-lifetime interface — the restoration bridge —
is pop-only by design, so a handoff today would have no consumer, and
absence of a consumer does not satisfy the requirement):
1. The navigator emits a per-entry lifetime token in `CommitEffects` on
   every permanent-removal path (pop, replace, `set_pages` removal,
   restore/fallback replacement) — declarative reorder and retention must
   not cancel — plus route activation events for focus save/restore.
2. A runtime focus owner observes activation: saves the focused element
   per route on deactivation, restores it on reactivation with an explicit
   fallback (autofocus query vs. none), and decides `focus_trap`
   enforcement or removal.
3. A task owner cancels the route scope on permanent removal only.
4. Item 5 remainder: the field-to-owner inventory above stands with one
   extraction done (`SimulationWaiters`); file-dialog FIFO, shutdown
   fan-out, and cancellation filtering stay rejected for the recorded
   reasons — further extractions only with a demonstrated defect.

Runtime ownership inventory (`Application` fields → logical owner; the
common request channel, scheduler, and teardown sequence stay put):

| Domain | Fields | Owner today |
| --- | --- | --- |
| Lifecycle | `application_shell`, `activations`, `launch_activation`, `single_instance_policy`, `close_request`, `should_exit`, `last_window_policy`, `primary_window` | Owned services plus the core loop's own policy flags |
| Scheduling | `scheduler`, `scheduler_counters` | `tasks::TaskScheduler` mechanics; counters are diagnostics |
| Input | `registry`, `manager`, `command_receiver`, `native_commands`, `request_cancellation_receiver` | `window_state` registry/manager; channels are transport |
| Restoration | `restoration`, `restoration_window_factories` | `restoration::RestorationManager`; the factories map is app wiring |
| Diagnostics | `profiler`, `hub` | `profiling` module |
| Pending replies | `pending_native_requests`; file-dialog receiver/bridge/pending/active/queued/native maps; global-shortcut receiver/bridge/pending/native queue; `simulation_receiver/bridge` plus frame/capture/GPU waiter maps; `display_catalog` | `RequestRegistry` plus per-domain maps on `Application` |

Extraction candidates and their invariants:

- `SimulationWaiters` (frame/capture/GPU maps with settlement) —
  IMPLEMENTED. Invariant: every waiter settles exactly once through one
  owner; window close and shutdown settle all three kinds together, while
  frame failure deliberately leaves GPU queries pending (no frame debt).
  Previously five methods took all of `Application` for waiter-only
  state; they are now one-line forwarders preserving the public,
  desktop-facing, and test API. Pinned by close/shutdown/complete/fail
  lifecycle tests including a new joint three-kind close test.
- File-dialog per-window FIFO (active/queued/promote with capability
  gating, cancellation phases, native handoff) — NOT extracted. The
  logic needs registry liveness, manager capabilities, and three
  registries together; splitting threads the same borrows through a new
  type without removing any. Revisit only with a demonstrated defect.
- Shutdown settlement fan-out (`fail_all_*` trio) — NOT extracted. The
  fan-out is the teardown sequence itself; an owner would re-list the
  domains without enforcing anything new.
- Native request cancellation filtering — NOT extracted: coherent
  through `RequestRegistry`, no intermingling found.

Problem/evidence: navigator.rs has parallel routes/restorable_routes, independent
push/active notifications and borrowed user iteration in set_pages. BackDispatcher
rejects only a direct self-link, not ancestor cycles.

Root cause: identity, mutation, observer delivery and topology validation have
different paths. Desired architecture: one RouteEntry per route, explicit page
identity, a commit then FIFO-deliver transaction for imperative/declarative/restore
operations, and acyclic back attachment with typed rejection before mutation.
Keep existing guarded-pop candidate/revision checks.

Ownership/files: incular-navigation owns stack and back topology; presentation
adapters project it. Update route_data/navigator/registry, facade consumers,
examples, tests and migrations; never add navigation dependency to widgets.
API: introduce stable page keys/typed invalid-identity outcomes, migrate all callers
without preserving ambiguous parallel paths. Evaluate application iterators before
borrowing/taking the live stack. Drop removed callback-bearing values outside borrows.

Lifecycle: reentrant observers cannot reorder committed transitions; dropped
observers stop immediately. Preserve existing distinction between permanent pop
cleanup and declarative reconciliation retaining persisted scope data. Invalid
iteration/identity/topology must leave the current stack/graph intact.

Performance: keyed reconciliation uses indexed lookup instead of repeated linear
search plus remove; deterministic operation-count tests cover reorder/churn.
Back dispatch must terminate for every accepted graph and pop at most one stack.

Tests/acceptance: reentrant push/pop/replace observers; iterator inspection/panic;
duplicate keys and repeated names; restoration metadata preservation; permanent
cleanup; cycle rejection, detach/drop and nested dispatch; existing tests retained.

Validation: navigation/facade/restoration tests and operation-count contracts;
architecture_contract; fmt/check; default/all-feature constrained workspace tests;
strict Clippy; warning-denied rustdoc; diff review and dedicated commit.
