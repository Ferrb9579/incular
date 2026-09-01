# incular-windows

Windows facade over the shared Incular desktop shell. In addition to the crash
reporter, this crate supplies the Win32 display work-area service used by
placement APIs. `GetMonitorInfoW`/`MONITORINFO.rcWork` is queried from Winit's
live `HMONITOR`, so centering respects the taskbar and other reserved desktop
space without teaching the shared desktop crate about Win32 handles.

Windows adapter for Incular's shared desktop shell. `incular-desktop` owns the
common Winit/WGPU/AccessKit runner; this crate owns Windows-specific lifecycle
hooks and native services.

The desktop runner installs a process crash handler while an application is
active. If native or dependency code reaches Windows stack-overflow exception
`0xC00000FD` before Incular's frame guards can report it, the handler writes a
minidump to `%LOCALAPPDATA%\Incular\CrashReports` (or
`INCULAR_CRASH_REPORT_DIR`).
