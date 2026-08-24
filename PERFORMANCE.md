# Incular performance architecture

This document describes how Incular's retained architecture behaves, what is
measured, and how to profile or benchmark it. It is deliberately explicit
about what costs work: nothing here claims "faster than X" — Task 14 measures
Incular itself.

## Retained model

Applications build a declarative widget tree; the framework retains it as an
element arena (`Element`) plus a render-object arena (`RenderObject`) and a
compositor layer tree. A frame runs fixed phases:

```text
messages → BUILD → LAYOUT → COMPOSITE → SEMANTICS → PAINT
```

- **BUILD** diffs desired widgets against retained elements. Keyed children
  use prefix/suffix fast paths before a keyed middle-diff that builds one
  old-key map and walks the new middle once (O(N), never O(N²)). Identical
  widgets short-circuit entirely (`identical_child_bailouts` counter), and
  unchanged child lists cost one borrow-compare per child with zero
  allocation: desired children are walked by reference and byte offsets are
  derived from the retained structs, not hand-written.
- **LAYOUT** re-resolves only render objects whose `DirtyFlags::LAYOUT` is set
  or whose incoming constraints changed (the constraint-equality check doubles
  as a cache).
- **COMPOSITE** pushes transform/opacity/blur/shadow/color-filter updates into
  retained layers without touching BUILD/LAYOUT/PAINT caches.
- **SEMANTICS** walks the tree into the semantic tree; the AccessKit
  projection skips publishing when the semantic revision is unchanged.
- **PAINT** records display-list commands per render object once into a
  per-node cache and replays that cache while it stays clean.

## Compositor-only animation

Transforms, opacity, blur, drop shadow, color filters, and scroll offsets are
retained layer state. Animating them ticks controllers during COMPOSITE and
produces zero BUILD/LAYOUT/PAINT work and zero GPU uploads for the affected
subtrees. This is enforced by structural tests, not asserted in comments.

## Virtualization

Fixed and variable-extent virtual lists materialize only
`visible + cache_extent` rows. The variable index (`MeasuredExtentIndex`)
keeps extents in chunks with Fenwick-summarized trees, so:

- offset→index and index→offset lookups are O(log N + chunk);
- viewport range computation is pure arithmetic;
- deep jumps never measure preceding rows.

Structural counters on the index prove this without timing thresholds.

## Reactivity

Signals track per-element dependencies. Writing a signal enqueues exactly the
elements that read it (deduplicated); unrelated retained subtrees do no work,
regardless of tree size. The contract test builds 1k/10k trees with three hot
texts and asserts bounded rebuilds after each write.

## Caches

Every cache exposes cumulative hit/miss/insert/eviction diagnostics:

| Cache | Contents |
| --- | --- |
| Text engine | shaped layouts keyed by text+style+options+font generation |
| Glyph atlas | DPI-keyed rasterized masks with page-level uploads |
| Images | decoded textures shared device-wide across windows |
| Paths | tessellated meshes keyed by path identity + fill/stroke parameters |
| Gradients | normalized LUT textures shared by brush key |
| Effects | offscreen targets, blur kernels, effect-stage chains |

Caches are *retained* by documented budgets/eviction policy; capacity is not a
leak. Teardown of application content returns live element/render/layer counts
to their baselines.

## GPU ownership

One `SharedGpuContext` owns the `wgpu` instance/adapter/device/queue per
application. Windows share pipelines per target format through
`Arc<SharedPipelineResources>`; opening another window does not duplicate
pipelines unless the surface format genuinely differs. All production
pipelines are created eagerly at renderer initialization from a single
contract registry and validated under wgpu error scopes; steady-state frames
create zero pipelines (enforced by test). Optional non-blocking GPU timestamp
queries bracket the main compositor pass when the adapter supports them;
results resolve asynchronously two frames later via `poll(Poll)` — never
`Wait`.

## Idle behavior

Frames run only when something requested one: dirty elements, pending
reactive rebuilds, active animations, or visible runtime completions. Runtime
wakes that mutate no visible state produce no redraw request and no frame.
There is no idle ticker.

## Profiling

- **Modes** (`Application::set_profiler_mode`): `Normal` (counters only),
  `Diagnostic` (bounded frame history), `Profiling`.
- **Snapshot** (`Application::performance_snapshot()`): scheduler counters,
  budget stats, per-window latest frame + renderer metrics + GPU sample,
  frame percentiles (p50/p95/p99), widget work totals, text cache behavior,
  accessibility skip counts. Serde-serializable for machine-readable export.
- **Budgets** derive from the actual refresh rate
  (`Application::set_refresh_rate_hz`), never a hardcoded 16.67 ms.
- **Overlay**: place `performance_overlay_placeholder()` anywhere in your
  tree and call `incular::install_performance_overlay(&mut app, window_id)`.
  The overlay is repaint-contained and rebuilds only when the hub publishes
  (~5 Hz, only in Diagnostic+ modes, only when observed).
- **tracing spans**: `incular.messages`, `incular.build`, `incular.layout`,
  `incular.composite`, `incular.semantics`, `incular.paint` — attach any
  subscriber; they cost nothing without one.

## Benchmarks

Criterion suites live in each crate's `benches/`:

| Suite | Crate | What it shows |
| --- | --- | --- |
| reconciliation | incular-widgets | diff cost vs tree size/mutation shape |
| layout | incular-widgets | cached/cold/constraint-change layout cost |
| semantics | incular-widgets | semantic collection scaling |
| signals | incular-runtime | invalidation cost vs subscriber count |
| restoration | incular-runtime | snapshot capture/restore/store round trip |
| variable_list | incular-scroll | lookup/deep-jump/update scaling to 1M rows |
| text_layout | incular-text | cold shaping vs warm cache across scripts |
| path_tessellation | incular-wgpu | Kurbo/Lyon tessellation cost by complexity |

Run targeted benchmarks:

```bash
cargo bench -p incular-scroll --bench variable_list -- --quick
```

Do not commit generated Criterion report directories (`target/criterion`).

## External profiling

Release measurements require optimized builds:

```bash
cargo run --release -p incular --example performance_gallery
# With symbols for perf/samply:
cargo run --profile profiling -p incular --example performance_gallery
perf record -g -- cargo run --profile profiling -p incular --example performance_gallery
```

Debug-mode timings are not representative.

## Measured optimization record (Task 15)

Structural counters live in `WidgetTree::diagnostics()` (child-list scans,
identical-widget bailouts, type/key/config comparisons, key-map builds,
created/reused/removed/moved) and `TextDiagnostics` (paragraphs considered /
reshaped, Parley layouts reused, documents composed). Criterion suites:
`reconciliation*` (widgets) and `text_layout` / `text_document` (text).
Heavy cases are opt-in: `INCULAR_HEAVY_BENCH=1` gates `unchanged_100k`, and
`scripts/bench-guard.sh` kills any benchmark process tree that exceeds a
resident-memory limit (`LIMIT_MB`, default 4096 MB).

### Font-blob retention root cause

`GlyphRun.font` carried a `FontHandle`, and `TextEngine` built that handle by
copying the entire font file into a fresh `Arc<[u8]>` for every glyph run —
plus hashing the full file to compute its id. Every cached text layout
therefore pinned a private ~500–750 KB copy of the font. Measured effect and
fix (handles are now shared per blob, id computed once):

| Metric | Before | After |
| --- | --- | --- |
| Retained heap per unique shaped string | ~515 KB | ~2.6 KB |
| Live heap, 8k mounted text widgets | 7.3 GB | 54 MB |
| Full reconciliation bench peak RSS | >4 GB | 57 MB |

### Reconciliation benchmarks (10k siblings, quick-mode medians)

| Case | Before | After |
| --- | --- | --- |
| unchanged | 8.86 ms | 2.06 ms |
| single leaf changed | 10.70 ms | 2.56 ms |
| all texts changed | 3454 ms | 2.47 ms |
| insert front | 11.74 ms | 2.28 ms |
| reverse keyed / rotate / insert & remove middle | — | 2.3–9.3 ms, all O(N) |

`all_text_changed` collapsed ~1400× because every relayout used to pay the
font copy+hash; reconciliation itself improved 4–5× from removing per-child
widget clones (borrow-compare plus reference-based child lists).

### Text documents (300 paragraphs)

| Case | Before | After |
| --- | --- | --- |
| cold layout | 264 ms | 16.5 ms |
| warm unchanged | 367 µs | 367 µs |
| single-paragraph edit | ~264 ms (monolithic key) | 361 µs |
| color-only change | — | 1.07 µs (layout-cache hit) |

Multi-paragraph documents now compose from per-paragraph retained layouts:
editing paragraph K reshapes exactly that paragraph (structural test asserts
`paragraphs_reshaped == 1` of 301). Selection/caret changes consume the
retained layout and never shape or rasterize; glyph clusters and line ranges
are rebased to document coordinates during composition (covered by the
textarea navigation regression test).

### Dirty queue

Invalidations count `dirty_requests` / `dirty_queue_insertions` /
`dirty_queue_deduplicated`. Ten thousand signal writes before a frame enqueue
one dependent once and produce exactly one rebuild carrying the final value
(contract test).

## DevTools overhead boundary

Production builds without the `devtools` feature contain no protocol transport
or per-node trace buffers. With the feature enabled, Basic and Performance
modes still record no node timestamps. Geometry overlays are sampled, batched
into the existing diagnostic display list, and capped at 10,000 regions.

Deep mode is deliberately diagnostic rather than benchmark mode. It performs
wall-clock reads around retained node work, caps each frame at 4,096 events,
auto-stops after 300 frames, and caps the standalone UI at 200,000 retained
events. Truncation and telemetry drops are visible. Use Task 14 aggregate frame
records—not Deep captures—for final before/after performance numbers.

## Known expensive features

These legitimately cost work; budgets should account for them:

- Large Gaussian blurs and big offscreen layers (extra passes and textures).
- Complex path tessellation on first appearance (cached afterward).
- Cold text shaping for new strings/scripts (cached by the text engine).
- First-frame glyph atlas population per font/DPI combination.
- Destination-read blending falls back to intermediate-target composition.
