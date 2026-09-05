# incular

## Architecture and support

| Contract | Status |
| --- | --- |
| Ownership | Feature-controlled public facade and application preludes. |
| API class | application; re-exports and path overrides follow the [architecture contract](../../docs/ARCHITECTURE.md). |
| Support | Re-exports inherit original API classes; platform features are target-specific. |

The public facade crate for Incular applications. It will re-export the stable
public API from the framework layers and expose desktop support through the
`desktop` feature.

The default facade feature includes `material`, so application code can use
Flutter-shaped controls from `incular::prelude` or `incular::material`. Disable
that feature for a renderer-neutral/core-only integration; core interactions
are composed with `GestureDetector` and editors use `EditableText`.

Reactive derived values and asynchronous primitives are available from
`incular::reactive::{Memo, Effect, Action}`. `Memo` is also included in the
prelude; `Effect` and `Action` remain namespaced because the facade already
exports rendering and widget types with those names.
