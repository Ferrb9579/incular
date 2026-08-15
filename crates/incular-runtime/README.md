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
