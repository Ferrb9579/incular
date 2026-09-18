use incular_config::Constraints;
use incular_core::{InputEvent, Offset, PointerPhase, Size};
use incular_material::{RangeSlider, RangeValues};
use incular_runtime::Runtime;
use std::{cell::Cell, rc::Rc};

fn pointer(runtime: &mut Runtime, phase: PointerPhase, x: f32, y: f32) {
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase,
        position: Offset::new(x, y),
    });
}

#[test]
fn range_drag_uses_press_origin_not_accumulated_total_delta() {
    let last = Rc::new(Cell::new(RangeValues::new(0.2, 0.8)));
    let observed = last.clone();
    let mut runtime = Runtime::new(
        RangeSlider::new(RangeValues::new(0.2, 0.8))
            .divisions(Some(10))
            .on_changed(move |next| observed.set(next))
            .into(),
    )
    .expect("range slider runtime");
    let constraints = Constraints::tight(Size::new(200.0, 32.0));
    runtime.run_frame(constraints).expect("initial frame");

    pointer(&mut runtime, PointerPhase::Down, 40.0, 15.0);
    pointer(&mut runtime, PointerPhase::Move, 60.0, 15.0);
    pointer(&mut runtime, PointerPhase::Move, 80.0, 15.0);
    pointer(&mut runtime, PointerPhase::Up, 80.0, 15.0);

    let value = last.get();
    assert!((value.start - 0.4).abs() < 0.001, "value={value:?}");
    assert!((value.end - 0.8).abs() < 0.001, "value={value:?}");
}

#[test]
fn second_range_drag_uses_the_new_start_value() {
    let last = Rc::new(Cell::new(RangeValues::new(0.2, 0.8)));
    let observed = last.clone();
    let mut runtime = Runtime::new(
        RangeSlider::new(RangeValues::new(0.2, 0.8))
            .divisions(Some(10))
            .on_changed(move |next| observed.set(next))
            .into(),
    )
    .expect("range slider runtime");
    let constraints = Constraints::tight(Size::new(200.0, 32.0));
    runtime.run_frame(constraints).expect("initial frame");

    pointer(&mut runtime, PointerPhase::Down, 40.0, 15.0);
    pointer(&mut runtime, PointerPhase::Move, 80.0, 15.0);
    pointer(&mut runtime, PointerPhase::Up, 80.0, 15.0);
    runtime.run_frame(constraints).expect("after first drag");
    assert!(
        (last.get().start - 0.4).abs() < 0.001,
        "first={:?}",
        last.get()
    );

    pointer(&mut runtime, PointerPhase::Down, 80.0, 15.0);
    pointer(&mut runtime, PointerPhase::Move, 100.0, 15.0);
    pointer(&mut runtime, PointerPhase::Up, 100.0, 15.0);
    assert!(
        (last.get().start - 0.5).abs() < 0.001,
        "second={:?}",
        last.get()
    );
}
