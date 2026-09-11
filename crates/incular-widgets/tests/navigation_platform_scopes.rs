//! Navigation-scope and platform-wrapper lifecycle coverage.
//!
//! W3 property/lifecycle contracts only: back dispatch, pop scopes,
//! page storage, restoration scopes, the modal barrier, window chrome,
//! and the neutral platform-menu wrappers. Router/transaction
//! redesign stays with W4 and is out of scope here. Neutral wrapper
//! behavior is separated from native execution: unbound or
//! unsupported operations report explicit `NoOpUnsupported`/error
//! results, never silent success.

use incular_config::Constraints;
use incular_core::{Color, Offset, Size, WindowResizeDirection};
use incular_widgets::NavigationBackButtonDispatcher as BackButtonDispatcher;
use incular_widgets::{
    AnimatedModalBarrier, BackButtonListener, BackHandlerResult, LayoutBuilder, MenuDispatchResult,
    MenuItemId, NavigatorPopHandler, NavigatorPopHandlerController, NoopPlatformMenuDelegate,
    PageStorage, PageStorageBucket, PageStorageKey, PlatformMenu, PlatformMenuBar,
    PlatformMenuBarController, PlatformMenuDelegate, PlatformMenuEvent, PlatformMenuItem,
    PlatformMenuItemGroup, PlatformMenuShortcut, PlatformMenuUpdate, PopAttempt, PopScope,
    PopScopeController, RootRestorationScope, ShortcutModifiers, Stack, Text, Widget,
    WindowDragRegion, WindowResizeRegion, current_page_storage_bucket, current_restoration_scope,
    internal::WidgetTree,
};
use serde_json::{Value, json};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

#[derive(Default)]
struct MemoryRestorationBackend(RefCell<BTreeMap<Vec<incular_core::RestorationKey>, Value>>);

impl incular_core::RestorationBackend for MemoryRestorationBackend {
    fn read_value(&self, path: &[incular_core::RestorationKey]) -> Option<Value> {
        self.0.borrow().get(path).cloned()
    }

    fn write_value(&self, path: &[incular_core::RestorationKey], value: Value) {
        self.0.borrow_mut().insert(path.to_vec(), value);
    }

    fn remove_value(&self, path: &[incular_core::RestorationKey]) {
        self.0.borrow_mut().remove(path);
    }
}

fn restoration_key(value: &str) -> incular_core::RestorationKey {
    incular_core::RestorationKey::new(value).unwrap()
}

fn mount(
    tree: &mut WidgetTree,
    widget: Widget,
    w: f32,
    h: f32,
) -> incular_widgets::internal::ElementId {
    let root = tree.mount(widget).expect("mount");
    tree.layout(Constraints::tight(Size::new(w, h)))
        .expect("layout");
    root
}

fn pop_scope_widget(controller: PopScopeController, dispatcher: BackButtonDispatcher) -> Widget {
    PopScope::with_controller(controller, Widget::box_(Size::new(10., 10.), Color::WHITE))
        .dispatcher(dispatcher)
        .into()
}

#[test]
fn pop_scope_controller_defaults_to_may_pop() {
    // A fresh scope may pop: blocking is opt-in, matching the constructor
    // contract, the sibling nested handler, and the platform convention.
    let controller = PopScopeController::new();
    assert!(controller.can_pop());
    let dispatcher = BackButtonDispatcher::new();
    let _registration = controller.register(&dispatcher);
    let report = dispatcher.dispatch_back();
    assert!(!report.blocked);
    controller.set_can_pop(false);
    let report = dispatcher.dispatch_back();
    assert!(report.handled);
    assert!(report.blocked);
}

#[test]
fn pop_scope_widget_replacement_retargets_dispatch() {
    let first = PopScopeController::new();
    let second = PopScopeController::new();
    let first_fires = Rc::new(RefCell::new(0));
    let second_fires = Rc::new(RefCell::new(0));
    let observed_first = first_fires.clone();
    first.set_on_pop_invoked(move |_| *observed_first.borrow_mut() += 1);
    let observed_second = second_fires.clone();
    second.set_on_pop_invoked(move |_| *observed_second.borrow_mut() += 1);
    let dispatcher = BackButtonDispatcher::new();
    let mut tree = WidgetTree::new();
    let root = mount(
        &mut tree,
        pop_scope_widget(first.clone(), dispatcher.clone()),
        100.,
        100.,
    );
    assert_eq!(dispatcher.pop_scope_count(), 1);
    // Fresh scopes may pop: the callback fires with did_pop=false and the
    // platform proceeds (handled=false). Blocking flips both bits.
    let report = dispatcher.dispatch_back();
    assert!(!report.handled);
    assert!(!report.blocked);
    assert_eq!(*first_fires.borrow(), 1);

    // Swapping the descriptor swaps the attached controller; the old one
    // no longer receives dispatches.
    tree.update(root, pop_scope_widget(second.clone(), dispatcher.clone()))
        .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert_eq!(dispatcher.pop_scope_count(), 1);
    let report = dispatcher.dispatch_back();
    assert!(!report.handled);
    assert_eq!(*first_fires.borrow(), 1);
    assert_eq!(*second_fires.borrow(), 1);

    // Descriptor callback swaps replace the controller callback in place.
    let swapped = Rc::new(RefCell::new(0));
    let observed_swapped = swapped.clone();
    tree.update(
        root,
        PopScope::with_controller(
            second.clone(),
            Widget::box_(Size::new(10., 10.), Color::WHITE),
        )
        .dispatcher(dispatcher.clone())
        .on_pop_invoked(move |_| *observed_swapped.borrow_mut() += 1)
        .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert!(!dispatcher.dispatch_back().handled);
    assert_eq!(*second_fires.borrow(), 1);
    assert_eq!(*swapped.borrow(), 1);
}

#[test]
fn pop_scope_unmount_unregisters() {
    let controller = PopScopeController::new();
    let fires = Rc::new(RefCell::new(0));
    let observed = fires.clone();
    controller.set_on_pop_invoked(move |_| *observed.borrow_mut() += 1);
    let dispatcher = BackButtonDispatcher::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            incular_widgets::Container::with_child(pop_scope_widget(
                controller.clone(),
                dispatcher.clone(),
            ))
            .into(),
        )
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert_eq!(dispatcher.pop_scope_count(), 1);
    tree.update(
        root,
        incular_widgets::Container::with_child(Widget::box_(Size::new(10., 10.), Color::WHITE))
            .into(),
    )
    .expect("unmount");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert_eq!(dispatcher.pop_scope_count(), 0);
    let report = dispatcher.dispatch_back();
    assert!(!report.handled);
    assert!(!report.blocked);
    assert_eq!(*fires.borrow(), 0);
}

#[test]
fn dispatcher_fallback_replacement_and_hierarchy() {
    let dispatcher = BackButtonDispatcher::new();
    dispatcher.set_fallback(|| PopAttempt::popped(None));
    let report = dispatcher.dispatch_back();
    assert!(report.handled);
    assert_eq!(report.pop.as_ref().map(|pop| pop.did_pop), Some(true));
    dispatcher.set_fallback(PopAttempt::rejected);
    let report = dispatcher.dispatch_back();
    assert!(report.handled);
    assert_eq!(report.pop.as_ref().map(|pop| pop.did_pop), Some(false));
    dispatcher.clear_fallback();
    assert!(!dispatcher.dispatch_back().handled);

    // Constructor, alias, and one-shot override agree.
    let scoped = BackButtonDispatcher::with_fallback(|| PopAttempt::popped(None));
    assert!(scoped.dispatch_back().handled);
    assert!(scoped.invoke_callback().handled);
    assert!(
        scoped
            .dispatch_back_with_fallback(PopAttempt::rejected)
            .pop
            .as_ref()
            .is_some_and(|pop| !pop.did_pop)
    );

    // Newest child dispatches first; detaching removes the branch.
    let child = dispatcher.create_child();
    assert_eq!(dispatcher.child_count(), 1);
    let order = Rc::new(RefCell::new(Vec::new()));
    let parent_order = order.clone();
    let _parent = dispatcher.add_callback(move || {
        parent_order.borrow_mut().push("parent");
        false
    });
    let child_order = order.clone();
    let _child = child.add_callback(move || {
        child_order.borrow_mut().push("child");
        false
    });
    let _ = dispatcher.dispatch_back();
    assert_eq!(&*order.borrow(), &["child", "parent"]);
    dispatcher.detach_child(&child);
    assert_eq!(dispatcher.child_count(), 0);

    // Handler tokens own their registrations: dropping one stops it.
    let handler = dispatcher.add_handler(BackHandlerResult::handled);
    assert!(dispatcher.dispatch_back().handled);
    drop(handler);
    assert!(!dispatcher.dispatch_back().handled);
}

#[test]
fn pop_attempt_and_handler_result_constructors() {
    let popped = PopAttempt::popped(Some(json!({"at": "end"})));
    assert!(popped.did_pop && popped.handled && !popped.blocked);
    assert_eq!(popped.result, Some(json!({"at": "end"})));
    let rejected = PopAttempt::rejected();
    assert!(!rejected.did_pop && rejected.handled && !rejected.blocked);
    let blocked = PopAttempt::blocked();
    assert!(!blocked.did_pop && blocked.handled && blocked.blocked);
    let unhandled = PopAttempt::unhandled();
    assert!(!unhandled.did_pop && !unhandled.handled && !unhandled.blocked);
    assert!(BackHandlerResult::handled().handled);
    assert!(!BackHandlerResult::unhandled().handled);
    let converted = BackHandlerResult::from_pop(popped);
    assert!(converted.handled && converted.pop.is_some());
}

#[test]
fn navigator_pop_handler_widget_replacement_and_unmount() {
    // Ownership requires a pop handler; the legacy callback alone never
    // fires. Each controller below owns its pop through set_pop_handler.
    let first = NavigatorPopHandlerController::new();
    let second = NavigatorPopHandlerController::new();
    assert!(first.enabled() && first.can_pop());
    let first_fires = Rc::new(RefCell::new(0));
    let second_fires = Rc::new(RefCell::new(0));
    let observed_first = first_fires.clone();
    first.set_pop_handler(|| PopAttempt::popped(None));
    first.set_on_pop(move || *observed_first.borrow_mut() += 1);
    let observed_second = second_fires.clone();
    second.set_pop_handler(|| PopAttempt::popped(None));
    second.set_on_pop(move || *observed_second.borrow_mut() += 1);
    let dispatcher = BackButtonDispatcher::new();
    let mut tree = WidgetTree::new();
    let handler_widget = |controller: NavigatorPopHandlerController| {
        NavigatorPopHandler::with_controller(
            controller,
            Widget::box_(Size::new(10., 10.), Color::WHITE),
        )
        .dispatcher(dispatcher.clone())
        .into()
    };
    let root = mount(&mut tree, handler_widget(first.clone()), 100., 100.);
    assert!(dispatcher.dispatch_back().handled);
    assert_eq!(*first_fires.borrow(), 1);

    // Swapping the descriptor swaps the owning controller.
    tree.update(root, handler_widget(second.clone()))
        .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert!(dispatcher.dispatch_back().handled);
    assert_eq!(*first_fires.borrow(), 1);
    assert_eq!(*second_fires.borrow(), 1);

    // Disabling through the descriptor stops dispatch without unmounting.
    tree.update(
        root,
        NavigatorPopHandler::with_controller(
            second.clone(),
            Widget::box_(Size::new(10., 10.), Color::WHITE),
        )
        .dispatcher(dispatcher.clone())
        .enabled(false)
        .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert!(!dispatcher.dispatch_back().handled);
    assert!(!second.enabled());
    assert_eq!(*second_fires.borrow(), 1);
}

#[test]
fn back_button_listener_dispatcher_swap() {
    let first = BackButtonDispatcher::new();
    let second = BackButtonDispatcher::new();
    let fires = Rc::new(RefCell::new(0));
    let mut tree = WidgetTree::new();
    let root = mount(
        &mut tree,
        BackButtonListener::new(Widget::box_(Size::new(10., 10.), Color::WHITE))
            .dispatcher(first.clone())
            .on_back_button_pressed({
                let observed = fires.clone();
                move || {
                    *observed.borrow_mut() += 1;
                    true
                }
            })
            .into(),
        100.,
        100.,
    );
    assert_eq!(first.handler_count(), 1);
    assert!(first.dispatch_back().handled);

    tree.update(
        root,
        BackButtonListener::new(Widget::box_(Size::new(10., 10.), Color::WHITE))
            .dispatcher(second.clone())
            .on_back_button_pressed({
                let observed = fires.clone();
                move || {
                    *observed.borrow_mut() += 1;
                    true
                }
            })
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert_eq!(first.handler_count(), 0);
    assert_eq!(second.handler_count(), 1);
    assert!(!first.dispatch_back().handled);
    assert!(second.dispatch_back().handled);
    assert_eq!(*fires.borrow(), 2);
}

#[test]
fn page_storage_bucket_replacement_updates_descendants() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let first = PageStorageBucket::default();
    let second = PageStorageBucket::default();
    let tab = PageStorageKey::new("tab");
    first.write_state(vec![tab.clone()], json!({"offset": 1}));
    second.write_state(vec![tab.clone()], json!({"offset": 2}));
    let child = |observed: Rc<RefCell<Vec<Option<serde_json::Value>>>>| {
        Widget::from(LayoutBuilder::new(move |context, _| {
            observed.borrow_mut().push(
                PageStorage::maybe_of(context)
                    .and_then(|bucket| bucket.read_state(vec![PageStorageKey::new("tab")])),
            );
            assert!(current_page_storage_bucket(context).is_some());
            Widget::box_(Size::new(10., 10.), Color::WHITE)
        }))
    };
    let mut tree = WidgetTree::new();
    let root = mount(
        &mut tree,
        PageStorage::with_bucket(first.clone(), child(seen.clone())).into(),
        100.,
        100.,
    );
    assert_eq!(*seen.borrow(), vec![Some(json!({"offset": 1}))]);
    tree.update(
        root,
        PageStorage::with_bucket(second.clone(), child(seen.clone())).into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert_eq!(
        *seen.borrow(),
        vec![Some(json!({"offset": 1})), Some(json!({"offset": 2}))]
    );
}

#[test]
fn restoration_scope_replacement_updates_descendants() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let child = |observed: Rc<RefCell<Vec<Option<String>>>>| {
        Widget::from(LayoutBuilder::new(move |context, _| {
            observed
                .borrow_mut()
                .push(current_restoration_scope(context).map(|scope| {
                    scope
                        .path()
                        .iter()
                        .map(|key| key.as_str().to_owned())
                        .collect::<Vec<_>>()
                        .join("/")
                }));
            Widget::box_(Size::new(10., 10.), Color::WHITE)
        }))
    };
    let backend = Rc::new(MemoryRestorationBackend::default());
    let first = incular_core::RestorationScope::root(backend.clone())
        .child_unchecked(restoration_key("first"));
    let second =
        incular_core::RestorationScope::root(backend).child_unchecked(restoration_key("second"));
    let mut tree = WidgetTree::new();
    let root = mount(
        &mut tree,
        RootRestorationScope::with_scope(first, "application", child(seen.clone())).into(),
        100.,
        100.,
    );
    assert_eq!(*seen.borrow(), vec![Some("first/application".to_owned())]);
    tree.update(
        root,
        RootRestorationScope::with_scope(second, "application", child(seen.clone())).into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    let reads = seen.borrow();
    assert_eq!(reads.len(), 2);
    assert_eq!(reads[1], Some("second/application".to_owned()));
}

#[test]
fn animated_modal_barrier_options() {
    // A non-dismissible barrier reports false without firing.
    let fires = Rc::new(RefCell::new(0));
    let observed = fires.clone();
    let blocked = AnimatedModalBarrier::new()
        .dismissible(false)
        .on_dismiss(move || *observed.borrow_mut() += 1)
        .child(Widget::box_(Size::new(10., 10.), Color::WHITE));
    assert!(!blocked.dismiss());
    assert_eq!(*fires.borrow(), 0);

    // The boolean bridge reports its own outcome.
    let bridged = AnimatedModalBarrier::new().dismissal_handler(|| false);
    assert!(!bridged.dismiss());

    // All flag combinations over Stack([sibling, barrier]): the modal
    // always blocks the preceding sibling, the label survives whenever
    // present, and Activate appears exactly when pointer dismissal and
    // the semantic-dismiss flag agree — independent of the label.
    for (dismissible, semantics_dismissible, labeled) in [
        (true, true, true),
        (true, true, false),
        (true, false, true),
        (true, false, false),
        (false, true, true),
        (false, true, false),
        (false, false, true),
        (false, false, false),
    ] {
        let mut config = AnimatedModalBarrier::new().dismissible(dismissible);
        config = config.barrier_semantics_dismissible(semantics_dismissible);
        if labeled {
            config = config
                .semantics_label("Dismiss")
                .semantics_on_tap_hint("Double tap");
        }
        let sibling: Widget = Text::new("sibling").into();
        let stacked: Widget = Stack::new([sibling, config.into()]).into();
        let mut tree = WidgetTree::new();
        mount(&mut tree, stacked, 100., 40.);
        tree.update_semantics();
        let dump = tree.semantics_debug_dump();
        assert!(
            !dump.contains("sibling"),
            "modal blocks preceding siblings ({dismissible},{semantics_dismissible},{labeled}):\n{dump}"
        );
        assert_eq!(
            dump.contains("Dismiss"),
            labeled,
            "label survives dismissal flags ({dismissible},{semantics_dismissible},{labeled}):\n{dump}"
        );
        if labeled {
            // The debug dump omits descriptions, so read the hint off the
            // live node carrying the label.
            let hinted = tree.semantics().iter().any(|(_, node)| {
                node.label.as_deref() == Some("Dismiss")
                    && node.description.as_deref() == Some("Double tap")
            });
            assert!(hinted, "hint rides the label node:\n{dump}");
        }
        assert_eq!(
            dump.contains("Activate"),
            dismissible && semantics_dismissible,
            "semantic dismissal is gated by both flags:\n{dump}"
        );
        if dismissible && semantics_dismissible {
            assert!(
                dump.contains("Button"),
                "dismissal node owns a Button role:\n{dump}"
            );
        } else if labeled {
            assert!(
                dump.contains("GenericContainer"),
                "label-only node stays neutral:\n{dump}"
            );
        } else {
            assert!(dump.is_empty(), "plain veil contributes no nodes:\n{dump}");
        }
    }

    // The barrier's own background child stays hidden behind the veil in
    // every mode; later siblings above the modal stay visible.
    for dismissible in [true, false] {
        let mut tree = WidgetTree::new();
        mount(
            &mut tree,
            AnimatedModalBarrier::new()
                .dismissible(dismissible)
                .semantics_label("Dismiss")
                .child(Text::new("behind"))
                .into(),
            100.,
            40.,
        );
        tree.update_semantics();
        let dump = tree.semantics_debug_dump();
        assert!(dump.contains("Dismiss"), ":\n{dump}");
        assert!(
            !dump.contains("behind"),
            "background child is blocked, not merged (dismissible={dismissible}):\n{dump}"
        );
    }
    let sibling: Widget = Text::new("sibling").into();
    let barrier: Widget = AnimatedModalBarrier::new()
        .semantics_label("Dismiss")
        .into();
    let front: Widget = Text::new("front").into();
    let mut tree = WidgetTree::new();
    mount(
        &mut tree,
        Stack::new([sibling, barrier, front]).into(),
        100.,
        40.,
    );
    tree.update_semantics();
    let dump = tree.semantics_debug_dump();
    assert!(!dump.contains("sibling"), ":\n{dump}");
    assert!(dump.contains("Dismiss"), ":\n{dump}");
    assert!(
        dump.contains("front"),
        "later siblings stay visible:\n{dump}"
    );
}

#[test]
fn animated_modal_barrier_semantic_ownership() {
    use incular_core::PointerPhase;
    use incular_gestures::PointerEvent;
    use incular_semantics::SemanticActionKind;
    use incular_widgets::internal::ElementId;
    use std::time::Instant;

    fn activate_owners(tree: &WidgetTree, root: ElementId) -> Vec<ElementId> {
        let mut owners = Vec::new();
        let mut work = vec![root];
        while let Some(element) = work.pop() {
            if tree
                .semantic_action_callback(element, SemanticActionKind::Activate)
                .is_some()
            {
                owners.push(element);
            }
            if let Some(children) = tree.children(element) {
                work.extend(children.iter().copied());
            }
        }
        owners
    }

    fn tap(tree: &mut WidgetTree, x: f32, y: f32) {
        for phase in [PointerPhase::Down, PointerPhase::Up] {
            let _ = tree.dispatch_device_gesture_in_window(
                9,
                11,
                PointerEvent {
                    pointer: 5,
                    position: incular_core::Offset::new(x, y),
                    phase,
                    time: Instant::now(),
                },
            );
        }
    }

    // Exactly one node owns Activate when semantic dismissal is enabled;
    // invoking it fires the configured callback, without a wrapper chain.
    let fires = Rc::new(RefCell::new(0));
    let observed = fires.clone();
    let mut tree = WidgetTree::new();
    let root = mount(
        &mut tree,
        AnimatedModalBarrier::new()
            .semantics_label("Dismiss")
            .on_dismiss(move || *observed.borrow_mut() += 1)
            .into(),
        100.,
        40.,
    );
    tree.update_semantics();
    let owners = activate_owners(&tree, root);
    assert_eq!(owners.len(), 1, "one veil node owns Activate");
    tree.semantic_action_callback(owners[0], SemanticActionKind::Activate)
        .expect("owner carries the callback")();
    assert_eq!(*fires.borrow(), 1);

    // Pointer taps dismiss while enabled and never fall through: the
    // locked veil still absorbs the gesture without firing.
    tap(&mut tree, 50., 20.);
    assert_eq!(*fires.borrow(), 2);
    let locked_fires = Rc::new(RefCell::new(0));
    let locked_observed = locked_fires.clone();
    tree.update(
        root,
        AnimatedModalBarrier::new()
            .dismissible(false)
            .semantics_label("Dismiss")
            .on_dismiss(move || *locked_observed.borrow_mut() += 1)
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 40.)))
        .expect("layout");
    tree.update_semantics();
    assert!(
        activate_owners(&tree, root).is_empty(),
        "mounted toggle withdraws the action"
    );
    assert!(
        tree.hit_test(incular_core::Offset::new(50., 20.)).is_some(),
        "locked veil still intercepts pointers"
    );
    tap(&mut tree, 50., 20.);
    assert_eq!(*locked_fires.borrow(), 0);

    // Callback replacement retargets both pointer and semantic dismissal
    // to the newest closure; absence of any callback still dismisses
    // silently through the same paths.
    let first = Rc::new(RefCell::new(0));
    let second = Rc::new(RefCell::new(0));
    let mut tree = WidgetTree::new();
    let root = mount(
        &mut tree,
        AnimatedModalBarrier::new()
            .on_dismiss({
                let first = first.clone();
                move || *first.borrow_mut() += 1
            })
            .into(),
        100.,
        40.,
    );
    tree.update(
        root,
        AnimatedModalBarrier::new()
            .on_dismiss({
                let second = second.clone();
                move || *second.borrow_mut() += 1
            })
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 40.)))
        .expect("layout");
    tap(&mut tree, 50., 20.);
    assert_eq!(*first.borrow(), 0);
    assert_eq!(*second.borrow(), 1);
    let owners = activate_owners(&tree, root);
    assert_eq!(owners.len(), 1);
    tree.semantic_action_callback(owners[0], SemanticActionKind::Activate)
        .expect("replaced callback")();
    assert_eq!(*second.borrow(), 2);
}

#[test]
fn window_chrome_child_replacement_and_unmount() {
    use incular_widgets::internal::WindowInteraction;
    let mut tree = WidgetTree::new();
    let root = mount(
        &mut tree,
        WindowDragRegion::new(Widget::box_(Size::new(120., 40.), Color::WHITE)).into(),
        120.,
        40.,
    );
    let (_, interaction) = tree
        .window_interaction_at(Offset::new(20., 10.))
        .expect("drag annotation");
    assert_eq!(interaction, WindowInteraction::Move);

    // Replacing the child keeps the drag annotation over the new content.
    tree.update(root, WindowDragRegion::new(Text::new("title")).into())
        .expect("update");
    tree.layout(Constraints::tight(Size::new(120., 40.)))
        .expect("layout");
    let (_, interaction) = tree
        .window_interaction_at(Offset::new(20., 10.))
        .expect("drag annotation survives child swap");
    assert_eq!(interaction, WindowInteraction::Move);

    // Unmounting through a same-type parent clears the annotation.
    let mut tree = WidgetTree::new();
    let chrome: Widget = WindowDragRegion::new(WindowResizeRegion::new(
        WindowResizeDirection::SouthEast,
        Widget::box_(Size::new(120., 40.), Color::WHITE),
    ))
    .into();
    let root = tree
        .mount(incular_widgets::Column::new([chrome]).into())
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(120., 40.)))
        .expect("layout");
    assert!(tree.window_interaction_at(Offset::new(60., 20.)).is_some());
    let plain: Widget = Widget::box_(Size::new(120., 40.), Color::WHITE);
    tree.update(root, incular_widgets::Column::new([plain]).into())
        .expect("unmount");
    tree.layout(Constraints::tight(Size::new(120., 40.)))
        .expect("layout");
    assert!(tree.window_interaction_at(Offset::new(60., 20.)).is_none());
}

#[derive(Default)]
struct RecordingDelegate {
    calls: RefCell<Vec<String>>,
}

impl PlatformMenuDelegate for RecordingDelegate {
    fn acquire(&self, owner: incular_widgets::MenuOwnerId) -> PlatformMenuUpdate {
        self.calls.borrow_mut().push(format!("acquire:{owner:?}"));
        PlatformMenuUpdate::Applied
    }
    fn set_menus(
        &self,
        owner: incular_widgets::MenuOwnerId,
        snapshot: incular_widgets::PlatformMenuSnapshot,
    ) -> PlatformMenuUpdate {
        self.calls
            .borrow_mut()
            .push(format!("set_menus:{owner:?}:{}", snapshot.menus.len()));
        PlatformMenuUpdate::Applied
    }
    fn clear_menus(&self, owner: incular_widgets::MenuOwnerId) -> PlatformMenuUpdate {
        self.calls.borrow_mut().push(format!("clear:{owner:?}"));
        PlatformMenuUpdate::Applied
    }
    fn release(&self, owner: incular_widgets::MenuOwnerId) -> PlatformMenuUpdate {
        self.calls.borrow_mut().push(format!("release:{owner:?}"));
        PlatformMenuUpdate::Applied
    }
    fn set_event_handler(
        &self,
        owner: incular_widgets::MenuOwnerId,
        handler: Option<Rc<dyn Fn(PlatformMenuEvent)>>,
    ) -> PlatformMenuUpdate {
        self.calls
            .borrow_mut()
            .push(format!("handler:{owner:?}:{}", handler.is_some()));
        PlatformMenuUpdate::Applied
    }
}

fn file_menu(selected: Rc<RefCell<Vec<String>>>) -> PlatformMenu {
    let observed = selected.clone();
    PlatformMenu::new(
        "File",
        vec![
            PlatformMenuItem::new("Open")
                .id("open")
                .tooltip("Open a file")
                .shortcut(
                    PlatformMenuShortcut::new("o")
                        .modifiers(ShortcutModifiers::CONTROL.union(ShortcutModifiers::SHIFT)),
                )
                .on_selected(move || observed.borrow_mut().push("open".to_owned()))
                .enabled(true)
                .into(),
        ],
    )
    .id("file")
    .tooltip("File menu")
    .on_open(|| {})
    .on_close(|| {})
}

#[test]
fn platform_menu_descriptors() {
    let item = PlatformMenuItem::new("Open")
        .id("open")
        .tooltip("tip")
        .shortcut(PlatformMenuShortcut::new("o").modifiers(ShortcutModifiers::CONTROL))
        .on_selected(|| {})
        .enabled(true);
    assert_eq!(item.item_id(), Some(&MenuItemId::new("open")));
    assert_eq!(item.label(), "Open");
    assert!(PlatformMenuItem::new("Cut").disabled().label() == "Cut");
    let group = PlatformMenuItemGroup::new(vec![item]);
    assert_eq!(group.items().len(), 1);
    let menu = PlatformMenu::with_items("File", vec![PlatformMenuItem::new("Open")]);
    assert_eq!(menu.entries().len(), 1);
    assert!(menu.menu_id().is_none());
    let identified = PlatformMenu::new("File", vec![]).id("file");
    assert_eq!(identified.menu_id(), Some(&MenuItemId::new("file")));
    assert_eq!(MenuItemId::new("open").as_str(), "open");
}

#[test]
fn platform_menu_unbound_reports_unsupported() {
    // No native host is attached: every operation reports an explicit
    // unsupported result instead of silent success.
    let controller = PlatformMenuBarController::new(Rc::new(NoopPlatformMenuDelegate));
    assert_eq!(
        controller.install(&[PlatformMenu::new("File", vec![])]),
        Ok(PlatformMenuUpdate::NoOpUnsupported)
    );
    assert_eq!(
        controller.last_update(),
        Some(PlatformMenuUpdate::NoOpUnsupported)
    );
    assert_eq!(
        controller.dispatch(&MenuItemId::new("open")),
        MenuDispatchResult::Unknown
    );
    assert_eq!(controller.detach(), PlatformMenuUpdate::NoOpUnsupported);
}

#[test]
fn platform_menu_controller_lifecycle_with_delegate() {
    let selected = Rc::new(RefCell::new(Vec::new()));
    let delegate: Rc<RecordingDelegate> = Rc::default();
    let controller = PlatformMenuBarController::new(delegate.clone());
    let owner = controller.owner();
    assert_eq!(
        controller.install(&[file_menu(selected.clone())]),
        Ok(PlatformMenuUpdate::Applied)
    );
    let calls = delegate.calls.borrow().clone();
    assert_eq!(calls.len(), 3);
    assert!(calls[0].starts_with("acquire:"));
    assert!(calls[1].ends_with(":true"));
    assert!(calls[2].starts_with("set_menus:"));
    assert!(calls[2].ends_with(":1"));
    assert_eq!(
        controller.snapshot().map(|snapshot| snapshot.menus.len()),
        Some(1)
    );
    // Installed callbacks dispatch through the retained controller.
    assert_eq!(
        controller.dispatch(&MenuItemId::new("open")),
        MenuDispatchResult::Handled
    );
    assert_eq!(*selected.borrow(), vec!["open".to_owned()]);
    assert_eq!(
        controller.dispatch(&MenuItemId::new("missing")),
        MenuDispatchResult::Unknown
    );
    assert_eq!(
        controller.open(&MenuItemId::new("missing")),
        MenuDispatchResult::Unknown
    );
    assert_eq!(
        controller.close(&MenuItemId::new("missing")),
        MenuDispatchResult::Unknown
    );

    // Reinstalling identical menus reports no native change.
    assert_eq!(
        controller.install(&[file_menu(selected.clone())]),
        Ok(PlatformMenuUpdate::Unchanged)
    );
    // Detach clears native state and releases the owner.
    assert_eq!(controller.detach(), PlatformMenuUpdate::Applied);
    let calls = delegate.calls.borrow().clone();
    assert!(calls.iter().any(|call| call.starts_with("clear:")));
    assert!(calls.iter().any(|call| call.starts_with("release:")));
    assert!(controller.snapshot().is_none());
    let _ = owner;
}

#[test]
fn platform_menu_duplicate_ids_are_rejected() {
    let delegate: Rc<RecordingDelegate> = Rc::default();
    let controller = PlatformMenuBarController::new(delegate);
    let duplicate = PlatformMenu::new(
        "File",
        vec![
            PlatformMenuItem::new("A").id("same").into(),
            PlatformMenuItem::new("B").id("same").into(),
        ],
    );
    assert!(matches!(
        controller.install(&[duplicate]),
        Err(incular_widgets::PlatformMenuBuildError::DuplicateId(_))
    ));
}

#[test]
fn platform_menu_bar_construction_paths() {
    // with_delegate arrives bound: installs succeed without a later
    // attach. The default bar stays unbound (explicit unsupported)
    // until a native host connects, which the widget test drives.
    let bound: Rc<RecordingDelegate> = Rc::default();
    let bound_bar = PlatformMenuBar::with_delegate(
        vec![PlatformMenu::new("File", vec![])],
        Widget::box_(Size::new(10., 10.), Color::WHITE),
        bound.clone(),
    );
    assert_eq!(
        bound_bar
            .controller()
            .install(&[PlatformMenu::new("File", vec![])]),
        Ok(PlatformMenuUpdate::Applied)
    );
    let plain_bar = PlatformMenuBar::new(
        vec![PlatformMenu::new("File", vec![])],
        Widget::box_(Size::new(10., 10.), Color::WHITE),
    );
    assert_eq!(
        plain_bar
            .controller()
            .install(&[PlatformMenu::new("File", vec![])]),
        Ok(PlatformMenuUpdate::NoOpUnsupported)
    );
    // A childless bar still lowers.
    let _ = PlatformMenuBar::without_child(vec![PlatformMenu::new("File", vec![])]);
}

#[test]
fn platform_menu_bar_widget_mount_update_unmount() {
    // Mounting publishes the binding but makes no native calls: the
    // desktop adapter attaches the OS delegate later through connect().
    let selected = Rc::new(RefCell::new(Vec::<String>::new()));
    let native: Rc<RecordingDelegate> = Rc::default();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            incular_widgets::Container::with_child(PlatformMenuBar::new(
                vec![file_menu(selected.clone())],
                Widget::box_(Size::new(10., 10.), Color::WHITE),
            ))
            .into(),
        )
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert_eq!(tree.platform_menu_bindings().len(), 1);
    assert!(native.calls.borrow().is_empty());

    // Simulated adapter attach installs through the native delegate.
    // The clone is scoped: holding it keeps the mount lease (and the
    // native attachment) alive by design.
    {
        let binding = tree.platform_menu_bindings()[0].clone();
        assert_eq!(
            binding.connect(native.clone()),
            Ok(PlatformMenuUpdate::Applied)
        );
    }
    assert_eq!(
        native
            .calls
            .borrow()
            .iter()
            .filter(|call| call.starts_with("set_menus:"))
            .count(),
        1
    );

    // Menu replacement reconciles the binding and reinstalls.
    tree.update(
        root,
        incular_widgets::Container::with_child(PlatformMenuBar::new(
            vec![
                file_menu(selected.clone()),
                PlatformMenu::new(
                    "Edit",
                    vec![PlatformMenuItem::new("Copy").id("copy").into()],
                ),
            ],
            Widget::box_(Size::new(10., 10.), Color::WHITE),
        ))
        .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert_eq!(tree.platform_menu_bindings().len(), 1);
    assert_eq!(
        native
            .calls
            .borrow()
            .iter()
            .filter(|call| call.starts_with("set_menus:"))
            .count(),
        2
    );

    // Unmounting detaches: the native side is cleared, not leaked.
    tree.update(
        root,
        incular_widgets::Container::with_child(Widget::box_(Size::new(10., 10.), Color::WHITE))
            .into(),
    )
    .expect("unmount");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert!(tree.platform_menu_bindings().is_empty());
    assert!(
        native
            .calls
            .borrow()
            .iter()
            .any(|call| call.starts_with("clear:"))
    );
}
