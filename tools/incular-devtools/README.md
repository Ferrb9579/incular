# incular-devtools-ui

## Architecture and support

| Contract | Status |
| --- | --- |
| Ownership | Standalone desktop DevTools application over the public facade and diagnostics protocol. |
| API class | application; this package is a tool, not a framework re-export surface. |
| Support | Available as an opt-in desktop tool; transport/model limits are enforced independently of application UI state. |

The tool opts into the exact facade features it needs (desktop, controls,
material, and devtools) instead of inheriting the facade default set.
