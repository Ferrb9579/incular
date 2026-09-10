//! Retained hover highlights for focus detectors through real tree
//! pointer dispatch.
//!
//! The existing hover sets in `dispatch_mouse_regions` track detector
//! behaviors alongside mouse regions: one map, one diff. Replacement
//! callbacks learn only subsequent transitions (consistent with focus
//! callbacks); synchronous reads expose current state.

use std::{cell::Cell, rc::Rc, time::Instant};

use incular_config::Constraints;
use incular_core::PointerPhase;
use incular_core::{Color, Offset, Size};
use incular_gestures::{PointerDeviceKind, RawPointerEvent};
use incular_widgets::{
    AbsorbPointer, FocusableActionDetector, Stack, Widget,
    internal::{ElementId, WidgetTree},
};

fn hover_at(position: Offset) -> RawPointerEvent {
    RawPointerEvent {
        pointer: 0,
        device: 1,
        kind: PointerDeviceKind::Mouse,
        buttons: 0,
        button: None,
        sample: incular_core::PointerSampleMetadata::default(),
        position,
        phase: PointerPhase::Move,
        time: Instant::now(),
    }
}

fn detector(
    highlights: Rc<Cell<Vec<bool>>>,
    build: impl FnOnce(FocusableActionDetector) -> FocusableActionDetector,
) -> Widget {
    let base = FocusableActionDetector::new(Widget::box_(Size::new(100., 100.), Color::WHITE))
        .on_show_hover_highlight(move |visible| {
            let mut current = highlights.take();
            current.push(visible);
            highlights.set(current);
        });
    build(base).into()
}

fn took(highlights: &Rc<Cell<Vec<bool>>>) -> Vec<bool> {
    highlights.take()
}

fn mount(tree: &mut WidgetTree, root: Widget) -> ElementId {
    let id = tree.mount(root).expect("mount");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    id
}

fn hover_sequence(tree: &mut WidgetTree) {
    // Outside, inside, inside again, outside.
    for point in [
        Offset::new(150., 150.),
        Offset::new(20., 20.),
        Offset::new(30., 30.),
        Offset::new(150., 150.),
    ] {
        let _ = tree.dispatch_raw_pointer_in_window(7, hover_at(point));
    }
}

#[test]
fn hover_enter_move_exit_notifies_once_each() {
    let highlights = Rc::new(Cell::new(Vec::new()));
    let mut tree = WidgetTree::new();
    mount(&mut tree, detector(highlights.clone(), |detector| detector));
    hover_sequence(&mut tree);
    assert_eq!(took(&highlights), vec![true, false]);
}

#[test]
fn rebuild_while_hovered_replays_no_enter() {
    let highlights = Rc::new(Cell::new(Vec::new()));
    let mut tree = WidgetTree::new();
    let root = mount(&mut tree, detector(highlights.clone(), |detector| detector));
    let _ = tree.dispatch_raw_pointer_in_window(7, hover_at(Offset::new(20., 20.)));
    assert_eq!(took(&highlights), vec![true]);

    // Same hovering, new behavior record: presence carries over silently.
    tree.update(root, detector(highlights.clone(), |detector| detector))
        .expect("rebuild while hovered");
    eprintln!("PROBE root={root:?} after={:?}", tree.root());
    let _ = tree.dispatch_raw_pointer_in_window(7, hover_at(Offset::new(30., 30.)));
    assert!(highlights.take().is_empty());
    let _ = tree.dispatch_raw_pointer_in_window(7, hover_at(Offset::new(150., 150.)));
    assert_eq!(took(&highlights), vec![false]);
}

#[test]
fn callback_replacement_while_hovered_learns_only_transitions() {
    let first = Rc::new(Cell::new(Vec::new()));
    let second = Rc::new(Cell::new(Vec::new()));
    let mut tree = WidgetTree::new();
    let root = mount(&mut tree, detector(first.clone(), |detector| detector));
    let _ = tree.dispatch_raw_pointer_in_window(7, hover_at(Offset::new(20., 20.)));
    assert_eq!(took(&first), vec![true]);

    // Replacement callbacks do not receive current state immediately;
    // the exit transition below reaches only the new generation.
    tree.update(root, detector(second.clone(), |detector| detector))
        .expect("replace callbacks while hovered");
    let _ = tree.dispatch_raw_pointer_in_window(7, hover_at(Offset::new(30., 30.)));
    assert!(first.take().is_empty());
    assert!(second.take().is_empty());
    let _ = tree.dispatch_raw_pointer_in_window(7, hover_at(Offset::new(150., 150.)));
    assert_eq!(took(&first), Vec::<bool>::new());
    assert_eq!(took(&second), vec![false]);
}

#[test]
fn disable_and_reenable_while_hovered() {
    let enabled = Rc::new(Cell::new(Vec::new()));
    let disabled = Rc::new(Cell::new(Vec::new()));
    let reenabled = Rc::new(Cell::new(Vec::new()));
    let mut tree = WidgetTree::new();
    let root = mount(&mut tree, detector(enabled.clone(), |detector| detector));
    let _ = tree.dispatch_raw_pointer_in_window(7, hover_at(Offset::new(20., 20.)));
    assert_eq!(took(&enabled), vec![true]);

    // Disabling carries presence silently; the new generation never
    // claimed visible, so nothing flips.
    tree.update(
        root,
        detector(disabled.clone(), |detector| detector.enabled(false)),
    )
    .expect("disable while hovered");
    let _ = tree.dispatch_raw_pointer_in_window(7, hover_at(Offset::new(30., 30.)));
    assert!(disabled.take().is_empty());

    // Re-enabling is equally silent; the next exit still notifies once.
    tree.update(root, detector(reenabled.clone(), |detector| detector))
        .expect("re-enable while hovered");
    let _ = tree.dispatch_raw_pointer_in_window(7, hover_at(Offset::new(150., 150.)));
    assert_eq!(took(&reenabled), vec![false]);
}

#[test]
fn unmount_while_hovered_clears_tracking() {
    let highlights = Rc::new(Cell::new(Vec::new()));
    let mut tree = WidgetTree::new();
    mount(&mut tree, detector(highlights.clone(), |detector| detector));
    let _ = tree.dispatch_raw_pointer_in_window(7, hover_at(Offset::new(20., 20.)));
    assert_eq!(took(&highlights), vec![true]);

    // Unmount drops the hovered id silently with the element, like mouse
    // regions: a remount re-enters instead of staying suppressed.
    tree.mount(Widget::box_(Size::new(10., 10.), Color::WHITE))
        .expect("replace root");
    let remounted = Rc::new(Cell::new(Vec::new()));
    mount(&mut tree, detector(remounted.clone(), |detector| detector));
    let _ = tree.dispatch_raw_pointer_in_window(7, hover_at(Offset::new(20., 20.)));
    assert_eq!(took(&remounted), vec![true]);
}

#[test]
fn nested_detectors_track_and_blocking_sibling_excludes() {
    let outer = Rc::new(Cell::new(Vec::new()));
    let inner = Rc::new(Cell::new(Vec::new()));
    let mut tree = WidgetTree::new();
    mount(
        &mut tree,
        FocusableActionDetector::new(Stack::new([
            Widget::box_(Size::new(100., 100.), Color::WHITE),
            FocusableActionDetector::new(Widget::box_(Size::new(40., 40.), Color::WHITE))
                .on_show_hover_highlight({
                    let inner = inner.clone();
                    move |visible| {
                        let mut current = inner.take();
                        current.push(visible);
                        inner.set(current);
                    }
                })
                .into(),
        ]))
        .on_show_hover_highlight({
            let outer = outer.clone();
            move |visible| {
                let mut current = outer.take();
                current.push(visible);
                outer.set(current);
            }
        })
        .into(),
    );
    // Inside both: both enter.
    let _ = tree.dispatch_raw_pointer_in_window(7, hover_at(Offset::new(10., 10.)));
    assert_eq!(took(&inner), vec![true]);
    assert_eq!(took(&outer), vec![true]);
    // Outer only: inner exits, outer stays without a duplicate enter.
    let _ = tree.dispatch_raw_pointer_in_window(7, hover_at(Offset::new(80., 80.)));
    assert_eq!(took(&inner), vec![false]);
    assert!(outer.take().is_empty());
    // Outside: outer exits.
    let _ = tree.dispatch_raw_pointer_in_window(7, hover_at(Offset::new(150., 150.)));
    assert_eq!(took(&outer), vec![false]);

    // An absorbing overlay owns the hit: the detector behind never enters.
    let blocked = Rc::new(Cell::new(Vec::new()));
    let mut tree = WidgetTree::new();
    mount(
        &mut tree,
        Stack::new([
            detector(blocked.clone(), |detector| detector),
            AbsorbPointer::new(Widget::box_(Size::new(100., 100.), Color::WHITE))
                .absorbing(true)
                .into(),
        ])
        .into(),
    );
    let _ = tree.dispatch_raw_pointer_in_window(7, hover_at(Offset::new(20., 20.)));
    assert!(blocked.take().is_empty());
}

#[test]
fn highlight_mode_change_notifies_stationary_pointer() {
    use incular_gestures::{FocusHighlightManager, FocusHighlightMode};
    let highlights = Rc::new(Cell::new(Vec::new()));
    let mut tree = WidgetTree::new();
    mount(&mut tree, detector(highlights.clone(), |detector| detector));
    let _ = tree.dispatch_raw_pointer_in_window(7, hover_at(Offset::new(20., 20.)));
    assert_eq!(took(&highlights), vec![true]);

    // No pointer events between these flips: the mode subscription alone
    // drives the highlight callbacks.
    FocusHighlightManager::new().set_mode(FocusHighlightMode::Touch);
    assert_eq!(took(&highlights), vec![false]);
    FocusHighlightManager::new().set_mode(FocusHighlightMode::Traditional);
    assert_eq!(took(&highlights), vec![true]);

    let _ = tree.dispatch_raw_pointer_in_window(7, hover_at(Offset::new(150., 150.)));
    assert_eq!(took(&highlights), vec![false]);
}
