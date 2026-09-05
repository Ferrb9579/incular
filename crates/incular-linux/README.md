# incular-linux

## Architecture and support

| Contract | Status |
| --- | --- |
| Ownership | Linux desktop entry and native services over the shared desktop host. |
| API class | backend; re-exports and path overrides follow the [architecture contract](../../docs/ARCHITECTURE.md). |
| Support | Desktop adapter; Linux runtime behavior requires Linux validation. |

Linux facade over the shared Winit desktop shell. X11 exposes desktop-global
window positions and monitor bounds through the shared adapter. Wayland
deliberately exposes neither top-level global positions nor setters. A reliable
per-monitor X11 usable-area service is not currently installed, so `work_area`
is explicitly unsupported rather than approximated from full monitor bounds.

X11 `Auto` transients use override-redirect windows with the appropriate EWMH
popup/menu/combo/tooltip type hint. Wayland deliberately reports native
transient hosting unsupported with stable Winit 0.30: Incular will not emulate
`xdg_popup` with a coordinate-positioned top-level window, so the semantic
transient remains in the owning overlay until a real xdg-positioner path exists.

Linux adapter for Incular's shared desktop shell.

`incular-desktop` owns the common Winit event loop, WGPU surface/renderer
lifecycle, AccessKit bridge, window-ID mapping, frame scheduling, clipboard,
and normalized keyboard/pointer/touch/IME routing. This crate exposes the Linux
runner facade and is the ownership boundary for future Linux-only services or
lifecycle behavior.

The adapter intentionally does not duplicate or source-include the shared
desktop implementation.
