# incular-android

## Architecture and support

| Contract | Status |
| --- | --- |
| Ownership | Android semantic adapter for a native host. |
| API class | backend; re-exports and path overrides follow the [architecture contract](../../docs/ARCHITECTURE.md). |
| Support | Partial; activity, input, surface and system UI require host wiring. |

Android platform integration for Incular. `AndroidAccessibilityAdapter` exposes
the stable incremental mobile semantics update/action contract for an Android
View or Compose host; JNI activity, input, surface, system UI, back handling,
and platform-resource wiring remain native-host responsibilities.
