# Incular parity/refactor checklist

## Standard-library migration

- [x] Replace custom keyboard identity/events/modifier flags with
  `keyboard-types`; Winit normalizes to `KeyboardEvent` at the platform
  boundary and the facade exposes the selected standard vocabulary.
- [x] Replace manual restoration-directory environment probing with
  `directories::ProjectDirs`; explicit file and in-memory test stores remain
  overrides.
- [x] Use ICU4X `Locale` for `RuntimeEnvironment`, ICU4X compiled grapheme
  segmentation for standalone editor navigation, locale fallback/direction,
  ICU cardinal-plural selection, and localized integer/date formatting through the stable
  `LocalizationCatalog` boundary.
- [x] Replace bespoke CPU sRGB/linear/HSL/HSV conversion with Palette, while
  preserving the straight-alpha public `Color` and explicit GPU shader math.
- [x] Replace custom retained path and affine geometry algorithms with Kurbo;
  retain Lyon tessellation and the WGPU backend.
- [x] Replace text orchestration with Parley 0.7/Fontique/HarfRust. Parley's
  `FontContext` and reusable `LayoutContext` now own discovery, fallback,
  shaping, bidi ordering, and line breaking; Fontdue remains solely the WGPU
  R8-mask rasterizer.

## Verified complete

- [x] Replace custom retained path verb storage and control-point bounds with
  Kurbo `BezPath`, including exact winding/tight Bézier bounds and direct
  Kurbo-to-Lyon tessellation.
- [x] Replace translation-only core transform math with Kurbo-backed affine
  composition, inverse point mapping, and transformed rectangle bounds.
- [x] Add retained affine `Transform`, `FittedBox`, `ScaleTransition`, and
  `RotationTransition` widgets with compositor-only updates, inverse hit
  testing, and transformed semantic bounds.
- [x] Close retained layout primitives: `LimitedBox`, `OverflowBox`,
  `Flexible`/`Expanded`/`Spacer`, `Positioned`, `IndexedStack`, and
  constraint-keyed `LayoutBuilder`, with layout, interaction, semantics, and
  facade coverage.
- [x] Add a measured variable-extent lazy viewport. `MeasuredExtentIndex`
  provides chunked prefix lookup, measured/estimated totals, mutation
  invalidation, and stable-anchor compensation; `VirtualList` and `ListView`
  share it with the fixed-extent retained path.
- [x] Add composable scroll policy values (always/never, clamping, finite
  bouncing spring, page/fixed-extent snap) and an innermost-first nested
  coordinator that transfers only unconsumed boundary delta.

- [x] Split `incular-core` into focused arena, geometry, input, context, key,
  and widget modules with root re-exports.
- [x] Add core build-context dependency tracking, signals, local/value keys,
  unique keys, and renderer-neutral widget contracts.
- [x] Add core unit tests for context invalidation, signal subscriptions, key
  identity, widget keys, and the existing arena/geometry behavior.
- [x] Split `incular-layout` into alignment, constraints, geometry, insets,
  descriptors, and algorithms modules.
- [x] Add and verify layout algorithm tests for flex, wrap, table, padding, and
  alignment behavior (`cargo test -p incular-layout`).
- [x] Split `incular-text` into engine, editing, spans, and style modules.
- [x] Add and verify text tests for shaping/cache behavior, editing boundaries,
  rich spans, widget spans, and text scaling (`cargo test -p incular-text`).
- [x] Complete the modular animation layer (curves, tweens, controllers, and
  composable values) with targeted tests (`cargo test -p incular-animation`).
- [x] Integrate retained `Stack` behavior and expanded scrolling descriptors
  (`GridView`, `PageView`, `CustomScrollView`, and the sliver protocol); the
  widget crate targeted suite passes (`cargo test -p incular-widgets`).
- [x] Connect each desktop Winit window's retained `SemanticsTree` to a
  generation-safe AccessKit projection and `accesskit_winit` adapter, with
  incremental publication, UI-thread action dispatch, multi-window isolation,
  semantic merge/exclude/block transformations, and headless coverage.

## Master specification coverage

### Repository restructuring

- [ ] Complete the required crate split before declaring parity complete.
  - [x] `incular-core` owns shared state/context/key/signal primitives.
  - [x] `incular-widgets` owns widget descriptions and retained containers.
  - [x] `incular-layout` owns constraints, measurement, and layout algorithms.
  - [x] `incular-text` owns shaping, spans, editing, and text metrics.
  - [x] `incular-animation` owns curves, tweens, controllers, and values.
  - [x] `incular-assets` owns font and non-raster resource foundations.
  - [x] `incular-image` owns renderer-neutral image identity, decode, source
    metadata, generated RGBA8 validation, and decode caching without GPU state.
  - [x] `incular-rendering` owns renderer-neutral paint/display-list types;
    `incular-painting` remains a compatibility-only import path.
  - [x] `incular-semantics` owns semantic roles, state, actions, generational
    IDs, diagnostics, revision signal, and retained semantics tree;
    `incular-accessibility` owns the AccessKit projection, native action
    translation, and adapter diagnostics.
  - [x] `incular-platform` owns shared platform abstractions.
  - [x] Add the dedicated `incular-image` crate. Painting, widgets, and WGPU
    consume it one-way; assets no longer owns raster images.
  - [x] Add the dedicated `incular-gestures` crate for platform-neutral pointer
    events, recognizers, focus nodes, and keyboard shortcuts. Retained widget
    bindings remain in `incular-widgets`, avoiding a dependency cycle.
  - [x] Add the dedicated `incular-scroll` crate for reusable controllers,
    clamping physics, and deterministic scrollbar geometry. Scroll widgets and
    sliver descriptions remain in `incular-widgets`.
  - [x] Add the dedicated `incular-navigation` crate for stack state, route
    transitions, deep links, and overlays. It depends one-way on widgets;
    facade reexports preserve the application-facing API.
  - [x] Add the dedicated `incular-rendering` crate. It owns paths, paint,
    Canvas/display-list recording, and retained compositor layers; widgets,
    text, runtime, and WGPU consume it directly without GPU dependencies.
  - [x] Add the dedicated `incular-config` crate. It canonically owns
    constraints, insets, alignment, axis/direction, and flex/wrap policy
    values; it depends only on core geometry and is consumed directly by the
    facade, widgets, runtime, and native adapters. `incular-layout` retains
    compatibility re-exports while owning the algorithms.
  - [x] Add the dedicated `incular-semantics` crate. Widgets and runtime now
    consume it directly, while `incular-accessibility` provides compatibility
    re-exports plus native-adapter contracts without reverse dependencies.
- [ ] Reduce every `lib.rs` to minimal module declarations/re-exports.
- [ ] Enforce strict ownership boundaries: no widget logic in core, no layout
  policy in widgets, no renderer state in widgets, and no dependency cycles.
- [ ] Preserve Flutter-compatible public class names while keeping the Rust
  implementation independent of Flutter's inheritance hierarchy.
- [ ] Ensure every crate compiles independently and retains its required
  `Cargo.toml`, `README.md`, and `src/lib.rs` boundary.
- [x] Keep built-in widgets in `incular-widgets` and avoid introducing a web
  target or web crate.
- [ ] Mark every intentionally incomplete API as explicitly `DEFERRED` before
  the final parity handoff.

### Phase 1 — core foundation

- [x] `BuildContext` base API.
- [x] Renderer-neutral `Widget` base contract.
- [x] Key system.
  - [x] `LocalKey` support.
  - [x] `UniqueKey` identity generation.
- [x] Signal system integration.
- [x] Environment/context dependency tracking and invalidation.

### Phase 2 — layout system

- [x] `Row`.
- [x] `Column`.
- [x] `Flex`.
- [x] `Stack`.
- [x] `Wrap` widget with horizontal/vertical run packing, spacing, run
  spacing, descriptor conversion, and retained run-placement coverage.
- [x] `Table` widget using max-content row-major geometry, column/row spacing,
  descriptor conversion, and retained offset/size coverage.
- [x] `Align`.
- [x] `Center`.
- [x] `Padding`.
- [x] `SizedBox`.
- [x] `ConstrainedBox` enforcement in retained layout, including child-bound
  intersection and minimum/maximum clamping.
- [x] `UnconstrainedBox` retained layout, including both-axis and one-axis
  unconstraining with natural-child-vs-parent-bounds coverage.
- [x] `AspectRatio` retained sizing behavior with its targeted largest-fitting
  box test.
- [x] `Baseline` using text baselines when available and child-bottom fallback,
  with retained layout coverage.
- [x] Fractional sizing via retained `FractionallySizedBox` with finite,
  non-negative width/height factors and test coverage.
- [x] `Visibility` retained behavior (layout participation, paint/hit-test,
  and semantics suppression).
- [x] `Offstage` retained behavior via the hidden-visibility lowering.
- [ ] Verify Flutter-equivalent behavior semantics for every layout widget.
- [x] Keep internal layout algorithms separate from widget descriptions.

### Phase 3 — text system

- [x] `Text`.
- [x] `RichText`.
- [x] `TextSpan`.
- [x] `EditableText` editing model.
- [x] `TextField`.
- [x] `TextStyle`.
- [x] `TextScaler`.
- [x] Read-only `SelectableText` / cross-widget `SelectionArea` selection,
  pointer/Shift extension, and clipboard extraction from cached Parley layout.
- [x] Inline `WidgetSpan` model.
- [x] Rich text tree flattening and inherited styles.
- [ ] Verify full selection/editing parity across all native input paths.

### Phase 4 — scrolling and slivers

- [x] `ScrollView`.
- [x] `ListView`.
- [x] `GridView`.
- [x] `PageView`.
- [x] `CustomScrollView`.
- [x] Unified sliver protocol rather than a duplicated sliver hierarchy.
  - [x] Named `SliverList` lazy fixed-extent adapter.
  - [x] Named `SliverGrid` lazy fixed-extent adapter.
  - [x] `SliverPadding` protocol entry point.
  - [x] SliverAppBar-like header behavior via composable `SliverAppBar` and
    caller-provided app-bar content.
  - [x] Persistent-header pinning/support with flow-extent preservation,
    compositor-only pinning, next-header push-off, clipping, and hit-testing.
- [ ] Verify lazy materialization and scroll physics across every sliver kind.

### Phase 5 — gestures and input

- [x] `GestureDetector` recognizer foundation.
- [x] Pointer events.
- [x] `MouseRegion`.
- [x] `DragGestureDetector` with start/update/end/cancel callbacks, total
  delta, per-sample velocity, and slop handling.
- [x] Tap recognition.
- [x] Double-tap recognition.
- [x] Long-press recognition.
- [x] Pan recognition.
- [x] `ScaleGestureDetector` two-pointer scale recognition.
- [x] Focus system foundation.
- [x] Keyboard event and shortcut foundation.
- [x] `Shortcuts`.
- [x] Scoped `FocusManager` with exclusive ownership, excluded-node handling,
  cyclic Tab/Shift+Tab traversal, and weak registration cleanup.
- [x] Typed `Command` / `Actions` dispatch bound through `Shortcuts`.
- [x] Retained-tree pointer/gesture dispatch via facade-exported
  `GestureRegion`, including hit-tested down-sequence capture, nested
  hit-ancestor arena arbitration, cancellation, and runtime routing before
  button handling.
- [x] Identified multi-pointer dispatch: `PointerWithId` preserves contact
  identity, retained regions capture sequences independently, and
  `GestureRegion.on_scale_update` claims compatible scale members across
  contacts before callbacks are emitted.
- [x] Per-window/pointer gesture arenas: tap/long-press/drag/scale candidates
  enter pending, exclusive recognizers cancel losers, and scale is explicitly
  simultaneous-compatible.
- [x] `IgnorePointer` / `AbsorbPointer` retained hit-test boundaries and a
  window-local `PointerCapture` token. Capture remains safe after a contact
  leaves bounds even when an OS backend has no native capture facility.
- [x] Typed local `Draggable<T>` / `DragTarget<T>` and arena-backed
  `Dismissible`. Payloads are scoped by an explicit `DragDropContext<T>`;
  targets receive enter/leave/update/drop/cancel behavior without cross-window
  transfer.
- [ ] Reorderable lazy list remains deferred: it needs a stable logical-item
  move transaction tied to virtual-list materialization and application data
  ownership, rather than an unsafe index-only callback.
- [x] Winit touch contact adaptation through the Linux native event loop.
- [ ] Verify desktop and mobile input abstraction parity.

### Phase 5a — forms

- [x] Controller-backed `Form` / `FormField` registration, validation, reset,
  submit, autovalidation, and drop-time unregistration.
- [x] Generic `Autocomplete<T>` plus the `Autocomplete<String>::strings`
  convenience constructor with deterministic filtering and selection.

### Phase 6 — navigation and overlays

- [x] `Navigator` stack foundation.
- [x] Route identity and route stack operations.
- [x] Reusable `Page` abstraction with navigator-specific route identities.
- [x] Overlay foundation.
- [x] Modal barrier foundation.
- [x] `Dialog` modal overlay presentation descriptor with dismissibility/order
  coverage.
- [x] `BottomSheet` modal overlay presentation descriptor with
  dismissibility/order coverage.
- [x] Stack-based navigation behavior.
- [x] Declarative routing via named `Page` reconciliation with route-ID
  preservation and removal handling.
- [x] Deep-link integration via facade-exported `RouteRegistry` location
  normalization, page resolution, and navigator dispatch.
- [x] Route transitions (`none`, fade, slide, and fade+slide) via retained
  controller layers and `Route::presented_child`.

### Phase 7 — animation

- [x] `AnimationController`.
- [x] Typed `Tween` values.
- [x] Curves.
- [x] Implicit animations.
- [x] Merged transition-widget set (`FadeTransition`, `SlideTransition`, and
  composable `Transition`).
- [x] Composable animation/value graph.
- [x] Avoid duplicate Flutter animation subclasses by reusing retained
  controllers/layers behind the merged transition API.

### Phase 8 — image and painting

- [x] `Image` widget foundation.
- [x] Asset, memory, and file image source foundations.
- [x] Fit and alignment calculations.
- [x] `ImageRepeat` modes (`NoRepeat`, `RepeatX`, `RepeatY`, and `Repeat`)
  with bounded retained paint tiling and repeat-geometry coverage.
- [x] `CustomPaint` widget backed by a renderer-neutral `DisplayList`, public
  `Canvas` convenience, and retained picture replay coverage.
- [x] `RepaintBoundary` retained wrapper/picture with paint-only child
  invalidation coverage for same-size `CustomPaint` changes.
- [x] Renderer-neutral `Canvas` abstraction.
- [x] Complete paint-system integration through the dedicated
  `incular-rendering` crate, with the legacy painting crate retained only for
  source compatibility.

### Phase 9 — configuration

- [x] Dedicated reusable configuration crate.
  - [x] Alignment values.
  - [x] `EdgeInsets`.
  - [x] `BoxConstraints`.
  - [x] Colors.
  - [x] Shapes.
  - [x] Borders.
  - [x] Gradients.
  - [x] Text configuration types.
  - [x] Scroll physics foundation.
  - [x] Gesture configuration foundation.
  - [x] Animation configuration foundation.
- [x] Verify canonical alignment, inset, direction, and constraint values
  remain widget-independent and reusable by layout, widgets, runtime, native
  adapters, and the facade (`cargo test -p incular-config -p incular-layout`).

### Test and validation requirements

- [x] Core unit tests for context, keys, signals, widget contracts, and arena
  behavior.
- [x] Layout algorithm tests.
- [ ] Dedicated layout-widget parity tests for every listed widget.
- [ ] Rendering correctness tests for every rendering-capable crate.
- [x] Interaction tests for the currently implemented gesture/navigation/
  retained-widget slices.
- [x] Expose the currently verified widget parity APIs, including
  `AspectRatio`, `Visibility`, `Offstage`, `ConstrainedBox`, `UnconstrainedBox`,
  `Wrap`, `FractionallySizedBox`, `Table`, `Baseline`, `GridView`, `PageView`,
  `Page`, `Dialog`, and `BottomSheet`, and
  `CustomScrollView`, `SliverList`, and `SliverGrid`, plus `ImageRepeat` and
  `CustomPaint` and `RepaintBoundary`,
  `RouteRegistry`, `RouteTransition`, `GestureRegion`, `DragGestureDetector`,
  `ScaleGestureDetector`, `FadeTransition`, `SlideTransition`, and
  `Transition`, through the `incular` facade prelude.
- [ ] Dedicated parity-validation tests for every listed Flutter API.
- [x] Full workspace build.
- [x] Full workspace test suite.
- [x] Workspace clippy with `-D warnings`.

## In progress

- [ ] Finish retained-widget/container parity and cross-crate integration,
  including the remaining widget behavior and facade surface.

## Task 12.1 — Tokio production application runtime

- [x] Inspect and preserve the single UI-thread retained runtime, frame
  scheduler, signals, platform conversion boundary, and Linux/Winit runner.
- [x] Replace the custom executor, timer heap/future, ready queue, and fixed
  worker pool with one Tokio multi-thread application runtime. Tokio owns
  general futures, timers, I/O facilities, and `spawn_blocking`; Incular owns
  only UI-frame/lifecycle/environment coordination.
- [x] Keep lightweight `Task<T>`, `TaskHandle`, and `TaskScope` strictly as
  Tokio ownership/result-delivery helpers. Owner completions validate their
  generational `ElementId`; started blocking jobs are explicitly result-discard
  only after owner cancellation.
- [x] Add a typed Tokio-to-UI message bridge with Winit user-event wake
  coalescing and a 128-message per-turn budget. Native idle waits do not poll
  Tokio or redraw merely because async work exists.
- [x] Add typed normalized runtime environment, field-read tracking at the
  declarative root, logical SafeArea resolution, and Linux resize/DPI updates.
- [x] Add lifecycle state/observer foundation and runtime error hook.
- [x] Add headless bridge/runtime/config/widget tests and `async_runtime` plus
  `environment` root examples. Tokio timer coverage uses a bounded millisecond
  wait; framework animation time remains deterministic through `run_frame_at`.
- [x] Run the final full workspace gate. Native smoke launch was attempted for
  `async_runtime`; this headless Linux session has no Wayland compositor, so
  visual launch remains an environment limitation rather than a build failure.

## Task 12.2 — Multi-window desktop runtime

- [x] Add Incular-owned generational `WindowId`, validated portable
  `WindowOptions`, data-only `WindowHandle` operations, and normalized
  window-level events/lifecycle without exposing Winit IDs or pointers.
- [x] Promote `Application` from one `Runtime` to a registry of independent
  retained roots sharing exactly one Tokio scheduler, services, signals, and
  application scope. Preserve `incular::run(app)` as the single-window path.
- [x] Add application → window → component cancellation hierarchy, stale
  generation checks, close-request policy, explicit last-window policy, and
  UI-message-only mutation for worker-held handles.
- [x] Track environment, focus, metrics, dirty/frame work, semantics, and
  diagnostics per window. Shared signals enqueue only subscribed roots; DPI or
  size change in one window leaves other roots and physical caches untouched.
- [x] Add the shared GPU/device context plus per-window surfaces/presentation
  state. Pipelines, images, glyph resources, and gradients are shared while
  surface/compositor/offscreen state remains window-local.
- [x] Refactor the Linux Winit 0.30 adapter to queue/create/mutate windows only
  in active event-loop callbacks and route all native input/focus/DPI/close
  events by Incular window ID.
- [x] Add headless multi-window tests (independent retained roots, routing,
  focus/environment/DPI, signal targeting, close/task/stale-handle behavior,
  32 simultaneous roots, 1,000 sequential open/close cycles) and the
  `multi_window` visual example.

## Task 12.3 — Production state persistence/restoration

- [x] Add versioned, serde/JSON-backed opt-in restoration with stable escaped
  `RestorationKey` paths, typed `Restorable<T>` signal integration, explicit
  removal/reset, scope diagnostics, migrations, corruption/future-format
  fallback, configurable byte limits, and deterministic in-memory storage.
- [x] Keep ordinary signals and live framework/native/GPU/task objects out of
  snapshots; persist only declarative JSON values and stable window/route
  descriptors.
- [x] Coalesce Tokio background writes with a 250ms default debounce, bounded
  shutdown/suspension flushes, and temporary-sibling file replacement that
  preserves the previous valid snapshot on failure.
- [x] Add opt-in text selection, form value, scroll/page offset, and stable
  navigator stack restoration. IME composition, stale validation UI, overlays,
  gesture/focus/animation state, and materialized lazy-list items stay
  transient.
- [x] Add stable restorable primary/auxiliary window IDs and registered
  factories; validate logical geometry, omit stale positions/DPI, safely skip
  unknown factories, preserve descriptors on shutdown, and remove an auxiliary
  descriptor on explicit user close.
- [x] Add restoration unit/runtime/facade tests and the `restoration` visual
  example. Headless launch can initialize and print its state path but cannot
  create a Wayland surface without a compositor.

## Task 11F parity audit

- [x] Add the checked-in machine-readable Flutter-derived capability inventory
  at `specs/widget_parity.jsonl` and the readable mapping at
  `WIDGET_PARITY.md`.
- [x] Add a root integration test that loads every manifest record, rejects
  unknown/unresolved states, missing required data, duplicate entries, TODO
  text, and deferred/skipped records without a concrete rationale.
- [x] Audit facade and documented subsystem imports without exposing retained
  element/render/layer, arena, scheduler, WGPU, or Winit implementation types.
- [ ] Close the precisely recorded behavior gaps before declaring full widget
  parity: bounded/overflow/intrinsic/fitted layout; flex-child factors and
  positioned/indexed stacks; retained transform variants; text overflow and
  selection areas; variable-extent and nested scrolling; gesture arbitration,
  drag/drop/dismiss/reorder, pointer policy; navigation observers/nesting;
  semantics merge/block behavior. Each prerequisite is recorded in the
  manifest rather than being claimed as implemented.

## Task 13 desktop/core closure

- [x] Retained `Text`/`RichText` paragraph configuration: soft wrapping,
  max-lines, clip/visible/ellipsis overflow, shaped grapheme-safe bidi ellipsis,
  facade exports, and focused engine/tree tests.

## Intentionally incomplete parity (deferred)

- [ ] Layout parity verification beyond the targeted retained-widget tests.
- [x] Scrolling: named `SliverList`/`SliverGrid` entries, SliverAppBar-like
  behavior, and persistent-header pinning.
- [ ] Input: platform-specific desktop/mobile lifecycle parity. Identified
  multi-pointer scale dispatch and Linux/Winit touch adaptation are covered;
  Android/iOS native adapters are not implemented here.
- [x] Dedicated semantics crate boundary. Rendering, image, gesture, scroll,
  navigation, configuration, and semantics are extracted and independently
  tested without reverse widget dependencies.

## Remaining validation

- [x] Run `cargo fmt --all -- --check` after all modular files parse.
- [x] Run `cargo check --workspace` and `cargo test --workspace`.
- [x] Run `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [x] Final full workspace checkpoint after the extracted gesture/scroll
  boundaries (60 `incular-widgets` tests and 21 focused runtime tests):
  formatting, workspace check/tests, and strict clippy all pass.
- [x] Resolve checked cross-crate API mismatches and confirm facade/prelude
  exports for the completed parity surface; `cargo check -p incular` passes.
  Intentionally deferred parity items remain listed above.
- [x] Validate the extracted configuration boundary: targeted check across all
  direct consumers, configuration/layout tests, and strict configuration
  clippy pass.

## Task 13.1 — renderer WGPU validation regression fix

- [x] Root cause: the Task 13 sweep-gradient work added an `options` brush-kind
  field (`GpuPathInstance.options`, `@location(6)` in `PATH_SHADER`'s vertex
  entry) but `path_instance_layout()` still declared only locations 1-5, so
  `create_render_pipeline` failed validation for "incular retained path
  pipeline" (and both path clip-mask pipelines sharing its vertex format)
  before any frame was presented. Fixed by declaring the missing
  `Float32x4` attribute at the struct-derived offset and rewriting every
  instance layout to derive offsets via `mem::offset_of!` on the actual
  `#[repr(C)]` upload structs instead of hand-written byte offsets.
- [x] Single source of truth for pipeline contracts: all 25 per-format render
  pipelines (rectangle, text, image, analytic rounded rectangle, retained
  path, opacity composite, 11 fixed-function Porter-Duff blends, separable
  Gaussian blur, multi-scale resample, color matrix, destination blend,
  rounded/path clip masks increment+decrement) are described declaratively in
  one `pipeline_contracts()` registry consumed by both production creation
  (`create_shared_pipeline_resources`) and tests.
- [x] Renderer startup errors: every pipeline is created eagerly once per
  target format inside a `wgpu` validation error scope; a contract failure now
  returns `RendererError::PipelineCreation { label, reason }` naming the
  pipeline instead of panicking with an uncaptured wgpu error.
- [x] GPU-independent contract tests (Naga, no display server): WGSL
  parse/validation of all eleven shader modules, vertex-entry inputs vs Rust
  vertex-buffer layouts (location/type/stride bounds), vertex outputs vs
  fragment inputs inter-stage compatibility, path paint/clip-mask layout
  sharing, and inventory completeness.
- [x] Regression test `retained_path_pipeline_vertex_interface_matches_layout`
  asserts the exact final path interface (locations 0-6, `Float32x2` quad +
  six `Float32x4` instance fields at struct offsets, stride =
  `size_of::<GpuPathInstance>`); verified it fails when the location-6
  attribute is removed.
- [x] Headless GPU smoke tests: request adapter without a surface, create a
  device, construct shared bind-group layouts, create all 25 pipelines under
  error scopes; reports `SKIPPED: no compatible headless adapter` when no
  adapter exists rather than passing vacuously. Multi-window reuse asserted:
  one `Arc<SharedPipelineResources>` per format is shared across windows.
- [x] Validation: `cargo fmt --all -- --check`, `cargo check --workspace`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo check -p incular --examples`, `git diff --check`; desktop runs of the
  scroll/painting/effects/color_effects/layout_complete/scrolling_complete/
  text/images examples complete without panics or wgpu validation errors.
  Note: `incular-workspace-tests::parity_manifest` fails on pristine HEAD
  before this task and remains unrelated to the renderer.

## Task 14 — performance profiler, benchmarks, regression contracts

- [x] Baseline repair: `specs/widget_parity.jsonl` evidence/notes fields were
  accurate in substance but lacked the literal markers the manifest contract
  requires (`test`, `prerequisite`). Every cited test was verified to exist,
  then wording corrected; workspace back to 0 failures.
- [x] Profiler core (incular-runtime `profiling` module): compact Copy
  `FrameTimings`/`FrameWork`/`FrameRecord`, bounded `FrameHistory`
  (300–1000 samples) with p50/p95/p99 statistics, `ProfilerMode`
  Normal/Diagnostic/Profiling, refresh-rate-derived budgets, present-stamp FPS.
- [x] Frame instrumentation: messages/build/layout/composite/semantics/paint
  phase spans in `run_frame_at` plus input-dispatch accumulation in
  `handle_input`; renderer prepare/encode/submit timings merged per window via
  `note_render_metrics`. Phases never nest or double-count.
- [x] Work counters: reconciliation fast paths, layout cache hits,
  display-list reuse, compositor-only updates, items built/reused, mounts,
  rebuilds — extending the existing widgets `Diagnostics`.
- [x] GPU metrics: per-frame draw/instance/pass/triangle/upload-byte/
  submission counts plus pipeline creations (steady-state invariant zero);
  optional non-blocking timestamp queries bracketing the main compositor pass
  with delayed two-frame resolution (`SKIPPED`-style "timing unavailable"
  when unsupported).
- [x] Scheduler/reactivity counters: signal reads/writes, dependents
  enqueued, runtime wakes, redraw requests, frames started/presented/skipped.
- [x] Scroll metrics: `ExtentIndexMetrics` on `MeasuredExtentIndex`
  (offset↔index queries, viewport queries, extent updates), shared across
  clones.
- [x] Public API: `Application::performance_snapshot()` (Serde JSON),
  `PerformanceHub` observer-gated publishing (~5 Hz max, Diagnostic+ only),
  repaint-contained overlay installed onto a keyed placeholder
  (`performance_overlay_placeholder()` +
  `incular::install_performance_overlay`); overlay rebuilds alone on publish.
- [x] Structural contracts (`tests/performance_contracts.rs`, all
  operation-count based, no timing thresholds): tiny-signal bounded rebuilds
  in 1k/10k trees; compositor transform animation with BUILD/LAYOUT/PAINT=0
  and COMPOSITE>0; 1M-row deep jump bounded materialization + query counts;
  warm text layout cache hit; bounded profiler history; hub observer gating;
  overlay install contract; static window never requests frames while another
  animates; idle runtime wakes produce no redraws.
- [x] Criterion suites: reconciliation/layout/semantics (widgets), signals/
  restoration (runtime), variable_list (scroll), text_layout (text),
  path_tessellation (wgpu). `cargo bench --no-run` passes; representative
  runs confirm logarithmic variable-list lookups (39 ns @10k → 246 ns @100k).
- [x] `examples/performance_gallery.rs`: 12 selectable scenarios (100k
  widgets, 1M fixed/variable lists, large text, images, paths, gradients,
  effects, nested scroll, gestures, transform animation, multi-window) with
  live overlay; auto-pilot mode cycles scenarios for hands-free validation.
- [x] tracing spans for frame phases; `[profile.profiling]` release+debug;
  PERFORMANCE.md documents architecture, caches, budgets, and known costs.

## Task 15 — measured hot-path optimization

- [x] Reconciliation root cause: per-child `Widget` deep clones (desired clone
  + old clone per visited child), a full deep-clone of every child widget via
  `children()`, and an O(font-file) copy+hash inside text layout that
  dominated changed-text frames.
- [x] Fixes: reference-based child lists (`children_refs`), borrow-compare
  unchanged bailout (zero allocation), move-out semantics cloning only on
  element creation, borrowed duplicate-key validation, custom `Key::hash`
  (single `write_u64`; measured 861 µs/frame → ~0 for keyless lists),
  guarded `sync_render_children`, O(N) keyed middle with old-position
  tracking (`elements_moved`).
- [x] Text root cause: `font_handle()` copied the entire font blob per glyph
  run and `font_id()` hashed the whole file per run; every cached layout
  pinned a private font copy (~515 KB/string). Fixed with a shared
  per-blob handle cache; document path composes retained per-paragraph
  layouts with byte-range/glyph-cluster rebasing.
- [x] Measured results (quick-mode medians, release): unchanged/10k siblings
  8.86→2.06 ms; single-child change 10.70→2.56 ms; all-texts-changed/10k
  3454→2.47 ms; insert-front/10k 11.74→2.28 ms. Document: cold 264→16.5 ms;
  warm 367 µs; single-paragraph edit 361 µs (structural test asserts exactly
  one paragraph reshaped of 301); color-only change 1.07 µs cache hit.
  Retained heap: 8k widgets 7.3 GB→54 MB; bench suite peak >4 GB→57 MB.
- [x] Correctness: proptest reconciliation-vs-reference-model suite (256
  randomized insert/remove/move/replace/change-key sequences asserting order,
  key identity, retained payload identity, counts) plus four structural
  counter contracts (unchanged 10k zero mutation work; single-change
  locality; keyed reorder preserves state with exact move counts;
  incompatible same-key replacement remounts fresh). The textarea
  pointer-navigation regression test caught (and now locks) document-cluster
  rebasing.
- [x] Dirty queue: `dirty_requests/insertions/deduplicated` counters; contract
  test proves 10,000 signal writes coalesce into one dependent rebuild.
- [x] Safety rails added after the guard incident: `scripts/bench-guard.sh`
  (RSS watchdog kills runaway benches; default 4096 MB),
  `INCULAR_HEAVY_BENCH=1` gates the 6 GB `unchanged_100k` case out of default
  sweeps.
- [x] Renderer untouched this task (Task 14 data showed it was not a
  bottleneck); Parley/Kurbo/Tokio/ICU4X/AccessKit boundaries preserved.

## Task 16 — DevTools inspector, profiler, runtime diagnostics

- [x] Crate split: `incular-devtools-protocol` (typed/versioned Serde wire
  protocol; serde-only), `incular-devtools` (target-side agent: WebSocket
  server on 127.0.0.1:<random>, random per-session token, versioned
  handshake, discovery records under the user data dir with stale-pid
  pruning, bounded telemetry queue with drop accounting), and
  `tools/incular-devtools-ui` (standalone DevTools application built with
  Incular itself; its own agent stays off to avoid recursive discovery).
- [x] Feature gating: `devtools` features chain through runtime/widgets/
  linux/facade. Production builds compile no DevTools code paths.
- [x] Runtime hooks (feature-gated): per-element build/layout/paint counters,
  content revisions, invalidation causes recorded at real invalidation sites,
  signal registry with `.devtools("name")` / `.devtools_editable()` builders,
  bounded Debug summaries captured at write time, tree snapshot extraction
  with opaque generational ids (`DevWidgetId{index,generation}`) that reject
  stale reuse.
- [x] Protocol: Hello/TargetInfo handshake with token + version validation,
  typed Request/Response/Event envelopes, WidgetNode snapshots plus
  Insert/Remove/Move/Update deltas, NodeDetails, FrameRecordEvent,
  MemorySnapshot, SignalSummary/Subscriber, DebugOption toggles, editable
  values. Malformed frames rejected; unauthorized handshakes closed.
- [x] Runner integration: agent lifecycle behind `INCULAR_DEVTOOLS=1`, on the
  application's existing Tokio runtime (no second executor); frame records,
  bounded 4 Hz subscribed-tree deltas, UI-thread command draining, and
  Select-Widget pointer interception (hover/click never dispatch to the app).
- [x] UI: connection status, multi-window selection, searchable virtualized
  widget tree, node properties/counters/invalidation details, frame timeline,
  and framework-resource/RSS snapshot. Built with Incular widgets; 100k-node
  targets retain compact rows rather than 100k DevTools widgets.
- [x] Tests: protocol round-trip/hello-validation/version-mismatch/token
  rejection/malformed rejection/tree-delta round-trip/generation semantics;
  workspace suites stay green.
- [x] Docs: DEVTOOLS.md covers architecture, enabling, security, panels,
  overhead model, and explicit non-goals (no source debugger, no hot reload,
  no network interception without an integration crate).

Known gaps (documented, not claimed): target-side layout/paint-phase visual
overlays (selected-widget and Select Widget hover overlays work),
layout-explorer visuals, render/layer/semantics tree cross-links, signal graph
panel, deep flamegraph/ranked views, memory snapshot diffing UI, logs tab
wiring, network integration crate, slow-animation time dilation, editable
widget-property overrides, and export/import formats.

## Task 16.1 — DevTools product completion

- [x] Inspector: bounded structured built-in property diffs rendered without
  raw `Debug`, with existing invalidation cause shown in the UI.
- [x] Signals: live generation/write/subscriber inventory, subscriber paths,
  selected-widget dependencies, and typed explicitly opted-in UI-thread edits.
- [x] Memory: named A/B framework resource snapshots with explicit retained
  inventory deltas (not claimed allocator/heap coverage).
- [ ] Inspector: source metadata, structured property diffs, causal rebuild
  chains, and linked retained objects.
- [ ] Layout Explorer and non-invasive target visual diagnostics.
- [ ] Signals graph and safe opted-in editing/temporary property overrides.
- [ ] Performance frame detail, filtering, deep trace, flamegraph, ranked
  aggregation, and bounded unified timeline.
- [ ] Memory snapshots/diffs and retention explanations; renderer/cache and
  oversized-image inspector.
- [ ] Accessibility, runtime/tasks/navigation/restoration, and logs panels.
- [ ] Applied slow animations; recording export/import and offline mode.
- [ ] Multi-window/reconnect/backpressure completion, overhead measurement,
  desktop validation, tests, and accurate documentation.

## Task 16.2 — Visual debugging, Layout Explorer & deep profiler

- [x] Retained Layout Explorer snapshot: constraints, resolved/local/world
  geometry, Kurbo affine coefficients, content/padding/clip/baseline, and
  typed Box/Flex/Stack/Positioned/Transform/Fitted/Scroll/Lazy/Text details.
- [x] Flex child allocation inspection uses actual retained child constraints,
  sizes, and offsets; stack/IndexedStack inspection reports actual child
  bounds and whether a child is painted.
- [x] Target-only layout overlays: selected subtree/whole-window retained
  bounds with a 10,000-rectangle ceiling and 8 Hz sampling; selected baseline
  and clip adornments remain outside widgets, hit testing, and semantics.
- [x] Animation time dilation: 1×/0.5×/0.25×/0.1×/pause commands now alter
  retained animation progression only, leaving Tokio and profiler wall time
  unchanged.
- [x] Bounded coalesced invalidation-cause representation and UI rendering.
- [x] BUILD/LAYOUT/PAINT/COMPOSITE flashes sample actual retained per-node
  counters only while enabled; display-list replay remains distinct from PAINT.
- [x] Repaint Rainbow advances only for actual retained PAINT work, never
  compositor-only updates or display-list replay.
- [x] Bounded hit-test, semantic-tree, scroll-viewport, application-layer,
  and selected padding/content overlays use retained subsystem geometry.
- [x] Opt-in Deep profiler: hierarchical typed per-node spans, 4,096-event
  frame cap with explicit drops, 300-frame auto-stop, and 200,000-event UI
  memory cap; Basic/Performance collect no node timestamps.
- [x] Batched `CustomPaint` flamegraph, clickable ranked current/range/all
  aggregation, selectable frame detail, and bounded timeline zoom/pan/range
  controls consume the same Deep trace records.
- [x] Named Signal writes carry exact bounded old/new causes through the
  reactive queue to dependent BUILDs; task-completion context is retained for
  Signal writes made by tracked async completions, while unknown edges remain
  unreported.
- [x] Deep-only 64-entry retained layout history plus explicit last
  layout/paint/composite work reasons powers Why Layout/Paint diagnostics.
- [x] Protocol/model/retained subsystem tests cover Deep hierarchy and drops,
  flamegraph geometry, range aggregation, memory bounds, Signal causality,
  overlay isolation/bounds, animation scaling, and multi-window routing.
- [x] Mandatory validation completed: serial whole-workspace tests, strict
  all-target/all-feature Clippy, and benchmark compilation pass. The gallery
  launches on native Wayland; because GNOME denies unattended Wayland window
  capture, its final interaction/pixel pass was repeated through XWayland with
  `xdotool` and a real GPU gradient workload.
- [x] Performance-gallery closure: fixed/variable virtual lists receive hard
  bounded viewports and materialize visible rows; paragraph, image, path,
  nested-scroll, and persistent gesture examples render as usable workloads.
  Example-level regression tests enforce bounded nonempty list materialization.
