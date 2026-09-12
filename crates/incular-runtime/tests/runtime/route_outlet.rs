//! Production outlet integration: real navigation plus real mounting,
//! with no manual save/restore/forget calls at any transition. The outlet
//! drives focus records and task bindings from live navigator state across
//! frames; tests assert the observable agreement.

use super::*;
use incular_navigation::{
    ModalBarrier, Navigator, OverlayEntry, Page, PageKey, Route, RoutePresentation, RouteTransition,
};
use incular_widgets::{
    Column, Focus, FocusNode, GestureDetector, SizedBox,
    internal::{Key, OpacityController, TranslationController},
};

fn focus_child(node: &FocusNode) -> Widget {
    Focus::new(Widget::box_(Size::new(40., 40.), Color::WHITE))
        .node(node.clone())
        .into()
}

fn focus_page(name: &str, node: &FocusNode) -> Page {
    Page::new(name, focus_child(node))
}

fn focus_widget(node: &FocusNode, child: Widget) -> Widget {
    Focus::new(child).node(node.clone()).into()
}

fn plain_page(name: &str) -> Page {
    Page::new(name, Widget::box_(Size::new(40., 40.), Color::WHITE))
}

fn frame(runtime: &mut Runtime) {
    runtime
        .run_frame(Constraints::tight(Size::new(200., 200.)))
        .expect("frame");
}

const RED: Color = Color::rgba(255, 0, 0, 255);
const GREEN: Color = Color::rgba(0, 255, 0, 255);
const BLUE: Color = Color::rgba(0, 0, 255, 255);

fn repaint(harness: &mut OutletHarness) -> DisplayList {
    harness
        .runtime
        .run_frame(Constraints::tight(Size::new(200., 200.)))
        .expect("repaint")
        .0
}

fn paints(commands: &[PaintCommand], color: Color) -> bool {
    commands
        .iter()
        .any(|command| matches!(command, PaintCommand::Rect { color: c, .. } if *c == color))
}

fn tap(harness: &mut OutletHarness, x: f32, y: f32) {
    for phase in [PointerPhase::Down, PointerPhase::Up] {
        let _ = harness.runtime.handle_input(InputEvent::Pointer {
            phase,
            position: Offset::new(x, y),
        });
    }
}

fn tappable_box(color: Color, size: f32, taps: &Rc<Cell<u32>>) -> Widget {
    tappable_rect(color, size, size, taps)
}

fn tappable_rect(color: Color, width: f32, height: f32, taps: &Rc<Cell<u32>>) -> Widget {
    let taps = taps.clone();
    GestureDetector::new(Widget::box_(Size::new(width, height), color))
        .on_tap(move || {
            taps.set(taps.get().wrapping_add(1));
        })
        .into()
}

fn labeled(widget: Widget, label: &str) -> Widget {
    widget.semantics(ExplicitSemantics::new(SemanticRole::GenericContainer).label(label.to_owned()))
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
}

/// Hosts `navigator` through an outlet exactly like a production host:
/// mount once, attach once, then drive every cycle with the single
/// `present_frame` operation (which owns invalidation internally, so no
/// external revision signal or observer subscription is needed).
fn harness(navigator: &Navigator) -> OutletHarness {
    let mut runtime =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    let parent = runtime.spawner().scope();
    let outlet = Rc::new(RefCell::new(RouteOutlet::new(navigator, &parent)));
    let outlet_for_build = outlet.clone();
    let root = runtime.tree().root().expect("root");
    runtime
        .register_builder(root, move || {
            // Bounded hosting like a production window root: modal veils
            // size to this area instead of an unbounded column axis.
            Column::new(vec![SizedBox::from_dimensions(
                Some(200.),
                Some(200.),
                Some(outlet_for_build.borrow().widget()),
            )])
            .into()
        })
        .expect("builder registers");
    frame(&mut runtime);
    RouteOutlet::attach(&outlet, &mut runtime).expect("outlet mounted");
    let mut harness = OutletHarness { runtime, outlet };
    present(&mut harness);
    harness
}

/// Presents one production cycle through the single supported operation:
/// rebuild, frame, and reconcile in enforced order.
fn present(harness: &mut OutletHarness) {
    RouteOutlet::present_frame(
        &harness.outlet,
        &mut harness.runtime,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect("present frame");
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
    let root = runtime.tree().root().expect("root");
    {
        let outlet_outer = outlet_outer.clone();
        let outlet_inner = outlet_inner.clone();
        runtime
            .register_builder(root, move || {
                Column::new(vec![
                    SizedBox::from_dimensions(
                        Some(200.),
                        Some(100.),
                        Some(outlet_outer.borrow().widget()),
                    ),
                    SizedBox::from_dimensions(
                        Some(200.),
                        Some(100.),
                        Some(outlet_inner.borrow().widget()),
                    ),
                ])
                .into()
            })
            .expect("builder registers");
    }
    frame(&mut runtime);
    RouteOutlet::attach(&outlet_outer, &mut runtime).expect("outer mounted");
    RouteOutlet::attach(&outlet_inner, &mut runtime).expect("inner mounted");
    let present_all = |runtime: &mut Runtime| {
        RouteOutlet::present_frame(
            &outlet_outer,
            runtime,
            Constraints::tight(Size::new(200., 200.)),
        )
        .expect("present outer");
        RouteOutlet::present_frame(
            &outlet_inner,
            runtime,
            Constraints::tight(Size::new(200., 200.)),
        )
        .expect("present inner");
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
fn outlet_integrates_focus_tasks_and_lifetimes_together() {
    // One supported production flow: a covered route keeps its focus
    // record and its tasks; the way back restores focus, cancels the
    // removed route's scope (discarding its late completion), and ends
    // both lifetimes — all driven by navigation plus frames alone.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let wake = Arc::new(TestWake::default());
    harness.runtime.set_wake_handler(wake.clone());
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    let ra = navigator.current().expect("route A").id;
    let lifetime_a = navigator.lifetime_of(ra).expect("A mounted");
    let scope_a = harness
        .outlet
        .borrow()
        .route_task_scope(ra)
        .expect("A bound");
    let completed = Arc::new(AtomicBool::new(false));
    let completed_for_completion = completed.clone();
    harness.runtime.spawner().spawn_into_in(
        &scope_a,
        async move {
            std::future::pending::<()>().await;
        },
        move |_, _| {
            completed_for_completion.store(true, Ordering::Release);
        },
    );
    navigator.push_page(focus_page("b", &node_b));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_b);
    let rb = navigator.current().expect("route B").id;
    let lifetime_b = navigator.lifetime_of(rb).expect("B mounted");
    // Covered, not removed: A's scope stays live with its pending task.
    assert!(!scope_a.is_cancelled());
    navigator.pop();
    present(&mut harness);
    assert!(node_a.has_focus());
    assert!(lifetime_a.is_live());
    assert!(!lifetime_b.is_live());
    navigator.pop();
    present(&mut harness);
    assert_eq!(harness.runtime.focused_element(), None);
    assert!(!lifetime_a.is_live());
    assert!(scope_a.is_cancelled());
    wait_for_wake(&wake);
    harness.runtime.process_runtime_work();
    assert!(!completed.load(Ordering::Acquire));
}

#[test]
fn outlet_opaque_page_retains_without_painting_below() {
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    let taps_a = Rc::new(Cell::new(0_u32));
    let taps_b = Rc::new(Cell::new(0_u32));
    navigator.push(
        Route::new(
            "a",
            Column::new(vec![
                focus_widget(&node_a, Widget::box_(Size::new(40., 40.), RED)),
                tappable_rect(RED, 200., 160., &taps_a),
            ]),
        )
        .presentation(RoutePresentation::page()),
    );
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    let ea = harness.runtime.focused_element().expect("A focused");
    navigator.push(
        Route::new(
            "b",
            Column::new(vec![
                focus_widget(&node_b, Widget::box_(Size::new(40., 40.), BLUE)),
                tappable_rect(BLUE, 200., 40., &taps_b),
            ]),
        )
        .presentation(RoutePresentation::page()),
    );
    present(&mut harness);
    // Mounted lifetime: the covered route stays mounted (retained), but its
    // opaque cover means no paint below, no focus below, and no input below.
    assert!(harness.runtime.tree().element_exists(ea));
    let commands = repaint(&mut harness).commands().to_vec();
    assert!(paints(&commands, BLUE));
    assert!(
        !paints(&commands, RED),
        "the opaque cover occludes the retained route"
    );
    tab_until(&mut harness.runtime, &node_b);
    tap(&mut harness, 10., 50.);
    assert_eq!(taps_b.get(), 1);
    tap(&mut harness, 100., 100.);
    assert_eq!(taps_a.get(), 0);
}

#[test]
fn outlet_transparent_popup_paints_and_passes_through() {
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let taps_a = Rc::new(Cell::new(0_u32));
    let taps_b = Rc::new(Cell::new(0_u32));
    navigator.push(
        Route::new("a", labeled(tappable_box(RED, 200., &taps_a), "route-a"))
            .presentation(RoutePresentation::page()),
    );
    navigator.push(
        Route::new("b", tappable_box(BLUE, 40., &taps_b))
            .presentation(RoutePresentation::popup(None)),
    );
    present(&mut harness);
    // Visible underlying content paints, keeps semantics, and receives
    // input where the popup does not cover it.
    let commands = repaint(&mut harness).commands().to_vec();
    assert!(paints(&commands, RED));
    assert!(paints(&commands, BLUE));
    assert!(
        harness
            .runtime
            .tree()
            .semantics_debug_dump()
            .contains("route-a")
    );
    tap(&mut harness, 100., 100.);
    assert_eq!(taps_a.get(), 1);
    tap(&mut harness, 10., 10.);
    assert_eq!(taps_b.get(), 1);
    assert_eq!(taps_a.get(), 1);
}

#[test]
fn outlet_modal_barrier_blocks_input_and_semantics() {
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let taps_a = Rc::new(Cell::new(0_u32));
    navigator.push(
        Route::new("a", labeled(tappable_box(RED, 200., &taps_a), "route-a"))
            .presentation(RoutePresentation::page()),
    );
    navigator.push(
        Route::new("b", Widget::box_(Size::new(40., 40.), BLUE)).presentation(
            RoutePresentation::modal(ModalBarrier {
                dismissible: false,
                ..Default::default()
            }),
        ),
    );
    present(&mut harness);
    // Translucent-safe paint-through still paints below, but the veil
    // absorbs pointer input and hides background semantics.
    let commands = repaint(&mut harness).commands().to_vec();
    assert!(paints(&commands, BLUE));
    tap(&mut harness, 100., 100.);
    assert_eq!(taps_a.get(), 0);
    assert!(
        !harness
            .runtime
            .tree()
            .semantics_debug_dump()
            .contains("route-a")
    );
}

#[test]
fn outlet_dismissible_barrier_tap_pops_its_route() {
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    navigator.push(Route::new("a", Widget::box_(Size::new(200., 200.), RED)));
    navigator.push(
        Route::new("b", Widget::box_(Size::new(40., 40.), BLUE))
            .presentation(RoutePresentation::modal(ModalBarrier::default())),
    );
    present(&mut harness);
    assert_eq!(navigator.routes().len(), 2);
    tap(&mut harness, 100., 100.);
    assert_eq!(
        navigator.routes().len(),
        1,
        "tapping the dismissible veil pops its own route"
    );
    assert_eq!(navigator.current().expect("remaining").name, "a");
}

#[test]
fn outlet_disposes_unretained_covered_content() {
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    navigator.push(
        Route::new(
            "a",
            Column::new(vec![focus_widget(
                &node_a,
                Widget::box_(Size::new(40., 40.), RED),
            )]),
        )
        .presentation(RoutePresentation::Page {
            opaque: true,
            maintain_state: false,
            fullscreen_dialog: false,
        }),
    );
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    let ea = harness.runtime.focused_element().expect("A focused");
    let ra = navigator.current().expect("route A").id;
    navigator.push(
        Route::new(
            "b",
            Column::new(vec![focus_widget(
                &node_b,
                Widget::box_(Size::new(40., 40.), BLUE),
            )]),
        )
        .presentation(RoutePresentation::page()),
    );
    present(&mut harness);
    // Disposal unmounts the covered route (no paint, no focus, no element)
    // while its navigator lifetime — and lifetime-bound tasks — persist.
    assert!(!harness.runtime.tree().element_exists(ea));
    let commands = repaint(&mut harness).commands().to_vec();
    assert!(!paints(&commands, RED));
    tab_until(&mut harness.runtime, &node_b);
    let lifetime_a = navigator
        .lifetime_of(ra)
        .expect("lifetime outlives disposal");
    assert!(lifetime_a.is_live());
    let scope_a = harness
        .outlet
        .borrow()
        .route_task_scope(ra)
        .expect("binding follows the lifetime, not the mount");
    assert!(!scope_a.is_cancelled());
}

#[test]
fn outlet_transition_keeps_route_identity() {
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a = FocusNode::new();
    let route = Route::new(
        "a",
        Column::new(vec![focus_widget(
            &node_a,
            Widget::box_(Size::new(40., 40.), RED),
        )]),
    )
    .transition(RouteTransition::FadeSlide {
        opacity: OpacityController::new(),
        translation: TranslationController::new(),
    });
    navigator.push(route);
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    let ea = harness.runtime.focused_element().expect("A focused");
    // Idle frames keep the transitioned element identical, and a covered
    // return still restores through the stable identity.
    present(&mut harness);
    assert_eq!(harness.runtime.focused_element(), Some(ea));
    navigator.push(Route::new("b", Widget::box_(Size::new(40., 40.), BLUE)));
    present(&mut harness);
    navigator.pop();
    present(&mut harness);
    assert_eq!(harness.runtime.focused_element(), Some(ea));
    assert!(node_a.has_focus());
}

#[test]
fn outlet_preserves_caller_keys_on_route_content() {
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a = FocusNode::new();
    navigator.push(Route::new(
        "a",
        Widget::from(Column::new(vec![focus_widget(
            &node_a,
            Widget::box_(Size::new(40., 40.), RED),
        )]))
        .with_key(99_u64),
    ));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    let ea = harness.runtime.focused_element().expect("A focused");
    // The outlet tags its own wrapper, never the presented child: the
    // caller-keyed column reconciles to the identical element across
    // rebuilds while focus anchors the inner focus element.
    let before = harness
        .runtime
        .tree()
        .element_with_key(&Key::from(99_u64))
        .expect("caller key resolves");
    present(&mut harness);
    let after = harness
        .runtime
        .tree()
        .element_with_key(&Key::from(99_u64))
        .expect("caller key survives outlet tagging");
    assert_eq!(before, after);
    navigator.push(Route::new("b", Widget::box_(Size::new(40., 40.), BLUE)));
    present(&mut harness);
    navigator.pop();
    present(&mut harness);
    assert_eq!(harness.runtime.focused_element(), Some(ea));
}

#[test]
fn outlet_rejects_overlay_routes_explicitly() {
    assert_eq!(
        OutletPlacement::of(&RoutePresentation::overlay(vec![]), true),
        OutletPlacement::ExternalOverlay
    );
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    navigator.push(Route::new("a", Widget::box_(Size::new(200., 200.), RED)));
    navigator.push(
        Route::new("b", Widget::box_(Size::new(40., 40.), GREEN)).overlay(vec![OverlayEntry::new(
            Widget::box_(Size::new(40., 40.), GREEN),
        )]),
    );
    present(&mut harness);
    // The overlay route is active but never outlet-mounted: its content
    // paints nowhere here (a portal host owns it), while ordinary content
    // is unaffected.
    let commands = repaint(&mut harness).commands().to_vec();
    assert!(paints(&commands, RED));
    assert!(!paints(&commands, GREEN));
    assert_eq!(navigator.routes().len(), 2);
}

#[test]
fn outlet_manual_after_frame_still_drives() {
    // Escape hatch for custom hosts that own their frame loop: widget()
    // plus an explicit after_frame, with host-owned invalidation. Prefer
    // present_frame, which enforces save-before-reconcile ordering.
    let navigator = Navigator::new();
    let mut runtime =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    let parent = runtime.spawner().scope();
    let outlet = Rc::new(RefCell::new(RouteOutlet::new(&navigator, &parent)));
    let revision = Signal::new(0_u32);
    let root = runtime.tree().root().expect("root");
    {
        let outlet = outlet.clone();
        let revision = revision.clone();
        runtime
            .register_builder(root, move || {
                let _ = revision.get();
                Column::new(vec![SizedBox::from_dimensions(
                    Some(200.),
                    Some(200.),
                    Some(outlet.borrow().widget()),
                )])
                .into()
            })
            .expect("builder registers");
    }
    let drive = |runtime: &mut Runtime| {
        revision.set(revision.get().wrapping_add(1));
        frame(runtime);
        outlet.borrow_mut().after_frame(runtime);
    };
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    drive(&mut runtime);
    tab_until(&mut runtime, &node_a);
    navigator.push_page(focus_page("b", &node_b));
    drive(&mut runtime);
    tab_until(&mut runtime, &node_b);
    navigator.pop();
    drive(&mut runtime);
    assert!(node_a.has_focus());
}

#[test]
fn outlet_incoming_autofocus_then_return_restores() {
    // Phase proof: the incoming route autofocuses on its mount frame while
    // the outgoing record (saved before reconciliation) survives it; popping
    // unmounts the incoming focus and the return restores the saved one.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    navigator.push_page(Page::new(
        "b",
        Focus::new(Widget::box_(Size::new(40., 40.), BLUE))
            .node(node_b.clone())
            .autofocus(true),
    ));
    present(&mut harness);
    assert!(
        node_b.has_focus(),
        "incoming autofocus wins its mount frame"
    );
    navigator.pop();
    present(&mut harness);
    assert!(
        node_a.has_focus(),
        "the pre-reconcile save restores on return"
    );
    assert!(harness.runtime.focused_element().is_some());
}

#[test]
fn outlet_deferred_enablement_converges_without_retries() {
    // Eligibility deferred past the transition: the target stays recorded
    // while disabled, and ordinary frames restore it once enabled — no
    // caller retries, and never by displacing valid focus.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    navigator.push_page(plain_page("b"));
    present(&mut harness);
    node_a.set_can_request_focus(false);
    navigator.pop();
    present(&mut harness);
    assert_eq!(harness.runtime.focused_element(), None);
    node_a.set_can_request_focus(true);
    present(&mut harness);
    assert!(node_a.has_focus());
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
