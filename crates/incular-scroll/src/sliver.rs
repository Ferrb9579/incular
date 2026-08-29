//! Renderer-independent sliver protocol values.
//!
//! A sliver is laid out in the viewport's main axis rather than receiving a
//! normal finite box. These values mirror the information exchanged by
//! Flutter's `RenderViewport` and `RenderSliver` without making the scroll
//! crate depend on widget or renderer types.

use incular_config::Axis;

/// The constraints supplied by a viewport to one sliver.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliverConstraints {
    pub axis: Axis,
    pub reverse: bool,
    pub scroll_offset: f32,
    pub preceding_scroll_extent: f32,
    pub overlap: f32,
    pub remaining_paint_extent: f32,
    pub cross_axis_extent: f32,
    pub viewport_main_axis_extent: f32,
    pub remaining_cache_extent: f32,
    pub cache_origin: f32,
}

impl SliverConstraints {
    /// Creates normalized constraints for one sliver.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        axis: Axis,
        reverse: bool,
        scroll_offset: f32,
        preceding_scroll_extent: f32,
        overlap: f32,
        remaining_paint_extent: f32,
        cross_axis_extent: f32,
        viewport_main_axis_extent: f32,
        remaining_cache_extent: f32,
        cache_origin: f32,
    ) -> Self {
        Self {
            axis,
            reverse,
            scroll_offset: finite_non_negative(scroll_offset),
            preceding_scroll_extent: finite_non_negative(preceding_scroll_extent),
            overlap: finite(overlap),
            remaining_paint_extent: finite_non_negative(remaining_paint_extent),
            cross_axis_extent: finite_non_negative(cross_axis_extent),
            viewport_main_axis_extent: finite_non_negative(viewport_main_axis_extent),
            remaining_cache_extent: finite_non_negative(remaining_cache_extent),
            cache_origin: finite(cache_origin),
        }
    }

    /// Returns the visible interval in this sliver's local coordinates.
    #[must_use]
    pub fn visible_interval(self, scroll_extent: f32) -> (f32, f32) {
        let extent = finite_non_negative(scroll_extent);
        let start = self.scroll_offset.min(extent);
        let end = (start + self.remaining_paint_extent).min(extent);
        (start, end.max(start))
    }
}

/// The result returned by a sliver after layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliverGeometry {
    pub scroll_extent: f32,
    pub paint_extent: f32,
    pub layout_extent: f32,
    pub max_paint_extent: f32,
    pub hit_test_extent: f32,
    pub paint_origin: f32,
    pub cache_extent: f32,
    pub visible: bool,
    pub has_visual_overflow: bool,
    /// A sliver can request a correction after a measurement changes the
    /// content before the current anchor.
    pub scroll_offset_correction: Option<f32>,
}

impl SliverGeometry {
    /// An empty geometry result.
    pub const ZERO: Self = Self {
        scroll_extent: 0.,
        paint_extent: 0.,
        layout_extent: 0.,
        max_paint_extent: 0.,
        hit_test_extent: 0.,
        paint_origin: 0.,
        cache_extent: 0.,
        visible: false,
        has_visual_overflow: false,
        scroll_offset_correction: None,
    };

    /// Builds ordinary flowing geometry for a sliver with `scroll_extent`.
    #[must_use]
    pub fn from_scroll_extent(constraints: SliverConstraints, scroll_extent: f32) -> Self {
        let scroll_extent = finite_non_negative(scroll_extent);
        let visible_start = constraints.scroll_offset.min(scroll_extent);
        let paint_extent = (scroll_extent - visible_start)
            .min(constraints.remaining_paint_extent)
            .max(0.);
        let layout_extent = paint_extent.min(scroll_extent).max(0.);
        let cache_start = (constraints.scroll_offset + constraints.cache_origin).max(0.);
        let cache_extent = (scroll_extent - cache_start)
            .min(constraints.remaining_cache_extent.max(0.))
            .max(0.);
        Self {
            scroll_extent,
            paint_extent,
            layout_extent,
            max_paint_extent: scroll_extent,
            hit_test_extent: paint_extent,
            paint_origin: 0.,
            cache_extent,
            visible: paint_extent > 0.,
            has_visual_overflow: visible_start > 0.
                || scroll_extent > constraints.remaining_paint_extent,
            scroll_offset_correction: None,
        }
    }

    /// Returns a copy with bounded, internally consistent values.
    #[must_use]
    pub fn normalized(self) -> Self {
        // A negative/non-finite scroll extent means the producer did not
        // return usable geometry. Treat the whole result as empty instead of
        // allowing an unrelated max-paint value to keep a phantom sliver
        // alive.
        if !self.scroll_extent.is_finite() || self.scroll_extent < 0. {
            return Self {
                scroll_offset_correction: self
                    .scroll_offset_correction
                    .filter(|value| value.is_finite()),
                ..Self::ZERO
            };
        }
        let scroll_extent = finite_non_negative(self.scroll_extent);
        let max_paint_extent = finite_non_negative(self.max_paint_extent).max(scroll_extent);
        let paint_extent = finite_non_negative(self.paint_extent).min(max_paint_extent);
        let layout_extent = finite_non_negative(self.layout_extent).min(paint_extent);
        Self {
            scroll_extent,
            paint_extent,
            layout_extent,
            max_paint_extent,
            hit_test_extent: finite_non_negative(self.hit_test_extent),
            paint_origin: finite(self.paint_origin),
            cache_extent: finite_non_negative(self.cache_extent),
            visible: self.visible && paint_extent > 0.,
            has_visual_overflow: self.has_visual_overflow,
            scroll_offset_correction: self
                .scroll_offset_correction
                .filter(|value| value.is_finite()),
        }
    }
}

fn finite(value: f32) -> f32 {
    if value.is_finite() { value } else { 0. }
}

fn finite_non_negative(value: f32) -> f32 {
    finite(value).max(0.)
}

#[cfg(test)]
mod tests {
    use super::{SliverConstraints, SliverGeometry};
    use incular_config::Axis;

    #[test]
    fn geometry_tracks_visible_and_total_extent() {
        let constraints = SliverConstraints::new(
            Axis::Vertical,
            false,
            25.,
            0.,
            0.,
            100.,
            320.,
            100.,
            150.,
            0.,
        );
        let geometry = SliverGeometry::from_scroll_extent(constraints, 400.);
        assert_eq!(geometry.scroll_extent, 400.);
        assert_eq!(geometry.paint_extent, 100.);
        assert_eq!(geometry.layout_extent, 100.);
        assert!(geometry.visible);
        assert!(geometry.has_visual_overflow);
    }

    #[test]
    fn normalization_prevents_invalid_geometry() {
        let geometry = SliverGeometry {
            scroll_extent: -1.,
            paint_extent: 100.,
            layout_extent: 90.,
            max_paint_extent: 20.,
            hit_test_extent: -3.,
            paint_origin: f32::NAN,
            cache_extent: f32::INFINITY,
            visible: true,
            has_visual_overflow: false,
            scroll_offset_correction: Some(f32::NAN),
        }
        .normalized();
        assert_eq!(geometry.scroll_extent, 0.);
        assert_eq!(geometry.paint_extent, 0.);
        assert_eq!(geometry.layout_extent, 0.);
        assert_eq!(geometry.max_paint_extent, 0.);
        assert_eq!(geometry.hit_test_extent, 0.);
        assert_eq!(geometry.scroll_offset_correction, None);
    }
}
