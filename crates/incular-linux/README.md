# incular-linux

Linux adapter for Incular's shared desktop shell.

`incular-desktop` owns the common Winit event loop, WGPU surface/renderer
lifecycle, AccessKit bridge, window-ID mapping, frame scheduling, clipboard,
and normalized keyboard/pointer/touch/IME routing. This crate exposes the Linux
runner facade and is the ownership boundary for future Linux-only services or
lifecycle behavior.

The adapter intentionally does not duplicate or source-include the shared
desktop implementation.
