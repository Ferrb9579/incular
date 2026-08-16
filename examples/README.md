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
- `semantics_gallery` — visible controls annotated for accessibility; it also
  prints the retained semantics tree and diagnostics before opening its window.

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
