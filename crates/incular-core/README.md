# incular-core

## Architecture and support

| Contract | Status |
| --- | --- |
| Ownership | Geometry, identity, input, interpolation and lower-level context values. |
| API class | application; re-exports and path overrides follow the [architecture contract](../../docs/ARCHITECTURE.md). |
| Support | Available; shared dependency sources and scoped subscriptions. |

Platform-independent foundation types for Incular. This crate is intentionally
free of windowing and renderer dependencies.

Owns dependency-free geometry, colors, normalized input events, phase dirty
flags, and the generational `Arena`. It does not own tree semantics or layout
policy. Arena IDs are stale after removal, even if the slot is reused.

## Opt-in restoration boundary

`RestorationKey`, `RestorationScope`, and `RestorationBackend` form the small
renderer-independent boundary for persisted declarative state. The runtime owns
the scope hierarchy, duplicate-ID checks, snapshot format, diagnostics, and
asynchronous storage. Lower-level crates can read, replace, or remove raw
`serde_json::Value` values through a supplied scope without depending on the
runtime or serializing framework objects.

A key is one stable path segment. Paths remain structured key vectors, so
arbitrary key text is never ambiguously concatenated into a storage path.
