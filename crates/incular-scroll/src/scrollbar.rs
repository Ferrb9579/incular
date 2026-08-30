use incular_core::{Color, Offset, Rect, Size};

use crate::controller::ScrollController;

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
            width: 8.,
            min_thumb_extent: 24.,
            track_color: Color::TRANSPARENT,
            thumb_color: Color::rgba(128, 128, 128, 96),
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

impl ScrollController {
    /// Returns the style used when a retained viewport paints its overlay
    /// scrollbar.
    #[must_use]
    pub fn scrollbar_style(&self) -> ScrollbarStyle {
        self.state.borrow().scrollbar_style
    }

    /// Installs the style used by a retained viewport's overlay scrollbar.
    ///
    /// The controller is shared by the viewport and any themed scrollbar
    /// wrapper, so the style is applied before layout/paint without creating
    /// a parallel visual widget tree.
    pub fn set_scrollbar_style(&self, style: ScrollbarStyle) -> bool {
        let mut state = self.state.borrow_mut();
        if state.scrollbar_style == style {
            return false;
        }
        state.scrollbar_style = style;
        state.revision = state.revision.wrapping_add(1);
        true
    }

    /// Whether the thumb is painted even while the pointer is away from the
    /// scrollbar. When false, the retained viewport still owns hit testing so
    /// the thumb reveals on hover and remains draggable.
    #[must_use]
    pub fn scrollbar_thumb_visibility(&self) -> bool {
        self.state.borrow().scrollbar_thumb_visibility
    }

    /// Controls whether the scrollbar thumb is always visible. The default is
    /// hover-reveal, matching compact desktop controls.
    pub fn set_scrollbar_thumb_visibility(&self, visible: bool) -> bool {
        let mut state = self.state.borrow_mut();
        if state.scrollbar_thumb_visibility == visible {
            return false;
        }
        state.scrollbar_thumb_visibility = visible;
        state.revision = state.revision.wrapping_add(1);
        true
    }
}
