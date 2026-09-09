# Plan 20 - Bounded DevTools client work

Status: planned. Audit Q06; specializes plan 15 W9. Depends on plan 19.

Problem/evidence: tools/incular-devtools/src/transport.rs uses unbounded request
and unit-update channels, ignores send failures, and drains commands without a
turn bound. A slow UI accumulates redundant wakeups beside an already updated model.

Root cause: notifications, commands and transport lifetime lack separate policies.
Desired ownership: model stores latest snapshots; at most one pending wake signals
model change. Commands use bounded admission with visible full/disconnected results.
The client session owns pending request correlation and its worker lifetime.

Affected: UI transport/session/inspector/views and tests; no renderer or runtime
dependency change. Migrate every ClientBridge caller; remove unbounded channels
and discarded connection failures. Preserve target authentication/protocol.

Lifecycle: dropping the final bridge stops the client; stale replies from previous
window/connection selection cannot overwrite current state. Disconnect is distinct
from an application-level request failure. No blocking admission on the UI thread.

Performance: bounded queue/model memory under synthetic bursts; coalesced wakeups,
bounded per-turn dispatch and no busy reconnect/request loop. Use deterministic
counts, not wall-clock performance gates.

Tests/acceptance: saturation, coalesced updates, final-owner drop, reconnect, stale
selection responses and explicit send failure; existing inspector behavior passes.

Validation: UI/transport tests; architecture_contract if graph changes; fmt/check;
default/all-feature constrained tests; strict Clippy; warning-denied rustdoc;
diff review and dedicated commit.
