use incular_animation::AnimationController;
use incular_core::BuildContext;
use std::{cell::Cell, rc::Rc, time::Duration};

#[test]
fn animation_values_track_and_owned_observers_unsubscribe() {
    let context = BuildContext::new();
    let controller = AnimationController::new(Duration::from_secs(1));
    let calls = Rc::new(Cell::new(0));
    let subscription = controller.observe({
        let calls = calls.clone();
        move |_| calls.set(calls.get() + 1)
    });
    context.build(|_| {
        let _ = controller.value();
    });
    controller.set_value(0.5);
    assert!(context.take_dirty());
    assert_eq!(calls.get(), 1);
    drop(subscription);
    controller.set_value(1.0);
    assert!(context.is_dirty());
    assert_eq!(calls.get(), 1);
}

#[test]
fn animation_listener_can_mutate_after_notification_releases_state_borrow() {
    let controller = AnimationController::new(Duration::from_secs(1));
    let listener = controller.add_listener({
        let controller = controller.clone();
        move |value| {
            if value == 0.0 {
                controller.set_value(0.5);
            }
        }
    });
    controller.forward(std::time::Instant::now());
    assert_eq!(controller.value(), 0.5);
    assert!(controller.remove_listener(listener));
}
