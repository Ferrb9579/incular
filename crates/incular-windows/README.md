# incular-windows

Windows facade over the shared Incular desktop shell. In addition to the crash
reporter, this crate supplies the Win32 display work-area service used by
placement APIs. `GetMonitorInfoW`/`MONITORINFO.rcWork` is queried from Winit's
live `HMONITOR`, so centering respects the taskbar and other reserved desktop
space without teaching the shared desktop crate about Win32 handles.

Windows adapter for Incular's shared desktop shell. `incular-desktop` owns the
common Winit/WGPU/AccessKit runner; this crate owns Windows-specific lifecycle
hooks and native services.

Retained transient surfaces are created as owned Win32 top-level windows. They
are initially hidden, then shown with `SW_SHOWNOACTIVATE`; the native adapter
applies and verifies `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW` after Winit's first
visibility transition so custom menus/tooltips stay out of task switching and
cannot steal foreground activation. The shared desktop shell continues to own
their WGPU surface and retained input routing.

Winit destroys Win32 windows asynchronously: `Window::drop` posts its private
destroy message and `WindowEvent::Destroyed` follows the resulting
`DestroyWindow`. The Windows service advertises that lifecycle contract so the
shared shell tracks every dropped parent/transient native ID and delays final
event-loop exit until all destruction acknowledgements arrive. This guarantees
an owned transient cannot be stranded after its Incular owner closes.

The desktop runner installs a process crash handler while an application is
active. If native or dependency code reaches Windows stack-overflow exception
`0xC00000FD` before Incular's frame guards can report it, the handler writes a
minidump to `%LOCALAPPDATA%\Incular\CrashReports` (or
`INCULAR_CRASH_REPORT_DIR`).
