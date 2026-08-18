# incular-scroll

Reusable, widget-independent scroll state for Incular.

The crate owns cloneable logical offset controllers, clamping physics, and
deterministic scrollbar geometry. `incular-widgets` owns `ScrollView`, lazy
viewports, and sliver descriptions, adapting them to these values without a
reverse dependency.

## Opt-in restoration

`ScrollController::restored(scope, key)` and
`ScrollController::bind_restoration(scope, key)` persist only the logical
offset through the runtime-provided restoration scope. A restored offset waits
for layout to establish content bounds, then clamps normally. If initially
loaded content is too short, the larger saved offset remains pending until the
content grows, avoiding a temporary layout overwriting the saved position.
`PageController` in `incular-widgets` is an alias of `ScrollController` and
uses the same API. Persistence is never enabled for an unbound controller.
