//! Operator-norm bound on affine magnification: no unit vector may
//! stretch more than [`Transform::magnification_bound`] reports. Lengths
//! below come from plain arithmetic on independently transformed unit
//! vectors, never from the bound itself.

use incular_core::{Offset, Transform};

fn stretch(world: Transform, direction: Offset) -> f64 {
    let mapped = world.transform_point(direction);
    let origin = world.transform_point(Offset::ZERO);
    let dx = f64::from(mapped.x - origin.x);
    let dy = f64::from(mapped.y - origin.y);
    dx.hypot(dy)
}

#[test]
fn bound_covers_axis_transforms_tightly() {
    // Frobenius values: identity/translation/rotation/reflection all
    // have orthonormal columns, so the bound is exactly sqrt(2) and the
    // offset must not leak into it.
    for world in [
        Transform::IDENTITY,
        Transform::translation(Offset::new(1e10, -1e10)),
        Transform::rotation(std::f32::consts::FRAC_PI_3),
        Transform::scale_non_uniform(-1., 1.),
    ] {
        let bound = f64::from(world.magnification_bound());
        assert!(
            (bound - std::f64::consts::SQRT_2).abs() < 1e-6,
            "orthonormal columns must bound to sqrt(2), got {bound}"
        );
    }
    let bound = f64::from(Transform::scale(3.).magnification_bound());
    assert!(
        (bound - 18f64.sqrt()).abs() < 1e-5,
        "uniform 3x must bound to sqrt(18), got {bound}"
    );
}

#[test]
fn bound_covers_shear_diagonal_stretch() {
    // skew(1, 0) shears x by y: the diagonal stretches ~1.581 while the
    // longest column only reaches ~1.414, which is exactly the gap a
    // largest-column bound misses.
    let world = Transform::skew(1., 0.);
    let columns = [
        stretch(world, Offset::new(1., 0.)),
        stretch(world, Offset::new(0., 1.)),
    ];
    let diagonal = stretch(
        world,
        Offset::new(
            std::f32::consts::FRAC_1_SQRT_2,
            std::f32::consts::FRAC_1_SQRT_2,
        ),
    );
    let column_max = columns[0].max(columns[1]);
    assert!(
        diagonal > column_max,
        "shear diagonal ({diagonal}) must out-stretch both columns ({columns:?})"
    );
    let bound = f64::from(world.magnification_bound());
    for (name, measured) in [
        ("e1", columns[0]),
        ("e2", columns[1]),
        ("diagonal", diagonal),
    ] {
        assert!(
            measured <= bound * (1. + 1e-6),
            "{name} stretch ({measured}) must stay within the bound ({bound})"
        );
    }
    // Frobenius overestimates a 2x2 operator norm by at most sqrt(2);
    // demand much better here so the bound cannot silently degrade into
    // a gross overestimate either.
    assert!(
        bound <= diagonal * 1.5,
        "bound ({bound}) must stay near the measured worst stretch ({diagonal})"
    );
}

#[test]
fn bound_handles_degenerate_and_non_finite() {
    assert_eq!(
        Transform::scale(0.).magnification_bound(),
        0.,
        "degenerate maps stretch nothing"
    );
    let nan = Transform::from_kurbo(kurbo::Affine::new([f64::NAN, 0., 0., 1., 0., 0.]));
    assert_eq!(
        nan.magnification_bound(),
        f32::INFINITY,
        "non-finite input must report unbounded, never NaN"
    );
    let huge = Transform::scale(1e30);
    let bound = huge.magnification_bound();
    assert!(
        !bound.is_nan(),
        "huge-but-finite input must not produce NaN, got {bound}"
    );
    assert!(
        f64::from(bound) >= 1e30,
        "huge scale must stay covered, got {bound}"
    );
}
