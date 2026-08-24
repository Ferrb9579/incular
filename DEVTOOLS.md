# Incular DevTools

Development-time inspector and profiler for running Incular applications,
dogfooded on Incular itself.

## Architecture

```text
Running Incular Application          Standalone DevTools App
  └─ Tokio transport task (feature)    └─ tools/incular-devtools-ui
       │ WebSocket 127.0.0.1:<random>       │ typed JSON protocol
       └────────── incular-devtools-protocol ──────────┘
```

- `incular-devtools-protocol` — versioned Serde messages, opaque IDs,
  snapshots/deltas. No runtime dependency.
- `incular-devtools` — target-side agent: localhost-only WebSocket server
  (`tokio-tungstenite`), per-session random auth token, discovery records,
  bounded telemetry queues. It is spawned on the application's existing Tokio
  runtime; it never creates a second executor.
- `tools/incular-devtools` — the standalone UI binary built with Incular.

## Enabling

Build the standalone UI once, then launch any DevTools-enabled application
with one runtime flag:

```bash
cargo build -p incular-devtools-ui
cargo run -p incular --example counter --features devtools -- --devtools
```

`--devtools` starts the target agent, opens `incular-devtools`, and connects it
to the exact target PID. Applications should forward their own Cargo feature
to `incular/devtools` when they depend on Incular from another package. Set
`INCULAR_DEVTOOLS_UI=/path/to/incular-devtools` when the UI binary is not next
to the application or available on `PATH`.

Without the runtime flag the agent never starts: no socket, discovery file, or
telemetry allocation. `INCULAR_DEVTOOLS=1` remains supported as an agent-only
mode for scripts that launch the UI separately.

## Launching

```bash
cargo run -p incular-devtools-ui
```

The standalone command connects to the newest discovered target by default.
`incular-devtools --target-pid <PID>` selects one exact process, and stale
records are pruned after crashes.

## Connection security

- Bind address is `127.0.0.1` only; remote debugging is not implemented.
- Each target generates a random 32-hex-char token; DevTools must echo it in
  the handshake or the socket closes with `Unauthorized`.
- Handshake rejects protocol-version mismatches cleanly.
- Sensitive values are excluded at the source: editable text fields transmit
  only a neutral summary ("text field"/placeholder), never user content.

## Inspector

Widget-tree snapshot plus incremental deltas keyed by stable arena
index/generation ids. The standalone UI keeps an id/depth row index and uses
`VirtualList`, so a 100k-node target does not create 100k DevTools widgets.
Subscribed trees are sampled at most four times per second and only changed
nodes cross the transport after the initial snapshot.
Framework plumbing widgets (RepaintBoundary, LayoutBuilder) are hidden unless
requested. Node details expose curated properties (Text: text/fontSize/color/
maxLines/…), element counters, constraints/size/bounds, render/semantics ids,
the retained baseline/clip state, and the last invalidation cause. Compatible
widget updates also retain a bounded, structured list of changed curated
properties; arbitrary application object graphs never cross the protocol.

The UI keeps the tree in a dedicated bounded pane. Hovering a row highlights
the corresponding widget in the running application without changing its
layout or hit testing; leaving the row restores the selected-widget overlay.
The disclosure chevron expands or collapses a branch without changing
selection; clicking the rest of a row selects the node, expands its ancestors,
and loads its properties. Expand tree and Collapse tree operate on the bounded
snapshot as a whole.
Select Widget mode temporarily intercepts target pointer input, highlights
the target under the mouse, and reveals the clicked widget in the tree.

Curated properties are always visible. Properties are read-only unless their
target adapter explicitly marks them editable. The typed editor rejects invalid
or stale writes, labels active values as `DEV OVERRIDE`, and Reset property
overrides restores the value captured before editing. Static opacity is the
first supported retained override; controlled/animated opacity remains
read-only so DevTools cannot silently replace an application controller.

### Layout Explorer

The selected-node Layout Explorer is a read-only retained snapshot: incoming
constraints, resolved size, local offset, Kurbo affine coefficients, exact
world bounds, padding/content bounds, baseline, and rectangular clip state.
It does not call layout again. Typed details currently cover Box, Flex (actual
child constraints/allocations/sizes/offsets), Stack/Positioned/IndexedStack
(including painted child), Transform/FittedBox, ScrollView, VirtualList, and
retained Parley text line count. Other widget types identify their retained
layout kind without pretending to have strategy-specific data.

### Visual overlays

The target paints debug geometry after its retained application display list;
it never inserts widgets and therefore cannot change application BUILD,
LAYOUT, semantics, restoration, or normal hit testing. Selected-subtree and
whole-window bounds use a single diagnostic display list, are limited to
10,000 rectangles, and are sampled at most 8 Hz. Selected baseline and clip
geometry are available as separate overlays. Additional bounded passes read
actual hit-routing participants, `SemanticsTree` bounds, scroll viewports, and
application-relevant retained layer owners. Padding widgets expose their exact
content rectangle. Select Widget is the sole mode that intentionally
intercepts pointer input.

BUILD/LAYOUT/PAINT/SEMANTICS/COMPOSITE flash controls sample the existing feature-gated
per-node retained counters only while enabled. A flash therefore represents
actual work in its named phase; display-list replay does not increment PAINT.
The initial implementation holds changes until the next target frame rather
than scheduling a fading debug animation, so it cannot keep an otherwise idle
application redrawing.

## Why did this rebuild?

Every element exposes its latest invalidation cause and a bounded (eight)
coalesced-cause list where a runtime path records several invalidations before
the next build. Direct configuration/environment paths and named Signal writes
feed that list. Signal old/new summaries travel through the reactive queue to
the exact dependent element rather than being inferred by the UI. The selected
node renders a compact graph containing only recorded cause → BUILD edges;
unavailable async/navigation edges remain absent rather than invented.
Signals registered via `.devtools("name")` capture bounded value summaries;
`.devtools_editable()` additionally allows DevTools-driven writes for
primitive payload kinds, routed UI-thread → Signal::set → normal invalidation.

## Signals

`ListSignals` / `GetSignalSubscribers` expose the registry: id, name, type,
value generation, write count, subscriber count/paths, and editability. The
standalone UI selects a signal to show live subscriber paths and supplies a
typed editor only for an application's explicit primitive opt-in. Selected
widgets also show the signals they actually consume. Unmounted subscribers
drop out through the existing weak-dependency tracking.

## Performance

Frames stream as `FrameRecordEvent`s (Task 14 remains authoritative): CPU phases,
renderer prepare/encode/submit, GPU main-pass time when timestamp queries
resolve, draw calls, instances, upload bytes, pipeline creations. Linux
reports a per-frame CPU budget from the active monitor refresh rate when Winit
exposes one; otherwise budget and GPU timing are shown as unavailable rather
than guessed.

The explicit Basic/Performance/Deep switch maps onto the runtime profiler.
Only an active Deep recording enables per-node wall-clock spans. BUILD,
LAYOUT, PAINT, SEMANTICS, and COMPOSITE events use typed phases and parent event
indices; layout cache hits and display-list replay do not create false work.
Each frame is capped at 4,096 events with drop/truncation metadata, recordings
auto-stop at 300 frames, and the UI additionally caps retained trace events at
200,000. The UI renders the selected trace as one batched `CustomPaint`
flamegraph, offers clickable ranked aggregation for current frame/selected
range/whole recording, and supplies selectable frame details plus bounded
timeline zoom/pan controls. Deep layout history keeps only 64 relevant changes
per element and exists only while Deep capture is active.

Deep instrumentation changes execution characteristics and must not be used
for final benchmark numbers; use Basic/Performance and Task 14 aggregates for
comparative performance work.

## Animation time dilation

The DevTools controls support 1×, 0.5×, 0.25×, 0.1×, and pause. They map real
frame deltas onto a retained animation-only clock. Tokio timers, filesystem or
network work, input timestamps, and profiler wall-clock timings stay real.
Pause freezes controllers while the application and DevTools remain
interactive.

## Memory

Resource counts (elements, render objects, semantics nodes, signals, tracked
tasks, and current process RSS) with snapshots over framework-owned state only.
The UI supports named A/B snapshots and reports their explicit retained-count
deltas. This is *not* arbitrary Rust heap introspection.

## Logs

The protocol reserves bounded log records. Target-side tracing capture and a
dedicated log panel are not implemented yet.

## Network

A telemetry API exists in the protocol; no automatic socket interception.
Panels report "unavailable" without an integration crate.

## Renderer

Pipeline inventory/counters come from Task 13.1 contracts; GPU timing from
Task 14 timestamp queries; cache hit/miss/eviction counters from Task 14.

## Overhead

Disconnected builds perform zero serialization, zero socket IO, and zero deep
per-node capture — all instrumentation sits behind `cfg(feature = "devtools")`
and runtime gating. Connected Basic mode streams one small frame event per
presented frame through a bounded queue that drops (and reports drops) under
backpressure. Tree sampling starts only after a client requests that window,
is capped at 4 Hz, and retains just one subscribed-tree snapshot for delta
comparison. Per-node timestamps are absent in Basic and Performance modes.

## Not included

- Source-level debugging: use rust-gdb/lldb/IDE debuggers. DevTools surfaces
  source locations only (future extension).
- True hot reload does not exist for Rust here; restoration-backed fast
  restart may come later as a separate launcher feature.
- Binary-size attribution: use cargo-bloat/cargo-llvm-lines.
