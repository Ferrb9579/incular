# incular-devtools-protocol

## Architecture and support

| Contract | Status |
| --- | --- |
| Ownership | Versioned diagnostics messages shared with tooling. |
| API class | backend; re-exports and path overrides follow the [architecture contract](https://github.com/Ferrb9579/incular/blob/master/system-design/ARCHITECTURE.md). |
| Support | Opt-in tooling protocol; no runtime dependency. |

Typed, versioned wire protocol between a running Incular application (the
*target*) and Incular DevTools. Depends only on `serde` so the standalone
DevTools UI, the in-target agent, and offline tooling can share it without
pulling in the framework runtime or renderer.
