//! Production outlet integration: real navigation plus real mounting,
//! with no manual save/restore/forget calls at any transition. The outlet
//! drives focus records and task bindings from live navigator state across
//! frames; tests assert the observable agreement.

use super::*;
use incular_navigation::{
    ModalBarrier, Navigator, OverlayEntry, Page, PageKey, Route, RoutePresentation, RouteTransition,
};
use incular_widgets::{
    Column, Focus, FocusNode, GestureDetector, LayoutBuilder, SizedBox, Stack,
    internal::{Key, OpacityController, TranslationController, TreeError},
};
use std::panic::{AssertUnwindSafe, catch_unwind};

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
const YELLOW: Color = Color::rgba(255, 255, 0, 255);

fn repaint(harness: &mut OutletHarness) -> DisplayList {
    harness
        .runtime
        .run_frame(Constraints::tight(Size::new(200., 200.)))
        .expect("repaint")
        .0
}

fn paints(commands: &[PaintCommand], color: Color) -> bool {
    commands.iter().any(|command| match command {
        PaintCommand::Rect { color: c, .. } => *c == color,
        PaintCommand::RRect { brush, .. } => {
            matches!(brush, incular_rendering::Brush::Solid(c) if *c == color)
        }
        _ => false,
    })
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

/// Pumps runtime work until `done` or a timeout: wake counters may already
/// be satisfied by earlier frames, so waiting on them alone can return
/// before the task under test settles.
fn pump_until(runtime: &mut Runtime, done: &AtomicBool) {
    let start = Instant::now();
    while !done.load(Ordering::Acquire) {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "runtime work never settled"
        );
        runtime.process_runtime_work();
        std::thread::yield_now();
    }
}

/// Pumps until the scheduler records the single cancelled task: proves a
/// discard ran rather than merely observing silence. Absolute rather than
/// baseline-relative, because the discard may already have drained during
/// an intervening frame.
fn pump_until_discarded(harness: &mut OutletHarness) {
    let start = Instant::now();
    while harness.runtime.runtime_diagnostics().tasks_cancelled < 1 {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "cancelled task never discarded"
        );
        harness.runtime.process_runtime_work();
        std::thread::yield_now();
    }
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
fn present(harness: &mut OutletHarness) -> DisplayList {
    RouteOutlet::present_frame(
        &harness.outlet,
        &mut harness.runtime,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect("present frame")
    .0
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
    pump_until_discarded(&mut harness);
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
    pump_until_discarded(&mut harness);
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
    let overlay_id = navigator.current().expect("overlay route").id;
    let outlet_id = harness.outlet.borrow().id();
    // Presenting fails typed before mounting or committing anything: the
    // overlay content is portal-managed, and silent omission is not an
    // option. Ordinary state stays exactly as it was.
    let error = RouteOutlet::present_frame(
        &harness.outlet,
        &mut harness.runtime,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect_err("overlay stacks are rejected");
    assert_eq!(
        error,
        OutletError::UnsupportedPresentation {
            outlet: outlet_id,
            routes: vec![overlay_id]
        }
    );
    assert!(harness.runtime.focused_element().is_none());
    assert!(
        harness
            .outlet
            .borrow()
            .route_task_scope(overlay_id)
            .is_none()
    );
    // Removing the offending route recovers the outlet immediately.
    navigator.pop();
    present(&mut harness);
    assert_eq!(navigator.routes().len(), 1);
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
    assert!(
        harness.runtime.frame_requested(),
        "the return restore schedules its follow-up"
    );
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
    assert!(
        harness.runtime.frame_requested(),
        "deferred restore schedules its follow-up"
    );
}

/// Hosts an inner outlet inside outer route A's content: the inner
/// outlet mounts in a keyed slot of A's subtree, so inner focus sits
/// strictly below the outer route tag.
type NestedSetup = (
    Navigator,
    Navigator,
    Runtime,
    Rc<RefCell<RouteOutlet>>,
    Rc<RefCell<RouteOutlet>>,
    FocusNode,
    FocusNode,
);

fn nested_inside_setup() -> NestedSetup {
    let outer = Navigator::new();
    let inner = Navigator::new();
    let mut runtime =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    let parent = runtime.spawner().scope();
    let outlet_outer = Rc::new(RefCell::new(RouteOutlet::new(&outer, &parent)));
    let outlet_inner = Rc::new(RefCell::new(RouteOutlet::new(&inner, &parent)));
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    // The nested outlet mounts through the supported composition helper:
    // no hand-assembled builders, no revision handles in test code.
    let outlet_inner_for_page = outlet_inner.clone();
    outer.push_page(Page::new(
        "a",
        Column::new(vec![
            focus_widget(&node_a, Widget::box_(Size::new(40., 40.), RED)),
            RouteOutlet::nested_widget(&outlet_inner_for_page),
        ]),
    ));
    let root = runtime.tree().root().expect("root");
    {
        let outlet_outer = outlet_outer.clone();
        runtime
            .register_builder(root, move || {
                Column::new(vec![SizedBox::from_dimensions(
                    Some(200.),
                    Some(200.),
                    Some(outlet_outer.borrow().widget()),
                )])
                .into()
            })
            .expect("builder registers");
    }
    frame(&mut runtime);
    RouteOutlet::attach(&outlet_outer, &mut runtime).expect("outer mounted");
    RouteOutlet::attach_nested(&outlet_outer, &outlet_inner).expect("nested attaches");
    (
        outer,
        inner,
        runtime,
        outlet_outer,
        outlet_inner,
        node_a,
        node_b,
    )
}

/// Presents the outer outlet only: the cascade drives nested outlets in
/// the same frame, so one frame presents the whole outlet tree. Inner
/// outlets are never presented directly here — that is the point under
/// test.
fn present_outer_tree(runtime: &mut Runtime, outlet_outer: &Rc<RefCell<RouteOutlet>>) {
    RouteOutlet::present_frame(
        outlet_outer,
        runtime,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect("present outer tree");
}

/// Like [`present_outer_tree`], returning the produced display list so
/// tests can assert which content actually painted.
fn present_outer_tree_output(
    runtime: &mut Runtime,
    outlet_outer: &Rc<RefCell<RouteOutlet>>,
) -> DisplayList {
    RouteOutlet::present_frame(
        outlet_outer,
        runtime,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect("present outer tree")
    .0
}

#[test]
fn nested_inner_focus_not_saved_by_outer() {
    let (outer, inner, mut runtime, outlet_outer, _outlet_inner, node_a, node_b) =
        nested_inside_setup();
    let node_c = FocusNode::new();
    inner.push_page(focus_page("b", &node_b));
    present_outer_tree(&mut runtime, &outlet_outer);
    tab_until(&mut runtime, &node_b);
    // Outer transition while focus sits inside the nested outlet: the
    // outer save must record nothing (not the inner element), so the
    // return leaves focus alone instead of yanking it back.
    outer.push_page(Page::new(
        "ob",
        focus_widget(&node_c, Widget::box_(Size::new(40., 40.), BLUE)),
    ));
    present_outer_tree(&mut runtime, &outlet_outer);
    eprintln!(
        "PUSHED slot={:?} a={} b={} c={}",
        runtime.focused_element(),
        node_a.has_focus(),
        node_b.has_focus(),
        node_c.has_focus()
    );
    tab_until(&mut runtime, &node_c);
    eprintln!(
        "TABBED slot={:?} a={} b={} c={}",
        runtime.focused_element(),
        node_a.has_focus(),
        node_b.has_focus(),
        node_c.has_focus()
    );
    outer.pop();
    present_outer_tree(&mut runtime, &outlet_outer);
    // The return leaves focus alone instead of yanking the inner element
    // back: the slot is empty (node flags for unmounted elements go stale
    // through the frame drain, so the slot is authoritative here), outer A
    // was never focused, and the inner route stays mounted and alive.
    assert_eq!(runtime.focused_element(), None);
    assert!(!node_a.has_focus());
    assert!(inner.current().is_some());
    let _ = (node_b, node_c);
}

#[test]
fn nested_removal_reactivation_preserves_scope() {
    let (outer, inner, mut runtime, outlet_outer, outlet_inner, node_a, node_b) =
        nested_inside_setup();
    inner.push_page(focus_page("b", &node_b));
    present_outer_tree(&mut runtime, &outlet_outer);
    tab_until(&mut runtime, &node_a);
    let oa = outer.current().expect("outer route").id;
    let ib = inner.current().expect("inner route").id;
    let outer_scope = outlet_outer
        .borrow()
        .route_task_scope(oa)
        .expect("outer bound");
    let inner_scope = outlet_inner
        .borrow()
        .route_task_scope(ib)
        .expect("inner bound");
    // Inner removal cancels only the inner scope; outer focus and scope
    // survive it.
    inner.pop();
    present_outer_tree(&mut runtime, &outlet_outer);
    assert!(inner_scope.is_cancelled());
    assert!(!outer_scope.is_cancelled());
    assert!(runtime.focused_element().is_some());
    assert!(node_a.has_focus());
    // Outer deactivation records its own focus; reactivation restores it
    // while the inner outlet stays empty and independent.
    tab_until(&mut runtime, &node_a);
    let node_c = FocusNode::new();
    outer.push_page(Page::new(
        "ob",
        focus_widget(&node_c, Widget::box_(Size::new(40., 40.), BLUE)),
    ));
    present_outer_tree(&mut runtime, &outlet_outer);
    tab_until(&mut runtime, &node_c);
    outer.pop();
    present_outer_tree(&mut runtime, &outlet_outer);
    assert!(node_a.has_focus());
    assert!(!outer_scope.is_cancelled());
    assert!(inner_scope.is_cancelled());
}

/// Test-local host: a runtime whose root renders the given outer outlet,
/// like [`harness`] but without creating a second outlet for the same
/// navigator.
fn nested_host(outlet_outer: Rc<RefCell<RouteOutlet>>) -> (Runtime, Signal<u32>) {
    let mut runtime =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    let outlet_for_build = outlet_outer.clone();
    let revision = Signal::new(0_u32);
    let revision_for_build = revision.clone();
    let root = runtime.tree().root().expect("root");
    runtime
        .register_builder(root, move || {
            let _ = revision_for_build.get();
            Column::new(vec![SizedBox::from_dimensions(
                Some(200.),
                Some(200.),
                Some(outlet_for_build.borrow().widget()),
            )])
            .into()
        })
        .expect("builder registers");
    frame(&mut runtime);
    RouteOutlet::attach(&outlet_outer, &mut runtime).expect("outer mounted");
    (runtime, revision)
}

fn present_nested(
    runtime: &mut Runtime,
    outlet_outer: &Rc<RefCell<RouteOutlet>>,
    revision: &Signal<u32>,
) {
    revision.set(revision.get().wrapping_add(1));
    RouteOutlet::present_frame(
        outlet_outer,
        runtime,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect("present outer tree");
}

#[test]
fn nested_repeated_replacement_stays_bounded() {
    // Replacing the nested outlet must supersede — not accumulate —
    // registrations: dropping a replaced outlet releases it, and the live
    // count stays flat across repeated cycles while the current inner
    // outlet keeps working through the cascade alone.
    let outer = Navigator::new();
    let parent_scope_holder =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    let parent = parent_scope_holder.spawner().scope();
    let outlet_outer = Rc::new(RefCell::new(RouteOutlet::new(&outer, &parent)));
    let key = PageKey::new("a").unwrap();
    let node_a = FocusNode::new();
    let (mut runtime, revision) = nested_host(outlet_outer.clone());
    let mut live_inner: Option<Rc<RefCell<RouteOutlet>>> = None;
    for generation in 0..3_u32 {
        let inner = Navigator::new();
        let outlet_inner = Rc::new(RefCell::new(RouteOutlet::new(&inner, &parent)));
        outer
            .set_pages([Page::new(
                "a",
                Column::new(vec![
                    focus_widget(&node_a, Widget::box_(Size::new(40., 40.), RED)),
                    RouteOutlet::nested_widget(&outlet_inner),
                ]),
            )
            .key(key.clone())])
            .unwrap();
        RouteOutlet::attach_nested(&outlet_outer, &outlet_inner).expect("nested attaches");
        // Drop the previous generation *before* asserting: replacement only
        // releases once the host lets go, and the count pins exactly that.
        live_inner = Some(outlet_inner);
        // Present through the outer outlet only: the cascade drives the
        // nested outlet without its own present_frame call.
        present_nested(&mut runtime, &outlet_outer, &revision);
        assert_eq!(
            outlet_outer.borrow().attached_nested_count(),
            1,
            "replacement supersedes instead of accumulating (generation {generation})"
        );
        // The current inner outlet works: navigate inside it and tab to
        // its content, all through outer presents.
        let node_b = FocusNode::new();
        inner.push_page(focus_page("b", &node_b));
        present_nested(&mut runtime, &outlet_outer, &revision);
        tab_until(&mut runtime, &node_b);
    }
    assert_eq!(outlet_outer.borrow().attached_nested_count(), 1);
    assert!(live_inner.is_some());
    drop(live_inner);
}

#[test]
fn nested_detach_releases_on_unmount() {
    // Removing the nested widget from outer content *and* dropping the
    // outlet releases the registration: the count returns to zero and
    // outer navigation is unaffected.
    let outer = Navigator::new();
    let parent_scope_holder =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    let parent = parent_scope_holder.spawner().scope();
    let outlet_outer = Rc::new(RefCell::new(RouteOutlet::new(&outer, &parent)));
    let key = PageKey::new("a").unwrap();
    let node_a = FocusNode::new();
    let (mut runtime, revision) = nested_host(outlet_outer.clone());
    let inner = Navigator::new();
    let outlet_inner = Rc::new(RefCell::new(RouteOutlet::new(&inner, &parent)));
    outer
        .set_pages([Page::new(
            "a",
            Column::new(vec![
                focus_widget(&node_a, Widget::box_(Size::new(40., 40.), RED)),
                RouteOutlet::nested_widget(&outlet_inner),
            ]),
        )
        .key(key.clone())])
        .unwrap();
    RouteOutlet::attach_nested(&outlet_outer, &outlet_inner).expect("nested attaches");
    present_nested(&mut runtime, &outlet_outer, &revision);
    assert_eq!(outlet_outer.borrow().attached_nested_count(), 1);
    outer
        .set_pages([Page::new(
            "a",
            focus_widget(&node_a, Widget::box_(Size::new(40., 40.), RED)),
        )
        .key(key)])
        .unwrap();
    drop(outlet_inner);
    drop(inner);
    present_nested(&mut runtime, &outlet_outer, &revision);
    assert_eq!(outlet_outer.borrow().attached_nested_count(), 0);
    // Outer navigation still works end to end after the detach.
    tab_until(&mut runtime, &node_a);
}

#[test]
fn outlet_reorder_keeps_tasks_and_completes() {
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let wake = Arc::new(TestWake::default());
    harness.runtime.set_wake_handler(wake.clone());
    let key = |name: &str| PageKey::new(name).unwrap();
    // Keyed from the start: unkeyed pushes carry no identity for a later
    // keyed reconciliation to retain (their lifetimes correctly end).
    navigator
        .set_pages([
            Page::new("a", Widget::box_(Size::new(40., 40.), RED)).key(key("a")),
            Page::new("b", Widget::box_(Size::new(40., 40.), BLUE)).key(key("b")),
        ])
        .unwrap();
    present(&mut harness);
    let a_id = navigator.routes()[0].id;
    let scope_a = harness
        .outlet
        .borrow()
        .route_task_scope(a_id)
        .expect("A bound");
    let completed = Arc::new(AtomicBool::new(false));
    let completed_for_completion = completed.clone();
    harness
        .runtime
        .spawner()
        .spawn_into_in(&scope_a, async { 7_u8 }, move |result, _| {
            assert_eq!(result.expect("reorder never cancels"), 7_u8);
            completed_for_completion.store(true, Ordering::Release);
        });
    // Keyed reorder is not removal: the scope stays live and the task
    // completes normally through the real scheduler.
    navigator
        .set_pages([
            Page::new("b", Widget::box_(Size::new(40., 40.), BLUE)).key(key("b")),
            Page::new("a", Widget::box_(Size::new(40., 40.), RED)).key(key("a")),
        ])
        .unwrap();
    present(&mut harness);
    pump_until(&mut harness.runtime, &completed);
    assert!(!scope_a.is_cancelled());
}

#[test]
fn outlet_modal_tasks_cancel_on_pop() {
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let wake = Arc::new(TestWake::default());
    harness.runtime.set_wake_handler(wake.clone());
    navigator.push(Route::new("a", Widget::box_(Size::new(200., 200.), RED)));
    navigator.push(
        Route::new("b", Widget::box_(Size::new(40., 40.), BLUE))
            .presentation(RoutePresentation::modal(ModalBarrier::default())),
    );
    present(&mut harness);
    let modal_id = navigator.current().expect("modal").id;
    let scope = harness
        .outlet
        .borrow()
        .route_task_scope(modal_id)
        .expect("modal bound");
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
    navigator.pop();
    present(&mut harness);
    assert!(scope.is_cancelled());
    pump_until_discarded(&mut harness);
    assert!(!completed.load(Ordering::Acquire));
}

#[test]
fn outlet_unretained_task_survives_unmount() {
    // Task lifetime follows navigator removal, not outlet retention: the
    // task below completes normally after its content unmounts.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let wake = Arc::new(TestWake::default());
    harness.runtime.set_wake_handler(wake.clone());
    navigator.push(
        Route::new("a", Widget::box_(Size::new(40., 40.), RED)).presentation(
            RoutePresentation::Page {
                opaque: true,
                maintain_state: false,
                fullscreen_dialog: false,
            },
        ),
    );
    present(&mut harness);
    let ra = navigator.current().expect("route A").id;
    let scope_a = harness
        .outlet
        .borrow()
        .route_task_scope(ra)
        .expect("A bound");
    let completed = Arc::new(AtomicBool::new(false));
    let completed_for_completion = completed.clone();
    harness
        .runtime
        .spawner()
        .spawn_into_in(&scope_a, async { 7_u8 }, move |result, _| {
            assert_eq!(result.expect("unmount never cancels"), 7_u8);
            completed_for_completion.store(true, Ordering::Release);
        });
    navigator.push(Route::new("b", Widget::box_(Size::new(40., 40.), BLUE)));
    present(&mut harness);
    pump_until(&mut harness.runtime, &completed);
    assert!(!scope_a.is_cancelled());
}

#[test]
fn outlet_shutdown_cancels_bindings() {
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    navigator.push_page(plain_page("task"));
    present(&mut harness);
    let id = navigator.current().expect("mounted").id;
    let scope = harness.outlet.borrow().route_task_scope(id).expect("bound");
    harness.runtime.shutdown();
    assert!(scope.is_cancelled());
}

#[test]
fn outlet_transient_routes_never_bind() {
    // Navigation between construction and frame completion: routes pushed
    // and popped before any frame end their lifetime before any frame, so
    // they never bind and leave no records. Bindings follow the lifetime,
    // not mounted content — unmounted-but-live routes do bind (see the
    // disposal coverage); ended ones never do.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    navigator.push_page(plain_page("a"));
    let transient = navigator.push_page(plain_page("transient"));
    navigator.pop();
    present(&mut harness);
    assert!(navigator.lifetime_of(transient).is_none());
    assert!(
        harness
            .outlet
            .borrow()
            .route_task_scope(transient)
            .is_none()
    );
    let id = navigator.current().expect("route A").id;
    assert!(harness.outlet.borrow().route_task_scope(id).is_some());
    assert_eq!(harness.runtime.focused_element(), None);
}

fn two_focus_route(name: &str, first: &FocusNode, second: &FocusNode, color: Color) -> Page {
    Page::new(
        name,
        Column::new(vec![
            focus_widget(first, Widget::box_(Size::new(40., 40.), color)),
            focus_widget(second, Widget::box_(Size::new(40., 40.), color)),
        ]),
    )
}

#[test]
fn outlet_rebuild_failure_consumes_nothing() {
    // Duplicate keys fail the explicit rebuild — rejected by prevalidation
    // before destructive reconciliation, so the tree is exactly as before:
    // focus, elements, and bindings assert immediately, not after healing.
    // The pending capture survives uncommitted, and recovery still heals
    // cleanly for good measure.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a1 = FocusNode::new();
    let node_a2 = FocusNode::new();
    navigator.push_page(two_focus_route_page("a", &node_a1, &node_a2));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a1);
    let ea1 = harness.runtime.focused_element().expect("A1 focused");
    let ra = navigator.current().expect("route A").id;
    navigator.push(Route::new(
        "bad",
        Column::new(vec![
            Widget::box_(Size::new(40., 40.), BLUE).with_key(7_u64),
            Widget::box_(Size::new(40., 40.), GREEN).with_key(7_u64),
        ]),
    ));
    let error = match RouteOutlet::present_frame(
        &harness.outlet,
        &mut harness.runtime,
        Constraints::tight(Size::new(200., 200.)),
    ) {
        Err(error) => error,
        Ok(_) => panic!("duplicate keys must fail the rebuild"),
    };
    assert!(
        matches!(error, OutletError::Frame(TreeError::DuplicateKey { .. })),
        "unexpected failure: {error:?}"
    );
    // Nothing consumed at the integration level: A still active and bound,
    // and the bad route — alive in the navigator — was never bound. And
    // nothing disturbed at the tree level either: the failed update never
    // applied, so focus sits exactly where the user left it.
    let rb = navigator.current().expect("bad route pushed").id;
    assert!(navigator.lifetime_of(ra).expect("A alive").is_live());
    assert!(harness.outlet.borrow().route_task_scope(ra).is_some());
    assert!(navigator.lifetime_of(rb).expect("B alive").is_live());
    assert!(harness.outlet.borrow().route_task_scope(rb).is_none());
    assert_eq!(harness.runtime.focused_element(), Some(ea1));
    assert!(node_a1.has_focus());
    assert!(
        harness.outlet.borrow().needs_frame(),
        "the failed frame still owes a retry"
    );
    // Recovery heals: popping the bad route and presenting remounts clean
    // content, and focus moves freely again.
    navigator.pop();
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a2);
    let ea2 = harness.runtime.focused_element().expect("A2 focused");
    assert!(navigator.lifetime_of(rb).is_none());
    // The dropped save never committed: disabling the current target (the
    // node flag clears immediately, the runtime slot keeps it) and
    // presenting must not resurrect the pre-failure focus through a stale
    // record — the slot stays exactly where the user left it.
    node_a2.set_can_request_focus(false);
    present(&mut harness);
    assert_eq!(harness.runtime.focused_element(), Some(ea2));
}

fn two_focus_route_page(name: &str, first: &FocusNode, second: &FocusNode) -> Page {
    two_focus_route(name, first, second, RED)
}

#[test]
fn outlet_failed_attempt_causes_no_side_effects() {
    // A failed attempt disturbs nothing but the error return: the live
    // route's task completes normally through the failure (no cancellation
    // caused by it), the drive count advances exactly once per present
    // (no leaked duplicate builder), bindings stay exact, and popping the
    // offender plus presenting recovers fully with the original save.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let wake = Arc::new(TestWake::default());
    harness.runtime.set_wake_handler(wake.clone());
    let node_a = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    let ra = navigator.current().expect("route A").id;
    let scope_a = harness
        .outlet
        .borrow()
        .route_task_scope(ra)
        .expect("A bound");
    let completed = Arc::new(AtomicBool::new(false));
    let completed_for_completion = completed.clone();
    harness
        .runtime
        .spawner()
        .spawn_into_in(&scope_a, async { 7_u8 }, move |result, _| {
            assert_eq!(result.expect("failure never cancels"), 7_u8);
            completed_for_completion.store(true, Ordering::Release);
        });
    let drives = harness.outlet.borrow().frame_drive_count();
    navigator.push(Route::new(
        "bad",
        Column::new(vec![
            Widget::box_(Size::new(40., 40.), BLUE).with_key(7_u64),
            Widget::box_(Size::new(40., 40.), GREEN).with_key(7_u64),
        ]),
    ));
    let error = RouteOutlet::present_frame(
        &harness.outlet,
        &mut harness.runtime,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect_err("duplicate keys fail the rebuild");
    assert!(matches!(
        error,
        OutletError::Frame(TreeError::DuplicateKey { .. })
    ));
    // Exact fallout: one drive (begin ran, reconcile did not), A's binding
    // untouched, the offender unbound, focus unmoved, and the task still
    // completes through the real scheduler afterwards.
    assert_eq!(
        harness.outlet.borrow().frame_drive_count(),
        drives + 1,
        "a failed present drives exactly once"
    );
    let rb = navigator.current().expect("bad route pushed").id;
    assert!(harness.outlet.borrow().route_task_scope(ra).is_some());
    assert!(harness.outlet.borrow().route_task_scope(rb).is_none());
    assert!(node_a.has_focus());
    assert!(!scope_a.is_cancelled());
    pump_until(&mut harness.runtime, &completed);
    // Recovery: pop, present, and the original save still restores —
    // repeat presents drive exactly once each, so no builder leaked.
    navigator.pop();
    present(&mut harness);
    assert_eq!(harness.outlet.borrow().frame_drive_count(), drives + 2);
    assert!(harness.outlet.borrow().route_task_scope(rb).is_none());
    navigator.push_page(focus_page("b", &node_a));
    present(&mut harness);
    navigator.pop();
    present(&mut harness);
    assert!(node_a.has_focus());
}

#[test]
fn outlet_frame_panic_retries_without_overwriting_save() {
    // A builder panic unwinds through the frame with the pending capture
    // intact: nothing commits, nothing binds, and the retry commits the
    // original save. (The panic hook prints "builder boom" even though the
    // harness catches it; the assertions below are what matter.)
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    let ra = navigator.current().expect("route A").id;
    let trip = Rc::new(Cell::new(true));
    let node_b_for_build = node_b.clone();
    navigator.push_page(Page::new(
        "b",
        Column::new(vec![Widget::from(LayoutBuilder::new(move |_, _| {
            if trip.take() {
                panic!("builder boom");
            }
            focus_widget(&node_b_for_build, Widget::box_(Size::new(40., 40.), BLUE))
        }))]),
    ));
    let rb = navigator.current().expect("route B").id;
    let outlet = harness.outlet.clone();
    let result = catch_unwind(AssertUnwindSafe(|| {
        RouteOutlet::present_frame(
            &outlet,
            &mut harness.runtime,
            Constraints::tight(Size::new(200., 200.)),
        )
    }));
    let error = result.expect_err("builder panic unwinds the frame");
    assert_eq!(error.downcast_ref::<&str>(), Some(&"builder boom"));
    // Exact post-failure state: the failed frame committed nothing — B is
    // alive but unbound — while established state holds.
    assert!(navigator.lifetime_of(rb).expect("B alive").is_live());
    assert!(harness.outlet.borrow().route_task_scope(rb).is_none());
    assert!(harness.outlet.borrow().route_task_scope(ra).is_some());
    // Recovery commits the kept capture: B binds, and the return restores
    // A's original save (only the kept capture could supply it — the retry
    // itself never re-saves, and no record existed before).
    present(&mut harness);
    assert!(harness.outlet.borrow().route_task_scope(rb).is_some());
    tab_until(&mut harness.runtime, &node_b);
    navigator.pop();
    present(&mut harness);
    assert!(node_a.has_focus());
}

#[test]
fn outlet_navigation_during_build_reconciles_mounted_snapshot() {
    // A builder that navigates mid-frame: the frame presents whatever it
    // mounted while the pending capture for the superseded target is
    // abandoned untouched, then the fresh transition commits normally.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    let trip = Rc::new(Cell::new(true));
    navigator.push_page(Page::new(
        "b",
        Column::new(vec![Widget::from(LayoutBuilder::new({
            let navigator = navigator.clone();
            move |_, _| {
                if trip.take() {
                    navigator.push_page(plain_page("c"));
                }
                focus_widget(&node_b, Widget::box_(Size::new(40., 40.), BLUE))
            }
        }))]),
    ));
    present(&mut harness);
    // Mounted A and B with active C: no restore ran for a route whose
    // content may not have mounted, and focus never moved.
    assert_eq!(navigator.current().expect("navigated").name, "c");
    assert!(node_a.has_focus());
    let rb = navigator.routes()[1].id;
    assert!(
        harness.outlet.borrow().route_task_scope(rb).is_some(),
        "live routes bind even when their activation is abandoned"
    );
    present(&mut harness);
    navigator.pop();
    assert_eq!(navigator.current().expect("route B").name, "b");
    present(&mut harness);
    navigator.pop();
    present(&mut harness);
    assert!(node_a.has_focus());
}

#[test]
fn outlet_removal_during_build_abandons_cleanly() {
    // A builder that pops its own route mid-frame: the pending capture is
    // abandoned, nothing commits for the removed route, and exact focus
    // and binding state holds.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    let ra = navigator.current().expect("route A").id;
    let trip = Rc::new(Cell::new(true));
    navigator.push_page(Page::new(
        "b",
        Column::new(vec![Widget::from(LayoutBuilder::new({
            let navigator = navigator.clone();
            move |_, _| {
                if trip.take() {
                    navigator.pop();
                }
                focus_widget(&node_b, Widget::box_(Size::new(40., 40.), BLUE))
            }
        }))]),
    ));
    let rb = navigator.current().expect("route B").id;
    present(&mut harness);
    assert_eq!(navigator.current().expect("back to A").id, ra);
    assert!(navigator.lifetime_of(rb).is_none());
    assert!(harness.outlet.borrow().route_task_scope(rb).is_none());
    assert!(node_a.has_focus());
    assert!(harness.outlet.borrow().route_task_scope(ra).is_some());
    // The outlet stays healthy: ordinary navigation afterwards works.
    navigator.push_page(plain_page("c"));
    present(&mut harness);
    navigator.pop();
    present(&mut harness);
    assert!(node_a.has_focus());
}

fn paints_focus_ring(commands: &[PaintCommand], color: Color) -> bool {
    commands.iter().any(
        |command| matches!(command, PaintCommand::Border { border, .. } if border.color == color),
    )
}

fn semantics_marks_focused(runtime: &Runtime, label: &str) -> bool {
    runtime
        .tree()
        .semantics_debug_dump()
        .lines()
        .any(|line| line.contains(label) && line.contains("focused=true"))
}

#[test]
fn outlet_restore_schedules_followup_for_stale_visuals() {
    // Frame contract: the presenting frame paints pre-restore focus while
    // the restore flags follow-up work explicitly — never silently stale.
    // Styling (focus ring), semantics (focused node), keyboard target
    // (traversal origin), and slot converge on the follow-up frame.
    use incular_widgets::internal::ActionSurface;
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_plain = FocusNode::new();
    let node_b = FocusNode::new();
    let surface_widget: Widget = ActionSurface::new("a-btn")
        .size(Size::new(40., 40.))
        .color(Color::TRANSPARENT)
        .focused_color(YELLOW)
        .into();
    navigator.push_page(Page::new(
        "a",
        Column::new(vec![surface_widget, focus_child(&node_plain)]),
    ));
    present(&mut harness);
    // Discover the surface through its ring (order-independent): tab until
    // the focused styling paints.
    for _ in 0..3 {
        let _ = harness
            .runtime
            .handle_input(InputEvent::Key(key_down(Code::Tab)));
        if paints_focus_ring(repaint(&mut harness).commands(), YELLOW) {
            break;
        }
    }
    let surface = harness.runtime.focused_element().expect("surface focused");
    assert!(paints_focus_ring(repaint(&mut harness).commands(), YELLOW));
    navigator.push_page(focus_page("b", &node_b));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_b);
    navigator.pop();
    // The restoring frame paints pre-restore focus (no ring yet) and flags
    // the follow-up explicitly.
    let stale = present(&mut harness);
    assert_eq!(harness.runtime.focused_element(), Some(surface));
    assert!(
        harness.runtime.frame_requested(),
        "restore schedules its follow-up"
    );
    assert!(
        !paints_focus_ring(stale.commands(), YELLOW),
        "presenting frame predates the restore"
    );
    assert!(!semantics_marks_focused(&harness.runtime, "a-btn"));
    // The follow-up frame finalizes every focus-dependent output with no
    // further navigation.
    let fresh = present(&mut harness);
    assert!(paints_focus_ring(fresh.commands(), YELLOW));
    assert!(semantics_marks_focused(&harness.runtime, "a-btn"));
    tab_until(&mut harness.runtime, &node_plain);
    assert!(harness.runtime.focused_element().is_some());
}

#[test]
fn outlet_covered_veil_deactivates_with_its_route() {
    // A retained covered modal keeps its content mounted but must not keep
    // an active veil: no veil paint, no veil hit interception, no veil
    // semantics while covered. Uncovering reactivates both together.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let taps_a = Rc::new(Cell::new(0_u32));
    let taps_c = Rc::new(Cell::new(0_u32));
    navigator.push(
        Route::new("a", tappable_rect(RED, 200., 200., &taps_a))
            .presentation(RoutePresentation::page()),
    );
    navigator.push(
        Route::new("b", Widget::box_(Size::new(40., 40.), BLUE)).modal(ModalBarrier {
            color: GREEN,
            ..Default::default()
        }),
    );
    navigator.push(Route::new("c", tappable_rect(RED, 200., 200., &taps_c)));
    present(&mut harness);
    let commands = repaint(&mut harness).commands().to_vec();
    assert!(paints(&commands, RED));
    assert!(!paints(&commands, GREEN), "covered veil paints nothing");
    assert!(!paints(&commands, BLUE));
    // Taps land on the cover itself: with the veil gone, nothing
    // intercepts above it (a present veil would absorb the tap and neither
    // counter would move).
    tap(&mut harness, 100., 100.);
    assert_eq!(taps_c.get(), 1);
    assert_eq!(taps_a.get(), 0);
    // Uncover: veil and content reactivate together.
    navigator.pop();
    present(&mut harness);
    let commands = repaint(&mut harness).commands().to_vec();
    eprintln!("UNCOVER DUMP: {commands:#?}");
    assert!(paints(&commands, GREEN));
    assert!(paints(&commands, BLUE));
}

#[test]
fn outlet_close_window_cancels_bindings_and_releases() {
    // Real Application::close_window through an adopted runtime: the
    // window-scope cancel cascades into route bindings through existing
    // ownership (no outlet-specific close path, no test-only accessor),
    // and teardown releases builders, registrations, records, and
    // bindings per policy — proven by the outlet actually dropping.
    let navigator = Navigator::new();
    let mut runtime =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    // Bind under the real window scope so close reaches the binding.
    let parent = runtime.window_task_scope();
    let outlet = Rc::new(RefCell::new(RouteOutlet::new(&navigator, &parent)));
    let outlet_for_build = outlet.clone();
    let root = runtime.tree().root().expect("root");
    runtime
        .register_builder(root, move || {
            Column::new(vec![SizedBox::from_dimensions(
                Some(200.),
                Some(200.),
                Some(outlet_for_build.borrow().widget()),
            )])
            .into()
        })
        .expect("builder registers");
    frame(&mut runtime);
    RouteOutlet::attach(&outlet, &mut runtime).expect("outer mounted");
    let node_a = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    RouteOutlet::present_frame(
        &outlet,
        &mut runtime,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect("present");
    tab_until(&mut runtime, &node_a);
    let id = navigator.current().expect("mounted").id;
    let scope = outlet.borrow().route_task_scope(id).expect("bound");
    assert!(!scope.is_cancelled());
    let weak_outlet = Rc::downgrade(&outlet);
    let mut app = Application::from_runtime(runtime, |_| {});
    app.close_window(app.primary_window());
    assert!(
        scope.is_cancelled(),
        "window close cascades through existing ownership"
    );
    // Teardown releases everything the outlet owned: builders (registered
    // closures), nested registrations, focus records, and bindings. The
    // weak handle dying proves no retainer survives close plus drop.
    drop(outlet);
    drop(app);
    assert!(
        weak_outlet.upgrade().is_none(),
        "outlet fully released after close and drop"
    );
    assert!(scope.is_cancelled());
}

#[test]
fn outlet_close_window_with_nested_tree_cancels_by_lifetime() {
    // Nested outlets under the real window scope: close cancels the outer
    // and inner bindings through task ownership — attachment plays no
    // role, so even a detached-but-retained inner binding cancels — and
    // teardown releases both outlets per policy.
    let outer = Navigator::new();
    let inner = Navigator::new();
    let mut runtime =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    // Bind under the real window scope so close reaches every binding.
    let parent = runtime.window_task_scope();
    let outlet_outer = Rc::new(RefCell::new(RouteOutlet::new(&outer, &parent)));
    let outlet_inner = Rc::new(RefCell::new(RouteOutlet::new(&inner, &parent)));
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    let outlet_inner_for_page = outlet_inner.clone();
    outer.push_page(Page::new(
        "a",
        Column::new(vec![
            focus_widget(&node_a, Widget::box_(Size::new(40., 40.), RED)),
            RouteOutlet::nested_widget(&outlet_inner_for_page),
        ]),
    ));
    // The composition helper cloned the handle into page content; the local
    // is surplus from here on and would read as a leak below.
    drop(outlet_inner_for_page);
    let outlet_for_build = outlet_outer.clone();
    let root = runtime.tree().root().expect("root");
    runtime
        .register_builder(root, move || {
            Column::new(vec![SizedBox::from_dimensions(
                Some(200.),
                Some(200.),
                Some(outlet_for_build.borrow().widget()),
            )])
            .into()
        })
        .expect("builder registers");
    frame(&mut runtime);
    RouteOutlet::attach(&outlet_outer, &mut runtime).expect("outer mounted");
    RouteOutlet::attach_nested(&outlet_outer, &outlet_inner).expect("nested attaches");
    inner.push_page(focus_page("b", &node_b));
    present_outer_tree(&mut runtime, &outlet_outer);
    tab_until(&mut runtime, &node_a);
    let oa = outer.current().expect("outer route").id;
    let ib = inner.current().expect("inner route").id;
    let outer_scope = outlet_outer
        .borrow()
        .route_task_scope(oa)
        .expect("outer bound");
    let inner_scope = outlet_inner
        .borrow()
        .route_task_scope(ib)
        .expect("inner bound");
    // Detach the inner outlet but retain it: the binding (lifetime-owned,
    // not attachment-owned) must still cancel on close.
    RouteOutlet::detach_nested(&outlet_outer, &outlet_inner);
    // The inner handle is additionally retained by outer route content
    // (the nested_widget closure), which is application state — not outlet
    // metadata — so replacing the outer page releases that retainer.
    // (Single-route stacks do not pop; replacement is the removal path.)
    outer
        .set_pages([Page::new("solo", Widget::box_(Size::new(40., 40.), BLUE))])
        .unwrap();
    inner
        .set_pages([Page::new("solo", Widget::box_(Size::new(40., 40.), BLUE))])
        .unwrap();
    present_outer_tree(&mut runtime, &outlet_outer);
    let weak_outer = Rc::downgrade(&outlet_outer);
    let weak_inner = Rc::downgrade(&outlet_inner);
    let mut app = Application::from_runtime(runtime, |_| {});
    app.close_window(app.primary_window());
    assert!(
        outer_scope.is_cancelled(),
        "window close cascades to the outer binding"
    );
    assert!(
        inner_scope.is_cancelled(),
        "window close cascades to the nested binding despite detachment"
    );
    drop(outer);
    drop(inner);
    drop(outlet_outer);
    drop(outlet_inner);
    drop(app);
    assert!(weak_outer.upgrade().is_none());
    assert!(weak_inner.upgrade().is_none());
}

#[test]
fn outlet_failed_attempt_output_and_followup_agree() {
    // A failed attempt returns the typed error with focus unmoved and no
    // restore behind it; the recovery present then returns output painting
    // the committed content, and the restoring return schedules exactly
    // the follow-up its restore needs — output and scheduling agree at
    // every step through the supported operation alone.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    let ea = harness.runtime.focused_element().expect("A focused");
    let trip = Rc::new(Cell::new(true));
    let node_b_for_build = node_b.clone();
    navigator.push_page(Page::new(
        "b",
        Column::new(vec![Widget::from(LayoutBuilder::new({
            let navigator = navigator.clone();
            move |_, _| {
                if trip.take() {
                    navigator.push(
                        Route::new("overlay", Widget::box_(Size::new(40., 40.), GREEN)).overlay(
                            vec![OverlayEntry::new(Widget::box_(Size::new(40., 40.), GREEN))],
                        ),
                    );
                }
                focus_widget(&node_b_for_build, Widget::box_(Size::new(40., 40.), BLUE))
            }
        }))]),
    ));
    // Failure: typed error, focus exactly where it was, offender unbound.
    let error = RouteOutlet::present_frame(
        &harness.outlet,
        &mut harness.runtime,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect_err("mid-build overlay fails explicitly");
    assert!(matches!(error, OutletError::UnsupportedPresentation { .. }));
    assert_eq!(harness.runtime.focused_element(), Some(ea));
    // Recovery commits the kept capture: the returned output paints the
    // incoming content it committed — output matches committed state.
    navigator.pop();
    let recovered = present(&mut harness);
    assert!(
        paints(recovered.commands(), BLUE),
        "recovery output presents the committed incoming route"
    );
    // The way back restores the original save and schedules its follow-up
    // explicitly; the follow-up frame holds steady with no new work.
    tab_until(&mut harness.runtime, &node_b);
    navigator.pop();
    present(&mut harness);
    assert_eq!(harness.runtime.focused_element(), Some(ea));
    assert!(
        harness.runtime.frame_requested(),
        "the restoring frame schedules its follow-up"
    );
    present(&mut harness);
    assert_eq!(harness.runtime.focused_element(), Some(ea));
}

#[test]
fn outlet_production_lifecycle_combined() {
    // One production story through the single supported lifecycle:
    // nested baseline, conflicting-driver rejection, mid-build
    // navigation, failure plus retry, then window closure — each phase
    // asserting exact integration state through public API only.
    // Bindings parent under the closing runtime's window scope, so the
    // closure phase cancels through existing ownership.
    let mut runtime1 =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    let parent_scope = runtime1.window_task_scope();
    let outer_nav = Navigator::new();
    let inner_nav = Navigator::new();
    let outlet_outer = Rc::new(RefCell::new(RouteOutlet::new(&outer_nav, &parent_scope)));
    let outlet_inner = Rc::new(RefCell::new(RouteOutlet::new(&inner_nav, &parent_scope)));
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    let outlet_inner_for_page = outlet_inner.clone();
    outer_nav.push_page(Page::new(
        "a",
        Column::new(vec![
            focus_widget(&node_a, Widget::box_(Size::new(40., 40.), RED)),
            RouteOutlet::nested_widget(&outlet_inner_for_page),
        ]),
    ));
    drop(outlet_inner_for_page);
    {
        let outlet_for_build = outlet_outer.clone();
        let root = runtime1.tree().root().expect("root");
        runtime1
            .register_builder(root, move || {
                Column::new(vec![SizedBox::from_dimensions(
                    Some(200.),
                    Some(200.),
                    Some(outlet_for_build.borrow().widget()),
                )])
                .into()
            })
            .expect("builder registers");
    }
    frame(&mut runtime1);
    RouteOutlet::attach(&outlet_outer, &mut runtime1).expect("outer attaches");
    RouteOutlet::attach_nested(&outlet_outer, &outlet_inner).expect("nested attaches");
    // Phase 1: nested baseline — both routes bound, outer focused.
    inner_nav.push_page(focus_page("b", &node_b));
    present_outer_tree(&mut runtime1, &outlet_outer);
    tab_until(&mut runtime1, &node_a);
    let oa = outer_nav.current().expect("outer route").id;
    let ib = inner_nav.current().expect("inner route").id;
    let outer_scope = outlet_outer
        .borrow()
        .route_task_scope(oa)
        .expect("outer bound");
    let inner_scope = outlet_inner
        .borrow()
        .route_task_scope(ib)
        .expect("inner bound");
    // Phase 2: conflicting driver rejected with identities, state frozen.
    let outer_drives = outlet_outer.borrow().frame_drive_count();
    let inner_drives = outlet_inner.borrow().frame_drive_count();
    let error = RouteOutlet::present_frame(
        &outlet_inner,
        &mut runtime1,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect_err("separate child driving is rejected");
    assert_eq!(
        error,
        OutletError::SeparateDrive {
            outlet: outlet_inner.borrow().id(),
            parent: outlet_outer.borrow().id(),
        }
    );
    assert_eq!(outlet_outer.borrow().frame_drive_count(), outer_drives);
    assert_eq!(outlet_inner.borrow().frame_drive_count(), inner_drives);
    assert!(node_a.has_focus());
    // Phase 3: mid-build navigation abandons cleanly — no restore, focus
    // unmoved, yet the live newcomer binds by lifetime.
    let trip_push = Rc::new(Cell::new(true));
    let node_bx = FocusNode::new();
    outer_nav.push_page(Page::new(
        "b",
        Column::new(vec![Widget::from(LayoutBuilder::new({
            let outer_nav = outer_nav.clone();
            move |_, _| {
                if trip_push.take() {
                    outer_nav.push_page(plain_page("c"));
                }
                focus_widget(&node_bx, Widget::box_(Size::new(40., 40.), BLUE))
            }
        }))]),
    ));
    present_outer_tree(&mut runtime1, &outlet_outer);
    assert!(node_a.has_focus());
    let ic = outer_nav.current().expect("pushed route").id;
    assert!(
        outlet_outer.borrow().route_task_scope(ic).is_some(),
        "abandon still binds the live newcomer"
    );
    assert!(
        outlet_outer.borrow().needs_frame(),
        "the mid-build push still requires a frame"
    );
    outer_nav.pop();
    outer_nav.pop();
    present_outer_tree(&mut runtime1, &outlet_outer);
    assert!(node_a.has_focus());
    assert!(!outlet_outer.borrow().needs_frame());
    // Phase 4: failure plus retry commits and restores end to end.
    let trip_panic = Rc::new(Cell::new(true));
    let node_d = FocusNode::new();
    let node_d_for_build = node_d.clone();
    outer_nav.push_page(Page::new(
        "d",
        Column::new(vec![Widget::from(LayoutBuilder::new(move |_, _| {
            if trip_panic.take() {
                panic!("combined boom");
            }
            focus_widget(&node_d_for_build, Widget::box_(Size::new(40., 40.), GREEN))
        }))]),
    ));
    let outlet = outlet_outer.clone();
    let result = catch_unwind(AssertUnwindSafe(|| {
        RouteOutlet::present_frame(
            &outlet,
            &mut runtime1,
            Constraints::tight(Size::new(200., 200.)),
        )
    }));
    result.expect_err("builder panic unwinds the frame");
    drop(outlet);
    present_outer_tree(&mut runtime1, &outlet_outer);
    tab_until(&mut runtime1, &node_d);
    outer_nav.pop();
    present_outer_tree(&mut runtime1, &outlet_outer);
    assert!(node_a.has_focus());
    // Phase 5: window closure cancels by lifetime; teardown releases.
    let weak_outer = Rc::downgrade(&outlet_outer);
    let weak_inner = Rc::downgrade(&outlet_inner);
    let mut app = Application::from_runtime(runtime1, |_| {});
    app.close_window(app.primary_window());
    assert!(outer_scope.is_cancelled());
    assert!(inner_scope.is_cancelled());
    outer_nav
        .set_pages([Page::new("solo", Widget::box_(Size::new(40., 40.), BLUE))])
        .unwrap();
    inner_nav
        .set_pages([Page::new("solo", Widget::box_(Size::new(40., 40.), BLUE))])
        .unwrap();
    drop(outer_nav);
    drop(inner_nav);
    drop(outlet_outer);
    drop(outlet_inner);
    drop(app);
    assert!(weak_outer.upgrade().is_none());
    assert!(weak_inner.upgrade().is_none());
    let _ = node_b;
}

#[test]
fn outlet_combined_receipt_scheduling_closeout() {
    // One flow through every contract: speculative compositions
    // discarded, skipped nested composition, failure plus retry,
    // navigation changes, and teardown — asserting output paint, focus,
    // task lifetime, and scheduler state together through public API.
    let mut runtime1 =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    let parent_scope = runtime1.window_task_scope();
    let outer_nav = Navigator::new();
    let inner_nav = Navigator::new();
    let outlet_outer = Rc::new(RefCell::new(RouteOutlet::new(&outer_nav, &parent_scope)));
    let outlet_inner = Rc::new(RefCell::new(RouteOutlet::new(&inner_nav, &parent_scope)));
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    let node_d = FocusNode::new();
    let outlet_inner_for_page = outlet_inner.clone();
    outer_nav.push_page(Page::new(
        "a",
        Column::new(vec![
            focus_widget(&node_a, Widget::box_(Size::new(40., 40.), RED)),
            RouteOutlet::nested_widget(&outlet_inner_for_page),
        ]),
    ));
    drop(outlet_inner_for_page);
    {
        let outlet_for_build = outlet_outer.clone();
        let root = runtime1.tree().root().expect("root");
        runtime1
            .register_builder(root, move || {
                Column::new(vec![SizedBox::from_dimensions(
                    Some(200.),
                    Some(200.),
                    Some(outlet_for_build.borrow().widget()),
                )])
                .into()
            })
            .expect("builder registers");
    }
    frame(&mut runtime1);
    RouteOutlet::attach(&outlet_outer, &mut runtime1).expect("outer attaches");
    RouteOutlet::attach_nested(&outlet_outer, &outlet_inner).expect("nested attaches");
    // Baseline: both bound, A focused, tree idle.
    inner_nav.push_page(Page::new(
        "b",
        focus_widget(&node_b, Widget::box_(Size::new(40., 40.), BLUE)),
    ));
    present_outer_tree(&mut runtime1, &outlet_outer);
    tab_until(&mut runtime1, &node_a);
    let ib = inner_nav.current().expect("inner route").id;
    let inner_scope = outlet_inner
        .borrow()
        .route_task_scope(ib)
        .expect("inner bound");
    present_outer_tree(&mut runtime1, &outlet_outer);
    assert!(!runtime1.frame_requested(), "quiet tree idles");
    // Speculative compositions discarded: a quiet present converges
    // without authorizing them, painting current content only.
    let _ = outlet_outer.borrow().widget();
    let _ = outlet_inner.borrow().widget();
    let quiet = present_outer_tree_output(&mut runtime1, &outlet_outer);
    assert!(paints(quiet.commands(), RED));
    assert!(paints(quiet.commands(), BLUE));
    assert!(!runtime1.frame_requested());
    assert!(node_a.has_focus());
    // Skipped nested composition: solo outer, inner advancing slotless.
    // The inner revision stays outstanding and schedules once.
    outer_nav
        .set_pages([Page::new(
            "solo",
            Widget::box_(Size::new(200., 200.), GREEN),
        )])
        .unwrap();
    inner_nav.push_page(Page::new(
        "d",
        focus_widget(&node_d, Widget::box_(Size::new(40., 40.), YELLOW)),
    ));
    present_outer_tree(&mut runtime1, &outlet_outer);
    assert_eq!(runtime1.focused_element(), None);
    assert!(outlet_inner.borrow().needs_frame());
    assert!(
        runtime1.frame_requested(),
        "outstanding nested work schedules once"
    );
    // Failure plus retry: a bad outer route errors (resetting the memo),
    // dropping it converges with the inner revision still outstanding —
    // and scheduling again for exactly that revision.
    outer_nav.push(Route::new(
        "bad",
        Column::new(vec![
            Widget::box_(Size::new(40., 40.), BLUE).with_key(7_u64),
            Widget::box_(Size::new(40., 40.), GREEN).with_key(7_u64),
        ]),
    ));
    RouteOutlet::present_frame(
        &outlet_outer,
        &mut runtime1,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect_err("duplicate keys fail the rebuild");
    outer_nav
        .set_pages([Page::new(
            "solo",
            Widget::box_(Size::new(200., 200.), GREEN),
        )])
        .unwrap();
    present_outer_tree(&mut runtime1, &outlet_outer);
    assert!(outlet_inner.borrow().needs_frame());
    assert!(
        runtime1.frame_requested(),
        "reset work schedules again after failure"
    );
    assert!(inner_nav.lifetime_of(ib).expect("B alive").is_live());
    assert!(!inner_scope.is_cancelled());
    // Navigation changes: remount consumes the pending revision, paints
    // it, idles, and keeps every lifetime bound.
    let outlet_inner_for_page = outlet_inner.clone();
    outer_nav
        .set_pages([Page::new(
            "a2",
            Column::new(vec![
                focus_widget(&node_a, Widget::box_(Size::new(40., 40.), RED)),
                RouteOutlet::nested_widget(&outlet_inner_for_page),
            ]),
        )])
        .unwrap();
    drop(outlet_inner_for_page);
    RouteOutlet::attach_nested(&outlet_outer, &outlet_inner).expect("nested attaches");
    let remounted = present_outer_tree_output(&mut runtime1, &outlet_outer);
    assert!(paints(remounted.commands(), RED));
    assert!(paints(remounted.commands(), YELLOW));
    assert!(!outlet_inner.borrow().needs_frame());
    assert!(!runtime1.frame_requested(), "consumed work idles");
    let oa2 = outer_nav.current().expect("outer route").id;
    let id = inner_nav.current().expect("inner route").id;
    let outer_scope = outlet_outer
        .borrow()
        .route_task_scope(oa2)
        .expect("outer bound");
    let inner_d_scope = outlet_inner
        .borrow()
        .route_task_scope(id)
        .expect("inner bound");
    tab_until(&mut runtime1, &node_d);
    inner_nav.pop();
    present_outer_tree(&mut runtime1, &outlet_outer);
    tab_until(&mut runtime1, &node_b);
    // Teardown: window closure cancels by lifetime, then everything
    // releases.
    let weak_outer = Rc::downgrade(&outlet_outer);
    let weak_inner = Rc::downgrade(&outlet_inner);
    let mut app = Application::from_runtime(runtime1, |_| {});
    app.close_window(app.primary_window());
    assert!(outer_scope.is_cancelled());
    assert!(inner_d_scope.is_cancelled());
    assert!(inner_scope.is_cancelled());
    outer_nav
        .set_pages([Page::new("solo", Widget::box_(Size::new(40., 40.), BLUE))])
        .unwrap();
    inner_nav
        .set_pages([Page::new("solo", Widget::box_(Size::new(40., 40.), BLUE))])
        .unwrap();
    drop(outer_nav);
    drop(inner_nav);
    drop(outlet_outer);
    drop(outlet_inner);
    drop(app);
    assert!(weak_outer.upgrade().is_none());
    assert!(weak_inner.upgrade().is_none());
}

#[test]
fn outlet_dropped_bindings_ignore_later_removal() {
    // Bindings detach with the outlet, independent of mounting: bind
    // through bookkeeping-only presents (no attach, no mounted content),
    // drop the outlet, then remove — the scope survives because the
    // removal callback went with the binding. All through present_frame.
    let navigator = Navigator::new();
    let mut runtime =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    let parent = runtime.spawner().scope();
    let outlet = Rc::new(RefCell::new(RouteOutlet::new(&navigator, &parent)));
    navigator.push_page(plain_page("task"));
    RouteOutlet::present_frame(
        &outlet,
        &mut runtime,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect("present binds live routes");
    let id = navigator.current().expect("mounted").id;
    let scope = outlet.borrow().route_task_scope(id).expect("bound");
    assert!(!scope.is_cancelled());
    let weak = Rc::downgrade(&outlet);
    drop(outlet);
    assert!(weak.upgrade().is_none());
    // Later removal ends the lifetime, but no binding remains to cancel
    // through: the scope outlives its route's removal by policy.
    navigator.pop();
    assert!(!scope.is_cancelled());
}

/// Mounts `outlet`'s widget in a fresh runtime (framed, not attached) —
/// the second driver domain for ownership-conflict tests. Attaching and
/// presenting stay explicit per test: they are the rejection points.
fn second_driver(outlet: &Rc<RefCell<RouteOutlet>>) -> Runtime {
    let mut runtime =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    let outlet_for_build = outlet.clone();
    let root = runtime.tree().root().expect("root");
    runtime
        .register_builder(root, move || {
            Column::new(vec![SizedBox::from_dimensions(
                Some(200.),
                Some(200.),
                Some(outlet_for_build.borrow().widget()),
            )])
            .into()
        })
        .expect("builder registers");
    frame(&mut runtime);
    runtime
}

#[test]
fn outlet_child_driven_separately_rejected() {
    // A nested child presented directly — instead of through its root's
    // cascade — is rejected with both identities before any mutation: no
    // drives, no capture, focus and bindings exactly intact.
    let (outer, inner, mut runtime, outlet_outer, outlet_inner, node_a, node_b) =
        nested_inside_setup();
    inner.push_page(focus_page("b", &node_b));
    present_outer_tree(&mut runtime, &outlet_outer);
    tab_until(&mut runtime, &node_a);
    let outer_drives = outlet_outer.borrow().frame_drive_count();
    let inner_drives = outlet_inner.borrow().frame_drive_count();
    let ib = inner.current().expect("inner route").id;
    assert!(
        outlet_inner.borrow().route_task_scope(ib).is_some(),
        "inner bound before the rejected drive"
    );
    let error = RouteOutlet::present_frame(
        &outlet_inner,
        &mut runtime,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect_err("separate child driving is rejected");
    assert_eq!(
        error,
        OutletError::SeparateDrive {
            outlet: outlet_inner.borrow().id(),
            parent: outlet_outer.borrow().id(),
        }
    );
    assert_eq!(outlet_outer.borrow().frame_drive_count(), outer_drives);
    assert_eq!(outlet_inner.borrow().frame_drive_count(), inner_drives);
    assert!(node_a.has_focus());
    assert!(
        outlet_inner.borrow().route_task_scope(ib).is_some(),
        "rejection commits nothing and binds nothing new"
    );
    // The supported path still works end to end afterwards.
    present_outer_tree(&mut runtime, &outlet_outer);
    assert!(node_a.has_focus());
    let _ = outer;
}

#[test]
fn outlet_second_runtime_rejected_before_mutation() {
    // One outlet, two live runtimes: attaching or presenting through the
    // second fails with the owning and attempted runtime ids, drives
    // nothing, and leaves the first runtime driving undisturbed.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    let first_id = harness.runtime.id();
    let drives = harness.outlet.borrow().frame_drive_count();
    let outlet_id = harness.outlet.borrow().id();
    let mut runtime2 = second_driver(&harness.outlet);
    let second_id = runtime2.id();
    assert_ne!(first_id, second_id);
    // Attaching to the second live runtime fails before registering
    // anything.
    let error = RouteOutlet::attach(&harness.outlet, &mut runtime2)
        .expect_err("second-runtime attach is rejected");
    assert_eq!(
        error,
        OutletError::DriverConflict {
            outlet: outlet_id,
            owner: first_id,
            attempted: second_id,
        }
    );
    // Presenting through the second runtime fails before any mutation.
    let error = RouteOutlet::present_frame(
        &harness.outlet,
        &mut runtime2,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect_err("second-runtime present is rejected");
    assert_eq!(
        error,
        OutletError::DriverConflict {
            outlet: outlet_id,
            owner: first_id,
            attempted: second_id,
        }
    );
    assert_eq!(harness.outlet.borrow().frame_drive_count(), drives);
    assert!(node_a.has_focus());
    // The first runtime drives on undisturbed.
    present(&mut harness);
    assert_eq!(harness.outlet.borrow().frame_drive_count(), drives + 1);
    assert!(node_a.has_focus());
}

#[test]
fn outlet_cross_runtime_nesting_rejected() {
    // A child owned by one live runtime, attached under a parent mounted
    // in another: presenting the parent fails naming the child and its
    // owner before anything drives or commits.
    let holder =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    let parent_scope = holder.spawner().scope();
    let inner_nav = Navigator::new();
    let outlet_inner = Rc::new(RefCell::new(RouteOutlet::new(&inner_nav, &parent_scope)));
    let mut runtime1 = second_driver(&outlet_inner);
    RouteOutlet::attach(&outlet_inner, &mut runtime1).expect("rt1 attaches");
    let node_b = FocusNode::new();
    inner_nav.push_page(focus_page("b", &node_b));
    RouteOutlet::present_frame(
        &outlet_inner,
        &mut runtime1,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect("rt1 presents");
    let first_id = runtime1.id();
    let inner_id = outlet_inner.borrow().id();
    let inner_drives = outlet_inner.borrow().frame_drive_count();
    // Parent in a second runtime, wired to the foreign child.
    let outer_nav = Navigator::new();
    let outlet_outer = Rc::new(RefCell::new(RouteOutlet::new(&outer_nav, &parent_scope)));
    let outlet_inner_for_page = outlet_inner.clone();
    outer_nav.push_page(Page::new(
        "a",
        Column::new(vec![RouteOutlet::nested_widget(&outlet_inner_for_page)]),
    ));
    drop(outlet_inner_for_page);
    let mut runtime2 = second_driver(&outlet_outer);
    RouteOutlet::attach(&outlet_outer, &mut runtime2).expect("rt2 attaches");
    RouteOutlet::attach_nested(&outlet_outer, &outlet_inner).expect("topology attaches");
    let second_id = runtime2.id();
    let outer_drives = outlet_outer.borrow().frame_drive_count();
    let error = RouteOutlet::present_frame(
        &outlet_outer,
        &mut runtime2,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect_err("cross-runtime nesting is rejected");
    assert_eq!(
        error,
        OutletError::DriverConflict {
            outlet: inner_id,
            owner: first_id,
            attempted: second_id,
        }
    );
    assert_eq!(outlet_outer.borrow().frame_drive_count(), outer_drives);
    assert_eq!(outlet_inner.borrow().frame_drive_count(), inner_drives);
    // Detaching releases the child: the parent then presents cleanly in
    // its own runtime.
    RouteOutlet::detach_nested(&outlet_outer, &outlet_inner);
    RouteOutlet::present_frame(
        &outlet_outer,
        &mut runtime2,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect("parent presents after detach");
    let _ = holder;
}

#[test]
fn outlet_detach_transfers_driver_ownership() {
    // A child driven under one runtime detaches and moves to another:
    // attach plus present claim the new domain, and the old domain
    // rejects it afterwards — while the old parent keeps presenting
    // without it.
    let holder =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    let parent_scope = holder.spawner().scope();
    let outer_nav = Navigator::new();
    let inner_nav = Navigator::new();
    let outlet_outer = Rc::new(RefCell::new(RouteOutlet::new(&outer_nav, &parent_scope)));
    let outlet_inner = Rc::new(RefCell::new(RouteOutlet::new(&inner_nav, &parent_scope)));
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    let outlet_inner_for_page = outlet_inner.clone();
    outer_nav.push_page(Page::new(
        "a",
        Column::new(vec![
            focus_widget(&node_a, Widget::box_(Size::new(40., 40.), RED)),
            RouteOutlet::nested_widget(&outlet_inner_for_page),
        ]),
    ));
    drop(outlet_inner_for_page);
    let mut runtime1 = second_driver(&outlet_outer);
    RouteOutlet::attach(&outlet_outer, &mut runtime1).expect("rt1 attaches");
    RouteOutlet::attach_nested(&outlet_outer, &outlet_inner).expect("nested attaches");
    inner_nav.push_page(focus_page("b", &node_b));
    present_outer_tree(&mut runtime1, &outlet_outer);
    tab_until(&mut runtime1, &node_a);
    let first_id = runtime1.id();
    // Detach, then move the child to a second runtime wholesale.
    RouteOutlet::detach_nested(&outlet_outer, &outlet_inner);
    let mut runtime2 = second_driver(&outlet_inner);
    RouteOutlet::attach(&outlet_inner, &mut runtime2).expect("rt2 attaches after detach");
    RouteOutlet::present_frame(
        &outlet_inner,
        &mut runtime2,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect("rt2 presents after detach");
    tab_until(&mut runtime2, &node_b);
    let second_id = runtime2.id();
    // The old domain no longer drives it.
    let error = RouteOutlet::present_frame(
        &outlet_inner,
        &mut runtime1,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect_err("old runtime loses the transferred child");
    assert_eq!(
        error,
        OutletError::DriverConflict {
            outlet: outlet_inner.borrow().id(),
            owner: second_id,
            attempted: first_id,
        }
    );
    // The old parent presents on without the child.
    present_outer_tree(&mut runtime1, &outlet_outer);
    assert!(node_a.has_focus());
    let _ = holder;
}

#[test]
fn outlet_teardown_releases_driver_claim() {
    // Teardown releases ownership without any registry: dropping the
    // first runtime lets a second runtime attach and present the same
    // outlet end to end. Task scopes live under a surviving holder, so
    // only the driver domain moves.
    let holder =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    let parent_scope = holder.spawner().scope();
    let navigator = Navigator::new();
    let outlet = Rc::new(RefCell::new(RouteOutlet::new(&navigator, &parent_scope)));
    let node_a = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    let mut runtime1 = second_driver(&outlet);
    RouteOutlet::attach(&outlet, &mut runtime1).expect("rt1 attaches");
    RouteOutlet::present_frame(
        &outlet,
        &mut runtime1,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect("rt1 presents");
    tab_until(&mut runtime1, &node_a);
    let first_id = runtime1.id();
    drop(runtime1);
    let mut runtime2 = second_driver(&outlet);
    assert_ne!(runtime2.id(), first_id);
    RouteOutlet::attach(&outlet, &mut runtime2).expect("teardown released the claim");
    RouteOutlet::present_frame(
        &outlet,
        &mut runtime2,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect("rt2 presents after teardown");
    tab_until(&mut runtime2, &node_a);
    assert!(node_a.has_focus());
    let _ = holder;
}

/// Topology-only outlets: no mounting, no frames — attachment policy is
/// pure registration. The runtime stays alive so task scopes stay valid.
fn topology_outlets(count: usize) -> (Runtime, Vec<Rc<RefCell<RouteOutlet>>>) {
    let runtime =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    let parent = runtime.spawner().scope();
    let navigators: Vec<Navigator> = (0..count).map(|_| Navigator::new()).collect();
    let outlets = navigators
        .iter()
        .map(|navigator| Rc::new(RefCell::new(RouteOutlet::new(navigator, &parent))))
        .collect();
    (runtime, outlets)
}

#[test]
fn nested_self_attachment_rejected_before_mutation() {
    let (_runtime, outlets) = topology_outlets(1);
    assert_eq!(
        RouteOutlet::attach_nested(&outlets[0], &outlets[0]),
        Err(OutletAttachError::SelfAttachment)
    );
    assert_eq!(outlets[0].borrow().attached_nested_count(), 0);
}

#[test]
fn nested_two_node_cycle_rejected_before_mutation() {
    let (_runtime, outlets) = topology_outlets(2);
    RouteOutlet::attach_nested(&outlets[0], &outlets[1]).expect("first edge attaches");
    assert_eq!(
        RouteOutlet::attach_nested(&outlets[1], &outlets[0]),
        Err(OutletAttachError::Cycle)
    );
    // The failed edge changed nothing: A still parents B, B parents none.
    assert_eq!(outlets[0].borrow().attached_nested_count(), 1);
    assert_eq!(outlets[1].borrow().attached_nested_count(), 0);
}

#[test]
fn nested_longer_cycle_rejected_before_mutation() {
    let (_runtime, outlets) = topology_outlets(4);
    RouteOutlet::attach_nested(&outlets[0], &outlets[1]).expect("a attaches");
    RouteOutlet::attach_nested(&outlets[1], &outlets[2]).expect("b attaches");
    RouteOutlet::attach_nested(&outlets[2], &outlets[3]).expect("c attaches");
    // Closing A→B→C→D→A: D's child A already reaches D.
    assert_eq!(
        RouteOutlet::attach_nested(&outlets[3], &outlets[0]),
        Err(OutletAttachError::Cycle)
    );
    // A non-cyclic edge elsewhere still works after the rejection.
    assert_eq!(outlets[3].borrow().attached_nested_count(), 0);
}

#[test]
fn nested_duplicate_attachment_is_idempotent() {
    let (_runtime, outlets) = topology_outlets(2);
    RouteOutlet::attach_nested(&outlets[0], &outlets[1]).expect("first attaches");
    RouteOutlet::attach_nested(&outlets[0], &outlets[1]).expect("duplicate is a no-op");
    assert_eq!(outlets[0].borrow().attached_nested_count(), 1);
}

#[test]
fn nested_second_parent_rejected_until_explicit_detach() {
    // Single-parent policy: cascade driving visits every registered child,
    // so a shared child would drive twice per frame. Moving parents takes
    // an explicit detach first.
    let (_runtime, outlets) = topology_outlets(3);
    RouteOutlet::attach_nested(&outlets[0], &outlets[2]).expect("first parent attaches");
    assert_eq!(
        RouteOutlet::attach_nested(&outlets[1], &outlets[2]),
        Err(OutletAttachError::AlreadyAttached)
    );
    assert_eq!(outlets[1].borrow().attached_nested_count(), 0);
    assert_eq!(outlets[0].borrow().attached_nested_count(), 1);
    RouteOutlet::detach_nested(&outlets[0], &outlets[2]);
    assert_eq!(outlets[0].borrow().attached_nested_count(), 0);
    RouteOutlet::attach_nested(&outlets[1], &outlets[2]).expect("reattach after detach");
    assert_eq!(outlets[1].borrow().attached_nested_count(), 1);
}

#[test]
fn nested_detached_child_gets_no_work_while_retained() {
    // Detach (not drop) the inner outlet but keep its handle: the outer
    // cascade must skip it — no drive count, no focus interference — while
    // the outer outlet keeps working.
    let (outer, inner, mut runtime, outlet_outer, outlet_inner, node_a, node_b) =
        nested_inside_setup();
    inner.push_page(focus_page("b", &node_b));
    present_outer_tree(&mut runtime, &outlet_outer);
    tab_until(&mut runtime, &node_b);
    let outer_drives = outlet_outer.borrow().frame_drive_count();
    let inner_drives = outlet_inner.borrow().frame_drive_count();
    assert!(inner_drives > 0);
    RouteOutlet::detach_nested(&outlet_outer, &outlet_inner);
    assert_eq!(outlet_outer.borrow().attached_nested_count(), 0);
    // Retained handle, live inner navigator — yet the cascade skips it.
    let node_c = FocusNode::new();
    outer.push_page(focus_page("ob", &node_c));
    present_outer_tree(&mut runtime, &outlet_outer);
    assert_eq!(outlet_outer.borrow().frame_drive_count(), outer_drives + 1);
    assert_eq!(
        outlet_inner.borrow().frame_drive_count(),
        inner_drives,
        "detached outlet receives no frame work while retained"
    );
    tab_until(&mut runtime, &node_c);
    outer.pop();
    present_outer_tree(&mut runtime, &outlet_outer);
    assert_eq!(outlet_inner.borrow().frame_drive_count(), inner_drives);
    // Reattachment resumes the cascade exactly where it left off.
    RouteOutlet::attach_nested(&outlet_outer, &outlet_inner).expect("reattaches");
    present_outer_tree(&mut runtime, &outlet_outer);
    assert_eq!(outlet_inner.borrow().frame_drive_count(), inner_drives + 1);
    let _ = (outer, inner, node_a);
}

#[test]
fn nested_tree_drives_each_outlet_exactly_once() {
    // Three levels through the cascade: one outer present advances every
    // participant by exactly one drive — the acyclic single-parent
    // topology makes double-driving structurally impossible.
    let outer = Navigator::new();
    let middle_nav = Navigator::new();
    let inner_nav = Navigator::new();
    let mut runtime =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    let parent = runtime.spawner().scope();
    let outlet_outer = Rc::new(RefCell::new(RouteOutlet::new(&outer, &parent)));
    let outlet_middle = Rc::new(RefCell::new(RouteOutlet::new(&middle_nav, &parent)));
    let outlet_inner = Rc::new(RefCell::new(RouteOutlet::new(&inner_nav, &parent)));
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    let middle_for_page = outlet_middle.clone();
    let inner_for_page = outlet_inner.clone();
    middle_nav.push_page(Page::new(
        "m",
        Column::new(vec![
            focus_widget(&node_b, Widget::box_(Size::new(40., 40.), BLUE)),
            RouteOutlet::nested_widget(&inner_for_page),
        ]),
    ));
    outer.push_page(Page::new(
        "a",
        Column::new(vec![
            focus_widget(&node_a, Widget::box_(Size::new(40., 40.), RED)),
            RouteOutlet::nested_widget(&middle_for_page),
        ]),
    ));
    let root = runtime.tree().root().expect("root");
    {
        let outlet_outer = outlet_outer.clone();
        runtime
            .register_builder(root, move || {
                Column::new(vec![SizedBox::from_dimensions(
                    Some(200.),
                    Some(200.),
                    Some(outlet_outer.borrow().widget()),
                )])
                .into()
            })
            .expect("builder registers");
    }
    frame(&mut runtime);
    RouteOutlet::attach(&outlet_outer, &mut runtime).expect("outer mounted");
    RouteOutlet::attach_nested(&outlet_outer, &outlet_middle).expect("middle attaches");
    RouteOutlet::attach_nested(&outlet_middle, &outlet_inner).expect("inner attaches");
    for _ in 0..3 {
        let before = [
            outlet_outer.borrow().frame_drive_count(),
            outlet_middle.borrow().frame_drive_count(),
            outlet_inner.borrow().frame_drive_count(),
        ];
        present_outer_tree(&mut runtime, &outlet_outer);
        assert_eq!(outlet_outer.borrow().frame_drive_count(), before[0] + 1);
        assert_eq!(outlet_middle.borrow().frame_drive_count(), before[1] + 1);
        assert_eq!(outlet_inner.borrow().frame_drive_count(), before[2] + 1);
    }
    // The whole tree still works end to end: inner content is live.
    tab_until(&mut runtime, &node_b);
}

#[test]
fn nested_unsupported_rejected_with_outlet_identity() {
    // The inner navigator hosts the offending route: the error names the
    // inner outlet (not the driving root) with the route, and no outlet's
    // integration state moves — no drives, no captures, no bindings.
    let (outer, inner, mut runtime, outlet_outer, outlet_inner, node_a, node_b) =
        nested_inside_setup();
    inner.push_page(focus_page("b", &node_b));
    present_outer_tree(&mut runtime, &outlet_outer);
    tab_until(&mut runtime, &node_a);
    let outer_drives = outlet_outer.borrow().frame_drive_count();
    let inner_drives = outlet_inner.borrow().frame_drive_count();
    let outer_id = outer.current().expect("outer route").id;
    let outer_scope = outlet_outer
        .borrow()
        .route_task_scope(outer_id)
        .expect("outer bound");
    inner.push(
        Route::new("overlay", Widget::box_(Size::new(40., 40.), GREEN)).overlay(vec![
            OverlayEntry::new(Widget::box_(Size::new(40., 40.), GREEN)),
        ]),
    );
    let overlay_id = inner.current().expect("overlay route").id;
    let inner_id = outlet_inner.borrow().id();
    let error = RouteOutlet::present_frame(
        &outlet_outer,
        &mut runtime,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect_err("nested overlay stacks are rejected");
    assert_eq!(
        error,
        OutletError::UnsupportedPresentation {
            outlet: inner_id,
            routes: vec![overlay_id]
        }
    );
    // Nothing moved: no outlet drove, outer focus and binding hold, and
    // the offending route never bound.
    assert_eq!(outlet_outer.borrow().frame_drive_count(), outer_drives);
    assert_eq!(outlet_inner.borrow().frame_drive_count(), inner_drives);
    assert!(node_a.has_focus());
    assert!(!outer_scope.is_cancelled());
    assert!(outlet_inner.borrow().route_task_scope(overlay_id).is_none());
    // Recovery: removing the offending route restores the whole tree.
    inner.pop();
    present_outer_tree(&mut runtime, &outlet_outer);
    tab_until(&mut runtime, &node_b);
    let _ = outer;
}

#[test]
fn nested_valid_siblings_present_and_recover_independently() {
    // Two nested outlets under one outer: both valid present together;
    // one turning unsupported rejects with its own identity while the
    // sibling's state holds; fixing it recovers everything.
    let outer = Navigator::new();
    let first_nav = Navigator::new();
    let second_nav = Navigator::new();
    let mut runtime =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    let parent = runtime.spawner().scope();
    let outlet_outer = Rc::new(RefCell::new(RouteOutlet::new(&outer, &parent)));
    let outlet_first = Rc::new(RefCell::new(RouteOutlet::new(&first_nav, &parent)));
    let outlet_second = Rc::new(RefCell::new(RouteOutlet::new(&second_nav, &parent)));
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    let first_for_page = outlet_first.clone();
    let second_for_page = outlet_second.clone();
    outer.push_page(Page::new(
        "a",
        Column::new(vec![
            focus_widget(&node_a, Widget::box_(Size::new(40., 40.), RED)),
            RouteOutlet::nested_widget(&first_for_page),
            RouteOutlet::nested_widget(&second_for_page),
        ]),
    ));
    let root = runtime.tree().root().expect("root");
    {
        let outlet_outer = outlet_outer.clone();
        runtime
            .register_builder(root, move || {
                Column::new(vec![SizedBox::from_dimensions(
                    Some(200.),
                    Some(200.),
                    Some(outlet_outer.borrow().widget()),
                )])
                .into()
            })
            .expect("builder registers");
    }
    frame(&mut runtime);
    RouteOutlet::attach(&outlet_outer, &mut runtime).expect("outer mounted");
    RouteOutlet::attach_nested(&outlet_outer, &outlet_first).expect("first attaches");
    RouteOutlet::attach_nested(&outlet_outer, &outlet_second).expect("second attaches");
    first_nav.push_page(focus_page("b", &node_b));
    second_nav.push_page(plain_page("c"));
    present_outer_tree(&mut runtime, &outlet_outer);
    tab_until(&mut runtime, &node_b);
    // The second sibling turns unsupported: the error names it, the first
    // sibling's focus and bindings hold, and nothing drove.
    second_nav.push(
        Route::new("overlay", Widget::box_(Size::new(40., 40.), GREEN)).overlay(vec![
            OverlayEntry::new(Widget::box_(Size::new(40., 40.), GREEN)),
        ]),
    );
    let overlay_id = second_nav.current().expect("overlay route").id;
    let second_id = outlet_second.borrow().id();
    let error = RouteOutlet::present_frame(
        &outlet_outer,
        &mut runtime,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect_err("sibling overlay stacks are rejected");
    assert_eq!(
        error,
        OutletError::UnsupportedPresentation {
            outlet: second_id,
            routes: vec![overlay_id]
        }
    );
    assert!(node_b.has_focus());
    // Recovery is per-navigator: popping the offender restores the tree.
    second_nav.pop();
    present_outer_tree(&mut runtime, &outlet_outer);
    assert!(node_b.has_focus());
}

/// Drives one A→B→A round trip and asserts the return restores A's focus:
/// proves the A→B frame committed (an abandoned capture restores nothing).
fn round_trip_restores(
    node_a: &FocusNode,
    node_b: &FocusNode,
    navigator: &Navigator,
    harness: &mut OutletHarness,
) {
    tab_until(&mut harness.runtime, node_b);
    navigator.pop();
    present(harness);
    assert!(
        node_a.has_focus(),
        "the way back restores the committed save"
    );
    let _ = navigator;
}

#[test]
fn outlet_same_identity_replacement_mid_build_commits() {
    // The attempt snapshot covers same-key/same-ID child replacement:
    // identities and configuration match, so the transition commits and
    // the way back restores — replacement is content churn, not a new
    // navigation. (The replacement mounts on the frame after the layout
    // that requested it, hence the second present before tabbing.)
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let ka = PageKey::new("a").unwrap();
    let kb = PageKey::new("b").unwrap();
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    let node_b2 = FocusNode::new();
    navigator.push_page(Page::new("a", focus_child(&node_a)).key(ka.clone()));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    let trip = Rc::new(Cell::new(true));
    let node_a_for_build = node_a.clone();
    let node_b2_for_build = node_b2.clone();
    navigator.push_page(
        Page::new(
            "b",
            Column::new(vec![Widget::from(LayoutBuilder::new({
                let navigator = navigator.clone();
                let ka = ka.clone();
                let kb = kb.clone();
                move |_, _| {
                    if trip.take() {
                        // Same keys, new widgets: entry identities survive.
                        navigator
                            .set_pages([
                                Page::new("a", focus_child(&node_a_for_build)).key(ka.clone()),
                                Page::new(
                                    "b",
                                    focus_widget(
                                        &node_b2_for_build,
                                        Widget::box_(Size::new(40., 40.), BLUE),
                                    ),
                                )
                                .key(kb.clone()),
                            ])
                            .unwrap();
                    }
                    focus_widget(&node_b, Widget::box_(Size::new(40., 40.), BLUE))
                }
            }))]),
        )
        .key(kb.clone()),
    );
    let rb = navigator.current().expect("route B").id;
    present(&mut harness);
    // Same route B throughout: still bound under its original id.
    assert!(harness.outlet.borrow().route_task_scope(rb).is_some());
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_b2);
    round_trip_restores(&node_a, &node_b2, &navigator, &mut harness);
}

#[test]
fn outlet_push_pop_to_original_top_mid_build_commits() {
    // A builder excursion that nets back to the attempt's stack (push X,
    // pop X) keeps the snapshot covering: the transition commits.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    let trip = Rc::new(Cell::new(true));
    let node_b_for_build = node_b.clone();
    navigator.push_page(Page::new(
        "b",
        Column::new(vec![Widget::from(LayoutBuilder::new({
            let navigator = navigator.clone();
            move |_, _| {
                if trip.take() {
                    let x = navigator.push_page(plain_page("x"));
                    assert!(navigator.lifetime_of(x).is_some());
                    navigator.pop();
                }
                focus_widget(&node_b_for_build, Widget::box_(Size::new(40., 40.), BLUE))
            }
        }))]),
    ));
    present(&mut harness);
    assert_eq!(navigator.routes().len(), 2);
    round_trip_restores(&node_a, &node_b, &navigator, &mut harness);
}

#[test]
fn outlet_lower_reorder_beneath_top_mid_build_commits() {
    // Reordering covered routes beneath an unchanged top is order-only
    // churn: the member set still covers, so the transition commits.
    // Shared widget instances: cloning the same value means reconciliation
    // sees identical content, so the mid-build reorder below exercises
    // order-only churn — no remount, no stale targets. (Fresh instances
    // would remount the subtrees and the return would correctly fall back
    // through the fail-closed restore instead.)
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let kx = PageKey::new("x").unwrap();
    let ky = PageKey::new("y").unwrap();
    let kb = PageKey::new("b").unwrap();
    let kc = PageKey::new("c").unwrap();
    let node_b = FocusNode::new();
    let node_c = FocusNode::new();
    let child_x = Widget::box_(Size::new(40., 40.), RED);
    let child_y = Widget::box_(Size::new(40., 40.), BLUE);
    let child_b = focus_child(&node_b);
    let child_c = focus_widget(&node_c, Widget::box_(Size::new(40., 40.), GREEN));
    navigator
        .set_pages([
            Page::new("x", child_x.clone()).key(kx.clone()),
            Page::new("y", child_y.clone()).key(ky.clone()),
            Page::new("b", child_b.clone()).key(kb.clone()),
        ])
        .unwrap();
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_b);
    // B→C transition in flight while the builder swaps X/Y strictly
    // beneath B; B never moves and stays on top once C pops.
    let trip = Rc::new(Cell::new(true));
    navigator.push_page(
        Page::new(
            "c",
            Column::new(vec![Widget::from(LayoutBuilder::new({
                let navigator = navigator.clone();
                let kx = kx.clone();
                let ky = ky.clone();
                let kb = kb.clone();
                let kc = kc.clone();
                let child_x = child_x.clone();
                let child_y = child_y.clone();
                let child_b = child_b.clone();
                let child_c = child_c.clone();
                move |_, _| {
                    if trip.take() {
                        navigator
                            .set_pages([
                                Page::new("y", child_y.clone()).key(ky.clone()),
                                Page::new("x", child_x.clone()).key(kx.clone()),
                                Page::new("b", child_b.clone()).key(kb.clone()),
                                Page::new("c", child_c.clone()).key(kc.clone()),
                            ])
                            .unwrap();
                    }
                    child_c.clone()
                }
            }))]),
        )
        .key(kc.clone()),
    );
    present(&mut harness);
    assert_eq!(navigator.current().expect("top").name, "c");
    round_trip_restores(&node_b, &node_c, &navigator, &mut harness);
}

#[test]
fn nested_detached_retained_stays_bounded_across_cycles() {
    // Callers retain every detached generation (nothing drops): explicit
    // detaches still keep registrations bounded, detached drives frozen,
    // and only the current generation answers the cascade — repeated
    // mount/unmount never leaks metadata into retained handles.
    let outer = Navigator::new();
    let parent_scope_holder =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    let parent = parent_scope_holder.spawner().scope();
    let outlet_outer = Rc::new(RefCell::new(RouteOutlet::new(&outer, &parent)));
    let key = PageKey::new("a").unwrap();
    let node_a = FocusNode::new();
    let (mut runtime, revision) = nested_host(outlet_outer.clone());
    // Retained (navigator, outlet, drives-at-detach) per generation.
    let mut retired: Vec<(Navigator, Rc<RefCell<RouteOutlet>>, u64)> = Vec::new();
    let mut current: Option<(Navigator, Rc<RefCell<RouteOutlet>>)> = None;
    for generation in 0..3_u32 {
        if let Some((nav, outlet)) = current.take() {
            let drives = outlet.borrow().frame_drive_count();
            RouteOutlet::detach_nested(&outlet_outer, &outlet);
            retired.push((nav, outlet, drives));
        }
        let inner = Navigator::new();
        let outlet_inner = Rc::new(RefCell::new(RouteOutlet::new(&inner, &parent)));
        outer
            .set_pages([Page::new(
                "a",
                Column::new(vec![
                    focus_widget(&node_a, Widget::box_(Size::new(40., 40.), RED)),
                    RouteOutlet::nested_widget(&outlet_inner),
                ]),
            )
            .key(key.clone())])
            .unwrap();
        RouteOutlet::attach_nested(&outlet_outer, &outlet_inner).expect("nested attaches");
        current = Some((inner, outlet_inner));
        present_nested(&mut runtime, &outlet_outer, &revision);
        assert_eq!(
            outlet_outer.borrow().attached_nested_count(),
            1,
            "explicit detach keeps registrations bounded while handles are retained (generation {generation})"
        );
    }
    // Retired generations are frozen: navigating a detached navigator and
    // presenting moves nothing inside the retained outlets.
    for (nav, _outlet, _drives) in &retired {
        nav.push_page(plain_page("z"));
    }
    present_nested(&mut runtime, &outlet_outer, &revision);
    for (_, outlet, drives) in &retired {
        assert_eq!(
            outlet.borrow().frame_drive_count(),
            *drives,
            "detached outlets receive no frame work while retained"
        );
    }
    assert_eq!(outlet_outer.borrow().attached_nested_count(), 1);
    // The live tree is fully functional end to end.
    tab_until(&mut runtime, &node_a);
    let (inner, _) = current.as_ref().expect("current generation");
    let node_b = FocusNode::new();
    inner.push_page(focus_page("b", &node_b));
    present_nested(&mut runtime, &outlet_outer, &revision);
    tab_until(&mut runtime, &node_b);
    assert_eq!(retired.len(), 2);
}

#[test]
fn outlet_reorder_elsewhere_with_new_content_restores() {
    // The reorder commits (same member set covers) and the transition's
    // own subtree is untouched — B never moves, so its element stays
    // stable while X/Y swap with fresh instances around it. The return
    // restores exactly: foreign remounts never disturb the transition's
    // own bookkeeping. (Stale targets from genuinely dead elements stay
    // fail-closed per the disposal and deferred coverage.)
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let kx = PageKey::new("x").unwrap();
    let ky = PageKey::new("y").unwrap();
    let kb = PageKey::new("b").unwrap();
    let kc = PageKey::new("c").unwrap();
    let node_b = FocusNode::new();
    let node_c = FocusNode::new();
    let node_b_for_build = node_b.clone();
    navigator
        .set_pages([
            Page::new("x", Widget::box_(Size::new(40., 40.), RED)).key(kx.clone()),
            Page::new("y", Widget::box_(Size::new(40., 40.), BLUE)).key(ky.clone()),
            Page::new("b", focus_child(&node_b)).key(kb.clone()),
        ])
        .unwrap();
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_b);
    let trip = Rc::new(Cell::new(true));
    let node_c_for_build = node_c.clone();
    navigator.push_page(
        Page::new(
            "c",
            Column::new(vec![Widget::from(LayoutBuilder::new({
                let navigator = navigator.clone();
                let kx = kx.clone();
                let ky = ky.clone();
                let kb = kb.clone();
                let kc = kc.clone();
                move |_, _| {
                    if trip.take() {
                        // Same identities, fresh content: order-only churn
                        // plus remounts.
                        navigator
                            .set_pages([
                                Page::new("y", Widget::box_(Size::new(40., 40.), BLUE))
                                    .key(ky.clone()),
                                Page::new("x", Widget::box_(Size::new(40., 40.), RED))
                                    .key(kx.clone()),
                                Page::new("b", focus_child(&node_b_for_build)).key(kb.clone()),
                                Page::new(
                                    "c",
                                    focus_widget(
                                        &node_c_for_build,
                                        Widget::box_(Size::new(40., 40.), GREEN),
                                    ),
                                )
                                .key(kc.clone()),
                            ])
                            .unwrap();
                    }
                    focus_widget(&node_c_for_build, Widget::box_(Size::new(40., 40.), GREEN))
                }
            }))]),
        )
        .key(kc.clone()),
    );
    present(&mut harness);
    assert_eq!(navigator.current().expect("top").name, "c");
    tab_until(&mut harness.runtime, &node_c);
    navigator.pop();
    present(&mut harness);
    assert!(node_b.has_focus());
    assert!(harness.runtime.focused_element().is_some());
}

#[test]
fn outlet_retry_keeps_original_save_despite_intervening_focus() {
    // The kept capture is never re-read: after a failed attempt focus
    // legitimately moves within the still-mounted outgoing route, yet the
    // retry commits the pre-attempt save. (A transparent cover keeps A
    // mounted so the move is legal while B's half-built content settles.)
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a1 = FocusNode::new();
    let node_a2 = FocusNode::new();
    navigator.push_page(two_focus_route_page("a", &node_a1, &node_a2));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a1);
    let ea1 = harness.runtime.focused_element().expect("A1 focused");
    let trip = Rc::new(Cell::new(true));
    navigator.push(
        Route::new(
            "b",
            Column::new(vec![Widget::from(LayoutBuilder::new(move |_, _| {
                if trip.take() {
                    panic!("cover boom");
                }
                Widget::box_(Size::new(40., 40.), BLUE)
            }))]),
        )
        .presentation(RoutePresentation::popup(None)),
    );
    let outlet = harness.outlet.clone();
    let result = catch_unwind(AssertUnwindSafe(|| {
        RouteOutlet::present_frame(
            &outlet,
            &mut harness.runtime,
            Constraints::tight(Size::new(200., 200.)),
        )
    }));
    result.expect_err("cover panic unwinds the frame");
    // The outgoing route stayed mounted under its transparent cover, so
    // focus moves within it before the retry.
    tab_until(&mut harness.runtime, &node_a2);
    assert_ne!(harness.runtime.focused_element(), Some(ea1));
    // Retry commits the original A1 save — never the intervening A2.
    present(&mut harness);
    navigator.pop();
    present(&mut harness);
    assert_eq!(harness.runtime.focused_element(), Some(ea1));
    assert!(node_a1.has_focus());
}

#[test]
fn outlet_unsupported_introduced_mid_build_fails_explicitly() {
    // A builder that pushes an overlay route mid-frame: the preflight
    // snapshot cannot authorize it, so the post-frame check fails typed
    // with the route — no commit, no bindings, no silent omission. The
    // pending transition survives for the retry, which commits the
    // original save once the offender is gone.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    let trip = Rc::new(Cell::new(true));
    let node_b_for_build = node_b.clone();
    navigator.push_page(Page::new(
        "b",
        Column::new(vec![Widget::from(LayoutBuilder::new({
            let navigator = navigator.clone();
            move |_, _| {
                if trip.take() {
                    navigator.push(
                        Route::new("overlay", Widget::box_(Size::new(40., 40.), GREEN)).overlay(
                            vec![OverlayEntry::new(Widget::box_(Size::new(40., 40.), GREEN))],
                        ),
                    );
                }
                focus_widget(&node_b_for_build, Widget::box_(Size::new(40., 40.), BLUE))
            }
        }))]),
    ));
    let outlet_id = harness.outlet.borrow().id();
    let error = RouteOutlet::present_frame(
        &harness.outlet,
        &mut harness.runtime,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect_err("mid-build overlay fails explicitly");
    let overlay_id = navigator.current().expect("overlay pushed").id;
    assert_eq!(
        error,
        OutletError::UnsupportedPresentation {
            outlet: outlet_id,
            routes: vec![overlay_id]
        }
    );
    // Nothing committed: the offender never bound, and focus never moved.
    assert!(
        harness
            .outlet
            .borrow()
            .route_task_scope(overlay_id)
            .is_none()
    );
    assert!(node_a.has_focus());
    // Recovery: pop the offender and retry — the kept A→B capture commits
    // the original save, so the way back restores A.
    navigator.pop();
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_b);
    navigator.pop();
    present(&mut harness);
    assert!(node_a.has_focus());
}

#[test]
fn outlet_same_id_replacement_paints_old_then_new() {
    // Replacement lands during layout, after the rebuild composed: the
    // first present paints the OLD child while the bookkeeping already
    // commits (same identities cover), with newer work left scheduled —
    // the second present paints the NEW child. Output and bookkeeping
    // disagree for exactly one frame, by design.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let ka = PageKey::new("a").unwrap();
    let kb = PageKey::new("b").unwrap();
    let node_a = FocusNode::new();
    let node_old = FocusNode::new();
    let node_new = FocusNode::new();
    navigator.push_page(Page::new("a", focus_child(&node_a)).key(ka.clone()));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    let trip = Rc::new(Cell::new(true));
    let node_a_for_build = node_a.clone();
    let node_new_for_build = node_new.clone();
    navigator.push_page(
        Page::new(
            "b",
            Column::new(vec![Widget::from(LayoutBuilder::new({
                let navigator = navigator.clone();
                let ka = ka.clone();
                let kb = kb.clone();
                move |_, _| {
                    if trip.take() {
                        navigator
                            .set_pages([
                                Page::new("a", focus_child(&node_a_for_build)).key(ka.clone()),
                                Page::new(
                                    "b",
                                    focus_widget(
                                        &node_new_for_build,
                                        Widget::box_(Size::new(40., 40.), GREEN),
                                    ),
                                )
                                .key(kb.clone()),
                            ])
                            .unwrap();
                    }
                    focus_widget(&node_old, Widget::box_(Size::new(40., 40.), RED))
                }
            }))]),
        )
        .key(kb.clone()),
    );
    // First present: stale paint (OLD child), committed bookkeeping, and
    // newer work still scheduled.
    let first = present(&mut harness);
    assert!(paints(first.commands(), RED));
    assert!(
        !paints(first.commands(), GREEN),
        "the presenting frame predates the mid-build replacement"
    );
    let rb = navigator.current().expect("route B").id;
    assert!(
        harness.outlet.borrow().route_task_scope(rb).is_some(),
        "the transition commits against the composed snapshot"
    );
    assert!(
        harness.outlet.borrow().needs_frame(),
        "the replacement still requires a frame"
    );
    // Second present: the NEW child paints and nothing remains scheduled.
    let second = present(&mut harness);
    assert!(paints(second.commands(), GREEN));
    assert!(!harness.outlet.borrow().needs_frame());
    tab_until(&mut harness.runtime, &node_new);
    navigator.pop();
    present(&mut harness);
    assert!(node_a.has_focus());
}

#[test]
fn outlet_transparent_top_reorder_paints_and_hits_in_order() {
    // Two opaque lowers under a small transparent popup top: a mid-build
    // reorder of the keyed lowers commits, and the returned frames must
    // prove the new order in paint commands and hit-testing — not just in
    // focus bookkeeping. The popup stays transparent across the
    // declarative update because its pages keep declaring it.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let ka = PageKey::new("a").unwrap();
    let kb = PageKey::new("b").unwrap();
    let kt = PageKey::new("t").unwrap();
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    let taps_a = Rc::new(Cell::new(0_u32));
    let taps_b = Rc::new(Cell::new(0_u32));
    // Full-area tappable boxes with a small focus target at the origin;
    // taps at the center avoid both the focus boxes and the popup corner.
    let lower = |node: &FocusNode, color: Color, taps: &Rc<Cell<u32>>| -> Widget {
        Stack::new(vec![
            tappable_rect(color, 200., 200., taps),
            focus_widget(node, Widget::box_(Size::new(40., 40.), color)),
        ])
        .into()
    };
    navigator
        .set_pages([
            Page::new("a", lower(&node_a, RED, &taps_a)).key(ka.clone()),
            Page::new("b", lower(&node_b, BLUE, &taps_b)).key(kb.clone()),
        ])
        .unwrap();
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_b);
    // Keyed transparent top whose builder reorders the lowers mid-build.
    let trip = Rc::new(Cell::new(true));
    navigator.push_page(
        Page::new(
            "t",
            Column::new(vec![Widget::from(LayoutBuilder::new({
                let navigator = navigator.clone();
                let ka = ka.clone();
                let kb = kb.clone();
                let kt = kt.clone();
                let node_a = node_a.clone();
                let node_b = node_b.clone();
                // Cloned handles share identity (nodes) and counters, so
                // the reordered pages match the originals exactly.
                let node_a = node_a.clone();
                let node_b = node_b.clone();
                let taps_a = taps_a.clone();
                let taps_b = taps_b.clone();
                move |_, _| {
                    if trip.take() {
                        let lower_b: Widget = Stack::new(vec![
                            tappable_rect(BLUE, 200., 200., &taps_b),
                            focus_widget(&node_b, Widget::box_(Size::new(40., 40.), BLUE)),
                        ])
                        .into();
                        let lower_a: Widget = Stack::new(vec![
                            tappable_rect(RED, 200., 200., &taps_a),
                            focus_widget(&node_a, Widget::box_(Size::new(40., 40.), RED)),
                        ])
                        .into();
                        navigator
                            .set_pages([
                                Page::new("b", lower_b).key(kb.clone()),
                                Page::new("a", lower_a).key(ka.clone()),
                                Page::new("t", Widget::box_(Size::new(40., 40.), GREEN))
                                    .key(kt.clone())
                                    .presentation(RoutePresentation::popup(None)),
                            ])
                            .unwrap();
                    }
                    Widget::box_(Size::new(40., 40.), GREEN)
                }
            }))]),
        )
        .key(kt.clone())
        .presentation(RoutePresentation::popup(None)),
    );
    // First present paints the OLD order (B occludes A under the
    // transparent top) and still owes a frame for the reorder.
    let first = present(&mut harness);
    assert!(paints(first.commands(), BLUE));
    assert!(paints(first.commands(), GREEN));
    assert!(
        !paints(first.commands(), RED),
        "paint follows the composed order, not the committed one"
    );
    assert!(harness.outlet.borrow().needs_frame());
    // Second present paints the NEW order with the top still
    // transparent; taps hit the new top lower.
    let second = present(&mut harness);
    assert!(paints(second.commands(), RED));
    assert!(paints(second.commands(), GREEN));
    assert!(!paints(second.commands(), BLUE), "A now occludes B");
    assert!(!harness.outlet.borrow().needs_frame());
    tap(&mut harness, 100., 100.);
    assert_eq!(taps_a.get(), 1);
    assert_eq!(taps_b.get(), 0);
    // Popping the popup returns to A: B's subtree remounted under fresh
    // instances, so its record cannot restore — focus falls back cleanly
    // instead of landing on a recycled element — while bindings follow
    // lifetimes exactly and the live top tabs normally.
    navigator.pop();
    present(&mut harness);
    assert_eq!(harness.runtime.focused_element(), None);
    let routes = navigator.routes();
    assert_eq!(routes.len(), 2);
    let (rb, ra) = (routes[0].id, routes[1].id);
    assert_eq!(navigator.current().expect("A on top").id, ra);
    assert!(harness.outlet.borrow().route_task_scope(rb).is_some());
    assert!(harness.outlet.borrow().route_task_scope(ra).is_some());
    tab_until(&mut harness.runtime, &node_a);
    assert!(node_a.has_focus());
}

#[test]
fn outlet_modal_barrier_same_key_update_repaints() {
    // A keyed modal's barrier follows the latest page: swapping GREEN for
    // RED repaints the veil on the next present with the lifetime — and
    // its task scope — preserved, so in-flight route work survives the
    // configuration change.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let wake = Arc::new(TestWake::default());
    harness.runtime.set_wake_handler(wake.clone());
    let km = PageKey::new("m").unwrap();
    navigator.push_page(
        Page::new("m", Widget::box_(Size::new(40., 40.), BLUE))
            .key(km.clone())
            .presentation(RoutePresentation::modal(ModalBarrier {
                color: GREEN,
                ..Default::default()
            })),
    );
    present(&mut harness);
    let id = navigator.current().expect("modal").id;
    let scope = harness
        .outlet
        .borrow()
        .route_task_scope(id)
        .expect("modal bound");
    let completed = Arc::new(AtomicBool::new(false));
    let completed_for_completion = completed.clone();
    harness
        .runtime
        .spawner()
        .spawn_into_in(&scope, async { 7_u8 }, move |result, _| {
            assert_eq!(result.expect("update never cancels"), 7_u8);
            completed_for_completion.store(true, Ordering::Release);
        });
    assert!(paints(repaint(&mut harness).commands(), GREEN));
    // Same key, new barrier: the veil repaints, the route ID holds, and
    // the task completes through the preserved lifetime.
    navigator
        .set_pages([Page::new("m", Widget::box_(Size::new(40., 40.), BLUE))
            .key(km)
            .presentation(RoutePresentation::modal(ModalBarrier {
                color: RED,
                ..Default::default()
            }))])
        .unwrap();
    assert_eq!(navigator.current().expect("retained").id, id);
    present(&mut harness);
    assert!(paints(repaint(&mut harness).commands(), RED));
    assert!(
        !paints(repaint(&mut harness).commands(), GREEN),
        "the old veil is gone"
    );
    pump_until(&mut harness.runtime, &completed);
    assert!(!scope.is_cancelled());
    navigator.pop();
    present(&mut harness);
    assert!(scope.is_cancelled());
}

#[test]
fn outlet_modal_pushed_mid_build_abandons_before_veil_paint() {
    // A→B in flight while B's builder pushes a modal: membership changes,
    // so the capture abandons — and the modal never mounts this frame, so
    // no veil paints, focus stays, yet the live modal binds. Recovery
    // mounts veil plus content; the way back restores A.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let ka = PageKey::new("a").unwrap();
    let kb = PageKey::new("b").unwrap();
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    navigator.push_page(Page::new("a", focus_child(&node_a)).key(ka.clone()));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    let trip = Rc::new(Cell::new(true));
    let node_b_for_build = node_b.clone();
    navigator.push_page(
        Page::new(
            "b",
            Column::new(vec![Widget::from(LayoutBuilder::new({
                let navigator = navigator.clone();
                move |_, _| {
                    if trip.take() {
                        navigator.push(
                            Route::new("m", Widget::box_(Size::new(40., 40.), YELLOW)).modal(
                                ModalBarrier {
                                    color: GREEN,
                                    ..Default::default()
                                },
                            ),
                        );
                    }
                    focus_widget(&node_b_for_build, Widget::box_(Size::new(40., 40.), BLUE))
                }
            }))]),
        )
        .key(kb.clone()),
    );
    let first = present(&mut harness);
    assert!(
        !paints(first.commands(), GREEN),
        "the abandoned frame mounts no veil for the unbuilt modal"
    );
    assert!(node_a.has_focus());
    let modal_id = navigator.current().expect("modal on top").id;
    assert!(
        harness.outlet.borrow().route_task_scope(modal_id).is_some(),
        "the live modal binds by lifetime although its activation abandoned"
    );
    assert!(harness.outlet.borrow().needs_frame());
    // Recovery presents veil plus content; the way back restores A.
    let second = present(&mut harness);
    assert!(paints(second.commands(), GREEN));
    assert!(!harness.outlet.borrow().needs_frame());
    navigator.pop();
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_b);
    navigator.pop();
    present(&mut harness);
    assert!(node_a.has_focus());
}

#[test]
fn outlet_rebuild_phase_navigation_consumes_pre_trip_snapshot() {
    // A first-build stateful closure navigates while its own mounting
    // rebuild is still in flight — after widget() already read the
    // routes. (Explicitly registered builders run before the mount
    // builder, so they cannot interleave this way; first-build closures
    // run as their elements mount.) The composition publishes the
    // pre-trip snapshot: paint shows only pre-trip content while
    // needs_frame reports the newer work, and the next present converges.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a = FocusNode::new();
    let node_b_for_build = FocusNode::new();
    navigator.push_page(Page::new(
        "a",
        focus_widget(&node_a, Widget::box_(Size::new(40., 40.), RED)),
    ));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    let trip = Rc::new(Cell::new(true));
    let builder_revision = Rc::new(Cell::new(0_u64));
    navigator.push_page(Page::new(
        "b",
        Column::new(vec![Widget::stateful_layout_builder(
            builder_revision.clone(),
            {
                let navigator = navigator.clone();
                move |_, _| {
                    if trip.take() {
                        navigator
                            .push_page(Page::new("c", Widget::box_(Size::new(40., 40.), YELLOW)));
                    }
                    focus_widget(&node_b_for_build, Widget::box_(Size::new(40., 40.), BLUE))
                }
            },
        )]),
    ));
    let first = present(&mut harness);
    assert!(
        !paints(first.commands(), YELLOW),
        "C pushed mid-rebuild never composed"
    );
    assert!(paints(first.commands(), BLUE), "pre-trip content painted");
    assert!(node_a.has_focus());
    let rc = navigator.current().expect("C pushed").id;
    assert!(
        harness.outlet.borrow().route_task_scope(rc).is_some(),
        "the live route binds although never mounted"
    );
    assert!(
        harness.outlet.borrow().needs_frame(),
        "the mid-rebuild push still requires a frame"
    );
    // The next present converges: mounts C, commits, clears the flag.
    let second = present(&mut harness);
    assert!(paints(second.commands(), YELLOW));
    assert!(!harness.outlet.borrow().needs_frame());
    navigator.pop();
    navigator.pop();
    present(&mut harness);
    assert!(node_a.has_focus());
}

#[test]
fn outlet_same_attempt_discard_changes_nothing() {
    // The receipt rule within one attempt: a fresh trip page navigates to
    // B mid-frame and discards a fresh widget(); the frame still reports
    // only what its builders consumed. B stays unconsumed with follow-up
    // pending — descriptor construction acknowledges nothing.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a = FocusNode::new();
    let node_b_for_build = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    let outlet_for_trip = harness.outlet.clone();
    let trip = Rc::new(Cell::new(true));
    navigator.push_page(Page::new(
        "t",
        Column::new(vec![Widget::from(LayoutBuilder::new({
            let navigator = navigator.clone();
            move |_, _| {
                if trip.take() {
                    navigator.push_page(Page::new(
                        "b",
                        focus_widget(&node_b_for_build, Widget::box_(Size::new(40., 40.), BLUE)),
                    ));
                    // Speculative composition mid-frame: discarded, and
                    // (by construction) unpublished.
                    let _ = outlet_for_trip.borrow().widget();
                }
                Widget::box_(Size::new(40., 40.), GREEN)
            }
        }))]),
    ));
    // The frame composes A and T, then the callback navigates and
    // discards: B never composes although the trip ran.
    let first = present(&mut harness);
    assert!(paints(first.commands(), GREEN));
    assert!(
        !paints(first.commands(), BLUE),
        "B pushed mid-frame never composed"
    );
    assert!(node_a.has_focus());
    let rb = navigator.current().expect("B pushed").id;
    assert!(
        harness.outlet.borrow().route_task_scope(rb).is_some(),
        "the live route binds although never mounted"
    );
    assert!(
        harness.outlet.borrow().needs_frame(),
        "B stays unconsumed with follow-up pending"
    );
    assert!(
        harness.runtime.frame_requested(),
        "the stale frame schedules its follow-up"
    );
    // The next present converges: mounts B, commits, consumes current —
    // then the way back restores A. (The frame still flags a follow-up
    // for covering A: the orphan sweep clears through the scheduler,
    // which is scheduling, not staleness.)
    let second = present(&mut harness);
    assert!(paints(second.commands(), BLUE));
    assert!(!harness.outlet.borrow().needs_frame());
    navigator.pop();
    navigator.pop();
    present(&mut harness);
    assert!(node_a.has_focus());
}

#[test]
fn outlet_nested_same_attempt_discard_changes_nothing() {
    // Same rule through a nested outlet: the inner layout callback
    // navigates and discards a fresh inner widget(); the inner outlet
    // keeps its build-time consumption while the outer stays current.
    let (outer, inner, mut runtime, outlet_outer, outlet_inner, node_a, _node_b) =
        nested_inside_setup();
    let armed = Rc::new(Cell::new(true));
    let outlet_inner_for_trip = outlet_inner.clone();
    inner.push_page(Page::new(
        "ib",
        Column::new(vec![
            Widget::box_(Size::new(40., 40.), BLUE),
            Widget::from(LayoutBuilder::new({
                let inner = inner.clone();
                let armed = armed.clone();
                move |_, _| {
                    if armed.take() {
                        inner.push_page(Page::new("ic", Widget::box_(Size::new(40., 40.), YELLOW)));
                        let _ = outlet_inner_for_trip.borrow().widget();
                    }
                    Widget::box_(Size::new(40., 40.), GREEN)
                }
            })),
        ]),
    ));
    // The trip is armed from construction: it fires on this present's
    // layout, after the slot builder consumed the pre-trip stack.
    let first = present_outer_tree_output(&mut runtime, &outlet_outer);
    assert!(!paints(first.commands(), YELLOW));
    assert!(
        outlet_inner.borrow().needs_frame(),
        "inner keeps its build-time consumption"
    );
    assert!(
        runtime.frame_requested(),
        "nested staleness wakes the root driver"
    );
    let ic = inner.current().expect("pushed route").id;
    assert!(
        outlet_inner.borrow().route_task_scope(ic).is_some(),
        "the live route binds although never mounted"
    );
    let second = present_outer_tree_output(&mut runtime, &outlet_outer);
    assert!(paints(second.commands(), YELLOW));
    assert!(!outlet_inner.borrow().needs_frame());
    assert!(!runtime.frame_requested());
    let _ = (outer, node_a);
}

#[test]
fn outlet_discarded_composition_authorizes_no_frame() {
    // The receipt rule, step by step: present revision A; navigate to B;
    // compose B outside any frame and discard it; run a frame whose
    // builder never consumes B. The outlet must still report A as
    // consumed with B outstanding — the discarded composition authorizes
    // nothing — while bookkeeping (which follows live state) advances.
    // Remounting converges normally.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    assert!(!harness.outlet.borrow().needs_frame());
    // Revision B arrives but never composes: the speculative widget is
    // discarded, then the outlet content unmounts before the frame.
    navigator.push_page(Page::new("b", Widget::box_(Size::new(40., 40.), BLUE)));
    let _ = harness.outlet.borrow().widget();
    let root = harness.runtime.tree().root().expect("root");
    harness
        .runtime
        .register_builder(root, move || {
            Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into()
        })
        .expect("root builder replaces");
    frame(&mut harness.runtime);
    assert_eq!(harness.runtime.focused_element(), None);
    // The frame presents without the outlet builder: bookkeeping follows
    // live navigation (B binds), but consumption stays at A.
    let unmounted = RouteOutlet::present_frame(
        &harness.outlet,
        &mut harness.runtime,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect("unmounted present still reconciles")
    .0;
    assert!(!paints(unmounted.commands(), BLUE));
    let rb = navigator.current().expect("route B").id;
    assert!(
        harness.outlet.borrow().route_task_scope(rb).is_some(),
        "bindings follow live state, not consumption"
    );
    assert!(
        harness.outlet.borrow().needs_frame(),
        "A stays consumed with B outstanding"
    );
    // Remounting converges: the next composed frame presents B and idles.
    let outlet_for_build = harness.outlet.clone();
    harness
        .runtime
        .register_builder(root, move || {
            Column::new(vec![SizedBox::from_dimensions(
                Some(200.),
                Some(200.),
                Some(outlet_for_build.borrow().widget()),
            )])
            .into()
        })
        .expect("outlet builder restores");
    frame(&mut harness.runtime);
    RouteOutlet::attach(&harness.outlet, &mut harness.runtime).expect("outlet reattaches");
    let remounted = present(&mut harness);
    assert!(paints(remounted.commands(), BLUE));
    assert!(!harness.outlet.borrow().needs_frame());
    assert!(harness.outlet.borrow().route_task_scope(rb).is_some());
}

#[test]
fn outlet_failed_attempt_then_skipped_builder_stays_stale() {
    // A failed attempt publishes nothing; the following successful frame
    // skips the nested builder (slotless), so the inner outlet keeps its
    // previous publication with newer navigation outstanding — until a
    // remount consumes it.
    let (outer, inner, mut runtime, outlet_outer, outlet_inner, node_a, _node_b) =
        nested_inside_setup();
    inner.push(Route::new(
        "bad",
        Column::new(vec![
            Widget::box_(Size::new(40., 40.), BLUE).with_key(7_u64),
            Widget::box_(Size::new(40., 40.), GREEN).with_key(7_u64),
        ]),
    ));
    RouteOutlet::present_frame(
        &outlet_outer,
        &mut runtime,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect_err("duplicate keys fail the rebuild");
    // Slotless success: the outer outlet promotes current state while the
    // inner builder never executes.
    outer
        .set_pages([Page::new("solo", focus_child(&node_a))])
        .unwrap();
    inner.pop();
    let node_c = FocusNode::new();
    inner.push_page(focus_page("c", &node_c));
    present_outer_tree(&mut runtime, &outlet_outer);
    assert!(
        outlet_inner.borrow().needs_frame(),
        "the skipped builder published nothing new"
    );
    assert!(
        outlet_outer.borrow().needs_frame(),
        "inner staleness wakes the root driver"
    );
    // Remounting consumes the pending inner revision through the cascade.
    let outlet_inner_for_page = outlet_inner.clone();
    outer
        .set_pages([Page::new(
            "a",
            Column::new(vec![
                focus_widget(&node_a, Widget::box_(Size::new(40., 40.), RED)),
                RouteOutlet::nested_widget(&outlet_inner_for_page),
            ]),
        )])
        .unwrap();
    drop(outlet_inner_for_page);
    RouteOutlet::attach_nested(&outlet_outer, &outlet_inner).expect("nested attaches");
    present_outer_tree(&mut runtime, &outlet_outer);
    assert!(!outlet_inner.borrow().needs_frame());
    assert!(!outlet_outer.borrow().needs_frame());
    tab_until(&mut runtime, &node_c);
    let _ = outer;
}

#[test]
fn outlet_skipped_nested_builder_keeps_stale_consumed() {
    // Unmount the nested slot: the inner builder never executes, so the
    // inner consumed snapshot freezes while inner navigation advances —
    // the inner outlet reports pending work even right after an outer
    // present that is current itself. Remounting resumes recording.
    let (outer, inner, mut runtime, outlet_outer, outlet_inner, node_a, node_b) =
        nested_inside_setup();
    inner.push_page(focus_page("b", &node_b));
    present_outer_tree(&mut runtime, &outlet_outer);
    tab_until(&mut runtime, &node_b);
    assert!(!outlet_inner.borrow().needs_frame());
    // Replace outer content without the slot, then advance inner
    // navigation while slotless.
    outer
        .set_pages([Page::new("solo", focus_child(&node_a))])
        .unwrap();
    present_outer_tree(&mut runtime, &outlet_outer);
    inner.push_page(plain_page("c"));
    present_outer_tree(&mut runtime, &outlet_outer);
    // needs_frame aggregates the tree: the skipped inner builder's
    // staleness surfaces at the root (it wakes the driver) even though
    // the outer outlet composed current state.
    assert!(
        outlet_inner.borrow().needs_frame(),
        "the skipped inner builder published nothing"
    );
    assert!(
        outlet_outer.borrow().needs_frame(),
        "inner staleness wakes the root driver"
    );
    // Remount the slot: recording resumes and the cascade still drives.
    let outlet_inner_for_page = outlet_inner.clone();
    outer
        .set_pages([Page::new(
            "a",
            Column::new(vec![
                focus_widget(&node_a, Widget::box_(Size::new(40., 40.), RED)),
                RouteOutlet::nested_widget(&outlet_inner_for_page),
            ]),
        )])
        .unwrap();
    drop(outlet_inner_for_page);
    RouteOutlet::attach_nested(&outlet_outer, &outlet_inner).expect("nested attaches");
    present_outer_tree(&mut runtime, &outlet_outer);
    assert!(!outlet_inner.borrow().needs_frame());
    inner.pop();
    present_outer_tree(&mut runtime, &outlet_outer);
    tab_until(&mut runtime, &node_b);
    let _ = outer;
}

#[test]
fn outlet_nested_layout_navigation_keeps_build_consumption() {
    // A layout builder inside NESTED content navigates during the frame:
    // the inner outlet keeps the snapshot its slot builder consumed (not
    // a post-frame sample) — paint omits the pushed route while
    // needs_frame reports it — and the outer outlet stays current.
    let (outer, inner, mut runtime, outlet_outer, outlet_inner, node_a, _node_b) =
        nested_inside_setup();
    let node_b = FocusNode::new();
    let node_b_for_build = node_b.clone();
    let trip = Rc::new(Cell::new(true));
    inner.push_page(Page::new(
        "b",
        Column::new(vec![Widget::from(LayoutBuilder::new({
            let inner = inner.clone();
            move |_, _| {
                if trip.take() {
                    inner.push_page(Page::new("c", Widget::box_(Size::new(40., 40.), YELLOW)));
                }
                focus_widget(&node_b_for_build, Widget::box_(Size::new(40., 40.), BLUE))
            }
        }))]),
    ));
    present_outer_tree(&mut runtime, &outlet_outer);
    // The pushed route never composed this frame, the inner outlet still
    // owes a frame for it, and the outer outlet is current.
    assert!(
        !paints(
            runtime
                .run_frame(Constraints::tight(Size::new(200., 200.)))
                .expect("repaint")
                .0
                .commands(),
            YELLOW
        ),
        "C pushed mid-frame never composed"
    );
    assert!(
        outlet_inner.borrow().needs_frame(),
        "inner owes a frame for the layout-phase push"
    );
    assert!(
        outlet_outer.borrow().needs_frame(),
        "inner staleness wakes the root driver"
    );
    let ic = inner.current().expect("pushed route").id;
    assert!(
        outlet_inner.borrow().route_task_scope(ic).is_some(),
        "the live route binds although never mounted"
    );
    // Recovery converges through the cascade alone.
    present_outer_tree(&mut runtime, &outlet_outer);
    assert!(!outlet_inner.borrow().needs_frame());
    assert!(!outlet_outer.borrow().needs_frame());
    let _ = (outer, node_a);
}

#[test]
fn outlet_mid_build_navigation_schedules_followup_frame() {
    // A→B in flight while B's builder pushes a yellow C: the presenting
    // frame paints pre-push content but schedules a follow-up through the
    // production scheduler (not a polling loop); the follow-up renders C
    // and goes idle. Focus never moves, so only the stale wake flags.
    // (All three routes stay transparent, keeping the saved focus valid
    // so restoration sweeps never flag either.)
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    let trip = Rc::new(Cell::new(true));
    navigator.push(
        Route::new(
            "b",
            Column::new(vec![Widget::from(LayoutBuilder::new({
                let navigator = navigator.clone();
                move |_, _| {
                    if trip.take() {
                        navigator.push(
                            Route::new("c", Widget::box_(Size::new(40., 40.), YELLOW))
                                .presentation(RoutePresentation::popup(None)),
                        );
                    }
                    Widget::box_(Size::new(40., 40.), BLUE)
                }
            }))]),
        )
        .presentation(RoutePresentation::popup(None)),
    );
    // Stale but successful: no yellow yet, follow-up scheduled.
    let first = present(&mut harness);
    assert!(!paints(first.commands(), YELLOW));
    assert!(paints(first.commands(), BLUE));
    assert!(
        harness.runtime.frame_requested(),
        "mid-build navigation schedules a follow-up"
    );
    // Follow-up renders the newer content, then goes idle.
    let second = present(&mut harness);
    assert!(paints(second.commands(), YELLOW));
    assert!(
        !harness.runtime.frame_requested(),
        "stable completion schedules nothing"
    );
    assert!(node_a.has_focus());
}

#[test]
fn outlet_nested_stale_content_wakes_root_driver() {
    // A layout trip inside NESTED content pushes mid-frame: the outer
    // navigator never moves, yet the outer present flags a follow-up —
    // nested staleness wakes the root driver. Recovery renders and idles,
    // with no focus anywhere (so only the stale wake can flag).
    let (_outer, inner, mut runtime, outlet_outer, outlet_inner, _node_a, _node_b) =
        nested_inside_setup();
    let node_ib = FocusNode::new();
    let node_ib_for_build = node_ib.clone();
    let trip = Rc::new(Cell::new(true));
    inner.push_page(Page::new(
        "ib",
        Column::new(vec![Widget::from(LayoutBuilder::new({
            let inner = inner.clone();
            move |_, _| {
                if trip.take() {
                    inner.push_page(Page::new("ic", Widget::box_(Size::new(40., 40.), YELLOW)));
                }
                focus_widget(&node_ib_for_build, Widget::box_(Size::new(40., 40.), BLUE))
            }
        }))]),
    ));
    // Stale but successful: no yellow yet, follow-up scheduled although
    // the outer navigator never moved.
    let first = present_outer_tree_output(&mut runtime, &outlet_outer);
    assert!(!paints(first.commands(), YELLOW));
    assert!(
        runtime.frame_requested(),
        "nested staleness wakes the root driver"
    );
    let ic = inner.current().expect("pushed route").id;
    assert!(
        outlet_inner.borrow().route_task_scope(ic).is_some(),
        "the live route binds although never mounted"
    );
    // Recovery renders the newer content, then goes idle.
    let second = present_outer_tree_output(&mut runtime, &outlet_outer);
    assert!(paints(second.commands(), YELLOW));
    assert!(
        !runtime.frame_requested(),
        "stable completion schedules nothing"
    );
    // The inner outlet still works end to end afterwards.
    inner.pop();
    present_outer_tree(&mut runtime, &outlet_outer);
    tab_until(&mut runtime, &node_ib);
}

#[test]
fn outlet_rejected_configuration_schedules_no_retry() {
    // An overlay pushed mid-build fails the post-frame validation: the
    // error returns with no follow-up scheduled — repeated failures
    // accumulate no work, so rejected stacks cannot spin a retry loop —
    // and popping the offender recovers cleanly.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    let trip = Rc::new(Cell::new(true));
    navigator.push_page(Page::new(
        "b",
        Column::new(vec![Widget::from(LayoutBuilder::new({
            let navigator = navigator.clone();
            move |_, _| {
                if trip.take() {
                    navigator.push(
                        Route::new("overlay", Widget::box_(Size::new(40., 40.), GREEN)).overlay(
                            vec![OverlayEntry::new(Widget::box_(Size::new(40., 40.), GREEN))],
                        ),
                    );
                }
                Widget::box_(Size::new(40., 40.), BLUE)
            }
        }))]),
    ));
    for _ in 0..2 {
        let error = RouteOutlet::present_frame(
            &harness.outlet,
            &mut harness.runtime,
            Constraints::tight(Size::new(200., 200.)),
        )
        .expect_err("mid-build overlay fails explicitly");
        assert!(
            matches!(error, OutletError::UnsupportedPresentation { .. }),
            "errors stay typed, never silent"
        );
        assert!(
            !harness.runtime.frame_requested(),
            "rejected stacks schedule no retry"
        );
    }
    assert!(node_a.has_focus());
    navigator.pop();
    present(&mut harness);
    navigator.pop();
    present(&mut harness);
    assert!(node_a.has_focus());
}

#[test]
fn outlet_failed_present_resets_schedule_suppression() {
    // P1: inner trip pushes mid-frame → inner stale, follow-up flagged.
    // P2: outer rebuild fails → the error return resets the schedule
    // memo. P3: slotless success leaves inner stale → must RE-FLAG
    // (without the reset the stale revision stays suppressed and no host
    // ever learns work is outstanding). P4 remount consumes and idles.
    let (outer, inner, mut runtime, outlet_outer, outlet_inner, _node_a, _node_b) =
        nested_inside_setup();
    let trip = Rc::new(Cell::new(true));
    inner.push_page(Page::new(
        "ib",
        Column::new(vec![Widget::from(LayoutBuilder::new({
            let inner = inner.clone();
            move |_, _| {
                if trip.take() {
                    inner.push_page(Page::new("ic", Widget::box_(Size::new(40., 40.), YELLOW)));
                }
                Widget::box_(Size::new(40., 40.), BLUE)
            }
        }))]),
    ));
    let first = present_outer_tree_output(&mut runtime, &outlet_outer);
    assert!(!paints(first.commands(), YELLOW));
    assert!(
        runtime.frame_requested(),
        "mid-frame inner navigation schedules a follow-up"
    );
    outer.push(Route::new(
        "bad",
        Column::new(vec![
            Widget::box_(Size::new(40., 40.), BLUE).with_key(7_u64),
            Widget::box_(Size::new(40., 40.), GREEN).with_key(7_u64),
        ]),
    ));
    RouteOutlet::present_frame(
        &outlet_outer,
        &mut runtime,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect_err("duplicate keys fail the rebuild");
    // Slotless success with inner work still outstanding: the follow-up
    // executes (outer converges) without consuming the inner revision —
    // and must schedule again for it.
    outer
        .set_pages([Page::new("solo", Widget::box_(Size::new(40., 40.), RED))])
        .unwrap();
    present_outer_tree(&mut runtime, &outlet_outer);
    assert!(
        outlet_inner.borrow().needs_frame(),
        "inner work is still outstanding"
    );
    assert!(
        runtime.frame_requested(),
        "the reset lets the outstanding revision schedule again"
    );
    // Remounting consumes the pending revision through the cascade, then
    // idles with no focus anywhere to flag anything else.
    let outlet_inner_for_page = outlet_inner.clone();
    outer
        .set_pages([Page::new(
            "a",
            Column::new(vec![
                Widget::box_(Size::new(40., 40.), RED),
                RouteOutlet::nested_widget(&outlet_inner_for_page),
            ]),
        )])
        .unwrap();
    drop(outlet_inner_for_page);
    RouteOutlet::attach_nested(&outlet_outer, &outlet_inner).expect("nested attaches");
    let recovered = present_outer_tree_output(&mut runtime, &outlet_outer);
    assert!(paints(recovered.commands(), YELLOW));
    assert!(!outlet_inner.borrow().needs_frame());
    assert!(
        !runtime.frame_requested(),
        "consumed work schedules nothing further"
    );
    let _ = outer;
}

#[test]
fn outlet_panicking_present_resets_schedule_suppression() {
    // Same shape through a panic instead of a typed error: the frame
    // unwinds (resetting the memo on its way out), the slotless follow-up
    // executes without consuming the inner revision yet re-flags it, and
    // remounting converges and idles.
    let (outer, inner, mut runtime, outlet_outer, outlet_inner, _node_a, _node_b) =
        nested_inside_setup();
    let trip = Rc::new(Cell::new(true));
    inner.push_page(Page::new(
        "ib",
        Column::new(vec![Widget::from(LayoutBuilder::new({
            let inner = inner.clone();
            move |_, _| {
                if trip.take() {
                    inner.push_page(Page::new("ic", Widget::box_(Size::new(40., 40.), YELLOW)));
                }
                Widget::box_(Size::new(40., 40.), BLUE)
            }
        }))]),
    ));
    present_outer_tree(&mut runtime, &outlet_outer);
    assert!(
        runtime.frame_requested(),
        "mid-frame inner navigation schedules a follow-up"
    );
    let panic_trip = Rc::new(Cell::new(true));
    outer.push_page(Page::new(
        "d",
        Column::new(vec![Widget::from(LayoutBuilder::new(move |_, _| {
            if panic_trip.take() {
                panic!("schedule boom");
            }
            Widget::box_(Size::new(40., 40.), GREEN)
        }))]),
    ));
    let outlet = outlet_outer.clone();
    let result = catch_unwind(AssertUnwindSafe(|| {
        RouteOutlet::present_frame(
            &outlet,
            &mut runtime,
            Constraints::tight(Size::new(200., 200.)),
        )
    }));
    result.expect_err("builder panic unwinds the frame");
    drop(outlet);
    outer
        .set_pages([Page::new("solo", Widget::box_(Size::new(40., 40.), RED))])
        .unwrap();
    present_outer_tree(&mut runtime, &outlet_outer);
    assert!(
        outlet_inner.borrow().needs_frame(),
        "inner work is still outstanding"
    );
    assert!(
        runtime.frame_requested(),
        "the panic reset lets the outstanding revision schedule again"
    );
    let outlet_inner_for_page = outlet_inner.clone();
    outer
        .set_pages([Page::new(
            "a",
            Column::new(vec![
                Widget::box_(Size::new(40., 40.), RED),
                RouteOutlet::nested_widget(&outlet_inner_for_page),
            ]),
        )])
        .unwrap();
    drop(outlet_inner_for_page);
    RouteOutlet::attach_nested(&outlet_outer, &outlet_inner).expect("nested attaches");
    let recovered = present_outer_tree_output(&mut runtime, &outlet_outer);
    assert!(paints(recovered.commands(), YELLOW));
    assert!(!runtime.frame_requested());
    let _ = outer;
}

#[test]
fn outlet_scheduling_tracks_participation() {
    // Participation through actual scheduler state (no focus anywhere,
    // so only outlet scheduling can flag): a stale slotless revision
    // flags once; the identical follow-up stays silent; detaching
    // freezes scheduling while retaining the deferred revision;
    // remounting consumes it and idles — all in one present each.
    let outer = Navigator::new();
    let inner = Navigator::new();
    let mut runtime =
        Runtime::new(Column::new(vec![Widget::box_(Size::new(200., 200.), Color::WHITE)]).into())
            .unwrap();
    let parent = runtime.spawner().scope();
    let outlet_outer = Rc::new(RefCell::new(RouteOutlet::new(&outer, &parent)));
    let outlet_inner = Rc::new(RefCell::new(RouteOutlet::new(&inner, &parent)));
    let outlet_inner_for_page = outlet_inner.clone();
    outer.push_page(Page::new(
        "a",
        Column::new(vec![
            Widget::box_(Size::new(40., 40.), RED),
            RouteOutlet::nested_widget(&outlet_inner_for_page),
        ]),
    ));
    drop(outlet_inner_for_page);
    inner.push_page(Page::new("b", Widget::box_(Size::new(40., 40.), BLUE)));
    let root = runtime.tree().root().expect("root");
    {
        let outlet_outer = outlet_outer.clone();
        runtime
            .register_builder(root, move || {
                Column::new(vec![SizedBox::from_dimensions(
                    Some(200.),
                    Some(200.),
                    Some(outlet_outer.borrow().widget()),
                )])
                .into()
            })
            .expect("builder registers");
    }
    frame(&mut runtime);
    RouteOutlet::attach(&outlet_outer, &mut runtime).expect("outer mounted");
    RouteOutlet::attach_nested(&outlet_outer, &outlet_inner).expect("nested attaches");
    // Baseline idle: everything composed, nothing scheduled.
    present_outer_tree(&mut runtime, &outlet_outer);
    assert!(!outlet_outer.borrow().needs_frame());
    assert!(!outlet_inner.borrow().needs_frame());
    assert!(!runtime.frame_requested(), "converged tree idles");
    // Slotless with inner work outstanding: first success flags...
    outer
        .set_pages([Page::new(
            "solo",
            Widget::box_(Size::new(200., 200.), GREEN),
        )])
        .unwrap();
    inner.push_page(Page::new("c", Widget::box_(Size::new(40., 40.), YELLOW)));
    let stale = present_outer_tree_output(&mut runtime, &outlet_outer);
    assert!(paints(stale.commands(), GREEN));
    assert!(!paints(stale.commands(), YELLOW));
    assert!(outlet_inner.borrow().needs_frame());
    assert!(
        runtime.frame_requested(),
        "runnable stale content retains frame demand"
    );
    let ic = inner.current().expect("inner route").id;
    assert!(
        outlet_inner.borrow().route_task_scope(ic).is_some(),
        "unmounted-but-live routes still bind"
    );
    // ...the identical follow-up stays silent: no spin.
    present_outer_tree(&mut runtime, &outlet_outer);
    assert!(outlet_inner.borrow().needs_frame());
    assert!(
        !runtime.frame_requested(),
        "stable staleness does not re-flag"
    );
    // Detaching freezes scheduling while retaining deferred work.
    RouteOutlet::detach_nested(&outlet_outer, &outlet_inner);
    let inner_drives = outlet_inner.borrow().frame_drive_count();
    inner.push_page(Page::new("d", Widget::box_(Size::new(40., 40.), BLUE)));
    present_outer_tree(&mut runtime, &outlet_outer);
    assert_eq!(
        outlet_inner.borrow().frame_drive_count(),
        inner_drives,
        "detached outlets receive no frame work"
    );
    assert!(outlet_inner.borrow().needs_frame());
    assert!(
        !runtime.frame_requested(),
        "nonparticipating outlets schedule nothing"
    );
    // Reactivation consumes promptly in a single present, then idles.
    RouteOutlet::attach_nested(&outlet_outer, &outlet_inner).expect("nested reattaches");
    let outlet_inner_for_page = outlet_inner.clone();
    outer
        .set_pages([Page::new(
            "a2",
            Column::new(vec![
                Widget::box_(Size::new(40., 40.), RED),
                RouteOutlet::nested_widget(&outlet_inner_for_page),
            ]),
        )])
        .unwrap();
    drop(outlet_inner_for_page);
    let remounted = present_outer_tree_output(&mut runtime, &outlet_outer);
    assert!(paints(remounted.commands(), RED));
    assert!(paints(remounted.commands(), BLUE));
    assert!(!paints(remounted.commands(), GREEN));
    assert!(!outlet_inner.borrow().needs_frame());
    assert!(!outlet_outer.borrow().needs_frame());
    assert!(!runtime.frame_requested(), "consumed revision idles");
    let _ = outer;
}

#[test]
fn outlet_preflight_failure_preserves_attempt_state() {
    // Failure before any attempt state: a stale-flagged tree takes an
    // overlay push, and the present fails pre-capture — no drives, no
    // capture disturbance, nothing scheduled. Popping the offender
    // recovers through the kept capture with the original save.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let node_a = FocusNode::new();
    navigator.push_page(focus_page("a", &node_a));
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    // Stale work first: a mid-frame push the frame cannot present.
    let trip = Rc::new(Cell::new(true));
    navigator.push_page(Page::new(
        "b",
        Column::new(vec![Widget::from(LayoutBuilder::new({
            let navigator = navigator.clone();
            move |_, _| {
                if trip.take() {
                    navigator.push_page(plain_page("c"));
                }
                Widget::box_(Size::new(40., 40.), BLUE)
            }
        }))]),
    ));
    present(&mut harness);
    assert!(
        harness.runtime.frame_requested(),
        "stale output schedules its follow-up"
    );
    // Overlay push, then a present that fails before capture or drives.
    navigator.push(
        Route::new("overlay", Widget::box_(Size::new(40., 40.), GREEN)).overlay(vec![
            OverlayEntry::new(Widget::box_(Size::new(40., 40.), GREEN)),
        ]),
    );
    let drives = harness.outlet.borrow().frame_drive_count();
    let error = RouteOutlet::present_frame(
        &harness.outlet,
        &mut harness.runtime,
        Constraints::tight(Size::new(200., 200.)),
    )
    .expect_err("overlay stacks fail pre-capture");
    assert!(matches!(error, OutletError::UnsupportedPresentation { .. }));
    assert_eq!(
        harness.outlet.borrow().frame_drive_count(),
        drives,
        "preflight failure drives nothing"
    );
    assert!(node_a.has_focus());
    // Recovery commits the kept capture with the original save intact.
    navigator.pop();
    present(&mut harness);
    navigator.pop();
    navigator.pop();
    present(&mut harness);
    assert!(node_a.has_focus());
}

#[test]
fn outlet_retry_after_below_removal_commits_promptly() {
    // A→B captured, the frame fails, then a covered route is removed
    // below (same active route, smaller member set): the retry builds the
    // current stack, so it must commit promptly against the refreshed
    // snapshot — not abandon against the stale one. Proved by the return:
    // only a P2 commit leaves the record the way back restores.
    let navigator = Navigator::new();
    let mut harness = harness(&navigator);
    let kx = PageKey::new("x").unwrap();
    let ka = PageKey::new("a").unwrap();
    let kb = PageKey::new("b").unwrap();
    let node_a = FocusNode::new();
    let node_b = FocusNode::new();
    navigator
        .set_pages([
            Page::new("x", Widget::box_(Size::new(40., 40.), RED)).key(kx.clone()),
            Page::new("a", focus_child(&node_a)).key(ka.clone()),
        ])
        .unwrap();
    present(&mut harness);
    tab_until(&mut harness.runtime, &node_a);
    let trip = Rc::new(Cell::new(true));
    let node_b_for_build = node_b.clone();
    navigator.push_page(
        Page::new(
            "b",
            Column::new(vec![Widget::from(LayoutBuilder::new(move |_, _| {
                if trip.take() {
                    panic!("retry boom");
                }
                focus_widget(&node_b_for_build, Widget::box_(Size::new(40., 40.), BLUE))
            }))]),
        )
        .key(kb.clone()),
    );
    let outlet = harness.outlet.clone();
    let result = catch_unwind(AssertUnwindSafe(|| {
        RouteOutlet::present_frame(
            &outlet,
            &mut harness.runtime,
            Constraints::tight(Size::new(200., 200.)),
        )
    }));
    result.expect_err("builder panic unwinds the frame");
    // The failed composition publishes nothing: newer work is still owed
    // even though builders ran before the panic.
    assert!(
        harness.outlet.borrow().needs_frame(),
        "a failed frame leaves the previous publication intact"
    );
    // Same active route, smaller stack: X goes away below the transition.
    navigator
        .set_pages([
            Page::new("a", focus_child(&node_a)).key(ka.clone()),
            Page::new("b", focus_child(&node_b)).key(kb.clone()),
        ])
        .unwrap();
    present(&mut harness);
    assert!(!harness.outlet.borrow().needs_frame());
    // Move focus away: only a promptly committed P2 leaves the record the
    // way back restores (an abandon leaves nothing behind).
    tab_until(&mut harness.runtime, &node_b);
    navigator.pop();
    present(&mut harness);
    assert!(
        node_a.has_focus(),
        "the way back restores the promptly committed save"
    );
}
