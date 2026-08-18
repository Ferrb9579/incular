# incular-linux

Owns the Linux `winit` application handler and native window lifetime. It
forwards normalized pointer events to `incular-runtime`, requests redraws only
for scheduled work/resize/input, and keeps the window alive while
`incular-wgpu` owns its raw-handle surface.

The Winit 0.30 active event loop is the only place this crate creates,
destroys, or mutates a native window. `Application` queues Incular-owned
generational window commands; this adapter keeps the private Winit-ID mapping,
creates a retained root's surface in `resumed`/event-loop turns, and routes
every input, IME, focus, resize, DPI, redraw, and close request back through
that Incular ID. A native close request is passed through the application's
close policy before the native object is dropped.

One `SharedGpuContext` is initialized from the first surface and creates a
per-window renderer for later surfaces. The device, queue, immutable pipelines,
images, glyph resources, and gradients are shared; surface configuration,
stencil/offscreen/compositor caches, size, and presentation remain local to
each native window. A zero-sized/minimized surface is never configured or
presented. Tokio wakes Winit only to process UI messages or queued window
commands, so idle auxiliary windows do not consume frames.

For accessibility, each Winit window is created initially invisible, receives
its own `accesskit_winit::Adapter` before first presentation, then becomes
visible. `Adapter::process_event` receives every Winit window event before
Incular's ordinary input routing, so no pointer, keyboard, IME, focus, resize,
or close event is swallowed. AccessKit activation/action/deactivation arrives
through the existing Winit user-event channel and is handled on the UI thread.
The adapter uses AccessKit's `tokio` feature on Unix, sharing Incular's existing
Tokio integration rather than starting an async-io or second Tokio runtime.
