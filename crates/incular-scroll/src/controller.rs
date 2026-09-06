use std::{cell::RefCell, rc::Rc};

use incular_core::{RestorationKey, RestorationScope};

use crate::{
    notifications::{ScrollNotificationListener, ScrollNotificationType},
    restoration::{ScrollRestoration, persist_scroll_offset, restored_scroll_offset},
    scrollbar::ScrollbarStyle,
};

/// Cloneable state for a logical vertical scroll position.
#[derive(Clone)]
pub struct ScrollController {
    pub(crate) changes: incular_core::reactivity::DependencySource,
    pub(crate) state: Rc<RefCell<ScrollState>>,
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

impl Default for ScrollController {
    fn default() -> Self {
        // Raw scroll views retain their historical always-visible overlay
        // behavior. The themed Controls wrapper opts into hover reveal when it
        // is built, without changing existing applications that only use the
        // core scroll API.
        let state = ScrollState {
            scrollbar_thumb_visibility: true,
            ..ScrollState::default()
        };
        Self {
            state: Rc::new(RefCell::new(state)),
            changes: incular_core::reactivity::DependencySource::default(),
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct ScrollState {
    pub(crate) offset: f32,
    pub(crate) max_offset: f32,
    pub(crate) content_extent: f32,
    pub(crate) viewport_extent: f32,
    /// The retained visual contract for an overlay scrollbar attached to this
    /// position. Keeping it with the shared controller means a themed
    /// scrollbar and the viewport always observe the same scroll state.
    pub(crate) scrollbar_style: ScrollbarStyle,
    pub(crate) scrollbar_thumb_visibility: bool,
    pub(crate) revision: u64,
    pub(crate) restoration: Option<ScrollRestoration>,
    pub(crate) pending_restored_offset: Option<f32>,
    pub(crate) pending_jump_offset: Option<f32>,
    pub(crate) notification_listeners: Rc<RefCell<Vec<(u64, ScrollNotificationListener)>>>,
    pub(crate) next_notification_listener: u64,
    pub(crate) notification_context: Option<(incular_config::Axis, bool)>,
    pub(crate) activity_active: bool,
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
        self.changes.track();
        self.state.borrow().offset
    }
    #[must_use]
    pub fn max_offset(&self) -> f32 {
        self.changes.track();
        self.state.borrow().max_offset
    }
    #[must_use]
    pub fn content_extent(&self) -> f32 {
        self.changes.track();
        self.state.borrow().content_extent
    }
    #[must_use]
    pub fn viewport_extent(&self) -> f32 {
        self.changes.track();
        self.state.borrow().viewport_extent
    }
}

impl ScrollController {
    /// Monotonic state revision for retained viewports. Layout owners use this
    /// to refresh sliver cache windows and pinned placements without requiring
    /// the application to rebuild its widget description.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.state.borrow().revision
    }
}

impl ScrollController {
    /// Moves the offset after clamping it to the current content bounds.
    pub fn jump_to(&self, offset: f32) -> bool {
        let (restoration, value, delta) = {
            let mut state = self.state.borrow_mut();
            if !offset.is_finite() {
                return false;
            }
            let value = offset.clamp(0., state.max_offset);
            if value == state.offset {
                return false;
            }
            let previous = state.offset;
            state.offset = value;
            state.pending_restored_offset = None;
            state.revision += 1;
            (state.restoration.clone(), value, value - previous)
        };
        persist_scroll_offset(restoration, value);
        self.dispatch_notification(ScrollNotificationType::Update, delta, 0.);
        true
    }

    /// Requests a programmatic position before the viewport has established
    /// its extents.  This is useful for retained page/tab controllers: the
    /// request is applied on the first layout pass instead of being clamped
    /// away when `max_offset` is still zero.
    pub fn deferred_jump_to(&self, offset: f32) -> bool {
        if !offset.is_finite() {
            return false;
        }
        let mut state = self.state.borrow_mut();
        let offset = offset.max(0.0);
        state.pending_jump_offset = Some(offset);
        state.revision = state.revision.wrapping_add(1);
        true
    }
    pub fn scroll_by(&self, delta: f32) -> bool {
        self.jump_to(self.offset() + delta)
    }
}
