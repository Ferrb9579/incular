# incular-scroll

## Architecture and support

| Contract | Status |
| --- | --- |
| Ownership | Scroll controllers, physics, metrics, extent indexing and geometry. |
| API class | application; re-exports and path overrides follow the [architecture contract](../../docs/ARCHITECTURE.md). |
| Support | Available; lifecycle and interruption contracts consolidate in G. |

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

## Variable-extent indexes

`MeasuredExtentIndex` is the measured-prefix structure used internally by
retained sliver render objects such as `SliverList`. It starts every row at a
supplied estimate, records exact extents only after a row is laid out, and
provides offset-to-index, index-to-offset, total-estimate, and viewport-range
queries without walking preceding rows. The chunked Fenwick index keeps deep
seeks bounded even for million-row data sets.

Keep an index outside rebuilt widget descriptions when data can change, then
use `set_measured_extent`, `invalidate_extent`, `insert`, `remove`, or
`move_item`. Structural changes rematerialize only the visible cache window;
post-layout measurements retain visible item identity and compensate the
viewport anchor when preceding rows change size.

## Policies and nested ownership

`ScrollPhysics` composes `Scrollability`, `BoundaryPhysics`, and `SnapPhysics`
instead of mirroring a class hierarchy. Start with `ScrollPhysics::clamping()`,
then opt into `always_scrollable`, `never_scrollable`, `bouncing`, page snap,
or fixed-extent snap. Bouncing is finite and resistant; applications advance
its `spring_step` with monotonic frame elapsed time and apply the result to a
controller, so policy code never sleeps or owns an executor.

`NestedScrollCoordinator` accepts controllers innermost-first. It applies a
wheel, drag, or momentum delta to one controller, then transfers only a
clamped boundary remainder to the next controller. This prevents the same
delta from moving both inner and outer viewports.
