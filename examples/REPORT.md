# Example QA and diagnostics report

Audit date: 2026-08-30  
Platform exercised: Windows desktop, debug profile

## Outcome

All 40 examples are isolated in named directories, compile successfully, include
automated tests, and define application-level simulations. Every simulation
completed successfully with its declared mouse, keyboard, scrolling, semantic,
and frame-capture steps. Representative native windows were also inspected from
whole-screen captures after a five-second paint interval.

| Examples | Build | Tests | Simulation |
| --- | --- | --- | --- |
| animation, async_runtime, base_ui_controls, canvas_layers, color_effects | Pass | Pass | Pass |
| controls_gallery, core_defaults, counter, custom, effects | Pass | Pass | Pass |
| environment, flutter_widgets_gallery, gesture_gallery, images, interaction_complete | Pass | Pass | Pass |
| layout_complete, layout_gallery, localization, material_gallery, material_workbench | Pass | Pass | Pass |
| multi_window, navigation_showcase, opacity, painting, performance_gallery | Pass | Pass | Pass |
| reactive, responsive_layout, restoration, scroll, scrolling_complete | Pass | Pass | Pass |
| semantics_gallery, simulation, slivers, studio, text | Pass | Pass | Pass |
| text_field, text_quality, text_selection, widget_basics, workbench | Pass | Pass | Pass |

The shared example harness lives in `examples/support/mod.rs`. Thirty-nine
examples use a dedicated `tests.rs`; `performance_gallery` retains its focused
unit tests in `main.rs` and `example_tests.rs`. Every example has a dedicated
`simulations.rs` contract.

The `performance_gallery` audit is stricter than the shared smoke sequence. It
visits all fifteen workloads, scrolls the fixed and variable lists, taps the
gesture grid twice, changes both reconciliation generations, reverses the
keyed list twice, edits a document paragraph twice, updates the shared counter,
opens the sibling window, sends keyboard input, and captures the resulting
frames. That run completed successfully after the fixes below.

## Defects found and fixed

- Material menu opening exhausted the Windows stack while shaping deeply nested
  text. Recursive framework paths now use segmented stack growth, while phase
  guards still fail early for genuine render-object re-entry or excessive depth.
- Build, layout, paint, semantics, and compositor diagnostics now preserve the
  triggering simulated input, constraints, widget path, external text call, and
  a forced backtrace in a persisted crash report.
- Windows installs a last-resort stack-overflow exception handler that emits a
  minidump when native or dependency recursion bypasses framework guards.
- Native desktop presentation now forwards renderer and GPU timings to the
  application profiler. The performance overlay displays `FPS idle` when an
  event-driven application is not requesting frames instead of implying missing
  instrumentation.
- Hover-only action surfaces now receive enter and exit callbacks even when no
  click callback is registered.
- The Flutter API parity entry for Material `TextField` now records behavior
  parity, satisfying the manifest's complete-audit contract.
- The diagnostic performance overlay no longer expands to the full lower half
  of the window or intercepts application pointer input. Its placeholder and
  live rendering are both non-interactive.
- A changed `LayoutBuilder` descriptor now invalidates layout and paint even
  when its render kind and constraints stay unchanged, so a selected workload
  cannot remain visually stale.
- Retained sliver slots now replace incompatible widget shapes instead of
  forcing an invalid element update when a workload changes.
- The gallery's sibling-window action now opens a live view backed by the same
  signal as the primary window; the strict simulation verifies the shared
  counter update.
- The gallery now starts at a high-DPI-friendly logical size and switches to
  its scrollable single-column layout when needed, keeping its controls and
  diagnostics inside the desktop work area.

## Visual review

- `target/example-review/whole-screen/material_workbench.png`
- `target/example-review/whole-screen/material_gallery.png`
- `target/example-review/whole-screen/flutter_widgets_gallery.png`
- `target/example-review/whole-screen/performance_gallery-final-autopilot-5s.png`
- `target/example-review/screenshots/performance_gallery/after-interaction.ppm`

The final performance capture visibly reports `FPS idle`, CPU, GPU, build,
layout, paint, composite, and render values. Debug-profile startup timings are
diagnostic evidence, not benchmark results; performance comparisons should use a
release build.

## Validation

The final repository state passed:

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test-constrained
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The automated simulations exercise Incular's application-level input pipeline.
They do not take over the attached OS mouse or keyboard, so a developer or agent
can continue using the computer while a test controls an Incular window in the
background.
