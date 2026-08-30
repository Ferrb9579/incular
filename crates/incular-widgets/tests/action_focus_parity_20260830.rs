//! Focused regression coverage for the Flutter-parity action and focus widgets.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use incular_config::Constraints;
use incular_core::{Code, Color, KeyboardEvent, KeyboardKey, Modifiers, Size};
use incular_widgets::internal::{Widget, WidgetTree};
use incular_widgets::{
    Action, ActionListener, ActionResult, Actions, CallbackShortcuts, Command, FocusNode,
    FocusScope, FocusTraversalOrder, FocusableActionDetector, Intent, LogicalShortcutKey,
    ShortcutActivator, ShortcutKey, ShortcutTrigger, Shortcuts, SingleActivator, SizedBox,
};

fn key_down(key: KeyboardKey, code: Code, modifiers: Modifiers) -> KeyboardEvent {
    let mut event = KeyboardEvent::key_down(key, code);
    event.modifiers = modifiers;
    event
}

fn save_intent() -> Intent {
    Intent::new(Command::new("save"))
}

#[test]
fn action_listener_callbacks_follow_invocation_lifecycle_and_unmount() {
    let events = Rc::new(RefCell::new(Vec::<&'static str>::new()));
    let action = Action::new(Command::new("save"), {
        let events = events.clone();
        move || events.borrow_mut().push("handler")
    });
    let listener = ActionListener::new(action.clone(), SizedBox::shrink())
        .on_action_start({
            let events = events.clone();
            move || events.borrow_mut().push("start")
        })
        .on_action({
            let events = events.clone();
            move || events.borrow_mut().push("action")
        })
        .on_action_end({
            let events = events.clone();
            move || events.borrow_mut().push("end")
        });

    let mut tree = WidgetTree::new();
    let root = tree.mount(listener.into()).expect("mount action listener");

    assert_eq!(action.invoke(&save_intent()), ActionResult::Handled);
    assert_eq!(&*events.borrow(), &["start", "handler", "action", "end"]);

    events.borrow_mut().clear();
    let rebuilt = ActionListener::new(action.clone(), SizedBox::shrink()).on_action({
        let events = events.clone();
        move || events.borrow_mut().push("rebuilt")
    });
    tree.update(root, rebuilt.into())
        .expect("rebuild action listener");
    assert_eq!(
        tree.root(),
        Some(root),
        "rebuild should retain element identity"
    );

    assert_eq!(action.invoke(&save_intent()), ActionResult::Handled);
    assert_eq!(&*events.borrow(), &["handler", "rebuilt"]);

    events.borrow_mut().clear();
    tree.mount(SizedBox::shrink().into()).expect("replace root");
    assert!(!tree.element_exists(root));
    assert_eq!(action.invoke(&save_intent()), ActionResult::Handled);
    assert_eq!(&*events.borrow(), &["handler"]);
}

#[test]
fn callback_shortcuts_match_physical_logical_and_single_activators() {
    let physical_hits = Rc::new(Cell::new(0));
    let logical_hits = Rc::new(Cell::new(0));
    let single_hits = Rc::new(Cell::new(0));

    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            CallbackShortcuts::new(SizedBox::shrink())
                .shortcut(
                    ShortcutActivator::physical_on(
                        ShortcutKey::new(Code::KeyK, Modifiers::CONTROL),
                        ShortcutTrigger::Press,
                    ),
                    {
                        let hits = physical_hits.clone();
                        move || hits.set(hits.get() + 1)
                    },
                )
                .shortcuts(vec![(
                    ShortcutActivator::logical_on(
                        LogicalShortcutKey::new(KeyboardKey::Character("x".into()), Modifiers::ALT),
                        ShortcutTrigger::Repeat,
                    ),
                    Box::new({
                        let hits = logical_hits.clone();
                        move || hits.set(hits.get() + 1)
                    }) as Box<dyn Fn()>,
                )])
                .shortcut(
                    SingleActivator::new(KeyboardKey::Character("y".into()), Modifiers::SHIFT)
                        .with_repeats(false),
                    {
                        let hits = single_hits.clone();
                        move || hits.set(hits.get() + 1)
                    },
                )
                .into(),
        )
        .expect("mount callback shortcuts");

    let physical = key_down(
        KeyboardKey::Named(incular_core::NamedKey::Unidentified),
        Code::KeyK,
        Modifiers::CONTROL,
    );
    let mut physical_repeat = physical.clone();
    physical_repeat.repeat = true;
    assert!(!tree.dispatch_keyboard(Some(root), physical_repeat));
    assert_eq!(physical_hits.get(), 0);
    assert!(tree.dispatch_keyboard(Some(root), physical));
    assert_eq!(physical_hits.get(), 1);

    let mut logical_repeat = key_down(
        KeyboardKey::Character("x".into()),
        Code::KeyX,
        Modifiers::ALT,
    );
    logical_repeat.repeat = true;
    assert!(tree.dispatch_keyboard(Some(root), logical_repeat));
    assert_eq!(logical_hits.get(), 1);

    let single = key_down(
        KeyboardKey::Character("y".into()),
        Code::KeyY,
        Modifiers::SHIFT,
    );
    assert!(tree.dispatch_keyboard(Some(root), single));
    assert_eq!(single_hits.get(), 1);

    let mut single_repeat = key_down(
        KeyboardKey::Character("y".into()),
        Code::KeyY,
        Modifiers::SHIFT,
    );
    single_repeat.repeat = true;
    assert!(!tree.dispatch_keyboard(Some(root), single_repeat));
    assert_eq!(single_hits.get(), 1);
}

// CallbackShortcuts intentionally exposes only `Fn()` values, so it has no
// public typed-intent binding hook. Typed intent dispatch is covered through
// FocusableActionDetector's public Shortcuts/Actions configuration below;
// adding a typed CallbackShortcuts API would require a shared-source change.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum TestCommand {
    Submit,
}

#[test]
fn focusable_action_detector_dispatches_typed_intents_through_configured_scopes() {
    let mut shortcuts = Shortcuts::<TestCommand>::typed();
    shortcuts.bind_logical(
        LogicalShortcutKey::new(KeyboardKey::Character("s".into()), Modifiers::CONTROL),
        TestCommand::Submit,
    );

    let received = Rc::new(RefCell::new(Vec::new()));
    let mut actions = Actions::<TestCommand>::typed();
    actions.register_handler(TestCommand::Submit, {
        let received = received.clone();
        move |intent| {
            received.borrow_mut().push(*intent.command());
            ActionResult::Handled
        }
    });
    let actions = Rc::new(actions);
    let action = actions
        .action(&TestCommand::Submit)
        .expect("typed action registration");

    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            FocusableActionDetector::new(SizedBox::shrink())
                .shortcuts(Rc::new(shortcuts))
                .actions(actions.clone())
                .into(),
        )
        .expect("mount focusable action detector");

    assert!(!tree.dispatch_keyboard(
        Some(root),
        key_down(
            KeyboardKey::Character("n".into()),
            Code::KeyN,
            Modifiers::CONTROL,
        ),
    ));
    assert!(received.borrow().is_empty());

    assert!(tree.dispatch_keyboard(
        Some(root),
        key_down(
            KeyboardKey::Character("s".into()),
            Code::KeyS,
            Modifiers::CONTROL,
        ),
    ));
    assert_eq!(&*received.borrow(), &[TestCommand::Submit]);

    action.set_enabled(false);
    assert!(!tree.dispatch_keyboard(
        Some(root),
        key_down(
            KeyboardKey::Character("s".into()),
            Code::KeyS,
            Modifiers::CONTROL,
        ),
    ));
    assert_eq!(&*received.borrow(), &[TestCommand::Submit]);
}

#[test]
fn focusable_action_detector_honors_enabled_autofocus_and_focus_callbacks() {
    let node = FocusNode::new();
    let focus_changes = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            FocusableActionDetector::new(SizedBox::shrink())
                .focus_node(node.clone())
                .enabled(false)
                .autofocus(true)
                .on_focus_change({
                    let focus_changes = focus_changes.clone();
                    move |focused| focus_changes.borrow_mut().push(focused)
                })
                .into(),
        )
        .expect("mount disabled detector");

    assert!(!node.is_enabled());
    assert!(!node.can_request_focus());
    assert!(tree.focusable_elements().is_empty());
    assert_eq!(tree.autofocus_element(), None);
    tree.set_keyboard_focus(root, true);
    assert!(!node.has_focus());

    tree.update(
        root,
        FocusableActionDetector::new(SizedBox::shrink())
            .focus_node(node.clone())
            .enabled(true)
            .autofocus(true)
            .on_focus_change({
                let focus_changes = focus_changes.clone();
                move |focused| focus_changes.borrow_mut().push(focused)
            })
            .into(),
    )
    .expect("enable detector");
    assert!(node.is_enabled());
    assert!(node.can_request_focus());
    assert_eq!(tree.focusable_elements(), vec![root]);
    assert_eq!(tree.autofocus_element(), Some(root));

    tree.set_keyboard_focus(root, true);
    tree.set_keyboard_focus(root, false);
    assert_eq!(&*focus_changes.borrow(), &[true, false]);
}

#[test]
fn focusable_action_detector_rebuild_retains_element_and_focus_node_identity() {
    let node = FocusNode::new();
    let old_changes = Rc::new(RefCell::new(Vec::new()));
    let new_changes = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            FocusableActionDetector::new(SizedBox::shrink())
                .focus_node(node.clone())
                .autofocus(true)
                .on_focus_change({
                    let old_changes = old_changes.clone();
                    move |focused| old_changes.borrow_mut().push(focused)
                })
                .into(),
        )
        .expect("mount detector");

    tree.set_keyboard_focus(root, true);
    assert!(node.has_focus());
    assert_eq!(&*old_changes.borrow(), &[true]);

    tree.update(
        root,
        FocusableActionDetector::new(SizedBox::shrink())
            .focus_node(node.clone())
            .autofocus(true)
            .on_focus_change({
                let new_changes = new_changes.clone();
                move |focused| new_changes.borrow_mut().push(focused)
            })
            .into(),
    )
    .expect("rebuild detector");

    assert_eq!(tree.root(), Some(root));
    assert!(tree.element_exists(root));
    assert_eq!(tree.focusable_elements(), vec![root]);
    assert_eq!(tree.autofocus_element(), Some(root));
    assert!(
        node.has_focus(),
        "external FocusNode state should survive rebuild"
    );

    tree.set_keyboard_focus(root, false);
    assert_eq!(&*old_changes.borrow(), &[true]);
    assert_eq!(&*new_changes.borrow(), &[false]);
}

#[test]
fn focus_scope_orders_descendants_and_selects_scope_autofocus_candidate() {
    let first_node = FocusNode::new();
    let second_node = FocusNode::new();
    let first = FocusTraversalOrder::new(
        FocusableActionDetector::new(Widget::box_(Size::new(10., 10.), Color::WHITE))
            .focus_node(first_node.clone()),
    )
    .order(20.);
    let second = FocusTraversalOrder::new(
        FocusableActionDetector::new(Widget::box_(Size::new(10., 10.), Color::WHITE))
            .focus_node(second_node.clone()),
    )
    .order(10.);

    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            FocusScope::new(Widget::row(vec![first.into(), second.into()]))
                .ordered()
                .autofocus(true)
                .into(),
        )
        .expect("mount focus scope");
    tree.layout(Constraints::tight(Size::new(40., 10.)));

    let children = tree.children(root).expect("scope children").to_vec();
    assert_eq!(children.len(), 2);
    assert_eq!(tree.focusable_elements(), vec![children[1], children[0]]);
    assert_eq!(tree.autofocus_element(), Some(children[1]));

    tree.set_keyboard_focus(children[1], true);
    assert!(!first_node.has_focus());
    assert!(second_node.has_focus());
}
