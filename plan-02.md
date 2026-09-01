# Plan 02 - Window Control and Custom Chrome

## Goal

Make Incular capable of building professional decorated or fully custom-chrome desktop windows without application-side native hacks.

This includes application/window dragging, native edge/corner resize dragging, minimize/maximize/restore/fullscreen, resizable/decorations/constraints changes, window level, icon, and user attention.

## Architecture

Extend the semantic `WindowOperation`/`WindowHandle` boundary rather than exposing Winit.

### Interactive compositor-owned operations

Add semantic operations equivalent to:

- begin native move drag;
- begin native resize drag with portable `ResizeDirection`.

The desktop backend uses Winit's `drag_window()` and `drag_resize_window()`.

Never implement move/resize by reading the mouse and repeatedly setting desktop coordinates. That breaks snapping, compositor policy, Wayland, DPI changes, and accessibility.

### Runtime window state

Add explicit operations/state for:

- minimized;
- maximized/restored;
- fullscreen;
- resizable;
- decorations;
- minimum/maximum logical size;
- window level (`Normal`, `AlwaysOnTop`, and only other levels with portable semantics);
- window icon where supported;
- request/cancel user attention.

Treat observed native state as authoritative when the platform reports it. Requested state and observed state must not be conflated.

### Widget integration

Add a neutral `WindowDragRegion` or equivalent widget-level descriptor only after the raw window operation exists.

It should:

- initiate native drag on a valid primary-button press;
- not steal clicks from interactive descendants;
- be composable into custom titlebars;
- have correct semantics/hit testing;
- never own platform handles.

Custom resize handles should use the same semantic native resize operation, not manual geometry.

## Crate ownership

- `incular-platform`: portable window state, operation enums, errors.
- `incular-runtime`: `WindowHandle`, requested/observed state, completion.
- `incular-desktop`: Winit mappings.
- `incular-widgets`: neutral drag/resize region descriptors if needed.
- OS crates only for capabilities Winit does not correctly/fully expose.

## Hard invariants

1. Interactive move/resize is always delegated to the native compositor/window manager.
2. Custom chrome works with `decorations: false` without requiring direct native handles.
3. Wayland limitations are not hidden.
4. Same Incular `WindowId` survives state changes.
5. Programmatic size changes keep the existing synchronous/async resize correctness path.
6. User-resizable policy, content-driven sizing, and programmatic resize remain independent concepts.
7. No widget directly mutates a Winit window.

## Implementation sequence

1. Add portable resize direction/window level/attention values.
2. Add begin-move and begin-resize operations with typed result completion.
3. Add minimize/maximize/fullscreen/runtime policy operations.
4. Add runtime observed-state events where Winit/native APIs expose them.
5. Add window icon and attention support.
6. Implement neutral custom-chrome interaction widgets.
7. Add examples for decorated and undecorated windows.
8. Remove any application-specific workaround code introduced before these APIs existed.

## Tests

- Command normalization/unit tests for every operation.
- Custom drag region does not intercept a child button.
- Resize regions map every edge/corner correctly.
- Same `WindowId` across maximize/restore/fullscreen and move/resize drag.
- Content-sized windows still resize correctly after custom-chrome operations.
- Unsupported operations produce typed failures.
- Opt-in live native tests exercise move/resize drag initiation and common state transitions on each available desktop OS.

## Acceptance criteria

An undecorated application can implement a complete custom titlebar using only public Incular APIs, with native move/resize behavior and normal OS snapping/window management.
