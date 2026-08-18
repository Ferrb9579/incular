# incular-platform

Owns normalized native events, raw window-handle extraction for backend use,
and the logical/physical DPI boundary. Widgets and runtime receive logical
coordinates only; Linux converts physical pointer positions through
`WindowMetrics` before hit testing.

## Window contract

`WindowId` is an Incular-owned `(slot, generation)` identity, never a Winit
ID or a native pointer. A window registry advances the generation whenever it
reuses a slot, allowing queued stale commands and handles to be rejected.

`WindowOptions` contains the portable desktop creation subset: title, initial
and minimum/maximum logical sizes, resizability, initial visibility,
decorations, transparency request, maximized state, and borderless fullscreen.
`validate` rejects contradictory or non-positive dimensions before native
creation.

`WindowCommand` pairs that ID with a data-only `WindowOperation` (`SetTitle`,
`SetVisible`, `SetLogicalSize`, `RequestFocus`, `RequestRedraw`, or `Close`).
It is safe to queue from asynchronous work, but a native adapter applies it
only during its UI/event-loop turn. A native close gesture is first represented
by `WindowEvent::platform(id, PlatformEvent::CloseRequested)` so the runtime
can apply an application close policy before issuing `Close`.

`WindowEvent` associates existing `PlatformEvent` input/metrics/close data
with a normalized `WindowId`; `WindowLifecycle` and `RedrawRequested` provide
the extra per-window state native adapters need without extending the legacy
single-window event stream. Application lifecycle remains separate from window
visibility and focus lifecycle.

Keyboard navigation, printable text and IME composition are normalized into
separate `InputEvent` variants. `Clipboard` is the backend boundary; Linux
installs an `arboard` system-clipboard bridge with a safe in-memory fallback.

The scroll convention is positive logical `delta.y` increasing the controller
offset (content moves upward). Winit `LineDelta` is scaled by 40 logical px;
`PixelDelta` is converted through DPI and normalized to natural content
direction without rounding, preserving fractional trackpad movement. Key commands are delivered on pressed events
(including native repeats); control-text payloads such as Backspace are not
forwarded as committed text.
