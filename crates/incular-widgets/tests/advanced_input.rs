use incular_core::{
    Color, Offset, PointerDeviceKind, PointerPhase, PointerSampleMetadata, Size, TrackpadGesture,
    TrackpadGesturePhase,
};
use incular_gestures::{PointerEvent, RawPointerEvent};
use incular_layout::Constraints;
use incular_widgets::{GestureDetector, MouseRegion, Widget, internal::WidgetTree};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

fn layout(tree: &mut WidgetTree, root: impl Into<Widget>) -> incular_widgets::internal::ElementId {
    let root = tree.mount(root.into()).expect("mount advanced input tree");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout advanced input tree");
    root
}

#[test]
fn native_trackpad_stream_has_one_deterministic_deepest_owner() {
    let inner_events = Rc::new(RefCell::new(Vec::new()));
    let outer_events = Rc::new(RefCell::new(Vec::new()));
    let inner = GestureDetector::new(Widget::box_(Size::new(100., 100.), Color::WHITE))
        .on_trackpad_gesture({
            let events = inner_events.clone();
            move |event| events.borrow_mut().push(event.phase())
        });
    let outer = GestureDetector::new(Widget::from(inner)).on_trackpad_gesture({
        let events = outer_events.clone();
        move |event| events.borrow_mut().push(event.phase())
    });
    let mut tree = WidgetTree::new();
    layout(&mut tree, outer);

    for phase in [
        TrackpadGesturePhase::Started,
        TrackpadGesturePhase::Updated,
        TrackpadGesturePhase::Ended,
    ] {
        assert!(
            tree.dispatch_trackpad_gesture_in_window(
                4,
                Offset::new(20., 20.),
                TrackpadGesture::Pinch {
                    device: 71,
                    phase,
                    magnification_delta: 0.1,
                },
            )
            .is_some()
        );
    }

    assert_eq!(
        &*inner_events.borrow(),
        &[
            Some(TrackpadGesturePhase::Started),
            Some(TrackpadGesturePhase::Updated),
            Some(TrackpadGesturePhase::Ended),
        ]
    );
    assert!(
        outer_events.borrow().is_empty(),
        "the native gesture already exists; retained arbitration chooses one owner"
    );
}

#[test]
fn equal_pointer_numbers_on_mouse_and_pen_remain_independent_streams() {
    let mut tree = WidgetTree::new();
    let root = layout(
        &mut tree,
        GestureDetector::new(Widget::box_(Size::new(100., 100.), Color::WHITE)).on_tap(|| {}),
    );
    let now = Instant::now();
    let down = PointerEvent {
        pointer: 5,
        position: Offset::new(10., 10.),
        phase: PointerPhase::Down,
        time: now,
    };

    assert_eq!(
        tree.dispatch_device_gesture_in_window(9, 11, down),
        Some(root)
    );
    assert_eq!(
        tree.dispatch_device_gesture_in_window(9, 22, down),
        Some(root)
    );
    assert_eq!(tree.pointer_capture_target_for_device(9, 11, 5), Some(root));
    assert_eq!(tree.pointer_capture_target_for_device(9, 22, 5), Some(root));
    assert_eq!(
        tree.pointer_capture_target(9, 5),
        None,
        "device-less lookup is intentionally ambiguous while both streams exist"
    );

    for device in [11, 22] {
        let _ = tree.dispatch_device_gesture_in_window(
            9,
            device,
            PointerEvent {
                phase: PointerPhase::Up,
                ..down
            },
        );
    }
    assert_eq!(tree.pointer_capture_target_for_device(9, 11, 5), None);
    assert_eq!(tree.pointer_capture_target_for_device(9, 22, 5), None);
}

#[test]
fn stylus_hover_uses_the_same_retained_hover_route_as_other_hover_devices() {
    let enters = Rc::new(Cell::new(0));
    let hovers = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    layout(
        &mut tree,
        MouseRegion::new(Widget::box_(Size::new(100., 100.), Color::WHITE))
            .on_enter({
                let enters = enters.clone();
                move |_| enters.set(enters.get() + 1)
            })
            .on_hover({
                let hovers = hovers.clone();
                move |_| hovers.set(hovers.get() + 1)
            }),
    );

    let event = RawPointerEvent {
        pointer: 31,
        device: 47,
        kind: PointerDeviceKind::Stylus,
        buttons: 0,
        button: None,
        sample: PointerSampleMetadata::EMPTY,
        position: Offset::new(20., 20.),
        phase: PointerPhase::Move,
        time: Instant::now(),
    };
    let _ = tree.dispatch_raw_pointer_in_window(5, event);

    assert_eq!(enters.get(), 1);
    assert_eq!(hovers.get(), 1);
}
