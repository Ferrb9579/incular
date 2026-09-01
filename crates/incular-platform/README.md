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
decorations, transparency request, maximized state, borderless fullscreen,
portable window level, and optional validated RGBA window icon. `validate`
rejects contradictory or non-positive dimensions before native creation.

Mutable native policy is expressed with semantic `WindowOperation` values:
compositor-owned move/resize dragging, minimize/maximize/fullscreen,
resizability/decorations, logical size limits, window level/icon, and user
attention. `WindowRequestedState` is deliberately separate from
`WindowObservedState`: enqueueing a setter records application intent, while a
backend `StateChanged` event records only values the operating system can
actually report. An unobservable state is `None`, never guessed from intent.

`WindowCommand` pairs that ID with a data-only `WindowOperation`. Ordinary
fire-and-observe operations carry no request identity; operations whose native
outcome matters may carry a runtime-owned `NativeRequestId`. The platform
adapter reports those results as `NativeOperationCompletion`, using the stable
`PlatformOperationErrorKind` taxonomy rather than leaking Winit/native error
objects through the framework. A native close gesture is first represented by
`WindowEvent::platform(id, PlatformEvent::CloseRequested)` so the runtime can
apply an application close policy before issuing `Close`.

`PlatformCapabilities` is the runtime capability contract. It is grouped into
window control, display/placement, transient surfaces, native menus, data
transfer, application services, and advanced input instead of one platform
flag. `Unknown` means no backend/session has published support yet;
`Unsupported` is an explicit backend statement. This distinction is important
for session-dependent facilities such as Wayland placement and native desktop
services.

## Displays and placement

`DisplayId` is a generational Incular identity, not a monitor name or an
enumeration index. `DisplaySnapshot` is immutable and distinguishes monitor
pixel size from optional desktop-global bounds and optional usable/work area.
Display-local logical/physical coordinates have dedicated wrapper types so a
mixed-DPI conversion never silently treats one monitor's scale as a global
desktop scale. Global screen positions likewise use explicit physical screen
types rather than widget `Offset` values.

Top-level placement is capability-gated. Win32, AppKit, and X11 can expose
Winit's physical desktop position/bounds; Wayland deliberately reports global
bounds, outer-position queries, and outer-position setters as unsupported.
Wayland still exposes display-local size/scale/name and current-display
association where Winit can provide them. A missing work area is represented as
`None` and must never be replaced with full monitor bounds under the same API.

`WindowEvent` associates existing `PlatformEvent` input/metrics/close data
with a normalized `WindowId`; `WindowLifecycle`, `RedrawRequested`, and
`StateChanged` provide the extra per-window state native adapters need without
extending the legacy single-window event stream. Application lifecycle remains
separate from window visibility and focus lifecycle.

Keyboard navigation, printable text and IME composition are normalized into
separate `InputEvent` variants. `TextInputConfiguration`, `TextInputState`, and
`TextInputCommand` form the data-only native text-input bridge: the runtime
selects the focused client, publishes selection/composing/caret state, and
receives explicit soft-keyboard actions through `PlatformEvent`. Winit adapters
apply those commands to the native IME; mobile hosts can consume the same
commands without a window dependency. `Clipboard` is the backend boundary;
the shared desktop runner installs an `arboard` system-clipboard bridge with a
safe in-memory fallback and publishes whether native clipboard interop is
actually available for the current session.

Pointer input has one metadata-rich representation. Mouse events carry a stable
process-local device ID, the complete pressed-button chord, and the button that
changed on Down/Up; primary, secondary, middle, back, forward, and representable
extra buttons use distinct portable bits. Touch uses the same event shape with
stable contact/device identity. `PointerPhase::Enter`/`Exit` represent native
surface crossings without fabricating a local coordinate on enter.

Native cursor policy is separate from cursor styling. `CursorGrabMode` and
validated `LogicalWindowPosition` back result-bearing window operations for
grab/lock, visibility, and cursor warping. `AdvancedInputCapabilities` reports
icons, visibility, position, confinement, and locking independently. Optional
Wayland pointer-constraints support remains `Unknown` until the actual Winit
operation reports success or a typed unsupported/native failure.

The scroll convention is positive logical `delta.y` increasing the controller
offset (content moves upward). Winit `LineDelta` is scaled by 40 logical px;
`PixelDelta` is converted through DPI and normalized to natural content
direction without rounding, preserving fractional trackpad movement. Key commands are delivered on pressed events
(including native repeats); control-text payloads such as Backspace are not
forwarded as committed text.
