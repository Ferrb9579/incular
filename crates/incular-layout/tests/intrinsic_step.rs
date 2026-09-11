//! Intrinsic step rounding arithmetic.
//!
//! `round_intrinsic_step` divides in `f64` so tiny steps and large extents
//! stay exact, saturates unrepresentable results to `f32::MAX` (following
//! `safe_add`), and never shrinks below the measured extent.

use incular_layout::round_intrinsic_step;

#[test]
fn exact_multiples_are_unchanged() {
    assert_eq!(round_intrinsic_step(32., Some(16.)), 32.);
    assert_eq!(round_intrinsic_step(0., Some(16.)), 0.);
}

#[test]
fn partial_extents_round_up() {
    assert_eq!(round_intrinsic_step(37., Some(16.)), 48.);
    assert_eq!(round_intrinsic_step(0.1, Some(16.)), 16.);
}

#[test]
fn invalid_steps_leave_the_extent_unchanged() {
    for step in [
        None,
        Some(0.),
        Some(-4.),
        Some(f32::NAN),
        Some(f32::INFINITY),
    ] {
        assert_eq!(round_intrinsic_step(37., step), 37., "step {step:?}");
    }
}

#[test]
fn non_finite_extents_pass_through() {
    assert!(round_intrinsic_step(f32::NAN, Some(16.)).is_nan());
    assert_eq!(
        round_intrinsic_step(f32::INFINITY, Some(16.)),
        f32::INFINITY
    );
}

#[test]
fn tiny_steps_do_not_grow_normal_extents() {
    // A subnormal-scale step still resolves to the extent itself: the f64
    // quotient is exact enough that ceil lands on the same multiple.
    let rounded = round_intrinsic_step(37., Some(1e-30));
    assert!(
        (rounded - 37.).abs() < 1e-3,
        "tiny step must not inflate, got {rounded}"
    );
}

#[test]
fn unrepresentable_results_saturate() {
    // f32::MAX rounded up by any step above one quantum exceeds the
    // representable range and saturates instead of going non-finite.
    assert_eq!(round_intrinsic_step(f32::MAX, Some(16.)), f32::MAX);
    assert!(round_intrinsic_step(1e30, Some(1e30)).is_finite());
}
