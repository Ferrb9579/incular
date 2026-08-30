use incular_core::{Code, KeyboardEvent, Modifiers, Offset, PointerPhase, Rect};
use incular_gestures::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

#[test]
fn shortcuts_only_dispatch_matching_pressed_events() {
    let count = Rc::new(Cell::new(0));
    let mut shortcuts = Shortcuts::new();
    let expected = count.clone();
    shortcuts.register(
        ShortcutKey::new(Code::KeyA, Modifiers::default()),
        move || expected.set(expected.get() + 1),
    );
    assert!(shortcuts.handle(KeyboardEvent::key_down(
        incular_core::KeyboardKey::Named(incular_core::NamedKey::Unidentified),
        Code::KeyA,
    )));
    assert_eq!(count.get(), 1);
}
#[test]
fn focus_manager_keeps_one_owner_and_skips_excluded_nodes() {
    let first = FocusNode::new();
    let skipped = FocusNode::new();
    let last = FocusNode::new();
    skipped.set_can_request_focus(false);
    let mut scope = FocusManager::new();
    for node in [&first, &skipped, &last] {
        scope.register(node);
    }
    assert!(scope.request_focus(&first));
    assert!(first.has_focus());
    assert_eq!(scope.focus_next(false), Some(last.clone()));
    assert!(!first.has_focus());
    assert!(last.has_focus());
    assert_eq!(scope.focus_next(true), Some(first.clone()));
    drop(last);
    assert_eq!(scope.registered_count(), 2);
}

#[test]
fn focus_manager_applies_reading_and_explicit_order_policies() {
    let top_right = FocusNode::new();
    let bottom_left = FocusNode::new();
    let top_left = FocusNode::new();
    top_right.set_rect(Rect::from_origin_size(
        Offset::new(100., 0.),
        incular_core::Size::new(20., 20.),
    ));
    bottom_left.set_rect(Rect::from_origin_size(
        Offset::new(0., 50.),
        incular_core::Size::new(20., 20.),
    ));
    top_left.set_rect(Rect::from_origin_size(
        Offset::new(0., 0.),
        incular_core::Size::new(20., 20.),
    ));
    let mut reading = FocusManager::with_policy(FocusTraversalPolicyKind::ReadingOrder);
    for node in [&top_right, &bottom_left, &top_left] {
        reading.register(node);
    }
    assert_eq!(reading.focus_next(false), Some(top_left.clone()));
    assert_eq!(reading.focus_next(false), Some(top_right.clone()));
    assert_eq!(reading.focus_next(false), Some(bottom_left.clone()));

    let first = FocusNode::new();
    let second = FocusNode::new();
    first.set_traversal_order(Some(20.));
    second.set_traversal_order(Some(10.));
    let mut ordered = FocusManager::with_policy(FocusTraversalPolicyKind::Ordered);
    ordered.register(&first);
    ordered.register(&second);
    assert_eq!(ordered.focus_next(false), Some(second));
    assert_eq!(ordered.focus_next(false), Some(first));
}
#[test]
fn shortcuts_can_dispatch_typed_commands() {
    let count = Rc::new(Cell::new(0));
    let mut actions = Actions::new();
    actions.register("save", {
        let count = count.clone();
        move || count.set(count.get() + 1)
    });
    let mut shortcuts = Shortcuts::new();
    shortcuts.bind(ShortcutKey::new(Code::KeyA, Modifiers::default()), "save");
    assert!(shortcuts.handle_actions(
        KeyboardEvent::key_down(
            incular_core::KeyboardKey::Named(incular_core::NamedKey::Unidentified),
            Code::KeyA,
        ),
        &actions,
    ));
    assert_eq!(count.get(), 1);
}
#[test]
fn generic_commands_dispatch_ctrl_s_without_string_conversion() {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum EditorCommand {
        Save,
    }

    let count = Rc::new(Cell::new(0));
    let mut actions = Actions::<EditorCommand>::typed();
    let observed = count.clone();
    actions.register(EditorCommand::Save, move || {
        observed.set(observed.get() + 1)
    });
    let mut shortcuts = Shortcuts::<EditorCommand>::typed();
    shortcuts.bind(
        ShortcutKey::new(Code::KeyS, Modifiers::CONTROL),
        EditorCommand::Save,
    );

    let mut event = KeyboardEvent::key_down(
        incular_core::KeyboardKey::Named(incular_core::NamedKey::Unidentified),
        Code::KeyS,
    );
    event.modifiers = Modifiers::CONTROL;
    assert!(shortcuts.handle_actions(event, &actions));
    assert_eq!(count.get(), 1);
}
#[test]
fn actions_dispatch_nearest_scope_first() {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let outer_observed = observed.clone();
    let mut actions = Actions::new();
    actions.register("open", move || outer_observed.borrow_mut().push("outer"));
    actions.push_scope();
    let inner_observed = observed.clone();
    actions.register("open", move || inner_observed.borrow_mut().push("inner"));

    assert!(actions.invoke(&Command::new("open")));
    assert_eq!(&*observed.borrow(), &["inner"]);
}

#[test]
fn action_invocation_listeners_observe_start_and_end() {
    let phases = Rc::new(RefCell::new(Vec::new()));
    let action = Action::<Command>::new("save", || {});
    let observed = phases.clone();
    let _subscription = action.add_invocation_listener(move |phase| {
        observed.borrow_mut().push(phase);
    });

    assert_eq!(
        action.invoke(&Intent::<Command>::from("save")),
        ActionResult::Handled
    );
    assert_eq!(
        &*phases.borrow(),
        &[ActionInvocationPhase::Start, ActionInvocationPhase::End]
    );
}

#[test]
fn disabled_or_ignored_actions_fall_through_to_outer_scope() {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let outer_observed = observed.clone();
    let mut actions = Actions::new();
    actions.register("save", move || outer_observed.borrow_mut().push("outer"));
    actions.push_scope();
    let disabled = Action::new("save", {
        let observed = observed.clone();
        move || observed.borrow_mut().push("disabled")
    });
    disabled.set_enabled(false);
    actions.register_action(disabled);
    assert!(actions.invoke(&Command::new("save")));
    assert_eq!(&*observed.borrow(), &["outer"]);

    actions.register_handler("save", |_| ActionResult::Ignored);
    assert!(actions.invoke(&Command::new("save")));
    assert_eq!(&*observed.borrow(), &["outer", "outer"]);

    actions.register_handler("save", |_| false);
    assert!(actions.invoke(&Command::new("save")));
    assert_eq!(&*observed.borrow(), &["outer", "outer", "outer"]);

    let ignored = Action::<Command>::with_handler("save", |_| false);
    assert_eq!(
        ignored.invoke(&Intent::<Command>::from("save")),
        ActionResult::Ignored
    );
}

#[test]
fn temporary_action_scope_restores_after_callback() {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let outer_observed = observed.clone();
    let mut actions = Actions::new();
    actions.register("toggle", move || outer_observed.borrow_mut().push("outer"));
    actions.with_scope(|actions| {
        let inner_observed = observed.clone();
        actions.register("toggle", move || inner_observed.borrow_mut().push("inner"));
        assert_eq!(actions.scope_depth(), 2);
        assert!(actions.invoke(&Command::new("toggle")));
    });
    assert_eq!(actions.scope_depth(), 1);
    assert!(actions.invoke(&Command::new("toggle")));
    assert_eq!(&*observed.borrow(), &["inner", "outer"]);
}

#[test]
fn shortcuts_distinguish_press_repeat_and_release() {
    let presses = Rc::new(Cell::new(0));
    let repeats = Rc::new(Cell::new(0));
    let releases = Rc::new(Cell::new(0));
    let mut shortcuts = Shortcuts::new();
    let observed_presses = presses.clone();
    shortcuts.register_on(
        ShortcutKey::new(Code::KeyA, Modifiers::default()),
        ShortcutTrigger::Press,
        move || observed_presses.set(observed_presses.get() + 1),
    );
    let observed_repeats = repeats.clone();
    shortcuts.register_on(
        ShortcutKey::new(Code::KeyA, Modifiers::default()),
        ShortcutTrigger::Repeat,
        move || observed_repeats.set(observed_repeats.get() + 1),
    );
    let observed_releases = releases.clone();
    shortcuts.register_on(
        ShortcutKey::new(Code::KeyA, Modifiers::default()),
        ShortcutTrigger::Up,
        move || observed_releases.set(observed_releases.get() + 1),
    );

    assert!(shortcuts.handle(KeyboardEvent::key_down(
        incular_core::KeyboardKey::Named(incular_core::NamedKey::Unidentified),
        Code::KeyA,
    )));
    let mut repeat = KeyboardEvent::key_down(
        incular_core::KeyboardKey::Named(incular_core::NamedKey::Unidentified),
        Code::KeyA,
    );
    repeat.repeat = true;
    assert!(shortcuts.handle(repeat));
    assert!(shortcuts.handle(KeyboardEvent::key_up(
        incular_core::KeyboardKey::Named(incular_core::NamedKey::Unidentified),
        Code::KeyA,
    )));
    assert_eq!(presses.get(), 1);
    assert_eq!(repeats.get(), 1);
    assert_eq!(releases.get(), 1);
}

#[test]
fn logical_shortcut_and_nested_scope_fallthrough_are_supported() {
    let observed = Rc::new(Cell::new(0));
    let mut actions = Actions::new();
    let outer = observed.clone();
    actions.register("save", move || outer.set(outer.get() + 1));

    let mut shortcuts = Shortcuts::new();
    shortcuts.bind_logical(
        LogicalShortcutKey::new(
            incular_core::KeyboardKey::Character("s".into()),
            Modifiers::CONTROL,
        ),
        "save",
    );
    shortcuts.push_scope();
    shortcuts.bind_on(
        ShortcutKey::new(Code::KeyS, Modifiers::CONTROL),
        ShortcutTrigger::Press,
        Intent::new("unhandled"),
    );

    let mut event =
        KeyboardEvent::key_down(incular_core::KeyboardKey::Character("s".into()), Code::KeyS);
    event.modifiers = Modifiers::CONTROL;
    assert!(shortcuts.handle_actions(event, &actions));
    assert_eq!(observed.get(), 1);
}

#[test]
fn focus_scope_node_restores_previous_focus() {
    let scope = FocusScopeNode::new();
    let first = FocusNode::new();
    let second = FocusNode::new();
    scope.register(&first);
    scope.register(&second);
    assert!(scope.request_focus(&first));
    scope.clear_focus();
    assert!(scope.focused().is_none());
    assert!(scope.restore_focus());
    assert_eq!(scope.focused(), Some(first));
    assert_eq!(scope.focus_next(false), Some(second.clone()));
    assert_eq!(scope.focused(), Some(second));
}

#[test]
fn focus_scope_observer_tracks_revision_and_stops_after_drop() {
    let scope = FocusScopeNode::new();
    let node = FocusNode::new();
    let notifications = Rc::new(Cell::new(0));
    let observed = notifications.clone();
    let subscription = scope.observe(move || observed.set(observed.get() + 1));
    let initial_revision = scope.revision();

    scope.register(&node);
    assert!(scope.revision() > initial_revision);
    assert_eq!(notifications.get(), 1);
    assert!(scope.request_focus(&node));
    assert_eq!(notifications.get(), 2);

    drop(subscription);
    scope.clear_focus();
    assert_eq!(notifications.get(), 2);
    assert!(scope.revision() > initial_revision);
}

#[test]
fn nested_focus_scope_restores_parent_child() {
    let parent = FocusScopeNode::new();
    let parent_child = FocusNode::new();
    parent.register(&parent_child);
    assert!(parent.request_focus(&parent_child));

    let child = FocusScopeNode::nested(&parent);
    let child_node = FocusNode::new();
    child.register(&child_node);
    assert!(child.request_focus(&child_node));
    assert!(parent.focused().is_none());
    child.unfocus();
    assert!(child.restore_parent_focus());
    assert_eq!(parent.focused(), Some(parent_child));
}
#[test]
fn scale_recognizer_reports_relative_two_pointer_distance() {
    let observed = Rc::new(Cell::new(0.));
    let expected = observed.clone();
    let mut detector = ScaleGestureDetector::new(move |details| expected.set(details.scale));
    let now = Instant::now();
    for (pointer, position) in [(1, Offset::new(0., 0.)), (2, Offset::new(10., 0.))] {
        assert!(detector.handle(PointerEvent {
            pointer,
            position,
            phase: PointerPhase::Down,
            time: now,
        }));
    }
    assert!(detector.handle(PointerEvent {
        pointer: 2,
        position: Offset::new(20., 0.),
        phase: PointerPhase::Move,
        time: now,
    }));
    assert_eq!(observed.get(), 2.);
}

#[test]
fn arena_rejects_competing_drag_but_keeps_compatible_scale_members() {
    let key = GestureArenaKey {
        window: 7,
        pointer: 3,
    };
    let mut arena = GestureArena::new();
    let tap = arena.add(key, false);
    let drag = arena.add(key, false);
    let entries = arena.accept(key, drag);
    assert!(
        entries
            .iter()
            .any(|entry| entry.member == drag && entry.disposition == GestureDisposition::Accepted)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.member == tap && entry.disposition == GestureDisposition::Rejected)
    );
    let scale_a = arena.add(key, true);
    let scale_b = arena.add(key, true);
    let entries = arena.accept(key, scale_a);
    assert!(
        entries.iter().any(
            |entry| entry.member == scale_b && entry.disposition == GestureDisposition::Pending
        )
    );
    assert_eq!(arena.cancel(key).len(), 4);
}
#[test]
fn drag_recognizer_reports_start_updates_and_end() {
    let starts = Rc::new(Cell::new(0));
    let total = Rc::new(Cell::new(Offset::ZERO));
    let ended = Rc::new(Cell::new(false));
    let mut detector = DragGestureDetector::new(DragCallbacks {
        on_start: Some({
            let starts = starts.clone();
            Rc::new(move |_| starts.set(starts.get() + 1))
        }),
        on_update: Some({
            let total = total.clone();
            Rc::new(move |details| total.set(details.total_delta))
        }),
        on_end: Some({
            let ended = ended.clone();
            Rc::new(move |details| ended.set(!details.cancelled))
        }),
    });
    let now = Instant::now();
    for (phase, position) in [
        (PointerPhase::Down, Offset::ZERO),
        (PointerPhase::Move, Offset::new(20., 0.)),
        (PointerPhase::Up, Offset::new(25., 0.)),
    ] {
        assert!(detector.handle(PointerEvent {
            pointer: 1,
            position,
            phase,
            time: now + Duration::from_millis(10),
        }));
    }
    assert_eq!(starts.get(), 1);
    assert_eq!(total.get(), Offset::new(20., 0.));
    assert!(ended.get());
}

#[test]
fn pointer_recognizer_reports_drag_end_after_arena_updates() {
    let ended = Rc::new(Cell::new(None));
    let mut recognizer = PointerGestureRecognizer::new(GestureCallbacks {
        on_horizontal_drag_update: Some(Rc::new(|_| {})),
        on_horizontal_drag_end: Some({
            let ended = ended.clone();
            Rc::new(move |details| ended.set(Some(details)))
        }),
        ..GestureCallbacks::default()
    });
    let now = Instant::now();
    assert_eq!(
        recognizer.observe(PointerEvent {
            pointer: 1,
            position: Offset::ZERO,
            phase: PointerPhase::Down,
            time: now,
        }),
        GestureDecision::Pending
    );
    assert!(matches!(
        recognizer.observe(PointerEvent {
            pointer: 1,
            position: Offset::new(24., 0.),
            phase: PointerPhase::Move,
            time: now + Duration::from_millis(10),
        }),
        GestureDecision::Accept(GestureAction::HorizontalDrag(_))
    ));
    let end = recognizer.observe(PointerEvent {
        pointer: 1,
        position: Offset::new(30., 0.),
        phase: PointerPhase::Up,
        time: now + Duration::from_millis(20),
    });
    assert!(matches!(
        end,
        GestureDecision::Accept(GestureAction::HorizontalDragEnd(_))
    ));
    if let GestureDecision::Accept(action) = end {
        recognizer.dispatch(action);
    }
    let details = ended.get().expect("drag end callback");
    assert_eq!(details.total_delta, Offset::new(30., 0.));
    assert!(!details.cancelled);
}
