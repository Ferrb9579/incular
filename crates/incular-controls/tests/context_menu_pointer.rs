use incular_config::Constraints;
use incular_controls::context_menu;
use incular_core::{Offset, PRIMARY_POINTER_BUTTON, PointerPhase, SECONDARY_POINTER_BUTTON, Size};
use incular_widgets::internal::{PointerDeviceKind, RawPointerEvent, WidgetTree};
use incular_widgets::{Color, Widget};
use std::{cell::RefCell, rc::Rc, time::Instant};

fn event(buttons: u32, button: u32) -> RawPointerEvent {
    RawPointerEvent {
        pointer: 0,
        device: 9,
        kind: PointerDeviceKind::Mouse,
        buttons,
        button: Some(button),
        position: Offset::new(20., 10.),
        phase: PointerPhase::Down,
        time: Instant::now(),
    }
}

#[test]
fn context_menu_trigger_observes_secondary_press_without_remapping_primary() {
    let opened = Rc::new(RefCell::new(Vec::new()));
    let trigger = context_menu::Trigger::new(Widget::box_(Size::new(80., 30.), Color::WHITE))
        .on_open({
            let opened = opened.clone();
            move |event| opened.borrow_mut().push((event.button, event.position))
        });
    let mut tree = WidgetTree::new();
    tree.mount(trigger.into()).expect("mount context trigger");
    tree.layout(Constraints::tight(Size::new(80., 30.)))
        .expect("layout context trigger");

    let _ = tree
        .dispatch_raw_pointer_in_window(1, event(PRIMARY_POINTER_BUTTON, PRIMARY_POINTER_BUTTON));
    assert!(opened.borrow().is_empty());

    let _ = tree.dispatch_raw_pointer_in_window(
        1,
        event(SECONDARY_POINTER_BUTTON, SECONDARY_POINTER_BUTTON),
    );
    assert_eq!(
        &*opened.borrow(),
        &[(Some(SECONDARY_POINTER_BUTTON), Offset::new(20., 10.))]
    );
}
