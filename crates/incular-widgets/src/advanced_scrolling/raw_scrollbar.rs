//! `RawScrollbar` geometry and pointer interaction.
//!
//! Flutter's raw scrollbar is an overlay render object rather than a second
//! scroll position. The implementation here keeps that distinction: the
//! existing [`ScrollController`] remains the source of truth and this module
//! only derives a track/thumb pair and maps pointer motion back to its offset.

use incular_config::Axis;
use incular_core::{Color, Offset, Rect, Size};
use incular_scroll::ScrollController;

/// Which edge receives the scrollbar overlay.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RawScrollbarOrientation {
    /// Place a vertical scrollbar on the right edge.
    #[default]
    Right,
    /// Place a vertical scrollbar on the left edge.
    Left,
    /// Place a horizontal scrollbar on the bottom edge.
    Bottom,
    /// Place a horizontal scrollbar on the top edge.
    Top,
}

impl RawScrollbarOrientation {
    fn axis(self) -> Axis {
        match self {
            Self::Left | Self::Right => Axis::Vertical,
            Self::Top | Self::Bottom => Axis::Horizontal,
        }
    }

    fn is_reversed(self, controller: &ScrollController) -> bool {
        let metrics = controller.metrics();
        metrics.axis_direction.is_reversed()
    }
}

/// The renderer-independent style inputs used by [`RawScrollbar`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawScrollbarStyle {
    /// Thickness of the track and thumb in the cross axis.
    pub thickness: f32,
    /// Minimum thumb extent when the content is larger than the viewport.
    pub min_thumb_extent: f32,
    /// Extra minimum used while the controller is out of range.
    pub min_overscroll_thumb_extent: f32,
    /// Empty space inset from both ends of the main axis.
    pub main_axis_margin: f32,
    /// Empty space inset from the overlay edge in the cross axis.
    pub cross_axis_margin: f32,
    /// Track paint color. A transparent track is the default overlay style.
    pub track_color: Color,
    /// Thumb paint color.
    pub thumb_color: Color,
    /// Overlay edge.
    pub orientation: RawScrollbarOrientation,
    /// Whether pointer interaction is enabled.
    pub interactive: bool,
    /// Whether the thumb is allowed to be visible without hover/fade policy.
    pub thumb_visibility: bool,
}

impl Default for RawScrollbarStyle {
    fn default() -> Self {
        Self {
            thickness: 8.0,
            min_thumb_extent: 24.0,
            min_overscroll_thumb_extent: 24.0,
            main_axis_margin: 0.0,
            cross_axis_margin: 0.0,
            track_color: Color::TRANSPARENT,
            thumb_color: Color::rgba(96, 96, 96, 180),
            orientation: RawScrollbarOrientation::Right,
            interactive: true,
            thumb_visibility: true,
        }
    }
}

impl RawScrollbarStyle {
    fn normalized(self) -> Self {
        Self {
            thickness: finite_positive(self.thickness, 1.0),
            min_thumb_extent: finite_positive(self.min_thumb_extent, 1.0),
            min_overscroll_thumb_extent: finite_positive(self.min_overscroll_thumb_extent, 1.0),
            main_axis_margin: finite_non_negative(self.main_axis_margin),
            cross_axis_margin: finite_non_negative(self.cross_axis_margin),
            ..self
        }
    }
}

/// The computed scrollbar geometry for one viewport size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawScrollbarGeometry {
    /// Whether the track/thumb should be painted.
    pub visible: bool,
    /// Track rectangle in viewport coordinates.
    pub track: Rect,
    /// Thumb rectangle in viewport coordinates.
    pub thumb: Rect,
    /// Current maximum logical scroll offset.
    pub max_scroll_extent: f32,
    /// Distance available to move the thumb's leading edge.
    pub thumb_travel: f32,
    /// Content extent reported by the controller.
    pub content_extent: f32,
    /// Viewport extent reported by the controller.
    pub viewport_extent: f32,
}

impl RawScrollbarGeometry {
    /// Returns whether a point is inside the painted thumb.
    #[must_use]
    pub fn thumb_contains(self, point: Offset) -> bool {
        self.visible && self.thumb.contains(point)
    }

    /// Returns whether a point is inside the painted track.
    #[must_use]
    pub fn track_contains(self, point: Offset) -> bool {
        self.visible && self.track.contains(point)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ThumbDrag {
    grab_offset: f32,
}

/// A stateful raw scrollbar overlay attached to an existing scroll controller.
///
/// Pointer methods use viewport coordinates and preserve Flutter's thumb
/// mapping: the pointer's grab point stays under the pointer and logical
/// offset is reversed when the controller's physical axis direction is
/// reversed.
#[derive(Clone, Debug)]
pub struct RawScrollbar {
    controller: ScrollController,
    style: RawScrollbarStyle,
    drag: Option<ThumbDrag>,
}

impl RawScrollbar {
    /// Creates a raw overlay for `controller` using the default style.
    #[must_use]
    pub fn new(controller: ScrollController) -> Self {
        Self {
            controller,
            style: RawScrollbarStyle::default(),
            drag: None,
        }
    }

    /// Returns the attached controller.
    #[must_use]
    pub fn controller(&self) -> ScrollController {
        self.controller.clone()
    }

    /// Returns the current style.
    #[must_use]
    pub fn style(&self) -> RawScrollbarStyle {
        self.style
    }

    /// Replaces the style and drops an active pointer drag.
    pub fn set_style(&mut self, style: RawScrollbarStyle) {
        self.style = style.normalized();
        self.drag = None;
    }

    /// Computes track and thumb rectangles for `viewport_size`.
    #[must_use]
    pub fn geometry(&self, viewport_size: Size) -> RawScrollbarGeometry {
        let style = self.style.normalized();
        let axis = style.orientation.axis();
        let content_extent = self.controller.content_extent().max(0.0);
        let viewport_extent = self.controller.viewport_extent().max(0.0);
        let max_scroll_extent = self.controller.max_offset().max(0.0);
        let main_extent = axis.main_extent(viewport_size);
        let cross_extent = axis.cross_extent(viewport_size);
        let track_main = (main_extent - style.main_axis_margin * 2.0).max(0.0);
        let track_cross = style
            .thickness
            .min((cross_extent - style.cross_axis_margin * 2.0).max(0.0));
        let cross_start = match style.orientation {
            RawScrollbarOrientation::Left | RawScrollbarOrientation::Top => style.cross_axis_margin,
            RawScrollbarOrientation::Right => {
                (cross_extent - style.cross_axis_margin - track_cross).max(0.0)
            }
            RawScrollbarOrientation::Bottom => {
                (cross_extent - style.cross_axis_margin - track_cross).max(0.0)
            }
        };
        let track_origin = axis.offset(style.main_axis_margin, cross_start);
        let track = Rect::from_origin_size(track_origin, axis.size(track_main, track_cross));

        if !style.thumb_visibility
            || content_extent <= viewport_extent
            || viewport_extent <= 0.0
            || track_main <= 0.0
        {
            return RawScrollbarGeometry {
                visible: false,
                track,
                thumb: Rect::from_origin_size(track_origin, Size::ZERO),
                max_scroll_extent,
                thumb_travel: 0.0,
                content_extent,
                viewport_extent,
            };
        }

        let fraction = (viewport_extent / content_extent).clamp(0.0, 1.0);
        let min_extent = if self.controller.metrics().out_of_range() {
            style.min_overscroll_thumb_extent
        } else {
            style.min_thumb_extent
        };
        let thumb_main = (track_main * fraction).max(min_extent.min(track_main));
        let thumb_travel = (track_main - thumb_main).max(0.0);
        let offset_fraction = if max_scroll_extent > 0.0 {
            (self.controller.offset() / max_scroll_extent).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let offset_fraction = if style.orientation.is_reversed(&self.controller) {
            1.0 - offset_fraction
        } else {
            offset_fraction
        };
        let thumb_origin = axis.offset(
            style.main_axis_margin + thumb_travel * offset_fraction,
            cross_start,
        );
        RawScrollbarGeometry {
            visible: true,
            track,
            thumb: Rect::from_origin_size(thumb_origin, axis.size(thumb_main, track_cross)),
            max_scroll_extent,
            thumb_travel,
            content_extent,
            viewport_extent,
        }
    }

    /// Starts a thumb drag or performs a page step on the track.
    pub fn pointer_down(&mut self, viewport_size: Size, point: Offset) -> bool {
        if !self.style.normalized().interactive {
            return false;
        }
        let geometry = self.geometry(viewport_size);
        if geometry.thumb_contains(point) {
            let main = self.style.orientation.axis().main_extent_of(point);
            let thumb_main = self
                .style
                .orientation
                .axis()
                .main_extent_of(geometry.thumb.origin);
            self.drag = Some(ThumbDrag {
                grab_offset: main - thumb_main,
            });
            return true;
        }
        if geometry.track_contains(point) {
            let axis = self.style.orientation.axis();
            let point_main = axis.main_extent_of(point);
            let thumb_start = axis.main_extent_of(geometry.thumb.origin);
            let thumb_extent = axis.main_extent_of(geometry.thumb.size);
            let delta = if point_main < thumb_start {
                -geometry.viewport_extent
            } else if point_main > thumb_start + thumb_extent {
                geometry.viewport_extent
            } else {
                0.0
            };
            if delta != 0.0 {
                let _ = self.controller.scroll_by(delta);
            }
            return true;
        }
        false
    }

    /// Updates a thumb drag and returns whether it moved the controller.
    pub fn pointer_move(&mut self, viewport_size: Size, point: Offset) -> bool {
        let Some(drag) = self.drag else {
            return false;
        };
        let geometry = self.geometry(viewport_size);
        if geometry.thumb_travel <= 0.0 || geometry.max_scroll_extent <= 0.0 {
            return false;
        }
        let axis = self.style.orientation.axis();
        let track_start = axis.main_extent_of(geometry.track.origin);
        let point_main = axis.main_extent_of(point);
        let desired_thumb =
            (point_main - drag.grab_offset - track_start).clamp(0.0, geometry.thumb_travel);
        let mut fraction = desired_thumb / geometry.thumb_travel;
        if self.style.orientation.is_reversed(&self.controller) {
            fraction = 1.0 - fraction;
        }
        self.controller
            .jump_to(geometry.max_scroll_extent * fraction)
    }

    /// Ends the active thumb drag.
    pub fn pointer_up(&mut self) {
        self.drag = None;
    }

    /// Returns whether a thumb drag is active.
    #[must_use]
    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Maps a track coordinate directly into a logical offset.
    pub fn scroll_to_thumb_position(&self, viewport_size: Size, main_position: f32) -> bool {
        let geometry = self.geometry(viewport_size);
        if !geometry.visible || geometry.thumb_travel <= 0.0 {
            return false;
        }
        let axis = self.style.orientation.axis();
        let track_start = axis.main_extent_of(geometry.track.origin);
        let mut fraction =
            (main_position - track_start - axis.main_extent_of(geometry.thumb.size) / 2.0)
                .clamp(0.0, geometry.thumb_travel)
                / geometry.thumb_travel;
        if self.style.orientation.is_reversed(&self.controller) {
            fraction = 1.0 - fraction;
        }
        self.controller
            .jump_to(geometry.max_scroll_extent * fraction)
    }

    /// Returns whether the given pointer is over the interactive thumb.
    #[must_use]
    pub fn hit_test(&self, viewport_size: Size, point: Offset) -> bool {
        self.geometry(viewport_size).thumb_contains(point)
    }
}

trait AxisGeometryExt {
    fn main_extent_of(self, value: impl MainExtentValue) -> f32;
}

trait MainExtentValue {
    fn main_extent(self, axis: Axis) -> f32;
}

impl MainExtentValue for Offset {
    fn main_extent(self, axis: Axis) -> f32 {
        match axis {
            Axis::Horizontal => self.x,
            Axis::Vertical => self.y,
        }
    }
}

impl MainExtentValue for Size {
    fn main_extent(self, axis: Axis) -> f32 {
        axis.main_extent(self)
    }
}

impl AxisGeometryExt for Axis {
    fn main_extent_of(self, value: impl MainExtentValue) -> f32 {
        value.main_extent(self)
    }
}

fn finite_positive(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

fn finite_non_negative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}
