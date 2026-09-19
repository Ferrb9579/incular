use incular_gestures::{FocusNode, FocusScopeNode, FocusScopeSubscription};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[test]
fn scope_observer_can_reenter_and_choose_new_focus() {
    let scope = FocusScopeNode::new();
    let first = FocusNode::new();
    let second = FocusNode::new();
    scope.register(&first);
    scope.register(&second);

    let reentered = Rc::new(Cell::new(false));
    let observed_scope = scope.clone();
    let observed_first = first.clone();
    let observed_second = second.clone();
    let observed_reentered = reentered.clone();
    let _subscription = scope.observe(move || {
        if observed_scope.focused() == Some(observed_first.clone())
            && !observed_reentered.replace(true)
        {
            assert!(observed_scope.request_focus(&observed_second));
        }
    });

    assert!(scope.request_focus(&first));
    assert!(reentered.get());
    assert_eq!(scope.focused(), Some(second));
}

#[test]
fn scope_observer_can_unregister_during_focus_notification() {
    let scope = FocusScopeNode::new();
    let node = FocusNode::new();
    scope.register(&node);

    let observed_scope = scope.clone();
    let observed_node = node.clone();
    let removed = Rc::new(Cell::new(false));
    let observed_removed = removed.clone();
    let _subscription = scope.observe(move || {
        if observed_scope.focused() == Some(observed_node.clone())
            && !observed_removed.replace(true)
        {
            observed_scope.unregister(&observed_node);
        }
    });

    assert!(scope.request_focus(&node));
    assert!(removed.get());
    assert!(scope.focused().is_none());
    assert_eq!(scope.registered_count(), 0);
}

#[test]
fn dropping_later_scope_subscription_during_delivery_skips_it() {
    let scope = FocusScopeNode::new();
    let node = FocusNode::new();
    scope.register(&node);

    let later_calls = Rc::new(Cell::new(0));
    let later_slot = Rc::new(RefCell::new(None::<FocusScopeSubscription>));
    let drop_slot = later_slot.clone();
    let _first = scope.observe(move || {
        drop_slot.borrow_mut().take();
    });
    let observed_calls = later_calls.clone();
    *later_slot.borrow_mut() = Some(scope.observe(move || {
        observed_calls.set(observed_calls.get() + 1);
    }));

    assert!(scope.request_focus(&node));
    assert_eq!(later_calls.get(), 0);
}

#[test]
fn clear_restore_and_traversal_are_committed_before_callbacks() {
    let scope = FocusScopeNode::new();
    let first = FocusNode::new();
    let second = FocusNode::new();
    scope.register(&first);
    scope.register(&second);
    assert!(scope.request_focus(&first));

    let snapshots = Rc::new(RefCell::new(Vec::new()));
    let observed_scope = scope.clone();
    let observed = snapshots.clone();
    let _subscription = scope.observe(move || {
        observed.borrow_mut().push(observed_scope.focused());
    });

    scope.clear_focus();
    assert!(scope.restore_focus());
    assert_eq!(scope.focus_next(false), Some(second.clone()));
    assert_eq!(scope.focus_next(true), Some(first.clone()));

    let snapshots = snapshots.borrow();
    assert_eq!(snapshots[0], None);
    assert_eq!(snapshots[1], Some(first.clone()));
    assert_eq!(snapshots[2], Some(second));
    assert_eq!(snapshots[3], Some(first));
}

#[test]
fn nested_parent_observer_can_make_newer_focus_decision() {
    let parent = FocusScopeNode::new();
    let parent_first = FocusNode::new();
    let parent_second = FocusNode::new();
    parent.register(&parent_first);
    parent.register(&parent_second);
    assert!(parent.request_focus(&parent_first));

    let child = FocusScopeNode::nested(&parent);
    let child_node = FocusNode::new();
    child.register(&child_node);

    let observed_parent = parent.clone();
    let observed_second = parent_second.clone();
    let switched = Rc::new(Cell::new(false));
    let observed_switched = switched.clone();
    let _subscription = parent.observe(move || {
        if observed_parent.focused().is_none() && !observed_switched.replace(true) {
            assert!(observed_parent.request_focus(&observed_second));
        }
    });

    assert!(child.request_focus(&child_node));
    assert!(switched.get());
    assert_eq!(parent.focused(), Some(parent_second));
    assert_eq!(child.focused(), Some(child_node));
}
