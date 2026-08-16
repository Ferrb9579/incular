# incular-scroll

Reusable, widget-independent scroll state for Incular.

The crate owns cloneable logical offset controllers, clamping physics, and
deterministic scrollbar geometry. `incular-widgets` owns `ScrollView`, lazy
viewports, and sliver descriptions, adapting them to these values without a
reverse dependency.
