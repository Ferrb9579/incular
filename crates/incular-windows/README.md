# incular-windows

Windows platform integration for Incular. Window lifecycle, input, display
scaling, menus, accessibility, and Windows-specific resource access will be
implemented here.

The desktop runner installs a process crash handler while an application is
active. If native or dependency code reaches Windows stack-overflow exception
`0xC00000FD` before Incular's frame guards can report it, the handler writes a
minidump to `%LOCALAPPDATA%\Incular\CrashReports` (or
`INCULAR_CRASH_REPORT_DIR`).
