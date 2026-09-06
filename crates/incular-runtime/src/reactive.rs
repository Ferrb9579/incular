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
    collections::HashMap,
    future::Future,
    pin::Pin,
    rc::{Rc, Weak},
};

use super::{
    BuildScope, Dependency, ElementId, ReactiveNode, ReactiveNodeId, ReactiveQueue, ReactiveRootId,
    Signal, TaskFailure, TaskHandle, TrackedDependency,
};

#[derive(Clone)]
struct ReactiveBinding {
    owner_element: Option<ElementId>,
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
        owner_element: scope.element,
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
        let tracked = subscribe_node(dependency, root, id, Rc::downgrade(&node), queue);
        node.record_dependency(tracked);
        return;
    }

    super::BUILD_SCOPE.with(|scope| {
        let current = scope.borrow();
        let Some(scope) = current.as_ref() else {
            dependency.source().track();
            return;
        };
        if let Some(element) = scope.element {
            let tracked = subscribe_element(dependency, scope.root, element, scope.queue.clone());
            if let Some(queue) = scope.queue.upgrade() {
                queue.borrow_mut().record(element, tracked);
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
    let _sources = incular_core::reactivity::TrackingGuard::enter(track_source);
    let result = compute();
    drop(guard);
    result
}

fn detach_sources(sources: &RefCell<Vec<(ReactiveRootId, TrackedDependency)>>) {
    sources.borrow_mut().clear();
}
fn detach_sources_for(
    root: ReactiveRootId,
    sources: &RefCell<HashMap<ReactiveRootId, Vec<TrackedDependency>>>,
) {
    sources.borrow_mut().remove(&root);
}

struct SourceDependency(incular_core::reactivity::DependencySource);
impl Dependency for SourceDependency {
    fn source(&self) -> &incular_core::reactivity::DependencySource {
        &self.0
    }
}
pub(super) fn track_source(source: incular_core::reactivity::DependencySource) {
    track_dependency(Rc::new(SourceDependency(source)));
}

pub(super) fn subscribe_element(
    dependency: Rc<dyn Dependency>,
    root: ReactiveRootId,
    element: ElementId,
    queue: Weak<RefCell<ReactiveQueue>>,
) -> TrackedDependency {
    if let Some(queue) = queue.upgrade() {
        dependency.prepare_subscription(&queue);
    }
    #[cfg(feature = "devtools")]
    let weak = Rc::downgrade(&dependency);
    let source = dependency.source().clone();
    #[cfg(feature = "devtools")]
    let callback_source = weak.clone();
    let is_signal = dependency.is_signal();
    let skips_current = dependency.skips_current_consumer();
    let subscription = source.subscribe((root, element), move || {
        if skips_current
            && current_element() == Some(element)
            && current_binding().is_some_and(|binding| binding.root == root)
        {
            return;
        }
        if let Some(queue) = queue.upgrade() {
            let mut queue = queue.borrow_mut();
            let newly_queued = queue.enqueue(element);
            if is_signal && newly_queued {
                super::scheduler_counters::DEPENDENTS_ENQUEUED
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            #[cfg(feature = "devtools")]
            {
                if super::environment::DEV_TASK_COMPLETION.with(Cell::get) {
                    queue.note_cause(
                        element,
                        incular_widgets::internal::InvalidationCause::TaskCompletion,
                    );
                }
                if let Some(cause) = callback_source
                    .upgrade()
                    .and_then(|dependency| dependency.cause())
                {
                    queue.note_cause(element, cause);
                }
            }
        }
    });
    TrackedDependency {
        source_id: dependency.source().id(),
        source: dependency.source().clone(),
        dependency: Rc::downgrade(&dependency),
        _subscription: subscription,
    }
}

fn subscribe_node(
    dependency: Rc<dyn Dependency>,
    root: ReactiveRootId,
    id: ReactiveNodeId,
    node: Weak<dyn ReactiveNode>,
    queue: Weak<RefCell<ReactiveQueue>>,
) -> TrackedDependency {
    if let Some(queue) = queue.upgrade() {
        dependency.prepare_subscription(&queue);
        queue.borrow_mut().register_node(id, node.clone());
    }
    let skips_current = dependency.skips_current_consumer();
    let subscription = dependency.source().subscribe((root, id), move || {
        if skips_current && current_node() == Some(id) {
            return;
        }
        if let (Some(queue), Some(live)) = (queue.upgrade(), node.upgrade()) {
            live.mark_dirty();
            queue.borrow_mut().enqueue_node(id, node.clone());
        }
    });
    TrackedDependency {
        source_id: dependency.source().id(),
        source: dependency.source().clone(),
        dependency: Rc::downgrade(&dependency),
        _subscription: subscription,
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
    sources: RefCell<HashMap<ReactiveRootId, Vec<TrackedDependency>>>,
    self_ref: RefCell<Weak<MemoInner<T>>>,
    source: incular_core::reactivity::DependencySource,
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
            source: incular_core::reactivity::DependencySource::default(),
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
        let unbound = self.runtime.borrow().is_empty() && current.is_none();
        if (unbound || self.value.borrow().is_none() || self.dirty.get() || needs_tracking)
            && self.recompute(current)
        {
            self.notify_dependents();
        }
    }

    fn recompute(self: &Rc<Self>, current: Option<ReactiveBinding>) -> bool {
        assert!(
            !self.evaluating.replace(true),
            "reactive memo dependency cycle"
        );
        let _evaluating = BoolGuard(&self.evaluating);
        let _read_only = incular_core::reactivity::ReadOnlyGuard::enter();
        let binding = current.or_else(|| self.runtime.borrow().values().next().cloned());
        if let Some(binding) = &binding {
            detach_sources_for(binding.root, &self.sources);
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
            self.runtime
                .borrow_mut()
                .insert(binding.root, binding.clone());
            // A shared memo has one value, so every root must observe the same
            // newly selected branch. Refresh other roots without recomputing.
            let dependencies = self
                .sources
                .borrow()
                .get(&binding.root)
                .map(|sources| {
                    sources
                        .iter()
                        .map(|tracked| {
                            tracked.dependency.upgrade().unwrap_or_else(|| {
                                Rc::new(SourceDependency(tracked.source.clone()))
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let others = self
                .runtime
                .borrow()
                .values()
                .filter(|other| other.root != binding.root)
                .cloned()
                .collect::<Vec<_>>();
            for other in others {
                let node: Rc<dyn ReactiveNode> = self.clone();
                let subscriptions = dependencies
                    .iter()
                    .map(|dependency| {
                        subscribe_node(
                            dependency.clone(),
                            other.root,
                            self.id,
                            Rc::downgrade(&node),
                            other.queue.clone(),
                        )
                    })
                    .collect();
                self.sources.borrow_mut().insert(other.root, subscriptions);
            }
        }
        changed
    }

    fn notify_dependents(&self) {
        self.source.notify();
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
        detach_sources_for(root, &self.sources);
        self.runtime.borrow_mut().remove(&root);
        if self.runtime.borrow().is_empty() {
            self.dirty.set(true);
        }
    }

    fn record_source(&self, dependency: TrackedDependency) {
        let root = current_binding().map(|binding| binding.root);
        let Some(root) = root.or_else(|| self.runtime.borrow().keys().next().copied()) else {
            return;
        };
        let mut sources = self.sources.borrow_mut();
        let entries = sources.entry(root).or_default();
        if !entries
            .iter()
            .any(|source| source.source_id == dependency.source_id)
        {
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

    fn record_dependency(&self, dependency: TrackedDependency) {
        self.record_source(dependency);
    }

    fn dispose(&self, root: ReactiveRootId) {
        self.dispose_root(root);
    }
}

impl<T: PartialEq + 'static> Dependency for MemoInner<T> {
    fn skips_current_consumer(&self) -> bool {
        true
    }
    fn source(&self) -> &incular_core::reactivity::DependencySource {
        &self.source
    }
    fn prepare_subscription(&self, queue: &Rc<RefCell<ReactiveQueue>>) {
        self.register_node(queue);
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
    /// Reads outside a runtime evaluate directly until a runtime owns its
    /// subscriptions; this avoids caching a value with no invalidation owner.
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
        self.inner.source.subscriber_count()
    }
}

struct EffectInner {
    id: ReactiveNodeId,
    callback: RefCell<Box<dyn FnMut()>>,
    dirty: Cell<bool>,
    running: Cell<bool>,
    mounted: Cell<bool>,
    runtime: RefCell<Option<ReactiveBinding>>,
    sources: RefCell<Vec<(ReactiveRootId, TrackedDependency)>>,
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
            detach_sources(&self.sources);
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
        detach_sources(&self.sources);
        let node: Rc<dyn ReactiveNode> = self
            .self_ref
            .borrow()
            .upgrade()
            .expect("running effect still alive");
        with_tracking(binding, node, || (self.callback.borrow_mut())());
    }

    fn record_source(&self, dependency: TrackedDependency) {
        let Some(root) = self.runtime.borrow().as_ref().map(|binding| binding.root) else {
            return;
        };
        let mut sources = self.sources.borrow_mut();
        if !sources.iter().any(|(source_root, source)| {
            *source_root == root && source.source_id == dependency.source_id
        }) {
            sources.push((root, dependency));
        }
    }
}

impl ReactiveNode for EffectInner {
    fn owns_element(&self, root: ReactiveRootId, element: ElementId) -> bool {
        self.runtime
            .borrow()
            .as_ref()
            .is_some_and(|binding| binding.root == root && binding.owner_element == Some(element))
    }
    fn id(&self) -> ReactiveNodeId {
        self.id
    }

    fn mark_dirty(&self) {
        self.dirty.set(true);
    }

    fn run(&self, _root: ReactiveRootId) {
        Self::run(self);
    }

    fn record_dependency(&self, dependency: TrackedDependency) {
        self.record_source(dependency);
    }

    fn dispose(&self, root: ReactiveRootId) {
        let owns_root = self
            .runtime
            .borrow()
            .as_ref()
            .is_some_and(|binding| binding.root == root);
        if owns_root {
            detach_sources(&self.sources);
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
        let root = self
            .inner
            .runtime
            .borrow()
            .as_ref()
            .map(|binding| binding.root);
        if let Some(root) = root {
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
