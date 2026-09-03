use incular_core::{
    NormalizedPressure, StylusOrientation, TrackpadGesture, TrackpadGestureKind,
    TrackpadGesturePhase,
};

#[test]
fn pressure_normalization_is_finite_bounded_and_single_range() {
    assert_eq!(NormalizedPressure::new(-0.25).unwrap().get(), 0.0);
    assert_eq!(NormalizedPressure::new(0.5).unwrap().get(), 0.5);
    assert_eq!(NormalizedPressure::new(1.25).unwrap().get(), 1.0);
    assert_eq!(
        NormalizedPressure::from_range(512.0, 1024.0).unwrap().get(),
        0.5
    );
    assert_eq!(
        NormalizedPressure::from_range(-5.0, 1024.0).unwrap().get(),
        0.0
    );
    assert_eq!(
        NormalizedPressure::from_range(2048.0, 1024.0)
            .unwrap()
            .get(),
        1.0
    );
    assert!(NormalizedPressure::new(f64::NAN).is_none());
    assert!(NormalizedPressure::from_range(1.0, 0.0).is_none());
    assert!(NormalizedPressure::from_range(1.0, f64::INFINITY).is_none());
}

#[test]
fn stylus_tilt_has_documented_altitude_and_screen_azimuth_units() {
    let vertical = StylusOrientation::from_tilt_degrees(0.0, 0.0).unwrap();
    assert!((vertical.altitude - std::f32::consts::FRAC_PI_2).abs() < 1e-6);

    let right = StylusOrientation::from_tilt_degrees(45.0, 0.0).unwrap();
    assert!((right.altitude - std::f32::consts::FRAC_PI_4).abs() < 1e-6);
    assert!(right.azimuth.abs() < 1e-6);

    let down = StylusOrientation::from_tilt_degrees(0.0, 45.0).unwrap();
    assert!((down.altitude - std::f32::consts::FRAC_PI_4).abs() < 1e-6);
    assert!((down.azimuth - std::f32::consts::FRAC_PI_2).abs() < 1e-6);

    assert!(StylusOrientation::from_tilt_degrees(91.0, 0.0).is_none());
    assert!(StylusOrientation::from_tilt_degrees(f64::NAN, 0.0).is_none());
}

#[test]
fn aggregate_trackpad_values_keep_device_kind_and_phase_explicit() {
    let gesture = TrackpadGesture::Pinch {
        device: 42,
        phase: TrackpadGesturePhase::Updated,
        magnification_delta: 0.25,
    };
    assert_eq!(gesture.device(), 42);
    assert_eq!(gesture.kind(), TrackpadGestureKind::Pinch);
    assert_eq!(gesture.phase(), Some(TrackpadGesturePhase::Updated));

    let instant = TrackpadGesture::SmartMagnify { device: 9 };
    assert_eq!(instant.kind(), TrackpadGestureKind::SmartMagnify);
    assert_eq!(instant.phase(), None);
}
