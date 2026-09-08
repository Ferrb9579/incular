//! Retained `SliverAppBar` snap-to-edge through the neutral header path.
//!
//! These regressions prove `snap(true)` changes behavior through Material's
//! retained sliver mapping (not geometry units): floating presentation,
//! snap-implied floating upgrades, completion, interruption silence without
//! the flag, and bottom-slot paint/semantics. Timing uses deterministic test
//! clocks; scrolling uses complete user activities.

use std::time::{Duration, Instant};

use incular_config::{Constraints, RuntimeEnvironment};
use incular_core::{Color, Offset, Size};
use incular_material::{AppBar, SliverAppBar};
use incular_scroll::{ScrollController, ScrollPhysics};
use incular_widgets::{
    CustomScrollView, Sliver, SliverToBoxAdapter, Text, Widget,
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

fn snap_viewport(
    header: SliverAppBar,
    controller: ScrollController,
    reverse: bool,
) -> (WidgetTree, ElementId) {
    let slivers: Vec<Box<dyn Sliver>> = vec![
        Box::new(header),
        Box::new(SliverToBoxAdapter::new(Widget::box_(
            Size::new(200., 800.),
            Color::BLACK,
        ))),
    ];
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            CustomScrollView::new(slivers)
                .controller(controller)
                .reverse(reverse)
                .into(),
        )
        .expect("mount");
    (tree, root)
}

fn header_child(tree: &WidgetTree, root: ElementId) -> ElementId {
    tree.children(root).expect("header")[0]
}

fn explicit_header(floating: bool, snap: bool) -> SliverAppBar {
    SliverAppBar::from_app_bar(AppBar::new(Text::new("Header")))
        .expanded_height(120.)
        .collapsed_height(60.)
        .floating(floating)
        .snap(snap)
}

#[test]
fn snap_reveals_explicit_header_and_hides_when_reversed() {
    for reverse in [false, true] {
        let controller = ScrollController::new();
        let (mut tree, root) =
            snap_viewport(explicit_header(true, true), controller.clone(), reverse);
        let constraints = Constraints::tight(Size::new(200., 200.));
        tree.layout(constraints).expect("layout");
        let render = tree.render_id(header_child(&tree, root)).expect("render");
        // Pin a known scroll state first: reversed offset zero shows the
        // content end, so first-frame anchor correction may have moved it.
        controller.begin_activity();
        controller.jump_to(0.);
        controller.end_activity();
        tree.layout(constraints).expect("pinned layout");

        if reverse {
            // Toward the leading edge reveals fully; backing off hides 70 of
            // 120, so the snap must hide.
            end_scroll_by(&controller, 400.);
            tree.layout(constraints).expect("revealed layout");
            assert_eq!(tree.render_size(render), Some(Size::new(200., 120.)));
            end_scroll_by(&controller, -70.);
            tree.layout(constraints).expect("partial layout");
            assert_eq!(tree.render_size(render), Some(Size::new(200., 60.)));
        } else {
            // Hide fully, then return 60 of 120: at the midpoint the snap
            // must reveal.
            end_scroll_by(&controller, 400.);
            tree.layout(constraints).expect("hidden layout");
            end_scroll_by(&controller, -60.);
            tree.layout(constraints).expect("partial layout");
            assert_eq!(tree.render_size(render), Some(Size::new(200., 60.)));
        }

        let start = Instant::now();
        let (_, active) = pump_frame(&mut tree, constraints, start);
        assert!(active, "a started snap must schedule frames");
        let (changed, active) =
            pump_frame(&mut tree, constraints, start + Duration::from_millis(300));
        assert!(changed, "the completion tick must apply the endpoint");
        assert!(active, "one settle frame stays scheduled past completion");
        let (changed, active) =
            pump_frame(&mut tree, constraints, start + Duration::from_millis(400));
        assert!(!changed, "the settled endpoint needs no further work");
        assert!(!active, "frames must stop after completion");
        assert_eq!(
            tree.render_size(render),
            Some(if reverse {
                Size::new(200., 60.)
            } else {
                Size::new(200., 120.)
            }),
            "snap must settle exactly at the chosen edge"
        );
        // Logical extent never moved for the animation.
        assert_eq!(controller.content_extent(), 920.);
    }
}

#[test]
fn snap_without_floating_upgrades_plain_header_to_floating() {
    let constraints = Constraints::tight(Size::new(200., 200.));
    // The snapped header floats and hides on scroll end; the plain header
    // without snap scrolls away statically through the box adapter.
    for (snap, floats) in [(true, true), (false, false)] {
        let controller = ScrollController::new();
        let (mut tree, root) = snap_viewport(
            SliverAppBar::from_app_bar(AppBar::new(Text::new("Header"))).snap(snap),
            controller.clone(),
            false,
        );
        tree.layout(constraints).expect("layout");
        let render = tree.render_id(header_child(&tree, root)).expect("render");
        assert_eq!(tree.render_size(render), Some(Size::new(200., 64.)));

        end_scroll_by(&controller, 400.);
        tree.layout(constraints).expect("hidden layout");
        end_scroll_by(&controller, -40.);
        tree.layout(constraints).expect("partial layout");

        let start = Instant::now();
        let (_, active) = pump_frame(&mut tree, constraints, start);
        assert_eq!(active, floats, "only the snapped header may animate");
        let _ = pump_frame(&mut tree, constraints, start + Duration::from_millis(300));
        let _ = pump_frame(&mut tree, constraints, start + Duration::from_millis(400));
        if floats {
            assert_eq!(
                tree.render_size(render),
                Some(Size::new(200., 64.)),
                "the upgraded header must complete revealed"
            );
            assert_eq!(
                tree.render_origin(render),
                Offset::ZERO,
                "the upgraded header must sit at the leading edge"
            );
        } else {
            assert_eq!(
                tree.render_origin(render),
                Offset::new(0., -360.),
                "the plain header must scroll away statically"
            );
        }
        let (_, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(500));
        assert!(!active);
    }
}

#[test]
fn snap_without_floating_upgrades_pinned_header_to_floating_pinned() {
    let constraints = Constraints::tight(Size::new(200., 200.));
    // The snapped header reveals 75 on reversal and then finishes
    // collapsing; the pinned header without snap stays collapsed at 60 and
    // never animates.
    for snap in [true, false] {
        let controller = ScrollController::new();
        let (mut tree, root) = snap_viewport(
            SliverAppBar::from_app_bar(AppBar::new(Text::new("Header")))
                .expanded_height(120.)
                .collapsed_height(60.)
                .pinned(true)
                .snap(snap),
            controller.clone(),
            false,
        );
        tree.layout(constraints).expect("layout");
        let render = tree.render_id(header_child(&tree, root)).expect("render");
        assert_eq!(tree.render_size(render), Some(Size::new(200., 120.)));

        // Collapse, then return 15 of the 60 collapse range: the upgraded
        // header reveals 75 and then finishes collapsing, while the pinned
        // header without snap stays collapsed at 60 throughout.
        end_scroll_by(&controller, 400.);
        tree.layout(constraints).expect("collapsed layout");
        end_scroll_by(&controller, -15.);
        tree.layout(constraints).expect("partial layout");
        assert_eq!(
            tree.render_size(render),
            Some(Size::new(200., if snap { 75. } else { 60. }))
        );

        let start = Instant::now();
        let (_, active) = pump_frame(&mut tree, constraints, start);
        assert_eq!(active, snap, "only the snapped header may animate");
        let _ = pump_frame(&mut tree, constraints, start + Duration::from_millis(300));
        let _ = pump_frame(&mut tree, constraints, start + Duration::from_millis(400));
        assert_eq!(
            tree.render_size(render),
            Some(Size::new(200., 60.)),
            "snap must settle the upgraded header at the range edge"
        );
        let (_, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(500));
        assert!(!active);
    }
}

#[test]
fn snap_natural_floating_header_keeps_bottom_paint_and_semantics() {
    use incular_rendering::{Brush, PaintCommand};
    use incular_widgets::{Semantics, SizedBox};

    let background = Color::rgba(30, 60, 90, 255);
    let bottom: Widget = Semantics::new(SizedBox::new().height(15.))
        .role(incular_semantics::Role::Group)
        .label("SnapBottom")
        .into();
    let controller = ScrollController::new();
    let (mut tree, root) = snap_viewport(
        SliverAppBar::from_app_bar(
            AppBar::new(Text::new("Header"))
                .toolbar_height(40.)
                .bottom(bottom)
                .background_color(background),
        )
        .floating(true)
        .snap(true),
        controller.clone(),
        false,
    );
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let render = tree.render_id(header_child(&tree, root)).expect("render");
    assert_eq!(tree.render_size(render), Some(Size::new(200., 55.)));

    // Hide, then reveal 40 of the measured 55: above the midpoint.
    end_scroll_by(&controller, 400.);
    tree.layout(constraints).expect("hidden layout");
    end_scroll_by(&controller, -40.);
    tree.layout(constraints).expect("partial layout");

    let start = Instant::now();
    let (_, active) = pump_frame(&mut tree, constraints, start);
    assert!(active, "a started snap must schedule frames");
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(300));
    assert!(changed, "the completion tick must apply the endpoint");
    assert!(active, "one settle frame stays scheduled past completion");
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(400));
    assert!(!changed, "the settled endpoint needs no further work");
    assert!(!active, "frames must stop after completion");
    assert_eq!(tree.render_size(render), Some(Size::new(200., 55.)));

    // The toolbar background fills the revealed header and the bottom slot
    // keeps its measured bounds after the animation.
    assert!(
        tree.paint()
            .commands()
            .iter()
            .any(|command| matches!(command,
                PaintCommand::RRect { rrect, brush: Brush::Solid(color), .. }
                if *color == background && rrect.rect.size.height == 40.
            )),
        "toolbar background must fill the revealed header"
    );
    tree.update_semantics();
    let bottom_bounds = tree
        .semantics()
        .iter()
        .find_map(|(_, node)| (node.label.as_deref() == Some("SnapBottom")).then_some(node.bounds))
        .expect("bottom semantics");
    assert_eq!(bottom_bounds.origin.y, 40.);
    assert_eq!(bottom_bounds.size.height, 15.);
    let body = tree.children(root).expect("body")[1];
    let hit = tree
        .hit_test(Offset::new(100., 47.))
        .map(|hit| tree.element_for_render(hit));
    assert!(hit.is_some(), "the revealed bottom slot must hit");
    assert_ne!(
        hit.flatten(),
        Some(body),
        "the bottom slot must not fall through to the body"
    );
}

#[test]
fn floating_header_without_snap_stays_put_after_scroll_ends() {
    let controller = ScrollController::new();
    let (mut tree, root) = snap_viewport(explicit_header(true, false), controller.clone(), false);
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let render = tree.render_id(header_child(&tree, root)).expect("render");

    end_scroll_by(&controller, 400.);
    tree.layout(constraints).expect("hidden layout");
    end_scroll_by(&controller, -60.);
    tree.layout(constraints).expect("partial layout");
    assert_eq!(tree.render_size(render), Some(Size::new(200., 60.)));

    let start = Instant::now();
    let _ = pump_frame(&mut tree, constraints, start);
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(100));
    assert!(!active, "no snap may schedule frames without the flag");
    assert!(!changed, "no snap may move presentation without the flag");
    let (_, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(400));
    assert!(!active);
    assert_eq!(tree.render_size(render), Some(Size::new(200., 60.)));
}

#[test]
fn snap_freezes_through_material_path_when_new_activity_begins() {
    let controller = ScrollController::new();
    let (mut tree, root) = snap_viewport(explicit_header(true, true), controller.clone(), false);
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let render = tree.render_id(header_child(&tree, root)).expect("render");

    // Partially reveal, then advance the retained snap halfway.
    end_scroll_by(&controller, 400.);
    tree.layout(constraints).expect("hidden layout");
    end_scroll_by(&controller, -60.);
    tree.layout(constraints).expect("partial layout");

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

    // A new activity with no movement freezes the Material header too, and
    // ending it afterwards restarts from the frozen presentation.
    assert!(controller.begin_activity());
    for at in [250, 400] {
        let (changed, active) =
            pump_frame(&mut tree, constraints, start + Duration::from_millis(at));
        assert!(!changed, "frozen presentation must not move at +{at}ms");
        assert!(!active, "no frames may be scheduled at +{at}ms");
        assert_eq!(
            tree.render_size(render),
            Some(Size::new(200., frozen)),
            "Material presentation must remain at the interrupted position"
        );
    }
    assert!(controller.end_activity());
    tree.layout(constraints).expect("restart layout");
    let restart = Instant::now();
    let (_, active) = pump_frame(&mut tree, constraints, restart);
    assert!(active, "ending the new activity must restart the snap");
    let (changed, active) =
        pump_frame(&mut tree, constraints, restart + Duration::from_millis(300));
    assert!(changed);
    assert!(active, "one settle frame stays scheduled past completion");
    let (changed, active) =
        pump_frame(&mut tree, constraints, restart + Duration::from_millis(400));
    assert!(!changed);
    assert!(!active);
    assert_eq!(
        tree.render_size(render),
        Some(Size::new(200., 120.)),
        "the restarted Material snap must complete revealed"
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
fn snap_resolves_immediately_through_material_path_under_reduced_motion() {
    use incular_rendering::{Brush, PaintCommand};

    let background = Color::rgba(30, 60, 90, 255);
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    // Enabled before mounting: the first decision already skips animation.
    set_reduced_motion(&mut tree, true);
    let root = tree
        .mount(
            CustomScrollView::new(vec![
                Box::new(
                    SliverAppBar::from_app_bar(
                        AppBar::new(Text::new("Header")).background_color(background),
                    )
                    .expanded_height(120.)
                    .collapsed_height(60.)
                    .floating(true)
                    .snap(true),
                ) as Box<dyn Sliver>,
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
    assert_eq!(tree.render_size(render), Some(Size::new(200., 120.)));

    // Partially reveal, then end: the retained Material header must resolve
    // to the revealed edge without interpolating.
    end_scroll_by(&controller, 400.);
    tree.layout(constraints).expect("hidden layout");
    end_scroll_by(&controller, -60.);
    tree.layout(constraints).expect("partial layout");
    assert_eq!(tree.render_size(render), Some(Size::new(200., 60.)));

    let start = Instant::now();
    let (changed, active) = pump_frame(&mut tree, constraints, start);
    assert!(changed, "the endpoint must apply on the deciding tick");
    assert!(active, "one settle frame stays scheduled past resolution");
    let (changed, active) = pump_frame(&mut tree, constraints, start + Duration::from_millis(50));
    assert!(!changed, "nothing further may move");
    assert!(!active, "frames must stop once the endpoint presents");
    assert_eq!(
        tree.render_size(render),
        Some(Size::new(200., 120.)),
        "the Material header must complete revealed without animating"
    );
    assert!(
        tree.paint()
            .commands()
            .iter()
            .any(|command| matches!(command,
                PaintCommand::RRect { rrect, brush: Brush::Solid(color), .. }
                if *color == background && rrect.rect.size.height == 120.
            )),
        "toolbar background must fill the revealed header"
    );
    let body = tree.children(root).expect("body")[1];
    let hit = tree
        .hit_test(Offset::new(100., 110.))
        .map(|hit| tree.element_for_render(hit));
    assert!(hit.is_some(), "the revealed header must hit");
    assert_ne!(
        hit.flatten(),
        Some(body),
        "the hit must land inside the header, not the body"
    );
    assert_eq!(controller.offset(), 340.);
    assert_eq!(controller.content_extent(), 920.);
}
