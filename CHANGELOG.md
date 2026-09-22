# Changelog

## Unreleased — 0.1.0 preparation

- Declarative native UI with reactive state, retained layout/painting, input,
  scrolling, text, navigation, and optional Controls/Material presentation.
- Shared Winit/WGPU desktop host and native Windows/Linux/macOS adapters.
- Android/iOS semantic adapters; complete mobile host integration remains pending.
- Opt-in local DevTools and a separate unpublished desktop inspector.
- Self-contained crate packaging, a published hello example, user guides,
  docs.rs metadata, and build-only desktop CI.
- DevTools discovery files use private exclusive temporary creation; idle HTTP
  upgrades time out and cancel on shutdown; browser-origin upgrades are rejected.
- Minimum supported Rust version is 1.89 to include the Linux notification stack.

No release is claimed by this entry. Add the actual tag/date and verification
results when publishing. The 0.1 API is experimental; patch releases preserve
compatible application APIs, and breaking changes require a new 0.x minor line.
