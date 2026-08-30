//! Retained selection registries and observable selection geometry.
//!
//! Flutter's selection containers are render-tree registrars. Incular keeps
//! the same boundary semantics, but the registrar is represented by a cheap
//! cloneable handle and the retained [`WidgetTree`](crate::internal::WidgetTree)
//! supplies the ordered selectable children. Keeping the mutable part here
//! means a selection listener can observe revisions without coupling the
//! renderer-neutral API to a particular text engine or backend.

use incular_core::{Offset, Rect};
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};

/// The current state of a selection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionStatus {
    /// No selection edges are active.
    #[default]
    None,
    /// Both selection edges are at the same content position.
    Collapsed,
    /// The selection covers a non-empty range.
    Uncollapsed,
}

/// A content-relative selection range, matching Flutter's
/// `SelectedContentRange` contract.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SelectedContentRange {
    pub start_offset: usize,
    pub end_offset: usize,
}

impl SelectedContentRange {
    #[must_use]
    pub const fn new(start_offset: usize, end_offset: usize) -> Self {
        Self {
            start_offset,
            end_offset,
        }
    }

    #[must_use]
    pub const fn is_collapsed(self) -> bool {
        self.start_offset == self.end_offset
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start_offset >= self.end_offset
    }
}

/// Plain-text selected content exposed to clipboard and platform adapters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SelectedContent {
    pub plain_text: String,
}

impl SelectedContent {
    #[must_use]
    pub fn new(plain_text: impl Into<String>) -> Self {
        Self {
            plain_text: plain_text.into(),
        }
    }
}

/// The handle orientation associated with a selection edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionHandleType {
    Left,
    Right,
    Collapsed,
}

/// Geometry for one selection boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionPoint {
    pub local_position: Offset,
    pub line_height: f32,
    pub handle_type: SelectionHandleType,
}

/// Geometry of a retained selection container.
///
/// Positions and rectangles are local to the container, just as Flutter's
/// `SelectionGeometry` is local to its `SelectionHandler`. A container with
/// selectable content but no active selection reports `has_content = true` and
/// `status = SelectionStatus::None`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SelectionGeometry {
    pub start_selection_point: Option<SelectionPoint>,
    pub end_selection_point: Option<SelectionPoint>,
    pub selection_rects: Vec<Rect>,
    pub status: SelectionStatus,
    pub has_content: bool,
}

impl SelectionGeometry {
    #[must_use]
    pub const fn has_selection(&self) -> bool {
        !matches!(self.status, SelectionStatus::None)
    }
}

/// The read-only details delivered by [`SelectionListenerNotifier`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SelectionDetails {
    pub range: Option<SelectedContentRange>,
    pub status: SelectionStatus,
    pub geometry: SelectionGeometry,
}

impl SelectionDetails {
    #[must_use]
    pub const fn has_selection(&self) -> bool {
        !matches!(self.status, SelectionStatus::None)
    }
}

/// Controls which selectable children a container admits.
///
/// Incular currently has one renderer-independent selectable primitive,
/// `SelectableText`. `All` admits every selectable primitive registered by the
/// retained tree; `None` is the disabled-container policy. Keeping this as an
/// explicit policy (instead of a boolean on the widget) leaves the registry
/// boundary available for future selectable primitives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectableChildPolicy {
    #[default]
    All,
    None,
}

impl SelectableChildPolicy {
    #[must_use]
    pub const fn accepts_children(self) -> bool {
        matches!(self, Self::All)
    }
}

#[derive(Default)]
struct SelectionAreaState {
    selected_text: String,
    range: Option<SelectedContentRange>,
    geometry: SelectionGeometry,
    revision: u64,
    registered_child_count: usize,
    next_listener: u64,
    listeners: HashMap<u64, Rc<dyn Fn() + 'static>>,
    next_geometry_listener: u64,
    geometry_listeners: HashMap<u64, Rc<dyn Fn(SelectionGeometry) + 'static>>,
}

/// Observable state shared by a retained selection boundary and its clients.
#[derive(Clone, Default)]
pub struct SelectionAreaController {
    state: Rc<RefCell<SelectionAreaState>>,
}

impl std::fmt::Debug for SelectionAreaController {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SelectionAreaController")
            .field("selected_text", &self.selected_text())
            .field("status", &self.selection_geometry().status)
            .field("revision", &self.revision())
            .field("registered_child_count", &self.registered_child_count())
            .finish()
    }
}

impl PartialEq for SelectionAreaController {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}

impl SelectionAreaController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Text currently selected in this area, suitable for a platform clipboard.
    #[must_use]
    pub fn selected_text(&self) -> String {
        self.state.borrow().selected_text.clone()
    }

    #[must_use]
    pub fn selected_content(&self) -> Option<SelectedContent> {
        let text = self.selected_text();
        (!text.is_empty()).then(|| SelectedContent::new(text))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.state.borrow().selected_text.is_empty()
    }

    /// The range and status currently reported by this container.
    #[must_use]
    pub fn selection(&self) -> SelectionDetails {
        let state = self.state.borrow();
        SelectionDetails {
            range: state.range,
            status: state.geometry.status,
            geometry: state.geometry.clone(),
        }
    }

    #[must_use]
    pub fn selection_geometry(&self) -> SelectionGeometry {
        self.state.borrow().geometry.clone()
    }

    /// Monotonically changes whenever selection text or geometry changes.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.state.borrow().revision
    }

    /// Number of selectable children currently registered with this boundary.
    #[must_use]
    pub fn registered_child_count(&self) -> usize {
        self.state.borrow().registered_child_count
    }

    /// Observes every selection or geometry revision. The returned token is
    /// stable for this controller and can be passed to `remove_listener`.
    pub fn add_listener(&self, listener: impl Fn() + 'static) -> u64 {
        let mut state = self.state.borrow_mut();
        let token = state.next_listener;
        state.next_listener = state.next_listener.wrapping_add(1);
        state.listeners.insert(token, Rc::new(listener));
        token
    }

    pub fn remove_listener(&self, token: u64) -> bool {
        self.state.borrow_mut().listeners.remove(&token).is_some()
    }

    /// Observes geometry changes with a clone of the new local geometry.
    pub fn add_geometry_listener(&self, listener: impl Fn(SelectionGeometry) + 'static) -> u64 {
        let mut state = self.state.borrow_mut();
        let token = state.next_geometry_listener;
        state.next_geometry_listener = state.next_geometry_listener.wrapping_add(1);
        state.geometry_listeners.insert(token, Rc::new(listener));
        token
    }

    pub fn remove_geometry_listener(&self, token: u64) -> bool {
        self.state
            .borrow_mut()
            .geometry_listeners
            .remove(&token)
            .is_some()
    }

    /// Clears the active range while retaining the fact that selectable
    /// content exists in the boundary.
    pub fn clear_selection(&self) {
        let has_content = self.registered_child_count() > 0;
        self.set_selection_snapshot(
            String::new(),
            None,
            SelectionGeometry {
                status: SelectionStatus::None,
                has_content,
                ..SelectionGeometry::default()
            },
        );
    }

    pub(crate) fn set_registered_child_count(&self, count: usize) {
        self.state.borrow_mut().registered_child_count = count;
    }

    /// Updates the complete retained snapshot and notifies observers only
    /// when a meaningful property changed. Callbacks run after the internal
    /// borrow is released so an observer may safely query or mutate the
    /// controller.
    pub(crate) fn set_selection_snapshot(
        &self,
        selected_text: String,
        range: Option<SelectedContentRange>,
        geometry: SelectionGeometry,
    ) {
        let (listeners, geometry_listeners, geometry) = {
            let mut state = self.state.borrow_mut();
            if state.selected_text == selected_text
                && state.range == range
                && state.geometry == geometry
            {
                return;
            }
            state.selected_text = selected_text;
            state.range = range;
            state.geometry = geometry;
            state.revision = state.revision.wrapping_add(1);
            (
                state.listeners.values().cloned().collect::<Vec<_>>(),
                state
                    .geometry_listeners
                    .values()
                    .cloned()
                    .collect::<Vec<_>>(),
                state.geometry.clone(),
            )
        };
        for listener in listeners {
            listener();
        }
        for listener in geometry_listeners {
            listener(geometry.clone());
        }
    }
}

/// The concrete retained delegate used by [`SelectionContainer`].
#[derive(Clone)]
pub struct SelectionContainerDelegate {
    controller: SelectionAreaController,
    policy: SelectableChildPolicy,
}

impl std::fmt::Debug for SelectionContainerDelegate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SelectionContainerDelegate")
            .field("controller", &self.controller)
            .field("policy", &self.policy)
            .finish()
    }
}

impl PartialEq for SelectionContainerDelegate {
    fn eq(&self, other: &Self) -> bool {
        self.policy == other.policy && self.controller == other.controller
    }
}

impl Default for SelectionContainerDelegate {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectionContainerDelegate {
    #[must_use]
    pub fn new() -> Self {
        Self {
            controller: SelectionAreaController::new(),
            policy: SelectableChildPolicy::All,
        }
    }

    #[must_use]
    pub fn with_controller(controller: SelectionAreaController) -> Self {
        Self {
            controller,
            policy: SelectableChildPolicy::All,
        }
    }

    #[must_use]
    pub fn disabled() -> Self {
        Self {
            controller: SelectionAreaController::new(),
            policy: SelectableChildPolicy::None,
        }
    }

    #[must_use]
    pub fn controller(&self) -> SelectionAreaController {
        self.controller.clone()
    }

    #[must_use]
    pub const fn policy(&self) -> SelectableChildPolicy {
        self.policy
    }

    #[must_use]
    pub fn selectable_child_policy(mut self, policy: SelectableChildPolicy) -> Self {
        self.policy = policy;
        if !policy.accepts_children() {
            self.controller.clear_selection();
        }
        self
    }

    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.policy.accepts_children()
    }
}

#[derive(Default)]
struct SelectionNotifierState {
    registered: bool,
    initial_notification_skipped: bool,
    controller_listener: Option<u64>,
    next_listener: u64,
    listeners: HashMap<u64, Rc<dyn Fn() + 'static>>,
}

/// ChangeNotifier-like bridge for selection details.
#[derive(Clone)]
pub struct SelectionListenerNotifier {
    controller: SelectionAreaController,
    state: Rc<RefCell<SelectionNotifierState>>,
}

impl std::fmt::Debug for SelectionListenerNotifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SelectionListenerNotifier")
            .field("registered", &self.registered())
            .field("revision", &self.controller.revision())
            .finish()
    }
}

impl PartialEq for SelectionListenerNotifier {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}

impl Default for SelectionListenerNotifier {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectionListenerNotifier {
    #[must_use]
    pub fn new() -> Self {
        Self {
            controller: SelectionAreaController::new(),
            state: Rc::new(RefCell::new(SelectionNotifierState::default())),
        }
    }

    /// Returns the details, or panics with the same misuse contract as
    /// Flutter's getter when no `SelectionListener` owns this notifier.
    #[must_use]
    pub fn selection(&self) -> SelectionDetails {
        self.try_selection()
            .unwrap_or_else(|| panic!("Selection client has not been registered to this notifier."))
    }

    #[must_use]
    pub fn try_selection(&self) -> Option<SelectionDetails> {
        self.registered().then(|| self.controller.selection())
    }

    #[must_use]
    pub fn registered(&self) -> bool {
        self.state.borrow().registered
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.controller.revision()
    }

    pub fn add_listener(&self, listener: impl Fn() + 'static) -> u64 {
        let mut state = self.state.borrow_mut();
        let token = state.next_listener;
        state.next_listener = state.next_listener.wrapping_add(1);
        state.listeners.insert(token, Rc::new(listener));
        token
    }

    pub fn remove_listener(&self, token: u64) -> bool {
        self.state.borrow_mut().listeners.remove(&token).is_some()
    }

    /// Registers the notifier with its retained selection delegate. The
    /// widget tree invokes this during mount and unregisters during unmount;
    /// it is public as a useful lifecycle hook for custom retained hosts.
    pub fn register(&self) -> bool {
        {
            let mut state = self.state.borrow_mut();
            if state.registered {
                return false;
            }
            state.registered = true;
            state.initial_notification_skipped = false;
        }
        let weak: Weak<RefCell<SelectionNotifierState>> = Rc::downgrade(&self.state);
        let controller = self.controller.clone();
        let token = self.controller.add_listener(move || {
            let Some(state) = weak.upgrade() else {
                return;
            };
            let should_notify = {
                let mut state = state.borrow_mut();
                if state.initial_notification_skipped {
                    true
                } else {
                    state.initial_notification_skipped = true;
                    controller.selection().has_selection()
                }
            };
            if !should_notify {
                return;
            }
            let listeners = state
                .borrow()
                .listeners
                .values()
                .cloned()
                .collect::<Vec<_>>();
            for listener in listeners {
                listener();
            }
        });
        self.state.borrow_mut().controller_listener = Some(token);
        true
    }

    pub fn unregister(&self) -> bool {
        let token = {
            let mut state = self.state.borrow_mut();
            if !state.registered {
                return false;
            }
            state.registered = false;
            state.initial_notification_skipped = false;
            state.controller_listener.take()
        };
        if let Some(token) = token {
            self.controller.remove_listener(token);
        }
        true
    }

    /// Releases the registration and all observer callbacks.
    pub fn dispose(&self) {
        self.unregister();
        self.state.borrow_mut().listeners.clear();
    }

    pub(crate) fn controller(&self) -> SelectionAreaController {
        self.controller.clone()
    }
}
