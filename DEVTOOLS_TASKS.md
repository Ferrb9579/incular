# DevTools completion checklist

Last evidence review: 2026-08-24 (current worktree and
`target/devtools-audit/01-before.png` through `05-signals-after.png`).

Checked means there is direct source, test, documentation, or screenshot
evidence in this worktree. An unchecked item is not necessarily absent; it is
not yet proven complete by the inspected evidence.

## Launch and product shell

- [x] `--devtools` / `--incular-devtools` starts the target agent and opens the
  standalone Incular-built DevTools UI for that target PID.
- [x] Connected target/platform status, Inspector, Performance, Memory, and
  Signals workspaces are visible in the post-change screenshots.
- [x] UI has a coherent dark theme, bounded two-pane layout, compact controls,
  clear grouping, and visible active states.
- [x] Searchable, virtualized stable-ID widget tree and multi-window selection
  are implemented.
- [x] Tree-row hover sends a target highlight request; row click selects the
  node, reveals ancestors, and requests details.
- [x] Native button callbacks remain bound through decorated ancestors; a
  desktop run visibly changed inspector-section and overlay active states, and
  framework/UI-shell regressions cover the failure.
- [x] Tree disclosure, row selection, Expand tree, and Collapse tree are
  independent controls; disclosure changes do not alter selection.
- [x] Curated properties are shown for the selected node, and explicitly safe
  typed properties support temporary `DEV OVERRIDE` editing and reset with
  stale-ID rejection. Static opacity is covered end-to-end.
- [x] An initial pre-frame tree request is recovered once the first retained
  frame mounts the target root; the desktop run populated the tree without a
  reconnect.
- [ ] Manually prove row click, target hover highlight, Select Widget input
  interception, reconnect, and Window B routing in one recorded desktop run.
- [ ] Investigate the gallery screenshot showing only 3 visible tree rows while
  the same target reports 247 retained elements.

## Inspector and Layout Explorer

- [x] Read-only retained layout snapshot exposes constraints, resolved/local/
  world geometry, affine transform, bounds, content/padding, clip, and baseline.
- [x] Typed retained details cover Box, Flex, Stack/Positioned/IndexedStack,
  Transform/Fitted, Scroll/Lazy viewport, and Text.
- [x] Flex allocations, stack child bounds/paint state, scroll ranges, and
  bounded Deep layout history come from retained results rather than re-layout.
- [x] Selected-node properties, structured property diffs, invalidation causes,
  Signal dependencies, retained-object links, and Why Layout/Paint summaries
  have model/UI source paths.
- [ ] Visually prove a selected node's Properties/Layout/Signals/Why/Semantics
  content; the Inspector screenshot is an unselected empty state.
- [ ] Source location, Copy file:line, and Open Source actions are not evidenced.
- [ ] Prove Flex, Positioned, Transform, Scroll, VariableList, and text explorers
  against their corresponding gallery examples in a desktop run.

## Visual debugging

- [x] Target-side selected/subtree/whole-window bounds are batched, sampled,
  and capped at 10,000 regions without inserting target widgets.
- [x] Padding/content, baseline, clip, hit-test, semantics, scroll viewport, and
  application-layer overlay controls are wired to retained geometry.
- [x] BUILD, LAYOUT, PAINT, SEMANTICS, COMPOSITE, and repaint-rainbow controls
  use actual retained phase counters; display-list replay is not treated as
  paint.
- [x] Overlay isolation/bounds and multi-window routing have subsystem tests.
- [ ] Visually exercise every overlay toggle and verify the highlight appears in
  the correct target window without changing BUILD/LAYOUT/semantics/hit tests.
- [ ] Visually prove Transform animation produces COMPOSITE only and that actual
  repaint increments repaint-rainbow.

## Why Did This Rebuild

- [x] Bounded coalesced causes, named Signal old/new generations, structured
  property diffs, task-completion context, and recorded-only cause edges exist.
- [x] Signal-causality and bounded-cause behavior have subsystem tests.
- [ ] Exercise and display every requested cause family (Signal, parent/config,
  environment/locale/window/constraints, animation/task/navigation/restoration,
  and manual invalidation); do not claim unrecorded edges.
- [ ] A full pointer/callback -> Signal -> dependent Element -> BUILD cause graph
  is not visually evidenced.

## Deep profiler and timeline

- [x] Explicit Basic, Performance, and opt-in Deep modes are wired; Basic and
  Performance avoid per-node timestamps.
- [x] Hierarchical typed traces are capped at 4,096 events/frame, 300 frames,
  and 200,000 UI-retained events with truncation/drop reporting.
- [x] Batched flamegraph geometry, ranked current/range/all aggregation,
  selectable frame summaries, recording controls, and bounded zoom/pan/range
  controls are implemented and model-tested.
- [x] Performance screenshot proves real frame rows, CPU/jank/work/draw-call
  summaries, profiler modes, recording controls, and readable compact layout.
- [ ] Capture Deep mode with a populated flamegraph, ranked results, frame
  details, and dropped/truncated trace indication where applicable.
- [ ] A synchronized multi-track timeline (Input, Signals, Tasks, phases,
  renderer/GPU, navigation, accessibility) with range interaction is not
  visually evidenced.
- [ ] Visually prove profiler-node selection synchronizes with Inspector.

## Animation debugging

- [x] 1x, 0.5x, 0.25x, 0.1x, and Pause controls change only the retained
  animation clock; deterministic scaling/pause tests exist.
- [ ] Desktop-test that Pause freezes animation while input, Tokio tasks, and
  DevTools remain responsive and profiler wall time stays real.

## Signals and memory

- [x] Signals registry, subscribers, generations/write counts, target-redacted
  summaries, and explicit primitive editing through normal `Signal::set` exist.
- [x] Memory workspace reports bounded framework resources and RSS, with named
  A/B snapshot/delta controls.
- [ ] Capture a target with registered Signals and exercise subscriber paths and
  an opted-in edit; the current screenshot is the empty state.
- [ ] Capture and verify a real Memory A/B retained-resource delta; the current
  screenshot shows only the connected snapshot.
- [ ] Full allocator/heap retention explanations are intentionally not complete.

## Performance, safety, tests, and docs

- [x] DevTools is feature/runtime gated; disabled mode has no transport or Deep
  serialization, and transport/trace/tree streams are bounded.
- [x] Structural tests cover retained layout data, overlays, causality, Deep
  hierarchy/drop limits, flamegraph/ranking models, animation scaling, and
  multi-window routing.
- [x] `DEVTOOLS.md` documents Layout Explorer, overlays/phases, Why, Deep
  profiling, flamegraph/ranking, animation slowdown, gating, and limitations.
- [x] `PERFORMANCE.md` records the instrumentation boundary and warns that Deep
  profiling changes execution characteristics.
- [ ] Record measured CPU/frame/RAM overhead for disabled, compiled-but-off,
  Basic, Performance, Deep, and overlays-enabled configurations.
- [x] Final gate rerun: formatting, workspace check/tests, all-target/all-feature
  Clippy with warnings denied, examples check, benchmark compilation, and
  `git diff --check` all pass.
- [ ] Complete the Task 16.2 manual matrix across layout, scrolling, text,
  performance, and multi-window examples with no wgpu validation errors.

## Explicit deferred product gaps

- [ ] Renderer/cache and oversized-image inspector.
- [ ] Full Accessibility, runtime/tasks/navigation/restoration, and Logs panels.
- [ ] Offline recording export/import and Network integration.
- [ ] Full Memory product beyond bounded framework counters and A/B deltas.
