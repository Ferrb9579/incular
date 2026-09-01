# Plan 05 - True Native Transient Surface Hosting

## Goal

Implement real transient/native popup surfaces so menus, context menus, combo boxes, tooltips, and popovers can escape the owning top-level window without resizing it.

The retained semantic groundwork already exists through `TransientSurfaceSnapshot`; this plan completes the backend host.

## Architecture

### Surface ownership

Introduce a desktop transient-surface registry keyed by retained `TransientSurfaceId` plus owning `WindowId`.

Each transient host owns:

- native popup/transient window/surface handle;
- WGPU surface configuration;
- current scale/physical size;
- parent/anchor association;
- input/focus/accessibility bridge state as required;
- generation-safe teardown state.

The application/widget tree remains the semantic owner. Native popup lifetime mirrors retained visibility/identity.

### Shared GPU

Use the existing shared GPU context. Do not create a new adapter/device/queue per popup.

Partition rendering so each native surface receives only the display/compositor content assigned to it. Do not render the entire parent scene into every popup.

### Platform semantics

Use real popup/transient relationships:

- Wayland: xdg popup/positioner semantics through a mature supported path; no global-coordinate fake window.
- Windows: owned popup/tool window semantics appropriate to focus/z-order.
- macOS: child/popup panel/window semantics appropriate to transient UI.

If Winit's stable public API remains insufficient, native behavior belongs in `incular-windows`, `incular-macos`, and `incular-linux`; do not pin to an unstable Winit branch merely to avoid correct native adapters.

### Fallback policy

`TransientPresentation::Overlay` always stays in-view.

`Auto` may fall back to in-view overlay only when the semantic role remains usable and the backend explicitly lacks native transient support. The capability/result must remain observable for diagnostics/tests.

Do not introduce a public `Window`/`Surface` promise until the backend can honor it on supported platforms.

## Hard invariants

1. Opening a transient never changes top-level content sizing.
2. Transient native identity follows retained generational identity.
3. Shared GPU device/queue; per-surface swapchain/config only.
4. Closing/unmounting a transient destroys native resources exactly once.
5. Parent close destroys owned transients before/with parent teardown.
6. A transient cannot outlive or reattach to a recycled parent `WindowId`.
7. Rendering outside the parent uses a real native surface, never oversized hidden parent buffers.

## Implementation sequence

1. Define portable transient-host commands/events and capability.
2. Add renderer surface partitioning API without platform policy in `incular-wgpu`.
3. Add desktop transient registry/lifecycle.
4. Implement one platform backend end-to-end, then the remaining supported desktop backends behind the same contract.
5. Route pointer/redraw/scale events to popup surfaces.
6. Attach accessibility bridge per native transient when required by the OS.
7. Preserve overlay fallback for unsupported sessions.

## Tests

- Retained transient open/update/close creates/updates/destroys exactly one host.
- Same transient ID across relayout does not recreate host.
- Parent resize/move does not mutate root content sizing due to popup.
- Multiple simultaneous transients do not share incorrect surface state.
- Popup physical capture dimensions match logical size × scale.
- Parent close tears down all transient surfaces.
- Live tests verify menu can visibly extend outside parent bounds.

## Acceptance criteria

An Incular menu can render outside a small native parent window while the parent retains the same size and identity.
