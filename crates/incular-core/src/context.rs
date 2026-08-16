//! Build contexts, inherited values, and dependency-aware signals.
//!
//! The context is deliberately scheduler agnostic. A runtime can associate a
//! [`ConsumerId`] with an element and use [`BuildContext::take_dirty`] to
//! enqueue it; core only records dependencies and reports invalidation.

use std::{
    any::{Any, TypeId},
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    fmt,
    rc::{Rc, Weak},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_CONSUMER: AtomicU64 = AtomicU64::new(1);
static NEXT_ENVIRONMENT: AtomicU64 = AtomicU64::new(1);
static NEXT_SIGNAL: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum DependencyKey {
    Signal(u64),
    Environment { environment: u64, type_id: TypeId },
}

#[derive(Default)]
struct TrackerInner {
    dependencies: RefCell<HashMap<ConsumerId, HashSet<DependencyKey>>>,
    subscribers: RefCell<HashMap<DependencyKey, HashSet<ConsumerId>>>,
    dirty: RefCell<HashSet<ConsumerId>>,
}

impl TrackerInner {
    fn record(&self, consumer: ConsumerId, dependency: DependencyKey) {
        let mut dependencies = self.dependencies.borrow_mut();
        if dependencies.entry(consumer).or_default().insert(dependency) {
            self.subscribers
                .borrow_mut()
                .entry(dependency)
                .or_default()
                .insert(consumer);
        }
    }

    fn clear(&self, consumer: ConsumerId) {
        let Some(dependencies) = self.dependencies.borrow_mut().remove(&consumer) else {
            return;
        };
        let mut subscribers = self.subscribers.borrow_mut();
        for dependency in dependencies {
            let Some(consumers) = subscribers.get_mut(&dependency) else {
                continue;
            };
            consumers.remove(&consumer);
            if consumers.is_empty() {
                subscribers.remove(&dependency);
            }
        }
        self.dirty.borrow_mut().remove(&consumer);
    }

    fn invalidate(&self, dependency: DependencyKey) {
        if let Some(consumers) = self.subscribers.borrow().get(&dependency) {
            self.dirty.borrow_mut().extend(consumers.iter().copied());
        }
    }

    fn dependency_count(&self, consumer: ConsumerId) -> usize {
        self.dependencies
            .borrow()
            .get(&consumer)
            .map_or(0, HashSet::len)
    }

    fn is_dirty(&self, consumer: ConsumerId) -> bool {
        self.dirty.borrow().contains(&consumer)
    }

    fn take_dirty(&self, consumer: ConsumerId) -> bool {
        self.dirty.borrow_mut().remove(&consumer)
    }
}

struct Environment {
    id: u64,
    parent: Option<Rc<Environment>>,
    values: RefCell<HashMap<TypeId, Rc<dyn Any>>>,
    revision: Cell<u64>,
}

impl Environment {
    fn root() -> Rc<Self> {
        Rc::new(Self {
            id: NEXT_ENVIRONMENT.fetch_add(1, Ordering::Relaxed),
            parent: None,
            values: RefCell::new(HashMap::new()),
            revision: Cell::new(0),
        })
    }

    fn child(parent: &Rc<Self>) -> Rc<Self> {
        Rc::new(Self {
            id: NEXT_ENVIRONMENT.fetch_add(1, Ordering::Relaxed),
            parent: Some(parent.clone()),
            values: RefCell::new(HashMap::new()),
            revision: Cell::new(0),
        })
    }

    fn lookup(&self, type_id: TypeId) -> Option<(u64, Rc<dyn Any>)> {
        if let Some(value) = self.values.borrow().get(&type_id) {
            return Some((self.id, value.clone()));
        }
        self.parent.as_deref()?.lookup(type_id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConsumerId(u64);

impl ConsumerId {
    #[must_use]
    pub fn new() -> Self {
        Self(NEXT_CONSUMER.fetch_add(1, Ordering::Relaxed))
    }

    #[must_use]
    pub const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl Default for ConsumerId {
    fn default() -> Self {
        Self::new()
    }
}

/// A read-only summary of the dependencies currently collected for a context.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DependencySnapshot {
    count: usize,
    dirty: bool,
}

impl DependencySnapshot {
    #[must_use]
    pub const fn count(self) -> usize {
        self.count
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.count == 0
    }

    #[must_use]
    pub const fn is_dirty(self) -> bool {
        self.dirty
    }
}

struct ActiveScope {
    tracker: Rc<TrackerInner>,
    consumer: ConsumerId,
}

thread_local! {
    static ACTIVE_SCOPES: RefCell<Vec<ActiveScope>> = const { RefCell::new(Vec::new()) };
}

/// A scoped build context. Cloning a context keeps the same consumer identity;
/// [`BuildContext::for_consumer`] creates a distinct dependency owner.
#[derive(Clone)]
pub struct BuildContext {
    environment: Rc<Environment>,
    tracker: Rc<TrackerInner>,
    consumer: ConsumerId,
}

impl fmt::Debug for BuildContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuildContext")
            .field("consumer", &self.consumer)
            .field("dependency_count", &self.dependency_count())
            .field("dirty", &self.is_dirty())
            .finish()
    }
}

impl Default for BuildContext {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildContext {
    /// Creates an empty root context with a fresh dependency consumer.
    #[must_use]
    pub fn new() -> Self {
        Self::for_consumer(ConsumerId::new())
    }

    /// Creates an empty root context for a caller-owned consumer identity.
    #[must_use]
    pub fn for_consumer(consumer: ConsumerId) -> Self {
        Self {
            environment: Environment::root(),
            tracker: Rc::new(TrackerInner::default()),
            consumer,
        }
    }

    /// Returns the identity used for dependency invalidation.
    #[must_use]
    pub const fn consumer_id(&self) -> ConsumerId {
        self.consumer
    }

    /// Creates a child environment. Values in the child shadow values in its
    /// parent while keeping the same dependency tracker and consumer.
    #[must_use]
    pub fn child(&self) -> Self {
        Self {
            environment: Environment::child(&self.environment),
            tracker: self.tracker.clone(),
            consumer: self.consumer,
        }
    }

    /// Creates a child context with one inherited value.
    #[must_use]
    pub fn provide<T: Any>(&self, value: T) -> Self {
        let child = self.child();
        child.insert(value);
        child
    }

    /// Inserts or replaces a value in this environment. Existing consumers
    /// that watched this type are marked dirty when the value changes.
    pub fn insert<T: Any>(&self, value: T) -> bool {
        let type_id = TypeId::of::<T>();
        self.environment
            .values
            .borrow_mut()
            .insert(type_id, Rc::new(value));
        self.environment
            .revision
            .set(self.environment.revision.get().wrapping_add(1));
        self.tracker.invalidate(DependencyKey::Environment {
            environment: self.environment.id,
            type_id,
        });
        true
    }

    /// Replaces a value only when its `PartialEq` value changed.
    pub fn set<T: Any + PartialEq>(&self, value: T) -> bool {
        let type_id = TypeId::of::<T>();
        let changed = self
            .environment
            .values
            .borrow()
            .get(&type_id)
            .and_then(|old| old.downcast_ref::<T>())
            .is_none_or(|old| old != &value);
        if changed {
            self.insert(value);
        }
        changed
    }

    /// Removes a value from this environment.
    pub fn remove<T: Any>(&self) -> bool {
        let type_id = TypeId::of::<T>();
        let removed = self
            .environment
            .values
            .borrow_mut()
            .remove(&type_id)
            .is_some();
        if removed {
            self.environment
                .revision
                .set(self.environment.revision.get().wrapping_add(1));
            self.tracker.invalidate(DependencyKey::Environment {
                environment: self.environment.id,
                type_id,
            });
        }
        removed
    }

    #[must_use]
    pub fn contains<T: Any>(&self) -> bool {
        self.lookup_value(TypeId::of::<T>()).is_some()
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.environment.revision.get()
    }

    fn lookup_value(&self, type_id: TypeId) -> Option<(u64, Rc<dyn Any>)> {
        self.environment.lookup(type_id)
    }

    /// Reads an inherited value without recording a dependency.
    #[must_use]
    pub fn read<T: Any + Clone>(&self) -> Option<T> {
        self.read_shared::<T>().map(|value| (*value).clone())
    }

    /// Alias for [`BuildContext::read`].
    #[must_use]
    pub fn get<T: Any + Clone>(&self) -> Option<T> {
        self.read()
    }

    /// Reads an inherited value as shared ownership without recording a
    /// dependency.
    #[must_use]
    pub fn read_shared<T: Any>(&self) -> Option<Rc<T>> {
        self.lookup_value(TypeId::of::<T>())
            .and_then(|(_, value)| value.downcast::<T>().ok())
    }

    fn record_environment(&self, type_id: TypeId) {
        let environment = self
            .lookup_value(type_id)
            .map_or(self.environment.id, |(environment, _)| environment);
        with_active_scope(|scope| {
            scope.tracker.record(
                scope.consumer,
                DependencyKey::Environment {
                    environment,
                    type_id,
                },
            );
        })
        .unwrap_or_else(|| {
            self.tracker.record(
                self.consumer,
                DependencyKey::Environment {
                    environment,
                    type_id,
                },
            )
        });
    }

    /// Reads a value and records the environment lookup as a dependency.
    #[must_use]
    pub fn depend<T: Any + Clone>(&self) -> Option<T> {
        self.record_environment(TypeId::of::<T>());
        self.read()
    }

    /// Alias for [`BuildContext::depend`], matching reactive UI terminology.
    #[must_use]
    pub fn watch<T: Any + Clone>(&self) -> Option<T> {
        self.depend()
    }

    /// Explicitly records a signal dependency for this context and returns its
    /// current value.
    #[must_use]
    pub fn watch_signal<T: Any + Clone>(&self, signal: &Signal<T>) -> T {
        signal.register(self);
        signal.get()
    }

    /// Enters this context as the active build scope. Signal reads made while
    /// the guard is alive are associated with this context's consumer.
    #[must_use]
    pub fn enter(&self) -> ContextGuard {
        ACTIVE_SCOPES.with(|scopes| {
            scopes.borrow_mut().push(ActiveScope {
                tracker: self.tracker.clone(),
                consumer: self.consumer,
            });
        });
        ContextGuard { active: true }
    }

    /// Alias for [`BuildContext::enter`].
    #[must_use]
    pub fn scope(&self) -> ContextGuard {
        self.enter()
    }

    /// Clears old dependencies, enters the build scope, and invokes the
    /// callback. This is the normal runtime integration point.
    pub fn build<R>(&self, callback: impl FnOnce(&BuildContext) -> R) -> R {
        self.clear_dependencies();
        let _guard = self.enter();
        callback(self)
    }

    /// Runs a closure in a fresh dependency collection. Use [`BuildContext::build`]
    /// when the callback needs the context as an argument.
    pub fn run<R>(&self, callback: impl FnOnce() -> R) -> R {
        self.clear_dependencies();
        let _guard = self.enter();
        callback()
    }

    /// Alias for [`BuildContext::build`].
    pub fn with_context<R>(&self, callback: impl FnOnce(&BuildContext) -> R) -> R {
        self.build(callback)
    }

    pub fn clear_dependencies(&self) {
        self.tracker.clear(self.consumer);
    }

    #[must_use]
    pub fn dependency_count(&self) -> usize {
        self.tracker.dependency_count(self.consumer)
    }

    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.tracker.is_dirty(self.consumer)
    }

    /// Takes and clears this consumer's invalidation bit.
    pub fn take_dirty(&self) -> bool {
        self.tracker.take_dirty(self.consumer)
    }

    #[must_use]
    pub fn dependencies(&self) -> DependencySnapshot {
        DependencySnapshot {
            count: self.dependency_count(),
            dirty: self.is_dirty(),
        }
    }
}

/// Restores the previous active build scope when dropped.
pub struct ContextGuard {
    active: bool,
}

impl Drop for ContextGuard {
    fn drop(&mut self) {
        if self.active {
            ACTIVE_SCOPES.with(|scopes| {
                let _ = scopes.borrow_mut().pop();
            });
            self.active = false;
        }
    }
}

fn with_active_scope<R>(callback: impl FnOnce(&ActiveScope) -> R) -> Option<R> {
    ACTIVE_SCOPES.with(|scopes| scopes.borrow().last().map(callback))
}

struct SignalInner<T> {
    id: u64,
    value: RefCell<T>,
    subscribers: RefCell<Vec<Weak<TrackerInner>>>,
    revision: Cell<u64>,
}

/// A cloneable, single-threaded reactive value. Reads inside a context scope
/// subscribe that context; writes mark subscribed consumers dirty.
#[derive(Clone)]
pub struct Signal<T: Any> {
    inner: Rc<SignalInner<T>>,
}

impl<T: Any> fmt::Debug for Signal<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Signal")
            .field("id", &self.id())
            .field("value", &self.inner.value.borrow())
            .field("revision", &self.revision())
            .finish()
    }
}

impl<T: Any> Signal<T> {
    #[must_use]
    pub fn new(value: T) -> Self {
        Self {
            inner: Rc::new(SignalInner {
                id: NEXT_SIGNAL.fetch_add(1, Ordering::Relaxed),
                value: RefCell::new(value),
                subscribers: RefCell::new(Vec::new()),
                revision: Cell::new(0),
            }),
        }
    }

    #[must_use]
    pub fn id(&self) -> u64 {
        self.inner.id
    }

    #[must_use]
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        with_active_scope(|scope| self.register_in(scope)).unwrap_or(());
        self.inner.value.borrow().clone()
    }

    /// Alias for [`Signal::get`].
    #[must_use]
    pub fn read(&self) -> T
    where
        T: Clone,
    {
        self.get()
    }

    /// Reads the current value without consulting the active dependency scope.
    #[must_use]
    pub fn peek(&self) -> T
    where
        T: Clone,
    {
        self.inner.value.borrow().clone()
    }

    fn register_in(&self, scope: &ActiveScope) {
        scope
            .tracker
            .record(scope.consumer, DependencyKey::Signal(self.id()));
        let weak = Rc::downgrade(&scope.tracker);
        let mut subscribers = self.inner.subscribers.borrow_mut();
        if !subscribers.iter().any(|current| current.ptr_eq(&weak)) {
            subscribers.push(weak);
        }
    }

    fn register(&self, context: &BuildContext) {
        self.register_in(&ActiveScope {
            tracker: context.tracker.clone(),
            consumer: context.consumer,
        });
    }

    pub fn set(&self, value: T) -> bool
    where
        T: PartialEq,
    {
        if *self.inner.value.borrow() == value {
            return false;
        }
        *self.inner.value.borrow_mut() = value;
        self.notify();
        true
    }

    /// Mutates the value and invalidates subscribed consumers once complete.
    pub fn update(&self, update: impl FnOnce(&mut T)) {
        update(&mut self.inner.value.borrow_mut());
        self.notify();
    }

    fn notify(&self) {
        self.inner
            .revision
            .set(self.inner.revision.get().wrapping_add(1));
        let dependency = DependencyKey::Signal(self.id());
        self.inner.subscribers.borrow_mut().retain(|subscriber| {
            let Some(tracker) = subscriber.upgrade() else {
                return false;
            };
            tracker.invalidate(dependency);
            true
        });
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.inner.revision.get()
    }

    #[must_use]
    pub fn dependent_count(&self) -> usize {
        self.inner
            .subscribers
            .borrow()
            .iter()
            .filter(|subscriber| subscriber.strong_count() > 0)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_reads_register_and_writes_invalidate() {
        let context = BuildContext::new();
        let signal = Signal::new(1_u32);
        context.build(|_| assert_eq!(signal.get(), 1));
        assert_eq!(context.dependency_count(), 1);
        assert!(!context.is_dirty());
        assert!(signal.set(2));
        assert!(context.is_dirty());
        assert!(context.take_dirty());
        assert!(!context.is_dirty());
    }

    #[test]
    fn inherited_values_track_the_environment_that_provided_them() {
        let root = BuildContext::new();
        let child = root.provide(7_u32);
        assert_eq!(child.build(|context| context.watch::<u32>()), Some(7));
        assert_eq!(child.dependency_count(), 1);
        assert!(!child.is_dirty());
        assert!(child.set(8_u32));
        assert!(child.is_dirty());
        assert_eq!(child.read::<u32>(), Some(8));
    }

    #[test]
    fn nested_scopes_restore_the_outer_context() {
        let outer = BuildContext::new();
        let inner = BuildContext::new();
        let signal = Signal::new(3_u32);
        outer.run(|| {
            let _guard = inner.enter();
            assert_eq!(signal.get(), 3);
        });
        assert_eq!(outer.dependency_count(), 0);
        assert_eq!(inner.dependency_count(), 1);
    }
}
