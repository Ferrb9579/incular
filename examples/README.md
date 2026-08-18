# Examples

Runnable examples live at the workspace root so they are easy to discover.
They remain registered with the `incular` package and can be run with:

```text
cargo run -p incular --example counter
```

The newer visual integration galleries are:

- `canvas_layers` — direct `Canvas` commands, clips, transforms, generated
  images, paths, gradients, and a retained `CustomPaint` boundary.
- `responsive_layout` — constraints, insets, alignment, wrapping, fractional
  sizing, aspect-ratio sizing, and tables. Resize the native window to inspect
  the resulting layout.
- `semantics_gallery` — headings, labelled/decorative images, enabled and
  disabled toggles, text field/area selection, list metadata, merged/excluded
  semantics, and a blocked modal region. It prints the retained semantics tree
  and diagnostics before opening its window.

Additional visual subsystem exercises:

- `gesture_gallery`: retained tap/pan input with touchscreen multi-pointer
  scale feedback.
- `navigation_showcase`: navigator stack, fade/slide route presentation,
  dialogs, bottom sheets, and modal overlay layers.

Additional visual feature demos:

- `cargo run -p incular --example layout_gallery` — retained wrapping,
  tables, stacks, fractional sizing, aspect ratios, baselines, constraints,
  and visibility.
- `cargo run -p incular --example slivers` — a pinned `SliverAppBar`, a
  second persistent header, and lazy sliver grid content.
- `cargo run -p incular --example async_runtime` — Tokio timers,
  component-scoped cancellation, Tokio `spawn_blocking`, UI-thread signal
  completion, and direct Tokio-handle access without networking.
- `cargo run -p incular --example environment` — typed logical viewport/DPI,
  scale, locale/direction, and runtime-resolved SafeArea padding.
- `cargo run -p incular --example multi_window` — one application with
  independently retained native windows sharing a Signal, Tokio runtime, and
  GPU device; the inspector shows its own logical metrics and DPI.
- `cargo run -p incular --example restoration` — opt-in, versioned session
  restoration for a counter, editor selection, scroll position, navigator
  stack, and a stable auxiliary inspector window. It prints the development
  snapshot path; close and relaunch to verify reconstruction, or use Reset to
  start a clean next session.
