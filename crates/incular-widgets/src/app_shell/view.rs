//! Independent view ownership and anchored auxiliary-view coordination.
//!
//! A [`ViewController`] is the widget-layer ownership boundary for one
//! platform view.  It retains metrics, environment, focus, lifecycle, and
//! observers even when a parent widget is rebuilt.  Native window creation is
//! supplied through [`AuxiliaryViewHost`], keeping the widgets crate free of a
//! dependency cycle on `incular-runtime`.

use super::Widget;
use incular_config::RuntimeEnvironment;
use incular_core::{Rect, Size};
use std::{
    cell::{Cell, RefCell},
    fmt,
    rc::{Rc, Weak},
};

/// Stable identity for a platform view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewId(u64);

impl ViewId {
    /// The primary application view identity.
    pub const PRIMARY: Self = Self(0);

    /// Creates an identity from a runtime-assigned number.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric identity used by platform bridges.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Alias for [`Self::PRIMARY`].
    #[must_use]
    pub const fn primary() -> Self {
        Self::PRIMARY
    }
}

/// Physical view dimensions and scale factor published by a platform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewMetrics {
    physical_width: u32,
    physical_height: u32,
    scale_factor: f64,
}

impl Default for ViewMetrics {
    fn default() -> Self {
        Self::new(0, 0, 1.0)
    }
}

impl ViewMetrics {
    /// Creates normalized physical metrics.
    #[must_use]
    pub fn new(physical_width: u32, physical_height: u32, scale_factor: f64) -> Self {
        Self {
            physical_width,
            physical_height,
            scale_factor: if scale_factor.is_finite() && scale_factor > 0.0 {
                scale_factor
            } else {
                1.0
            },
        }
    }

    #[must_use]
    pub const fn physical_width(self) -> u32 {
        self.physical_width
    }

    #[must_use]
    pub const fn physical_height(self) -> u32 {
        self.physical_height
    }

    #[must_use]
    pub const fn scale_factor(self) -> f64 {
        self.scale_factor
    }

    /// Converts physical dimensions to logical layout dimensions.
    #[must_use]
    pub fn logical_size(self) -> Size {
        Size::new(
            self.physical_width as f32 / self.scale_factor as f32,
            self.physical_height as f32 / self.scale_factor as f32,
        )
    }

    #[must_use]
    pub fn logical_width(self) -> f32 {
        self.logical_size().width
    }

    #[must_use]
    pub fn logical_height(self) -> f32 {
        self.logical_size().height
    }
}

/// Lifecycle state retained for one view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewLifecycle {
    #[default]
    Creating,
    Visible,
    Hidden,
    Focused,
    Unfocused,
    Closing,
    Closed,
}

impl ViewLifecycle {
    #[must_use]
    pub const fn is_closed(self) -> bool {
        matches!(self, Self::Closing | Self::Closed)
    }

    #[must_use]
    pub const fn is_visible(self) -> bool {
        matches!(self, Self::Visible | Self::Focused | Self::Unfocused)
    }
}

/// The complete widget-facing state of one view.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewData {
    pub id: ViewId,
    pub metrics: ViewMetrics,
    pub environment: RuntimeEnvironment,
    pub lifecycle: ViewLifecycle,
    pub focused: bool,
}

impl ViewData {
    /// Creates a primary or auxiliary view with a normalized environment.
    #[must_use]
    pub fn new(id: ViewId, metrics: ViewMetrics, environment: RuntimeEnvironment) -> Self {
        let metrics = ViewMetrics::new(
            metrics.physical_width(),
            metrics.physical_height(),
            metrics.scale_factor(),
        );
        let mut environment = environment.normalized();
        environment.viewport = metrics.logical_size();
        environment.physical_width = metrics.physical_width();
        environment.physical_height = metrics.physical_height();
        environment.scale_factor = metrics.scale_factor();
        Self {
            id,
            metrics,
            environment,
            lifecycle: ViewLifecycle::Creating,
            focused: false,
        }
    }

    /// Returns whether this view can still receive layout or input.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.lifecycle.is_closed()
    }
}

/// A change emitted by a [`ViewController`].
#[derive(Clone, Debug, PartialEq)]
pub enum ViewEvent {
    MetricsChanged(ViewData),
    EnvironmentChanged(ViewData),
    LifecycleChanged {
        previous: ViewLifecycle,
        current: ViewLifecycle,
    },
    FocusChanged(bool),
    Disposed(ViewId),
}

/// A retained view observer subscription.
pub struct ViewSubscription {
    _listener: Option<ViewListener>,
}

impl fmt::Debug for ViewSubscription {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ViewSubscription(..)")
    }
}

type ViewListener = Rc<dyn Fn(ViewEvent)>;

struct ViewState {
    data: ViewData,
    revision: Rc<Cell<u64>>,
    listeners: Vec<Weak<dyn Fn(ViewEvent)>>,
}

/// Retained metrics/lifecycle ownership for one view.
#[derive(Clone)]
pub struct ViewController {
    state: Rc<RefCell<ViewState>>,
}

impl fmt::Debug for ViewController {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ViewController")
            .field("data", &self.data())
            .field("revision", &self.revision())
            .finish()
    }
}

impl ViewController {
    /// Creates a controller with the supplied initial metrics/environment.
    #[must_use]
    pub fn new(id: ViewId, metrics: ViewMetrics, environment: RuntimeEnvironment) -> Self {
        Self {
            state: Rc::new(RefCell::new(ViewState {
                data: ViewData::new(id, metrics, environment),
                revision: Rc::new(Cell::new(0)),
                listeners: Vec::new(),
            })),
        }
    }

    /// Creates the primary view using Incular's application environment
    /// defaults until the platform publishes real metrics.
    #[must_use]
    pub fn primary(metrics: ViewMetrics) -> Self {
        Self::new(ViewId::PRIMARY, metrics, RuntimeEnvironment::default())
    }

    #[must_use]
    pub fn id(&self) -> ViewId {
        self.state.borrow().data.id
    }

    #[must_use]
    pub fn data(&self) -> ViewData {
        self.state.borrow().data.clone()
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.state.borrow().revision.get()
    }

    /// Subscribes to metrics, environment, focus, lifecycle, and disposal.
    pub fn subscribe(&self, listener: impl Fn(ViewEvent) + 'static) -> ViewSubscription {
        let listener: ViewListener = Rc::new(listener);
        self.state
            .borrow_mut()
            .listeners
            .push(Rc::downgrade(&listener));
        ViewSubscription {
            _listener: Some(listener),
        }
    }

    /// Publishes platform metrics and updates the logical viewport.
    pub fn update_metrics(&self, metrics: ViewMetrics) -> bool {
        let metrics = ViewMetrics::new(
            metrics.physical_width(),
            metrics.physical_height(),
            metrics.scale_factor(),
        );
        let event = {
            let mut state = self.state.borrow_mut();
            if state.data.metrics == metrics || state.data.is_closed() {
                return false;
            }
            state.data.metrics = metrics;
            state.data.environment.viewport = metrics.logical_size();
            state.data.environment.physical_width = metrics.physical_width();
            state.data.environment.physical_height = metrics.physical_height();
            state.data.environment.scale_factor = metrics.scale_factor();
            state.revision.set(state.revision.get().wrapping_add(1));
            ViewEvent::MetricsChanged(state.data.clone())
        };
        self.notify(event);
        true
    }

    /// Publishes the runtime's normalized environment snapshot.
    pub fn set_environment(&self, environment: RuntimeEnvironment) -> bool {
        let event = {
            let mut state = self.state.borrow_mut();
            let mut environment = environment.normalized();
            let metrics = state.data.metrics;
            environment.viewport = metrics.logical_size();
            environment.physical_width = metrics.physical_width();
            environment.physical_height = metrics.physical_height();
            environment.scale_factor = metrics.scale_factor();
            if state.data.environment == environment || state.data.is_closed() {
                return false;
            }
            state.data.environment = environment;
            state.revision.set(state.revision.get().wrapping_add(1));
            ViewEvent::EnvironmentChanged(state.data.clone())
        };
        self.notify(event);
        true
    }

    /// Changes the retained lifecycle state.
    pub fn set_lifecycle(&self, lifecycle: ViewLifecycle) -> bool {
        let event = {
            let mut state = self.state.borrow_mut();
            let previous = state.data.lifecycle;
            if previous == lifecycle || matches!(previous, ViewLifecycle::Closed) {
                return false;
            }
            state.data.lifecycle = lifecycle;
            state.data.focused = matches!(lifecycle, ViewLifecycle::Focused);
            state.revision.set(state.revision.get().wrapping_add(1));
            ViewEvent::LifecycleChanged {
                previous,
                current: lifecycle,
            }
        };
        self.notify(event);
        true
    }

    /// Updates native focus while preserving the widget subtree.
    pub fn set_focused(&self, focused: bool) -> bool {
        let event = {
            let mut state = self.state.borrow_mut();
            if state.data.focused == focused || state.data.is_closed() {
                return false;
            }
            state.data.focused = focused;
            state.data.lifecycle = if focused {
                ViewLifecycle::Focused
            } else {
                ViewLifecycle::Unfocused
            };
            state.revision.set(state.revision.get().wrapping_add(1));
            ViewEvent::FocusChanged(focused)
        };
        self.notify(event);
        true
    }

    /// Performs the closing/closed transition and emits a disposal event.
    pub fn dispose(&self) -> bool {
        let (closing, disposed) = {
            let mut state = self.state.borrow_mut();
            if matches!(
                state.data.lifecycle,
                ViewLifecycle::Closing | ViewLifecycle::Closed
            ) {
                return false;
            }
            let previous = state.data.lifecycle;
            state.data.lifecycle = ViewLifecycle::Closing;
            state.revision.set(state.revision.get().wrapping_add(1));
            let closing = ViewEvent::LifecycleChanged {
                previous,
                current: ViewLifecycle::Closing,
            };
            state.data.lifecycle = ViewLifecycle::Closed;
            state.data.focused = false;
            state.revision.set(state.revision.get().wrapping_add(1));
            (closing, ViewEvent::Disposed(state.data.id))
        };
        self.notify(closing);
        self.notify(disposed);
        true
    }

    fn notify(&self, event: ViewEvent) {
        let listeners = {
            let mut state = self.state.borrow_mut();
            state
                .listeners
                .retain(|listener| listener.strong_count() > 0);
            state
                .listeners
                .iter()
                .filter_map(Weak::upgrade)
                .collect::<Vec<_>>()
        };
        for listener in listeners {
            listener(event.clone());
        }
    }
}

/// A widget that installs one view's environment and retains its lifecycle.
#[derive(Clone)]
pub struct View {
    controller: ViewController,
    child: Widget,
}

impl fmt::Debug for View {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("View")
            .field("controller", &self.controller)
            .field("child", &self.child)
            .finish()
    }
}

impl View {
    /// Creates a view with a fresh controller.
    #[must_use]
    pub fn new(id: ViewId, child: impl Into<Widget>) -> Self {
        Self::with_controller(
            ViewController::new(id, ViewMetrics::default(), RuntimeEnvironment::default()),
            child,
        )
    }

    /// Creates a primary view using a fresh retained controller.
    #[must_use]
    pub fn primary(child: impl Into<Widget>) -> Self {
        Self::new(ViewId::PRIMARY, child)
    }

    /// Uses an existing metrics/lifecycle owner.
    #[must_use]
    pub fn with_controller(controller: ViewController, child: impl Into<Widget>) -> Self {
        Self {
            controller,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn controller(&self) -> ViewController {
        self.controller.clone()
    }

    #[must_use]
    pub fn id(&self) -> ViewId {
        self.controller.id()
    }

    #[must_use]
    pub fn data(&self) -> ViewData {
        self.controller.data()
    }

    /// Builds a retained environment scope that disappears when the view is
    /// closed while preserving the controller for later native events.
    #[must_use]
    pub fn into_widget(self) -> Widget {
        let controller = self.controller;
        let child = self.child;
        let revision = controller.state.borrow().revision.clone();
        let retained_controller = controller.clone();
        Widget::stateful_layout_builder(revision, move |_, _| {
            let data = retained_controller.data();
            let child = Widget::visibility(!data.is_closed(), child.clone());
            Widget::environment_scope(
                data.environment.clone(),
                Widget::environment_scope(data, child),
            )
        })
    }

    /// Alias for [`Self::into_widget`].
    #[must_use]
    pub fn widget(self) -> Widget {
        self.into_widget()
    }
}

impl From<View> for Widget {
    fn from(value: View) -> Self {
        value.into_widget()
    }
}

/// Request sent to an auxiliary-view host for a view anchor.
#[derive(Clone, Debug, PartialEq)]
pub struct AuxiliaryViewRequest {
    pub view_id: ViewId,
    pub child: Widget,
    pub metrics: ViewMetrics,
    pub environment: RuntimeEnvironment,
    pub anchor: Rect,
    pub title: Option<String>,
}

/// Opaque identity returned by an auxiliary-view host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AuxiliaryViewHandle {
    pub view_id: ViewId,
    pub token: u64,
}

/// Why an auxiliary view could not be created or updated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuxiliaryViewError {
    /// The selected backend has no secondary-window capability.
    Unsupported,
    /// The backend rejected a request or a stale handle.
    Rejected(String),
    /// The request contains an invalid anchor or closed view.
    Invalid(String),
}

impl fmt::Display for AuxiliaryViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => formatter.write_str("auxiliary views are unsupported"),
            Self::Rejected(message) => write!(formatter, "auxiliary view rejected: {message}"),
            Self::Invalid(message) => write!(formatter, "invalid auxiliary view: {message}"),
        }
    }
}

impl std::error::Error for AuxiliaryViewError {}

/// Backend-neutral creation/update/disposal boundary for [`ViewAnchor`].
pub trait AuxiliaryViewHost: 'static {
    fn create(
        &self,
        request: AuxiliaryViewRequest,
    ) -> Result<AuxiliaryViewHandle, AuxiliaryViewError>;

    fn update(
        &self,
        handle: AuxiliaryViewHandle,
        request: AuxiliaryViewRequest,
    ) -> Result<(), AuxiliaryViewError>;

    fn dispose(&self, handle: AuxiliaryViewHandle);
}

/// Deterministic no-op host used when the platform has no auxiliary-view
/// capability.  It never allocates a fake handle or claims success.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopAuxiliaryViewHost;

impl AuxiliaryViewHost for NoopAuxiliaryViewHost {
    fn create(
        &self,
        _request: AuxiliaryViewRequest,
    ) -> Result<AuxiliaryViewHandle, AuxiliaryViewError> {
        Err(AuxiliaryViewError::Unsupported)
    }

    fn update(
        &self,
        _handle: AuxiliaryViewHandle,
        _request: AuxiliaryViewRequest,
    ) -> Result<(), AuxiliaryViewError> {
        Err(AuxiliaryViewError::Unsupported)
    }

    fn dispose(&self, _handle: AuxiliaryViewHandle) {}
}

struct ViewAnchorState {
    data: ViewAnchorData,
    active: Option<AuxiliaryViewHandle>,
    last_request: Option<AuxiliaryViewRequest>,
    last_error: Option<AuxiliaryViewError>,
    revision: Rc<Cell<u64>>,
    listeners: Vec<Weak<dyn Fn(ViewAnchorData)>>,
}

/// Ambient state exposed to a view-anchor child.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewAnchorData {
    pub view_id: Option<ViewId>,
    pub anchor: Rect,
    pub attached: bool,
    pub last_error: Option<String>,
}

struct ViewAnchorInner {
    host: Rc<dyn AuxiliaryViewHost>,
    state: RefCell<ViewAnchorState>,
}

impl Drop for ViewAnchorInner {
    fn drop(&mut self) {
        if let Some(handle) = self.state.get_mut().active.take() {
            self.host.dispose(handle);
        }
    }
}

/// Retained owner of one anchored auxiliary view.
#[derive(Clone)]
pub struct ViewAnchorController {
    inner: Rc<ViewAnchorInner>,
}

impl fmt::Debug for ViewAnchorController {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ViewAnchorController")
            .field("data", &self.data())
            .field("handle", &self.active_handle())
            .finish()
    }
}

impl ViewAnchorController {
    /// Creates a controller using the deterministic unsupported host.
    #[must_use]
    pub fn new() -> Self {
        Self::with_host(Rc::new(NoopAuxiliaryViewHost))
    }

    /// Creates a controller connected to an application/platform host.
    #[must_use]
    pub fn with_host(host: Rc<dyn AuxiliaryViewHost>) -> Self {
        Self {
            inner: Rc::new(ViewAnchorInner {
                host,
                state: RefCell::new(ViewAnchorState {
                    data: ViewAnchorData::default(),
                    active: None,
                    last_request: None,
                    last_error: None,
                    revision: Rc::new(Cell::new(0)),
                    listeners: Vec::new(),
                }),
            }),
        }
    }

    #[must_use]
    pub fn data(&self) -> ViewAnchorData {
        self.inner.state.borrow().data.clone()
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.inner.state.borrow().revision.get()
    }

    #[must_use]
    pub fn active_handle(&self) -> Option<AuxiliaryViewHandle> {
        self.inner.state.borrow().active
    }

    #[must_use]
    pub fn last_error(&self) -> Option<AuxiliaryViewError> {
        self.inner.state.borrow().last_error.clone()
    }

    pub fn subscribe(&self, listener: impl Fn(ViewAnchorData) + 'static) -> ViewAnchorSubscription {
        let listener: ViewAnchorListener = Rc::new(listener);
        self.inner
            .state
            .borrow_mut()
            .listeners
            .push(Rc::downgrade(&listener));
        ViewAnchorSubscription {
            _listener: Some(listener),
        }
    }

    /// Creates or updates the host-owned side view, coalescing identical
    /// requests so a layout rebuild does not recreate native windows.
    pub fn sync(
        &self,
        request: AuxiliaryViewRequest,
    ) -> Result<AuxiliaryViewOutcome, AuxiliaryViewError> {
        if request.environment.window_focused && request.metrics.scale_factor() <= 0.0 {
            return self.fail(AuxiliaryViewError::Invalid(
                "focused auxiliary views require a valid scale factor".into(),
            ));
        }
        let existing = {
            let state = self.inner.state.borrow();
            if state.last_request.as_ref() == Some(&request) && state.active.is_some() {
                return Ok(AuxiliaryViewOutcome::Unchanged(state.active));
            }
            state.active
        };
        let outcome = if let Some(handle) = existing {
            match self.inner.host.update(handle, request.clone()) {
                Ok(()) => AuxiliaryViewOutcome::Updated(handle),
                Err(error) => return self.fail(error),
            }
        } else {
            match self.inner.host.create(request.clone()) {
                Ok(handle) => AuxiliaryViewOutcome::Created(handle),
                Err(error) => return self.fail(error),
            }
        };
        {
            let mut state = self.inner.state.borrow_mut();
            let handle = match outcome {
                AuxiliaryViewOutcome::Created(handle)
                | AuxiliaryViewOutcome::Updated(handle)
                | AuxiliaryViewOutcome::Unchanged(Some(handle)) => Some(handle),
                AuxiliaryViewOutcome::Unchanged(None) => None,
            };
            state.active = handle;
            state.last_request = Some(request.clone());
            state.last_error = None;
            state.data = ViewAnchorData {
                view_id: Some(request.view_id),
                anchor: request.anchor,
                attached: handle.is_some(),
                last_error: None,
            };
            state.revision.set(state.revision.get().wrapping_add(1));
        }
        self.notify();
        Ok(outcome)
    }

    /// Creates or updates an anchor using the last request's other fields.
    pub fn update_anchor(&self, anchor: Rect) -> Result<AuxiliaryViewOutcome, AuxiliaryViewError> {
        let request = self
            .inner
            .state
            .borrow()
            .last_request
            .clone()
            .ok_or_else(|| AuxiliaryViewError::Invalid("anchor has not been attached".into()))?;
        self.sync(AuxiliaryViewRequest { anchor, ..request })
    }

    /// Disposes the host-owned side view while retaining the controller.
    pub fn detach(&self) -> bool {
        let handle = {
            let mut state = self.inner.state.borrow_mut();
            let handle = state.active.take();
            if handle.is_none() && !state.data.attached {
                return false;
            }
            state.data.attached = false;
            state.data.last_error = None;
            state.last_request = None;
            state.last_error = None;
            state.revision.set(state.revision.get().wrapping_add(1));
            handle
        };
        if let Some(handle) = handle {
            self.inner.host.dispose(handle);
        }
        self.notify();
        true
    }

    fn fail(&self, error: AuxiliaryViewError) -> Result<AuxiliaryViewOutcome, AuxiliaryViewError> {
        {
            let mut state = self.inner.state.borrow_mut();
            state.last_error = Some(error.clone());
            state.data.last_error = Some(error.to_string());
            state.data.attached = state.active.is_some();
            state.revision.set(state.revision.get().wrapping_add(1));
        }
        self.notify();
        Err(error)
    }

    fn notify(&self) {
        let (data, listeners) = {
            let mut state = self.inner.state.borrow_mut();
            state
                .listeners
                .retain(|listener| listener.strong_count() > 0);
            (
                state.data.clone(),
                state
                    .listeners
                    .iter()
                    .filter_map(Weak::upgrade)
                    .collect::<Vec<_>>(),
            )
        };
        for listener in listeners {
            listener(data.clone());
        }
    }
}

impl Default for ViewAnchorController {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of synchronizing a side view with its host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuxiliaryViewOutcome {
    Created(AuxiliaryViewHandle),
    Updated(AuxiliaryViewHandle),
    Unchanged(Option<AuxiliaryViewHandle>),
}

/// Keeps an anchor observer alive.
pub struct ViewAnchorSubscription {
    _listener: Option<ViewAnchorListener>,
}

impl fmt::Debug for ViewAnchorSubscription {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ViewAnchorSubscription(..)")
    }
}

type ViewAnchorListener = Rc<dyn Fn(ViewAnchorData)>;

/// A child widget paired with a host-owned auxiliary view.
#[derive(Clone)]
pub struct ViewAnchor {
    view: Option<View>,
    child: Widget,
    controller: ViewAnchorController,
    anchor: Rect,
    title: Option<String>,
}

impl fmt::Debug for ViewAnchor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ViewAnchor")
            .field("view", &self.view)
            .field("controller", &self.controller)
            .field("anchor", &self.anchor)
            .field("title", &self.title)
            .finish()
    }
}

impl ViewAnchor {
    /// Creates an anchor with no side view.  This is an intentional detached
    /// state, not a fake auxiliary window.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            view: None,
            child: child.into(),
            controller: ViewAnchorController::default(),
            anchor: Rect::default(),
            title: None,
        }
    }

    /// Creates an anchor for a side view.
    #[must_use]
    pub fn with_view(view: View, child: impl Into<Widget>) -> Self {
        Self {
            view: Some(view),
            child: child.into(),
            controller: ViewAnchorController::default(),
            anchor: Rect::default(),
            title: None,
        }
    }

    /// Reuses an existing retained anchor owner.
    #[must_use]
    pub fn with_controller(mut self, controller: ViewAnchorController) -> Self {
        self.controller = controller;
        self
    }

    /// Installs a platform/runtime auxiliary-view host.
    #[must_use]
    pub fn host(self, host: Rc<dyn AuxiliaryViewHost>) -> Self {
        self.with_controller(ViewAnchorController::with_host(host))
    }

    /// Sets the anchor rectangle in surrounding-view logical coordinates.
    #[must_use]
    pub fn anchor(mut self, anchor: Rect) -> Self {
        self.anchor = anchor;
        self
    }

    /// Sets the optional native title for the auxiliary view.
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    #[must_use]
    pub fn controller(&self) -> ViewAnchorController {
        self.controller.clone()
    }

    #[must_use]
    pub fn view(&self) -> Option<View> {
        self.view.clone()
    }

    /// Keeps the surrounding child in place while synchronizing the side view
    /// with the retained host owner.
    #[must_use]
    pub fn into_widget(self) -> Widget {
        let view = self.view;
        let child = self.child;
        let controller = self.controller;
        let anchor = self.anchor;
        let title = self.title;
        let revision = controller.inner.state.borrow().revision.clone();
        let retained_controller = controller.clone();
        Widget::stateful_layout_builder(revision, move |_, _| {
            if let Some(view) = view.as_ref() {
                let data = view.data();
                let request = AuxiliaryViewRequest {
                    view_id: data.id,
                    child: view.child.clone(),
                    metrics: data.metrics,
                    environment: data.environment.clone(),
                    anchor,
                    title: title.clone(),
                };
                let _ = retained_controller.sync(request);
            } else {
                retained_controller.detach();
            }
            Widget::environment_scope(retained_controller.data(), child.clone())
        })
    }

    /// Alias for [`Self::into_widget`].
    #[must_use]
    pub fn widget(self) -> Widget {
        self.into_widget()
    }
}

impl From<ViewAnchor> for Widget {
    fn from(value: ViewAnchor) -> Self {
        value.into_widget()
    }
}
