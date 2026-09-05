# incular-painting

## Architecture and support

| Contract | Status |
| --- | --- |
| Ownership | Pure compatibility re-export of incular-rendering. |
| API class | backend; re-exports and path overrides follow the [architecture contract](../../docs/ARCHITECTURE.md). |
| Support | Transition shim; removal condition B08 / Stage J. |

This crate only re-exports `incular-rendering`; it owns no commands, layers,
resources or implementation. Use [incular-rendering](../incular-rendering/README.md)
for the canvas, display-list, compositor and effect contracts.

Existing `incular_painting` and `incular::painting` imports remain available during
the pre-1.0 campaign. New code uses rendering. Decision B08 requires removal in
Stage J after examples and migration guidance are updated and repository
consumers are gone except for the compatibility test. No new implementation or
domain dependency may be added here.
