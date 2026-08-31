use crate::frame::Runtime;
use crate::reactive;
use crate::scheduler_counters;
use crate::tasks;
use incular_config::RuntimeEnvironment;
use incular_widgets::internal::ElementId;
#[cfg(feature = "devtools")]
use incular_widgets::internal::InvalidationCause;
use incular_widgets::{FocusScopeNode, FocusScopeSubscription};
#[cfg(feature = "devtools")]
use std::any::{Any, TypeId};
#[cfg(feature = "devtools")]
use std::cell::Cell;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    rc::{Rc, Weak},
    sync::atomic::{AtomicU64, Ordering},
};

/// Truncates a Debug rendering to keep DevTools payloads bounded.
#[cfg(feature = "devtools")]
pub(crate) fn truncate_debug<T: std::fmt::Debug>(value: &T) -> String {
    let rendered = format!("{value:?}");
    if rendered.chars().count() <= 80 {
        rendered
    } else {
        let mut out: String = rendered.chars().take(77).collect();
        out.push('…');
        out
    }
}

pub(crate) const ENV_VIEWPORT: u16 = 1 << 0;
pub(crate) const ENV_SCALE: u16 = 1 << 1;
pub(crate) const ENV_BRIGHTNESS: u16 = 1 << 2;
pub(crate) const ENV_TEXT_SCALE: u16 = 1 << 3;
pub(crate) const ENV_SAFE_INSETS: u16 = 1 << 4;
pub(crate) const ENV_VIEW_INSETS: u16 = 1 << 5;
pub(crate) const ENV_LOCALE: u16 = 1 << 6;
pub(crate) const ENV_DIRECTION: u16 = 1 << 7;
pub(crate) const ENV_REDUCED_MOTION: u16 = 1 << 8;
pub(crate) const ENV_INPUT: u16 = 1 << 9;
pub(crate) const ENV_WINDOW_FOCUS: u16 = 1 << 10;
pub(crate) const ENV_ALL: u16 = (1 << 11) - 1;

pub(crate) fn environment_change_mask(
    previous: &RuntimeEnvironment,
    next: &RuntimeEnvironment,
) -> u16 {
    let mut mask = 0;
    if previous.viewport != next.viewport
        || previous.physical_width != next.physical_width
        || previous.physical_height != next.physical_height
    {
        mask |= ENV_VIEWPORT;
    }
    if previous.scale_factor != next.scale_factor {
        mask |= ENV_SCALE;
    }
    if previous.brightness != next.brightness {
        mask |= ENV_BRIGHTNESS;
    }
    if previous.text_scale != next.text_scale {
        mask |= ENV_TEXT_SCALE;
    }
    if previous.safe_insets != next.safe_insets {
        mask |= ENV_SAFE_INSETS;
    }
    if previous.view_insets != next.view_insets {
        mask |= ENV_VIEW_INSETS;
    }
    if previous.locales != next.locales {
        mask |= ENV_LOCALE;
    }
    if previous.text_direction != next.text_direction {
        mask |= ENV_DIRECTION;
    }
    if previous.reduced_motion != next.reduced_motion {
        mask |= ENV_REDUCED_MOTION;
    }
    if previous.input != next.input {
        mask |= ENV_INPUT;
    }
    if previous.window_focused != next.window_focused {
        mask |= ENV_WINDOW_FOCUS;
    }
    mask
}

pub(crate) type ReactiveRootId = u64;
pub(crate) type ReactiveNodeId = u64;
pub(crate) type ReactiveNodeDependents =
    HashMap<ReactiveRootId, HashMap<ReactiveNodeId, Weak<dyn ReactiveNode>>>;

pub(crate) static NEXT_REACTIVE_ROOT: AtomicU64 = AtomicU64::new(1);
pub(crate) static NEXT_REACTIVE_NODE: AtomicU64 = AtomicU64::new(1);

pub(crate) trait ReactiveNode {
    fn id(&self) -> ReactiveNodeId;
    fn mark_dirty(&self);
    fn run(&self, root: ReactiveRootId);
    fn record_dependency(&self, dependency: Weak<dyn Dependency>);
    fn dispose(&self, root: ReactiveRootId);
}

pub(crate) trait Dependency {
    fn subscribe(
        &self,
        root: ReactiveRootId,
        element: ElementId,
        queue: Weak<RefCell<ReactiveQueue>>,
    );
    fn remove(&self, root: ReactiveRootId, element: ElementId);
    fn subscribe_node(
        &self,
        root: ReactiveRootId,
        node: ReactiveNodeId,
        subscriber: Weak<dyn ReactiveNode>,
        queue: Weak<RefCell<ReactiveQueue>>,
    );
    fn remove_node(&self, root: ReactiveRootId, node: ReactiveNodeId);
}
pub(crate) struct ReactiveQueue {
    pub(crate) root: ReactiveRootId,
    pub(crate) queued: HashSet<ElementId>,
    pub(crate) order: VecDeque<ElementId>,
    pub(crate) queued_nodes: HashSet<ReactiveNodeId>,
    pub(crate) node_order: VecDeque<ReactiveNodeId>,
    pub(crate) nodes: HashMap<ReactiveNodeId, Weak<dyn ReactiveNode>>,
    pub(crate) dependencies: HashMap<ElementId, Vec<Weak<dyn Dependency>>>,
    /// Focus-scope subscriptions registered by builders. Keeping the token in
    /// the same lifetime bucket as signal dependencies makes unmount and
    /// rebuild cleanup deterministic rather than leaking callbacks into a
    /// long-lived scope node.
    pub(crate) focus_scopes: HashMap<ElementId, Vec<FocusScopeSubscription>>,
    #[cfg(feature = "devtools")]
    pub(crate) causes: HashMap<ElementId, Vec<InvalidationCause>>,
}
impl ReactiveQueue {
    pub(crate) fn new() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            root: NEXT_REACTIVE_ROOT.fetch_add(1, Ordering::Relaxed),
            queued: HashSet::new(),
            order: VecDeque::new(),
            queued_nodes: HashSet::new(),
            node_order: VecDeque::new(),
            nodes: HashMap::new(),
            dependencies: HashMap::new(),
            focus_scopes: HashMap::new(),
            #[cfg(feature = "devtools")]
            causes: HashMap::new(),
        }))
    }

    pub(crate) fn enqueue(&mut self, id: ElementId) -> bool {
        if self.queued.insert(id) {
            self.order.push_back(id);
            true
        } else {
            false
        }
    }
    pub(crate) fn enqueue_node(
        &mut self,
        id: ReactiveNodeId,
        node: Weak<dyn ReactiveNode>,
    ) -> bool {
        self.register_node(id, node);
        if self.queued_nodes.insert(id) {
            self.node_order.push_back(id);
            true
        } else {
            false
        }
    }
    pub(crate) fn register_node(&mut self, id: ReactiveNodeId, node: Weak<dyn ReactiveNode>) {
        self.nodes.insert(id, node);
    }
    #[cfg(feature = "devtools")]
    pub(crate) fn note_cause(&mut self, id: ElementId, cause: InvalidationCause) {
        const MAX_CAUSES: usize = 8;
        let causes = self.causes.entry(id).or_default();
        if causes.len() == MAX_CAUSES {
            causes.remove(0);
        }
        causes.push(cause);
    }
    #[cfg(feature = "devtools")]
    pub(crate) fn take_causes(&mut self, id: ElementId) -> Vec<InvalidationCause> {
        self.causes.remove(&id).unwrap_or_default()
    }
    pub(crate) fn take(&mut self) -> Option<ElementId> {
        let id = self.order.pop_front()?;
        self.queued.remove(&id);
        Some(id)
    }
    pub(crate) fn take_node(&mut self) -> Option<(ReactiveRootId, Weak<dyn ReactiveNode>)> {
        let id = self.node_order.pop_front()?;
        self.queued_nodes.remove(&id);
        self.nodes.get(&id).cloned().map(|node| (self.root, node))
    }
    pub(crate) fn has_work(&self) -> bool {
        !self.queued.is_empty() || !self.queued_nodes.is_empty()
    }
    pub(crate) fn refresh(&mut self, id: ElementId) {
        if let Some(deps) = self.dependencies.remove(&id) {
            for dep in deps {
                if let Some(dep) = dep.upgrade() {
                    dep.remove(self.root, id);
                }
            }
        }
        self.focus_scopes.remove(&id);
    }
    pub(crate) fn record(&mut self, id: ElementId, dep: Weak<dyn Dependency>) {
        let entries = self.dependencies.entry(id).or_default();
        if !entries.iter().any(|current| current.ptr_eq(&dep)) {
            entries.push(dep);
        }
    }

    pub(crate) fn watch_focus_scope(
        &mut self,
        id: ElementId,
        subscription: FocusScopeSubscription,
    ) {
        self.focus_scopes.entry(id).or_default().push(subscription);
    }
    pub(crate) fn forget(&mut self, id: ElementId) {
        self.refresh(id);
        self.queued.remove(&id);
        #[cfg(feature = "devtools")]
        self.causes.remove(&id);
    }

    pub(crate) fn clear(&mut self) {
        // Focus-only builders do not appear in `dependencies`; include their
        // owner IDs so closing a window drops every subscription token too.
        let mut ids: HashSet<_> = self.dependencies.keys().copied().collect();
        ids.extend(self.focus_scopes.keys().copied());
        let ids: Vec<_> = ids.into_iter().collect();
        for id in ids {
            self.refresh(id);
        }
        self.queued.clear();
        self.order.clear();
        let nodes: Vec<_> = self.nodes.values().filter_map(Weak::upgrade).collect();
        self.queued_nodes.clear();
        self.node_order.clear();
        self.nodes.clear();
        for node in nodes {
            node.dispose(self.root);
        }
        #[cfg(feature = "devtools")]
        self.causes.clear();
    }
}
pub(crate) fn install_focus_scope_watch(
    queue: &Rc<RefCell<ReactiveQueue>>,
    element: ElementId,
    scope: &FocusScopeNode,
) {
    let weak_queue = Rc::downgrade(queue);
    let subscription = scope.observe(move || {
        if let Some(queue) = weak_queue.upgrade() {
            let mut queue = queue.borrow_mut();
            queue.enqueue(element);
            #[cfg(feature = "devtools")]
            queue.note_cause(element, InvalidationCause::Manual);
        }
    });
    queue.borrow_mut().watch_focus_scope(element, subscription);
}
#[derive(Default)]
pub(crate) struct InitialBuildDependencies {
    pub(crate) signals: Vec<Rc<dyn Dependency>>,
    pub(crate) focus_scopes: Vec<FocusScopeNode>,
}
impl InitialBuildDependencies {
    pub(crate) fn record_signal(&mut self, dependency: Rc<dyn Dependency>) {
        if !self
            .signals
            .iter()
            .any(|current| Rc::ptr_eq(current, &dependency))
        {
            self.signals.push(dependency);
        }
    }
}
pub(crate) struct BuildScope {
    pub(crate) root: ReactiveRootId,
    pub(crate) element: Option<ElementId>,
    pub(crate) queue: Weak<RefCell<ReactiveQueue>>,
    pub(crate) initial_dependencies: Option<Rc<RefCell<InitialBuildDependencies>>>,
    pub(crate) spawner: tasks::RuntimeSpawner,
    pub(crate) owner_scope: tasks::TaskScope,
}
thread_local! { pub(crate) static BUILD_SCOPE: RefCell<Option<BuildScope>> = const { RefCell::new(None) }; }

/// Restores the previous builder scope even when user code panics.
pub(crate) struct BuildScopeGuard {
    previous: Option<BuildScope>,
}

impl BuildScopeGuard {
    pub(crate) fn enter(next: BuildScope) -> Self {
        let previous = BUILD_SCOPE.with(|scope| scope.replace(Some(next)));
        Self { previous }
    }
}

impl Drop for BuildScopeGuard {
    fn drop(&mut self) {
        BUILD_SCOPE.with(|scope| {
            scope.replace(self.previous.take());
        });
    }
}

fn assert_reactive_mutation_allowed() {
    BUILD_SCOPE.with(|scope| {
        let current = scope.borrow();
        let Some(scope) = current.as_ref() else {
            return;
        };
        let owner = scope.element.map_or_else(
            || "the application root".to_owned(),
            |id| format!("element {id:?}"),
        );
        panic!(
            "Incular reactive state was mutated while building {owner}; move the mutation to an event callback, Effect, or Action"
        );
    });
}

#[cfg(feature = "devtools")]
thread_local! { pub(crate) static DEV_TASK_COMPLETION: Cell<bool> = const { Cell::new(false) }; }
struct SignalInner<T> {
    value: RefCell<T>,
    dependents: RefCell<HashMap<ReactiveRootId, HashSet<ElementId>>>,
    queues: RefCell<HashMap<ReactiveRootId, Weak<RefCell<ReactiveQueue>>>>,
    node_dependents: RefCell<ReactiveNodeDependents>,
    node_queues: RefCell<HashMap<ReactiveRootId, Weak<RefCell<ReactiveQueue>>>>,
    /// DevTools write counter; always present, only read under the feature.
    dev_write_count: std::cell::Cell<u64>,
    #[cfg(feature = "devtools")]
    dev_signal_id: std::cell::Cell<Option<u64>>,
    #[cfg(feature = "devtools")]
    dev_name: RefCell<Option<String>>,
    #[cfg(feature = "devtools")]
    dev_editable_kind: std::cell::Cell<Option<EditableSignalKind>>,
    #[cfg(feature = "devtools")]
    dev_last_write: RefCell<(Option<String>, Option<String>)>,
    #[cfg(feature = "devtools")]
    dev_summarize: RefCell<Option<DevSummarizer<T>>>,
}
impl<T> Dependency for SignalInner<T> {
    fn subscribe(
        &self,
        root: ReactiveRootId,
        element: ElementId,
        queue: Weak<RefCell<ReactiveQueue>>,
    ) {
        self.queues.borrow_mut().insert(root, queue);
        self.dependents
            .borrow_mut()
            .entry(root)
            .or_default()
            .insert(element);
    }

    fn remove(&self, root: ReactiveRootId, element: ElementId) {
        let mut dependents = self.dependents.borrow_mut();
        if let Some(elements) = dependents.get_mut(&root) {
            elements.remove(&element);
            if elements.is_empty() {
                dependents.remove(&root);
                self.queues.borrow_mut().remove(&root);
            }
        }
    }

    fn subscribe_node(
        &self,
        root: ReactiveRootId,
        node: ReactiveNodeId,
        subscriber: Weak<dyn ReactiveNode>,
        queue: Weak<RefCell<ReactiveQueue>>,
    ) {
        if let Some(queue) = queue.upgrade() {
            queue.borrow_mut().register_node(node, subscriber.clone());
        }
        self.node_queues.borrow_mut().insert(root, queue);
        self.node_dependents
            .borrow_mut()
            .entry(root)
            .or_default()
            .insert(node, subscriber);
    }

    fn remove_node(&self, root: ReactiveRootId, node: ReactiveNodeId) {
        let mut dependents = self.node_dependents.borrow_mut();
        if let Some(nodes) = dependents.get_mut(&root) {
            nodes.remove(&node);
            if nodes.is_empty() {
                dependents.remove(&root);
                self.node_queues.borrow_mut().remove(&root);
            }
        }
    }
}
/// Shared state. A read in a registered builder subscribes that Element.
#[derive(Clone)]
pub struct Signal<T> {
    inner: Rc<SignalInner<T>>,
}
#[cfg(feature = "devtools")]
type DevSummarizer<T> = Box<dyn Fn(&T) -> String>;

#[cfg(feature = "devtools")]
fn editable_signal_value<T: 'static>(
    value: &incular_devtools_protocol::EditableValue,
) -> Option<T> {
    fn cast<T: 'static, U: Any>(value: U) -> Option<T> {
        let value: Box<dyn Any> = Box::new(value);
        value.downcast::<T>().ok().map(|value| *value)
    }

    let target = TypeId::of::<T>();
    match value {
        incular_devtools_protocol::EditableValue::Bool(value) if target == TypeId::of::<bool>() => {
            cast(*value)
        }
        incular_devtools_protocol::EditableValue::Int(value) if target == TypeId::of::<i64>() => {
            cast(*value)
        }
        incular_devtools_protocol::EditableValue::Uint(value) if target == TypeId::of::<u64>() => {
            cast(*value)
        }
        incular_devtools_protocol::EditableValue::Float(value) if target == TypeId::of::<f64>() => {
            cast(*value)
        }
        incular_devtools_protocol::EditableValue::Str(value)
            if target == TypeId::of::<String>() =>
        {
            cast(value.clone())
        }
        _ => None,
    }
}

impl<T: 'static> Signal<T> {
    /// Creates single-threaded application state. It attaches to the runtime
    /// that first reads it during a build; a signal is therefore not shared
    /// between independent applications.
    #[must_use]
    pub fn new(value: T) -> Self {
        Self {
            inner: Rc::new(SignalInner {
                value: RefCell::new(value),
                dependents: RefCell::new(HashMap::new()),
                queues: RefCell::new(HashMap::new()),
                node_dependents: RefCell::new(HashMap::new()),
                node_queues: RefCell::new(HashMap::new()),
                dev_write_count: std::cell::Cell::new(0),
                #[cfg(feature = "devtools")]
                dev_signal_id: std::cell::Cell::new(None),
                #[cfg(feature = "devtools")]
                dev_name: RefCell::new(None),
                #[cfg(feature = "devtools")]
                dev_editable_kind: std::cell::Cell::new(None),
                #[cfg(feature = "devtools")]
                dev_last_write: RefCell::new((None, None)),
                #[cfg(feature = "devtools")]
                dev_summarize: RefCell::new(None),
            }),
        }
    }

    #[must_use]
    pub fn with_runtime(value: T, _runtime: &Runtime) -> Self {
        Self::new(value)
    }
    /// Attaches a DevTools-visible debug name and registers the signal in
    /// the DevTools registry. Requires `T: Debug` for value summaries.
    #[cfg(feature = "devtools")]
    pub fn devtools(self, name: &str) -> Self
    where
        T: std::fmt::Debug + 'static,
    {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        self.inner.dev_signal_id.set(Some(id));
        *self.inner.dev_name.borrow_mut() = Some(name.to_owned());
        *self.inner.dev_summarize.borrow_mut() = Some(Box::new(|value: &T| truncate_debug(value)));

        let weak_value = Rc::downgrade(&self.inner);
        let weak_value_count = weak_value.clone();
        let weak_value_subscribers = weak_value.clone();
        let weak_value_subscriber_entries = weak_value.clone();
        let registration = crate::devtools_registry::SignalRegistration {
            name: Some(name.to_owned()),
            type_name: std::any::type_name::<T>(),
            editable_kind: None,
            write_count: Box::new(move || {
                weak_value_count
                    .upgrade()
                    .map(|inner| inner.dev_write_count.get())
                    .unwrap_or_default()
            }),
            subscriber_count: Box::new(move || {
                weak_value_subscribers
                    .upgrade()
                    .map(|inner| inner.dependents.borrow().values().map(HashSet::len).sum())
                    .unwrap_or_default()
            }),
            subscribers: Box::new(move || {
                weak_value_subscriber_entries
                    .upgrade()
                    .map(|inner| {
                        inner
                            .dependents
                            .borrow()
                            .iter()
                            .flat_map(|(root, elements)| {
                                elements.iter().copied().map(|element| (*root, element))
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            }),
            last_write: Box::new(move || {
                weak_value.upgrade().map_or((None, None), |inner| {
                    let guard = inner.dev_last_write.borrow();
                    (guard.0.clone(), guard.1.clone())
                })
            }),
            apply_edit: Box::new(move |_| false),
        };
        crate::devtools_registry::register(id, registration);
        self
    }

    /// Marks this named signal development-editable from DevTools. Only the
    /// supported primitive kinds can round-trip an `EditableValue`.
    #[cfg(feature = "devtools")]
    pub fn devtools_editable(self) -> Self
    where
        T: std::fmt::Debug + PartialEq + 'static,
    {
        let kind = if std::any::TypeId::of::<T>() == std::any::TypeId::of::<bool>() {
            Some(EditableSignalKind::Bool)
        } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<i64>() {
            Some(EditableSignalKind::Int)
        } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<u64>() {
            Some(EditableSignalKind::Uint)
        } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<f64>() {
            Some(EditableSignalKind::Float)
        } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<String>() {
            Some(EditableSignalKind::Str)
        } else {
            None
        };
        self.inner.dev_editable_kind.set(kind);
        let weak_value = Rc::downgrade(&self.inner);
        // Re-register with an explicit, typed conversion closure. The closure
        // can only create the documented primitive types, then uses Signal's
        // ordinary UI-thread mutation path and dependency invalidation.
        crate::devtools_registry::set_editable(
            self.inner.dev_signal_id.get().unwrap_or_default(),
            kind,
            Box::new(move |value| {
                let Some(value) = editable_signal_value::<T>(value) else {
                    return false;
                };
                let Some(inner) = weak_value.upgrade() else {
                    return false;
                };
                Signal { inner }.set(value)
            }),
        );
        self
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        scheduler_counters::SIGNAL_READS.fetch_add(1, Ordering::Relaxed);
        reactive::track_dependency(self.inner.clone());
        self.inner.value.borrow().clone()
    }
    /// Replaces the value and invalidates dependents when it changed.
    ///
    /// Reactive state must be changed from an event callback, effect, or
    /// action. Mutating a signal while a widget builder is running is
    /// rejected because builders must remain side-effect free.
    pub fn set(&self, value: T) -> bool
    where
        T: PartialEq,
    {
        assert_reactive_mutation_allowed();
        if *self.inner.value.borrow() == value {
            return false;
        }
        #[cfg(feature = "devtools")]
        let old_summary = self
            .inner
            .dev_summarize
            .borrow()
            .as_ref()
            .map(|format| format(&self.inner.value.borrow()));
        #[cfg(feature = "devtools")]
        let new_summary = self
            .inner
            .dev_summarize
            .borrow()
            .as_ref()
            .map(|format| format(&value));
        *self.inner.value.borrow_mut() = value;
        #[cfg(feature = "devtools")]
        if old_summary.is_some() || new_summary.is_some() {
            *self.inner.dev_last_write.borrow_mut() = (old_summary, new_summary);
        }
        self.enqueue_dependents();
        true
    }
    pub fn dependent_count(&self) -> usize {
        self.inner
            .dependents
            .borrow()
            .values()
            .map(HashSet::len)
            .sum()
    }
    /// Mutates the value once and schedules only the Elements that read it.
    ///
    /// See [`Signal::set`] for the builder-side-effect rule.
    pub fn update(&self, update: impl FnOnce(&mut T)) {
        assert_reactive_mutation_allowed();
        #[cfg(feature = "devtools")]
        let old_summary = self
            .inner
            .dev_summarize
            .borrow()
            .as_ref()
            .map(|format| format(&self.inner.value.borrow()));
        update(&mut self.inner.value.borrow_mut());
        #[cfg(feature = "devtools")]
        {
            let new_summary = self
                .inner
                .dev_summarize
                .borrow()
                .as_ref()
                .map(|format| format(&self.inner.value.borrow()));
            if old_summary.is_some() || new_summary.is_some() {
                *self.inner.dev_last_write.borrow_mut() = (old_summary, new_summary);
            }
        }
        self.enqueue_dependents();
    }

    /// Mutates the value and invalidates dependents only when the value
    /// changed. This is the equality-aware counterpart to [`Signal::update`]
    /// for mutable values that implement [`Clone`] and [`PartialEq`].
    pub fn update_if_changed(&self, update: impl FnOnce(&mut T)) -> bool
    where
        T: Clone + PartialEq,
    {
        assert_reactive_mutation_allowed();
        let previous = self.inner.value.borrow().clone();
        #[cfg(feature = "devtools")]
        let old_summary = self
            .inner
            .dev_summarize
            .borrow()
            .as_ref()
            .map(|format| format(&previous));
        update(&mut self.inner.value.borrow_mut());
        if *self.inner.value.borrow() == previous {
            return false;
        }
        #[cfg(feature = "devtools")]
        {
            let new_summary = self
                .inner
                .dev_summarize
                .borrow()
                .as_ref()
                .map(|format| format(&self.inner.value.borrow()));
            if old_summary.is_some() || new_summary.is_some() {
                *self.inner.dev_last_write.borrow_mut() = (old_summary, new_summary);
            }
        }
        self.enqueue_dependents();
        true
    }

    fn enqueue_dependents(&self) {
        self.inner
            .dev_write_count
            .set(self.inner.dev_write_count.get() + 1);
        scheduler_counters::SIGNAL_WRITES.fetch_add(1, Ordering::Relaxed);
        let mut enqueued = 0_usize;
        let dependencies: Vec<_> = self
            .inner
            .dependents
            .borrow()
            .iter()
            .map(|(root, elements)| (*root, elements.iter().copied().collect::<Vec<_>>()))
            .collect();
        let nodes: Vec<_> = self
            .inner
            .node_dependents
            .borrow()
            .iter()
            .map(|(root, nodes)| {
                (
                    *root,
                    nodes
                        .iter()
                        .map(|(id, node)| (*id, node.clone()))
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        let queues = self.inner.queues.borrow();
        #[cfg(feature = "devtools")]
        let cause = self.inner.dev_signal_id.get().map(|id| {
            let (old, new) = self.inner.dev_last_write.borrow().clone();
            InvalidationCause::Signal {
                id,
                name: self.inner.dev_name.borrow().clone(),
                old,
                new,
            }
        });
        for (root, elements) in dependencies {
            if let Some(queue) = queues.get(&root).and_then(Weak::upgrade) {
                let mut queue = queue.borrow_mut();
                for element in elements {
                    let newly_queued = queue.enqueue(element);
                    #[cfg(feature = "devtools")]
                    {
                        if DEV_TASK_COMPLETION.with(Cell::get) {
                            queue.note_cause(element, InvalidationCause::TaskCompletion);
                        }
                        if let Some(cause) = &cause {
                            queue.note_cause(element, cause.clone());
                        }
                    }
                    if newly_queued {
                        enqueued += 1;
                    }
                }
            }
        }
        let node_queues = self.inner.node_queues.borrow();
        for (root, nodes) in nodes {
            if let Some(queue) = node_queues.get(&root).and_then(Weak::upgrade) {
                let mut queue = queue.borrow_mut();
                for (node_id, weak_node) in nodes {
                    let newly_queued = queue.enqueue_node(node_id, weak_node.clone());
                    if newly_queued && let Some(node) = weak_node.upgrade() {
                        node.mark_dirty();
                    }
                }
            }
        }
        if enqueued > 0 {
            scheduler_counters::DEPENDENTS_ENQUEUED.fetch_add(enqueued as u64, Ordering::Relaxed);
        }
    }
}

#[cfg(feature = "devtools")]
pub mod devtools_registry {
    pub use super::SignalRegistration;

    use std::cell::RefCell;
    std::thread_local! {
        static SIGNALS: RefCell<Vec<(u64, SignalRegistration)>> = const { RefCell::new(Vec::new()) };
    }

    pub fn register(id: u64, registration: SignalRegistration) {
        SIGNALS.with(|signals| signals.borrow_mut().push((id, registration)));
    }

    pub fn set_editable(
        id: u64,
        kind: Option<super::EditableSignalKind>,
        apply_edit: Box<dyn Fn(&incular_devtools_protocol::EditableValue) -> bool>,
    ) {
        SIGNALS.with(|signals| {
            for (registered_id, registration) in signals.borrow_mut().iter_mut() {
                if *registered_id == id {
                    registration.editable_kind = kind;
                    registration.apply_edit = apply_edit;
                    return;
                }
            }
        });
    }

    pub fn with_all<R>(visit: impl FnOnce(&[(u64, SignalRegistration)]) -> R) -> R {
        SIGNALS.with(|signals| visit(&signals.borrow()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditableSignalKind {
    Bool,
    Int,
    Uint,
    Float,
    Str,
}

#[cfg_attr(not(feature = "devtools"), allow(dead_code))]
pub struct SignalRegistration {
    pub name: Option<String>,
    pub type_name: &'static str,
    pub editable_kind: Option<EditableSignalKind>,
    pub write_count: Box<dyn Fn() -> u64>,
    pub subscriber_count: Box<dyn Fn() -> usize>,
    pub subscribers: Box<dyn Fn() -> Vec<(ReactiveRootId, ElementId)>>,
    pub last_write: Box<dyn Fn() -> (Option<String>, Option<String>)>,
    #[cfg(feature = "devtools")]
    pub apply_edit: Box<dyn Fn(&incular_devtools_protocol::EditableValue) -> bool>,
}
