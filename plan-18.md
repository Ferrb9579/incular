# Plan 18 - Session-owned DevTools command replies

Status: planned. Audit Q03; specializes plan 15 W9. Depends on plan 17.

Problem/evidence: target session.rs selects over a spawn_blocking receiver;
desktop devtools_runner.rs sends through a blocking shared reply channel.
Cancelled waits can lose replies and backpressure can freeze the application.
Shared request IDs alone do not identify the originating connection.

Root cause: reply lifetime is attached to the server instead of one request.
Desired architecture: each admitted command owns one nonblocking completion
capability; the originating session owns its receiver. Use existing Tokio channel
primitives, cancellation-safe async waiting and bounded pending admission. UI
draining has a bounded per-turn allowance. Never spawn a blocking job per select.

Affected: incular-devtools commands/lib/session/tests, desktop command draining,
backend API migration docs. UI wire protocol may remain unchanged. Migrate every
consumer of reply_sender/UiCommand; remove the shared blocking reply queue.

Lifecycle: disconnected sessions drop receivers; queued abandoned commands do not
execute, while already executed effects cannot be undone. A new connection cannot
receive an old reply with a reused request ID. Shutdown interrupts accept/handshake/
session waits. Handler abandonment and admission failure have explicit outcomes.

Performance: bounded command and in-flight counts; no blocking UI/network send,
detached receive tasks, periodic blocking pool churn, or unbounded reply queue.

Tests/acceptance: force telemetry/request races deterministically; exactly-once
delivery; queue saturation; dropped handler; slow client; disconnect/reconnect with
reused IDs; stalled handshake and shutdown. Test transport, not only token syntax.

Validation: focused devtools/desktop tests; architecture_contract; fmt/check;
default and all-feature constrained workspace tests; strict all-feature/all-target
Clippy; warning-denied workspace rustdoc; diff review and dedicated commit.
