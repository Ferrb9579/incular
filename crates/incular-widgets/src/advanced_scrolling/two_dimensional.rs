//! Independent two-axis scrolling and viewport layout.
//!
//! Flutter's two-dimensional viewport has two positions, two viewport
//! dimensions, and a child vicinity `(xIndex, yIndex)`. This implementation
//! preserves those independent axes instead of lowering a grid to a single
//! sliver: row and column extent indexes virtualize separately, cache windows
//! are rectangular, each child receives a two-axis constraint, and hit tests
//! use the same physical offsets as painting.

use std::{collections::BTreeMap, ops::Range, rc::Rc};

use incular_config::{Axis, AxisDirection, Clip, Constraints};
use incular_core::{Offset, Rect, Size};
use incular_scroll::{
    MeasuredExtentIndex, ScrollController, ScrollDelta, ScrollPhysics, SliverConstraints,
};

/// The child identity used by a two-dimensional viewport.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChildVicinity {
    /// Column index.
    pub x_index: usize,
    /// Row index.
    pub y_index: usize,
}

impl ChildVicinity {
    /// Creates a child vicinity.
    #[must_use]
    pub const fn new(x_index: usize, y_index: usize) -> Self {
        Self { x_index, y_index }
    }
}

/// A lazy two-dimensional child source.
pub struct TwoDimensionalChildDelegate<T> {
    row_count: usize,
    column_count: usize,
    builder: Rc<dyn Fn(ChildVicinity) -> Option<T>>,
}

impl<T> Clone for TwoDimensionalChildDelegate<T> {
    fn clone(&self) -> Self {
        Self {
            row_count: self.row_count,
            column_count: self.column_count,
            builder: self.builder.clone(),
        }
    }
}

impl<T> TwoDimensionalChildDelegate<T> {
    /// Creates a finite delegate.
    #[must_use]
    pub fn new(
        row_count: usize,
        column_count: usize,
        builder: impl Fn(ChildVicinity) -> Option<T> + 'static,
    ) -> Self {
        Self {
            row_count,
            column_count,
            builder: Rc::new(builder),
        }
    }

    /// Creates a delegate from a rectangular eager matrix.
    #[must_use]
    pub fn from_rows(rows: Vec<Vec<T>>) -> Self
    where
        T: Clone + 'static,
    {
        let row_count = rows.len();
        let column_count = rows.iter().map(Vec::len).max().unwrap_or(0);
        Self::new(row_count, column_count, move |vicinity| {
            rows.get(vicinity.y_index)
                .and_then(|row| row.get(vicinity.x_index))
                .cloned()
        })
    }

    /// Returns the finite row count.
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.row_count
    }

    /// Returns the finite column count.
    #[must_use]
    pub fn column_count(&self) -> usize {
        self.column_count
    }
}

impl<T: Clone> TwoDimensionalChildDelegate<T> {
    fn child_at(&self, vicinity: ChildVicinity) -> Option<T> {
        if vicinity.y_index >= self.row_count || vicinity.x_index >= self.column_count {
            return None;
        }
        (self.builder)(vicinity)
    }
}

/// How a two-axis cache extent is interpreted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CacheExtentStyle {
    /// Cache this many physical pixels on each side of each axis.
    #[default]
    Pixels,
    /// Cache this many viewport dimensions on each side of each axis.
    Viewport,
}

/// How a diagonal gesture is distributed between the two positions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DiagonalDragBehavior {
    /// Lock to the dominant axis (main-axis wins ties).
    #[default]
    None,
    /// Apply both components independently.
    Free,
    /// Give the dominant component full weight and attenuate the minor one.
    Weighted,
}

/// Result of applying a two-dimensional logical delta.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TwoDimensionalScrollDelta {
    /// Horizontal physics result.
    pub horizontal: ScrollDelta,
    /// Vertical physics result.
    pub vertical: ScrollDelta,
    /// Unconsumed logical components.
    pub unconsumed: Offset,
}

/// The two independent scroll positions and gesture policy.
#[derive(Clone, Debug)]
pub struct TwoDimensionalScrollable {
    horizontal_controller: ScrollController,
    vertical_controller: ScrollController,
    horizontal_physics: ScrollPhysics,
    vertical_physics: ScrollPhysics,
    horizontal_axis_direction: AxisDirection,
    vertical_axis_direction: AxisDirection,
    diagonal_drag_behavior: DiagonalDragBehavior,
    main_axis: Axis,
}

impl TwoDimensionalScrollable {
    /// Creates a two-axis scrollable from independent controllers.
    #[must_use]
    pub fn new(
        horizontal_controller: ScrollController,
        vertical_controller: ScrollController,
    ) -> Self {
        horizontal_controller.set_metrics_context(Axis::Horizontal, false);
        vertical_controller.set_metrics_context(Axis::Vertical, false);
        Self {
            horizontal_controller,
            vertical_controller,
            horizontal_physics: ScrollPhysics::clamping(),
            vertical_physics: ScrollPhysics::clamping(),
            horizontal_axis_direction: AxisDirection::Right,
            vertical_axis_direction: AxisDirection::Down,
            diagonal_drag_behavior: DiagonalDragBehavior::None,
            main_axis: Axis::Vertical,
        }
    }

    /// Creates a scrollable with two fresh controllers.
    #[must_use]
    pub fn with_new_controllers() -> Self {
        Self::new(ScrollController::new(), ScrollController::new())
    }

    /// Returns the horizontal controller.
    #[must_use]
    pub fn horizontal_controller(&self) -> ScrollController {
        self.horizontal_controller.clone()
    }

    /// Returns the vertical controller.
    #[must_use]
    pub fn vertical_controller(&self) -> ScrollController {
        self.vertical_controller.clone()
    }

    /// Configures physics independently for each axis.
    pub fn set_physics(&mut self, horizontal: ScrollPhysics, vertical: ScrollPhysics) {
        self.horizontal_physics = horizontal;
        self.vertical_physics = vertical;
    }

    /// Configures physical directions.
    pub fn set_axis_directions(&mut self, horizontal: AxisDirection, vertical: AxisDirection) {
        assert!(horizontal.axis() == Axis::Horizontal);
        assert!(vertical.axis() == Axis::Vertical);
        self.horizontal_axis_direction = horizontal;
        self.vertical_axis_direction = vertical;
        self.horizontal_controller
            .set_metrics_context(Axis::Horizontal, horizontal.is_reversed());
        self.vertical_controller
            .set_metrics_context(Axis::Vertical, vertical.is_reversed());
    }

    /// Sets the tie-breaking/main axis used by axis locking.
    pub fn set_main_axis(&mut self, main_axis: Axis) {
        self.main_axis = main_axis;
    }

    /// Sets diagonal gesture distribution.
    pub fn set_diagonal_drag_behavior(&mut self, behavior: DiagonalDragBehavior) {
        self.diagonal_drag_behavior = behavior;
    }

    /// Returns the physical directions used by the two positions.
    #[must_use]
    pub fn axis_directions(&self) -> (AxisDirection, AxisDirection) {
        (self.horizontal_axis_direction, self.vertical_axis_direction)
    }

    /// Updates both controller ranges after a viewport layout pass.
    pub fn update_extents(
        &self,
        horizontal_content: f32,
        horizontal_viewport: f32,
        vertical_content: f32,
        vertical_viewport: f32,
    ) {
        self.horizontal_controller.update_extents_with_physics(
            horizontal_content,
            horizontal_viewport,
            self.horizontal_physics,
        );
        self.vertical_controller.update_extents_with_physics(
            vertical_content,
            vertical_viewport,
            self.vertical_physics,
        );
    }

    /// Applies a diagonal logical delta according to the configured behavior.
    #[must_use]
    pub fn apply_delta(&self, delta: Offset) -> TwoDimensionalScrollDelta {
        let directionally_adjusted = Offset::new(
            if self.horizontal_axis_direction.is_reversed() {
                -delta.x
            } else {
                delta.x
            },
            if self.vertical_axis_direction.is_reversed() {
                -delta.y
            } else {
                delta.y
            },
        );
        let (horizontal_delta, vertical_delta) = match self.diagonal_drag_behavior {
            DiagonalDragBehavior::Free => (directionally_adjusted.x, directionally_adjusted.y),
            DiagonalDragBehavior::None => {
                let horizontal_dominant = directionally_adjusted.x.abs()
                    > directionally_adjusted.y.abs()
                    || (directionally_adjusted.x.abs() == directionally_adjusted.y.abs()
                        && self.main_axis == Axis::Horizontal);
                if horizontal_dominant {
                    (directionally_adjusted.x, 0.0)
                } else {
                    (0.0, directionally_adjusted.y)
                }
            }
            DiagonalDragBehavior::Weighted => {
                let horizontal = directionally_adjusted.x.abs();
                let vertical = directionally_adjusted.y.abs();
                let total = horizontal + vertical;
                if total <= f32::EPSILON {
                    (0.0, 0.0)
                } else {
                    let horizontal_weight = 0.5 + horizontal / total * 0.5;
                    let vertical_weight = 0.5 + vertical / total * 0.5;
                    (
                        directionally_adjusted.x * horizontal_weight,
                        directionally_adjusted.y * vertical_weight,
                    )
                }
            }
        };
        let horizontal = self
            .horizontal_controller
            .apply_physics(self.horizontal_physics, horizontal_delta);
        let vertical = self
            .vertical_controller
            .apply_physics(self.vertical_physics, vertical_delta);
        TwoDimensionalScrollDelta {
            horizontal,
            vertical,
            unconsumed: Offset::new(horizontal.unconsumed, vertical.unconsumed),
        }
    }
}

/// Child constraints for one cell of a two-dimensional viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TwoDimensionalConstraints {
    /// Minimum width.
    pub min_width: f32,
    /// Maximum width for this column.
    pub max_width: f32,
    /// Minimum height.
    pub min_height: f32,
    /// Maximum height for this row.
    pub max_height: f32,
}

impl TwoDimensionalConstraints {
    /// Creates loose cell constraints bounded by one row and column extent.
    #[must_use]
    pub fn new(column_extent: f32, row_extent: f32) -> Self {
        let column_extent = column_extent.max(0.0);
        let row_extent = row_extent.max(0.0);
        Self {
            min_width: 0.0,
            max_width: column_extent,
            min_height: 0.0,
            max_height: row_extent,
        }
    }

    /// Converts to the common box constraint type.
    #[must_use]
    pub fn as_box_constraints(self) -> Constraints {
        Constraints::new(
            self.min_width,
            self.max_width,
            self.min_height,
            self.max_height,
        )
    }

    /// Constrains a measured child size.
    #[must_use]
    pub fn constrain(self, size: Size) -> Size {
        self.as_box_constraints().constrain(size)
    }

    /// Returns the largest cell size allowed by this constraint.
    #[must_use]
    pub fn biggest(self) -> Size {
        Size::new(self.max_width, self.max_height)
    }
}

/// A child after layout, with independent logical and physical offsets.
pub struct TwoDimensionalChildLayout<T> {
    /// Child vicinity.
    pub vicinity: ChildVicinity,
    /// Child value.
    pub child: T,
    /// Logical layout offset after subtracting controller offsets.
    pub layout_offset: Offset,
    /// Physical paint origin after axis-direction mapping.
    pub paint_offset: Offset,
    /// Measured child size.
    pub size: Size,
    /// Constraints supplied to the child.
    pub constraints: TwoDimensionalConstraints,
    /// Original physical paint rectangle.
    pub paint_rect: Rect,
    /// Intersection with the viewport clip.
    pub paint_extent: Rect,
    /// Whether the child contributes to paint and hit testing.
    pub visible: bool,
}

/// A two-dimensional viewport layout result.
pub struct TwoDimensionalViewportLayout<T> {
    /// Viewport size.
    pub size: Size,
    /// Logical content size.
    pub content_size: Size,
    /// Independently materialized row range.
    pub row_range: Range<usize>,
    /// Independently materialized column range.
    pub column_range: Range<usize>,
    /// Rectangular cache window in logical viewport coordinates.
    pub cache_rect: Rect,
    /// Children sorted in main-axis paint order.
    pub children: Vec<TwoDimensionalChildLayout<T>>,
    /// Whether any child/content exceeds the viewport.
    pub has_visual_overflow: bool,
    /// Horizontal sliver-compatible constraints.
    pub horizontal_sliver_constraints: SliverConstraints,
    /// Vertical sliver-compatible constraints.
    pub vertical_sliver_constraints: SliverConstraints,
}

impl<T> TwoDimensionalViewportLayout<T> {
    /// Hit-tests visible children from front to back.
    #[must_use]
    pub fn hit_test(&self, point: Offset) -> Option<ChildVicinity> {
        for child in self.children.iter().rev() {
            if child.visible && child.paint_rect.contains(point) {
                return Some(child.vicinity);
            }
        }
        None
    }

    /// Finds a laid-out child by vicinity.
    #[must_use]
    pub fn child(&self, vicinity: ChildVicinity) -> Option<&TwoDimensionalChildLayout<T>> {
        self.children
            .iter()
            .find(|child| child.vicinity == vicinity)
    }
}

/// Independent row/column viewport state and cache.
pub struct TwoDimensionalViewport<T> {
    delegate: TwoDimensionalChildDelegate<T>,
    horizontal_controller: ScrollController,
    vertical_controller: ScrollController,
    horizontal_physics: ScrollPhysics,
    vertical_physics: ScrollPhysics,
    horizontal_axis_direction: AxisDirection,
    vertical_axis_direction: AxisDirection,
    main_axis: Axis,
    cache_extent: f32,
    cache_extent_style: CacheExtentStyle,
    clip_behavior: Clip,
    rows: MeasuredExtentIndex,
    columns: MeasuredExtentIndex,
    cache: BTreeMap<ChildVicinity, T>,
}

impl<T> TwoDimensionalViewport<T> {
    /// Creates a viewport with independent estimated row and column extents.
    #[must_use]
    pub fn new(
        delegate: TwoDimensionalChildDelegate<T>,
        horizontal_controller: ScrollController,
        vertical_controller: ScrollController,
        row_estimate: f32,
        column_estimate: f32,
    ) -> Self {
        assert!(row_estimate.is_finite() && row_estimate > 0.0);
        assert!(column_estimate.is_finite() && column_estimate > 0.0);
        Self {
            rows: MeasuredExtentIndex::new(delegate.row_count(), row_estimate),
            columns: MeasuredExtentIndex::new(delegate.column_count(), column_estimate),
            delegate,
            horizontal_controller,
            vertical_controller,
            horizontal_physics: ScrollPhysics::clamping(),
            vertical_physics: ScrollPhysics::clamping(),
            horizontal_axis_direction: AxisDirection::Right,
            vertical_axis_direction: AxisDirection::Down,
            main_axis: Axis::Vertical,
            cache_extent: 250.0,
            cache_extent_style: CacheExtentStyle::Pixels,
            clip_behavior: Clip::HardEdge,
            cache: BTreeMap::new(),
        }
    }

    /// Configures independent axis physics.
    pub fn set_physics(&mut self, horizontal: ScrollPhysics, vertical: ScrollPhysics) {
        self.horizontal_physics = horizontal;
        self.vertical_physics = vertical;
    }

    /// Configures physical directions.
    pub fn set_axis_directions(&mut self, horizontal: AxisDirection, vertical: AxisDirection) {
        assert!(horizontal.axis() == Axis::Horizontal);
        assert!(vertical.axis() == Axis::Vertical);
        self.horizontal_axis_direction = horizontal;
        self.vertical_axis_direction = vertical;
        self.horizontal_controller
            .set_metrics_context(Axis::Horizontal, horizontal.is_reversed());
        self.vertical_controller
            .set_metrics_context(Axis::Vertical, vertical.is_reversed());
    }

    /// Sets paint ordering's main axis.
    pub fn set_main_axis(&mut self, main_axis: Axis) {
        self.main_axis = main_axis;
    }

    /// Sets the rectangular cache extent.
    pub fn set_cache_extent(&mut self, extent: f32, style: CacheExtentStyle) {
        assert!(extent.is_finite() && extent >= 0.0);
        self.cache_extent = extent;
        self.cache_extent_style = style;
    }

    /// Sets the clip policy returned to the tree paint adapter.
    pub fn set_clip_behavior(&mut self, clip_behavior: Clip) {
        self.clip_behavior = clip_behavior;
    }

    /// Records an exact measured row extent.
    pub fn set_row_extent(&self, index: usize, extent: f32) -> bool {
        self.rows.set_measured_extent(index, extent)
    }

    /// Records an exact measured column extent.
    pub fn set_column_extent(&self, index: usize, extent: f32) -> bool {
        self.columns.set_measured_extent(index, extent)
    }

    /// Returns the row extent index revision.
    #[must_use]
    pub fn row_revision(&self) -> u64 {
        self.rows.revision()
    }

    /// Returns the column extent index revision.
    #[must_use]
    pub fn column_revision(&self) -> u64 {
        self.columns.revision()
    }

    /// Returns a cell's estimated/measured logical offset.
    #[must_use]
    pub fn logical_offset(&self, vicinity: ChildVicinity) -> Offset {
        Offset::new(
            self.columns.offset_for_index(vicinity.x_index) - self.horizontal_controller.offset(),
            self.rows.offset_for_index(vicinity.y_index) - self.vertical_controller.offset(),
        )
    }

    /// Performs a layout pass with each child constrained by its independent
    /// row and column extent.
    pub fn layout(&mut self, size: Size) -> TwoDimensionalViewportLayout<T>
    where
        T: Clone,
    {
        self.layout_with_measure(size, |_, constraints| constraints.biggest())
    }

    /// Performs a layout pass using a renderer-neutral child measurement
    /// callback.
    pub fn layout_with_measure(
        &mut self,
        size: Size,
        measure: impl Fn(&T, TwoDimensionalConstraints) -> Size,
    ) -> TwoDimensionalViewportLayout<T>
    where
        T: Clone,
    {
        self.horizontal_controller.set_metrics_context(
            Axis::Horizontal,
            self.horizontal_axis_direction.is_reversed(),
        );
        self.vertical_controller
            .set_metrics_context(Axis::Vertical, self.vertical_axis_direction.is_reversed());
        self.horizontal_controller.update_extents_with_physics(
            self.columns.total_extent(),
            size.width,
            self.horizontal_physics,
        );
        self.vertical_controller.update_extents_with_physics(
            self.rows.total_extent(),
            size.height,
            self.vertical_physics,
        );

        let (cache_x, cache_y) = self.cache_padding(size);
        let row_range =
            self.rows
                .materialized_range(self.vertical_controller.offset(), size.height, cache_y);
        let column_range = self.columns.materialized_range(
            self.horizontal_controller.offset(),
            size.width,
            cache_x,
        );

        let mut raw_children = Vec::new();
        let mut measured_rows = BTreeMap::<usize, f32>::new();
        let mut measured_columns = BTreeMap::<usize, f32>::new();
        for y_index in row_range.clone() {
            for x_index in column_range.clone() {
                let vicinity = ChildVicinity::new(x_index, y_index);
                if !self.cache.contains_key(&vicinity)
                    && let Some(child) = self.delegate.child_at(vicinity)
                {
                    self.cache.insert(vicinity, child);
                }
                let Some(child) = self.cache.get(&vicinity).cloned() else {
                    continue;
                };
                let row_extent = extent_at(&self.rows, y_index);
                let column_extent = extent_at(&self.columns, x_index);
                let constraints = TwoDimensionalConstraints::new(column_extent, row_extent);
                let child_size = constraints.constrain(measure(&child, constraints));
                measured_rows
                    .entry(y_index)
                    .and_modify(|extent| *extent = extent.max(child_size.height))
                    .or_insert(child_size.height);
                measured_columns
                    .entry(x_index)
                    .and_modify(|extent| *extent = extent.max(child_size.width))
                    .or_insert(child_size.width);
                raw_children.push((vicinity, child, child_size, constraints));
            }
        }
        for (index, extent) in measured_rows {
            let _ = self
                .rows
                .set_measured_extent(index, extent.max(f32::EPSILON));
        }
        for (index, extent) in measured_columns {
            let _ = self
                .columns
                .set_measured_extent(index, extent.max(f32::EPSILON));
        }
        self.horizontal_controller.update_extents_with_physics(
            self.columns.total_extent(),
            size.width,
            self.horizontal_physics,
        );
        self.vertical_controller.update_extents_with_physics(
            self.rows.total_extent(),
            size.height,
            self.vertical_physics,
        );

        let cache_rect = Rect::from_origin_size(
            Offset::new(
                (self.horizontal_controller.offset() - cache_x).max(0.0),
                (self.vertical_controller.offset() - cache_y).max(0.0),
            ),
            Size::new(size.width + cache_x * 2.0, size.height + cache_y * 2.0),
        );
        self.cache.retain(|vicinity, _| {
            row_range.contains(&vicinity.y_index) && column_range.contains(&vicinity.x_index)
        });

        let viewport_rect = Rect::from_origin_size(Offset::ZERO, size);
        let mut children = raw_children
            .into_iter()
            .map(|(vicinity, child, child_size, constraints)| {
                let logical_offset = Offset::new(
                    self.columns.offset_for_index(vicinity.x_index)
                        - self.horizontal_controller.offset(),
                    self.rows.offset_for_index(vicinity.y_index)
                        - self.vertical_controller.offset(),
                );
                let paint_x = match self.horizontal_axis_direction {
                    AxisDirection::Right => logical_offset.x,
                    AxisDirection::Left => size.width - (logical_offset.x + child_size.width),
                    AxisDirection::Down | AxisDirection::Up => logical_offset.x,
                };
                let paint_y = match self.vertical_axis_direction {
                    AxisDirection::Down => logical_offset.y,
                    AxisDirection::Up => size.height - (logical_offset.y + child_size.height),
                    AxisDirection::Left | AxisDirection::Right => logical_offset.y,
                };
                let paint_offset = Offset::new(paint_x, paint_y);
                let paint_rect = Rect::from_origin_size(paint_offset, child_size);
                let paint_extent = paint_rect
                    .intersection(viewport_rect)
                    .unwrap_or_else(|| Rect::from_origin_size(Offset::ZERO, Size::ZERO));
                let visible = match self.clip_behavior {
                    Clip::None => paint_rect.intersects(cache_rect),
                    Clip::HardEdge | Clip::AntiAlias | Clip::AntiAliasWithSaveLayer => {
                        paint_rect.intersects(viewport_rect)
                    }
                };
                TwoDimensionalChildLayout {
                    vicinity,
                    child,
                    layout_offset: logical_offset,
                    paint_offset,
                    size: child_size,
                    constraints,
                    paint_rect,
                    paint_extent,
                    visible,
                }
            })
            .collect::<Vec<_>>();
        children.sort_by(|left, right| match self.main_axis {
            Axis::Horizontal => left
                .vicinity
                .x_index
                .cmp(&right.vicinity.x_index)
                .then(left.vicinity.y_index.cmp(&right.vicinity.y_index)),
            Axis::Vertical => left
                .vicinity
                .y_index
                .cmp(&right.vicinity.y_index)
                .then(left.vicinity.x_index.cmp(&right.vicinity.x_index)),
        });

        let content_size = Size::new(self.columns.total_extent(), self.rows.total_extent());
        let has_visual_overflow =
            content_size.width > size.width || content_size.height > size.height;
        let horizontal_sliver_constraints = SliverConstraints::new(
            Axis::Horizontal,
            self.horizontal_axis_direction.is_reversed(),
            self.horizontal_controller.offset(),
            0.0,
            0.0,
            size.width,
            size.height,
            size.width,
            size.width + cache_x * 2.0,
            -cache_x,
        );
        let vertical_sliver_constraints = SliverConstraints::new(
            Axis::Vertical,
            self.vertical_axis_direction.is_reversed(),
            self.vertical_controller.offset(),
            0.0,
            0.0,
            size.height,
            size.width,
            size.height,
            size.height + cache_y * 2.0,
            -cache_y,
        );
        TwoDimensionalViewportLayout {
            size,
            content_size,
            row_range,
            column_range,
            cache_rect,
            children,
            has_visual_overflow,
            horizontal_sliver_constraints,
            vertical_sliver_constraints,
        }
    }

    /// Scrolls enough to reveal a child in either or both axes. Alignment is
    /// `0` for leading, `0.5` for center, and `1` for trailing.
    pub fn show_in_viewport(
        &self,
        layout: &TwoDimensionalViewportLayout<T>,
        vicinity: ChildVicinity,
        alignment_x: f32,
        alignment_y: f32,
    ) -> bool {
        let Some(child) = layout.child(vicinity) else {
            return false;
        };
        let x = child.layout_offset.x + self.horizontal_controller.offset();
        let y = child.layout_offset.y + self.vertical_controller.offset();
        let target_x =
            x - (layout.size.width - child.size.width).max(0.0) * alignment_x.clamp(0.0, 1.0);
        let target_y =
            y - (layout.size.height - child.size.height).max(0.0) * alignment_y.clamp(0.0, 1.0);
        let changed_x = self.horizontal_controller.jump_to(target_x);
        let changed_y = self.vertical_controller.jump_to(target_y);
        changed_x || changed_y
    }

    fn cache_padding(&self, size: Size) -> (f32, f32) {
        match self.cache_extent_style {
            CacheExtentStyle::Pixels => (self.cache_extent, self.cache_extent),
            CacheExtentStyle::Viewport => (
                self.cache_extent * size.width,
                self.cache_extent * size.height,
            ),
        }
    }
}

/// A two-dimensional scroll view that owns the scrollable and viewport pair.
pub struct TwoDimensionalScrollView<T> {
    scrollable: TwoDimensionalScrollable,
    viewport: TwoDimensionalViewport<T>,
}

impl<T> TwoDimensionalScrollView<T> {
    /// Creates a scroll view with fresh horizontal and vertical controllers.
    #[must_use]
    pub fn new(
        delegate: TwoDimensionalChildDelegate<T>,
        row_estimate: f32,
        column_estimate: f32,
    ) -> Self {
        let scrollable = TwoDimensionalScrollable::with_new_controllers();
        let viewport = TwoDimensionalViewport::new(
            delegate,
            scrollable.horizontal_controller(),
            scrollable.vertical_controller(),
            row_estimate,
            column_estimate,
        );
        Self {
            scrollable,
            viewport,
        }
    }

    /// Returns the scrollable model.
    #[must_use]
    pub fn scrollable(&self) -> &TwoDimensionalScrollable {
        &self.scrollable
    }

    /// Returns the mutable scrollable model.
    pub fn scrollable_mut(&mut self) -> &mut TwoDimensionalScrollable {
        &mut self.scrollable
    }

    /// Returns the viewport model.
    #[must_use]
    pub fn viewport(&self) -> &TwoDimensionalViewport<T> {
        &self.viewport
    }

    /// Returns the mutable viewport model.
    pub fn viewport_mut(&mut self) -> &mut TwoDimensionalViewport<T> {
        &mut self.viewport
    }

    /// Applies one gesture/wheel delta.
    #[must_use]
    pub fn apply_delta(&self, delta: Offset) -> TwoDimensionalScrollDelta {
        self.scrollable.apply_delta(delta)
    }

    /// Lays out the independently virtualized grid.
    pub fn layout(&mut self, size: Size) -> TwoDimensionalViewportLayout<T>
    where
        T: Clone,
    {
        self.viewport.layout(size)
    }
}

fn extent_at(index: &MeasuredExtentIndex, item: usize) -> f32 {
    let start = index.offset_for_index(item);
    let end = if item + 1 < index.len() {
        index.offset_for_index(item + 1)
    } else {
        index.total_extent()
    };
    (end - start).max(f32::EPSILON)
}
