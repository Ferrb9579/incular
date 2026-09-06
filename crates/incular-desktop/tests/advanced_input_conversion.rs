use incular_core::{
    InputEvent, NormalizedPressure, PointerDeviceKind, PointerSampleMetadata,
    SECONDARY_POINTER_BUTTON, StylusMetadata, StylusOrientation, TrackpadGesture,
    TrackpadGesturePhase,
};
use incular_desktop::winit_adapter::{
    touch_event_with_native_sample, trackpad_pinch_event, trackpad_rotation_event,
};
use incular_platform::{NativePointerSample, PhysicalSize, PlatformEvent, WindowMetrics};
use winit::{
    dpi::PhysicalPosition,
    event::{DeviceId, Touch, TouchPhase},
};

fn touch(phase: TouchPhase) -> Touch {
    Touch {
        device_id: DeviceId::dummy(),
        phase,
        location: PhysicalPosition::new(40.0, 20.0),
        force: None,
        id: 7,
    }
}

#[test]
fn native_stylus_metadata_preserves_inversion_barrel_hover_and_device_identity() {
    let sample = NativePointerSample {
        device: Some(99),
        kind: PointerDeviceKind::InvertedStylus,
        sample: PointerSampleMetadata {
            pressure: Some(NormalizedPressure::new(0.4).unwrap()),
            stylus: Some(StylusMetadata {
                orientation: StylusOrientation::from_tilt_degrees(30.0, -15.0),
                barrel_button: true,
            }),
        },
        in_contact: Some(false),
    };
    let event = touch_event_with_native_sample(
        touch(TouchPhase::Moved),
        5,
        Some(sample),
        WindowMetrics::new(PhysicalSize::new(200, 100), 2.0),
    );
    let PlatformEvent::Input(InputEvent::PointerWithMetadata {
        pointer,
        device,
        kind,
        buttons,
        button,
        sample: actual,
        position,
        ..
    }) = event
    else {
        panic!("expected rich pointer event");
    };
    assert_eq!(pointer, 7);
    assert_eq!(device, 99);
    assert_eq!(kind, PointerDeviceKind::InvertedStylus);
    assert_eq!(buttons, SECONDARY_POINTER_BUTTON);
    assert_eq!(button, None);
    assert_eq!(actual, sample.sample);
    assert_eq!(position.x, 20.0);
    assert_eq!(position.y, 10.0);
}

#[test]
fn pinch_and_rotation_keep_native_phase_order_and_normalized_units() {
    let phases = [TouchPhase::Started, TouchPhase::Moved, TouchPhase::Ended];
    let expected = [
        TrackpadGesturePhase::Started,
        TrackpadGesturePhase::Updated,
        TrackpadGesturePhase::Ended,
    ];
    for (phase, expected) in phases.into_iter().zip(expected) {
        let PlatformEvent::Input(InputEvent::TrackpadGesture(TrackpadGesture::Pinch {
            device,
            phase,
            magnification_delta,
        })) = trackpad_pinch_event(3, 0.125, phase).unwrap()
        else {
            panic!("expected pinch");
        };
        assert_eq!(device, 3);
        assert_eq!(phase, expected);
        assert_eq!(magnification_delta, 0.125);
    }

    let PlatformEvent::Input(InputEvent::TrackpadGesture(TrackpadGesture::Rotation {
        phase,
        delta_radians,
        ..
    })) = trackpad_rotation_event(4, 180.0, TouchPhase::Moved).unwrap()
    else {
        panic!("expected rotation");
    };
    assert_eq!(phase, TrackpadGesturePhase::Updated);
    assert!((delta_radians - std::f32::consts::PI).abs() < 1e-6);

    assert!(trackpad_pinch_event(3, f64::NAN, TouchPhase::Moved).is_none());
}
