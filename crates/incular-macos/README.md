# incular-macos

macOS adapter for Incular's shared desktop shell. `incular-desktop` owns the
common Winit/WGPU/AccessKit runner; this crate is the ownership boundary for
AppKit-specific lifecycle, menus, titlebar behavior, and native services.

The display service derives physical usable bounds from `NSScreen.visibleFrame`
as per-monitor insets relative to Winit's physical monitor rectangle. This
preserves Winit's top-left coordinate convention while respecting the menu bar
and Dock, including side-mounted Docks.

The adapter intentionally does not duplicate or source-include another
platform crate's implementation.
