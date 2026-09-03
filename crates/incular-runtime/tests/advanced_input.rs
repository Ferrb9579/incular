use incular_config::Constraints;
use incular_core::{
    InputEvent, Offset, PointerDeviceKind, PointerPhase, PointerSampleMetadata, Size,
    TrackpadGesture, TrackpadGesturePhase,
};
use incular_platform::{PlatformEvent, WindowEvent, WindowOptions};
use incular_runtime::Application;
use incular_widgets::{Color, GestureDetector, Widget};
use std::{cell::RefCell, rc::Rc, time::Instant};

fn application(events: Rc<RefCell<Vec<TrackpadGesturePhase>>>) -> Application {
    let root = GestureDetector::new(Widget::box_(Size::new(100., 100.), Color::WHITE))
        .on_trackpad_gesture(move |event| {
            if let Some(phase) = event.phase() {
                events.borrow_mut().push(phase);
            }
        });
    let retained = Widget::from(root);
    let mut application = Application::new_with_options(
        WindowOptions {
            initial_logical_size: Size::new(100., 100.),
            ..WindowOptions::new("advanced input test")
        },
        move |_| retained.clone(),
    )
    .expect("application");
    let id = application.primary_window();
    let _ = application.take_native_window_commands();
    application
        .run_window_frame_at(
            id,
            Constraints::tight(Size::new(100., 100.)),
            Instant::now(),
        )
        .expect("initial frame");
    application
}

fn send_trackpad(application: &mut Application, phase: TrackpadGesturePhase) {
    let id = application.primary_window();
    application.handle_window_event(WindowEvent::platform(
        id,
        PlatformEvent::Input(InputEvent::TrackpadGesture(TrackpadGesture::Pinch {
            device: 88,
            phase,
            magnification_delta: 0.2,
        })),
    ));
}

#[test]
fn aggregate_trackpad_gesture_requires_real_pointer_location_then_routes_normally() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut application = application(events.clone());

    send_trackpad(&mut application, TrackpadGesturePhase::Started);
    assert!(
        events.borrow().is_empty(),
        "runtime must not manufacture a focal contact for an aggregate gesture"
    );

    let id = application.primary_window();
    application.handle_window_event(WindowEvent::platform(
        id,
        PlatformEvent::Input(InputEvent::PointerWithMetadata {
            pointer: 0,
            device: 3,
            kind: PointerDeviceKind::Mouse,
            buttons: 0,
            button: None,
            sample: PointerSampleMetadata::EMPTY,
            phase: PointerPhase::Move,
            position: Offset::new(20., 20.),
        }),
    ));

    for phase in [
        TrackpadGesturePhase::Started,
        TrackpadGesturePhase::Updated,
        TrackpadGesturePhase::Ended,
    ] {
        send_trackpad(&mut application, phase);
    }
    assert_eq!(
        &*events.borrow(),
        &[
            TrackpadGesturePhase::Started,
            TrackpadGesturePhase::Updated,
            TrackpadGesturePhase::Ended,
        ]
    );
}
