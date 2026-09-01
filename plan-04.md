# Plan 04 - Pointer, Mouse, and Cursor Completion

## Goal

Complete the ordinary desktop pointer pipeline before adding more complex input facilities.

Current retained input types can represent more metadata than `incular-desktop` currently forwards. Fix the backend so right/middle/extra buttons, device/button metadata, cursor enter/leave, and native cursor changes work end to end.

## Architecture

### One authoritative pointer path

Normalize desktop mouse/touch input into `InputEvent::PointerWithMetadata` where metadata exists.

Do not keep a second long-term mouse-only behavior path. Legacy/simple pointer variants may remain only where required by existing internal compatibility, but runtime routing should converge on the metadata-rich representation.

Define one portable button bit mapping and translate Winit mouse buttons into it, including additional buttons.

### Cursor boundary

`WidgetTree::mouse_cursor_at()` already resolves retained cursor intent. Add a runtime/desktop bridge that:

1. resolves cursor after relevant pointer/tree changes;
2. emits a cursor update only when the effective cursor changes;
3. maps portable `MouseCursor` to native Winit cursor values;
4. falls back deterministically when a specific cursor is unsupported.

Expand the cursor vocabulary only where mature desktop semantics require it, including diagonal resize cursors if custom chrome uses them.

### Enter/leave

Normalize `CursorEntered`/`CursorLeft` into retained hover routing so hover state cannot stick after leaving the native surface.

### Cursor grab/visibility

Expose pointer lock/grab and visibility as explicit window/pointer operations with typed support errors. Do not mix these with ordinary `MouseRegion` cursor styling.

## Crate ownership

- `incular-core`: only portable input/button primitive if not already appropriately located.
- `incular-gestures`: cursor vocabulary and raw pointer semantics.
- `incular-widgets`: retained routing/hover resolution.
- `incular-platform`: native-normalized pointer/cursor operations.
- `incular-desktop`: Winit event and cursor mapping.

## Hard invariants

1. Right/middle/extra buttons are not converted into synthetic left-click behavior.
2. Button state is consistent across down/move/up for a pointer sequence.
3. Cursor leave clears retained hover state deterministically.
4. Cursor updates are deduplicated.
5. Cursor styling does not affect hit testing or gesture arbitration.
6. Cursor grab/lock failure is typed and does not leave internal state claiming success.

## Implementation sequence

1. Finalize portable button mapping.
2. Route all Winit mouse buttons and device metadata.
3. Add cursor enter/leave normalization.
4. Connect retained cursor resolution to Winit `set_cursor`.
5. Add cursor visibility/grab/position operations where portable.
6. Update context-menu/right-click primitives to use the completed path.

## Tests

- Left/right/middle/back/forward/additional button masks.
- Multi-button chord state across movement.
- Hover enters/leaves and cannot stick outside window.
- Nested `MouseRegion` defer/override behavior maps to exactly one native cursor change.
- Custom resize cursors resolve correctly.
- Grab/lock unsupported path returns typed error.

## Acceptance criteria

Ordinary desktop pointer behavior should no longer require a platform-specific workaround in application code.
