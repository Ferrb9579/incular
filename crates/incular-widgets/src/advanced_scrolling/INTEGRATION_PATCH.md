# Master-agent integration patch

This focused implementation is intentionally not wired into the retained
tree because the task's write scope excludes `src/lib.rs`, `src/tree.rs`, and
the existing scrolling modules. The algorithms are complete and can be used
directly by the adapter; the following exact patch is the remaining bridge.

1. In `crates/incular-widgets/src/lib.rs`, immediately after `mod scrolling;`,
   add `mod advanced_scrolling;`. Add a `pub use advanced_scrolling::{ ... };`
   block for the public names re-exported by
   `advanced_scrolling/mod.rs`. Add the same names to the hidden bridge in
   `src/internal.rs` if sibling crates need to construct the models.

2. In `src/tree.rs`, add a `WidgetKind` variant for each retained wrapper:

   - `RawScrollbar { controller, style, child }`
   - `ListWheelScrollView { viewport, child }`
   - `DraggableScrollableSheet { sheet, child }`
   - `TwoDimensionalScrollView { view, child }`

   `ListWheelViewport`, `DraggableScrollableActuator`, and
   `TwoDimensionalViewport` are render/state roles rather than additional
   child wrappers, so they should be represented by the corresponding
   render-kind fields (or by a dedicated `WidgetKind` if the public API needs
   them as standalone constructors).

3. Thread those four variants through the existing exhaustive matches in
   `Widget::child`, `Widget::type_`, `Widget::debug`, widget equality,
   reconciliation, layout, painting, semantics, focus, raw-input, and
   interaction. The `RenderKind` additions should hold the focused model and
   the last layout result rather than reimplementing controller state.

4. Add render adapters with these responsibilities:

   - `RawScrollbar`: subscribe to the existing `ScrollController` metrics,
     paint `RawScrollbar::geometry`, route captured pointer motion through
     `pointer_down`/`pointer_move`/`pointer_up`, and preserve the current
     retained overlay scrollbar layer for ordinary `Scroll` nodes.
   - `ListWheelViewport`: call `layout_with_measure` from the render layout
     pass, paint children in `WheelLayout::children` order using each stored
     `WheelMatrix`, and reverse-iterate `WheelLayout::hit_test` before normal
     child hit testing. Do not lower it to `Scroll` because the 3D transform is
     part of hit testing.
   - `DraggableScrollableSheet`: mount the state once, pass
     `inner_controller()` into the builder, call `apply_user_offset` from the
     drag position, and attach `DraggableScrollableActuator` to the nearest
     inherited reset channel. Forward each emitted
     `DraggableScrollableNotification` through the existing depth-aware
     notification pipeline.
   - `TwoDimensionalViewport`: call `layout_with_measure`; use
     `paint_offset`/`paint_extent` for painting and `hit_test` for reverse
     paint-order hit testing. Keep the two controller metrics and sliver
     constraints independent. A single-axis `SliverViewport` adapter is not a
     valid substitute.

5. Add the public constructors in the same files as the existing scrolling
   constructors (`src/scrolling/scroll_views.rs` and
   `src/scrolling/sliver_descriptors.rs`) only after the tree variants exist.
   The focused modules expose all state/configuration needed by those
   constructors, so no new runtime hook is required.

The only unavoidable missing hook is the exhaustive retained-tree dispatch;
no algorithmic placeholder is hidden behind this document.
