# incular-macros

## Architecture and support

| Contract | Status |
| --- | --- |
| Ownership | Reserved procedural-macro boundary for future framework composition helpers. |
| API class | application; re-exports and path overrides follow the [architecture contract](../../docs/ARCHITECTURE.md). |
| Support | Scaffolded only; this crate intentionally exports no procedural macros yet. |

This package reserves the procedural-macro boundary without claiming an
implemented macro surface. Macros will be added only after a concrete,
stabilized composition requirement justifies them.
