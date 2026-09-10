//! Subscription ownership and scope behavior for the retained
//! shortcut/action wrappers: CallbackShortcuts, ActionListener, and
//! FocusableActionDetector.
//!
//! All assertions drive production dispatch (keyboard routing, action
//! invocation, focus nodes) and observe application callbacks. Existing
//! action/focus surface tests cover first-mount matching; these cover
//! replacement, nesting, unmount release, reentrancy, and detector
//! enablement.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use incular_config::Constraints;
use incular_core::{Code, KeyboardEvent, KeyboardKey, Modifiers, NamedKey, Size};
use incular_widgets::internal::{Widget, WidgetTree};
use incular_widgets::{
    Action, ActionListener, ActionResult, Actions, CallbackShortcuts, Command, FocusNode,
    FocusableActionDetector, Intent, LogicalShortcutKey, ShortcutActivator, ShortcutKey,
    ShortcutTrigger, Shortcuts, SizedBox,
};

fn key_down(key: KeyboardKey, code: Code, modifiers: Modifiers) -> KeyboardEvent {
    let mut event = KeyboardEvent::key_down(key, code);
    event.modifiers = modifiers;
    event
}

fn control(code: Code) -> KeyboardEvent {
    key_down(
        KeyboardKey::Named(NamedKey::Unidentified),
        code,
        Modifiers::CONTROL,
    )
}

fn layout(tree: &mut WidgetTree, root: Widget) -> incular_widgets::internal::ElementId {
    let id = tree.mount(root).expect("mount");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    id
}

fn control_k() -> ShortcutActivator {
    ShortcutActivator::physical_on(
        ShortcutKey::new(Code::KeyK, Modifiers::CONTROL),
        ShortcutTrigger::Press,
    )
}

fn control_n() -> ShortcutActivator {
    ShortcutActivator::physical_on(
        ShortcutKey::new(Code::KeyN, Modifiers::CONTROL),
        ShortcutTrigger::Press,
    )
}

#[test]
fn callback_shortcuts_rebuild_replaces_bindings_and_unmount_releases() {
    let first = Rc::new(Cell::new(0));
    let second = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    let root = layout(
        &mut tree,
        CallbackShortcuts::new(SizedBox::shrink())
            .shortcut(control_k(), {
                let first = first.clone();
                move || first.set(first.get() + 1)
            })
            .into(),
    );
    assert!(tree.dispatch_keyboard(Some(root), control(Code::KeyK)));
    assert_eq!((first.get(), second.get()), (1, 0));

    // Rebuilding with the same activator swaps the handler; the old
    // closure never fires again.
    tree.update(
        root,
        CallbackShortcuts::new(SizedBox::shrink())
            .shortcut(control_k(), {
                let second = second.clone();
                move || second.set(second.get() + 1)
            })
            .into(),
    )
    .expect("replace bindings");
    assert!(tree.dispatch_keyboard(Some(root), control(Code::KeyK)));
    assert_eq!((first.get(), second.get()), (1, 1));

    // Unmounting drops the scope with the element: the stale id routes
    // nowhere and no handler can fire through it.
    tree.mount(SizedBox::shrink().into()).expect("replace root");
    assert!(!tree.element_exists(root));
    assert!(!tree.dispatch_keyboard(Some(root), control(Code::KeyK)));
    assert_eq!((first.get(), second.get()), (1, 1));
}

#[test]
fn callback_shortcuts_nested_scopes_prefer_inner() {
    let inner = Rc::new(Cell::new(0));
    let outer = Rc::new(Cell::new(0));
    let outer_other = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    let root = layout(
        &mut tree,
        CallbackShortcuts::new(
            CallbackShortcuts::new(Widget::box_(
                incular_core::Size::new(40., 40.),
                incular_core::Color::WHITE,
            ))
            .shortcut(control_k(), {
                let inner = inner.clone();
                move || inner.set(inner.get() + 1)
            }),
        )
        .shortcut(control_k(), {
            let outer = outer.clone();
            move || outer.set(outer.get() + 1)
        })
        .shortcut(control_n(), {
            let outer_other = outer_other.clone();
            move || outer_other.set(outer_other.get() + 1)
        })
        .into(),
    );
    let inner_el = tree.children(root).expect("outer child")[0];
    // Same activator on both scopes: the nearest handler wins.
    assert!(tree.dispatch_keyboard(Some(inner_el), control(Code::KeyK)));
    assert_eq!((inner.get(), outer.get()), (1, 0));
    // An activator bound only outside still falls through to it.
    assert!(tree.dispatch_keyboard(Some(inner_el), control(Code::KeyN)));
    assert_eq!(outer_other.get(), 1);
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum TestCommand {
    Submit,
    Outer,
}

fn save_intent() -> Intent {
    Intent::new(Command::new("save"))
}

#[test]
fn action_listener_reentrant_invoke_terminates_without_borrows() {
    let events = Rc::new(RefCell::new(Vec::<&'static str>::new()));
    let nested = Rc::new(Cell::new(false));
    // Reenter once from inside the handler: listener snapshots must be
    // released before delivery, or the nested invoke would hit a live
    // mutable borrow.
    let slot: Rc<RefCell<Option<Action>>> = Rc::new(RefCell::new(None));
    let action = Action::new(Command::new("save"), {
        let events = events.clone();
        let nested = nested.clone();
        let slot = slot.clone();
        move || {
            events.borrow_mut().push("handler");
            if !nested.replace(true) {
                let borrowed = slot.borrow();
                let action = borrowed.as_ref().expect("action installed");
                assert_eq!(action.invoke(&save_intent()), ActionResult::Handled);
            }
        }
    });
    *slot.borrow_mut() = Some(action.clone());
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
    tree.mount(listener.into()).expect("mount listener");
    assert_eq!(action.invoke(&save_intent()), ActionResult::Handled);
    // The nested invocation completes inside the outer handler: inner
    // start/handler/action/end all land before the outer action/end.
    assert_eq!(
        &*events.borrow(),
        &[
            "start", "handler", "start", "handler", "action", "end", "action", "end"
        ]
    );
}

#[test]
fn focusable_action_detector_rebuild_keeps_single_focus_subscription() {
    let node = FocusNode::new();
    let changes = Rc::new(Cell::new(0));
    let build = |changes: Rc<Cell<u32>>| {
        FocusableActionDetector::new(SizedBox::shrink())
            .focus_node(node.clone())
            .on_focus_change(move |_| changes.set(changes.get() + 1))
            .into()
    };
    let mut tree = WidgetTree::new();
    let root = layout(&mut tree, build(changes.clone()));
    // Rebuilding swaps behavior records; the node must keep exactly one
    // live observer, or one focus change would notify repeatedly.
    tree.update(root, build(changes.clone()))
        .expect("rebuild detector");
    tree.update(root, build(changes.clone()))
        .expect("rebuild detector again");
    node.request_focus();
    assert_eq!(changes.get(), 1);
    node.unfocus();
    assert_eq!(changes.get(), 2);
}

#[test]
fn focusable_action_detector_replaced_callbacks_stop() {
    let node = FocusNode::new();
    let first = Rc::new(Cell::new(0));
    let second = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    let root = layout(
        &mut tree,
        FocusableActionDetector::new(SizedBox::shrink())
            .focus_node(node.clone())
            .on_focus_change({
                let first = first.clone();
                move |_| first.set(first.get() + 1)
            })
            .into(),
    );
    tree.update(
        root,
        FocusableActionDetector::new(SizedBox::shrink())
            .focus_node(node.clone())
            .on_focus_change({
                let second = second.clone();
                move |_| second.set(second.get() + 1)
            })
            .into(),
    )
    .expect("replace callbacks");
    node.request_focus();
    assert_eq!((first.get(), second.get()), (0, 1));
    node.unfocus();
    assert_eq!((first.get(), second.get()), (0, 2));
}

#[test]
fn focusable_action_detector_unmount_releases_behavior() {
    let node = FocusNode::new();
    let changes = Rc::new(Cell::new(0));
    let highlights = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    layout(
        &mut tree,
        FocusableActionDetector::new(SizedBox::shrink())
            .focus_node(node.clone())
            .on_focus_change({
                let changes = changes.clone();
                move |_| changes.set(changes.get() + 1)
            })
            .on_show_focus_highlight({
                let highlights = highlights.clone();
                move |_| highlights.set(highlights.get() + 1)
            })
            .into(),
    );
    node.request_focus();
    assert_eq!(changes.get(), 1);
    assert_eq!(
        highlights.get(),
        1,
        "focus highlight callback fires on the focus transition"
    );

    tree.mount(SizedBox::shrink().into()).expect("replace root");
    node.unfocus();
    node.request_focus();
    assert_eq!(
        (changes.get(), highlights.get()),
        (1, 1),
        "dropped behavior observes nothing further"
    );
}

#[test]
fn focusable_action_detector_nested_scopes_select_inner_first() {
    let inner_hits = Rc::new(RefCell::new(Vec::new()));
    let outer_hits = Rc::new(RefCell::new(Vec::new()));
    let mut inner_shortcuts = Shortcuts::<TestCommand>::typed();
    inner_shortcuts.bind_logical(
        LogicalShortcutKey::new(KeyboardKey::Character("s".into()), Modifiers::CONTROL),
        TestCommand::Submit,
    );
    let mut outer_shortcuts = Shortcuts::<TestCommand>::typed();
    outer_shortcuts.bind_logical(
        LogicalShortcutKey::new(KeyboardKey::Character("s".into()), Modifiers::CONTROL),
        TestCommand::Outer,
    );
    outer_shortcuts.bind_logical(
        LogicalShortcutKey::new(KeyboardKey::Character("n".into()), Modifiers::CONTROL),
        TestCommand::Outer,
    );
    let mut inner_actions = Actions::<TestCommand>::typed();
    inner_actions.register_handler(TestCommand::Submit, {
        let inner_hits = inner_hits.clone();
        move |intent| {
            inner_hits.borrow_mut().push(*intent.command());
            ActionResult::Handled
        }
    });
    let mut outer_actions = Actions::<TestCommand>::typed();
    outer_actions.register_handler(TestCommand::Outer, {
        let outer_hits = outer_hits.clone();
        move |intent| {
            outer_hits.borrow_mut().push(*intent.command());
            ActionResult::Handled
        }
    });

    let mut tree = WidgetTree::new();
    let root = layout(
        &mut tree,
        FocusableActionDetector::new(
            FocusableActionDetector::new(SizedBox::shrink())
                .shortcuts(Rc::new(inner_shortcuts))
                .actions(Rc::new(inner_actions)),
        )
        .shortcuts(Rc::new(outer_shortcuts))
        .actions(Rc::new(outer_actions))
        .into(),
    );
    let inner_el = tree.children(root).expect("outer child")[0];
    let submit = || {
        let mut event = KeyboardEvent::key_down(KeyboardKey::Character("s".into()), Code::KeyS);
        event.modifiers = Modifiers::CONTROL;
        event
    };
    let other = || {
        let mut event = KeyboardEvent::key_down(KeyboardKey::Character("n".into()), Code::KeyN);
        event.modifiers = Modifiers::CONTROL;
        event
    };
    assert!(tree.dispatch_keyboard(Some(inner_el), submit()));
    assert_eq!(&*inner_hits.borrow(), &[TestCommand::Submit]);
    assert!(outer_hits.borrow().is_empty());
    assert!(tree.dispatch_keyboard(Some(inner_el), other()));
    assert_eq!(&*outer_hits.borrow(), &[TestCommand::Outer]);
}

#[test]
fn focusable_action_detector_autofocus_advertises() {
    let node = FocusNode::new();
    let mut tree = WidgetTree::new();
    let root = layout(
        &mut tree,
        FocusableActionDetector::new(SizedBox::shrink())
            .focus_node(node)
            .autofocus(true)
            .into(),
    );
    assert_eq!(tree.autofocus_element(), Some(root));
}

#[test]
fn focusable_action_detector_disabled_contract() {
    let node = FocusNode::new();
    let changes = Rc::new(Cell::new(0));
    let highlights = Rc::new(Cell::new(0));
    let fired = Rc::new(Cell::new(0));
    let mut shortcuts = Shortcuts::<TestCommand>::typed();
    shortcuts.bind_logical(
        LogicalShortcutKey::new(KeyboardKey::Character("s".into()), Modifiers::CONTROL),
        TestCommand::Submit,
    );
    let mut actions = Actions::<TestCommand>::typed();
    actions.register_handler(TestCommand::Submit, {
        let fired = fired.clone();
        move |_| {
            fired.set(fired.get() + 1);
            ActionResult::Handled
        }
    });
    let mut tree = WidgetTree::new();
    let root = layout(
        &mut tree,
        FocusableActionDetector::new(SizedBox::shrink())
            .focus_node(node.clone())
            .enabled(false)
            .on_focus_change({
                let changes = changes.clone();
                move |_| changes.set(changes.get() + 1)
            })
            .on_show_focus_highlight({
                let highlights = highlights.clone();
                move |_| highlights.set(highlights.get() + 1)
            })
            .shortcuts(Rc::new(shortcuts))
            .actions(Rc::new(actions))
            .into(),
    );
    // Enablement gates the node and the highlights: focus is refused
    // and no highlight callback runs.
    assert!(!node.is_enabled());
    node.request_focus();
    assert!(!node.has_focus());
    assert_eq!((changes.get(), highlights.get()), (0, 0));
    // Shortcut and action scopes are independently owned registries;
    // the detector composes but does not gate them.
    let mut event = KeyboardEvent::key_down(KeyboardKey::Character("s".into()), Code::KeyS);
    event.modifiers = Modifiers::CONTROL;
    assert!(tree.dispatch_keyboard(Some(root), event));
    assert_eq!(fired.get(), 1);
}
