# incular-windows

Windows adapter for Incular's shared desktop shell. `incular-desktop` owns the
common Winit/WGPU/AccessKit runner; this crate owns Windows-specific lifecycle
hooks and native services.

The desktop runner installs a process crash handler while an application is
active. If native or dependency code reaches Windows stack-overflow exception
`0xC00000FD` before Incular's frame guards can report it, the handler writes a
minidump to `%LOCALAPPDATA%\Incular\CrashReports` (or
`INCULAR_CRASH_REPORT_DIR`).
