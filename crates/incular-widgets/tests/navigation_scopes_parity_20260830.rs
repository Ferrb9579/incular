//! Focused retained navigation/state parity tests.
//!
//! The source modules are included here because the public Widgets re-export
//! is intentionally owned by the facade/integration agent.  Keeping these
//! tests beside the crate still exercises the exact implementation compiled
//! by that integration patch.

use incular_config::Constraints;
use incular_core::RestorationKey;
use incular_widgets::NavigationBackButtonDispatcher as BackButtonDispatcher;
use incular_widgets::internal::WidgetTree;
use incular_widgets::{
    AnimatedModalBarrier, Animation, AnimationController, BackButtonListener, Color,
    NavigatorPopHandlerController, PageStorage, PageStorageBucket, PageStorageKey, PopAttempt,
    PopScopeController, RootRestorationScope, Size, SizedBox, Text, UnmanagedRestorationScope,
    Widget, current_restoration_scope,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
    time::Duration,
};

#[derive(Default)]
struct MemoryRestorationBackend(RefCell<BTreeMap<Vec<RestorationKey>, Value>>);

impl incular_core::RestorationBackend for MemoryRestorationBackend {
    fn read_value(&self, path: &[RestorationKey]) -> Option<Value> {
        self.0.borrow().get(path).cloned()
    }

    fn write_value(&self, path: &[RestorationKey], value: Value) {
        self.0.borrow_mut().insert(path.to_vec(), value);
    }

    fn remove_value(&self, path: &[RestorationKey]) {
        self.0.borrow_mut().remove(path);
    }
}

fn restoration_key(value: &str) -> RestorationKey {
    RestorationKey::new(value).expect("valid restoration key")
}

fn restoration_scope() -> incular_core::RestorationScope {
    incular_core::RestorationScope::root(Rc::new(MemoryRestorationBackend::default()))
        .child_unchecked(restoration_key("window"))
}

#[test]
fn back_dispatch_runs_newest_child_first_and_falls_back_outward() {
    let dispatcher = BackButtonDispatcher::new();
    let first = dispatcher.create_child();
    let second = dispatcher.create_child();
    let order = Rc::new(RefCell::new(Vec::new()));

    let first_order = order.clone();
    let _first_handler = first.add_callback(move || {
        first_order.borrow_mut().push("first");
        true
    });
    let second_order = order.clone();
    let _second_handler = second.add_callback(move || {
        second_order.borrow_mut().push("second");
        false
    });

    let report = dispatcher.dispatch_back();
    assert!(report.handled);
    assert_eq!(&*order.borrow(), &["second", "first"]);

    second.take_priority();
    let second_order = order.clone();
    let _second_handler = second.add_callback(move || {
        second_order.borrow_mut().push("second");
        true
    });
    assert!(dispatcher.dispatch_back().handled);
    assert_eq!(&*order.borrow(), &["second", "first", "second"]);
}

#[test]
fn pop_scope_blocks_or_propagates_the_final_result() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let controller = PopScopeController::new();
    controller.set_on_pop_invoked_with_result({
        let seen = seen.clone();
        move |did_pop, result| seen.borrow_mut().push((did_pop, result))
    });
    controller.set_can_pop(false);

    let dispatcher = BackButtonDispatcher::with_fallback(|| PopAttempt::popped(Some(json!(7))));
    let _registration = controller.register(&dispatcher);
    let blocked = dispatcher.dispatch_back();
    assert!(blocked.handled);
    assert!(blocked.blocked);
    assert_eq!(&*seen.borrow(), &[(false, None)]);

    controller.set_can_pop(true);
    let popped = dispatcher.dispatch_back();
    assert!(popped.handled);
    assert_eq!(
        popped.pop.as_ref().and_then(|pop| pop.result.clone()),
        Some(json!(7))
    );
    assert_eq!(&*seen.borrow(), &[(false, None), (true, Some(json!(7)))]);
}

#[test]
fn nested_pop_handler_propagates_result_and_disabled_handler_defers() {
    let dispatcher = BackButtonDispatcher::new();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let controller = NavigatorPopHandlerController::new();
    controller.set_pop_handler(|| PopAttempt::popped(Some(json!("nested"))));
    controller.set_on_pop_with_result({
        let seen = seen.clone();
        move |result| seen.borrow_mut().push(result)
    });
    let _registration = controller.register(&dispatcher);

    assert!(dispatcher.dispatch_back().handled);
    assert_eq!(&*seen.borrow(), &[Some(json!("nested"))]);

    controller.set_enabled(false);
    assert!(!dispatcher.dispatch_back().handled);
}

#[test]
fn retained_back_listener_unregisters_when_its_element_is_replaced() {
    let dispatcher = BackButtonDispatcher::new();
    let calls = Rc::new(Cell::new(0));
    let listener = BackButtonListener::new(SizedBox::shrink())
        .dispatcher(dispatcher.clone())
        .on_back_button_pressed({
            let calls = calls.clone();
            move || {
                calls.set(calls.get() + 1);
                true
            }
        });
    let mut tree = WidgetTree::new();
    let root = tree.mount(listener.into()).expect("mount listener");
    tree.layout(Constraints::tight(Size::new(40., 40.)));
    assert_eq!(dispatcher.handler_count(), 1);
    assert!(dispatcher.dispatch_back().handled);
    assert_eq!(calls.get(), 1);

    tree.update(
        root,
        BackButtonListener::new(Text::new("replacement")).into(),
    )
    .expect("replace listener");
    assert_eq!(dispatcher.handler_count(), 0);
    assert!(!dispatcher.dispatch_back().handled);
}

#[test]
fn page_storage_uses_ordered_key_chains_and_exposes_the_ambient_bucket() {
    let bucket = PageStorageBucket::default();
    let tab = PageStorageKey::new("tab");
    let list = PageStorageKey::new("list");
    assert!(bucket.write_state(vec![tab.clone(), list.clone()], json!({"offset": 12})));
    assert_eq!(
        bucket.read_state(vec![tab.clone(), list.clone()]),
        Some(json!({"offset": 12}))
    );
    assert!(!bucket.write_state(vec![tab.clone(), list.clone()], json!({"offset": 12})));
    assert_eq!(bucket.len(), 1);
    assert!(
        bucket
            .remove_state(PageStorageKey::new("missing"))
            .is_none()
    );

    let seen = Rc::new(RefCell::new(None));
    let seen_in_builder = seen.clone();
    let child = Widget::layout_builder(move |context, _| {
        *seen_in_builder.borrow_mut() = PageStorage::maybe_of(context);
        SizedBox::shrink().into()
    });
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(PageStorage::with_bucket(bucket.clone(), child).into())
        .expect("mount page storage");
    tree.layout(Constraints::tight(Size::new(40., 40.)));
    assert_eq!(*seen.borrow(), Some(bucket));
    assert!(tree.element_exists(root));
}

#[test]
fn restoration_scopes_claim_stable_paths_and_none_shadows_ambient_scope() {
    let root_scope = restoration_scope();
    let seen = Rc::new(RefCell::new(None));
    let seen_in_builder = seen.clone();
    let child = Widget::layout_builder(move |context, _| {
        *seen_in_builder.borrow_mut() = current_restoration_scope(context);
        SizedBox::shrink().into()
    });
    let mut tree = WidgetTree::new();
    tree.mount(RootRestorationScope::with_scope(root_scope.clone(), "application", child).into())
        .expect("mount root restoration scope");
    tree.layout(Constraints::tight(Size::new(40., 40.)));
    let claimed = seen.borrow().clone().expect("claimed restoration scope");
    assert_eq!(
        claimed
            .path()
            .iter()
            .map(RestorationKey::as_str)
            .collect::<Vec<_>>(),
        vec!["window", "application"]
    );

    let seen_disabled = Rc::new(RefCell::new(Some(root_scope.clone())));
    let seen_in_builder = seen_disabled.clone();
    let child = Widget::layout_builder(move |context, _| {
        *seen_in_builder.borrow_mut() = current_restoration_scope(context);
        SizedBox::shrink().into()
    });
    let mut tree = WidgetTree::new();
    tree.mount(UnmanagedRestorationScope::with_optional_scope(None, child).into())
        .expect("mount disabled restoration scope");
    tree.layout(Constraints::tight(Size::new(40., 40.)));
    assert!(seen_disabled.borrow().is_none());
}

#[test]
fn animated_barrier_samples_animation_and_dismisses_from_the_configured_bridge() {
    let controller = AnimationController::new(Duration::from_secs(1));
    let animation = Animation::new(
        controller.clone(),
        incular_animation::TweenValue::new(Color::TRANSPARENT, Color::BLACK),
    );
    let dismissals = Rc::new(Cell::new(0));
    let barrier = AnimatedModalBarrier::with_color(animation)
        .semantics_label("Dismiss dialog")
        .dismissal_handler({
            let dismissals = dismissals.clone();
            move || {
                dismissals.set(dismissals.get() + 1);
                true
            }
        });
    assert_eq!(barrier.current_color(), Color::TRANSPARENT);
    controller.set_value(1.0);
    assert_eq!(barrier.current_color(), Color::BLACK);
    assert!(barrier.dismiss());
    assert_eq!(dismissals.get(), 1);
    assert!(!barrier.clone().dismissible(false).dismiss());

    let mut tree = WidgetTree::new();
    let root = tree.mount(barrier.into()).expect("mount barrier");
    tree.layout(Constraints::tight(Size::new(160., 90.)));
    assert_eq!(
        tree.render_size(tree.render_id(root).expect("barrier render")),
        Some(Size::new(160., 90.))
    );
}
