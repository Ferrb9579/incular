//! Additional reactive primitives built on top of the runtime's retained
//! element dependency queue.
//!
//! `Signal` remains the inexpensive direct `Signal -> Element` path.  The
//! primitives in this module add graph nodes only when an application opts in
//! to derived values or external work:
//!
//! * [`Memo`] caches an expensive derived value and only propagates changes in
//!   its result.
//! * [`Effect`] runs an owner-mounted side effect after a reactive dependency
//!   changes.
//! * [`Action`] owns an explicitly dispatched asynchronous operation and its
//!   UI-observable state.

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    future::Future,
    mem,
    pin::Pin,
    rc::{Rc, Weak},
};

use super::{
    BuildScope, Dependency, ElementId, ReactiveNode, ReactiveNodeDependents, ReactiveNodeId,
    ReactiveQueue, ReactiveRootId, Signal, TaskFailure, TaskHandle,
};

#[derive(Clone)]
struct ReactiveBinding {
    root: ReactiveRootId,
    queue: Weak<RefCell<ReactiveQueue>>,
    spawner: Option<super::tasks::RuntimeSpawner>,
    owner_scope: Option<super::tasks::TaskScope>,
}

struct TrackingScope {
    binding: ReactiveBinding,
    node: Rc<dyn ReactiveNode>,
}

thread_local! {
    static TRACKING_STACK: RefCell<Vec<TrackingScope>> = const { RefCell::new(Vec::new()) };
}

fn binding_from_build_scope(scope: &BuildScope) -> ReactiveBinding {
    ReactiveBinding {
        root: scope.root,
        queue: scope.queue.clone(),
        spawner: Some(scope.spawner.clone()),
        owner_scope: Some(scope.owner_scope.clone()),
    }
}

fn current_binding() -> Option<ReactiveBinding> {
    TRACKING_STACK
        .with(|stack| stack.borrow().last().map(|scope| scope.binding.clone()))
        .or_else(|| {
            super::BUILD_SCOPE.with(|scope| scope.borrow().as_ref().map(binding_from_build_scope))
        })
}

fn current_element() -> Option<ElementId> {
    super::BUILD_SCOPE.with(|scope| scope.borrow().as_ref().and_then(|scope| scope.element))
}

fn current_node() -> Option<ReactiveNodeId> {
    TRACKING_STACK.with(|stack| stack.borrow().last().map(|scope| scope.node.id()))
}

/// Records a source read against either the currently computing graph node or
/// the current retained element. This is the common hook used by `Signal` and
/// all derived reactive values.
pub(super) fn track_dependency(dependency: Rc<dyn Dependency>) {
    let tracking = TRACKING_STACK.with(|stack| {
        stack.borrow().last().map(|scope| {
            (
                scope.binding.root,
                scope.binding.queue.clone(),
                scope.node.clone(),
            )
        })
    });
    if let Some((root, queue, node)) = tracking {
        let id = node.id();
        dependency.subscribe_node(root, id, Rc::downgrade(&node), queue);
        node.record_dependency(Rc::downgrade(&dependency));
        return;
    }

    super::BUILD_SCOPE.with(|scope| {
        let current = scope.borrow();
        let Some(scope) = current.as_ref() else {
            return;
        };
        if let Some(element) = scope.element {
            dependency.subscribe(scope.root, element, scope.queue.clone());
            if let Some(queue) = scope.queue.upgrade() {
                queue
                    .borrow_mut()
                    .record(element, Rc::downgrade(&dependency));
            }
        } else if let Some(initial_dependencies) = &scope.initial_dependencies {
            initial_dependencies.borrow_mut().record_signal(dependency);
        }
    });
}

struct TrackingGuard;

impl Drop for TrackingGuard {
    fn drop(&mut self) {
        TRACKING_STACK.with(|stack| {
            stack.borrow_mut().pop();
        });
    }
}

fn with_tracking<R>(
    binding: ReactiveBinding,
    node: Rc<dyn ReactiveNode>,
    compute: impl FnOnce() -> R,
) -> R {
    TRACKING_STACK.with(|stack| {
        stack.borrow_mut().push(TrackingScope { binding, node });
    });
    let guard = TrackingGuard;
    let result = compute();
    drop(guard);
    result
}

fn detach_sources(
    id: ReactiveNodeId,
    sources: &RefCell<Vec<(ReactiveRootId, Weak<dyn Dependency>)>>,
) {
    let previous = mem::take(&mut *sources.borrow_mut());
    for (root, dependency) in previous {
        if let Some(dependency) = dependency.upgrade() {
            dependency.remove_node(root, id);
        }
    }
}

fn detach_sources_for(
    id: ReactiveNodeId,
    root: ReactiveRootId,
    sources: &RefCell<HashMap<ReactiveRootId, Vec<Weak<dyn Dependency>>>>,
) {
    let previous = sources.borrow_mut().remove(&root).unwrap_or_default();
    for dependency in previous {
        if let Some(dependency) = dependency.upgrade() {
            dependency.remove_node(root, id);
        }
    }
}

fn notify_dependents(
    element_dependents: &RefCell<HashMap<ReactiveRootId, HashSet<ElementId>>>,
    element_queues: &RefCell<HashMap<ReactiveRootId, Weak<RefCell<ReactiveQueue>>>>,
    node_dependents: &RefCell<ReactiveNodeDependents>,
    node_queues: &RefCell<HashMap<ReactiveRootId, Weak<RefCell<ReactiveQueue>>>>,
) {
    let skip_element = current_element();
    let skip_node = current_node();
    let elements: Vec<_> = element_dependents
        .borrow()
        .iter()
        .map(|(root, elements)| (*root, elements.iter().copied().collect::<Vec<_>>()))
        .collect();
    let queues = element_queues.borrow();
    for (root, elements) in elements {
        if let Some(queue) = queues.get(&root).and_then(Weak::upgrade) {
            let mut queue = queue.borrow_mut();
            for element in elements {
                if Some(element) != skip_element {
                    queue.enqueue(element);
                }
            }
        }
    }

    let nodes: Vec<_> = node_dependents
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
    let queues = node_queues.borrow();
    for (root, nodes) in nodes {
        if let Some(queue) = queues.get(&root).and_then(Weak::upgrade) {
            let mut queue = queue.borrow_mut();
            for (id, node) in nodes {
                if Some(id) != skip_node {
                    let newly_queued = queue.enqueue_node(id, node.clone());
                    if newly_queued && let Some(node) = node.upgrade() {
                        node.mark_dirty();
                    }
                }
            }
        }
    }
}

struct BoolGuard<'a>(&'a Cell<bool>);

impl Drop for BoolGuard<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

struct MemoInner<T> {
    id: ReactiveNodeId,
    value: RefCell<Option<T>>,
    compute: RefCell<Box<dyn FnMut() -> T>>,
    dirty: Cell<bool>,
    evaluating: Cell<bool>,
    runtime: RefCell<HashMap<ReactiveRootId, ReactiveBinding>>,
    sources: RefCell<HashMap<ReactiveRootId, Vec<Weak<dyn Dependency>>>>,
    self_ref: RefCell<Weak<MemoInner<T>>>,
    dependents: RefCell<HashMap<ReactiveRootId, HashSet<ElementId>>>,
    queues: RefCell<HashMap<ReactiveRootId, Weak<RefCell<ReactiveQueue>>>>,
    node_dependents: RefCell<ReactiveNodeDependents>,
    node_queues: RefCell<HashMap<ReactiveRootId, Weak<RefCell<ReactiveQueue>>>>,
}

impl<T> MemoInner<T>
where
    T: PartialEq + 'static,
{
    fn new(compute: impl FnMut() -> T + 'static) -> Rc<Self> {
        let inner = Rc::new(Self {
            id: super::NEXT_REACTIVE_NODE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            value: RefCell::new(None),
            compute: RefCell::new(Box::new(compute)),
            dirty: Cell::new(true),
            evaluating: Cell::new(false),
            runtime: RefCell::new(HashMap::new()),
            sources: RefCell::new(HashMap::new()),
            self_ref: RefCell::new(Weak::new()),
            dependents: RefCell::new(HashMap::new()),
            queues: RefCell::new(HashMap::new()),
            node_dependents: RefCell::new(HashMap::new()),
            node_queues: RefCell::new(HashMap::new()),
        });
        *inner.self_ref.borrow_mut() = Rc::downgrade(&inner);
        inner
    }

    fn ensure_fresh(self: &Rc<Self>) {
        let current = current_binding();
        let bound = self.runtime.borrow();
        let needs_tracking = current
            .as_ref()
            .is_some_and(|binding| !bound.contains_key(&binding.root));
        drop(bound);
        if (self.value.borrow().is_none() || self.dirty.get() || needs_tracking)
            && self.recompute(current)
        {
            self.notify_dependents();
        }
    }

    fn recompute(self: &Rc<Self>, current: Option<ReactiveBinding>) -> bool {
        if self.evaluating.replace(true) {
            return false;
        }
        let _evaluating = BoolGuard(&self.evaluating);
        let binding = current.or_else(|| self.runtime.borrow().values().next().cloned());
        if let Some(binding) = &binding {
            detach_sources_for(self.id, binding.root, &self.sources);
        }
        let value = if let Some(binding) = binding.clone() {
            let node: Rc<dyn ReactiveNode> = self.clone();
            with_tracking(binding, node, || (self.compute.borrow_mut())())
        } else {
            (self.compute.borrow_mut())()
        };
        let changed = {
            let mut previous = self.value.borrow_mut();
            let changed = match previous.as_ref() {
                Some(previous) => previous != &value,
                None => true,
            };
            *previous = Some(value);
            changed
        };
        self.dirty.set(false);
        if let Some(binding) = binding {
            self.runtime.borrow_mut().insert(binding.root, binding);
        }
        changed
    }

    fn notify_dependents(&self) {
        notify_dependents(
            &self.dependents,
            &self.queues,
            &self.node_dependents,
            &self.node_queues,
        );
    }

    fn register_node(&self, queue: &Rc<RefCell<ReactiveQueue>>) {
        let Some(this) = self.self_ref.borrow().upgrade() else {
            return;
        };
        let node: Rc<dyn ReactiveNode> = this;
        queue
            .borrow_mut()
            .register_node(self.id, Rc::downgrade(&node));
    }

    fn dispose_root(&self, root: ReactiveRootId) {
        detach_sources_for(self.id, root, &self.sources);
        self.runtime.borrow_mut().remove(&root);
        self.queues.borrow_mut().remove(&root);
        self.node_queues.borrow_mut().remove(&root);
        if self.runtime.borrow().is_empty() {
            self.dirty.set(true);
        }
    }

    fn record_source(&self, dependency: Weak<dyn Dependency>) {
        let root = current_binding().map(|binding| binding.root);
        let Some(root) = root.or_else(|| self.runtime.borrow().keys().next().copied()) else {
            return;
        };
        let mut sources = self.sources.borrow_mut();
        let entries = sources.entry(root).or_default();
        if !entries.iter().any(|source| source.ptr_eq(&dependency)) {
            entries.push(dependency);
        }
    }
}

impl<T> ReactiveNode for MemoInner<T>
where
    T: PartialEq + 'static,
{
    fn id(&self) -> ReactiveNodeId {
        self.id
    }

    fn mark_dirty(&self) {
        self.dirty.set(true);
    }

    fn run(&self, root: ReactiveRootId) {
        if !self.dirty.get() {
            return;
        }
        let Some(this) = self.self_ref.borrow().upgrade() else {
            return;
        };
        let binding = self.runtime.borrow().get(&root).cloned();
        if this.recompute(binding) {
            this.notify_dependents();
        }
    }

    fn record_dependency(&self, dependency: Weak<dyn Dependency>) {
        self.record_source(dependency);
    }

    fn dispose(&self, root: ReactiveRootId) {
        self.dispose_root(root);
    }
}

impl<T> Dependency for MemoInner<T>
where
    T: PartialEq + 'static,
{
    fn subscribe(
        &self,
        root: ReactiveRootId,
        element: ElementId,
        queue: Weak<RefCell<ReactiveQueue>>,
    ) {
        if let Some(queue) = queue.upgrade() {
            self.register_node(&queue);
        }
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
        if let Some(queue_ref) = queue.upgrade() {
            self.register_node(&queue_ref);
            queue_ref
                .borrow_mut()
                .register_node(node, subscriber.clone());
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

/// A lazily evaluated, cached value derived from one or more [`super::Signal`]s
/// or other reactive values.
#[derive(Clone)]
pub struct Memo<T> {
    inner: Rc<MemoInner<T>>,
}

impl<T> Memo<T>
where
    T: PartialEq + 'static,
{
    /// Creates a memo. The function is not evaluated until the memo is read.
    #[must_use]
    pub fn new(compute: impl Fn() -> T + 'static) -> Self {
        Self {
            inner: MemoInner::new(compute),
        }
    }
}

impl<T> Memo<T>
where
    T: Clone + PartialEq + 'static,
{
    /// Reads the current value and subscribes the current builder or reactive
    /// computation to this memo.
    pub fn get(&self) -> T {
        track_dependency(self.inner.clone());
        self.inner.ensure_fresh();
        self.inner
            .value
            .borrow()
            .as_ref()
            .expect("memo value initialized after evaluation")
            .clone()
    }

    /// Reads the value without subscribing the current builder. The memo's
    /// own source dependencies remain tracked when it needs evaluation.
    pub fn get_untracked(&self) -> T {
        self.inner.ensure_fresh();
        self.inner
            .value
            .borrow()
            .as_ref()
            .expect("memo value initialized after evaluation")
            .clone()
    }

    /// Reads by reference after ensuring that the memo is current.
    pub fn with<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        track_dependency(self.inner.clone());
        self.inner.ensure_fresh();
        let value = self.inner.value.borrow();
        read(
            value
                .as_ref()
                .expect("memo value initialized after evaluation"),
        )
    }

    /// Reference-based untracked read.
    pub fn with_untracked<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        self.inner.ensure_fresh();
        let value = self.inner.value.borrow();
        read(
            value
                .as_ref()
                .expect("memo value initialized after evaluation"),
        )
    }

    #[must_use]
    pub fn dependent_count(&self) -> usize {
        self.inner
            .dependents
            .borrow()
            .values()
            .map(HashSet::len)
            .sum()
    }
}

struct EffectInner {
    id: ReactiveNodeId,
    callback: RefCell<Box<dyn FnMut()>>,
    dirty: Cell<bool>,
    running: Cell<bool>,
    mounted: Cell<bool>,
    runtime: RefCell<Option<ReactiveBinding>>,
    sources: RefCell<Vec<(ReactiveRootId, Weak<dyn Dependency>)>>,
    self_ref: RefCell<Weak<EffectInner>>,
}

impl EffectInner {
    fn new(callback: impl FnMut() + 'static) -> Rc<Self> {
        let inner = Rc::new(Self {
            id: super::NEXT_REACTIVE_NODE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            callback: RefCell::new(Box::new(callback)),
            dirty: Cell::new(false),
            running: Cell::new(false),
            mounted: Cell::new(false),
            runtime: RefCell::new(None),
            sources: RefCell::new(Vec::new()),
            self_ref: RefCell::new(Weak::new()),
        });
        *inner.self_ref.borrow_mut() = Rc::downgrade(&inner);
        inner
    }

    fn mount(&self, binding: ReactiveBinding) -> bool {
        let same_root = self
            .runtime
            .borrow()
            .as_ref()
            .is_some_and(|current| current.root == binding.root);
        if self.mounted.get() && same_root {
            return true;
        }
        if self.mounted.get() {
            detach_sources(self.id, &self.sources);
        }
        self.mounted.set(true);
        self.dirty.set(true);
        *self.runtime.borrow_mut() = Some(binding.clone());
        let Some(queue) = binding.queue.upgrade() else {
            self.mounted.set(false);
            self.dirty.set(false);
            *self.runtime.borrow_mut() = None;
            return false;
        };
        let node: Rc<dyn ReactiveNode> = self
            .self_ref
            .borrow()
            .upgrade()
            .expect("mounted effect still alive");
        queue
            .borrow_mut()
            .enqueue_node(self.id, Rc::downgrade(&node));
        true
    }

    fn run(&self) {
        if !self.mounted.get() || !self.dirty.get() || self.running.replace(true) {
            return;
        }
        let _running = BoolGuard(&self.running);
        self.dirty.set(false);
        let Some(binding) = self.runtime.borrow().clone() else {
            return;
        };
        detach_sources(self.id, &self.sources);
        let node: Rc<dyn ReactiveNode> = self
            .self_ref
            .borrow()
            .upgrade()
            .expect("running effect still alive");
        with_tracking(binding, node, || (self.callback.borrow_mut())());
    }

    fn record_source(&self, dependency: Weak<dyn Dependency>) {
        let Some(root) = self.runtime.borrow().as_ref().map(|binding| binding.root) else {
            return;
        };
        let mut sources = self.sources.borrow_mut();
        if !sources
            .iter()
            .any(|(source_root, source)| *source_root == root && source.ptr_eq(&dependency))
        {
            sources.push((root, dependency));
        }
    }
}

impl ReactiveNode for EffectInner {
    fn id(&self) -> ReactiveNodeId {
        self.id
    }

    fn mark_dirty(&self) {
        self.dirty.set(true);
    }

    fn run(&self, _root: ReactiveRootId) {
        Self::run(self);
    }

    fn record_dependency(&self, dependency: Weak<dyn Dependency>) {
        self.record_source(dependency);
    }

    fn dispose(&self, root: ReactiveRootId) {
        let owns_root = self
            .runtime
            .borrow()
            .as_ref()
            .is_some_and(|binding| binding.root == root);
        if owns_root {
            detach_sources(self.id, &self.sources);
            self.mounted.set(false);
            self.dirty.set(false);
            *self.runtime.borrow_mut() = None;
        }
    }
}

/// An owner-mounted side effect. Effects never run as part of a widget build;
/// [`Effect::mount`] queues the initial run for the runtime's reactive phase.
#[derive(Clone)]
pub struct Effect {
    inner: Rc<EffectInner>,
}

impl Effect {
    #[must_use]
    pub fn new(callback: impl FnMut() + 'static) -> Self {
        Self {
            inner: EffectInner::new(callback),
        }
    }

    /// Mounts this effect into the currently executing Incular builder. It is
    /// idempotent, so a stable handle can safely call this from a rebuilding
    /// root without creating duplicate effects.
    pub fn mount(&self) -> bool {
        let Some(binding) = current_binding() else {
            return false;
        };
        self.inner.mount(binding)
    }

    #[must_use]
    pub fn is_mounted(&self) -> bool {
        self.inner.mounted.get()
    }

    pub fn dispose(&self) {
        if let Some(root) = self
            .inner
            .runtime
            .borrow()
            .as_ref()
            .map(|binding| binding.root)
        {
            self.inner.dispose(root);
        }
    }
}

/// Error returned when an action is dispatched before it has been observed by
/// or mounted into an Incular application.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionDispatchError {
    NotMounted,
}

/// Failure from an [`Action`] operation or from the Incular task runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionError<E> {
    Operation(E),
    Runtime(TaskFailure),
}

/// UI-observable lifecycle state for an [`Action`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionState<T, E> {
    Idle,
    Loading,
    Ready(T),
    Error(ActionError<E>),
}

impl<T, E> ActionState<T, E> {
    #[must_use]
    pub const fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    #[must_use]
    pub const fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    #[must_use]
    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }

    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
}

type ActionFuture<O, E> = Pin<Box<dyn Future<Output = Result<O, E>> + Send + 'static>>;

#[derive(Clone)]
struct ActionBinding {
    root: ReactiveRootId,
    spawner: super::tasks::RuntimeSpawner,
    owner_scope: super::tasks::TaskScope,
}

struct ActionInner<I, O, E> {
    operation: Rc<dyn Fn(I) -> ActionFuture<O, E>>,
    state: Signal<ActionState<O, E>>,
    binding: RefCell<Option<ActionBinding>>,
    active: RefCell<Option<TaskHandle>>,
    generation: Cell<u64>,
}

impl<I, O, E> Drop for ActionInner<I, O, E> {
    fn drop(&mut self) {
        if let Some(active) = self.active.get_mut().take() {
            active.cancel();
        }
    }
}

/// An explicitly dispatched asynchronous operation with reactive lifecycle
/// state. The operation is never started merely because the action is read.
#[derive(Clone)]
pub struct Action<I, O, E> {
    inner: Rc<ActionInner<I, O, E>>,
}

impl<I, O, E> Action<I, O, E>
where
    I: 'static,
    O: Clone + Send + 'static,
    E: Clone + Send + 'static,
{
    /// Creates an action. `operation` is invoked only by [`Self::dispatch`].
    #[must_use]
    pub fn new<F, Fut>(operation: F) -> Self
    where
        F: Fn(I) -> Fut + 'static,
        Fut: Future<Output = Result<O, E>> + Send + 'static,
    {
        let operation: Rc<dyn Fn(I) -> ActionFuture<O, E>> =
            Rc::new(move |input| Box::pin(operation(input)));
        Self {
            inner: Rc::new(ActionInner {
                operation,
                state: Signal::new(ActionState::Idle),
                binding: RefCell::new(None),
                active: RefCell::new(None),
                generation: Cell::new(0),
            }),
        }
    }

    fn bind_current(&self) {
        let Some(binding) = current_binding() else {
            return;
        };
        let (Some(spawner), Some(owner_scope)) = (binding.spawner, binding.owner_scope) else {
            return;
        };
        let mut current = self.inner.binding.borrow_mut();
        let same_root = current.as_ref().is_some_and(|current| {
            current.root == binding.root && !current.owner_scope.is_cancelled()
        });
        if !same_root {
            *current = Some(ActionBinding {
                root: binding.root,
                spawner,
                owner_scope,
            });
        }
    }

    /// Mounts the action to the current builder's owner without requiring a
    /// `BuildContext` argument. Reading [`Self::state`] also mounts it.
    pub fn mount(&self) -> bool {
        self.bind_current();
        self.inner
            .binding
            .borrow()
            .as_ref()
            .is_some_and(|binding| !binding.owner_scope.is_cancelled())
    }

    /// Returns the current state and subscribes the current builder to it.
    pub fn state(&self) -> ActionState<O, E> {
        self.bind_current();
        self.inner.state.get()
    }

    /// Returns the underlying state signal for advanced composition.
    pub fn state_signal(&self) -> Signal<ActionState<O, E>> {
        self.bind_current();
        self.inner.state.clone()
    }

    /// Dispatches the operation. A second dispatch cancels the previous
    /// operation logically; a late completion cannot overwrite the latest
    /// result.
    pub fn dispatch(&self, input: I) -> Result<(), ActionDispatchError> {
        let binding = {
            let mut current = self.inner.binding.borrow_mut();
            let Some(binding) = current.clone() else {
                return Err(ActionDispatchError::NotMounted);
            };
            if binding.owner_scope.is_cancelled() {
                *current = None;
                return Err(ActionDispatchError::NotMounted);
            }
            binding
        };
        if let Some(active) = self.inner.active.borrow_mut().take() {
            active.cancel();
        }
        let generation = self.inner.generation.get().wrapping_add(1);
        self.inner.generation.set(generation);
        self.inner
            .state
            .update(|state| *state = ActionState::Loading);

        let operation = self.inner.operation.clone();
        let weak = Rc::downgrade(&self.inner);
        let handle = binding.spawner.spawn_into_in(
            &binding.owner_scope,
            operation(input),
            move |result, _runtime| {
                let Some(inner) = weak.upgrade() else {
                    return;
                };
                if inner.generation.get() != generation {
                    return;
                }
                inner.active.borrow_mut().take();
                inner.state.update(|state| {
                    *state = match result {
                        Ok(Ok(value)) => ActionState::Ready(value),
                        Ok(Err(error)) => ActionState::Error(ActionError::Operation(error)),
                        Err(error) => ActionState::Error(ActionError::Runtime(error)),
                    };
                });
            },
        );
        *self.inner.active.borrow_mut() = Some(handle);
        Ok(())
    }

    /// Cancels the active operation and exposes a runtime cancellation error.
    pub fn cancel(&self) {
        let generation = self.inner.generation.get().wrapping_add(1);
        self.inner.generation.set(generation);
        if let Some(active) = self.inner.active.borrow_mut().take() {
            active.cancel();
            self.inner.state.update(|state| {
                *state = ActionState::Error(ActionError::Runtime(TaskFailure::Cancelled));
            });
        }
    }
}
