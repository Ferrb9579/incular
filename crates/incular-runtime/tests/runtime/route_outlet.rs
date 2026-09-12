//! Production outlet integration: real navigation plus real mounting,
//! with no manual save/restore/forget calls at any transition. The outlet
//! drives focus records and task bindings from live navigator state across
//! frames; tests assert the observable agreement.

use super::*;
use incular_navigation::{Navigator, NavigatorObserver, Page, PageKey};
use incular_widgets::{Column, Focus, FocusNode};

fn focus_child(node: &FocusNode) -> Widget {
    Focus::new(Widget::box_(Size::new(40., 40.), Color::WHITE))
        .node(node.clone())
        .into()
}

fn focus_page(name: &str, node: &FocusNode) -> Page {
    Page::new(name, focus_child(node))
}

fn plain_page(name: &str) -> Page {
    Page::new(name, Widget::box_(Size::new(40., 40.), Color::WHITE))
}

fn frame(runtime: &mut Runtime) {
    runtime
        .run_frame(Constraints::tight(Size::new(200., 200.)))
        .expect("frame");
}

fn tab_until(runtime: &mut Runtime, node: &FocusNode) {
    for _ in 0..4 {
        if node.has_focus() {
            return;
        }
        let _ = runtime.handle_input(InputEvent::Key(key_down(Code::Tab)));
    }
    assert!(node.has_focus(), "tab reaches the expected node");
}

struct OutletHarness {
    runtime: Runtime,
    outlet: Rc<RefCell<RouteOutlet>>,
    revision: Signal<u32>,
    _observer: NavigatorObserver,
}

/// Hosts `navigator` through an outlet: the root rebuilds outlet content
/// whenever navigation bumps the revision, exactly like a production host.
fn harness(navigator: &Navigator) -> OutletHarness {
    let mut runtime =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    let parent = runtime.spawner().scope();
    let outlet = Rc::new(RefCell::new(RouteOutlet::new(navigator, &parent)));
    let outlet_for_build = outlet.clone();
    let revision = Signal::new(0_u32);
    let revision_for_build = revision.clone();
    let revision_for_observer = revision.clone();
    let root = runtime.tree().root().expect("root");
    runtime
        .register_builder(root, move || {
            let _ = revision_for_build.get();
            Column::new(vec![outlet_for_build.borrow().widget()]).into()
        })
        .expect("builder registers");
    let observer = navigator.observe(move |_| {
        revision_for_observer.set(revision_for_observer.get().wrapping_add(1));
    });
    OutletHarness {
        runtime,
        outlet,
        revision,
        _observer: observer,
    }
}

/// Presents one production cycle: rebuild after navigation, then let the
/// outlet reconcile integration state with the mounted tree.
fn present(harness: &mut OutletHarness) {
    harness.revision.set(harness.revision.get().wrapping_add(1));
    frame(&mut harness.runtime);
    harness
        .outlet
        .borrow_mut()
        .after_frame(&mut harness.runtime);
}

#[test]
fn outlet_push_pop_restores_focus_without_manual_drive() {
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    navigator.push_page(focus_page("b", &node_b));
    present(&mut harness);
    // Deactivation saved A's focus through the outlet; B has no record, so
    // focus stays where the transition left it until the user moves it.
    tab_until(&mut harness.runtime, &node_b);
    navigator.pop();
    present(&mut harness);
    assert!(node_a.has_focus());
    assert!(
        harness.runtime.focused_element().is_some(),
        "reactivation restores the eligible saved target"
    );
}

#[test]
fn outlet_reorder_preserves_and_repush_forgets() {
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let key = |name: &str| PageKey::new(name).unwrap();
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    navigator.push_page(focus_page("b", &node_b));
    present(&mut harness);
    // Only the active route is focusable through the outlet: B is on top.
    tab_until(&mut harness.runtime, &node_b);
    // Keyed reorder deactivates B without removing it: the record survives
    // reconciliation and restores after reordering back.
    navigator
        .set_pages([
            Page::new("b", focus_child(&node_b)).key(key("b")),
            Page::new("a", focus_child(&node_a)).key(key("a")),
        ])
        .unwrap();
    present(&mut harness);
    assert!(node_b.has_focus());
    tab_until(&mut harness.runtime, &node_a);
    navigator
        .set_pages([
            Page::new("a", focus_child(&node_a)).key(key("a")),
            Page::new("b", focus_child(&node_b)).key(key("b")),
        ])
        .unwrap();
    present(&mut harness);
    assert!(node_b.has_focus());
    // Permanent removal forgets through the outlet: re-pushing the same key
    // mounts a fresh route with no history to restore and a fresh binding.
    navigator.pop();
    navigator.pop();
    present(&mut harness);
    assert_eq!(harness.runtime.focused_element(), None);
    let node_a2 = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a2));
    present(&mut harness);
    assert_eq!(harness.runtime.focused_element(), None);
    assert!(!node_a2.has_focus());
    let fresh_id = navigator.current().expect("repushed route").id;
    let fresh_scope = harness
        .outlet
        .borrow()
        .route_task_scope(fresh_id)
        .expect("repushed route is bound");
    assert!(!fresh_scope.is_cancelled());
}

#[test]
fn outlet_nested_navigators_stay_isolated() {
    let outer = Navigator::new();
    let inner = Navigator::new();
    let mut runtime =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    let parent = runtime.spawner().scope();
    let outlet_outer = Rc::new(RefCell::new(RouteOutlet::new(&outer, &parent)));
    let outlet_inner = Rc::new(RefCell::new(RouteOutlet::new(&inner, &parent)));
    let revision = Signal::new(0_u32);
    let root = runtime.tree().root().expect("root");
    {
        let outlet_outer = outlet_outer.clone();
        let outlet_inner = outlet_inner.clone();
        let revision = revision.clone();
        runtime
            .register_builder(root, move || {
                let _ = revision.get();
                Column::new(vec![
                    outlet_outer.borrow().widget(),
                    outlet_inner.borrow().widget(),
                ])
                .into()
            })
            .expect("builder registers");
    }
    let revision_for_observers = revision.clone();
    let _outer_observer = outer.observe(move |_| {
        revision_for_observers.set(revision_for_observers.get().wrapping_add(1));
    });
    let revision_for_observers = revision.clone();
    let _inner_observer = inner.observe(move |_| {
        revision_for_observers.set(revision_for_observers.get().wrapping_add(1));
    });
    let present_all = |runtime: &mut Runtime| {
        revision.set(revision.get().wrapping_add(1));
        frame(runtime);
        outlet_outer.borrow_mut().after_frame(runtime);
        outlet_inner.borrow_mut().after_frame(runtime);
    };

    let node_a1 = FocusNode::new();
    let node_a2 = FocusNode::new();
    let node_b1 = FocusNode::new();
    let node_b2 = FocusNode::new();
    outer.push_page(focus_page("a1", &node_a1));
    inner.push_page(focus_page("b1", &node_b1));
    present_all(&mut runtime);
    tab_until(&mut runtime, &node_a1);
    outer.push_page(focus_page("a2", &node_a2));
    present_all(&mut runtime);
    tab_until(&mut runtime, &node_a2);
    outer.pop();
    present_all(&mut runtime);
    assert!(node_a1.has_focus());
    // B's transition runs while A's focus sits elsewhere: A keeps its
    // focus because B's outlet records nothing foreign and restores
    // nothing over it.
    inner.push_page(focus_page("b2", &node_b2));
    present_all(&mut runtime);
    assert!(node_a1.has_focus());
    inner.pop();
    present_all(&mut runtime);
    assert!(node_a1.has_focus());
    let _ = node_b1;
}

#[test]
fn outlet_removal_cancels_bound_tasks() {
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let wake = Arc::new(TestWake::default());
    harness.runtime.set_wake_handler(wake.clone());
    navigator.push_page(plain_page("task"));
    present(&mut harness);
    let id = navigator.current().expect("mounted").id;
    let scope = harness
        .outlet
        .borrow()
        .route_task_scope(id)
        .expect("mounted route is bound");
    assert!(!scope.is_cancelled());
    let completed = Arc::new(AtomicBool::new(false));
    let completed_for_completion = completed.clone();
    harness.runtime.spawner().spawn_into_in(
        &scope,
        async move {
            std::future::pending::<()>().await;
        },
        move |_, _| {
            completed_for_completion.store(true, Ordering::Release);
        },
    );
    // The outlet created the binding itself; removal cancels through it
    // with no manual wiring, and the late completion discards.
    navigator.pop();
    assert!(scope.is_cancelled());
    present(&mut harness);
    wait_for_wake(&wake);
    harness.runtime.process_runtime_work();
    assert!(!completed.load(Ordering::Acquire));
    assert!(harness.outlet.borrow().route_task_scope(id).is_none());
}

#[test]
fn outlet_teardown_releases_focus_and_tasks() {
    // The builder holds the outlet weakly so dropping it models window
    // teardown: content unmounts on the next frame, bindings detach per
    // documented policy, and later navigation moves nothing.
    let navigator = Navigator::new();
    let mut runtime =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    let parent = runtime.spawner().scope();
    let outlet = Rc::new(RefCell::new(RouteOutlet::new(&navigator, &parent)));
    let weak = Rc::downgrade(&outlet);
    let revision = Signal::new(0_u32);
    let root = runtime.tree().root().expect("root");
    {
        let revision = revision.clone();
        runtime
            .register_builder(root, move || {
                let _ = revision.get();
                Column::new(vec![
                    weak.upgrade()
                        .map(|outlet| outlet.borrow().widget())
                        .unwrap_or_else(|| Widget::box_(Size::new(200., 200.), Color::WHITE)),
                ])
                .into()
            })
            .expect("builder registers");
    }
    let observer = navigator.observe({
        let revision = revision.clone();
        move |_| {
            revision.set(revision.get().wrapping_add(1));
        }
    });
    let present_teardown = |runtime: &mut Runtime| {
        revision.set(revision.get().wrapping_add(1));
        frame(runtime);
    };
    let node_a = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    present_teardown(&mut runtime);
    outlet.borrow_mut().after_frame(&mut runtime);
    tab_until(&mut runtime, &node_a);
    let id = navigator.current().expect("mounted").id;
    let scope = outlet.borrow().route_task_scope(id).expect("bound");
    drop(outlet);
    // Teardown: content unmounts (runtime clears the dead focus itself),
    // and the detached binding ignores the later removal.
    present_teardown(&mut runtime);
    assert_eq!(runtime.focused_element(), None);
    navigator.pop();
    present_teardown(&mut runtime);
    assert!(!scope.is_cancelled());
    assert_eq!(runtime.focused_element(), None);
    drop(observer);
}
