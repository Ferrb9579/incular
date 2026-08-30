# Examples

Every runnable example has its own directory, matching the Studio layout. The
directory contains `main.rs`, a `tests/simulations.rs` scenario, and a
`tests/example_tests.rs` integration test target. Examples remain registered
with the `incular` package and keep
their stable Cargo target names:

```text
cargo run -p incular --example counter
```

The API-focused examples demonstrate both construction styles:

- `core_defaults` uses `Container::new()` with fluent setters alongside
  `Container::builder()` and explicit defaults.
- `widget_basics` shows a generic `Container` child containing `Text`, a
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

## Example QA

Each example owns a deterministic QA contract in `tests/simulations.rs` and a
contract test in `tests/example_tests.rs`. The shared harness exercises the normalized
pointer, keyboard, scroll, semantic-click, and application-frame capture paths
without using the host mouse, keyboard, or desktop capture APIs.

Run one simulation and save its two application-frame captures:

```text
$env:INCULAR_EXAMPLE_SIMULATION=1
$env:INCULAR_EXAMPLE_SIMULATION_EXIT=1
cargo run -p incular --example counter
```

Captures are written below `target/example-review/screenshots/<example>/` as
portable PPM files. Set `INCULAR_EXAMPLE_SCREENSHOT_DIR` to choose another
output directory. The exit flag is intended for CI/review batches; omit it to
leave the example open after the simulation completes.

Run the local example contract tests with:

```text
cargo test -p incular --example counter -- --test-threads=1
```

`examples/REPORT.md` records the review matrix, screenshot evidence, and any
example-specific limitations.
