use incular_core::{Offset, Transform};

#[path = "../src/glyph_position.rs"]
mod glyph_position;

#[test]
fn device_phase_includes_dpi_and_compositor_translation_once() {
    let p = glyph_position::device_origin(
        Offset::new(10.25, 20.5),
        Transform::translation(Offset::new(1.25, -0.25)),
        1.5,
    )
    .unwrap();
    assert_eq!(p, Offset::new(17.25, 30.5));
    assert_eq!(glyph_position::phase(Some(p)), [1, 2]);
    // Reconstructing the device position from quad origin plus mask phase must
    // agree, without applying the compositor translation or DPI a second time.
    let phase = glyph_position::phase(Some(p));
    assert_eq!(
        Offset::new(
            p.x.floor() + f32::from(phase[0]) / 4.,
            p.y.floor() + f32::from(phase[1]) / 4.
        ),
        p
    );
}

#[test]
fn negative_coordinates_and_pixel_carry_preserve_fractional_origin() {
    for (input, expected, phase) in [
        (-1.125, -1., 0),
        (-0.875, -0.75, 1),
        (-0.125, 0., 0),
        (0.875, 1., 0),
    ] {
        let p = glyph_position::device_origin(Offset::new(input, input), Transform::IDENTITY, 1.)
            .unwrap();
        assert_eq!(p, Offset::new(expected, expected));
        assert_eq!(glyph_position::phase(Some(p)), [phase; 2]);
    }
}

#[test]
fn rotated_scaled_and_skewed_text_keeps_affine_sampling() {
    for transform in [
        Transform::rotation(0.3),
        Transform::scale(2.),
        Transform::skew(0.1, 0.),
    ] {
        assert!(glyph_position::device_origin(Offset::new(0.25, 0.5), transform, 1.5).is_none());
    }
    assert_eq!(glyph_position::phase(None), [0, 0]);
}
