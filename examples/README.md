# Examples

Runnable examples live at the workspace root so they are easy to discover.
They remain registered with the `incular` package and can be run with:

```text
cargo run -p incular --example counter
```

The API-focused examples demonstrate both construction styles:

- `core_defaults` uses `Container::new()` with fluent setters alongside
  `Container::builder()` and explicit defaults.
- `widget_basics.rs` shows a generic `Container` child containing `Text`, a
  feature-gated Controls button, and a feature-gated Material action surface.
- `counter` keeps its increment surface compact: the raw Material hit target
  has a transparent interaction layer, while the visible child owns its color
  and padding without an extra border or focus ring.

The facade exposes optional imports through `incular::controls_prelude` and
`incular::material_prelude`; both are available only when their corresponding
features are enabled. Generic children are ordinary `Widget` values, so a
builder can accept `Text`, controls, and Material components through
`Into<Widget>`.

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
- `interaction_complete`: nested horizontal/vertical gesture-arena claims,
  compatible scale, `IgnorePointer`, `AbsorbPointer`, and the automatic
  window-local retained pointer-capture lifecycle.
- `navigation_showcase`: navigator stack, fade/slide route presentation,
  dialogs, bottom sheets, and modal overlay layers.

Additional visual feature demos:

- `cargo run -p incular --example layout_gallery` — retained wrapping,
  tables, stacks, fractional sizing, aspect ratios, baselines, constraints,
  and visibility.
- `cargo run -p incular --example layout_complete` — LimitedBox,
  OverflowBox, flex parent data, positioned/indexed stacks, affine fitting,
  and constraint-driven LayoutBuilder output.
- `cargo run -p incular --example slivers` — a pinned `SliverAppBar`, a
  second persistent header, and lazy sliver grid content.
- `cargo run -p incular --example scrolling_complete` — nested wheel boundary
  transfer, variable-height lazy rows, and the composable scroll-policy API.
- `cargo run -p incular --example async_runtime` — Tokio timers,
  component-scoped cancellation, Tokio `spawn_blocking`, UI-thread signal
  completion, and direct Tokio-handle access without networking.
- `cargo run -p incular --example reactive` — no-`cx` `Memo`, owner-mounted
  `Effect`, and explicitly dispatched `Action` primitives.
- `cargo run -p incular --example environment` — typed logical viewport/DPI,
  scale, locale/direction, and runtime-resolved SafeArea padding.
- `cargo run -p incular --example localization` — ICU4X parent-locale
  fallback, catalog selection, script directionality, and localized number/date
  formatting.
- `cargo run -p incular --example multi_window` — one application with
  independently retained native windows sharing a Signal, Tokio runtime, and
  GPU device; the inspector shows its own logical metrics and DPI.
- `cargo run -p incular --example restoration` — opt-in, versioned session
  restoration for a counter, editor selection, scroll position, navigator
  stack, and a stable auxiliary inspector window. It prints the development
  snapshot path; close and relaunch to verify reconstruction, or use Reset to
  start a clean next session.
