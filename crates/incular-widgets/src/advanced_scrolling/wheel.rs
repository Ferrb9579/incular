//! Cylindrical fixed-extent wheel scrolling.
//!
//! This module follows Flutter's `RenderListWheelViewport` coordinate model:
//! children are laid out in a flat, fixed-extent list, painted through a
//! perspective cylindrical projection, and hit-tested by inverting the exact
//! transform used for painting. The controller's logical offset is therefore
//! still useful to sliver and scroll notification code.

use std::{f32::consts::FRAC_PI_2, ops::Range, rc::Rc};

use incular_config::{Axis, Clip, Constraints};
use incular_core::{Offset, Rect, Size};
use incular_scroll::{ScrollController, ScrollPhysics, SliverConstraints};

/// How a fixed-extent wheel reports selection changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChangeReportingBehavior {
    /// Report as soon as the rounded selected index changes.
    #[default]
    OnScrollUpdate,
    /// Report only after [`ListWheelViewport::settle`] snaps to an item.
    OnScrollEnd,
}

/// A child source for a wheel viewport.
pub enum WheelChildDelegate<T> {
    /// A finite, eagerly available child list.
    Children(Vec<T>),
    /// A lazily built source. `None` from the builder terminates an unknown
    /// child range at that index.
    Builder {
        /// Exact count when known; `None` represents a lazy/unknown tail.
        child_count: Option<usize>,
        /// Child builder.
        builder: Rc<dyn Fn(usize) -> Option<T>>,
    },
}

impl<T: Clone> Clone for WheelChildDelegate<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Children(children) => Self::Children(children.clone()),
            Self::Builder {
                child_count,
                builder,
            } => Self::Builder {
                child_count: *child_count,
                builder: builder.clone(),
            },
        }
    }
}

impl<T> WheelChildDelegate<T> {
    /// Creates a finite eager delegate.
    #[must_use]
    pub fn children(children: Vec<T>) -> Self {
        Self::Children(children)
    }

    /// Creates a builder delegate.
    #[must_use]
    pub fn builder(
        child_count: Option<usize>,
        builder: impl Fn(usize) -> Option<T> + 'static,
    ) -> Self {
        Self::Builder {
            child_count,
            builder: Rc::new(builder),
        }
    }

    /// Returns the known child count, if the delegate has one.
    #[must_use]
    pub fn child_count(&self) -> Option<usize> {
        match self {
            Self::Children(children) => Some(children.len()),
            Self::Builder { child_count, .. } => *child_count,
        }
    }
}

impl<T: Clone> WheelChildDelegate<T> {
    fn child_at(&self, index: usize) -> Option<T> {
        match self {
            Self::Children(children) => children.get(index).cloned(),
            Self::Builder { builder, .. } => builder(index),
        }
    }
}

/// A compact column-major 4×4 matrix used by the wheel's perspective layer.
///
/// `WheelMatrix` intentionally lives in the widgets crate because the normal
/// retained tree only needs affine transforms; wheel hit testing needs the
/// perspective transform as well as its inverse.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WheelMatrix {
    /// Column-major matrix entries, matching Flutter's `Matrix4` indexing.
    pub values: [f32; 16],
}

impl WheelMatrix {
    /// Returns the identity matrix.
    #[must_use]
    pub const fn identity() -> Self {
        Self {
            values: [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    /// Multiplies two matrices using column vectors.
    #[must_use]
    pub fn multiply(self, rhs: Self) -> Self {
        let mut result = [0.0; 16];
        for row in 0..4 {
            for column in 0..4 {
                result[column * 4 + row] = (0..4)
                    .map(|index| self.get(row, index) * rhs.get(index, column))
                    .sum();
            }
        }
        Self { values: result }
    }

    /// Returns a translation matrix.
    #[must_use]
    pub fn translation(x: f32, y: f32, z: f32) -> Self {
        let mut result = Self::identity();
        result.values[12] = x;
        result.values[13] = y;
        result.values[14] = z;
        result
    }

    /// Returns an x-axis rotation matrix.
    #[must_use]
    pub fn rotation_x(angle: f32) -> Self {
        let (sin, cos) = angle.sin_cos();
        Self {
            values: [
                1.0, 0.0, 0.0, 0.0, 0.0, cos, sin, 0.0, 0.0, -sin, cos, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    /// Returns a y-axis rotation matrix.
    #[must_use]
    pub fn rotation_y(angle: f32) -> Self {
        let (sin, cos) = angle.sin_cos();
        Self {
            values: [
                cos, 0.0, -sin, 0.0, 0.0, 1.0, 0.0, 0.0, sin, 0.0, cos, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    /// Returns a uniform scale matrix.
    #[must_use]
    pub fn scale(value: f32) -> Self {
        Self {
            values: [
                value, 0.0, 0.0, 0.0, 0.0, value, 0.0, 0.0, 0.0, 0.0, value, 0.0, 0.0, 0.0, 0.0,
                1.0,
            ],
        }
    }

    /// Returns the cylindrical perspective matrix used by Flutter's
    /// `createCylindricalProjectionTransform`.
    #[must_use]
    pub fn cylindrical_projection(
        radius: f32,
        angle: f32,
        perspective: f32,
        horizontal: bool,
    ) -> Self {
        let mut result = Self::identity();
        result.set(3, 2, -perspective);
        result.set(2, 3, -radius);
        result.set(3, 3, perspective * radius + 1.0);
        let rotation = if horizontal {
            Self::rotation_y(angle)
        } else {
            Self::rotation_x(angle)
        };
        result.multiply(rotation.multiply(Self::translation(0.0, 0.0, radius)))
    }

    /// Transforms and perspective-divides a point on the child plane.
    #[must_use]
    pub fn project_point(self, point: Offset) -> Option<WheelProjectedPoint> {
        let x = self.get(0, 0) * point.x + self.get(0, 1) * point.y + self.get(0, 3);
        let y = self.get(1, 0) * point.x + self.get(1, 1) * point.y + self.get(1, 3);
        let w = self.get(3, 0) * point.x + self.get(3, 1) * point.y + self.get(3, 3);
        (w.is_finite() && w.abs() > f32::EPSILON && x.is_finite() && y.is_finite()).then(|| {
            WheelProjectedPoint {
                point: Offset::new(x / w, y / w),
                w,
            }
        })
    }

    /// Transforms a point by the affine inverse path used for hit testing.
    #[must_use]
    pub fn inverse_transform_point(self, point: Offset) -> Option<Offset> {
        let inverse = self.inverse()?;
        let x = inverse.get(0, 0) * point.x + inverse.get(0, 1) * point.y + inverse.get(0, 3);
        let y = inverse.get(1, 0) * point.x + inverse.get(1, 1) * point.y + inverse.get(1, 3);
        let w = inverse.get(3, 0) * point.x + inverse.get(3, 1) * point.y + inverse.get(3, 3);
        (w.is_finite() && w.abs() > f32::EPSILON && x.is_finite() && y.is_finite())
            .then(|| Offset::new(x / w, y / w))
    }

    /// Computes a general 4×4 inverse for a perspective transform.
    #[must_use]
    pub fn inverse(self) -> Option<Self> {
        let mut augmented = [[0.0; 8]; 4];
        for (row, augmented_row) in augmented.iter_mut().enumerate() {
            for (column, value) in augmented_row.iter_mut().take(4).enumerate() {
                *value = self.get(row, column);
            }
            augmented_row[row + 4] = 1.0;
        }
        for pivot in 0..4 {
            let mut pivot_row = pivot;
            for row in (pivot + 1)..4 {
                if augmented[row][pivot].abs() > augmented[pivot_row][pivot].abs() {
                    pivot_row = row;
                }
            }
            if augmented[pivot_row][pivot].abs() <= 1.0e-7 {
                return None;
            }
            augmented.swap(pivot, pivot_row);
            let divisor = augmented[pivot][pivot];
            for value in &mut augmented[pivot] {
                *value /= divisor;
            }
            let normalized_pivot = augmented[pivot];
            for (row, augmented_row) in augmented.iter_mut().enumerate() {
                if row == pivot {
                    continue;
                }
                let factor = augmented_row[pivot];
                for (value, normalized) in augmented_row.iter_mut().zip(normalized_pivot.iter()) {
                    *value -= factor * *normalized;
                }
            }
        }
        let mut values = [0.0; 16];
        for row in 0..4 {
            for column in 0..4 {
                values[column * 4 + row] = augmented[row][column + 4];
            }
        }
        Some(Self { values })
    }

    fn get(self, row: usize, column: usize) -> f32 {
        self.values[column * 4 + row]
    }

    fn set(&mut self, row: usize, column: usize, value: f32) {
        self.values[column * 4 + row] = value;
    }
}

/// A perspective-divided wheel point, retaining its homogeneous `w` value for
/// diagnostics and paint culling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WheelProjectedPoint {
    /// The 2D projected point.
    pub point: Offset,
    /// Homogeneous divisor before perspective division.
    pub w: f32,
}

/// The geometric parameters of a cylindrical wheel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WheelProjection {
    /// Cylinder diameter divided by viewport height.
    pub diameter_ratio: f32,
    /// Perspective entry, constrained to Flutter's safe `0..=0.01` range.
    pub perspective: f32,
    /// Horizontal displacement of the cylinder center as a fraction of width.
    pub off_axis_fraction: f32,
    /// Layout-to-cylinder compression factor.
    pub squeeze: f32,
}

impl Default for WheelProjection {
    fn default() -> Self {
        Self {
            diameter_ratio: 2.0,
            perspective: 0.003,
            off_axis_fraction: 0.0,
            squeeze: 1.0,
        }
    }
}

impl WheelProjection {
    /// Creates validated projection parameters.
    #[must_use]
    pub fn new(
        diameter_ratio: f32,
        perspective: f32,
        off_axis_fraction: f32,
        squeeze: f32,
    ) -> Self {
        assert!(diameter_ratio.is_finite() && diameter_ratio > 0.0);
        assert!(perspective.is_finite() && perspective > 0.0 && perspective <= 0.01);
        assert!(off_axis_fraction.is_finite());
        assert!(squeeze.is_finite() && squeeze > 0.0);
        Self {
            diameter_ratio,
            perspective,
            off_axis_fraction,
            squeeze,
        }
    }

    /// Cylinder radius in viewport coordinates.
    #[must_use]
    pub fn radius(self, size: Size) -> f32 {
        size.height * self.diameter_ratio * 0.5
    }

    /// Maximum visible cylinder angle, matching Flutter's diameter-ratio
    /// branch for narrow cylinders.
    #[must_use]
    pub fn max_visible_radian(self) -> f32 {
        if self.diameter_ratio < 1.0 {
            FRAC_PI_2
        } else {
            (1.0 / self.diameter_ratio).asin()
        }
    }

    /// Leading flat-list margin that centers the first and final items.
    #[must_use]
    pub fn top_scroll_margin(self, size: Size, item_extent: f32) -> f32 {
        -size.height * 0.5 + item_extent * 0.5
    }

    /// Converts a flat painting y-coordinate to a cylinder angle.
    #[must_use]
    pub fn angle_for(self, untransformed_center_y: f32, viewport_height: f32) -> f32 {
        let fractional_y = untransformed_center_y / viewport_height;
        -(fractional_y - 0.5) * 2.0 * self.max_visible_radian() / self.squeeze
    }

    /// Builds the centered cylindrical transform for one child.
    #[must_use]
    pub fn transform_for(
        self,
        size: Size,
        angle: f32,
        use_magnifier: bool,
        magnification: f32,
        centered_item: bool,
    ) -> WheelMatrix {
        let center = Offset::new(
            size.width * 0.5 * (-self.off_axis_fraction * 2.0 + 1.0),
            size.height * 0.5,
        );
        let centered = WheelMatrix::translation(center.x, center.y, 0.0)
            .multiply(WheelMatrix::cylindrical_projection(
                self.radius(size),
                angle,
                self.perspective,
                false,
            ))
            .multiply(WheelMatrix::translation(-center.x, -center.y, 0.0));
        if use_magnifier && centered_item {
            WheelMatrix::translation(center.x, center.y, 0.0)
                .multiply(WheelMatrix::scale(magnification))
                .multiply(WheelMatrix::translation(-center.x, -center.y, 0.0))
                .multiply(centered)
        } else {
            centered
        }
    }
}

/// A laid-out and transformed wheel child.
pub struct WheelChildLayout<T> {
    /// Logical child index.
    pub index: usize,
    /// Child value supplied by the delegate.
    pub child: T,
    /// Flat-list layout origin before scroll and projection.
    pub layout_offset: Offset,
    /// Flat-list rectangle after scroll but before projection.
    pub untransformed_rect: Rect,
    /// Cylindrical angle in radians.
    pub angle: f32,
    /// Exact matrix used for paint and hit testing.
    pub transform: WheelMatrix,
    /// Axis-aligned projected paint bounds.
    pub projected_rect: Rect,
    /// Opacity after over/under-center policy.
    pub opacity: f32,
    /// Whether this child is eligible for paint in the current viewport.
    pub visible: bool,
    /// Whether this is the selected center item.
    pub in_center: bool,
}

/// The result of a wheel layout pass.
pub struct WheelLayout<T> {
    /// Viewport size used by the pass.
    pub size: Size,
    /// Logical target range queried from the delegate.
    pub target_range: Range<usize>,
    /// Children in paint order.
    pub children: Vec<WheelChildLayout<T>>,
    /// Rounded fixed-extent selection.
    pub selected_index: Option<usize>,
    /// Maximum logical scroll offset.
    pub max_scroll_extent: f32,
    /// Sliver-compatible one-axis constraints for integration with the
    /// existing viewport protocol.
    pub sliver_constraints: SliverConstraints,
}

impl<T> WheelLayout<T> {
    /// Hit-tests transformed children from front to back.
    #[must_use]
    pub fn hit_test(&self, point: Offset) -> Option<usize> {
        for child in self.children.iter().rev() {
            if !child.visible {
                continue;
            }
            let Some(local) = child.transform.inverse_transform_point(point) else {
                continue;
            };
            if child.untransformed_rect.contains(local) {
                return Some(child.index);
            }
        }
        None
    }
}

/// Fixed-extent wheel viewport state and layout algorithm.
pub struct ListWheelViewport<T> {
    controller: ScrollController,
    physics: ScrollPhysics,
    item_extent: f32,
    projection: WheelProjection,
    magnification: f32,
    use_magnifier: bool,
    over_under_center_opacity: f32,
    render_children_outside_viewport: bool,
    clip_behavior: Clip,
    delegate: WheelChildDelegate<T>,
    change_reporting_behavior: ChangeReportingBehavior,
    on_selected_item_changed: Option<Rc<dyn Fn(usize)>>,
    last_selected_index: Option<usize>,
}

impl<T> ListWheelViewport<T> {
    /// Creates a wheel with Flutter's default diameter ratio, perspective, and
    /// clamping/fixed-extent physics.
    #[must_use]
    pub fn new(
        controller: ScrollController,
        item_extent: f32,
        delegate: WheelChildDelegate<T>,
    ) -> Self {
        assert!(item_extent.is_finite() && item_extent > 0.0);
        Self {
            controller,
            physics: ScrollPhysics::clamping().fixed_extent_snapping(item_extent),
            item_extent,
            projection: WheelProjection::default(),
            magnification: 1.0,
            use_magnifier: false,
            over_under_center_opacity: 1.0,
            render_children_outside_viewport: false,
            clip_behavior: Clip::HardEdge,
            delegate,
            change_reporting_behavior: ChangeReportingBehavior::OnScrollUpdate,
            on_selected_item_changed: None,
            last_selected_index: None,
        }
    }

    /// Creates a viewport using a newly allocated controller.
    #[must_use]
    pub fn with_new_controller(item_extent: f32, delegate: WheelChildDelegate<T>) -> Self {
        Self::new(ScrollController::new(), item_extent, delegate)
    }

    /// Returns the controller.
    #[must_use]
    pub fn controller(&self) -> ScrollController {
        self.controller.clone()
    }

    /// Sets wheel projection and magnifier properties.
    pub fn set_projection(&mut self, projection: WheelProjection) {
        self.projection = projection;
    }

    /// Sets magnifier behavior.
    pub fn set_magnifier(&mut self, enabled: bool, magnification: f32) {
        assert!(magnification.is_finite() && magnification > 0.0);
        self.use_magnifier = enabled;
        self.magnification = magnification;
    }

    /// Sets opacity for non-center children.
    pub fn set_over_under_center_opacity(&mut self, opacity: f32) {
        assert!((0.0..=1.0).contains(&opacity));
        self.over_under_center_opacity = opacity;
    }

    /// Enables eager outside-viewport child rendering and chooses clipping.
    pub fn set_rendering_policy(&mut self, outside_viewport: bool, clip_behavior: Clip) {
        assert!(!outside_viewport || clip_behavior == Clip::None);
        self.render_children_outside_viewport = outside_viewport;
        self.clip_behavior = clip_behavior;
    }

    /// Sets the physics used for direct scroll deltas and settle operations.
    pub fn set_physics(&mut self, physics: ScrollPhysics) {
        self.physics = physics;
    }

    /// Sets selection callback behavior.
    pub fn set_selection_callback(
        &mut self,
        behavior: ChangeReportingBehavior,
        callback: Option<impl Fn(usize) + 'static>,
    ) {
        self.change_reporting_behavior = behavior;
        self.on_selected_item_changed =
            callback.map(|callback| Rc::new(callback) as Rc<dyn Fn(usize)>);
    }

    /// Returns the currently rounded item index, bounded by a finite delegate.
    #[must_use]
    pub fn selected_item(&self) -> Option<usize> {
        self.selected_item_for_offset(self.controller.offset())
    }

    /// Applies one logical scroll delta through the configured physics.
    pub fn apply_delta(&self, delta: f32) -> incular_scroll::ScrollDelta {
        self.controller.apply_physics(self.physics, delta)
    }

    /// Jumps to an item, retaining a deferred request until the first layout
    /// establishes the scroll range.
    pub fn jump_to_item(&self, index: usize) -> bool {
        let target = index as f32 * self.item_extent;
        if self.controller.max_offset() == 0.0 && target > 0.0 {
            self.controller.deferred_jump_to(target)
        } else {
            self.controller.jump_to(target)
        }
    }

    /// Snaps the controller to the nearest fixed-extent item and reports an
    /// end-of-scroll selection when configured to do so.
    pub fn settle(&mut self) -> Option<usize> {
        let selected = self.selected_item_for_offset(self.controller.offset());
        if let Some(index) = selected {
            let _ = self.controller.jump_to(index as f32 * self.item_extent);
            self.report_selection(index, true);
        }
        selected
    }

    /// Performs a layout pass using full-width children.
    pub fn layout(&mut self, size: Size) -> WheelLayout<T>
    where
        T: Clone,
    {
        self.layout_with_measure(size, |_, constraints| constraints.biggest())
    }

    /// Performs a layout pass with a child measurement callback.
    pub fn layout_with_measure(
        &mut self,
        size: Size,
        measure: impl Fn(&T, Constraints) -> Size,
    ) -> WheelLayout<T>
    where
        T: Clone,
    {
        self.controller.set_metrics_context(Axis::Vertical, false);
        let max_scroll_extent = self.max_scroll_extent();
        // The wheel's centered first/last item makes its controller range equal
        // to `(count - 1) * itemExtent`, so add the viewport extent back when
        // feeding the ordinary content-minus-viewport controller.
        self.controller.update_extents_with_physics(
            max_scroll_extent + size.height,
            size.height,
            self.physics,
        );
        let scroll_offset = self.controller.offset();
        let visible_height = size.height
            * self.projection.squeeze
            * if self.render_children_outside_viewport {
                2.0
            } else {
                1.0
            };
        let first_visible_offset = scroll_offset + self.item_extent * 0.5 - visible_height * 0.5;
        let last_visible_offset = first_visible_offset + visible_height;
        let mut first_index = floor_to_index(first_visible_offset / self.item_extent);
        let mut last_index = floor_to_index(last_visible_offset / self.item_extent);
        if (last_index as f32 * self.item_extent - last_visible_offset).abs() <= 1.0e-5 {
            last_index = last_index.saturating_sub(1);
        }
        let child_count = self.delegate.child_count();
        if let Some(count) = child_count {
            if count == 0 {
                first_index = 0;
                last_index = 0;
            } else {
                first_index = first_index.min(count - 1);
                last_index = last_index.min(count - 1);
            }
        }
        let target_range = if child_count == Some(0) || last_index < first_index {
            first_index..first_index
        } else {
            first_index..last_index.saturating_add(1)
        };
        let top_margin = self.projection.top_scroll_margin(size, self.item_extent);
        let selected_index = self.selected_item_for_offset(scroll_offset);
        let viewport_rect = Rect::from_origin_size(Offset::ZERO, size);
        let mut children = Vec::new();
        for index in target_range.clone() {
            let Some(child) = self.delegate.child_at(index) else {
                break;
            };
            let constraints = Constraints::new(0.0, size.width, self.item_extent, self.item_extent);
            let child_size = constraints.constrain(measure(&child, constraints));
            let layout_offset = Offset::new(
                (size.width - child_size.width).max(0.0) * 0.5,
                index as f32 * self.item_extent,
            );
            let untransformed_y = layout_offset.y - top_margin - scroll_offset;
            let angle = self
                .projection
                .angle_for(untransformed_y + self.item_extent * 0.5, size.height);
            let in_center = selected_index == Some(index);
            let transform = self.projection.transform_for(
                size,
                angle,
                self.use_magnifier,
                self.magnification,
                in_center,
            );
            let untransformed_rect =
                Rect::from_origin_size(Offset::new(layout_offset.x, untransformed_y), child_size);
            let projected_rect =
                projected_rect(transform, child_size, layout_offset.x, untransformed_y)
                    .unwrap_or_else(|| Rect::from_origin_size(Offset::ZERO, Size::ZERO));
            let angle_visible = angle.is_finite() && angle.abs() <= FRAC_PI_2;
            let clipped_to_viewport = match self.clip_behavior {
                Clip::None => {
                    projected_rect.intersects(viewport_rect)
                        || self.render_children_outside_viewport
                }
                Clip::HardEdge | Clip::AntiAlias | Clip::AntiAliasWithSaveLayer => {
                    projected_rect.intersects(viewport_rect)
                }
            };
            let visible =
                angle_visible && (self.render_children_outside_viewport || clipped_to_viewport);
            let opacity = if in_center {
                1.0
            } else {
                self.over_under_center_opacity
            };
            children.push(WheelChildLayout {
                index,
                child,
                layout_offset,
                untransformed_rect,
                angle,
                transform,
                projected_rect,
                opacity,
                visible,
                in_center,
            });
        }
        self.report_selection_if_needed(selected_index);
        WheelLayout {
            size,
            target_range,
            children,
            selected_index,
            max_scroll_extent,
            sliver_constraints: SliverConstraints::new(
                Axis::Vertical,
                false,
                scroll_offset,
                0.0,
                0.0,
                size.height,
                size.width,
                size.height,
                size.height,
                0.0,
            ),
        }
    }

    fn max_scroll_extent(&self) -> f32 {
        match self.delegate.child_count() {
            Some(0) => 0.0,
            Some(count) => count.saturating_sub(1) as f32 * self.item_extent,
            None => f32::MAX * 0.25,
        }
    }

    fn selected_item_for_offset(&self, offset: f32) -> Option<usize> {
        let rounded = (offset.max(0.0) / self.item_extent).round() as usize;
        self.delegate.child_count().map_or(Some(rounded), |count| {
            (count > 0).then(|| rounded.min(count - 1))
        })
    }

    fn report_selection_if_needed(&mut self, selected: Option<usize>) {
        if self.change_reporting_behavior == ChangeReportingBehavior::OnScrollUpdate {
            if let Some(index) = selected {
                self.report_selection(index, false);
            }
        }
    }

    fn report_selection(&mut self, index: usize, force: bool) {
        if !force && self.last_selected_index == Some(index) {
            return;
        }
        self.last_selected_index = Some(index);
        if let Some(callback) = &self.on_selected_item_changed {
            callback(index);
        }
    }
}

/// A convenience wrapper corresponding to Flutter's `ListWheelScrollView`.
pub struct ListWheelScrollView<T> {
    viewport: ListWheelViewport<T>,
}

impl<T> ListWheelScrollView<T> {
    /// Creates a wheel scroll view from an existing controller and delegate.
    #[must_use]
    pub fn new(
        controller: ScrollController,
        item_extent: f32,
        delegate: WheelChildDelegate<T>,
    ) -> Self {
        Self {
            viewport: ListWheelViewport::new(controller, item_extent, delegate),
        }
    }

    /// Returns the viewport model for configuration and layout.
    #[must_use]
    pub fn viewport(&self) -> &ListWheelViewport<T> {
        &self.viewport
    }

    /// Returns the mutable viewport model.
    pub fn viewport_mut(&mut self) -> &mut ListWheelViewport<T> {
        &mut self.viewport
    }

    /// Lays out the wheel.
    pub fn layout(&mut self, size: Size) -> WheelLayout<T>
    where
        T: Clone,
    {
        self.viewport.layout(size)
    }
}

/// A controller that exposes fixed-extent item operations over an ordinary
/// [`ScrollController`].
#[derive(Clone, Debug)]
pub struct FixedExtentScrollController {
    controller: ScrollController,
    initial_item: usize,
}

impl FixedExtentScrollController {
    /// Creates a controller that reports `initial_item` until a wheel extent
    /// is established.
    #[must_use]
    pub fn new(initial_item: usize) -> Self {
        let controller = ScrollController::new();
        Self {
            controller,
            initial_item,
        }
    }

    /// Creates a controller and records an initial item using a known extent.
    #[must_use]
    pub fn with_item_extent(initial_item: usize, item_extent: f32) -> Self {
        assert!(item_extent.is_finite() && item_extent > 0.0);
        let controller = ScrollController::new();
        let _ = controller.deferred_jump_to(initial_item as f32 * item_extent);
        Self {
            controller,
            initial_item,
        }
    }

    /// Returns the wrapped controller.
    #[must_use]
    pub fn controller(&self) -> ScrollController {
        self.controller.clone()
    }

    /// Returns the configured initial item.
    #[must_use]
    pub fn initial_item(&self) -> usize {
        self.initial_item
    }

    /// Returns the rounded selected item for `item_extent`.
    #[must_use]
    pub fn selected_item(&self, item_extent: f32) -> usize {
        if self.controller.content_extent() == 0.0 {
            return self.initial_item;
        }
        (self.controller.offset().max(0.0) / item_extent.max(f32::EPSILON)).round() as usize
    }

    /// Jumps to a fixed-extent item.
    pub fn jump_to_item(&self, index: usize, item_extent: f32) -> bool {
        let target = index as f32 * item_extent;
        if self.controller.max_offset() == 0.0 && target > 0.0 {
            self.controller.deferred_jump_to(target)
        } else {
            self.controller.jump_to(target)
        }
    }
}

fn floor_to_index(value: f32) -> usize {
    if !value.is_finite() || value <= 0.0 {
        0
    } else {
        value.floor() as usize
    }
}

fn projected_rect(transform: WheelMatrix, child_size: Size, x: f32, y: f32) -> Option<Rect> {
    let corners = [
        Offset::new(x, y),
        Offset::new(x + child_size.width, y),
        Offset::new(x, y + child_size.height),
        Offset::new(x + child_size.width, y + child_size.height),
    ];
    let mut projected = corners
        .into_iter()
        .filter_map(|corner| transform.project_point(corner));
    let first = projected.next()?.point;
    let (mut left, mut right, mut top, mut bottom) = (first.x, first.x, first.y, first.y);
    for point in projected.map(|point| point.point) {
        left = left.min(point.x);
        right = right.max(point.x);
        top = top.min(point.y);
        bottom = bottom.max(point.y);
    }
    (left.is_finite() && right.is_finite() && top.is_finite() && bottom.is_finite()).then(|| {
        Rect::from_origin_size(
            Offset::new(left, top),
            Size::new((right - left).max(0.0), (bottom - top).max(0.0)),
        )
    })
}
