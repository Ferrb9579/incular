# incular-desktop

Shared desktop shell for Incular.

This crate owns the platform-neutral Winit/WGPU/AccessKit integration used by
the Linux, Windows, and macOS adapters: native window lifetime, Incular/native
window ID mapping, frame scheduling, renderer setup, input/IME translation,
clipboard wiring, accessibility projection, and optional DevTools attachment.

The shared shell also implements portable native window control through Winit:
compositor-owned move/resize drag, mutable minimize/maximize/fullscreen,
decorations/resizability/size limits, window level/icon, and attention requests.
Capabilities are refined from the concrete window system after creation (for
example X11 vs Wayland) before an operation is attempted. Incular does not
paper over a Winit method that is a documented no-op on the active backend;
unsupported operations are reported through the platform capability/error
contract.

Display discovery is retained in a private generational registry keyed by
Winit's native monitor identity. Reordering enumeration does not change an
Incular `DisplayId`; disconnecting a monitor invalidates its generation before
that slot can be reused. The shell publishes monitor size/scale and current
association on every supported desktop, plus physical desktop bounds and
top-level placement only on window systems that define them. Wayland output
geometry is intentionally not promoted into a fake top-level global coordinate
space.

Winit does not expose taskbar/dock-adjusted work areas, so the shared shell has
a narrow `DesktopPlatformServices` seam for OS facade crates. Windows supplies
`MONITORINFO.rcWork`; macOS supplies `NSScreen.visibleFrame` converted into
Winit's top-left physical coordinate model. The default service (including the
current X11 adapter) reports work areas unsupported rather than guessing from
full monitor bounds.

Mouse input is normalized without a left-button shortcut. A process-local
device registry preserves Winit `DeviceId` identity, each native window owns its
pressed-button chord, and every representable mouse button reaches the runtime
with both the complete chord and changed-button bit. Cursor enter/leave is
forwarded explicitly, touch uses the same metadata-rich path, and multi-window
button state cannot leak between windows.

Retained cursor styling is synchronized back to Winit after input and frame
updates through a deduplicating coordinator, including diagonal resize cursors.
Cursor visibility, warping, confinement, and locking are semantic window
operations with capability refinement and normalized native errors. Win32
publishes all three movement/grab facilities as supported; AppKit and X11
publish their Winit-supported subsets; Wayland leaves optional
pointer-constraints operations unknown until execution.

The native `winit::Window` remains alive for the full lifetime of the WGPU
surface created from its raw handles. Per-window surfaces and presentation state
remain local, while the renderer may share GPU resources across windows.

OS crates deliberately stay thin. They own only behavior that is truly
platform-specific, such as the Windows crash handler today and future native
menus, lifecycle quirks, or OS services. Winit/AccessKit/WGPU behavior is not
re-abstracted behind another Incular trait when those libraries already provide
the cross-platform contract.
