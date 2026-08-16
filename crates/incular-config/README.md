# incular-config

Renderer-independent configuration values shared across Incular layers.

This crate owns box constraints, physical edge insets, axis and direction
values, and alignment/flex/wrap policy enums. It depends only on
`incular-core` geometry, so layout, widgets, runtime, and platform adapters can
exchange these values without depending on one another. It deliberately owns
no layout algorithms, widget state, renderer state, or platform handles.
