# W9 API migrations

W9 is primarily a correctness, tooling, and ownership closure. It deliberately
does not introduce a new application composition model or a new DevTools wire
version. The externally observable changes below are the migrations that may
matter to framework/tool integrators.

## DevTools target startup and lifetime

- `incular_devtools::spawn(...)` reports startup failure as
  `Result<AgentHandle, SpawnError>`. A desktop host may continue running the
  application without DevTools after logging the redacted error; startup
  failure is no longer represented as an unexplained absent handle.
- `AgentHandle::stop()` remains nonblocking and idempotent. The handle now also
  exposes `AgentHandle::stopped().await` for owners that need positive shutdown
  completion. Dropping the handle signals shutdown but does not create or block
  on an async runtime in `Drop`.
- `TelemetryQueue::try_push` returns whether the bounded telemetry admission
  succeeded. Callers that maintain an authoritative incremental baseline must
  only advance it after `true`; a rejected diagnostic event is counted and may
  require a later bounded snapshot refresh.

These APIs are bridge/tooling APIs rather than application widget vocabulary.

## Facade `devtools` feature

The facade's `devtools` feature no longer activates optional desktop backend
dependencies by itself. Platform forwarding now uses Cargo's weak optional
dependency feature form (`dep?/feature`). Consumers that need the desktop host
enable `desktop` as well; portable instrumentation can enable `devtools` without
pulling Winit/native desktop backends into the facade graph.

The standalone `incular-devtools-ui` tool explicitly enables the exact facade
features it owns: `desktop`, `controls`, `material`, and `devtools`.

## Identity exhaustion

Framework-generated identities and correctness-sensitive generations/revisions
no longer silently wrap or saturate into an already-observable value. Depending
on the owner, exhaustion now either retires the generational slot permanently or
fails explicitly before publishing a duplicate identity. Public ID widths and
wire encodings are unchanged.

This is observable only at integer exhaustion, but it closes a stale-handle
correctness hole: callers must not depend on identifiers eventually cycling back
to an earlier value.

## Application-shell resource generations

Tray-item and notification slots still reuse ordinary vacant indices with a new
generation. A slot whose generation itself is exhausted is now retired instead
of reused. Existing stale-resource outcomes and public ID types are unchanged.

## DevTools reconnect behavior

The standalone UI uses explicit user-triggered Retry after a fatal transport
failure. Retry performs bounded discovery again for the original target PID,
authenticates using the newly discovered record, starts a fresh request-ID
session, and requests fresh state. Unanswered mutations are **not** replayed;
their effect may already have occurred and the UI reports that uncertainty.

Request-level target errors such as unsupported operations, unknown IDs,
validation failures, and overload no longer masquerade as a disconnected
transport.

## Painting and macros

No migration is required for painting: `incular-painting` remains the accepted
pure compatibility re-export of `incular-rendering`, with type identity guarded
by the architecture tests.

`incular-macros` remains an intentional scaffold and exports no procedural
macros. W9 corrects its support documentation rather than inventing a macro API
without a concrete composition requirement.

## Build policy

The repository keeps ordinary Cargo workflows. `.cargo/config.toml` now agrees
with the workspace profiles and `CARGO_INCREMENTAL=0`: incremental compilation is
explicitly disabled and the bounded workspace/test job ceiling is four. No CI,
xtask, or separate command orchestrator is introduced by W9.
