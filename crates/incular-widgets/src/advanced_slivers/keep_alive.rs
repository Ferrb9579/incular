use super::*;

/// A disposable descendant keep-alive handle.
///
/// Flutter's handle is a `Listenable`: disposing it tells the nearest
/// `AutomaticKeepAlive` that this descendant no longer needs retention.  The
/// callback list is deliberately cloneable so one client can pass a handle
/// through several widget layers without changing its identity.
#[derive(Clone)]
pub struct KeepAliveHandle {
    inner: Rc<RefCell<KeepAliveHandleState>>,
}

struct KeepAliveHandleState {
    id: u64,
    released: bool,
    next_listener: u64,
    listeners: Vec<(u64, Rc<dyn Fn()>)>,
}

impl KeepAliveHandle {
    #[must_use]
    pub fn new() -> Self {
        static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);
        Self {
            inner: Rc::new(RefCell::new(KeepAliveHandleState {
                id: NEXT_HANDLE_ID.fetch_add(1, Ordering::Relaxed).max(1),
                released: false,
                next_listener: 1,
                listeners: Vec::new(),
            })),
        }
    }

    #[must_use]
    pub fn id(&self) -> u64 {
        self.inner.borrow().id
    }

    #[must_use]
    pub fn is_released(&self) -> bool {
        self.inner.borrow().released
    }

    /// Releases this handle and synchronously notifies every automatic
    /// keep-alive ancestor that registered it. This is idempotent.
    pub fn release(&self) {
        let listeners = {
            let mut state = self.inner.borrow_mut();
            if state.released {
                return;
            }
            state.released = true;
            state
                .listeners
                .iter()
                .map(|(_, listener)| listener.clone())
                .collect::<Vec<_>>()
        };
        for listener in listeners {
            listener();
        }
    }

    fn add_listener(&self, listener: Rc<dyn Fn()>) -> Option<u64> {
        let mut state = self.inner.borrow_mut();
        if state.released {
            return None;
        }
        let id = state.next_listener;
        state.next_listener = state.next_listener.saturating_add(1).max(1);
        state.listeners.push((id, listener));
        Some(id)
    }

    fn remove_listener(&self, listener_id: u64) {
        self.inner
            .borrow_mut()
            .listeners
            .retain(|(id, _)| *id != listener_id);
    }
}

impl Default for KeepAliveHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Descendant notification consumed by `AutomaticKeepAlive`.
#[derive(Clone)]
pub struct KeepAliveNotification {
    handle: KeepAliveHandle,
}

impl KeepAliveNotification {
    #[must_use]
    pub fn new(handle: KeepAliveHandle) -> Self {
        Self { handle }
    }

    #[must_use]
    pub fn handle(&self) -> KeepAliveHandle {
        self.handle.clone()
    }
}

struct KeepAliveRegistryState {
    clients: HashMap<u64, (KeepAliveHandle, u64)>,
    revision: u64,
}

/// Ancestor-side registry implementing the notification semantics of
/// `AutomaticKeepAlive`.
#[derive(Clone)]
pub struct KeepAliveRegistry {
    state: Rc<RefCell<KeepAliveRegistryState>>,
}

impl KeepAliveRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(KeepAliveRegistryState {
                clients: HashMap::new(),
                revision: 0,
            })),
        }
    }

    /// Registers a notification handle. `false` mirrors Flutter's
    /// `NotificationListener` behavior: keep-alive notifications continue to
    /// bubble to outer ancestors.
    pub fn dispatch(&self, notification: KeepAliveNotification) -> bool {
        let handle = notification.handle;
        let id = handle.id();
        if handle.is_released() {
            return false;
        }
        {
            let state = self.state.borrow();
            if state.clients.contains_key(&id) {
                return false;
            }
        }
        let weak_state: Weak<RefCell<KeepAliveRegistryState>> = Rc::downgrade(&self.state);
        let weak_handle = Rc::downgrade(&handle.inner);
        let listener_id_cell = Rc::new(RefCell::new(None::<u64>));
        let listener_id_for_callback = listener_id_cell.clone();
        let listener = Rc::new(move || {
            let Some(state) = weak_state.upgrade() else {
                return;
            };
            let removed = {
                let mut state = state.borrow_mut();
                let removed = state.clients.remove(&id).is_some();
                if removed {
                    state.revision = state.revision.wrapping_add(1);
                }
                removed
            };
            if removed
                && let (Some(handle), Some(listener_id)) =
                    (weak_handle.upgrade(), *listener_id_for_callback.borrow())
            {
                handle
                    .borrow_mut()
                    .listeners
                    .retain(|(id, _)| *id != listener_id);
            }
        });
        let Some(listener_id) = handle.add_listener(listener) else {
            return false;
        };
        *listener_id_cell.borrow_mut() = Some(listener_id);
        self.state
            .borrow_mut()
            .clients
            .insert(id, (handle, listener_id));
        let mut state = self.state.borrow_mut();
        state.revision = state.revision.wrapping_add(1);
        false
    }

    pub fn notify(&self, notification: KeepAliveNotification) -> bool {
        self.dispatch(notification)
    }

    #[must_use]
    pub fn keeping_alive(&self) -> bool {
        !self.state.borrow().clients.is_empty()
    }

    #[must_use]
    pub fn active_count(&self) -> usize {
        self.state.borrow().clients.len()
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.state.borrow().revision
    }

    /// Detaches every registered listener. Existing handles remain valid and
    /// can be dispatched to a different automatic ancestor.
    pub fn clear(&self) {
        let clients = {
            let mut state = self.state.borrow_mut();
            let clients = state
                .clients
                .drain()
                .map(|(_, value)| value)
                .collect::<Vec<_>>();
            state.revision = state.revision.wrapping_add(1);
            clients
        };
        for (handle, listener_id) in clients {
            handle.remove_listener(listener_id);
        }
    }
}

impl Default for KeepAliveRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Parent-data intent equivalent to Flutter's `KeepAlive` widget.
///
/// The retained tree currently has no public parent-data channel. Therefore
/// conversion to a plain `Widget` is transparent, while the request remains
/// available through `request()` for the sliver materializer integration
/// patch. This keeps the state semantics real without smuggling retention
/// through a user key or changing an existing widget kind.
#[derive(Clone)]
pub struct KeepAlive {
    keep_alive: bool,
    child: Widget,
}

impl KeepAlive {
    #[must_use]
    pub fn new(keep_alive: bool, child: impl Into<Widget>) -> Self {
        Self {
            keep_alive,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn keep_alive(&self) -> bool {
        self.keep_alive
    }

    #[must_use]
    pub fn child(&self) -> &Widget {
        &self.child
    }

    #[must_use]
    pub fn request(&self) -> KeepAliveRequest {
        KeepAliveRequest {
            keep_alive: self.keep_alive,
        }
    }

    #[must_use]
    pub fn into_child(self) -> Widget {
        self.child
    }
}

impl From<KeepAlive> for Widget {
    fn from(keep_alive: KeepAlive) -> Self {
        keep_alive.into_child()
    }
}

/// The parent-data value a sliver materializer must read from `KeepAlive`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeepAliveRequest {
    pub keep_alive: bool,
}

/// An automatic ancestor that aggregates all active descendant handles.
#[derive(Clone)]
pub struct AutomaticKeepAlive {
    child: Widget,
    registry: KeepAliveRegistry,
}

impl AutomaticKeepAlive {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            registry: KeepAliveRegistry::new(),
        }
    }

    #[must_use]
    pub fn with_registry(child: impl Into<Widget>, registry: KeepAliveRegistry) -> Self {
        Self {
            child: child.into(),
            registry,
        }
    }

    #[must_use]
    pub fn registry(&self) -> KeepAliveRegistry {
        self.registry.clone()
    }

    #[must_use]
    pub fn keeping_alive(&self) -> bool {
        self.registry.keeping_alive()
    }

    #[must_use]
    pub fn child(&self) -> &Widget {
        &self.child
    }

    /// Returns `false` so outer automatic ancestors can also observe the
    /// notification, matching Flutter's bubbling listener.
    pub fn on_notification(&self, notification: KeepAliveNotification) -> bool {
        self.registry.dispatch(notification)
    }

    #[must_use]
    pub fn into_child(self) -> Widget {
        self.child
    }
}

impl From<AutomaticKeepAlive> for Widget {
    fn from(automatic: AutomaticKeepAlive) -> Self {
        automatic.into_child()
    }
}
