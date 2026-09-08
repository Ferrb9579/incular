//! Retained snap-to-edge for floating headers.
//!
//! Snapping starts only from actual scroll-activity ends observed through the
//! controller's notification stream, advances on compositor ticks with
//! deterministic test clocks, and animates presentation only: the logical
//! scroll extent and controller offset never move for it. A new activity
//! interrupts running and settling work even before the offset moves, while
//! preserving the current presentation.

use std::time::{Duration, Instant};

use incular_config::{Axis, Constraints, RuntimeEnvironment};
use incular_core::{Color, Offset, Size};
use incular_scroll::{ScrollController, ScrollPhysics};
use incular_widgets::{
    CustomScrollView, Sliver, SliverFloatingHeader, SliverHeaderScrollBehavior,
    SliverNaturalHeader, SliverResizingHeader, SliverToBoxAdapter, Widget,
    internal::{ElementId, WidgetTree},
};

/// Scrolls by `delta` as one complete user activity, mirroring the tree's
/// wheel adapter: begin, apply, then end. Only the end starts a snap.
fn end_scroll_by(controller: &ScrollController, delta: f32) {
    controller.notify_user_scroll(delta);
    let _ = controller.apply_physics(ScrollPhysics::default(), delta);
    controller.end_activity();
}

/// Runs one layout plus compositor pass at a deterministic timestamp.
/// Returns `(changed, animations_active)` from the compositor pass.
fn pump_frame(tree: &mut WidgetTree, constraints: Constraints, now: Instant) -> (bool, bool) {
    tree.layout(constraints).expect("layout");
    tree.update_compositor(now).expect("compositor")
}

fn viewport_with_header(
    header: Box<dyn Sliver>,
    controller: ScrollController,
    axis: Axis,
    reverse: bool,
    physics: ScrollPhysics,
) -> (WidgetTree, ElementId) {
    let slivers: Vec<Box<dyn Sliver>> = vec![
        header,
        Box::new(SliverToBoxAdapter::new(Widget::box_(
            axis.size(800., 200.),
            Color::BLACK,
        ))),
    ];
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            CustomScrollView::new(slivers)
                .controller(controller)
                .scroll_direction(axis)
                .reverse(reverse)
                .physics(physics)
                .into(),
        )
        .expect("mount");
    (tree, root)
}

fn header_child(tree: &WidgetTree, root: ElementId) -> ElementId {
    tree.children(root).expect("header")[0]
}

/// White header rects as recorded (unclipped; viewport clips separately).
fn white_rects(tree: &mut WidgetTree) -> Vec<(Offset, Size)> {
    use incular_rendering::PaintCommand;
    tree.paint()
        .commands()
        .iter()
        .filter_map(|command| match command {
            PaintCommand::Rect { rect, color } if *color == Color::WHITE => {
                Some((rect.origin, rect.size))
            }
            _ => None,
        })
        .collect()
}

fn resizing_floating_header() -> Box<dyn Sliver> {
    Box::new(
        SliverResizingHeader::new(60., 120., Widget::box_(Size::new(200., 120.), Color::WHITE))
            .scroll_behavior(SliverHeaderScrollBehavior::Floating)
            .snap(true),
    ) as Box<dyn Sliver>
}

#[test]
fn snap_reveals_partially_visible_floating_header() {
    let controller = ScrollController::new();
    let (mut tree, root) = viewport_with_header(
        resizing_floating_header(),
        controller.clone(),
        Axis::Vertical,
        false,
        ScrollPhysics::default(),
    );
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let render = tree.render_id(header_child(&tree, root)).expect("render");
    assert_eq!(tree.render_size(render), Some(Size::new(200., 120.)));

    // Hide fully, then return until 60 of 120 are showing: at the
    // midpoint the snap must choose the revealed edge.
    end_scroll_by(&controller, 400.);
    tree.layout(constraints).expect("hidden layout");
    end_scroll_by(&controller, -60.);
    tree.layout(constraints).expect("partial layout");
    assert_eq!(tree.render_size(render), Some(Size::new(200., 60.)));

    let start = Instant::now();
    let (_, active) = pump_frame(&mut tree, constraints, start);
    assert!(active, "a started snap must schedule frames");
    // The start tick adopts the run without moving presentation yet.
    assert_eq!(tree.render_size(render), Some(Size::new(200., 60.)));

    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(150));
    assert!(active, "frames continue mid-snap");
    assert!(changed, "mid-snap ticks must invalidate presentation");
    // The pump's layout predates its tick; one more layout presents the
    // tick's state without advancing the clock.
    tree.layout(constraints).expect("mid layout");
    let mid = tree.render_size(render).expect("header size").height;
    assert!(
        (60.0..120.0).contains(&mid) && mid != 60. && mid != 120.,
        "mid-snap presentation must lie strictly between: {mid}"
    );

    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(300));
    assert!(changed, "the completion tick must apply the endpoint");
    assert!(active, "one settle frame stays scheduled past completion");
    assert_eq!(controller.offset(), 340.);
    assert_eq!(controller.content_extent(), 920.);

    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(400));
    assert!(!changed, "the settled endpoint needs no further work");
    assert!(!active, "frames must stop after completion");
    assert_eq!(
        tree.render_size(render),
        Some(Size::new(200., 120.)),
        "snap must complete exactly at the revealed edge"
    );
}

#[test]
fn snap_reveals_partially_visible_floating_header_when_reversed() {
    let controller = ScrollController::new();
    let (mut tree, root) = viewport_with_header(
        resizing_floating_header(),
        controller.clone(),
        Axis::Vertical,
        true,
        ScrollPhysics::default(),
    );
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let render = tree.render_id(header_child(&tree, root)).expect("render");

    // Reversed offset zero shows the content end, so pin the known hidden
    // state first (first-frame anchor correction may have moved the offset
    // while learning the range). Scrolling toward the leading edge then
    // reveals 90 of 120: above the midpoint, so the snap must choose the
    // revealed edge.
    controller.begin_activity();
    controller.jump_to(0.);
    controller.end_activity();
    tree.layout(constraints).expect("pinned layout");
    assert_eq!(tree.render_size(render), Some(Size::new(200., 60.)));
    end_scroll_by(&controller, 90.);
    tree.layout(constraints).expect("partial layout");
    assert_eq!(tree.render_size(render), Some(Size::new(200., 90.)));

    let start = Instant::now();
    let (_, active) = pump_frame(&mut tree, constraints, start);
    assert!(active, "a started snap must schedule frames");
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(300));
    assert!(changed, "the completion tick must apply the endpoint");
    assert!(active, "one settle frame stays scheduled past completion");
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(400));
    assert!(!changed, "the settled endpoint needs no further work");
    assert!(!active, "frames must stop after completion");
    assert_eq!(
        tree.render_size(render),
        Some(Size::new(200., 120.)),
        "reversed snap must complete exactly at the revealed edge"
    );
    assert_eq!(controller.offset(), 90.);
    assert_eq!(controller.content_extent(), 920.);
}

#[test]
fn snap_hides_partially_visible_floating_header() {
    let controller = ScrollController::new();
    let (mut tree, root) = viewport_with_header(
        resizing_floating_header(),
        controller.clone(),
        Axis::Vertical,
        false,
        ScrollPhysics::default(),
    );
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let render = tree.render_id(header_child(&tree, root)).expect("render");

    // Return until only 30 of 120 show: below the midpoint the snap must
    // choose the hidden edge. The child stays clamped at its 60 minimum
    // while paint shrinks to zero, so observe paint and hit testing. The
    // overlay picture needs its compositor pass, which also starts the run
    // without moving presentation yet.
    end_scroll_by(&controller, 400.);
    tree.layout(constraints).expect("hidden layout");
    end_scroll_by(&controller, -30.);
    tree.layout(constraints).expect("partial layout");

    let start = Instant::now();
    let (_, active) = pump_frame(&mut tree, constraints, start);
    assert!(active);
    // Overlay paint pins the visible part to the viewport top: the 60-tall
    // child paints at the origin while only 30 show through the clip.
    assert_eq!(
        white_rects(&mut tree),
        vec![(Offset::ZERO, Size::new(200., 60.))]
    );
    assert_eq!(
        tree.hit_test(Offset::new(100., 20.)),
        Some(render),
        "partial header must hit"
    );
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(150));
    assert!(active);
    assert!(changed);

    // Reads lag one pump behind ticks: this layout presents the +150 tick,
    // whose 7.5 paint covers y=5 but no longer y=20.
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(300));
    assert!(changed);
    assert!(active, "one settle frame stays scheduled past completion");
    assert_eq!(
        tree.hit_test(Offset::new(100., 5.)),
        Some(render),
        "mid-snap header must still hit near the edge"
    );
    assert_ne!(
        tree.hit_test(Offset::new(100., 20.)),
        Some(render),
        "mid-snap header must stop hitting further down"
    );

    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(400));
    assert!(!changed);
    assert!(!active, "frames must stop after completion");
    assert_eq!(
        white_rects(&mut tree),
        Vec::<(Offset, Size)>::new(),
        "hidden snap must paint nothing"
    );
    assert_ne!(
        tree.hit_test(Offset::new(100., 20.)),
        Some(render),
        "hidden header must not hit"
    );
    // Logical extent and offset never moved for the animation.
    assert_eq!(controller.offset(), 370.);
    assert_eq!(controller.content_extent(), 920.);
}

#[test]
fn snap_moves_only_the_collapsible_range_of_floating_pinned_headers() {
    for (deltas, final_height) in [(vec![400., -15.], 60.), (vec![400., -30.], 120.)] {
        let controller = ScrollController::new();
        let (mut tree, root) = viewport_with_header(
            Box::new(
                SliverResizingHeader::new(
                    60.,
                    120.,
                    Widget::box_(Size::new(200., 120.), Color::WHITE),
                )
                .scroll_behavior(SliverHeaderScrollBehavior::FloatingPinned)
                .snap(true),
            ) as Box<dyn Sliver>,
            controller.clone(),
            Axis::Vertical,
            false,
            ScrollPhysics::default(),
        );
        let constraints = Constraints::tight(Size::new(200., 200.));
        tree.layout(constraints).expect("layout");
        let render = tree.render_id(header_child(&tree, root)).expect("render");

        for delta in deltas {
            end_scroll_by(&controller, delta);
            tree.layout(constraints).expect("scroll layout");
        }

        let start = Instant::now();
        let (_, active) = pump_frame(&mut tree, constraints, start);
        assert!(active);
        let (changed, active) =
            pump_frame(&mut tree, constraints, start + Duration::from_millis(150));
        assert!(active);
        assert!(changed);
        // The pump's layout predates its tick; one more layout presents the
        // tick's state without advancing the clock.
        tree.layout(constraints).expect("mid layout");
        let mid = tree.render_size(render).expect("header size").height;
        assert!(
            (60.0..120.0).contains(&mid) && mid != final_height,
            "mid-snap height must travel strictly inside the collapse range: {mid}"
        );
        let (changed, active) =
            pump_frame(&mut tree, constraints, start + Duration::from_millis(300));
        assert!(changed);
        assert!(active, "one settle frame stays scheduled past completion");
        let (changed, active) =
            pump_frame(&mut tree, constraints, start + Duration::from_millis(400));
        assert!(!changed);
        assert!(!active);
        assert_eq!(
            tree.render_size(render),
            Some(Size::new(200., final_height)),
            "snap must settle exactly at the chosen range edge"
        );
    }
}

#[test]
fn snap_starts_no_run_when_already_maximally_hidden_for_its_offset() {
    // Scroll-driven presentation clamps hidden distance to the scrolled
    // distance, so at offset 45 a 60-range collapse can never reach 60: the
    // 75 presentation is already the hidden edge for that offset. Ending the
    // scroll there must schedule no frames and move nothing.
    let controller = ScrollController::new();
    let (mut tree, root) = viewport_with_header(
        Box::new(
            SliverResizingHeader::new(60., 120., Widget::box_(Size::new(200., 120.), Color::WHITE))
                .scroll_behavior(SliverHeaderScrollBehavior::FloatingPinned)
                .snap(true),
        ) as Box<dyn Sliver>,
        controller.clone(),
        Axis::Vertical,
        false,
        ScrollPhysics::default(),
    );
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let render = tree.render_id(header_child(&tree, root)).expect("render");
    // Absorb first-compositor setup so later pumps are quiet by default.
    let start = Instant::now();
    let _ = pump_frame(&mut tree, constraints, start);
    end_scroll_by(&controller, 45.);
    tree.layout(constraints).expect("partial layout");
    assert_eq!(tree.render_size(render), Some(Size::new(200., 75.)));

    // The scroll's own transform change is absorbed here; the end trigger is
    // consumed with it and decides idle, so the confirming pump stays quiet.
    let _ = pump_frame(&mut tree, constraints, start + Duration::from_millis(100));
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(200));
    assert!(!active, "an infeasible edge must schedule no frames");
    assert!(!changed, "an infeasible edge must move nothing");
    let (_, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(500));
    assert!(!active);
    assert_eq!(tree.render_size(render), Some(Size::new(200., 75.)));
}

#[test]
fn snap_natural_header_follows_its_measured_extent() {
    use incular_semantics::SemanticRole;
    use incular_widgets::Semantics;

    let controller = ScrollController::new();
    let child: Widget = Semantics::new(Widget::box_(Size::new(200., 55.), Color::WHITE))
        .role(SemanticRole::GenericContainer)
        .label("snap-row")
        .into();
    let (mut tree, root) = viewport_with_header(
        Box::new(
            SliverNaturalHeader::new(child)
                .scroll_behavior(SliverHeaderScrollBehavior::Floating)
                .snap(true),
        ) as Box<dyn Sliver>,
        controller.clone(),
        Axis::Vertical,
        false,
        ScrollPhysics::default(),
    );
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let render = tree.render_id(header_child(&tree, root)).expect("render");
    assert_eq!(tree.render_size(render), Some(Size::new(200., 55.)));

    end_scroll_by(&controller, 400.);
    tree.layout(constraints).expect("hidden layout");
    // Reveal 40 of the measured 55: above the midpoint, so snap reveals.
    // The overlay picture needs its compositor pass, which also starts the
    // run without moving presentation yet.
    end_scroll_by(&controller, -40.);
    tree.layout(constraints).expect("partial layout");

    let start = Instant::now();
    let (_, active) = pump_frame(&mut tree, constraints, start);
    assert!(active);
    // Overlay paint pins the visible part to the viewport top: the 55-tall
    // child paints at the origin while 40 show through the clip.
    assert_eq!(
        white_rects(&mut tree),
        vec![(Offset::ZERO, Size::new(200., 55.))]
    );
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(300));
    assert!(changed);
    assert!(active, "one settle frame stays scheduled past completion");
    assert_eq!(controller.content_extent(), 855.);
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(400));
    assert!(!changed);
    assert!(!active);
    assert_eq!(
        white_rects(&mut tree),
        vec![(Offset::ZERO, Size::new(200., 55.))],
        "natural snap must complete at the measured extent"
    );
    tree.update_semantics();
    let (_, bounds) = tree
        .semantics()
        .iter()
        .find_map(|(id, node)| {
            (node.label.as_deref() == Some("snap-row")).then_some((id, node.bounds))
        })
        .expect("row semantics");
    assert_eq!(bounds.origin, Offset::ZERO);
    assert_eq!(bounds.size, Size::new(200., 55.));
}

fn floating_snap_viewport(controller: ScrollController, child: Widget) -> (WidgetTree, ElementId) {
    viewport_with_header(
        Box::new(SliverFloatingHeader::new(child).snap(true)) as Box<dyn Sliver>,
        controller,
        Axis::Vertical,
        false,
        ScrollPhysics::default(),
    )
}

#[test]
fn snap_fixed_floating_header_survives_compatible_descriptor_replacement() {
    let controller = ScrollController::new();
    let (mut tree, root) = floating_snap_viewport(
        controller.clone(),
        Widget::box_(Size::new(200., 70.), Color::WHITE),
    );
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let render = tree.render_id(header_child(&tree, root)).expect("render");
    assert_eq!(tree.render_size(render), Some(Size::new(200., 70.)));

    // Return until 20 of 70 show: below the midpoint, so snap hides. The
    // overlay picture needs its compositor pass, which also starts the run
    // without moving presentation yet.
    end_scroll_by(&controller, 400.);
    tree.layout(constraints).expect("hidden layout");
    end_scroll_by(&controller, -20.);
    tree.layout(constraints).expect("partial layout");

    let start = Instant::now();
    let (_, active) = pump_frame(&mut tree, constraints, start);
    assert!(active);
    // Overlay paint pins the visible part to the viewport top: the 70-tall
    // child paints at the origin while 20 show through the clip.
    assert_eq!(
        white_rects(&mut tree),
        vec![(Offset::ZERO, Size::new(200., 70.))]
    );
    let _ = pump_frame(&mut tree, constraints, start + Duration::from_millis(100));
    assert_eq!(controller.offset(), 380.);

    // Replace the descriptor with identical content mid-snap. The running
    // snap transfers with the adopted reversal tracking and completes
    // against the unchanged extent instead of sticking or jumping.
    let slivers: Vec<Box<dyn Sliver>> = vec![
        Box::new(
            SliverFloatingHeader::new(Widget::box_(Size::new(200., 70.), Color::WHITE)).snap(true),
        ),
        Box::new(SliverToBoxAdapter::new(Widget::box_(
            Size::new(200., 800.),
            Color::BLACK,
        ))),
    ];
    tree.update(
        root,
        CustomScrollView::new(slivers)
            .controller(controller.clone())
            .into(),
    )
    .expect("update");
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(400));
    assert!(changed);
    assert!(active, "one settle frame stays scheduled past completion");
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(500));
    assert!(!changed);
    assert!(!active, "frames must stop after a transferred completion");
    assert_eq!(
        white_rects(&mut tree),
        Vec::<(Offset, Size)>::new(),
        "hide must complete after replacement"
    );
    assert_eq!(controller.offset(), 380.);
    let render = tree.render_id(header_child(&tree, root)).expect("render");
    assert_eq!(tree.render_size(render), Some(Size::new(200., 70.)));
}

#[test]
fn snap_yields_coherently_when_replacement_moves_the_scroll_range() {
    let controller = ScrollController::new();
    let (mut tree, root) = floating_snap_viewport(
        controller.clone(),
        Widget::box_(Size::new(200., 70.), Color::WHITE),
    );
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");

    // Start hiding, then shrink the content mid-snap. The range change moves
    // the retained offset through anchor correction, which cancels the run
    // like any offset movement; presentation must stay coherent and stop.
    end_scroll_by(&controller, 400.);
    tree.layout(constraints).expect("hidden layout");
    end_scroll_by(&controller, -20.);
    tree.layout(constraints).expect("partial layout");

    let start = Instant::now();
    let (_, active) = pump_frame(&mut tree, constraints, start);
    assert!(active);
    let _ = pump_frame(&mut tree, constraints, start + Duration::from_millis(100));

    let slivers: Vec<Box<dyn Sliver>> = vec![
        Box::new(
            SliverFloatingHeader::new(Widget::box_(Size::new(200., 40.), Color::WHITE)).snap(true),
        ),
        Box::new(SliverToBoxAdapter::new(Widget::box_(
            Size::new(200., 800.),
            Color::BLACK,
        ))),
    ];
    tree.update(
        root,
        CustomScrollView::new(slivers)
            .controller(controller.clone())
            .into(),
    )
    .expect("update");
    let _ = pump_frame(&mut tree, constraints, start + Duration::from_millis(400));
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(500));
    assert!(!changed, "the cancelled run must schedule nothing further");
    assert!(!active, "frames must stop after range-change cancellation");
    assert_eq!(controller.max_offset(), 640.);
    assert!(
        (0.0..=controller.max_offset()).contains(&controller.offset()),
        "the corrected offset must stay in range"
    );
    assert_eq!(
        white_rects(&mut tree),
        Vec::<(Offset, Size)>::new(),
        "the header must rest hidden, not flash revealed"
    );

    // The machinery survives: a new scroll end snaps again from the current
    // feasible presentation.
    end_scroll_by(&controller, -30.);
    tree.layout(constraints).expect("reveal layout");
    let restart = Instant::now();
    let (_, active) = pump_frame(&mut tree, constraints, restart);
    assert!(active, "a new end must start a fresh snap");
    let (changed, active) =
        pump_frame(&mut tree, constraints, restart + Duration::from_millis(300));
    assert!(changed);
    assert!(active);
    let (changed, active) =
        pump_frame(&mut tree, constraints, restart + Duration::from_millis(400));
    assert!(!changed);
    assert!(!active);
    assert_eq!(
        white_rects(&mut tree),
        vec![(Offset::ZERO, Size::new(200., 40.))],
        "the fresh snap must complete revealed at the new extent"
    );
}

#[test]
fn snap_stays_inert_without_floating_or_without_the_flag() {
    // Neutral policy: snap engages only for Floating/FloatingPinned. A pinned
    // header with snap set, and floating headers without snap set, must both
    // sit still after scroll ends.
    let cases: Vec<Box<dyn Fn() -> Box<dyn Sliver>>> = vec![
        Box::new(|| {
            Box::new(
                SliverResizingHeader::new(
                    60.,
                    120.,
                    Widget::box_(Size::new(200., 120.), Color::WHITE),
                )
                .scroll_behavior(SliverHeaderScrollBehavior::Pinned)
                .snap(true),
            ) as Box<dyn Sliver>
        }),
        Box::new(|| {
            Box::new(
                SliverResizingHeader::new(
                    60.,
                    120.,
                    Widget::box_(Size::new(200., 120.), Color::WHITE),
                )
                .scroll_behavior(SliverHeaderScrollBehavior::Floating),
            ) as Box<dyn Sliver>
        }),
        Box::new(|| {
            Box::new(
                SliverNaturalHeader::new(Widget::box_(Size::new(200., 55.), Color::WHITE))
                    .scroll_behavior(SliverHeaderScrollBehavior::Floating),
            ) as Box<dyn Sliver>
        }),
    ];
    for make_header in cases {
        let controller = ScrollController::new();
        let (mut tree, _root) = viewport_with_header(
            make_header(),
            controller.clone(),
            Axis::Vertical,
            false,
            ScrollPhysics::default(),
        );
        let constraints = Constraints::tight(Size::new(200., 200.));
        tree.layout(constraints).expect("layout");
        // Absorb first-compositor setup so later pumps are quiet by default.
        let start = Instant::now();
        let _ = pump_frame(&mut tree, constraints, start);
        end_scroll_by(&controller, 400.);
        tree.layout(constraints).expect("hidden layout");
        end_scroll_by(&controller, -30.);
        tree.layout(constraints).expect("partial layout");
        // The scroll's own transform change lands on the next compositor;
        // absorb it (the end trigger is consumed idle with it), then confirm.
        let _ = pump_frame(&mut tree, constraints, start + Duration::from_millis(100));
        let (changed, active) =
            pump_frame(&mut tree, constraints, start + Duration::from_millis(200));
        assert!(!active, "no snap may schedule frames");
        assert!(!changed, "no snap may move presentation");
        let (_, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(500));
        assert!(!active);
    }
}

#[test]
fn snap_interruption_continues_smoothly_and_reverses_on_the_next_end() {
    let controller = ScrollController::new();
    let (mut tree, root) = viewport_with_header(
        resizing_floating_header(),
        controller.clone(),
        Axis::Vertical,
        false,
        ScrollPhysics::default(),
    );
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let render = tree.render_id(header_child(&tree, root)).expect("render");

    // Partially reveal, then snap toward revealed.
    end_scroll_by(&controller, 400.);
    tree.layout(constraints).expect("hidden layout");
    end_scroll_by(&controller, -60.);
    tree.layout(constraints).expect("partial layout");
    let start = Instant::now();
    let _ = pump_frame(&mut tree, constraints, start);
    let _ = pump_frame(&mut tree, constraints, start + Duration::from_millis(150));
    tree.layout(constraints).expect("mid layout");
    let interrupted = tree.render_size(render).expect("header size").height;
    assert!(
        (60.0..120.0).contains(&interrupted) && interrupted != 60. && interrupted != 120.,
        "mid-snap presentation must lie strictly between: {interrupted}"
    );

    // New scroll movement interrupts from the current presentation: the next
    // layout must continue from the animated value, not jump.
    controller.notify_user_scroll(20.);
    assert!(controller.scroll_by(20.));
    tree.layout(constraints).expect("interrupted layout");
    let resumed = tree.render_size(render).expect("header size").height;
    assert!(
        resumed < interrupted,
        "scrolling down must hide further from the animated value"
    );
    assert!(
        (interrupted - resumed - 20.).abs() <= 1.,
        "interruption must continue from current presentation: {interrupted} -> {resumed}"
    );
    // Programmatic movement alone never starts a snap.
    let (_, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(200));
    assert!(!active, "programmatic scroll must not start a snap");

    // Scroll back down past the midpoint and end: the next snap hides.
    controller.notify_user_scroll(40.);
    assert!(controller.scroll_by(40.));
    controller.end_activity();
    tree.layout(constraints).expect("reversed layout");
    let restart = Instant::now();
    let (_, active) = pump_frame(&mut tree, constraints, restart);
    assert!(active, "the next end must start a fresh snap");
    let (changed, active) =
        pump_frame(&mut tree, constraints, restart + Duration::from_millis(300));
    assert!(changed);
    assert!(active, "one settle frame stays scheduled past completion");
    let (changed, active) =
        pump_frame(&mut tree, constraints, restart + Duration::from_millis(400));
    assert!(!changed);
    assert!(!active);
    assert_eq!(
        white_rects(&mut tree),
        Vec::<(Offset, Size)>::new(),
        "reversed snap must complete hidden"
    );
}

#[test]
fn snap_repeated_cycles_settle_exactly() {
    let controller = ScrollController::new();
    let (mut tree, root) = viewport_with_header(
        Box::new(
            SliverResizingHeader::new(60., 120., Widget::box_(Size::new(200., 120.), Color::WHITE))
                .scroll_behavior(SliverHeaderScrollBehavior::FloatingPinned)
                .snap(true),
        ) as Box<dyn Sliver>,
        controller.clone(),
        Axis::Vertical,
        false,
        ScrollPhysics::default(),
    );
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let render = tree.render_id(header_child(&tree, root)).expect("render");
    let start = Instant::now();

    // Collapse, expand, collapse again: every cycle must land exactly.
    for (cycle, (deltas, edge)) in [
        (0, (vec![400., -15.], 60.)),
        (1, (vec![-30.], 120.)),
        (2, (vec![45.], 60.)),
    ] {
        for delta in deltas {
            end_scroll_by(&controller, delta);
            tree.layout(constraints).expect("scroll layout");
        }
        let base = start + Duration::from_millis(cycle * 1000);
        let (_, active) = pump_frame(&mut tree, constraints, base);
        assert!(active, "cycle {cycle} must animate");
        let (changed, active) =
            pump_frame(&mut tree, constraints, base + Duration::from_millis(300));
        assert!(changed, "cycle {cycle} must apply its endpoint");
        assert!(active, "cycle {cycle} keeps one settle frame");
        let (changed, active) =
            pump_frame(&mut tree, constraints, base + Duration::from_millis(400));
        assert!(!changed, "cycle {cycle} must need no further work");
        assert!(!active, "cycle {cycle} must stop scheduling frames");
        assert_eq!(
            tree.render_size(render),
            Some(Size::new(200., edge)),
            "cycle {cycle} must settle exactly"
        );
    }
    assert_eq!(controller.content_extent(), 920.);
}

#[test]
fn snap_does_not_start_while_stretched() {
    use incular_widgets::SliverHeaderOverscrollBehavior;

    let controller = ScrollController::new();
    let physics = ScrollPhysics::default().bouncing();
    let (mut tree, root) = viewport_with_header(
        Box::new(
            SliverResizingHeader::new(40., 100., Widget::box_(Size::new(200., 100.), Color::WHITE))
                .scroll_behavior(SliverHeaderScrollBehavior::Floating)
                .overscroll_behavior(SliverHeaderOverscrollBehavior::Stretch)
                .snap(true),
        ) as Box<dyn Sliver>,
        controller.clone(),
        Axis::Vertical,
        false,
        physics,
    );
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let render = tree.render_id(header_child(&tree, root)).expect("render");
    // Absorb first-compositor setup so later pumps are quiet by default.
    let start = Instant::now();
    let _ = pump_frame(&mut tree, constraints, start);

    // Pull into leading overscroll and end the activity there: stretch owns
    // presentation, so no snap may start and the range never moves for it.
    controller.notify_user_scroll(-40.);
    let _ = controller.apply_physics(physics, -40.);
    controller.end_activity();
    let stretch = -controller.offset();
    assert!(stretch > 0., "test must actually overscroll");
    tree.layout(constraints).expect("overscroll layout");
    assert_eq!(
        tree.render_size(render),
        Some(Size::new(200., 100. + stretch))
    );
    // The overscroll's own transform change lands on the next compositor;
    // absorb it (the end trigger is consumed suppressed with it), then
    // confirm.
    let _ = pump_frame(&mut tree, constraints, start + Duration::from_millis(100));
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(200));
    assert!(!active, "no snap may start while stretched");
    assert!(!changed);
    assert_eq!(controller.content_extent(), 900.);

    // Recovery still presents the settled header with no snap pending.
    assert!(controller.jump_to(0.));
    tree.layout(constraints).expect("settled layout");
    assert_eq!(tree.render_size(render), Some(Size::new(200., 100.)));
    let (_, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(300));
    assert!(!active);
}

/// Drives a floating resizing header to a partially revealed snap start:
/// hidden at offset 400, then 60 of 120 showing at offset 340.
fn partial_reveal_fixture() -> (WidgetTree, ElementId, ScrollController, Constraints) {
    let controller = ScrollController::new();
    let (mut tree, root) = viewport_with_header(
        resizing_floating_header(),
        controller.clone(),
        Axis::Vertical,
        false,
        ScrollPhysics::default(),
    );
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    end_scroll_by(&controller, 400.);
    tree.layout(constraints).expect("hidden layout");
    end_scroll_by(&controller, -60.);
    tree.layout(constraints).expect("partial layout");
    (tree, root, controller, constraints)
}

#[test]
fn snap_freezes_when_new_activity_begins_without_moving() {
    let (mut tree, root, controller, constraints) = partial_reveal_fixture();
    let render = tree.render_id(header_child(&tree, root)).expect("render");

    let start = Instant::now();
    let (_, active) = pump_frame(&mut tree, constraints, start);
    assert!(active, "a started snap must schedule frames");
    let _ = pump_frame(&mut tree, constraints, start + Duration::from_millis(150));
    tree.layout(constraints).expect("mid layout");
    let frozen = tree.render_size(render).expect("header size").height;
    assert!(
        (60.0..120.0).contains(&frozen) && frozen != 60. && frozen != 120.,
        "mid-snap presentation must lie strictly between: {frozen}"
    );

    // A new activity with no movement still interrupts: the run drops while
    // the current presentation stays put, and later frames stay quiet.
    assert!(
        controller.begin_activity(),
        "the interrupting activity must be genuinely new"
    );
    for at in [200, 300, 400] {
        let (changed, active) =
            pump_frame(&mut tree, constraints, start + Duration::from_millis(at));
        assert!(!changed, "frozen presentation must not move at +{at}ms");
        assert!(!active, "no frames may be scheduled at +{at}ms");
        assert_eq!(
            tree.render_size(render),
            Some(Size::new(200., frozen)),
            "presentation must remain at the interrupted position"
        );
    }
    assert_eq!(controller.offset(), 340.);
}

#[test]
fn snap_ignores_end_stranded_inside_newer_activity() {
    let (mut tree, root, controller, constraints) = partial_reveal_fixture();
    let render = tree.render_id(header_child(&tree, root)).expect("render");

    // The fixture's last scroll already left an end pending with no tick
    // since. A newer Start overwrites it with no tick between: the overwrite
    // order must win and no snap may begin inside the new activity.
    assert!(
        controller.begin_activity(),
        "the superseding activity must be genuinely new"
    );
    tree.layout(constraints).expect("partial layout");
    assert_eq!(tree.render_size(render), Some(Size::new(200., 60.)));

    let start = Instant::now();
    let (_, active) = pump_frame(&mut tree, constraints, start);
    assert!(!active, "no snap may schedule frames");
    assert_eq!(
        tree.render_size(render),
        Some(Size::new(200., 60.)),
        "no snap may move presentation"
    );
    // The first compositor also performs one-time transform setup; the
    // confirming pump must be fully quiet.
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(100));
    assert!(!changed, "no snap may move presentation");
    assert!(!active, "no snap may schedule frames");
    let (_, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(400));
    assert!(!active);
    assert_eq!(tree.render_size(render), Some(Size::new(200., 60.)));
}

#[test]
fn snap_restarts_after_start_then_end() {
    let (mut tree, root, controller, constraints) = partial_reveal_fixture();
    let render = tree.render_id(header_child(&tree, root)).expect("render");

    // Start followed by End leaves a pending end: a fresh endpoint decision
    // runs from the current presentation.
    assert!(controller.begin_activity());
    assert!(controller.end_activity());
    tree.layout(constraints).expect("partial layout");

    let start = Instant::now();
    let (_, active) = pump_frame(&mut tree, constraints, start);
    assert!(active, "the fresh end must start a snap");
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(300));
    assert!(changed);
    assert!(active, "one settle frame stays scheduled past completion");
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(400));
    assert!(!changed);
    assert!(!active);
    assert_eq!(
        tree.render_size(render),
        Some(Size::new(200., 120.)),
        "the restarted snap must complete revealed"
    );
}

#[test]
fn snap_continues_smoothly_from_activity_cancelled_presentation() {
    let (mut tree, root, controller, constraints) = partial_reveal_fixture();
    let render = tree.render_id(header_child(&tree, root)).expect("render");

    let start = Instant::now();
    let _ = pump_frame(&mut tree, constraints, start);
    let _ = pump_frame(&mut tree, constraints, start + Duration::from_millis(150));
    tree.layout(constraints).expect("mid layout");
    let frozen = tree.render_size(render).expect("header size").height;
    assert!((60.0..120.0).contains(&frozen) && frozen != 60. && frozen != 120.);

    // Cancel via activity start with no movement, then move unbracketed: the
    // offset delta complements activity interruption for programmatic moves.
    assert!(controller.begin_activity());
    assert!(controller.scroll_by(5.));
    tree.layout(constraints).expect("moved layout");
    let resumed = tree.render_size(render).expect("header size").height;
    assert!(
        resumed < frozen,
        "scrolling down must hide further from the frozen value"
    );
    assert!(
        (frozen - resumed - 5.).abs() <= 1.,
        "movement must continue from the frozen presentation: {frozen} -> {resumed}"
    );
    // No end followed, so no snap restarts.
    let (_, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(250));
    assert!(!active, "movement without an end must not restart a snap");
}

#[test]
fn snap_cancels_settling_work_on_new_activity() {
    let (mut tree, root, controller, constraints) = partial_reveal_fixture();
    let render = tree.render_id(header_child(&tree, root)).expect("render");

    // Drive the run to its completing tick: the endpoint is exact and one
    // settle frame stays scheduled.
    let start = Instant::now();
    let (_, active) = pump_frame(&mut tree, constraints, start);
    assert!(active);
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(300));
    assert!(changed);
    assert!(active);

    // A new activity cancels the settling work. The already-exact endpoint
    // still presents through the pending layout, then frames stop.
    assert!(controller.begin_activity());
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(400));
    assert!(!changed, "cancelling settle work must move nothing further");
    assert!(!active, "frames must stop once settling is cancelled");
    assert_eq!(
        tree.render_size(render),
        Some(Size::new(200., 120.)),
        "the exact endpoint still presents"
    );
}

/// Sets the ambient reduced-motion policy through the real
/// environment-update path.
fn set_reduced_motion(tree: &mut WidgetTree, reduced: bool) {
    let _ = tree.set_environment(RuntimeEnvironment {
        reduced_motion: reduced,
        ..tree.environment().clone()
    });
}

#[test]
fn snap_resolves_immediately_when_reduced_motion_is_set() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    // Enabled before mounting: the first decision already skips animation.
    set_reduced_motion(&mut tree, true);
    let root = tree
        .mount(
            CustomScrollView::new(vec![
                resizing_floating_header(),
                Box::new(SliverToBoxAdapter::new(Widget::box_(
                    Size::new(200., 800.),
                    Color::BLACK,
                ))),
            ])
            .controller(controller.clone())
            .into(),
        )
        .expect("mount");
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let render = tree.render_id(header_child(&tree, root)).expect("render");

    end_scroll_by(&controller, 400.);
    tree.layout(constraints).expect("hidden layout");
    end_scroll_by(&controller, -60.);
    tree.layout(constraints).expect("partial layout");
    assert_eq!(tree.render_size(render), Some(Size::new(200., 60.)));

    let start = Instant::now();
    let (changed, active) = pump_frame(&mut tree, constraints, start);
    assert!(changed, "the endpoint must apply on the deciding tick");
    assert!(active, "one settle frame stays scheduled past resolution");
    // +50ms later the endpoint already presents: interpolation could never
    // get there that fast, so this proves no animation ran.
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(50));
    assert!(!changed, "nothing further may move");
    assert!(!active, "frames must stop once the endpoint presents");
    assert_eq!(
        tree.render_size(render),
        Some(Size::new(200., 120.)),
        "reduced motion must complete exactly at the revealed edge"
    );
    assert_eq!(
        white_rects(&mut tree),
        vec![(Offset::ZERO, Size::new(200., 120.))],
        "the revealed header must paint fully"
    );
    assert_eq!(
        tree.hit_test(Offset::new(100., 110.)),
        Some(render),
        "the revealed header must hit near its far edge"
    );
    assert_eq!(controller.offset(), 340.);
    assert_eq!(controller.content_extent(), 920.);
}

#[test]
fn snap_resolves_running_reveal_immediately_when_reduced_motion_starts() {
    let (mut tree, root, controller, constraints) = partial_reveal_fixture();
    let render = tree.render_id(header_child(&tree, root)).expect("render");
    let header = header_child(&tree, root);
    let before_render = render;

    let start = Instant::now();
    let (_, active) = pump_frame(&mut tree, constraints, start);
    assert!(active, "a started snap must schedule frames");
    let _ = pump_frame(&mut tree, constraints, start + Duration::from_millis(150));
    tree.layout(constraints).expect("mid layout");
    let mid = tree.render_size(render).expect("header size").height;
    assert!((60.0..120.0).contains(&mid) && mid != 60. && mid != 120.);

    // Enabling reduced motion mid-flight resolves the run's own target on
    // the next tick instead of interpolating toward it.
    set_reduced_motion(&mut tree, true);
    // Identity survives the policy update: no rebuild, no replacement.
    assert_eq!(header_child(&tree, root), header);
    assert_eq!(tree.render_id(header), Some(before_render));
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(200));
    assert!(changed, "the resolve tick must apply the endpoint");
    assert!(active, "one settle frame stays scheduled past resolution");
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(250));
    assert!(!changed, "nothing further may move");
    assert!(!active, "frames must stop once the endpoint presents");
    assert_eq!(
        tree.render_size(render),
        Some(Size::new(200., 120.)),
        "the run's revealed target must present exactly"
    );
    assert_eq!(controller.offset(), 340.);
    assert_eq!(controller.content_extent(), 920.);
}

#[test]
fn snap_resolves_running_hide_immediately_when_reduced_motion_starts() {
    let controller = ScrollController::new();
    let (mut tree, root) = viewport_with_header(
        resizing_floating_header(),
        controller.clone(),
        Axis::Vertical,
        false,
        ScrollPhysics::default(),
    );
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let render = tree.render_id(header_child(&tree, root)).expect("render");

    // Return until only 30 of 120 show so the run heads for hidden.
    end_scroll_by(&controller, 400.);
    tree.layout(constraints).expect("hidden layout");
    end_scroll_by(&controller, -30.);
    tree.layout(constraints).expect("partial layout");

    let start = Instant::now();
    let (_, active) = pump_frame(&mut tree, constraints, start);
    assert!(active, "a started snap must schedule frames");
    let _ = pump_frame(&mut tree, constraints, start + Duration::from_millis(150));

    set_reduced_motion(&mut tree, true);
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(200));
    assert!(changed, "the resolve tick must apply the endpoint");
    assert!(active, "one settle frame stays scheduled past resolution");
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(250));
    assert!(!changed, "nothing further may move");
    assert!(!active, "frames must stop once the endpoint presents");
    assert_eq!(
        white_rects(&mut tree),
        Vec::<(Offset, Size)>::new(),
        "the run's hidden target must present exactly"
    );
    assert_ne!(
        tree.hit_test(Offset::new(100., 20.)),
        Some(render),
        "the hidden header must not hit"
    );
    assert_eq!(controller.offset(), 370.);
}

#[test]
fn snap_does_not_restart_when_reduced_motion_is_disabled() {
    let controller = ScrollController::new();
    let (mut tree, root) = viewport_with_header(
        resizing_floating_header(),
        controller.clone(),
        Axis::Vertical,
        false,
        ScrollPhysics::default(),
    );
    set_reduced_motion(&mut tree, true);
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let render = tree.render_id(header_child(&tree, root)).expect("render");

    end_scroll_by(&controller, 400.);
    tree.layout(constraints).expect("hidden layout");
    end_scroll_by(&controller, -60.);
    tree.layout(constraints).expect("partial layout");

    let start = Instant::now();
    let (_, active) = pump_frame(&mut tree, constraints, start);
    assert!(active);
    let _ = pump_frame(&mut tree, constraints, start + Duration::from_millis(50));
    assert_eq!(tree.render_size(render), Some(Size::new(200., 120.)));

    // Disabling afterwards completes nothing anew and restarts nothing: a
    // bare end at the endpoint stays quiet under normal timing too.
    set_reduced_motion(&mut tree, false);
    for at in [200, 400] {
        let (changed, active) =
            pump_frame(&mut tree, constraints, start + Duration::from_millis(at));
        assert!(!changed, "disabling must not move presentation at +{at}ms");
        assert!(!active, "disabling must not schedule frames at +{at}ms");
    }
    assert_eq!(tree.render_size(render), Some(Size::new(200., 120.)));
    // A bare Start-then-End at the endpoint stays quiet too: the decision
    // finds nothing to animate under normal timing either.
    assert!(controller.begin_activity());
    assert!(controller.end_activity());
    tree.layout(constraints).expect("end layout");
    let (_, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(500));
    assert!(!active, "a bare end at the endpoint must stay quiet");
    assert_eq!(tree.render_size(render), Some(Size::new(200., 120.)));
}

#[test]
fn snap_start_cancel_applies_under_reduced_motion() {
    let (mut tree, root, controller, constraints) = partial_reveal_fixture();
    let render = tree.render_id(header_child(&tree, root)).expect("render");

    // Cancel a running snap via activity start, then flip the policy: the
    // cancelled run must stay cancelled, and only a new end may resolve.
    let start = Instant::now();
    let (_, active) = pump_frame(&mut tree, constraints, start);
    assert!(active);
    let _ = pump_frame(&mut tree, constraints, start + Duration::from_millis(150));
    assert!(controller.begin_activity());
    set_reduced_motion(&mut tree, true);
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(200));
    assert!(!changed, "cancelled work must not move");
    assert!(!active, "cancelled work must schedule nothing");
    tree.layout(constraints).expect("frozen layout");
    let frozen = tree.render_size(render).expect("header size").height;
    assert!((60.0..120.0).contains(&frozen) && frozen != 60. && frozen != 120.);

    assert!(controller.end_activity());
    tree.layout(constraints).expect("end layout");
    let restart = Instant::now();
    let (changed, active) = pump_frame(&mut tree, constraints, restart);
    assert!(changed, "the fresh end must resolve immediately");
    assert!(active, "one settle frame stays scheduled past resolution");
    let (changed, active) = pump_frame(&mut tree, constraints, restart + Duration::from_millis(50));
    assert!(!changed);
    assert!(!active);
    assert_eq!(
        tree.render_size(render),
        Some(Size::new(200., 120.)),
        "the fresh decision resolves revealed without animating"
    );
}

#[test]
fn snap_reduced_motion_natural_header_resolves_with_semantics() {
    use incular_semantics::SemanticRole;
    use incular_widgets::Semantics;

    let controller = ScrollController::new();
    let child: Widget = Semantics::new(Widget::box_(Size::new(200., 55.), Color::WHITE))
        .role(SemanticRole::GenericContainer)
        .label("snap-row")
        .into();
    let (mut tree, root) = viewport_with_header(
        Box::new(
            SliverNaturalHeader::new(child)
                .scroll_behavior(SliverHeaderScrollBehavior::Floating)
                .snap(true),
        ) as Box<dyn Sliver>,
        controller.clone(),
        Axis::Vertical,
        false,
        ScrollPhysics::default(),
    );
    set_reduced_motion(&mut tree, true);
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let render = tree.render_id(header_child(&tree, root)).expect("render");
    assert_eq!(tree.render_size(render), Some(Size::new(200., 55.)));

    end_scroll_by(&controller, 400.);
    tree.layout(constraints).expect("hidden layout");
    end_scroll_by(&controller, -40.);
    tree.layout(constraints).expect("partial layout");

    let start = Instant::now();
    let (changed, active) = pump_frame(&mut tree, constraints, start);
    assert!(changed, "the endpoint must apply on the deciding tick");
    assert!(active, "one settle frame stays scheduled past resolution");
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(50));
    assert!(!changed, "nothing further may move");
    assert!(!active, "frames must stop once the endpoint presents");
    assert_eq!(
        white_rects(&mut tree),
        vec![(Offset::ZERO, Size::new(200., 55.))],
        "the measured extent must paint fully"
    );
    assert_eq!(
        tree.hit_test(Offset::new(100., 50.)),
        Some(render),
        "the resolved header must hit near its far edge"
    );
    tree.update_semantics();
    let (_, bounds) = tree
        .semantics()
        .iter()
        .find_map(|(id, node)| {
            (node.label.as_deref() == Some("snap-row")).then_some((id, node.bounds))
        })
        .expect("row semantics");
    assert_eq!(bounds.origin, Offset::ZERO);
    assert_eq!(bounds.size, Size::new(200., 55.));
    assert_eq!(controller.offset(), 360.);
    assert_eq!(controller.content_extent(), 855.);
}
