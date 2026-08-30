/// A read-only snapshot of current scroll geometry and extents.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollMetrics {
    pub pixels: f32,
    pub min_scroll_extent: f32,
    pub max_scroll_extent: f32,
    pub viewport_dimension: f32,
    pub axis: incular_config::Axis,
    pub axis_direction: incular_config::AxisDirection,
    pub device_pixel_ratio: f32,
}

impl ScrollMetrics {
    #[must_use]
    pub fn extent_before(&self) -> f32 {
        (self.pixels - self.min_scroll_extent).max(0.0)
    }

    #[must_use]
    pub fn extent_inside(&self) -> f32 {
        self.viewport_dimension
    }

    #[must_use]
    pub fn extent_after(&self) -> f32 {
        (self.max_scroll_extent - self.pixels).max(0.0)
    }

    #[must_use]
    pub fn extent_total(&self) -> f32 {
        self.max_scroll_extent - self.min_scroll_extent + self.viewport_dimension
    }

    #[must_use]
    pub fn at_edge(&self) -> bool {
        self.pixels <= self.min_scroll_extent || self.pixels >= self.max_scroll_extent
    }

    #[must_use]
    pub fn out_of_range(&self) -> bool {
        self.pixels < self.min_scroll_extent || self.pixels > self.max_scroll_extent
    }
}

use crate::controller::ScrollController;

impl ScrollController {
    /// Returns the current metrics snapshot used by scroll notifications and
    /// accessibility adapters.
    #[must_use]
    pub fn metrics(&self) -> ScrollMetrics {
        let state = self.state.borrow();
        let (axis, reverse) = state
            .notification_context
            .unwrap_or((incular_config::Axis::Vertical, false));
        ScrollMetrics {
            pixels: state.offset,
            min_scroll_extent: 0.,
            max_scroll_extent: state.max_offset,
            viewport_dimension: state.viewport_extent,
            axis,
            axis_direction: match (axis, reverse) {
                (incular_config::Axis::Horizontal, false) => incular_config::AxisDirection::Right,
                (incular_config::Axis::Horizontal, true) => incular_config::AxisDirection::Left,
                (incular_config::Axis::Vertical, false) => incular_config::AxisDirection::Down,
                (incular_config::Axis::Vertical, true) => incular_config::AxisDirection::Up,
            },
            device_pixel_ratio: 1.,
        }
    }

    /// Associates the controller with a viewport's axis and direction. This
    /// does not alter scroll position; it only makes notifications and
    /// metrics describe the physical viewport correctly.
    pub fn set_metrics_context(&self, axis: incular_config::Axis, reverse: bool) {
        self.state.borrow_mut().notification_context = Some((axis, reverse));
    }
}
