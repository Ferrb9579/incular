//! Widget-independent scroll state, physics, and scrollbar geometry.

use std::{cell::RefCell, rc::Rc};

use incular_core::{Color, Offset, Rect, RestorationKey, RestorationScope, Size};

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
#[derive(Clone, Default)]
struct ScrollState {
    offset: f32,
    max_offset: f32,
    content_extent: f32,
    viewport_extent: f32,
    revision: u64,
    restoration: Option<ScrollRestoration>,
    pending_restored_offset: Option<f32>,
}
#[derive(Clone)]
struct ScrollRestoration {
    scope: RestorationScope,
    key: RestorationKey,
}
impl ScrollController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Creates a scroll controller that restores its position, if present.
    ///
    /// The saved logical offset is applied after the first layout establishes
    /// content bounds. The widgets crate's `PageController` alias gains the
    /// same opt-in behavior.
    #[must_use]
    pub fn restored(scope: RestorationScope, key: RestorationKey) -> Self {
        let controller = Self::new();
        controller.bind_restoration(scope, key);
        controller
    }
    /// Binds this controller's logical offset to a stable restoration value.
    ///
    /// Binding a second scope replaces the previous binding. No value is
    /// written until a position actually changes, so defaults remain defaults
    /// when an application never scrolls the viewport.
    pub fn bind_restoration(&self, scope: RestorationScope, key: RestorationKey) {
        let pending_restored_offset = scope.get_json(&key).and_then(restored_scroll_offset);
        let mut state = self.state.borrow_mut();
        state.restoration = Some(ScrollRestoration { scope, key });
        state.pending_restored_offset = pending_restored_offset;
    }
    /// Stops persisting subsequent position changes without removing the
    /// already-stored restoration value.
    pub fn unbind_restoration(&self) {
        let mut state = self.state.borrow_mut();
        state.restoration = None;
        state.pending_restored_offset = None;
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
        let (restoration, value) = {
            let mut state = self.state.borrow_mut();
            if !offset.is_finite() {
                return false;
            }
            let value = offset.clamp(0., state.max_offset);
            if value == state.offset {
                return false;
            }
            state.offset = value;
            state.pending_restored_offset = None;
            state.revision += 1;
            (state.restoration.clone(), value)
        };
        persist_scroll_offset(restoration, value);
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
        let persistence = {
            let mut state = self.state.borrow_mut();
            state.content_extent = content.max(0.);
            state.viewport_extent = viewport.max(0.);
            state.max_offset = (state.content_extent - state.viewport_extent).max(0.);
            if let Some(restored) = state.pending_restored_offset {
                let next = restored.min(state.max_offset);
                if next != state.offset {
                    state.offset = next;
                    state.revision += 1;
                }
                // Keep a larger requested position pending while asynchronous
                // content grows, instead of overwriting the only snapshot
                // with a temporary short-content clamp.
                if restored <= state.max_offset {
                    state.pending_restored_offset = None;
                }
                None
            } else {
                let next = state.offset.min(state.max_offset);
                if next != state.offset {
                    state.offset = next;
                    state.revision += 1;
                    Some((state.restoration.clone(), next))
                } else {
                    None
                }
            }
        };
        if let Some((restoration, offset)) = persistence {
            persist_scroll_offset(restoration, offset);
        }
    }
}

fn restored_scroll_offset(value: serde_json::Value) -> Option<f32> {
    let offset = value.get("offset")?.as_f64()? as f32;
    (offset.is_finite() && offset >= 0.).then_some(offset)
}

fn persist_scroll_offset(restoration: Option<ScrollRestoration>, offset: f32) {
    if let Some(restoration) = restoration
        && offset.is_finite()
        && offset >= 0.
    {
        restoration
            .scope
            .set_json(&restoration.key, serde_json::json!({ "offset": offset }));
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
    use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

    use serde_json::{Value, json};

    use super::*;

    #[derive(Default)]
    struct MemoryRestorationBackend(RefCell<BTreeMap<Vec<RestorationKey>, Value>>);

    impl incular_core::RestorationBackend for MemoryRestorationBackend {
        fn read_value(&self, path: &[RestorationKey]) -> Option<Value> {
            self.0.borrow().get(path).cloned()
        }

        fn write_value(&self, path: &[RestorationKey], value: Value) {
            self.0.borrow_mut().insert(path.to_vec(), value);
        }

        fn remove_value(&self, path: &[RestorationKey]) {
            self.0.borrow_mut().remove(path);
        }
    }

    fn restoration_key(value: &str) -> RestorationKey {
        RestorationKey::new(value).unwrap()
    }

    fn restoration_scope() -> RestorationScope {
        RestorationScope::root(Rc::new(MemoryRestorationBackend::default()))
            .child_unchecked(restoration_key("window"))
            .child_unchecked(restoration_key("main"))
    }

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

    #[test]
    fn restored_offsets_wait_for_layout_and_survive_temporary_short_content() {
        let scope = restoration_scope();
        let key = restoration_key("sidebar");
        scope.set_json(&key, json!({ "offset": 80. }));

        let controller = ScrollController::restored(scope.clone(), key.clone());
        assert_eq!(controller.offset(), 0.);

        controller.update_extents(40., 20.);
        assert_eq!(controller.offset(), 20.);
        assert_eq!(scope.get_json(&key), Some(json!({ "offset": 80. })));

        controller.update_extents(120., 20.);
        assert_eq!(controller.offset(), 80.);
        assert!(controller.jump_to(50.));
        assert_eq!(scope.get_json(&key), Some(json!({ "offset": 50. })));
    }
}
