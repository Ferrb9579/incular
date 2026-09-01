# Plan 13 - Advanced Pen, Trackpad, and Pointer-Device Input

## Goal

Finish device-rich input after the ordinary pointer pipeline in Plan 04 is stable.

Cover pen/stylus metadata and platform gestures such as pinch/rotation/trackpad pressure where meaningful.

## Architecture

### Extend pointer sample metadata deliberately

Add optional device data only when backed by platform sources:

- pressure;
- tilt/azimuth/altitude as a normalized portable representation;
- eraser/inverted stylus;
- barrel/secondary buttons;
- hover distance only if there is a strong cross-platform contract.

Do not put every platform sensor field into a universal struct. Use optional typed metadata/extensions where semantics are genuinely portable.

### Trackpad gestures

Normalize higher-level native gestures separately from raw pointer motion:

- pinch/magnification;
- rotation;
- smart/double-tap magnification where supported;
- trackpad pressure where useful.

Feed appropriate gestures into the gesture arena without manufacturing fake touch contacts when the platform only provides an aggregate gesture.

### Capability reporting

`RuntimeEnvironment.input` should reflect discovered device capability conservatively. Dynamic device arrival/removal may require separate device events rather than a permanent `stylus=true` guess.

## Crate ownership

- `incular-core`/`incular-gestures`: portable device/gesture values.
- `incular-platform`: normalized native events.
- `incular-desktop` and OS/mobile crates: event translation.
- `incular-widgets`: retained listeners/recognizers using the shared gesture system.

## Hard invariants

1. Aggregate trackpad gestures are not fabricated into inaccurate pointer contacts.
2. Pressure/tilt units/ranges are documented and normalized once.
3. Missing hardware metadata remains `None`, never guessed.
4. Existing mouse/touch gesture behavior is unchanged.
5. Platform-specific gesture support is capability-based.

## Tests

- Pressure normalization boundaries.
- Stylus/inverted stylus and button metadata.
- Pinch/rotation phase sequencing.
- Mixed mouse + pen devices do not share pointer identity.
- Unsupported gesture platforms do not emit synthetic events.
- Gesture arena behavior remains deterministic.

## Acceptance criteria

Design/drawing and advanced desktop interaction apps can consume native pen/trackpad information without bypassing Incular's input system.
