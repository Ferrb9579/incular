//! Validated bounds shared by widget and pure algorithm consumers.
use incular_config::{ConstraintError, Constraints};
use incular_core::Size;

#[test]
fn invalid_external_bounds_are_rejected_without_panicking() {
    for values in [
        [f32::NAN, 10.0, 0.0, 10.0],
        [0.0, f32::NAN, 0.0, 10.0],
        [0.0, 10.0, f32::NAN, 10.0],
        [0.0, 10.0, 0.0, f32::NAN],
        [-1.0, 10.0, 0.0, 10.0],
        [0.0, 10.0, -1.0, 10.0],
        [20.0, 10.0, 0.0, 10.0],
        [0.0, 10.0, 20.0, 10.0],
        [f32::INFINITY, f32::INFINITY, 0.0, 10.0],
        [0.0, 10.0, f32::INFINITY, f32::INFINITY],
        [0.0, f32::NEG_INFINITY, 0.0, 10.0],
    ] {
        assert_eq!(
            Constraints::try_new(values[0], values[1], values[2], values[3]),
            Err(ConstraintError::Invalid)
        );
    }
}

#[test]
fn valid_unbounded_and_tight_bounds_preserve_layout_behavior() {
    let bounds = Constraints::try_new(5.0, f32::INFINITY, 10.0, 40.0).unwrap();
    assert_eq!(
        (
            bounds.min_width(),
            bounds.max_width(),
            bounds.min_height(),
            bounds.max_height()
        ),
        (5.0, f32::INFINITY, 10.0, 40.0)
    );
    assert_eq!(bounds.constrain(Size::new(1.0, 80.0)), Size::new(5.0, 40.0));
    assert_eq!(
        bounds.deflate(2.0, 4.0),
        Constraints::new(3.0, f32::INFINITY, 6.0, 36.0)
    );
    assert_eq!(bounds.loosen().smallest(), Size::ZERO);
    let size = Size::new(20.0, 30.0);
    assert_eq!(Constraints::tight(size).constrain(Size::ZERO), size);
}

#[test]
#[should_panic(expected = "invalid constraints")]
fn infallible_constructor_rejects_the_same_invalid_bounds() {
    let _ = Constraints::new(4.0, 2.0, 0.0, 10.0);
}
