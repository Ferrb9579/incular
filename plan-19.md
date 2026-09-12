# Plan 19 - Navigation transactions and valid back topology

Status: in progress. Audit Q04/Q05; specializes plan 15 W4. Depends on plan 18.

Completed (navigator stack; BackDispatcher topology untouched): one private
`RouteEntry` collection replaces the parallel routes/restorable_routes vectors,
with metadata association pinned through mixed stacks, replacement, removal,
and restoration; explicit `PageKey` identity with typed `DuplicatePageKey`
rejection before mutation, atomic keyed reconciliation (iterator drains before
any borrow; unkeyed pages mount anew), and one shared commit/effect path
(cleanups before observer events, both after borrows end) for push, pop,
replace, set_pages, and restoration. Guarded-pop candidate/revision checks are
preserved. Evidence: `crates/incular-navigation/tests/navigation.rs` (observer
sequences, reentrant observer/cleanup navigation, keyed reorder, repeated
names, duplicate rejection without mutation or events, retained IDs,
removal/reinsertion, iterator mutation, metadata preservation, permanent
cleanup, active-transition consistency).

Remaining: runtime ownership mapping; deep-link delivery exactly once; focus
restoration and route-scoped task ownership; acyclic back attachment with typed
rejection before mutation (BackDispatcher still rejects only a direct
self-link); indexed key lookup with deterministic operation-count tests for
reorder/churn (reconciliation is still linear search plus remove).

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
