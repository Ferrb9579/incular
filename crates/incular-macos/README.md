# incular-macos

macOS adapter for Incular's shared desktop shell. `incular-desktop` owns the
common Winit/WGPU/AccessKit runner; this crate is the ownership boundary for
future AppKit-specific lifecycle, menus, titlebar behavior, and native services.

The adapter intentionally does not duplicate or source-include another
platform crate's implementation.
