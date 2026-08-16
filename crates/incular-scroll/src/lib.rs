//! Widget-independent scroll state, physics, and scrollbar geometry.

use std::{cell::RefCell, rc::Rc};

use incular_core::{Color, Offset, Rect, Size};

/// Cloneable state for a logical vertical scroll position.
#[derive(Clone, Default)]
pub struct ScrollController {
    state: Rc<RefCell<ScrollState>>,
}
impl std::fmt::Debug for ScrollController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScrollController")
            .field("offset", &self.offset())
            .field("max_offset", &self.max_offset())
            .finish()
    }
}
impl PartialEq for ScrollController {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}
#[derive(Clone, Copy, Debug, Default)]
struct ScrollState {
    offset: f32,
    max_offset: f32,
    content_extent: f32,
    viewport_extent: f32,
    revision: u64,
}
impl ScrollController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn offset(&self) -> f32 {
        self.state.borrow().offset
    }
    #[must_use]
    pub fn max_offset(&self) -> f32 {
        self.state.borrow().max_offset
    }
    #[must_use]
    pub fn content_extent(&self) -> f32 {
        self.state.borrow().content_extent
    }
    #[must_use]
    pub fn viewport_extent(&self) -> f32 {
        self.state.borrow().viewport_extent
    }
    /// Moves the offset after clamping it to the current content bounds.
    pub fn jump_to(&self, offset: f32) -> bool {
        let mut state = self.state.borrow_mut();
        let value = offset.clamp(0., state.max_offset);
        if value == state.offset {
            return false;
        }
        state.offset = value;
        state.revision += 1;
        true
    }
    pub fn scroll_by(&self, delta: f32) -> bool {
        self.jump_to(self.offset() + delta)
    }
    /// Updates content and viewport extents after a viewport layout pass.
    ///
    /// The method is public so independent viewport implementations can share
    /// a controller; applications normally use `jump_to` or `scroll_by`.
    pub fn update_extents(&self, content: f32, viewport: f32) {
        let mut state = self.state.borrow_mut();
        state.content_extent = content.max(0.);
        state.viewport_extent = viewport.max(0.);
        state.max_offset = (state.content_extent - state.viewport_extent).max(0.);
        let next = state.offset.min(state.max_offset);
        if next != state.offset {
            state.offset = next;
            state.revision += 1;
        }
    }
}

/// Visual configuration for a logical vertical overlay scrollbar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollbarStyle {
    pub width: f32,
    pub min_thumb_extent: f32,
    pub track_color: Color,
    pub thumb_color: Color,
}
impl Default for ScrollbarStyle {
    fn default() -> Self {
        Self {
            width: 10.,
            min_thumb_extent: 24.,
            track_color: Color::rgba(20, 22, 30, 120),
            thumb_color: Color::rgba(170, 180, 205, 190),
        }
    }
}

/// Deterministic geometry for an overlay scrollbar.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollbarGeometry {
    pub visible: bool,
    pub track: Rect,
    pub thumb: Rect,
    pub max_scroll_extent: f32,
    pub thumb_travel: f32,
}
impl ScrollbarGeometry {
    #[must_use]
    pub fn thumb_top_for_offset(self, offset: f32) -> f32 {
        let normalized = if self.max_scroll_extent > 0. && offset.is_finite() {
            (offset / self.max_scroll_extent).clamp(0., 1.)
        } else {
            0.
        };
        self.track.origin.y + normalized * self.thumb_travel
    }
    #[must_use]
    pub fn offset_for_thumb_top(self, thumb_top: f32) -> f32 {
        if self.thumb_travel <= 0. || self.max_scroll_extent <= 0. || !thumb_top.is_finite() {
            return 0.;
        }
        ((thumb_top - self.track.origin.y).clamp(0., self.thumb_travel) / self.thumb_travel)
            * self.max_scroll_extent
    }
}

/// Derives display geometry without creating renderer state.
#[must_use]
pub fn scrollbar_geometry(
    size: Size,
    controller: &ScrollController,
    style: ScrollbarStyle,
) -> ScrollbarGeometry {
    let viewport = controller.viewport_extent();
    let content = controller.content_extent();
    if !viewport.is_finite()
        || !content.is_finite()
        || content <= viewport
        || viewport <= 0.
        || !size.height.is_finite()
        || size.height <= 0.
    {
        return ScrollbarGeometry::default();
    }
    let width = style.width.min(size.width).max(0.);
    let track = Rect::from_origin_size(
        Offset::new(size.width - width, 0.),
        Size::new(width, size.height),
    );
    let extent = (size.height * (viewport / content))
        .clamp(style.min_thumb_extent.min(size.height), size.height);
    let mut geometry = ScrollbarGeometry {
        visible: true,
        track,
        thumb: Rect::from_origin_size(Offset::new(track.origin.x, 0.), Size::new(width, extent)),
        max_scroll_extent: controller.max_offset().max(0.),
        thumb_travel: (size.height - extent).max(0.),
    };
    geometry.thumb.origin.y = geometry.thumb_top_for_offset(controller.offset());
    geometry
}

/// The default clamping policy used by native desktop/mobile views.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClampingScrollPhysics;
impl ClampingScrollPhysics {
    #[must_use]
    pub fn apply(self, current: f32, delta: f32, min: f32, max: f32) -> f32 {
        (current + delta).clamp(min.min(max), max.max(min))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extents_clamp_an_existing_offset() {
        let controller = ScrollController::new();
        controller.update_extents(100., 20.);
        assert!(controller.jump_to(80.));
        controller.update_extents(40., 20.);
        assert_eq!(controller.offset(), 20.);
    }
    #[test]
    fn scrollbar_round_trips_offset() {
        let controller = ScrollController::new();
        controller.update_extents(200., 100.);
        controller.jump_to(50.);
        let geometry = scrollbar_geometry(
            Size::new(100., 100.),
            &controller,
            ScrollbarStyle::default(),
        );
        assert!(geometry.visible);
        assert!((geometry.offset_for_thumb_top(geometry.thumb.origin.y) - 50.).abs() < 0.001);
    }
}
