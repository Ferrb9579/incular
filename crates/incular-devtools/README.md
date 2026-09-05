# incular-devtools

## Architecture and support

| Contract | Status |
| --- | --- |
| Ownership | Opt-in diagnostics transport and runtime command bridge. |
| API class | bridge; re-exports and path overrides follow the [architecture contract](../../docs/ARCHITECTURE.md). |
| Support | Implementation integration; not application widget vocabulary. |

The target-side DevTools agent for Incular applications.

This crate owns the opt-in localhost transport used by the standalone
`incular-devtools` application: target discovery, session authentication,
bounded command and telemetry queues, and protocol message routing. It keeps
network I/O off the UI thread and exposes only the typed protocol boundary to
the runtime.

Enable DevTools through the facade feature and launch the UI from the
workspace:

```text
cargo run -p incular-devtools-ui
```

The agent is intended for local development and diagnostics, not production
remote control. See the repository README and `examples/` for application
integration examples.
