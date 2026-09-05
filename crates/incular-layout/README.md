# incular-layout

## Architecture and support

| Contract | Status |
| --- | --- |
| Ownership | Layout algorithms over measured sizes; config values are exact re-exports. |
| API class | application; re-exports and path overrides follow the [architecture contract](../../docs/ARCHITECTURE.md). |
| Support | Available; retained measurement belongs to widgets. |

Renderer-independent algorithms operate on measured child sizes. Constraints,
insets, axes and alignment are exact re-exports from `incular-config`, which
owns their validation and policy. Widgets own retained measurement and execute
these algorithms. Layout descriptors are algorithm inputs, not another retained
widget tree.
