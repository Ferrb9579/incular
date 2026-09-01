# Plan 03 - Displays, Work Areas, and Window Placement

## Goal

Provide a correct display/monitor model and portable window placement API without making false promises on compositors that do not expose global coordinates.

This plan also provides the geometry foundation required by popup placement.

## Architecture

### Display model

Add stable `DisplayId` and immutable display snapshots containing only portable data:

- logical/physical bounds when available;
- work area / usable bounds when available;
- scale factor;
- primary/current association when known;
- display name only as optional diagnostics, not identity.

The API must represent unavailable fields explicitly rather than synthesizing coordinates.

### Window placement capabilities

Separate:

- query current display;
- query outer position;
- set outer position;
- center on display/work area;
- move relative to a known display;
- compositor-owned placement.

On Wayland, global top-level coordinates are generally unavailable. Capability results must say so.

### Coordinate spaces

Document and enforce distinct spaces:

- widget/view logical coordinates;
- window-local physical coordinates;
- display logical coordinates where supported;
- display physical coordinates where supported.

Never overload `Offset` with undocumented coordinate-space meaning in public placement APIs. Use named wrapper types or structs where ambiguity would be dangerous.

## Crate ownership

- `incular-platform`: display/placement contracts and coordinate-space types.
- `incular-runtime`: display snapshots exposed to applications; window/display association.
- `incular-desktop`: Winit monitor discovery and portable position operations.
- OS crates: native work-area/capability corrections when Winit cannot provide them.

## Hard invariants

1. No fabricated global position on Wayland.
2. Display identity is stable for the lifetime of a discovered display generation, not based on array index/name.
3. DPI conversion is explicit at boundaries.
4. Moving a window between displays updates runtime scale/environment through normal metrics events.
5. Popup code later consumes this service; it must not implement its own monitor discovery.

## Implementation sequence

1. Define display/coordinate-space types and capabilities.
2. Add display enumeration/current-display snapshots.
3. Add position query/set where supported.
4. Add work-area and centering helpers as pure policy over capability-backed geometry.
5. Feed display changes through runtime events.
6. Add simulation/memory display provider for deterministic tests.

## Tests

- Mixed-DPI logical/physical conversions.
- Work-area centering excludes taskbar/dock reservations.
- Unsupported global position remains explicitly unavailable.
- Display removal invalidates stale `DisplayId`.
- Moving between displays updates scale without recreating the window.

## Acceptance criteria

Applications can reason about screens and place windows where the OS permits, while Wayland remains correct rather than emulated.
