# Plan 16 - Reentrant focus-highlight delivery

Status: complete. Audit Q01; specializes plan 15 W6. Baseline f068993.

Problem/evidence: `incular-gestures/src/focus.rs`, HighlightModeState and
FocusHighlightManager::set_mode_locked, invoke application observers under the
modality RefMut. Reading the current mode in a callback panics.

Root cause: state mutation and external delivery share one borrowed operation.
Desired ownership: the existing thread-local modality owner commits changes;
one synchronous dispatcher delivers ordered transition snapshots after releasing
the state. Nested mutations commit immediately but enqueue delivery behind the
current event. Subscription tokens remain the authoritative callback lifetime.

Implementation: replace the locked-callback helper, keep one queue/dispatch guard,
skip dropped subscriptions, exclude new subscriptions from an already committed
event, prune dead registrations on registration, restore dispatchability after
unwinding. No second reactive engine, timer, trait hierarchy or cross-thread lock.

Affected: gestures focus implementation, focused tests under tests/, README.
API/migration: signatures and input-to-mode policy unchanged; document ordered
reentrant delivery, latest-state queries and panic behavior. Remove the superseded
locked-delivery path. Focus-scope selection is explicitly plan 17, not this plan.

Lifecycle: never hold modality state across application callbacks or destruction
of callback-owned values. A callback panic propagates; committed mode remains,
unfinished notifications are discarded and subsequent updates can dispatch.
Reentrant application transitions must converge, like reactive effects.

Performance: unchanged input/mode produces no event or observer snapshot. Work
is proportional to changed transitions and live listeners; no persistent event
history or unbounded registration growth under register/drop churn.

Tests/acceptance: callback mode/strategy reads; changes from callbacks; FIFO for
multiple observers; drop/register during dispatch; FocusBehavior application
callbacks; no-op/input policy; recovery after panic. Reproduce before fixing.

Validation: cargo test -p incular-gestures --test focus_highlight;
cargo fmt --all -- --check; cargo check --workspace; cargo test-constrained;
cargo test-constrained --all-features;
cargo clippy --workspace --all-targets --all-features -- -D warnings;
warning-denied cargo doc --workspace --all-features --no-deps; git diff --check.
Inspect status/diff, record evidence and commit only this plan plus campaign docs.

## Completion evidence

The initial public-API regression run had 8 failures and 1 pass; failures reproduced
RefCell conflicts in mode reads, nested mutation, registration and application
highlight callbacks. The expanded suite now passes all 11 tests, including queued
subscription lifetime and registration timing. A 5,000-transition reentrant chain
completes without recursive delivery; 1,000 unchanged-input rounds notify nobody.

Implemented commit-before-delivery, weak event snapshots, ordered nested delivery,
immediate cancellation, dead-registration pruning and unwind recovery. Last input
is now an explicit mode instead of a misleading boolean. Public signatures,
input policy, crate boundaries and dependencies are unchanged. Focus-scope
selection is not fixed here; it remains plan 17.

Windows validation passed: formatting, workspace check, default workspace tests
(1,429 passed / 0 failed / 0 ignored), all-feature workspace tests (same totals),
strict all-target/all-feature Clippy and warning-denied all-feature rustdoc.
Detailed gate output is in local `target/quality-plan16-*.log`. Source/test diff
review and whitespace validation are part of the commit gate. No native GUI
manual session, non-Windows native run, MSRV or dependency-policy run is claimed.
