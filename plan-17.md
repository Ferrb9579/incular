# Plan 17 - Atomic focus-scope transitions

Status: planned. Audit Q02; specializes plan 15 W6.

Problem/evidence: FocusScopeNode request/clear/traversal/restore/unregister calls
FocusManager while holding its RefMut; FocusManager synchronously notifies nodes.
A node callback querying the owning scope panics. State recorded after callbacks
can become stale. Existing ordinary traversal tests do not cover this boundary.

Root cause: selection commit and notification are interleaved across owners.
Desired ownership: FocusManager owns scope membership and selection; the scope
owns parent/restoration bookkeeping. Commit all affected selection and bookkeeping
before notifying; release manager and parent borrows first. Reuse DependencySource
for scope invalidation instead of another weak callback registry.

Affected: gestures focus implementation/tests/README; retained focus adapters only
where their contract requires migration. Keep standalone FocusNode behavior.
API: preserve usable signatures; define success as a committed operation, not a
promise that a reentrant callback cannot subsequently change focus. Remove old
interleaved mutation and scope observer machinery, not public compatibility names.

Lifecycle: parent and child transitions must be committed before callbacks;
restoration state cannot overwrite callback changes afterward. Dropped subscribers
are skipped immediately. Clear, unregister, restore and traversal use the same
commit/deliver boundary; no RefCell try_borrow fallback or dropped updates.

Performance: one bounded pass over registered live nodes; no duplicate membership
registry, extra executor or global focus scheduler. Preserve traversal policy.

Tests/acceptance: node callbacks inspect scope and registered count; callbacks
request/clear/unregister/disable focus; exclusive final selection; parent restore;
selection revisions committed before delivery; immediate unsubscribe; ordinary
traversal regressions remain green. Add failing public-API regressions first.

Validation: cargo test -p incular-gestures; focused widget/runtime focus tests;
cargo fmt --all -- --check; cargo check --workspace; cargo test-constrained;
cargo test-constrained --all-features;
cargo clippy --workspace --all-targets --all-features -- -D warnings;
warning-denied workspace rustdoc; git diff --check; inspect and commit.
