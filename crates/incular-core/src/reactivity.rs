//! Scheduler-independent dependency sources and owned subscriptions.
//!
//! Hosts supply stable metadata and invalidation callbacks. Core never knows
//! about windows, elements, phases or executors. Dropping a subscription removes
//! its edge immediately, including during notification.
use std::{
    any::Any,
    cell::{Cell, Ref, RefCell, RefMut},
    collections::BTreeMap,
    marker::PhantomData,
    rc::{Rc, Weak},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
fn next_id() -> u64 {
    NEXT_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .expect("reactive identity exhausted")
}

struct Observer {
    active: Cell<bool>,
    metadata: Box<dyn Any>,
    invalidate: Box<dyn Fn()>,
}
struct SourceInner {
    id: u64,
    observers: RefCell<BTreeMap<u64, Weak<Observer>>>,
}

/// A dependency identity with no scheduling policy of its own.
#[derive(Clone)]
pub struct DependencySource(Rc<SourceInner>);
impl Default for DependencySource {
    fn default() -> Self {
        Self(Rc::new(SourceInner {
            id: next_id(),
            observers: RefCell::new(BTreeMap::new()),
        }))
    }
}
impl DependencySource {
    #[must_use]
    pub fn id(&self) -> u64 {
        self.0.id
    }

    /// The returned token owns the edge. Metadata is diagnostic identity only.
    pub fn subscribe(&self, metadata: impl Any, invalidate: impl Fn() + 'static) -> Subscription {
        let id = next_id();
        let observer = Rc::new(Observer {
            active: Cell::new(true),
            metadata: Box::new(metadata),
            invalidate: Box::new(invalidate),
        });
        self.0
            .observers
            .borrow_mut()
            .insert(id, Rc::downgrade(&observer));
        Subscription {
            id,
            source: Rc::downgrade(&self.0),
            observer,
        }
    }

    /// Calls live observers outside registry borrows. New observers wait for
    /// the next notification; observers removed during dispatch are skipped.
    pub fn notify(&self) {
        let observers: Vec<_> = self
            .0
            .observers
            .borrow()
            .values()
            .filter_map(Weak::upgrade)
            .collect();
        for observer in observers {
            if observer.active.get() {
                (observer.invalidate)();
            }
        }
    }

    pub fn subscriber_count(&self) -> usize {
        self.0.observers.borrow().len()
    }

    /// Returns only live subscriber metadata of the requested host-owned type.
    pub fn subscribers<T: Any + Clone>(&self) -> Vec<T> {
        let observers: Vec<_> = self
            .0
            .observers
            .borrow()
            .values()
            .filter_map(Weak::upgrade)
            .collect();
        observers
            .into_iter()
            .filter(|observer| observer.active.get())
            .filter_map(|observer| observer.metadata.downcast_ref::<T>().cloned())
            .collect()
    }

    /// Records this source in the nearest active collection, if any.
    pub fn track(&self) -> bool {
        let callback = TRACKING.with(|stack| stack.borrow().last().cloned());
        if let Some(callback) = callback {
            callback(self.clone());
            true
        } else {
            false
        }
    }
}

/// An owned invalidation edge. It must live as long as its consumer.
#[must_use = "keep the subscription alive to observe changes"]
pub struct Subscription {
    id: u64,
    source: Weak<SourceInner>,
    observer: Rc<Observer>,
}
impl Drop for Subscription {
    fn drop(&mut self) {
        self.observer.active.set(false);
        if let Some(source) = self.source.upgrade() {
            source.observers.borrow_mut().remove(&self.id);
        }
    }
}

type Collector = Rc<dyn Fn(DependencySource)>;
thread_local! { static TRACKING: RefCell<Vec<Collector>> = const { RefCell::new(Vec::new()) }; }
thread_local! { static READ_ONLY: Cell<usize> = const { Cell::new(0) }; }

/// Forbids reactive writes while a pure builder or memo is evaluating.
pub struct ReadOnlyGuard(PhantomData<Rc<()>>);
impl ReadOnlyGuard {
    #[must_use]
    pub fn enter() -> Self {
        READ_ONLY.with(|depth| depth.set(depth.get() + 1));
        Self(PhantomData)
    }
}
impl Drop for ReadOnlyGuard {
    fn drop(&mut self) {
        READ_ONLY.with(|depth| depth.set(depth.get() - 1));
    }
}
/// Panics before modifying state when a pure evaluation is active.
pub fn assert_mutation_allowed() {
    assert!(
        READ_ONLY.with(Cell::get) == 0,
        "reactive mutation during a pure build or memo evaluation"
    );
}

/// Restores the previous collector on normal return or unwinding.
pub struct TrackingGuard(PhantomData<Rc<()>>);
impl TrackingGuard {
    #[must_use]
    pub fn enter(collect: impl Fn(DependencySource) + 'static) -> Self {
        TRACKING.with(|stack| stack.borrow_mut().push(Rc::new(collect)));
        Self(PhantomData)
    }
}
impl Drop for TrackingGuard {
    fn drop(&mut self) {
        TRACKING.with(|stack| {
            stack.borrow_mut().pop();
        });
    }
}

/// Storage for a reactive domain value. The domain API decides equality and
/// validation and calls `source().notify()` after releasing mutable borrows.
/// This is backend infrastructure, not another application signal API.
pub struct ReactiveCell<T> {
    value: RefCell<T>,
    source: DependencySource,
}
impl<T> ReactiveCell<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: RefCell::new(value),
            source: DependencySource::default(),
        }
    }
    pub fn borrow(&self) -> Ref<'_, T> {
        self.value.borrow()
    }
    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        self.value.borrow_mut()
    }
    pub fn source(&self) -> &DependencySource {
        &self.source
    }
}
