use incular_core::{
    BuildContext,
    reactivity::{DependencySource, ReadOnlyGuard, Subscription, assert_mutation_allowed},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[test]
fn dropping_a_pending_observer_during_notification_skips_it() {
    let source = DependencySource::default();
    let pending = Rc::new(RefCell::new(None::<Subscription>));
    let _first = source.subscribe((), {
        let pending = pending.clone();
        move || {
            pending.borrow_mut().take();
        }
    });
    let calls = Rc::new(Cell::new(0));
    *pending.borrow_mut() = Some(source.subscribe((), {
        let calls = calls.clone();
        move || calls.set(calls.get() + 1)
    }));
    source.notify();
    assert_eq!(calls.get(), 0);
    assert_eq!(source.subscriber_count(), 1);
}

#[test]
fn context_replaces_branches_and_last_owner_releases_subscriptions() {
    let first = DependencySource::default();
    let second = DependencySource::default();
    let context = BuildContext::new();
    context.build(|_| {
        first.track();
        first.track();
    });
    assert_eq!(first.subscriber_count(), 1);
    context.build(|_| {
        second.track();
    });
    assert_eq!(first.subscriber_count(), 0);
    assert_eq!(second.subscriber_count(), 1);
    let clone = context.clone();
    drop(context);
    second.notify();
    assert!(clone.is_dirty());
    drop(clone);
    assert_eq!(second.subscriber_count(), 0);
}

#[test]
fn pure_scope_restores_mutation_permission_after_unwind() {
    let panic = std::panic::catch_unwind(|| {
        let _guard = ReadOnlyGuard::enter();
        assert_mutation_allowed();
    });
    assert!(panic.is_err());
    assert_mutation_allowed();
}
