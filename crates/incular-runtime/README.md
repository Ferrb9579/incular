# incular-runtime

Application lifecycle and scheduling foundations for Incular, including frame
scheduling, widget-tree updates, input dispatch, animation ticking, resource
coordination, and rendering orchestration.
# incular-runtime

Owns the controlled frame queue, declarative `Application` roots, callback
lifetimes, and `Signal<T>` dependency scheduler.
`schedule_update` coalesces direct updates; signal writes queue only Elements
whose registered builders read that signal. `run_frame` processes build updates,
incremental layout, then cached paint generation. It has no platform or GPU
dependency.

It then performs a COMPOSITE phase that writes retained transform properties and
flattens the layer tree. `run_frame_at` accepts a monotonic `Instant` for
deterministic animation tests; ordinary `run_frame` supplies `Instant::now()`.
Idle applications request no recurring frames, while an active compositor
animation requests the next one.

## Production runtime foundation

Tokio is Incular's application async runtime. Each `Application`/`Runtime`
owns one Tokio multi-thread runtime (time and I/O enabled) and exposes a cloned
`TokioHandle` for advanced libraries. Tokio owns futures, Tokio timers, async
I/O, synchronization, and `spawn_blocking`; Incular deliberately does not
implement another executor, timer wheel, or worker pool.

The Incular runtime still owns the UI thread, retained widget/render/layer
trees, frame phases, environment/lifecycle state, and native windows. Tokio
work returns `Send` data through a typed UI message bridge. The bridge wakes
Winit only for newly queued UI-relevant work and coalesces bursts; the UI owner
drains at most 128 messages per native turn. A wake alone never requests a
frame—only an actual `Signal`/widget/compositor mutation does.

Use `cx.spawn` for a component-owned Tokio task and `cx.spawn_into` when a
completion must update UI-local state. The future and its result are `Send`,
but the completion callback runs solely on the UI thread and can therefore
capture a single-threaded `Signal`. `app.spawn` is application-scoped;
`app.tokio_handle()` is intentionally available for direct Tokio-native use and
does not attach automatic component ownership.

`TaskScope` aborts normal Tokio tasks when an owner unmounts. A pending
completion validates both the scope and its generational `ElementId` before it
can execute, so a stale completion cannot mutate a reused element slot. A
started blocking closure cannot safely be force-stopped; cancellation marks its
result uninterested and the UI bridge discards it. Shutdown aborts tracked work
and calls Tokio's bounded `shutdown_timeout` policy rather than waiting
indefinitely for blocking jobs.

`RuntimeEnvironment` is the normalized logical environment snapshot: viewport,
physical size, DPI, brightness, text scale, logical insets, locale/direction,
motion, and input capability. `BuildContext` records typed field reads so an
unrelated environment change does not rebuild the root just because, for
example, it only reads text scale.

## Multi-window runtime

An `Application` owns exactly one Tokio runtime and can own many retained
windows. Every `WindowId` is an Incular `(slot, generation)` value; public APIs
never expose Winit IDs, a native window pointer, or a GPU surface.
`Application::open_window` mounts a static root and
`Application::open_window_with` mounts a declarative root. A build may call
`cx.open_window`, while a callback can retain `cx.window_opener()` and queue a
new root later. Native creation is deferred to the desktop adapter's active
event-loop callback.

Each window owns its WidgetTree, input and logical focus, environment,
semantics tree, compositor work, presentation scheduling, and a window
`TaskScope`. The hierarchy is application → window → component: application
tasks survive a window close, while window/component completions are cancelled
and generationally rejected once their root closes. `WindowHandle` operations
are data-only messages, so a Tokio worker cannot mutate a native window.

`WindowHandle` distinguishes command transport from native execution. Ordinary
setters return `Result<(), WindowCommandEnqueueError>`, which only reports
whether the command crossed the runtime/UI bridge. Operations that require a
native outcome return `NativeOperationRequest`, an awaitable request completed
by the platform backend. Completion is keyed by both `NativeRequestId` and the
generational `WindowId`; a close/reused slot cannot consume an old result, and
each request resolves at most once. Dropping the request detaches result
delivery without trying to undo an operation that may already have executed.

`Application::platform_capabilities` and per-window capability snapshots expose
what the active backend/session can actually honor. Before a backend attaches,
capabilities are `Unknown`; explicit headless/unsupported backends publish
`Unsupported` rather than pretending an operation succeeded. Native failures
are normalized to stable platform-error categories and the last per-window
failure is visible through `WindowDiagnostics` without retaining native error
objects.

`Signal` subscriptions are retained per root. A shared signal rebuilds only
the roots that read it; a signal read only by one window leaves every other
window idle. Physical metrics, DPI, safe/view insets, and native activation are
window fields. Locale, direction, and text scale may still originate from an
application default but resolve through the owning `BuildContext`.

## Derived values and asynchronous reactive work

`Memo<T>` is the opt-in graph path for an expensive or shared derived value. It
is created like a `Signal`, without a `BuildContext`, and attaches lazily when
its `get` method is first read by a builder. Its computation tracks signals and
other memos dynamically; a dirty memo is evaluated once per queued runtime
phase and only notifies consumers when its output changes. Cheap expressions
should remain ordinary Rust expressions because a memo has bookkeeping and
comparison overhead.

`Effect` is an owner-mounted side effect. Create a stable handle outside a
rebuilding builder, call `mount` from that builder, and the initial run is
queued for the reactive phase rather than executing during build. Mounting is
idempotent, source reads are retracked after every run, and disposing the
runtime detaches its source subscriptions.

`Action<I, O, E>` is an explicitly dispatched asynchronous operation. Reading
its state binds it to the current owner; constructing it never starts work.
`dispatch` marks the state as loading, runs through the existing Tokio/UI task
bridge, and exposes success, application failure, or runtime cancellation.
Dispatches are latest-wins, so a late completion cannot overwrite a newer
request. The public facade exposes these names under `incular::reactive` so
they do not collide with the rendering `Effect` or widget `Action` types.

## Native accessibility

The runtime keeps the retained `SemanticsTree` independent of paint and native
accessibility. A desktop runner owns one `incular-accessibility::AccessKitProjection`
and one `accesskit_winit::Adapter` for each live `WindowId`; it calls
`Application::sync_accessibility` only after native AccessKit activation. The
tree revision makes unchanged/compositor-only frames a zero-update path. A
native action is translated by the adapter to an owned Incular semantic action,
then `Application::dispatch_accessibility_action` resolves it against that
same window's retained tree on the UI thread. Generational semantic IDs reject
stale actions before a reused element can be touched.

`WindowDiagnostics::accessibility` exposes value-free, per-window bridge
health. Closing a window removes its native adapter/projection record only;
other windows keep independent semantic and native trees. The runtime does not
construct an additional async executor for accessibility.

The default `LastWindowPolicy::ExitOnLastWindow` exits when no visible retained
window remains, so a hidden auxiliary window cannot accidentally keep a desktop
application alive. Applications that intentionally provide background/headless
service lifetime must opt into `KeepRunning`. `Application::debug_dump` and
per-window diagnostics expose frame, input, metric, retained-tree, semantics,
and stale-command state without leaking native details.

## Opt-in state restoration

The default file-backed restoration store resolves its persistent location with
`directories::ProjectDirs` through `incular-platform`. It uses the OS local
application-data directory, never the cache directory; explicit file and
in-memory stores remain available for embedding and tests.

Restoration persists small, declarative application values—not a live widget,
element, render, layer, semantics, native-window, WGPU, gesture, callback, or
Tokio-task graph. It is disabled unless the application uses
`Application::new_with_restoration` or `Application::new_restorable` with a
`RestorationConfig`. `RestorationConfig::file_backed` stores a versioned JSON
snapshot under the stable application ID's platform state directory; tests and
embedders can use `InMemoryRestorationStore` or supply a narrow custom
`RestorationStore` instead.

Each value and scope uses an explicit `RestorationKey` segment. The runtime
escapes segments deterministically for its internal path map, never derives
identity from an element slot or child position, and rejects concurrently
mounted duplicate restorable-window IDs. A builder obtains its scope from
`BuildContext::restoration_scope`, then opts in with
`cx.restorable(key, default)`/`cx.restored_signal(key, default)`. Mutate the
returned `Restorable<T>` rather than a plain `Signal` to capture updates; plain
signals remain deliberately non-persistent.

Text editing, form fields, scroll/page controllers, and navigator snapshots
can bind a scope explicitly. Text stores committed text plus a normalized
selection, never IME composition; forms recompute validation; scroll restores
only a logical offset after layout and clamps it; navigation stores only stable
registered route IDs plus JSON arguments/state. Dialogs and arbitrary overlays
are transient by default.

Snapshots carry distinct framework and application schema versions. A supplied
migration runs before the first UI build observes values. Corrupt, future, or
unmigratable files leave the original file available for debugging and start
from defaults. Saves are coalesced for 250ms by default, serialize/write on
Tokio's blocking pool, enforce an 8 MiB default limit, flush a sibling temporary
file, sync it, then rename it. A failed replacement leaves the previous valid
snapshot untouched; actual power-loss durability is the filesystem's guarantee,
not a stronger framework claim. `flush_restoration`, app stopping, and platform
suspension bypass the ordinary debounce; shutdown waits only a bounded 100ms
for the normal save completion.

Restorable windows use `WindowRestorationId`, not `WindowId`. Register each
auxiliary kind with `register_restorable_window_factory`, then call
`restore_restorable_windows` before entering the native runner. Incular
restores validated logical size/maximized/fullscreen state and skips unknown
factories safely; it does not restore arbitrary window position or DPI. A user
closing an auxiliary restorable window removes it from the next session, while
application shutdown preserves active descriptors. Restoration storage is not
encrypted: never place passwords, tokens, or other secrets in it.
