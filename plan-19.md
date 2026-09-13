# Plan 19 - Navigation transactions and valid back topology

Status: reopened for the lifecycle-ownership pass (A–D below) — not
re-closed: the outlet contracts are implemented and verified per the
five evidence areas, but W4 stays open on the two framework-level
residuals named at the end. Earlier passes established transactional
frames, the restoration frame contract, owned nested lifecycles, and
explicit unsupported/teardown behavior. Modal focus containment stays
explicitly separate future work (removing `focus_trap` implemented
nothing). Audit Q04/Q05; specializes plan 15
W4. Depends on plan 18.

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

Remaining (residual future work, not exit debt): modal focus containment
(no traversal/dispatch scope clamp exists; `focus_trap` removed rather
than kept as a no-op, barrier blocking unchanged); further runtime-owner
extractions only with a demonstrated defect. The mount-outlet residual is
closed: `RouteOutlet` mounts navigator content with stable per-route key
tags and derives ownership from the mounted tree, so ordinary navigation
needs no application oracle.

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

Route-lifetime ownership decision (implemented — the interface work below
is done; only the mount outlet that would drive it automatically remains,
recorded above):

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

Implemented against that decision:
1. The navigator issues a neutral per-entry `RouteLifetime` moved with
   keyed reuse and ended exactly once on every permanent-removal path
   (pop, replace, `set_pages` removal, restore/fallback replacement, plus
   explicit disposal) through one retirement slot in `CommitEffects` —
   declarative reorder and retention never end it, restoration-data
   retention stays independent, event `Route` snapshots carry no handle,
   and callbacks run post-borrow. No task/focus types in the navigation
   crate, no exposed effects type, no registry.
2. A runtime `RouteFocusState` (one per `Navigator`) saves the focused
   element per route on deactivation and restores on reactivation only
   when the target remains mounted (generational), focusable, enabled,
   and oracle-owned by the route — deterministic clear fallback
   otherwise, no-op without a record, records dropped on removal and
   with the state. `focus_trap` is removed (accepted no-op) with a
   migration row; barrier input blocking is unchanged.
3. A runtime `RouteTaskBinding` couples one `TaskScope` child to one
   lifetime with no new engine: removal cancels, reorder/deactivation do
   not, late completions discard through the existing cancelled-scope
   path, close/shutdown cascade unchanged, disposal detaches by policy.
4. Item 5 stands: the field-to-owner inventory with `SimulationWaiters`
   done; file-dialog FIFO, shutdown fan-out, and cancellation filtering
   stay rejected for the recorded reasons — further extractions only with
   a demonstrated defect.

W4 exit reconciliation (criteria, not just green tests): no parallel
route arrays (single `RouteEntry` collection, no zip bookkeeping);
no name-as-identity ambiguity (`PageKey` identity with pre-mutation
typed rejection, names as routing metadata only); no callbacks under
mutable domain borrows (retirement outside borrows across navigator,
router, delegate, sheet, and activation paths, pinned by destructor and
reentrancy regressions); no duplicated lifecycle engine (one router
transaction, one activation queue, one scheduler; lifetimes are neutral
handles and both bindings reuse existing owners). Residuals above are
future product work outside these criteria.

Production integration evidence (reopen → re-close):
- Focus saves validate ownership: `save_focused` records only
  oracle-owned focus, so retained records never lie. Restores validate
  eligibility through the tree's own `focusable_elements` rule —
  hidden-but-mounted targets restore by framework design
  (`Visibility`/`Offstage` retain focus), while `IndexedStack`-inactive,
  disabled, detached, and stale-generation targets fall back. Fallback
  clears only orphaned focus (dead, unfocusable, or removed-owner
  targets) and never steals mounted-owned or unattributable focus;
  removed routes restore nothing even with leftover records.
- `RouteOutlet` (runtime) hosts one navigator as a key-tagged
  `IndexedStack` with the active route indexed: post-frame drive saves
  deactivated routes, restores activated ones after layout, forgets and
  prunes removals (records, bindings, tags), and binds task scopes for
  mounted routes — no application oracle or manual drive calls, nested
  outlets isolated by key namespace, teardown dropping everything per
  documented policy. Ownership derives from mounted key tags via public
  tree queries (`element_with_key`, `parent`/`children`).
- Panic policy: terminal marks precede all delivery, so every removed
  route is terminal even if a notification aborts. Every effect callback
  (scope cleanups, lifetime endings, observers) runs isolated with the
  first panic resuming afterwards — swallowed only while already
  unwinding, so disposal during an unwind cannot abort the process
  (previously `STATUS_STACK_BUFFER_OVERRUN`). Mandatory cancellation is
  attempted exactly once per `end` regardless of sibling failures;
  `TaskScope::cancel` itself runs no application code. No general event
  framework was added.

Integration architecture review: navigation owns route identity and
lifetime (`RouteId`, `PageKey`, `RouteLifetime`, all removal paths; no
runtime/task/focus/widget-tree types inside); runtime owns task/focus
integration (`RouteTaskBinding`, `RouteFocusState`, `RouteOutlet`; the
runtime→navigation edge is allow-listed and one-way); widgets owns
mounted element eligibility (existence, `focusable_elements`, tree
queries — runtime only calls them) and the barrier/visibility primitives
the outlet composes (no second presentation engine); event `Route`
snapshots carry no lifetime handle; supported usage runs through the
outlet drive contract, so cleanup needs no undocumented maintenance
sequence — manual helpers stay as documented lower-level coverage.

Topology/snapshot/closeout pass (reopened — not re-closed):
- A — nested-outlet topology and attachment lifetime
  (`OutletAttachError`, `detach_nested`, `frame_drive_count`): self,
  two-node, and longer cycles rejected before mutation; duplicate attach
  idempotent; single-parent policy (a shared child would drive twice per
  frame — second parent rejected until explicit detach); detached outlets
  receive no frame work while retained; reattachment resumes; three-level
  trees drive each outlet exactly once per present. Borrows the
  back-dispatch reachability lesson with no cross-domain coupling.
- B — whole-tree validation (`OutletId`, pre/post-frame preflight):
  nested rejections name the responsible outlet plus routes with all
  integration state (drives, captures, bindings, focus) unchanged; valid
  siblings present together and recover independently; mid-build overlay
  introduction fails typed with the pending capture kept for retry
  (recovery commits the original save). The `widget()` overlay skip is
  unreachable through the supported path and documented as such; manual
  `after_frame` hosts own their validation.
- C — frame-attempt snapshot (`FrameAttempt`, `RouteConfig`,
  `covers()`): same-identity child replacement, push-then-pop excursions,
  and lower-route reorder beneath an unchanged top all commit and
  restore; removal abandons; retries never re-read the outgoing save
  (pinned with intervening focus movement under a transparent cover);
  foreign remounts leave the transition's own bookkeeping exact.
  Bindings are documented as lifetime-owned (including
  unmounted-but-alive routes), distinct from the mounted-content focus
  records — the transient test comment claiming mounted ownership is
  corrected.
- D — closeout through the supported host only (`present_frame`,
  `Application::close_window`): retained-but-detached generations stay
  bounded with frozen drives across mount/unmount cycles; nested close
  cancels outer and inner bindings by lifetime (attachment plays no
  role) and releases both outlets once application route content lets go
  (a live `nested_widget` closure is an application retainer, pinned by
  strong-count diagnosis during development); failed-attempt output
  paints the committed content on recovery with follow-up scheduled
  exactly on restoring frames.
- Not re-closed while: the manual `after_frame` path has no preflight
  (one pinning test, hosts own validation); subtree identity across
  positional reorder is a framework reconciliation property the outlet
  relies on but does not own; contention between two drivers for one
  outlet stays a documented host error (`frame_drive_count` exposes it)
  rather than a runtime guard.

Lifecycle-ownership pass (reopened — not re-closed), evidence by area:
- A — lifetime continuity separated from frame identity: the
  `RouteConfig` four-boolean approximation is gone; `FrameAttempt` is
  the authoritative revision plus ordered identities, refreshed at every
  build boundary without re-reading the outgoing save (`refresh_attempt`,
  composed-snapshot `built`, advisory `needs_frame`). Same-ID
  replacement paints old-then-new with the commit landing first;
  transparent-top reorder proves paint and hit order; mid-build modal
  abandons before any veil paints; retry after below-removal commits
  promptly (fails without the refresh — demonstrated). Keyed popups are
  inexpressible through the public navigation API (pushed pages are
  opaque; `set_pages` preserves entry presentation), so that variant is
  documented, not tested — a navigation-design residual below.
- B — one supported lifecycle: `after_frame`, `restore_active`, and
  `capture_tree` removed with a migration entry (no production users;
  the unvalidated path could commit against unpreflighted stacks). The
  manual test is deleted; teardown coverage is migrated to
  `present_frame` (dropped bindings ignore later removal). Rejections
  through the public API leave records and bindings unchanged per the
  existing nested/overlay rejection tests.
- C — enforced driver ownership (`Runtime::id`, driver-domain token,
  `OutletError::{DriverConflict, SeparateDrive}`, `attach` returning
  `OutletError`): separate child driving, second-runtime attach and
  present, and cross-runtime nesting are rejected before mutation with
  both identities; detach transfers ownership; teardown (runtime drop)
  releases the claim with no registry — all pinned, with frozen drive
  counts on every rejection path.
- D — supported-lifecycle verification: one combined production test
  (nested baseline, conflicting-driver rejection, mid-build navigation,
  failure plus retry, window closure with full release); `OutletError`
  timing corrected (post-frame validation can follow tree mutation —
  integration state still untouched); API consolidation reviewed
  (`attached_nested_count` stays for dynamic hosts, `frame_drive_count`
  verifies enforcement, `needs_frame` advises scheduling,
  `RouteFocusState` primitives keep their policy tests — the removals in
  B were the duplicates).
- Evidence areas: route lifetime correctness (lifetime-owned bindings,
  transient/reorder/removal/close pins); focus restoration (ownership
  saves, eligibility restores, retry/reorder/recovery returns);
  rendered-frame consistency (stale-then-new paint, order paint plus
  hits, veil timing, output/follow-up agreement); driver ownership (the
  C rejections plus transfer plus release); error recovery (failure
  keeps capture, retry commits, abandon recaptures, typed overlay
  errors pre- and post-frame).
- Not re-closed while: failed tree updates apply partially
  (`update_existing` is not transactional — recovery heals, pinned, but
  the framework property stands); declarative non-page presentations
  have no keyed identity (keyed popups/modals cannot be declared, so
  reconciliation re-ids them — navigation design, outside outlet
  scope).

Hardening pass (reopen → re-close):
- Frame transitions are transactional: pending capture (outgoing route,
  incoming route, saved focus read once) versus committed records and
  presented identity. Rebuild/frame failures keep the capture for retry
  without re-saving; navigation mid-frame or removal abandons it without
  touching records. Proved by rebuild-failure, frame-panic, mid-build
  navigation, and mid-build removal tests with exact focus/binding
  state. Found on the way: failed tree updates may leave partial state
  (update_existing is not transactional) — recovery heals through the
  next present, pinned by test.
- Restoration runs inside an explicit frame contract: build/layout mount
  first, reconcile restores after, and the restore flags follow-up work
  through the existing scheduler instead of silently stale visuals. No
  unconditional extra frame, no second loop, no new hook (set_focus
  already flags). Pinned across styling (focus ring), semantics
  (focused node), keyboard target, and slot, plus deferred and
  autofocus scheduling.
- Nested attachment has an owned lifecycle: weak registrations that
  prune on present (bounded across replacement, released on unmount),
  one-frame cascaded driving (no per-outlet frames), and a supported
  `nested_widget` composition helper so hosts never assemble
  stateful-builder slots by hand. The `revision()` handle was removed
  rather than exposed for bookkeeping. Proven by replacement-bounded,
  detach-release, and cascade-only nested tests.
- Unsupported and teardown behavior is explicit: overlay stacks fail
  typed (`OutletError::UnsupportedPresentation`) before changing
  anything; covered veils deactivate with their routes; real
  `Application::close_window` cancels bindings through existing
  ownership and releases builders, registrations, records, and bindings
  (proven by handle death, with no test-only window accessor).
  Panic policy re-reviewed unchanged.

Second production-integration pass (reopen → re-close):
- Presentation is an explicit `OutletPlacement` policy, not an
  unconditional stack: opaque pages occlude (covered content paints
  nothing), transparent popups paint through and pass input through,
  modals veil through the shared animated barrier (sized to the bounded
  host area; covered routes keep dimming correctly), unretained covered
  routes unmount while their lifetimes and task scopes persist by design,
  transitions wrap inside stable tag boundaries, caller keys are never
  overwritten (tags sit on outlet-owned wrappers), and overlay entries
  are explicitly rejected to portal hosts. Verified across mounted
  lifetime, paint, input, and semantics — not just top identity.
- One ordered operation: `RouteOutlet::present_frame` (attach once,
  then rebuild, frame, and reconcile per cycle) replaces the manual
  widget/frame/after_frame protocol; saves precede reconciliation so
  disposal-unmounts cannot erase them, restores follow layout, deferred
  enablement converges through frame scheduling without caller retries,
  and incoming autofocus resolves through the same path. `after_frame`
  stays as a documented manual escape hatch with one pinning test.
- Nearest-outlet ownership: `attach_nested` registers child outlet roots
  so inner focus resolves to the inner outlet (namespaces alone do not
  suffice — proven by removing the registration). Nested mounting uses
  stateful-builder slots keyed stably: builder-owned children survive
  ancestor rebuilds by framework contract, while replacing register
  builders remount every outer frame and orphan their registrations
  (diagnosed live, documented on `attach_nested`).
- Lifecycle verification: combined focus+task+lifetime flows, reorder
  and modal task behavior, unretained-task survival, real shutdown,
  transient pre-frame navigation (never bound, never recorded), and
  teardown release. Task-completion tests pump until settled rather than
  trusting wake counters already satisfied by setup frames. Panic policy
  re-reviewed unchanged: terminal-first marks, isolated delivery,
  resume-after-cleanup; `TaskScope::cancel` runs no application code.
  Window `close_window` itself remains untestable without an Application
  window-scope accessor — shutdown plus the real window-scope cascade
  cover the mechanism; node flags for unmounted elements go stale
  through the frame drain (explicit `set_focus` clears them), so tests
  assert slot state there.

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
