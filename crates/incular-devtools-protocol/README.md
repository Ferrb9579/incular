# incular-devtools-protocol

Typed, versioned wire protocol between a running Incular application (the
*target*) and Incular DevTools. Depends only on `serde` so the standalone
DevTools UI, the in-target agent, and offline tooling can share it without
pulling in the framework runtime or renderer.
