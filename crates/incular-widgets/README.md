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

## Focused editable text

`TextEditingController` owns a UTF-8 buffer, base/extent selection and active
IME preedit independently of a rebuilt `TextField` description. Edits use
extended grapheme cluster boundaries (`unicode-segmentation`), while public
selection offsets remain valid UTF-8 byte offsets for direct Rust slicing.
`TextField` is intentionally single-line: Enter calls `on_submit` and never
inserts a newline. `TextArea` shares the same controller but shapes a wrapped
paragraph in a fixed viewport; Enter and Shift+Enter replace the selection with
a hard newline, ArrowUp/Down and Home/End use shaped-line geometry, and the
caret keeps itself vertically visible. Selection painting is line-by-line.
Neither control implements bidi visual cursor movement yet.

`ScrollView::vertical` retains a viewport clip and content translation using a
persistent `ScrollController`; wheel changes do not rebuild, relayout, or
repaint unchanged content. `TranslationController` similarly drives a
compositor transform and supports runtime-ticked animation. Hit testing applies
the same scroll/translation coordinate changes as painting.

Scrollable viewports draw a logical-pixel overlay `ScrollbarStyle` by default.
The proportional vertical thumb reads the shared controller's content/viewport
extents, captures pointer drags, and track clicks page by one viewport.
`VirtualList` uses the same path, so a thumb jump computes its destination
offset directly without materializing intermediate rows.

## Lazy fixed-extent viewports

`ScrollView` remains the eager choice for ordinary, arbitrary child trees.
`VirtualList::fixed_extent(item_count, item_extent, builder)` is the lazy
vertical alternative for large indexed data. Its builder runs only as an item
enters the viewport plus a bounded 240 logical-pixel cache before and after it;
it never expands `0..item_count` into Widget values. `VirtualList::builder`
uses a 48 logical-pixel default extent, while
`fixed_extent_with_controller` lets application code retain and `jump_to` a
`ScrollController`.

The fixed path computes content extent as checked/saturating
`item_count * item_extent`, then derives an exclusive range with direct
division: `floor((offset-cache)/extent)..ceil((offset+viewport+cache)/extent)`.
Only intersecting rows are mounted. A row at exactly the cache edge is omitted;
any intersecting row is retained. Existing indices in the next range keep their
Elements, RenderObjects, local pictures, text layouts and layers. Departing
indices unmount (there is deliberately no unsafe state recycling), which drops
their callbacks and reactive subscriptions through the runtime's normal
generational lifetime path.

The virtual viewport owns an outer layout layer, local clip, and inner
`-scroll_offset` content transform, just like `ScrollView`. A scroll inside the
same materialized range updates only that retained transform. Crossing a cache
boundary mounts/unmounts only the changed edge rows and leaves retained rows
unchanged. Runtime diagnostics expose logical count, range, viewport/cache
sizes, and live Element/RenderObject/PictureLayer counts for debugging.

Fixed-extent virtualization is implemented. Variable measured extents,
estimated extent caches, grids, sticky headers, and keep-alive policies remain
future work. Future semantics can expose logical child count and materialized
item indices without creating semantic nodes for every logical row.

## Coordinate spaces and transform composition

Visual render objects cache local picture commands. Each object has an outer
retained transform for its parent-derived layout offset. Scroll and translation
widgets place a second, inner transform below that placement: it holds only
`-scroll_offset` or animation displacement. This keeps normal layout, retained
picture reuse, clipping, and hit testing in the same coordinate model without
turning compositor updates into repaint work.
