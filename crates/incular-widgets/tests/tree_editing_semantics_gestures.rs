//! Editing, semantics, and gesture tests.

mod common;

use common::*;
use incular_config::Constraints;
use incular_core::{Offset, Size};
use incular_image::ImageHandle;
use incular_rendering::{Brush, PaintCommand};
use incular_semantics::{SemanticActionKind, SemanticRole};
use incular_text::{TextAlign, TextEditingController, TextRange, TextSelection, TextStyle};
use incular_widgets::internal::*;
use incular_widgets::*;
use serde_json::json;
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};

#[test]
fn selectable_text_drag_uses_cached_parley_layout_and_copies_across_widgets() {
    let controller = SelectionAreaController::new();
    let mut tree = WidgetTree::new();
    let area = tree
        .mount(Widget::selection_area(
            controller.clone(),
            Widget::column(vec![
                Widget::selectable_text_styled(
                    "Latin café",
                    TextStyle::default(),
                    TextAlign::Start,
                ),
                Widget::selectable_text_styled(
                    "עברית mixed 世界",
                    TextStyle::default(),
                    TextAlign::Start,
                ),
            ]),
        ))
        .expect("mount selection area");
    tree.layout(Constraints::tight(Size::new(180., 80.)))
        .expect("layout");
    let column = tree.children(area).unwrap()[0];
    let labels = tree.children(column).unwrap();
    let first = labels[0];
    let second = labels[1];
    let first_origin = tree.element_bounds(first).unwrap().origin;
    let second_origin = tree.element_bounds(second).unwrap().origin;
    assert!(tree.selectable_text_set_selection(first, first_origin, false));
    assert!(tree.selectable_text_set_selection(
        second,
        second_origin + Offset::new(10_000., 0.),
        true,
    ));
    assert_eq!(controller.selected_text(), "Latin café\nעברית mixed 世界");
    let before = tree.text_diagnostics().layouts_requested;
    let _ = tree.paint();
    assert_eq!(tree.text_diagnostics().layouts_requested, before);
}

#[test]
fn selectable_text_keyboard_motion_stays_on_grapheme_boundaries() {
    let controller = SelectionAreaController::new();
    let mut tree = WidgetTree::new();
    let area = tree
        .mount(Widget::selection_area(
            controller.clone(),
            Widget::selectable_text_styled(
                "a👩\u{200d}💻b",
                TextStyle::default(),
                TextAlign::Start,
            ),
        ))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(180., 40.)))
        .expect("layout");
    let label = tree.children(area).unwrap()[0];
    assert!(tree.selectable_text_move(label, true, false));
    assert!(tree.selectable_text_move(label, true, true));
    assert_eq!(controller.selected_text(), "👩\u{200d}💻");
    assert!(tree.selectable_text_select_all(label));
    assert_eq!(controller.selected_text(), "a👩\u{200d}💻b");
}

#[test]
fn standalone_selectable_text_has_its_own_read_only_selection_region() {
    let mut tree = WidgetTree::new();
    let label = tree
        .mount(Widget::selectable_text_styled(
            "copy me",
            TextStyle::default(),
            TextAlign::Start,
        ))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 30.)))
        .expect("layout");
    assert!(tree.selectable_text_select_all(label));
    assert_eq!(
        tree.selectable_text_selected_text(label),
        Some("copy me".into())
    );
}

#[test]
fn editing_uses_graphemes_and_keeps_utf8_boundaries() {
    let controller = TextEditingController::with_text("a👩‍💻é");
    controller.move_end(false);
    controller.backspace();
    assert_eq!(controller.text(), "a👩‍💻");
    controller.backspace();
    assert_eq!(controller.text(), "a");
    controller.set_selection(TextSelection { base: 1, extent: 1 });
    controller.insert("नमस्ते");
    assert_eq!(controller.text(), "aनमस्ते");
    let value = controller.value();
    assert!(controller.text().is_char_boundary(value.selection.extent));
}

#[test]
fn ime_preedit_does_not_mutate_committed_text() {
    let controller = TextEditingController::with_text("hello");
    controller.set_preedit("世界", Some(TextRange::new(0, 3)));
    assert_eq!(controller.text(), "hello");
    assert_eq!(controller.preedit().as_deref(), Some("世界"));
    controller.commit_preedit("世界");
    assert_eq!(controller.text(), "hello世界");
    assert!(controller.preedit().is_none());
}

#[test]
fn text_editing_restoration_round_trips_committed_text_and_selection_only() {
    let scope = restoration_scope();
    let key = restoration_key("document");
    let controller = TextEditingController::with_text("seed");
    controller.bind_restoration(scope.clone(), key.clone());
    controller.set_selection(TextSelection::collapsed(1));
    controller.insert("β");
    controller.set_preedit("transient", Some(TextRange::new(0, 3)));

    assert_eq!(
        scope.get_json(&key),
        Some(json!({
            "text": "sβeed",
            "selection": { "base": 3, "extent": 3 },
        }))
    );

    let restored = TextEditingController::restored(scope, key);
    assert_eq!(restored.text(), "sβeed");
    assert_eq!(restored.value().selection, TextSelection::collapsed(3));
    assert!(restored.preedit().is_none());
}

#[test]
fn text_editing_restoration_normalizes_stale_utf8_selection_offsets() {
    let scope = restoration_scope();
    let key = restoration_key("document");
    scope.set_json(
        &key,
        json!({
            "text": "é",
            "selection": { "base": 99, "extent": 1 },
        }),
    );

    let restored = TextEditingController::restored(scope, key);
    assert_eq!(restored.text(), "é");
    // `base` clamps to the text end and `extent` to its preceding UTF-8
    // boundary; no invalid byte index reaches the editor.
    assert_eq!(
        restored.value().selection,
        TextSelection { base: 2, extent: 0 }
    );
}

#[test]
fn editing_newlines_and_boundaries_are_grapheme_safe() {
    let controller = TextEditingController::with_text("hello\nworld");
    controller.set_selection(TextSelection::collapsed(6));
    controller.backspace();
    assert_eq!(controller.text(), "helloworld");
    controller.set_selection(TextSelection { base: 2, extent: 5 });
    controller.insert("\n");
    assert_eq!(controller.text(), "he\nworld");
    controller.set_selection(TextSelection::collapsed(2));
    controller.delete();
    assert_eq!(controller.text(), "heworld");
}

#[test]
fn textarea_uses_shaped_lines_for_pointer_and_vertical_navigation() {
    let controller = TextEditingController::with_text("abcdef\nxy\n123456");
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            EditableText::new(controller.clone())
                .multiline(true)
                .size(Size::new(120., 100.))
                .into(),
        )
        .unwrap();
    tree.layout(Constraints::tight(Size::new(120., 100.)))
        .expect("layout");
    tree.text_field_set_caret(root, Offset::new(30., 10.), false, Instant::now());
    let initial = controller.value().selection.extent;
    assert!(initial > 0 && initial <= 6);
    assert!(tree.text_field_move_vertical(root, true, true));
    let selection = controller.value().selection;
    assert_eq!(selection.base, initial);
    assert!(
        selection.extent > 6 && selection.extent <= 9,
        "vertical move should land on the second shaped line: {selection:?}"
    );
    assert!(tree.text_field_move_line_edge(root, true, false));
    assert_eq!(controller.value().selection.extent, 9);
    tree.text_field_set_caret(root, Offset::new(5., 55.), false, Instant::now());
    assert!(controller.value().selection.extent >= 10);
}

#[test]
fn transparent_button_focus_is_an_outline_not_a_surface_fill() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            ActionSurface::with_child(Widget::fixed_box(Size::new(96., 32.), Color::WHITE))
                .color(Color::TRANSPARENT)
                .focused_color(Color::rgba(85, 150, 255, 200))
                .into(),
        )
        .unwrap();
    tree.layout(Constraints::tight(Size::new(96., 32.)))
        .expect("layout");
    tree.set_focused(root, true, Instant::now()).unwrap();
    let list = tree.paint();

    assert!(list.commands().iter().any(|command| {
        matches!(
            command,
            PaintCommand::Border { border, .. }
                if border.width == 2.0 && border.color == Color::rgba(85, 150, 255, 200)
        )
    }));
    assert!(!list.commands().iter().any(|command| {
        matches!(
            command,
            PaintCommand::RRect {
                brush: Brush::Solid(color),
                ..
            } if *color == Color::rgba(85, 150, 255, 200)
        )
    }));
    let content_index = list
        .commands()
        .iter()
        .position(|command| {
            matches!(
                command,
                PaintCommand::Rect { color, .. } if *color == Color::WHITE
            )
        })
        .expect("focused button content should be painted");
    let ring_index = list
        .commands()
        .iter()
        .position(|command| {
            matches!(
                command,
                PaintCommand::Border { border, .. }
                    if border.color == Color::rgba(85, 150, 255, 200)
            )
        })
        .expect("focused button should paint its outline");
    assert!(
        ring_index > content_index,
        "focus outline must overlay content"
    );
}

#[test]
fn focused_text_field_does_not_paint_framework_outline() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            EditableText::new(TextEditingController::with_text("value"))
                .size(Size::new(180., 32.))
                .into(),
        )
        .unwrap();
    tree.layout(Constraints::tight(Size::new(180., 32.)))
        .expect("layout");
    tree.set_focused(root, true, Instant::now()).unwrap();
    let list = tree.paint();

    assert!(
        !list
            .commands()
            .iter()
            .any(|command| matches!(command, PaintCommand::Border { .. }))
    );
    assert!(!list.commands().iter().any(|command| {
        matches!(
            command,
            PaintCommand::Rect { rect, color }
                if color == &Color::rgba(120, 170, 245, 220)
                    && (rect.size.width - 180.).abs() < f32::EPSILON
        )
    }));
}

#[test]
fn gesture_region_captures_a_hit_tested_pointer_sequence() {
    let taps = Rc::new(Cell::new(0));
    let observed = taps.clone();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::gesture(
            GestureCallbacks {
                on_tap: Some(Rc::new(move || observed.set(observed.get() + 1))),
                ..GestureCallbacks::default()
            },
            Widget::box_(Size::new(40., 40.), Color::WHITE),
        ))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    let now = Instant::now();
    assert_eq!(
        tree.dispatch_gesture(PointerEvent {
            pointer: 7,
            position: Offset::new(10., 10.),
            phase: incular_core::PointerPhase::Down,
            time: now,
        }),
        Some(root)
    );
    assert_eq!(
        tree.dispatch_gesture(PointerEvent {
            pointer: 7,
            position: Offset::new(80., 80.),
            phase: incular_core::PointerPhase::Up,
            time: now + Duration::from_millis(20),
        }),
        Some(root)
    );
    assert_eq!(taps.get(), 1);
}

#[test]
fn gesture_region_combines_identified_contacts_for_scale_updates() {
    let scale = Rc::new(Cell::new(0.));
    let observed = scale.clone();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::gesture(
            GestureCallbacks {
                on_scale_update: Some(Rc::new(move |details| observed.set(details.scale))),
                ..GestureCallbacks::default()
            },
            Widget::box_(Size::new(100., 100.), Color::WHITE),
        ))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    let now = Instant::now();
    for (pointer, position) in [(10, Offset::new(10., 10.)), (11, Offset::new(20., 10.))] {
        assert_eq!(
            tree.dispatch_gesture(PointerEvent {
                pointer,
                position,
                phase: incular_core::PointerPhase::Down,
                time: now,
            }),
            Some(root)
        );
    }
    assert_eq!(
        tree.dispatch_gesture(PointerEvent {
            pointer: 11,
            position: Offset::new(30., 10.),
            phase: incular_core::PointerPhase::Move,
            time: now + Duration::from_millis(16),
        }),
        Some(root)
    );
    assert_eq!(scale.get(), 2.);
    assert_eq!(
        tree.dispatch_gesture(PointerEvent {
            pointer: 10,
            position: Offset::new(10., 10.),
            phase: incular_core::PointerPhase::Up,
            time: now + Duration::from_millis(32),
        }),
        Some(root)
    );
}

#[test]
fn retained_arena_allows_drag_to_defeat_nested_tap_before_callbacks() {
    let taps = Rc::new(Cell::new(0));
    let pans = Rc::new(Cell::new(0));
    let cancelled = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    tree.mount(Widget::gesture(
        GestureCallbacks {
            on_pan_update: Some({
                let pans = pans.clone();
                Rc::new(move |_| pans.set(pans.get() + 1))
            }),
            ..GestureCallbacks::default()
        },
        Widget::gesture(
            GestureCallbacks {
                on_tap: Some({
                    let taps = taps.clone();
                    Rc::new(move || taps.set(taps.get() + 1))
                }),
                on_cancel: Some({
                    let cancelled = cancelled.clone();
                    Rc::new(move || cancelled.set(cancelled.get() + 1))
                }),
                ..GestureCallbacks::default()
            },
            Widget::box_(Size::new(100., 100.), Color::WHITE),
        ),
    ))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    let now = Instant::now();
    for (phase, position) in [
        (incular_core::PointerPhase::Down, Offset::new(10., 10.)),
        (incular_core::PointerPhase::Move, Offset::new(40., 10.)),
        (incular_core::PointerPhase::Up, Offset::new(40., 10.)),
    ] {
        assert!(
            tree.dispatch_gesture(PointerEvent {
                pointer: 1,
                position,
                phase,
                time: now
            })
            .is_some()
        );
    }
    assert_eq!(pans.get(), 1);
    assert_eq!(taps.get(), 0);
    assert_eq!(cancelled.get(), 1);
}

#[test]
fn retained_arena_arbitrates_horizontal_against_vertical_drag() {
    let horizontal = Rc::new(Cell::new(0));
    let vertical = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    tree.mount(Widget::gesture(
        GestureCallbacks {
            on_vertical_drag_update: Some({
                let vertical = vertical.clone();
                Rc::new(move |_| vertical.set(vertical.get() + 1))
            }),
            ..GestureCallbacks::default()
        },
        Widget::gesture(
            GestureCallbacks {
                on_horizontal_drag_update: Some({
                    let horizontal = horizontal.clone();
                    Rc::new(move |_| horizontal.set(horizontal.get() + 1))
                }),
                ..GestureCallbacks::default()
            },
            Widget::box_(Size::new(100., 100.), Color::WHITE),
        ),
    ))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    let now = Instant::now();
    for (phase, position) in [
        (incular_core::PointerPhase::Down, Offset::new(10., 10.)),
        (incular_core::PointerPhase::Move, Offset::new(12., 40.)),
        (incular_core::PointerPhase::Up, Offset::new(12., 40.)),
    ] {
        let _ = tree.dispatch_gesture(PointerEvent {
            pointer: 2,
            position,
            phase,
            time: now,
        });
    }
    assert_eq!(horizontal.get(), 0);
    assert_eq!(vertical.get(), 1);
}

#[test]
fn retained_arena_cancels_long_press_when_drag_claims_stream() {
    let long_presses = Rc::new(Cell::new(0));
    let cancellations = Rc::new(Cell::new(0));
    let pans = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    tree.mount(Widget::gesture(
        GestureCallbacks {
            on_pan_update: Some({
                let pans = pans.clone();
                Rc::new(move |_| pans.set(pans.get() + 1))
            }),
            ..GestureCallbacks::default()
        },
        Widget::gesture(
            GestureCallbacks {
                on_long_press: Some({
                    let long_presses = long_presses.clone();
                    Rc::new(move || long_presses.set(long_presses.get() + 1))
                }),
                on_cancel: Some({
                    let cancellations = cancellations.clone();
                    Rc::new(move || cancellations.set(cancellations.get() + 1))
                }),
                ..GestureCallbacks::default()
            },
            Widget::box_(Size::new(100., 100.), Color::WHITE),
        ),
    ))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    let now = Instant::now();
    let _ = tree.dispatch_gesture(PointerEvent {
        pointer: 3,
        position: Offset::new(10., 10.),
        phase: incular_core::PointerPhase::Down,
        time: now,
    });
    let _ = tree.dispatch_gesture(PointerEvent {
        pointer: 3,
        position: Offset::new(40., 10.),
        phase: incular_core::PointerPhase::Move,
        time: now + Duration::from_millis(100),
    });
    let _ = tree.dispatch_gesture(PointerEvent {
        pointer: 3,
        position: Offset::new(40., 10.),
        phase: incular_core::PointerPhase::Up,
        time: now + PointerGestureRecognizer::LONG_PRESS_TIMEOUT + Duration::from_millis(1),
    });
    assert_eq!(pans.get(), 1);
    assert_eq!(long_presses.get(), 0);
    assert_eq!(cancellations.get(), 1);
}

#[test]
fn retained_arena_allows_scale_to_defeat_pan_and_share_two_contacts() {
    let pans = Rc::new(Cell::new(0));
    let scales = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    tree.mount(Widget::gesture(
        GestureCallbacks {
            on_pan_update: Some({
                let pans = pans.clone();
                Rc::new(move |_| pans.set(pans.get() + 1))
            }),
            ..GestureCallbacks::default()
        },
        Widget::gesture(
            GestureCallbacks {
                on_scale_update: Some({
                    let scales = scales.clone();
                    Rc::new(move |_| scales.set(scales.get() + 1))
                }),
                ..GestureCallbacks::default()
            },
            Widget::box_(Size::new(100., 100.), Color::WHITE),
        ),
    ))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    let now = Instant::now();
    for (pointer, position) in [(10, Offset::new(10., 10.)), (11, Offset::new(20., 10.))] {
        let _ = tree.dispatch_gesture(PointerEvent {
            pointer,
            position,
            phase: incular_core::PointerPhase::Down,
            time: now,
        });
    }
    let _ = tree.dispatch_gesture(PointerEvent {
        pointer: 11,
        position: Offset::new(40., 10.),
        phase: incular_core::PointerPhase::Move,
        time: now + Duration::from_millis(16),
    });
    assert_eq!(pans.get(), 0);
    assert_eq!(scales.get(), 1);
}

#[test]
fn ignore_pointer_skips_its_subtree_and_reveals_a_stacked_target() {
    let behind = Rc::new(Cell::new(0));
    let ignored = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    tree.mount(Widget::stack(
        Alignment::CENTER,
        vec![
            Widget::gesture(
                GestureCallbacks {
                    on_tap: Some({
                        let behind = behind.clone();
                        Rc::new(move || behind.set(behind.get() + 1))
                    }),
                    ..GestureCallbacks::default()
                },
                Widget::box_(Size::new(100., 100.), Color::WHITE),
            ),
            IgnorePointer::new(Widget::gesture(
                GestureCallbacks {
                    on_tap: Some({
                        let ignored = ignored.clone();
                        Rc::new(move || ignored.set(ignored.get() + 1))
                    }),
                    ..GestureCallbacks::default()
                },
                Widget::box_(Size::new(100., 100.), Color::BLACK),
            ))
            .into(),
        ],
    ))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    let now = Instant::now();
    for phase in [
        incular_core::PointerPhase::Down,
        incular_core::PointerPhase::Up,
    ] {
        let _ = tree.dispatch_gesture(PointerEvent {
            pointer: 20,
            position: Offset::new(20., 20.),
            phase,
            time: now,
        });
    }
    assert_eq!(behind.get(), 1);
    assert_eq!(ignored.get(), 0);
}

#[test]
fn absorb_pointer_blocks_descendant_and_stacked_gesture_targets() {
    let behind = Rc::new(Cell::new(0));
    let absorbed_child = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    tree.mount(Widget::stack(
        Alignment::CENTER,
        vec![
            Widget::gesture(
                GestureCallbacks {
                    on_tap: Some({
                        let behind = behind.clone();
                        Rc::new(move || behind.set(behind.get() + 1))
                    }),
                    ..GestureCallbacks::default()
                },
                Widget::box_(Size::new(100., 100.), Color::WHITE),
            ),
            AbsorbPointer::new(Widget::gesture(
                GestureCallbacks {
                    on_tap: Some({
                        let absorbed_child = absorbed_child.clone();
                        Rc::new(move || absorbed_child.set(absorbed_child.get() + 1))
                    }),
                    ..GestureCallbacks::default()
                },
                Widget::box_(Size::new(100., 100.), Color::BLACK),
            ))
            .into(),
        ],
    ))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    let now = Instant::now();
    for phase in [
        incular_core::PointerPhase::Down,
        incular_core::PointerPhase::Up,
    ] {
        assert!(
            tree.dispatch_gesture(PointerEvent {
                pointer: 21,
                position: Offset::new(20., 20.),
                phase,
                time: now,
            })
            .is_none()
        );
    }
    assert_eq!(behind.get(), 0);
    assert_eq!(absorbed_child.get(), 0);
}

#[test]
fn retained_pointer_capture_is_window_local_and_released_with_the_stream() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::gesture(
            GestureCallbacks {
                on_tap: Some(Rc::new(|| {})),
                ..GestureCallbacks::default()
            },
            Widget::box_(Size::new(100., 100.), Color::WHITE),
        ))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    let now = Instant::now();
    let _ = tree.dispatch_gesture_in_window(
        9,
        PointerEvent {
            pointer: 5,
            position: Offset::new(10., 10.),
            phase: incular_core::PointerPhase::Down,
            time: now,
        },
    );
    assert_eq!(tree.pointer_capture_target(9, 5), Some(root));
    assert_eq!(tree.pointer_capture_target(10, 5), None);
    let capture = tree.request_pointer_capture(9, 5, root).unwrap();
    assert_eq!(capture.window(), 9);
    assert_eq!(capture.pointer(), 5);
    assert!(tree.release_pointer_capture(capture));
    assert_eq!(tree.pointer_capture_target(9, 5), None);
    let _ = tree.dispatch_gesture_in_window(
        9,
        PointerEvent {
            pointer: 5,
            position: Offset::new(90., 90.),
            phase: incular_core::PointerPhase::Up,
            time: now,
        },
    );
    assert_eq!(tree.pointer_capture_target(9, 5), None);
}

#[test]
fn typed_local_drag_drop_enters_updates_and_drops_through_the_arena() {
    let context = DragDropContext::new();
    let entered = Rc::new(Cell::new(0));
    let updates = Rc::new(Cell::new(0));
    let dropped = Rc::new(Cell::new(0));
    let ended = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    tree.mount(Widget::row(vec![
        Draggable::new(
            context.clone(),
            String::from("card"),
            Widget::box_(Size::new(100., 80.), Color::WHITE),
        )
        .feedback(|payload| Text::new(payload).into())
        .on_end({
            let ended = ended.clone();
            move |_| ended.set(ended.get() + 1)
        })
        .into(),
        DragTarget::new(
            context.clone(),
            Widget::box_(Size::new(100., 80.), Color::BLACK),
        )
        .on_enter({
            let entered = entered.clone();
            move |_| entered.set(entered.get() + 1)
        })
        .on_update({
            let updates = updates.clone();
            move |_, _| updates.set(updates.get() + 1)
        })
        .on_drop({
            let dropped = dropped.clone();
            move |payload| {
                assert_eq!(payload, "card");
                dropped.set(dropped.get() + 1);
            }
        })
        .into(),
    ]))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 80.)))
        .expect("layout");
    let now = Instant::now();
    for (phase, position) in [
        (incular_core::PointerPhase::Down, Offset::new(10., 20.)),
        (incular_core::PointerPhase::Move, Offset::new(35., 20.)),
        (incular_core::PointerPhase::Move, Offset::new(140., 20.)),
        (incular_core::PointerPhase::Up, Offset::new(140., 20.)),
    ] {
        let _ = tree.dispatch_gesture(PointerEvent {
            pointer: 30,
            position,
            phase,
            time: now,
        });
    }
    assert_eq!(entered.get(), 1);
    assert!(updates.get() >= 1);
    assert_eq!(dropped.get(), 1);
    assert_eq!(ended.get(), 1);
    assert!(!context.is_dragging());
    assert!(context.feedback().is_none());
}

#[test]
fn typed_local_drag_drop_leaves_and_cancels_without_drop() {
    let context = DragDropContext::new();
    let left = Rc::new(Cell::new(0));
    let cancelled = Rc::new(Cell::new(0));
    let dropped = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    tree.mount(Widget::row(vec![
        Draggable::new(
            context.clone(),
            7_u32,
            Widget::box_(Size::new(100., 80.), Color::WHITE),
        )
        .on_cancel({
            let cancelled = cancelled.clone();
            move |_| cancelled.set(cancelled.get() + 1)
        })
        .into(),
        DragTarget::new(context, Widget::box_(Size::new(100., 80.), Color::BLACK))
            .on_leave({
                let left = left.clone();
                move |_| left.set(left.get() + 1)
            })
            .on_drop({
                let dropped = dropped.clone();
                move |_| dropped.set(dropped.get() + 1)
            })
            .into(),
    ]))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 80.)))
        .expect("layout");
    let now = Instant::now();
    for (phase, position) in [
        (incular_core::PointerPhase::Down, Offset::new(10., 20.)),
        (incular_core::PointerPhase::Move, Offset::new(35., 20.)),
        (incular_core::PointerPhase::Move, Offset::new(140., 20.)),
        (incular_core::PointerPhase::Move, Offset::new(230., 20.)),
        (incular_core::PointerPhase::Cancel, Offset::new(230., 20.)),
    ] {
        let _ = tree.dispatch_gesture(PointerEvent {
            pointer: 31,
            position,
            phase,
            time: now,
        });
    }
    assert_eq!(left.get(), 1);
    assert_eq!(cancelled.get(), 1);
    assert_eq!(dropped.get(), 0);
}

#[test]
fn dismissible_claims_its_directional_drag_before_callback() {
    let dismissals = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    tree.mount(
        Dismissible::new(
            DismissDirection::Horizontal,
            Widget::box_(Size::new(120., 80.), Color::WHITE),
            {
                let dismissals = dismissals.clone();
                move |_| dismissals.set(dismissals.get() + 1)
            },
        )
        .threshold(40.)
        .into(),
    )
    .unwrap();
    tree.layout(Constraints::tight(Size::new(120., 80.)))
        .expect("layout");
    let now = Instant::now();
    for (phase, position) in [
        (incular_core::PointerPhase::Down, Offset::new(10., 20.)),
        (incular_core::PointerPhase::Move, Offset::new(70., 20.)),
        (incular_core::PointerPhase::Up, Offset::new(70., 20.)),
    ] {
        let _ = tree.dispatch_gesture(PointerEvent {
            pointer: 32,
            position,
            phase,
            time: now,
        });
    }
    assert_eq!(dismissals.get(), 1);
}

#[test]
fn explicit_merge_exclude_and_block_semantics_transform_the_retained_tree() {
    let dialog = Widget::box_(Size::new(80., 40.), Color::WHITE)
        .semantics(
            ExplicitSemantics::new(SemanticRole::Dialog)
                .label("Delete document")
                .actions([SemanticActionKind::Focus]),
        )
        .merge_semantics()
        .block_semantics();
    let background = ActionSurface::new("Save").into();
    let decorative: Widget = Text::new("sparkle").into();
    let decorative = decorative.exclude_semantics();
    let mut tree = WidgetTree::new();
    tree.mount(Widget::stack(
        Alignment::CENTER,
        vec![background, decorative, dialog],
    ))
    .expect("mount modal semantics");
    tree.layout(Constraints::tight(Size::new(160., 100.)))
        .expect("layout");
    tree.update_semantics();
    let nodes: Vec<_> = tree.semantics().iter().collect();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].1.role, SemanticRole::Dialog);
    assert_eq!(nodes[0].1.label.as_deref(), Some("Delete document"));
    assert_eq!(nodes[0].1.children.len(), 0);
}

#[test]
fn meaningful_images_are_semantic_but_unlabelled_images_are_decorative() {
    let image = ImageHandle::from_rgba8(1, 1, vec![255, 255, 255, 255]).unwrap();
    let mut tree = WidgetTree::new();
    tree.mount(Widget::row(vec![
        Image::new(image.clone()).into(),
        Widget::from(Image::new(image)).accessibility_label("Incular logo"),
    ]))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(80., 40.)))
        .expect("layout");
    tree.update_semantics();
    let images: Vec<_> = tree
        .semantics()
        .iter()
        .filter(|(_, node)| node.role == SemanticRole::Image)
        .collect();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].1.label.as_deref(), Some("Incular logo"));
}
