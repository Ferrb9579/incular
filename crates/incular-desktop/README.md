# incular-desktop

Shared desktop shell for Incular.

This crate owns the platform-neutral Winit/WGPU/AccessKit integration used by
the Linux, Windows, and macOS adapters: native window lifetime, Incular/native
window ID mapping, frame scheduling, renderer setup, input/IME translation,
clipboard wiring, accessibility projection, and optional DevTools attachment.

The native `winit::Window` remains alive for the full lifetime of the WGPU
surface created from its raw handles. Per-window surfaces and presentation state
remain local, while the renderer may share GPU resources across windows.

OS crates deliberately stay thin. They own only behavior that is truly
platform-specific, such as the Windows crash handler today and future native
menus, lifecycle quirks, or OS services. Winit/AccessKit/WGPU behavior is not
re-abstracted behind another Incular trait when those libraries already provide
the cross-platform contract.
