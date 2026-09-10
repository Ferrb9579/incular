use incular_gestures::{
    FocusBehavior, FocusHighlightManager, FocusHighlightMode, FocusHighlightStrategy,
    FocusHighlightSubscription, FocusNode, PointerDeviceKind,
};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

fn manager() -> FocusHighlightManager {
    let manager = FocusHighlightManager::new();
    manager.set_strategy(FocusHighlightStrategy::Automatic);
    manager.note_keyboard_input();
    manager.set_mode(FocusHighlightMode::Traditional);
    manager
}

#[test]
fn listeners_can_read_committed_mode_and_strategy_from_every_input_path() {
    let manager = manager();
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _subscription = manager.observe({
        let observed = observed.clone();
        move |mode| {
            assert_eq!(manager.mode(), mode);
            observed.borrow_mut().push((mode, manager.strategy()));
        }
    });

    manager.set_mode(FocusHighlightMode::Touch);
    manager.set_strategy(FocusHighlightStrategy::AlwaysTraditional);
    manager.set_strategy(FocusHighlightStrategy::Automatic);
    manager.note_pointer_input(PointerDeviceKind::Touch);
    manager.note_keyboard_input();

    assert_eq!(
        &*observed.borrow(),
        &[
            (FocusHighlightMode::Touch, FocusHighlightStrategy::Automatic),
            (
                FocusHighlightMode::Traditional,
                FocusHighlightStrategy::AlwaysTraditional,
            ),
            (FocusHighlightMode::Touch, FocusHighlightStrategy::Automatic),
            (
                FocusHighlightMode::Traditional,
                FocusHighlightStrategy::Automatic,
            ),
        ]
    );
}

#[test]
fn reentrant_changes_deliver_in_commit_order_to_all_listeners() {
    let manager = manager();
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _first = manager.observe({
        let observed = observed.clone();
        move |mode| {
            observed.borrow_mut().push((1, mode));
            if mode == FocusHighlightMode::Touch {
                manager.set_strategy(FocusHighlightStrategy::AlwaysTraditional);
                assert_eq!(manager.mode(), FocusHighlightMode::Traditional);
            }
        }
    });
    let _second = manager.observe({
        let observed = observed.clone();
        move |mode| observed.borrow_mut().push((2, mode))
    });

    manager.set_mode(FocusHighlightMode::Touch);

    assert_eq!(
        &*observed.borrow(),
        &[
            (1, FocusHighlightMode::Touch),
            (2, FocusHighlightMode::Touch),
            (1, FocusHighlightMode::Traditional),
            (2, FocusHighlightMode::Traditional),
        ]
    );
}

#[test]
fn listeners_added_during_delivery_start_with_the_next_committed_change() {
    let manager = manager();
    let added = Rc::new(RefCell::new(None::<FocusHighlightSubscription>));
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _first = manager.observe({
        let added = added.clone();
        let observed = observed.clone();
        move |mode| {
            if mode == FocusHighlightMode::Touch {
                let observed = observed.clone();
                *added.borrow_mut() = Some(manager.observe(move |mode| {
                    observed.borrow_mut().push(mode);
                }));
                manager.set_mode(FocusHighlightMode::Traditional);
            }
        }
    });

    manager.set_mode(FocusHighlightMode::Touch);

    assert_eq!(&*observed.borrow(), &[FocusHighlightMode::Traditional]);
}

#[test]
fn dropping_a_later_listener_cancels_its_current_and_queued_delivery() {
    let manager = manager();
    let later = Rc::new(RefCell::new(None::<FocusHighlightSubscription>));
    let calls = Rc::new(Cell::new(0));
    let _first = manager.observe({
        let later = later.clone();
        move |mode| {
            if mode == FocusHighlightMode::Touch {
                manager.set_mode(FocusHighlightMode::Traditional);
                later.borrow_mut().take();
            }
        }
    });
    *later.borrow_mut() = Some(manager.observe({
        let calls = calls.clone();
        move |_| calls.set(calls.get() + 1)
    }));

    manager.set_mode(FocusHighlightMode::Touch);

    assert_eq!(calls.get(), 0);
}

#[test]
fn a_listener_can_unsubscribe_itself_before_a_nested_change() {
    let manager = manager();
    let subscription = Rc::new(RefCell::new(None::<FocusHighlightSubscription>));
    let calls = Rc::new(Cell::new(0));
    *subscription.borrow_mut() = Some(manager.observe({
        let subscription = subscription.clone();
        let calls = calls.clone();
        move |_| {
            calls.set(calls.get() + 1);
            subscription.borrow_mut().take();
            manager.set_mode(FocusHighlightMode::Traditional);
        }
    }));

    manager.set_mode(FocusHighlightMode::Touch);

    assert_eq!(calls.get(), 1);
    assert!(subscription.borrow().is_none());
}

#[test]
fn application_highlight_callback_can_query_the_modality_owner() {
    let manager = manager();
    let focus = FocusNode::new();
    focus.request_focus();
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _behavior = FocusBehavior::new(
        focus,
        true,
        None,
        Some(Rc::new({
            let observed = observed.clone();
            move |visible| observed.borrow_mut().push((visible, manager.mode()))
        })),
        None,
    );

    manager.note_pointer_input(PointerDeviceKind::Stylus);
    manager.note_keyboard_input();

    assert_eq!(
        &*observed.borrow(),
        &[
            (false, FocusHighlightMode::Touch),
            (true, FocusHighlightMode::Traditional),
        ]
    );
}

#[test]
fn panic_keeps_committed_state_but_discards_unfinished_notifications() {
    let manager = manager();
    let panicking = manager.observe(move |_| {
        manager.set_mode(FocusHighlightMode::Traditional);
        panic!("application listener failed");
    });
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _later = manager.observe({
        let observed = observed.clone();
        move |mode| observed.borrow_mut().push(mode)
    });

    let result = catch_unwind(AssertUnwindSafe(|| {
        manager.set_mode(FocusHighlightMode::Touch);
    }));
    assert!(result.is_err());
    assert_eq!(manager.mode(), FocusHighlightMode::Traditional);
    assert!(observed.borrow().is_empty());
    drop(panicking);

    manager.set_mode(FocusHighlightMode::Touch);
    assert_eq!(&*observed.borrow(), &[FocusHighlightMode::Touch]);
}

#[test]
fn long_reentrant_transition_sequence_does_not_recurse() {
    let manager = manager();
    let calls = Rc::new(Cell::new(0));
    let _subscription = manager.observe({
        let calls = calls.clone();
        move |mode| {
            calls.set(calls.get() + 1);
            if calls.get() < 5_000 {
                manager.set_mode(match mode {
                    FocusHighlightMode::Touch => FocusHighlightMode::Traditional,
                    FocusHighlightMode::Traditional => FocusHighlightMode::Touch,
                });
            }
        }
    });

    manager.set_mode(FocusHighlightMode::Touch);

    assert_eq!(calls.get(), 5_000);
    assert_eq!(manager.mode(), FocusHighlightMode::Traditional);
}

#[test]
fn unchanged_input_does_not_notify_and_forced_strategy_preserves_policy() {
    let manager = manager();
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _subscription = manager.observe({
        let observed = observed.clone();
        move |mode| observed.borrow_mut().push(mode)
    });

    for _ in 0..1_000 {
        manager.set_mode(FocusHighlightMode::Traditional);
        manager.set_strategy(FocusHighlightStrategy::Automatic);
        manager.note_keyboard_input();
        manager.note_pointer_input(PointerDeviceKind::Mouse);
    }
    assert!(observed.borrow().is_empty());

    manager.set_strategy(FocusHighlightStrategy::AlwaysTouch);
    manager.note_keyboard_input();
    assert_eq!(manager.mode(), FocusHighlightMode::Touch);
    manager.set_strategy(FocusHighlightStrategy::Automatic);
    manager.note_pointer_input(PointerDeviceKind::InvertedStylus);
    manager.note_pointer_input(PointerDeviceKind::Mouse);
    assert_eq!(manager.mode(), FocusHighlightMode::Touch);
    assert_eq!(
        &*observed.borrow(),
        &[
            FocusHighlightMode::Touch,
            FocusHighlightMode::Traditional,
            FocusHighlightMode::Touch,
        ]
    );
}

#[test]
fn late_registration_does_not_receive_a_previously_queued_transition() {
    let manager = manager();
    let added = Rc::new(RefCell::new(None::<FocusHighlightSubscription>));
    let calls = Rc::new(Cell::new(0));
    let first = manager.observe({
        let added = added.clone();
        let calls = calls.clone();
        move |mode| {
            if mode == FocusHighlightMode::Touch {
                manager.set_mode(FocusHighlightMode::Traditional);
                let calls = calls.clone();
                *added.borrow_mut() = Some(manager.observe(move |_| {
                    calls.set(calls.get() + 1);
                }));
            }
        }
    });

    manager.set_mode(FocusHighlightMode::Touch);
    assert_eq!(calls.get(), 0);
    drop(first);
    manager.set_mode(FocusHighlightMode::Touch);
    assert_eq!(calls.get(), 1);
}

#[test]
fn queued_notifications_do_not_own_dropped_callback_captures() {
    struct Capture {
        manager: FocusHighlightManager,
        dropped: Rc<Cell<bool>>,
    }
    impl Drop for Capture {
        fn drop(&mut self) {
            assert_eq!(self.manager.mode(), FocusHighlightMode::Traditional);
            self.dropped.set(true);
        }
    }

    let manager = manager();
    let later = Rc::new(RefCell::new(None::<FocusHighlightSubscription>));
    let dropped = Rc::new(Cell::new(false));
    let _first = manager.observe({
        let later = later.clone();
        let dropped = dropped.clone();
        move |mode| {
            if mode == FocusHighlightMode::Touch {
                manager.set_mode(FocusHighlightMode::Traditional);
                later.borrow_mut().take();
                assert!(dropped.get(), "a queued event must not retain the callback");
            }
        }
    });
    let capture = Capture {
        manager,
        dropped: dropped.clone(),
    };
    *later.borrow_mut() = Some(manager.observe(move |_| {
        std::hint::black_box(&capture);
        panic!("removed listener ran");
    }));

    manager.set_mode(FocusHighlightMode::Touch);

    assert!(dropped.get());
}

#[test]
fn focus_behavior_hover_drives_hover_highlight_callbacks() {
    // No tree driver calls mouse_enter today; the behavior contract is
    // pinned at its owner so the widget audit can reference it.
    let _manager = manager();
    let hovered = Rc::new(RefCell::new(Vec::new()));
    let behavior = FocusBehavior::new(
        FocusNode::new(),
        true,
        None,
        None,
        Some(Rc::new({
            let hovered = hovered.clone();
            move |visible| hovered.borrow_mut().push(visible)
        })),
    );
    behavior.mouse_enter();
    behavior.mouse_enter();
    behavior.mouse_exit();
    assert_eq!(&*hovered.borrow(), &[true, false]);

    let suppressed = Rc::new(RefCell::new(Vec::new()));
    let disabled = FocusBehavior::new(
        FocusNode::new(),
        false,
        None,
        None,
        Some(Rc::new({
            let suppressed = suppressed.clone();
            move |visible| suppressed.borrow_mut().push(visible)
        })),
    );
    disabled.mouse_enter();
    disabled.mouse_exit();
    assert!(suppressed.borrow().is_empty());
}
