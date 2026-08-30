use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};

use incular_animation::{
    Animation, AnimationController, AnimationStatus, AnimationValue, Curve, ImplicitAnimation,
    TweenValue,
};
use incular_core::Offset;

#[test]
fn bounds_and_drive_apis_are_deterministic() {
    let origin = Instant::now();
    let controller = AnimationController::with_bounds(Duration::from_secs(1), 10.0, 20.0);
    controller.animate_to(origin, 15.0);
    assert!(controller.tick(origin + Duration::from_millis(500)));
    assert_eq!(controller.value(), 12.5);
    assert!(controller.tick(origin + Duration::from_secs(1)));
    assert_eq!(controller.value(), 15.0);
    assert_eq!(controller.status(), AnimationStatus::Completed);
}

#[test]
fn repeat_can_run_in_reverse_and_notifies() {
    let origin = Instant::now();
    let controller = AnimationController::new(Duration::from_secs(1));
    let calls = Rc::new(Cell::new(0));
    let calls_in_listener = calls.clone();
    let token = controller.add_listener(move |_| {
        calls_in_listener.set(calls_in_listener.get() + 1);
    });
    controller.repeat(origin, true);
    controller.tick(origin + Duration::from_millis(500));
    assert_eq!(controller.value(), 0.5);
    controller.tick(origin + Duration::from_millis(1500));
    assert_eq!(controller.value(), 0.5);
    assert!(calls.get() >= 2);
    assert!(controller.remove_listener(token));
}

#[test]
fn zero_duration_completes_on_first_tick() {
    let origin = Instant::now();
    let controller = AnimationController::new(Duration::ZERO);
    controller.forward(origin);
    assert!(controller.tick(origin));
    assert_eq!(controller.value(), 1.0);
    assert_eq!(controller.status(), AnimationStatus::Completed);
}

#[test]
fn curves_are_clamped_and_reversible() {
    assert_eq!(Curve::Linear.apply(-1.0), 0.0);
    assert_eq!(Curve::Linear.apply(2.0), 1.0);
    assert!((Curve::EaseIn.reverse().apply(0.25) - Curve::EaseOut.apply(0.25)).abs() < 1e-6);
}

#[test]
fn chained_curves_preserve_endpoints() {
    let curve = Curve::EaseIn.then(Curve::EaseOut);
    assert_eq!(curve.apply(0.0), 0.0);
    assert_eq!(curve.apply(1.0), 1.0);
}

#[test]
fn typed_tween_interpolates_scalars_and_offsets() {
    assert_eq!(TweenValue::new(0.0_f32, 10.0).evaluate(0.25), 2.5);
    assert_eq!(
        TweenValue::new(Offset::ZERO, Offset::new(10.0, 20.0)).evaluate(0.5),
        Offset::new(5.0, 10.0)
    );
}

#[test]
fn weighted_sequence_uses_local_progress() {
    let sequence = TweenValue::new(0.0_f32, 10.0).then(TweenValue::new(10.0, 20.0));
    assert_eq!(sequence.evaluate(0.25), Some(5.0));
    assert_eq!(sequence.evaluate(0.75), Some(15.0));
}

#[test]
fn animation_samples_typed_tween_from_shared_controller() {
    let origin = Instant::now();
    let animation = Animation::from_to(0.0_f32, 10.0, Duration::from_secs(1), Curve::Linear);
    animation.forward(origin);
    animation.tick(origin + Duration::from_millis(500));
    assert_eq!(animation.value(), 5.0);
    let mapped = animation.clone().map(|value| value.to_string());
    assert_eq!(mapped.value(), "5");
}

#[test]
fn implicit_animation_retargets_from_current_value() {
    let origin = Instant::now();
    let mut animation = ImplicitAnimation::new(0.0_f32, Duration::from_secs(1));
    animation.set_target(origin, 10.0);
    animation.tick(origin + Duration::from_millis(500));
    assert_eq!(animation.value(), 5.0);
    animation.set_target(origin + Duration::from_millis(500), 20.0);
    assert_eq!(animation.begin(), 5.0);
    animation.tick(origin + Duration::from_millis(1000));
    assert_eq!(animation.value(), 12.5);
}

#[test]
fn composed_values_support_constants_and_sequences() {
    let constant = AnimationValue::constant(3_i32);
    assert_eq!(constant.evaluate(0.4), Some(3));
    let sequence = AnimationValue::sequence(
        TweenValue::new(0_i32, 10_i32).then(TweenValue::new(10_i32, 20_i32)),
    );
    assert_eq!(sequence.evaluate(0.75), Some(15));
}
