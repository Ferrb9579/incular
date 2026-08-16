# incular-platform

Owns normalized native events, raw window-handle extraction for backend use,
and the logical/physical DPI boundary. Widgets and runtime receive logical
coordinates only; Linux converts physical pointer positions through
`WindowMetrics` before hit testing.

Keyboard navigation, printable text and IME composition are normalized into
separate `InputEvent` variants. `Clipboard` is the backend boundary; Linux
installs an `arboard` system-clipboard bridge with a safe in-memory fallback.

The scroll convention is positive logical `delta.y` increasing the controller
offset (content moves upward). Winit `LineDelta` is scaled by 40 logical px;
`PixelDelta` is converted through DPI without rounding or inversion, preserving
fractional trackpad movement. Key commands are delivered on pressed events
(including native repeats); control-text payloads such as Backspace are not
forwarded as committed text.
