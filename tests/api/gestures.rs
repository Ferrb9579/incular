use incular::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn test_gestures_and_recognizer_contract() {
    let tap_count = Rc::new(Cell::new(0));
    let tap_count_clone = tap_count.clone();

    let long_press_flag = Rc::new(Cell::new(false));
    let long_press_clone = long_press_flag.clone();

    let detector: Widget = GestureDetector::new(SizedBox::shrink())
        .on_tap(move || {
            tap_count_clone.set(tap_count_clone.get() + 1);
        })
        .on_long_press(move || {
            long_press_clone.set(true);
        })
        .on_pan_update(|offset| {
            let _ = (offset.x, offset.y);
        })
        .on_scale_update(|details| {
            let _ = (details.scale, details.pointer_count);
        })
        .into();

    let _ = detector;
}
