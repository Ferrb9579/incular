use incular_core::{Offset, PRIMARY_POINTER_BUTTON, PointerPhase, SECONDARY_POINTER_BUTTON, Size};
use incular_gestures::{MouseCursor, PointerDeviceKind, RawPointerEvent};
use incular_layout::Constraints;
use incular_widgets::internal::WidgetTree;
use incular_widgets::{Color, MouseRegion, TapRegion, TapRegionSurface, Widget};
use std::{cell::Cell, rc::Rc, time::Instant};

fn event(phase: PointerPhase, position: Offset) -> RawPointerEvent {
    RawPointerEvent {
        pointer: 0,
        device: 1,
        kind: PointerDeviceKind::Mouse,
        buttons: 0,
        button: None,
        sample: incular_core::PointerSampleMetadata::default(),
        position,
        phase,
        time: Instant::now(),
    }
}

fn button_event(button: u32) -> RawPointerEvent {
    RawPointerEvent {
        buttons: button,
        button: Some(button),
        phase: PointerPhase::Down,
        ..event(PointerPhase::Down, Offset::new(20., 20.))
    }
}

fn layout(tree: &mut WidgetTree, root: impl Into<Widget>) {
    tree.mount(root.into()).expect("mount pointer tree");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout pointer tree");
}

#[test]
fn native_surface_exit_clears_mouse_region_hover_exactly_once() {
    let enters = Rc::new(Cell::new(0));
    let exits = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    layout(
        &mut tree,
        MouseRegion::new(Widget::box_(Size::new(100., 100.), Color::WHITE))
            .on_enter({
                let enters = enters.clone();
                move |_| enters.set(enters.get() + 1)
            })
            .on_exit({
                let exits = exits.clone();
                move |_| exits.set(exits.get() + 1)
            }),
    );

    assert!(
        tree.dispatch_raw_pointer_in_window(7, event(PointerPhase::Enter, Offset::ZERO))
            .is_none()
    );
    assert_eq!(
        enters.get(),
        0,
        "surface enter has no reliable local position"
    );
    let _ =
        tree.dispatch_raw_pointer_in_window(7, event(PointerPhase::Move, Offset::new(20., 20.)));
    assert_eq!(enters.get(), 1);

    let _ =
        tree.dispatch_raw_pointer_in_window(7, event(PointerPhase::Exit, Offset::new(20., 20.)));
    assert_eq!(exits.get(), 1);
    let _ =
        tree.dispatch_raw_pointer_in_window(7, event(PointerPhase::Exit, Offset::new(20., 20.)));
    assert_eq!(
        exits.get(),
        1,
        "duplicate native exits must not re-exit stale regions"
    );

    let _ =
        tree.dispatch_raw_pointer_in_window(7, event(PointerPhase::Move, Offset::new(20., 20.)));
    assert_eq!(enters.get(), 2, "a later positioned sample can enter again");
}

#[test]
fn deferred_mouse_cursor_falls_through_to_ancestor_without_changing_hit_testing() {
    let child = MouseRegion::new(Widget::box_(Size::new(100., 100.), Color::WHITE))
        .cursor(MouseCursor::Defer);
    let mut tree = WidgetTree::new();
    layout(&mut tree, MouseRegion::new(child).cursor(MouseCursor::Text));
    let point = Offset::new(40., 40.);
    let hit_before = tree.hit_test(point);
    assert_eq!(tree.mouse_cursor_at(point), MouseCursor::Text);
    assert_eq!(tree.hit_test(point), hit_before);

    let child = MouseRegion::new(Widget::box_(Size::new(100., 100.), Color::WHITE))
        .cursor(MouseCursor::Click);
    let mut tree = WidgetTree::new();
    layout(&mut tree, MouseRegion::new(child).cursor(MouseCursor::Text));
    assert_eq!(tree.mouse_cursor_at(point), MouseCursor::Click);
}

#[test]
fn tap_region_is_primary_only_while_raw_secondary_input_remains_distinct() {
    let taps = Rc::new(Cell::new(0));
    let region = TapRegion::new(Widget::box_(Size::new(100., 100.), Color::WHITE)).on_tap_inside({
        let taps = taps.clone();
        move |_| taps.set(taps.get() + 1)
    });
    let mut tree = WidgetTree::new();
    layout(&mut tree, TapRegionSurface::new(region));

    let _ = tree.dispatch_raw_pointer_in_window(7, button_event(SECONDARY_POINTER_BUTTON));
    assert_eq!(taps.get(), 0);
    let _ = tree.dispatch_raw_pointer_in_window(7, button_event(PRIMARY_POINTER_BUTTON));
    assert_eq!(taps.get(), 1);
}
