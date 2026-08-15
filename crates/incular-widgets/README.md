# incular-widgets

Widget contracts, the widget tree, built-in components, layout integration,
build context, lifecycle, and future state handling for Incular. Components
such as buttons, text fields, lists, and containers belong in this crate.
# incular-widgets

Owns declarative built-in `Widget` descriptions and the persistent
`WidgetTree`. Elements and render objects reside in separate generational
arenas. Updating an element reconciles only its direct children: matching
prefix/suffix are reused, and a keyed lookup is created only for a changed
middle range that contains keys. Layout runs constraints down and sizes up;
paint caches are regenerated only for paint-dirty render objects.

This crate depends on core, layout, and painting; it owns neither scheduling,
native input loops, nor GPU resources.

The current built-ins include a target-oriented `button` with an `ActionId`.
Runtime resolves it through persistent render hit testing; widgets never see
raw OS events.

`ScrollView::vertical` retains a viewport clip and content translation using a
persistent `ScrollController`; wheel changes do not rebuild, relayout, or
repaint unchanged content. `TranslationController` similarly drives a
compositor transform and supports runtime-ticked animation. Hit testing applies
the same scroll/translation coordinate changes as painting.

## Coordinate spaces and transform composition

Visual render objects cache local picture commands. Each object has an outer
retained transform for its parent-derived layout offset. Scroll and translation
widgets place a second, inner transform below that placement: it holds only
`-scroll_offset` or animation displacement. This keeps normal layout, retained
picture reuse, clipping, and hit testing in the same coordinate model without
turning compositor updates into repaint work.
