# incular-ios

## Architecture and support

| Contract | Status |
| --- | --- |
| Ownership | iOS semantic adapter for a native host. |
| API class | backend; re-exports and path overrides follow the [architecture contract](../../docs/ARCHITECTURE.md). |
| Support | Partial; UIKit lifecycle, input and surface require host wiring. |

iOS platform integration for Incular. `IosAccessibilityAdapter` exposes the
stable incremental mobile semantics update/action contract for a UIKit or
SwiftUI host; UIKit lifecycle, touch and gesture input, safe areas, system UI,
and native-resource wiring remain native-host responsibilities.
