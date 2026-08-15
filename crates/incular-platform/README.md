# incular-platform

Owns normalized native events, raw window-handle extraction for backend use,
and the logical/physical DPI boundary. Widgets and runtime receive logical
coordinates only; Linux converts physical pointer positions through
`WindowMetrics` before hit testing.
