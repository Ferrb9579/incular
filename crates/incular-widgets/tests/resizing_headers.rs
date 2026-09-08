use incular_config::{Axis, Constraints};
use incular_core::{Color, Size};
use incular_scroll::{ScrollController, SliverConstraints};
use incular_widgets::{
    CustomScrollView, Sliver, SliverNaturalHeader, SliverResizingHeader, SliverToBoxAdapter,
    Widget, internal::WidgetTree,
};

/// Inherited custom value driving test bottom heights. Height is the
/// measured dimension; tint exercises paint-only updates that must not
/// disturb measurement.
#[derive(Clone)]
struct HeaderMode {
    height: f32,
    tint: Color,
}

#[test]
fn constructor_and_builder_normalize_header_bounds_consistently() {
    for (min, max, expected) in [
        (40., 100., (40., 100.)),
        (100., 40., (100., 100.)),
        (-10., -20., (0., 0.)),
        (f32::NAN, 100., (0., 100.)),
        (40., f32::INFINITY, (40., 40.)),
        (f32::NEG_INFINITY, f32::NAN, (0., 0.)),
    ] {
        for header in [
            SliverResizingHeader::new(min, max, Widget::box_(Size::ZERO, Color::WHITE)),
            SliverResizingHeader::builder()
                .min_extent(min)
                .max_extent(max)
                .child(Widget::box_(Size::ZERO, Color::WHITE))
                .build(),
        ] {
            assert_eq!((header.min_extent(), header.max_extent()), expected);
            let mut render =
                header.create_render_sliver(&ScrollController::new(), Axis::Vertical, false);
            for offset in [0., 50., 200.] {
                let layout = render.perform_layout(SliverConstraints::new(
                    Axis::Vertical,
                    false,
                    offset,
                    0.,
                    0.,
                    200.,
                    200.,
                    200.,
                    400.,
                    0.,
                ));
                assert_eq!(layout.geometry.scroll_extent, expected.1);
                assert_eq!(
                    layout.children[0].extent,
                    (expected.1 - offset).clamp(expected.0, expected.1)
                );
            }
        }
    }
}

#[test]
fn resizing_header_updates_during_layout_inside_the_cache_window() {
    let controller = ScrollController::new();
    let slivers: Vec<Box<dyn Sliver>> = vec![
        Box::new(SliverResizingHeader::new(
            40.,
            100.,
            Widget::box_(Size::new(200., 100.), Color::WHITE),
        )),
        Box::new(SliverToBoxAdapter::new(Widget::box_(
            Size::new(200., 800.),
            Color::BLACK,
        ))),
    ];
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            CustomScrollView::new(slivers)
                .controller(controller.clone())
                .cache_extent(1000.)
                .into(),
        )
        .expect("mount");
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let child = tree.children(root).expect("header")[0];
    let render = tree.render_id(child).expect("render");
    let before = tree.diagnostics();
    for (offset, height) in [(40., 60.), (80., 40.), (20., 80.), (0., 100.)] {
        assert!(controller.jump_to(offset));
        tree.layout(constraints).expect("scroll layout");
        assert_eq!(tree.render_size(render), Some(Size::new(200., height)));
    }
    assert_eq!(tree.diagnostics().rebuilds, before.rebuilds);
}

#[test]
fn resizing_scroll_modes_preserve_minimum_extent_and_reverse_correctly() {
    use incular_core::Offset;
    use incular_widgets::SliverHeaderScrollBehavior;
    use std::time::Instant;

    for behavior in [
        SliverHeaderScrollBehavior::Scroll,
        SliverHeaderScrollBehavior::Pinned,
        SliverHeaderScrollBehavior::Floating,
        SliverHeaderScrollBehavior::FloatingPinned,
    ] {
        let controller = ScrollController::new();
        let header = SliverResizingHeader::builder()
            .min_extent(40.)
            .max_extent(100.)
            .child(Widget::box_(Size::new(200., 100.), Color::WHITE))
            .scroll_behavior(behavior)
            .build();
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
                    .controller(controller.clone())
                    .into(),
            )
            .expect("mount");
        tree.layout(Constraints::tight(Size::new(200., 200.)))
            .expect("layout");
        let child = tree.children(root).expect("header")[0];
        let render = tree.render_id(child).expect("render");
        let before = tree.diagnostics();
        for (step, offset) in [40., 400., 390., 360., 340., 0.].into_iter().enumerate() {
            assert!(controller.jump_to(offset));
            // Normal runtime layout and standalone compositor entry points
            // must agree even when the same offset is laid out twice.
            tree.layout(Constraints::tight(Size::new(200., 200.)))
                .expect("scroll layout");
            tree.update_compositor(Instant::now()).expect("compositor");
            let (height, origin) = match behavior {
                SliverHeaderScrollBehavior::Scroll => {
                    ((100. - offset).clamp(40., 100.), -(offset - 60.).max(0.))
                }
                SliverHeaderScrollBehavior::Pinned => ((100. - offset).clamp(40., 100.), 0.),
                SliverHeaderScrollBehavior::Floating => [
                    (60., 0.),
                    (40., -40.),
                    (40., -30.),
                    (40., 0.),
                    (60., 0.),
                    (100., 0.),
                ][step],
                SliverHeaderScrollBehavior::FloatingPinned => [
                    (60., 0.),
                    (40., 0.),
                    (50., 0.),
                    (80., 0.),
                    (100., 0.),
                    (100., 0.),
                ][step],
            };
            assert_eq!(
                tree.render_size(render),
                Some(Size::new(200., height)),
                "{behavior:?} at {offset}"
            );
            assert_eq!(
                tree.render_origin(render),
                Offset::new(0., origin),
                "{behavior:?} at {offset}"
            );
        }
        assert_eq!(tree.diagnostics().rebuilds, before.rebuilds);
    }
}

#[test]
fn stretch_uses_only_leading_overscroll_and_preserves_logical_extent() {
    use incular_widgets::SliverHeaderOverscrollBehavior;
    for axis in [Axis::Vertical, Axis::Horizontal] {
        let header = SliverResizingHeader::builder()
            .min_extent(40.)
            .max_extent(100.)
            .child(Widget::box_(Size::new(100., 100.), Color::WHITE))
            .overscroll_behavior(SliverHeaderOverscrollBehavior::Stretch)
            .build();
        let mut render = header.create_render_sliver(&ScrollController::new(), axis, false);
        for (scroll, preceding, overlap, expected) in [
            (0., 0., -20., 120.),
            (0., 0., 0., 100.),
            (0., 0., 20., 100.),
            (0., 50., -20., 100.),
            (30., 0., -20., 70.),
        ] {
            let layout = render.perform_layout(SliverConstraints::new(
                axis, false, scroll, preceding, overlap, 180., 200., 200., 400., 0.,
            ));
            assert_eq!(layout.geometry.scroll_extent, 100.);
            assert_eq!(layout.children[0].extent, expected);
            if expected == 120. {
                assert_eq!(layout.geometry.paint_origin, -20.);
                assert_eq!(layout.geometry.paint_extent, 120.);
                assert_eq!(layout.geometry.hit_test_extent, 120.);
                assert_eq!(layout.absorbed_overlap, 0.);
            }
        }
    }
}

#[test]
fn stretched_header_fills_overscroll_without_a_gap_or_scroll_range_drift() {
    use incular_core::Offset;
    use incular_scroll::ScrollPhysics;
    use incular_widgets::{SliverHeaderOverscrollBehavior, SliverHeaderScrollBehavior};
    use std::time::Instant;

    for axis in [Axis::Vertical, Axis::Horizontal] {
        for behavior in [
            SliverHeaderScrollBehavior::Scroll,
            SliverHeaderScrollBehavior::Pinned,
            SliverHeaderScrollBehavior::Floating,
            SliverHeaderScrollBehavior::FloatingPinned,
        ] {
            let controller = ScrollController::new();
            let physics = ScrollPhysics::default().bouncing();
            let slivers: Vec<Box<dyn Sliver>> = vec![
                Box::new(
                    SliverResizingHeader::new(
                        40.,
                        100.,
                        Widget::box_(axis.size(100., 200.), Color::WHITE),
                    )
                    .scroll_behavior(behavior)
                    .overscroll_behavior(SliverHeaderOverscrollBehavior::Stretch),
                ),
                Box::new(SliverToBoxAdapter::new(Widget::box_(
                    axis.size(800., 200.),
                    Color::BLACK,
                ))),
            ];
            let mut tree = WidgetTree::new();
            let root = tree
                .mount(
                    CustomScrollView::new(slivers)
                        .controller(controller.clone())
                        .scroll_direction(axis)
                        .physics(physics)
                        .into(),
                )
                .expect("mount");
            let constraints = Constraints::tight(Size::new(200., 200.));
            tree.layout(constraints).expect("layout");
            let children = tree.children(root).expect("children").to_vec();
            let header = tree.render_id(children[0]).expect("header");
            let body = tree.render_id(children[1]).expect("body");
            controller.apply_physics(physics, -40.);
            let stretch = -controller.offset();
            assert!(stretch > 0.);
            tree.layout(constraints).expect("overscroll layout");
            tree.update_compositor(Instant::now()).expect("compositor");
            assert_eq!(tree.render_origin(header), Offset::ZERO);
            assert_eq!(
                tree.render_size(header),
                Some(axis.size(100. + stretch, 200.))
            );
            assert_eq!(tree.render_origin(body), axis.offset(100. + stretch, 0.));
            assert_eq!(controller.content_extent(), 900.);
            assert!(controller.jump_to(0.));
            tree.layout(constraints).expect("settled layout");
            assert_eq!(tree.render_size(header), Some(axis.size(100., 200.)));
            assert_eq!(controller.content_extent(), 900.);
        }
    }
}

#[test]
fn natural_header_stretch_uses_only_leading_overscroll() {
    use incular_widgets::{SliverHeaderOverscrollBehavior, SliverHeaderScrollBehavior};
    for axis in [Axis::Vertical, Axis::Horizontal] {
        for behavior in [
            SliverHeaderScrollBehavior::Scroll,
            SliverHeaderScrollBehavior::Pinned,
            SliverHeaderScrollBehavior::Floating,
            SliverHeaderScrollBehavior::FloatingPinned,
        ] {
            let header =
                SliverNaturalHeader::new(Widget::box_(Size::new(100., 100.), Color::WHITE))
                    .scroll_behavior(behavior)
                    .overscroll_behavior(SliverHeaderOverscrollBehavior::Stretch);
            let mut render = header.create_render_sliver(&ScrollController::new(), axis, false);
            // First unstretched layout learns the natural extent from the
            // unbounded child measurement.
            let first = render.perform_layout(SliverConstraints::new(
                axis, false, 0., 0., 0., 200., 200., 200., 400., 0.,
            ));
            let measured = first.children[0].id;
            // Box children hint their extent, so this is already 100 and
            // reports no change; the call documents the measurement contract.
            let _ = render.set_child_extent(measured, 100.);
            for (scroll, preceding, overlap, expected) in [
                (0., 0., -20., 120.),
                (0., 0., 0., 100.),
                (0., 0., 20., 100.),
                (0., 50., -20., 100.),
                (30., 0., -20., 100.),
            ] {
                let layout = render.perform_layout(SliverConstraints::new(
                    axis, false, scroll, preceding, overlap, 180., 200., 200., 400., 0.,
                ));
                // Logical scroll extent never grows because of stretching.
                assert_eq!(layout.geometry.scroll_extent, 100., "{behavior:?}");
                assert_eq!(layout.children[0].extent, expected, "{behavior:?}");
                if expected == 120. {
                    assert_eq!(layout.geometry.paint_origin, -20.);
                    assert_eq!(layout.geometry.paint_extent, 120.);
                    assert_eq!(layout.geometry.hit_test_extent, 120.);
                    assert_eq!(layout.absorbed_overlap, 0.);
                }
            }
            // Translate preserves the measured extent under the same overlap.
            let plain = SliverNaturalHeader::new(Widget::box_(Size::new(100., 100.), Color::WHITE))
                .scroll_behavior(behavior);
            let mut plain_render =
                plain.create_render_sliver(&ScrollController::new(), axis, false);
            let first_plain = plain_render.perform_layout(SliverConstraints::new(
                axis, false, 0., 0., 0., 200., 200., 200., 400., 0.,
            ));
            let plain_id = first_plain.children[0].id;
            let _ = plain_render.set_child_extent(plain_id, 100.);
            let layout = plain_render.perform_layout(SliverConstraints::new(
                axis, false, 0., 0., -20., 180., 200., 200., 400., 0.,
            ));
            assert_eq!(layout.children[0].extent, 100., "{behavior:?}");
        }
    }
}

#[test]
fn unmeasured_header_measures_unbounded_before_presenting_stretch() {
    use incular_widgets::{LayoutBuilder, SizedBox, SliverHeaderOverscrollBehavior};
    // A hint-less child (LayoutBuilder reports no usable extent) starts from
    // an unverified estimate. Its first layout during active overscroll must
    // still measure unbounded so the true natural size is established instead
    // of presenting stretched guesswork as logical extent.
    let child: Widget = LayoutBuilder::new(|_, _| Widget::from(SizedBox::new().height(70.))).into();
    let header = SliverNaturalHeader::new(child)
        .overscroll_behavior(SliverHeaderOverscrollBehavior::Stretch);
    let mut render = header.create_render_sliver(&ScrollController::new(), Axis::Vertical, false);
    let stretched = || {
        SliverConstraints::new(
            Axis::Vertical,
            false,
            0.,
            0.,
            -20.,
            180.,
            200.,
            200.,
            400.,
            0.,
        )
    };
    let first = render.perform_layout(stretched());
    assert!(
        !first.children[0].constraints.max_height().is_finite(),
        "unverified estimates must measure unbounded, got {:?}",
        first.children[0].constraints
    );
    let measured = first.children[0].id;
    assert!(render.set_child_extent(measured, 70.));
    let second = render.perform_layout(stretched());
    assert_eq!(second.geometry.scroll_extent, 70.);
    assert_eq!(second.children[0].extent, 90.);
    assert_eq!(second.children[0].constraints.max_height(), 90.);
    // An estimate that validates exactly equal must still request another
    // layout: presentation constraints have to switch from unbounded
    // learning to settled or tight stretch even though the value is right.
    // (The 48px box hints exactly its true size, so estimate and actual
    // coincide here by construction.)
    let exact = SliverNaturalHeader::new(Widget::box_(Size::new(48., 48.), Color::WHITE))
        .overscroll_behavior(SliverHeaderOverscrollBehavior::Stretch);
    let mut exact_render =
        exact.create_render_sliver(&ScrollController::new(), Axis::Vertical, false);
    let learning = exact_render.perform_layout(stretched());
    assert!(!learning.children[0].constraints.max_height().is_finite());
    assert!(exact_render.set_child_extent(learning.children[0].id, 48.));
    let tightened = exact_render.perform_layout(stretched());
    assert_eq!(tightened.children[0].constraints.max_height(), 68.);
    assert_eq!(tightened.children[0].extent, 68.);
    assert_eq!(tightened.geometry.scroll_extent, 48.);
}

#[test]
fn natural_header_replacement_during_overscroll_inherits_measurement() {
    use incular_core::Offset;
    use incular_scroll::ScrollPhysics;
    use incular_widgets::{
        LayoutBuilder, SizedBox, SliverHeaderOverscrollBehavior, SliverHeaderScrollBehavior,
    };
    use std::time::Instant;

    let controller = ScrollController::new();
    let physics = ScrollPhysics::default().bouncing();
    let view = || {
        let slivers: Vec<Box<dyn Sliver>> = vec![
            Box::new(
                SliverNaturalHeader::new(Widget::from(LayoutBuilder::new(|_, _| {
                    Widget::from(SizedBox::new().height(60.))
                })))
                .scroll_behavior(SliverHeaderScrollBehavior::Pinned)
                .overscroll_behavior(SliverHeaderOverscrollBehavior::Stretch),
            ),
            Box::new(SliverToBoxAdapter::new(Widget::box_(
                Size::new(200., 800.),
                Color::BLACK,
            ))),
        ];
        CustomScrollView::new(slivers)
            .controller(controller.clone())
            .physics(physics)
            .into()
    };
    let mut tree = WidgetTree::new();
    let root = tree.mount(view()).expect("mount");
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let header = tree
        .render_id(tree.children(root).expect("children")[0])
        .expect("header");
    assert_eq!(tree.render_size(header), Some(Size::new(200., 60.)));
    controller.apply_physics(physics, -40.);
    let stretch = -controller.offset();
    assert!(stretch > 0.);
    // Replace the whole viewport delegate while overscroll is active. The new
    // retained header seeds its estimate from the validated measurement and
    // revalidates unbounded, so equivalent content stays range-stable and
    // presentation stays exact with the scroll activity surviving.
    tree.update(root, view()).expect("update");
    tree.layout(constraints).expect("replaced layout");
    tree.update_compositor(Instant::now()).expect("compositor");
    assert_eq!(controller.offset(), -stretch);
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(200., 60. + stretch))
    );
    assert_eq!(tree.render_origin(header), Offset::ZERO);
    assert_eq!(controller.content_extent(), 860.);
    assert!(controller.jump_to(0.));
    tree.layout(constraints).expect("settled layout");
    assert_eq!(tree.render_size(header), Some(Size::new(200., 60.)));
    assert_eq!(controller.content_extent(), 860.);
}

#[test]
fn taller_replacement_during_overscroll_revalidates_and_settles() {
    use incular_core::Offset;
    use incular_scroll::ScrollPhysics;
    use incular_widgets::{
        LayoutBuilder, SizedBox, SliverHeaderOverscrollBehavior, SliverHeaderScrollBehavior,
    };
    use std::time::Instant;

    let controller = ScrollController::new();
    let physics = ScrollPhysics::default().bouncing();
    let view = |height: f32| {
        let slivers: Vec<Box<dyn Sliver>> = vec![
            Box::new(
                SliverNaturalHeader::new(Widget::from(LayoutBuilder::new(move |_, _| {
                    Widget::from(SizedBox::new().height(height))
                })))
                .scroll_behavior(SliverHeaderScrollBehavior::Pinned)
                .overscroll_behavior(SliverHeaderOverscrollBehavior::Stretch),
            ),
            Box::new(SliverToBoxAdapter::new(Widget::box_(
                Size::new(200., 800.),
                Color::BLACK,
            ))),
        ];
        CustomScrollView::new(slivers)
            .controller(controller.clone())
            .physics(physics)
            .into()
    };
    let mut tree = WidgetTree::new();
    let root = tree.mount(view(60.)).expect("mount");
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let header = tree
        .render_id(tree.children(root).expect("children")[0])
        .expect("header");
    controller.apply_physics(physics, -40.);
    assert!(-controller.offset() > 0.);
    // Replace with genuinely taller content during overscroll. The stale
    // measurement must not be retained: the replacement revalidates unbounded
    // to its true size, and the resulting range change settles through
    // Scroll's documented extent policy instead of preserving overscroll.
    tree.update(root, view(80.)).expect("update");
    tree.layout(constraints).expect("replaced layout");
    tree.update_compositor(Instant::now()).expect("compositor");
    assert_eq!(controller.offset(), 0.);
    assert_eq!(tree.render_size(header), Some(Size::new(200., 80.)));
    assert_eq!(tree.render_origin(header), Offset::ZERO);
    assert_eq!(controller.content_extent(), 880.);
    // Fresh overscroll stretches the revalidated measurement exactly, with no
    // accumulated drift.
    controller.apply_physics(physics, -40.);
    let stretch = -controller.offset();
    assert!(stretch > 0.);
    tree.layout(constraints).expect("overscroll layout");
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(200., 80. + stretch))
    );
    assert!(controller.jump_to(0.));
    tree.layout(constraints).expect("settled layout");
    assert_eq!(tree.render_size(header), Some(Size::new(200., 80.)));
    assert_eq!(controller.content_extent(), 880.);
}

#[test]
fn exact_estimate_replacement_still_tightens_presentation() {
    use incular_core::Offset;
    use incular_scroll::ScrollPhysics;
    use incular_widgets::{
        LayoutBuilder, SizedBox, SliverHeaderOverscrollBehavior, SliverHeaderScrollBehavior,
    };
    use std::time::Instant;

    // Replacing a hint-less header with a hint-exact box of the same size
    // during overscroll: the seeded estimate validates exactly equal, which
    // must still tighten presentation constraints (unbounded learning to
    // tight stretch) instead of leaving the natural-sized child under
    // stretched paint. This locks the equal-value transition; the unit test
    // above is the case that fails without it.
    let controller = ScrollController::new();
    let physics = ScrollPhysics::default().bouncing();
    let layout_built = || {
        Widget::from(LayoutBuilder::new(|_, _| {
            Widget::from(SizedBox::new().height(60.))
        }))
    };
    let view = |header: SliverNaturalHeader| {
        let slivers: Vec<Box<dyn Sliver>> = vec![
            Box::new(header),
            Box::new(SliverToBoxAdapter::new(Widget::box_(
                Size::new(200., 800.),
                Color::BLACK,
            ))),
        ];
        CustomScrollView::new(slivers)
            .controller(controller.clone())
            .physics(physics)
            .into()
    };
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(view(
            SliverNaturalHeader::new(layout_built())
                .scroll_behavior(SliverHeaderScrollBehavior::Pinned)
                .overscroll_behavior(SliverHeaderOverscrollBehavior::Stretch),
        ))
        .expect("mount");
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let header = tree
        .render_id(tree.children(root).expect("children")[0])
        .expect("header");
    assert_eq!(tree.render_size(header), Some(Size::new(200., 60.)));
    controller.apply_physics(physics, -40.);
    let stretch = -controller.offset();
    assert!(stretch > 0.);
    tree.update(
        root,
        view(
            SliverNaturalHeader::new(Widget::box_(Size::new(200., 60.), Color::WHITE))
                .scroll_behavior(SliverHeaderScrollBehavior::Pinned)
                .overscroll_behavior(SliverHeaderOverscrollBehavior::Stretch),
        ),
    )
    .expect("update");
    tree.layout(constraints).expect("replaced layout");
    tree.update_compositor(Instant::now()).expect("compositor");
    // The child type changed (LayoutBuilder to box), so the materialized
    // render object is legitimately new; re-query it instead of the stale id.
    let header = tree
        .render_id(tree.children(root).expect("children")[0])
        .expect("header");
    assert_eq!(controller.offset(), -stretch);
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(200., 60. + stretch))
    );
    assert_eq!(tree.render_origin(header), Offset::ZERO);
    assert_eq!(controller.content_extent(), 860.);
}

#[test]
fn stateful_content_change_during_overscroll_revalidates() {
    use incular_core::Offset;
    use incular_scroll::ScrollPhysics;
    use incular_widgets::{SizedBox, SliverHeaderOverscrollBehavior, SliverHeaderScrollBehavior};
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::Instant;

    // A stateful descendant mutates intrinsic height in place: bumping its
    // revision rebuilds only that child with the same viewport delegate and
    // header retained. The rebuild invalidates the cached measurement so the
    // next layout revalidates unbounded — even mid-overscroll — instead of
    // retaining staleness. Genuine range changes settle per Scroll policy.
    let controller = ScrollController::new();
    let physics = ScrollPhysics::default().bouncing();
    let height = Rc::new(Cell::new(60.));
    let revision = Rc::new(Cell::new(0_u64));
    let slivers: Vec<Box<dyn Sliver>> = vec![
        Box::new(
            SliverNaturalHeader::new(Widget::stateful_layout_builder(revision.clone(), {
                let height = height.clone();
                move |_, _| Widget::from(SizedBox::new().height(height.get()))
            }))
            .scroll_behavior(SliverHeaderScrollBehavior::Pinned)
            .overscroll_behavior(SliverHeaderOverscrollBehavior::Stretch),
        ),
        Box::new(SliverToBoxAdapter::new(Widget::box_(
            Size::new(200., 800.),
            Color::BLACK,
        ))),
    ];
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            CustomScrollView::new(slivers)
                .controller(controller.clone())
                .physics(physics)
                .into(),
        )
        .expect("mount");
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let header = tree
        .render_id(tree.children(root).expect("children")[0])
        .expect("header");
    assert_eq!(tree.render_size(header), Some(Size::new(200., 60.)));
    controller.apply_physics(physics, -40.);
    let stretch = -controller.offset();
    assert!(stretch > 0.);
    // Grow beyond the old stretched total (60 + stretch) during overscroll.
    height.set(100.);
    revision.set(1);
    tree.layout(constraints).expect("grown layout");
    tree.update_compositor(Instant::now()).expect("compositor");
    assert_eq!(controller.offset(), 0.);
    assert_eq!(tree.render_size(header), Some(Size::new(200., 100.)));
    assert_eq!(tree.render_origin(header), Offset::ZERO);
    assert_eq!(controller.content_extent(), 900.);
    // Fresh overscroll stretches the revalidated measurement exactly.
    controller.apply_physics(physics, -40.);
    let stretch = -controller.offset();
    assert!(stretch > 0.);
    tree.layout(constraints).expect("overscroll layout");
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(200., 100. + stretch))
    );
    assert!(controller.jump_to(0.));
    tree.layout(constraints).expect("settled layout");
    // Shrink during overscroll: same invalidation, opposite direction.
    controller.apply_physics(physics, -40.);
    assert!(-controller.offset() > 0.);
    height.set(50.);
    revision.set(2);
    tree.layout(constraints).expect("shrunk layout");
    assert_eq!(controller.offset(), 0.);
    assert_eq!(tree.render_size(header), Some(Size::new(200., 50.)));
    assert_eq!(controller.content_extent(), 850.);
    // The retained header and its delegate survived every in-place change.
    assert_eq!(
        tree.render_id(tree.children(root).expect("children")[0]),
        Some(header)
    );
    controller.apply_physics(physics, -40.);
    let stretch = -controller.offset();
    tree.layout(constraints).expect("final overscroll layout");
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(200., 50. + stretch))
    );
    assert!(controller.jump_to(0.));
    tree.layout(constraints).expect("final settled layout");
    assert_eq!(tree.render_size(header), Some(Size::new(200., 50.)));
    assert_eq!(controller.content_extent(), 850.);
}

#[test]
fn stateful_rebuild_without_size_change_preserves_overscroll() {
    use incular_scroll::ScrollPhysics;
    use incular_widgets::{SizedBox, SliverHeaderOverscrollBehavior, SliverHeaderScrollBehavior};
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::Instant;

    // A revision bump that leaves intrinsic size unchanged must converge
    // without settling overscroll: validation confirms the same value, so
    // Scroll sees no metric change.
    let controller = ScrollController::new();
    let physics = ScrollPhysics::default().bouncing();
    let revision = Rc::new(Cell::new(0_u64));
    let slivers: Vec<Box<dyn Sliver>> = vec![
        Box::new(
            SliverNaturalHeader::new(Widget::stateful_layout_builder(revision.clone(), |_, _| {
                Widget::from(SizedBox::new().height(60.))
            }))
            .scroll_behavior(SliverHeaderScrollBehavior::Pinned)
            .overscroll_behavior(SliverHeaderOverscrollBehavior::Stretch),
        ),
        Box::new(SliverToBoxAdapter::new(Widget::box_(
            Size::new(200., 800.),
            Color::BLACK,
        ))),
    ];
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            CustomScrollView::new(slivers)
                .controller(controller.clone())
                .physics(physics)
                .into(),
        )
        .expect("mount");
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let header = tree
        .render_id(tree.children(root).expect("children")[0])
        .expect("header");
    controller.apply_physics(physics, -40.);
    let stretch = -controller.offset();
    assert!(stretch > 0.);
    revision.set(1);
    tree.layout(constraints).expect("rebuilt layout");
    tree.update_compositor(Instant::now()).expect("compositor");
    assert_eq!(controller.offset(), -stretch);
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(200., 60. + stretch))
    );
    assert_eq!(controller.content_extent(), 860.);
    // Repeated settled layouts stay converged with no drift.
    for _ in 0..3 {
        tree.layout(constraints).expect("repeat layout");
    }
    assert!(controller.jump_to(0.));
    tree.layout(constraints).expect("settled layout");
    assert_eq!(tree.render_size(header), Some(Size::new(200., 60.)));
}

#[test]
fn natural_header_stretch_recovers_and_ignores_stretched_measurements() {
    use incular_core::Offset;
    use incular_scroll::ScrollPhysics;
    use incular_widgets::{SliverHeaderOverscrollBehavior, SliverHeaderScrollBehavior};
    use std::time::Instant;

    for behavior in [
        SliverHeaderScrollBehavior::Scroll,
        SliverHeaderScrollBehavior::Pinned,
        SliverHeaderScrollBehavior::Floating,
        SliverHeaderScrollBehavior::FloatingPinned,
    ] {
        let controller = ScrollController::new();
        let physics = ScrollPhysics::default().bouncing();
        let view = |height: f32| {
            let slivers: Vec<Box<dyn Sliver>> = vec![
                Box::new(
                    SliverNaturalHeader::new(Widget::box_(Size::new(200., height), Color::WHITE))
                        .scroll_behavior(behavior)
                        .overscroll_behavior(SliverHeaderOverscrollBehavior::Stretch),
                ),
                Box::new(SliverToBoxAdapter::new(Widget::box_(
                    Size::new(200., 800.),
                    Color::BLACK,
                ))),
            ];
            CustomScrollView::new(slivers)
                .controller(controller.clone())
                .physics(physics)
                .into()
        };
        let mut tree = WidgetTree::new();
        let root = tree.mount(view(60.)).expect("mount");
        let constraints = Constraints::tight(Size::new(200., 200.));
        tree.layout(constraints).expect("layout");
        let header = tree
            .render_id(tree.children(root).expect("children")[0])
            .expect("header");
        let body = tree
            .render_id(tree.children(root).expect("children")[1])
            .expect("body");
        assert_eq!(tree.render_size(header), Some(Size::new(200., 60.)));
        // Repeated overscroll/recovery must not drift the natural extent.
        for _ in 0..3 {
            controller.apply_physics(physics, -40.);
            let stretch = -controller.offset();
            assert!(stretch > 0., "{behavior:?}");
            tree.layout(constraints).expect("overscroll layout");
            tree.update_compositor(Instant::now()).expect("compositor");
            assert_eq!(
                tree.render_size(header),
                Some(Size::new(200., 60. + stretch)),
                "{behavior:?}"
            );
            assert_eq!(
                tree.render_origin(body),
                Offset::new(0., 60. + stretch),
                "{behavior:?}"
            );
            assert_eq!(controller.content_extent(), 860.);
            assert!(controller.jump_to(0.));
            tree.layout(constraints).expect("settled layout");
            assert_eq!(tree.render_size(header), Some(Size::new(200., 60.)));
            assert_eq!(controller.content_extent(), 860.);
        }
        // Later content changes are learned once overscroll ends; stretched
        // measurements never become the new natural extent.
        tree.update(root, view(80.)).expect("update");
        tree.layout(constraints).expect("changed layout");
        assert_eq!(tree.render_size(header), Some(Size::new(200., 80.)));
        controller.apply_physics(physics, -30.);
        tree.layout(constraints).expect("stretched changed layout");
        assert_eq!(
            tree.render_size(header),
            Some(Size::new(200., 80. - controller.offset()))
        );
        assert!(controller.jump_to(0.));
        tree.layout(constraints).expect("recovered changed layout");
        assert_eq!(tree.render_size(header), Some(Size::new(200., 80.)));
        assert_eq!(controller.content_extent(), 880.);
    }
}

#[test]
fn inherited_dependency_change_during_overscroll_revalidates() {
    use incular_core::Offset;
    use incular_scroll::ScrollPhysics;
    use incular_widgets::{
        LayoutBuilder, SliverHeaderOverscrollBehavior, SliverHeaderScrollBehavior,
    };
    use std::time::Instant;

    // An ordinary LayoutBuilder bottom reads an inherited value at
    // materialization time. Updating the scope above the viewport
    // re-materializes it in place with the same delegate retained; the
    // invalidation drain demotes the header so it revalidates unbounded
    // instead of stretching stale content.
    let controller = ScrollController::new();
    let physics = ScrollPhysics::default().bouncing();
    let viewport = || {
        let slivers: Vec<Box<dyn Sliver>> = vec![
            Box::new(
                SliverNaturalHeader::new(Widget::from(LayoutBuilder::new(|context, _| {
                    let (height, tint) = context
                        .depend_on::<HeaderMode>()
                        .map_or((60., Color::WHITE), |mode| (mode.height, mode.tint));
                    Widget::box_(Size::new(200., height), tint)
                })))
                .scroll_behavior(SliverHeaderScrollBehavior::Pinned)
                .overscroll_behavior(SliverHeaderOverscrollBehavior::Stretch),
            ),
            Box::new(SliverToBoxAdapter::new(Widget::box_(
                Size::new(200., 800.),
                Color::BLACK,
            ))),
        ];
        CustomScrollView::new(slivers)
            .controller(controller.clone())
            .physics(physics)
            .into()
    };
    let mode = |height: f32, tint: Color| HeaderMode { height, tint };
    // Hold one viewport widget object: every scope update below reuses this
    // identical value, so the viewport element bails out and the delegate
    // (with all retained sliver state) is genuinely preserved.
    let viewport: Widget = viewport();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::environment_scope(
            mode(60., Color::WHITE),
            viewport.clone(),
        ))
        .expect("mount");
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let viewport_id = tree.children(root).expect("scope child")[0];
    let header = tree
        .render_id(tree.children(viewport_id).expect("children")[0])
        .expect("header");
    assert_eq!(tree.render_size(header), Some(Size::new(200., 60.)));
    controller.apply_physics(physics, -40.);
    let stretch = -controller.offset();
    assert!(stretch > 0.);
    // Grow during overscroll at unchanged width. The genuine range change
    // settles through Scroll's documented extent policy with the
    // authoritative new measurement presented.
    tree.update(
        root,
        Widget::environment_scope(mode(100., Color::WHITE), viewport.clone()),
    )
    .expect("update");
    tree.layout(constraints).expect("grown layout");
    tree.update_compositor(Instant::now()).expect("compositor");
    assert_eq!(controller.offset(), 0.);
    assert_eq!(tree.render_size(header), Some(Size::new(200., 100.)));
    assert_eq!(tree.render_origin(header), Offset::ZERO);
    assert_eq!(controller.content_extent(), 900.);
    // The retained header, delegate, and render identity survived.
    assert_eq!(
        tree.render_id(tree.children(viewport_id).expect("children")[0]),
        Some(header)
    );
    controller.apply_physics(physics, -40.);
    let stretch = -controller.offset();
    assert!(stretch > 0.);
    tree.layout(constraints).expect("overscroll layout");
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(200., 100. + stretch))
    );
    assert!(controller.jump_to(0.));
    tree.layout(constraints).expect("settled layout");
    // Shrink during overscroll follows the same path in reverse.
    controller.apply_physics(physics, -40.);
    assert!(-controller.offset() > 0.);
    tree.update(
        root,
        Widget::environment_scope(mode(50., Color::WHITE), viewport.clone()),
    )
    .expect("update");
    tree.layout(constraints).expect("shrunk layout");
    assert_eq!(controller.offset(), 0.);
    assert_eq!(tree.render_size(header), Some(Size::new(200., 50.)));
    assert_eq!(controller.content_extent(), 850.);
    // A paint-only inherited change (tint without height) disturbs nothing:
    // overscroll is preserved and sizes stay put.
    controller.apply_physics(physics, -40.);
    let stretch = -controller.offset();
    assert!(stretch > 0.);
    tree.update(
        root,
        Widget::environment_scope(mode(50., Color::rgba(1, 2, 3, 255)), viewport.clone()),
    )
    .expect("update");
    tree.layout(constraints).expect("tint layout");
    assert_eq!(controller.offset(), -stretch);
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(200., 50. + stretch))
    );
    assert_eq!(controller.content_extent(), 850.);
    // Repeated layouts converge with no drift and no measurement loops.
    for _ in 0..3 {
        tree.layout(constraints).expect("repeat layout");
    }
    assert_eq!(controller.offset(), -stretch);
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(200., 50. + stretch))
    );
    assert!(controller.jump_to(0.));
    tree.layout(constraints).expect("recovered layout");
    assert_eq!(tree.render_size(header), Some(Size::new(200., 50.)));
}

#[test]
fn scope_update_with_fresh_descriptors_composes_drain_and_transfer() {
    use incular_scroll::ScrollPhysics;
    use incular_widgets::{
        LayoutBuilder, SliverHeaderOverscrollBehavior, SliverHeaderScrollBehavior,
    };
    use std::time::Instant;

    // Updating the scope with rebuilt (not identical) viewport descriptors
    // exercises drain demotion and transfer seeding in one update: the drain
    // demotes the old delegate first, then transfer seeds the replacement
    // from that demoted value rather than a blind hint, so equivalent
    // content stays range-stable with overscroll preserved.
    let controller = ScrollController::new();
    let physics = ScrollPhysics::default().bouncing();
    let viewport = || {
        let slivers: Vec<Box<dyn Sliver>> = vec![
            Box::new(
                SliverNaturalHeader::new(Widget::from(LayoutBuilder::new(|context, _| {
                    let height = context
                        .depend_on::<HeaderMode>()
                        .map_or(60., |mode| mode.height);
                    Widget::box_(Size::new(200., height), Color::WHITE)
                })))
                .scroll_behavior(SliverHeaderScrollBehavior::Pinned)
                .overscroll_behavior(SliverHeaderOverscrollBehavior::Stretch),
            ),
            Box::new(SliverToBoxAdapter::new(Widget::box_(
                Size::new(200., 800.),
                Color::BLACK,
            ))),
        ];
        CustomScrollView::new(slivers)
            .controller(controller.clone())
            .physics(physics)
            .into()
    };
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::environment_scope(
            HeaderMode {
                height: 60.,
                tint: Color::WHITE,
            },
            viewport(),
        ))
        .expect("mount");
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    controller.apply_physics(physics, -40.);
    let stretch = -controller.offset();
    assert!(stretch > 0.);
    // Same height, fresh descriptors: tint-only change, full reconstruction.
    tree.update(
        root,
        Widget::environment_scope(
            HeaderMode {
                height: 60.,
                tint: Color::rgba(4, 5, 6, 255),
            },
            viewport(),
        ),
    )
    .expect("update");
    tree.layout(constraints).expect("tint layout");
    tree.update_compositor(Instant::now()).expect("compositor");
    let viewport_id = tree.children(root).expect("scope child")[0];
    let header = tree
        .render_id(tree.children(viewport_id).expect("children")[0])
        .expect("header");
    assert_eq!(controller.offset(), -stretch);
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(200., 60. + stretch))
    );
    assert_eq!(controller.content_extent(), 860.);
}

#[test]
fn inherited_change_inside_padding_wrapper_routes_invalidation() {
    use incular_config::EdgeInsets;
    use incular_scroll::ScrollPhysics;
    use incular_widgets::{
        LayoutBuilder, SliverHeaderOverscrollBehavior, SliverHeaderScrollBehavior, SliverPadding,
    };
    use std::time::Instant;

    // Invalidation routes through transparent wrappers by sliver position,
    // so padded headers revalidate exactly like bare ones. Cross-axis-only
    // padding keeps the wrapped header leading (zero preceding extent), so
    // stretch flows through the wrapper while the cross extent narrows.
    let controller = ScrollController::new();
    let physics = ScrollPhysics::default().bouncing();
    let viewport = || {
        let slivers: Vec<Box<dyn Sliver>> = vec![
            Box::new(SliverPadding::new(
                EdgeInsets::symmetric(10., 0.),
                SliverNaturalHeader::new(Widget::from(LayoutBuilder::new(|context, _| {
                    let height = context
                        .depend_on::<HeaderMode>()
                        .map_or(60., |mode| mode.height);
                    Widget::box_(Size::new(200., height), Color::WHITE)
                })))
                .scroll_behavior(SliverHeaderScrollBehavior::Pinned)
                .overscroll_behavior(SliverHeaderOverscrollBehavior::Stretch),
            )),
            Box::new(SliverToBoxAdapter::new(Widget::box_(
                Size::new(200., 800.),
                Color::BLACK,
            ))),
        ];
        CustomScrollView::new(slivers)
            .controller(controller.clone())
            .physics(physics)
            .into()
    };
    let viewport: Widget = viewport();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::environment_scope(
            HeaderMode {
                height: 60.,
                tint: Color::WHITE,
            },
            viewport.clone(),
        ))
        .expect("mount");
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let viewport_id = tree.children(root).expect("scope child")[0];
    let header = tree
        .render_id(tree.children(viewport_id).expect("children")[0])
        .expect("header");
    // 60 measured; cross-axis padding narrows the child to 180 wide.
    assert_eq!(tree.render_size(header), Some(Size::new(180., 60.)));
    assert_eq!(controller.content_extent(), 860.);
    controller.apply_physics(physics, -40.);
    let stretch = -controller.offset();
    assert!(stretch > 0.);
    tree.layout(constraints).expect("overscroll layout");
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(180., 60. + stretch))
    );
    tree.update(
        root,
        Widget::environment_scope(
            HeaderMode {
                height: 90.,
                tint: Color::WHITE,
            },
            viewport.clone(),
        ),
    )
    .expect("update");
    tree.layout(constraints).expect("grown layout");
    tree.update_compositor(Instant::now()).expect("compositor");
    assert_eq!(controller.offset(), 0.);
    assert_eq!(tree.render_size(header), Some(Size::new(180., 90.)));
    assert_eq!(controller.content_extent(), 890.);
    controller.apply_physics(physics, -40.);
    let stretch = -controller.offset();
    assert!(stretch > 0.);
    tree.layout(constraints).expect("repeat overscroll layout");
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(180., 90. + stretch))
    );
    assert!(controller.jump_to(0.));
    tree.layout(constraints).expect("recovered layout");
    assert_eq!(tree.render_size(header), Some(Size::new(180., 90.)));
}

#[test]
fn controller_driven_text_change_revalidates_same_delegate() {
    use incular_core::Offset;
    use incular_scroll::ScrollPhysics;
    use incular_widgets::{
        EditableText, SliverHeaderOverscrollBehavior, SliverHeaderScrollBehavior,
        TextEditingController,
    };
    use std::time::Instant;

    // A multiline editor mutates intrinsic height through its controller with
    // no descriptor rebuild and no new delegate. The content-revision check
    // in the layout preamble routes through the same enclosing-sliver
    // invalidation channel, so growth during overscroll lands authoritatively
    // on the next completed layout. Selection and caret revisions never enter
    // that channel, so visual-only edits disturb nothing by construction.
    let edit = TextEditingController::with_text("Hi");
    let controller = ScrollController::new();
    let physics = ScrollPhysics::default().bouncing();
    let slivers: Vec<Box<dyn Sliver>> = vec![
        Box::new(
            SliverNaturalHeader::new(Widget::from(
                EditableText::new(edit.clone()).multiline(true),
            ))
            .scroll_behavior(SliverHeaderScrollBehavior::Pinned)
            .overscroll_behavior(SliverHeaderOverscrollBehavior::Stretch),
        ),
        Box::new(SliverToBoxAdapter::new(Widget::box_(
            Size::new(200., 800.),
            Color::BLACK,
        ))),
    ];
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            CustomScrollView::new(slivers)
                .controller(controller.clone())
                .physics(physics)
                .into(),
        )
        .expect("mount");
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let header = tree
        .render_id(tree.children(root).expect("children")[0])
        .expect("header");
    let short = tree.render_size(header).expect("short size").height;
    // Multiline growth while settled learns through the ordinary pass.
    edit.set_text("Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua");
    tree.layout(constraints).expect("grown layout");
    let grown = tree.render_size(header).expect("grown size").height;
    assert!(
        grown > short,
        "long text must lengthen the editor: {short} -> {grown}"
    );
    assert_eq!(controller.content_extent(), 800. + grown);
    // Growth during overscroll, beyond the old stretched total: the next
    // completed layout carries the authoritative extent with no manual
    // recovery step, settling through Scroll's extent policy.
    controller.apply_physics(physics, -40.);
    let stretch = -controller.offset();
    assert!(stretch > 0.);
    let stale_total = grown + stretch;
    edit.set_text("Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua pack my box with five dozen liquor jugs how vexingly quick daft zebras jump");
    tree.layout(constraints).expect("overscroll growth layout");
    tree.update_compositor(Instant::now()).expect("compositor");
    assert_eq!(controller.offset(), 0.);
    let live = tree.render_size(header).expect("live size").height;
    assert!(
        live > stale_total,
        "growth must exceed {stale_total}, got {live}"
    );
    assert_eq!(tree.render_origin(header), Offset::ZERO);
    assert_eq!(controller.content_extent(), 800. + live);
    assert_eq!(
        tree.render_id(tree.children(root).expect("children")[0]),
        Some(header),
        "controller, element, render, and delegate identity preserved"
    );
    // Shrinkage follows the same path; a same-height edit and a
    // selection-only change preserve range and overscroll instead.
    controller.apply_physics(physics, -40.);
    assert!(-controller.offset() > 0.);
    edit.set_text("Lorem ipsum dolor sit amet");
    tree.layout(constraints).expect("overscroll shrink layout");
    assert_eq!(controller.offset(), 0.);
    let shrunk = tree.render_size(header).expect("shrunk size").height;
    assert!(shrunk < live);
    assert_eq!(controller.content_extent(), 800. + shrunk);
    controller.apply_physics(physics, -40.);
    let stretch = -controller.offset();
    assert!(stretch > 0.);
    edit.set_text("Lorem ipsum dolor sit amet!");
    tree.layout(constraints).expect("same-height edit layout");
    assert_eq!(controller.offset(), -stretch);
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(200., shrunk + stretch))
    );
    let end = edit.selection().end();
    edit.set_selection(incular_widgets::TextSelection::collapsed(if end == 0 {
        1
    } else {
        0
    }));
    tree.layout(constraints).expect("selection layout");
    assert_eq!(controller.offset(), -stretch);
    for _ in 0..3 {
        tree.layout(constraints).expect("repeat layout");
    }
    assert_eq!(controller.offset(), -stretch);
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(200., shrunk + stretch))
    );
    assert!(controller.jump_to(0.));
    tree.layout(constraints).expect("recovered layout");
    assert_eq!(tree.render_size(header), Some(Size::new(200., shrunk)));
    assert_eq!(controller.content_extent(), 800. + shrunk);
}

#[test]
fn nested_shrink_wrap_content_change_during_outer_overscroll() {
    use incular_core::Offset;
    use incular_scroll::ScrollPhysics;
    use incular_widgets::{
        SizedBox, SliverHeaderOverscrollBehavior, SliverHeaderScrollBehavior,
        internal::ShrinkWrappingViewport,
    };
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::Instant;

    // An intrinsically sized inner viewport lives inside an outer natural
    // header. Mutating inner content in place retains both delegates; the
    // rebuild must invalidate the outer measurement through the
    // shrink-wrapping boundary so the next layout revalidates unbounded.
    // Initial content matches the lazy default so the test isolates
    // propagation from first-frame estimate convergence.
    let controller = ScrollController::new();
    let physics = ScrollPhysics::default().bouncing();
    let inner_height = Rc::new(Cell::new(48.));
    let inner_revision = Rc::new(Cell::new(0_u64));
    let inner_slivers: Vec<Box<dyn Sliver>> = vec![Box::new(SliverToBoxAdapter::new(
        Widget::stateful_layout_builder(inner_revision.clone(), {
            let inner_height = inner_height.clone();
            move |_, _| Widget::from(SizedBox::new().height(inner_height.get()))
        }),
    ))];
    let slivers: Vec<Box<dyn Sliver>> = vec![
        Box::new(
            SliverNaturalHeader::new(Widget::from(ShrinkWrappingViewport::new(inner_slivers)))
                .scroll_behavior(SliverHeaderScrollBehavior::Pinned)
                .overscroll_behavior(SliverHeaderOverscrollBehavior::Stretch),
        ),
        Box::new(SliverToBoxAdapter::new(Widget::box_(
            Size::new(200., 800.),
            Color::BLACK,
        ))),
    ];
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            CustomScrollView::new(slivers)
                .controller(controller.clone())
                .physics(physics)
                .into(),
        )
        .expect("mount");
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let header = tree
        .render_id(tree.children(root).expect("children")[0])
        .expect("header");
    assert_eq!(tree.render_size(header), Some(Size::new(200., 48.)));
    assert_eq!(controller.content_extent(), 848.);
    controller.apply_physics(physics, -40.);
    let stretch = -controller.offset();
    assert!(stretch > 0.);
    // Grow beyond the old stretched total (48 + stretch) during overscroll.
    inner_height.set(100.);
    inner_revision.set(1);
    tree.layout(constraints).expect("grown layout");
    tree.update_compositor(Instant::now()).expect("compositor");
    assert_eq!(controller.offset(), 0.);
    assert_eq!(tree.render_size(header), Some(Size::new(200., 100.)));
    assert_eq!(tree.render_origin(header), Offset::ZERO);
    assert_eq!(controller.content_extent(), 900.);
    // Both delegates and the retained header survived the in-place change.
    assert_eq!(
        tree.render_id(tree.children(root).expect("children")[0]),
        Some(header)
    );
    // Fresh overscroll stretches the revalidated measurement exactly.
    controller.apply_physics(physics, -40.);
    let stretch = -controller.offset();
    assert!(stretch > 0.);
    tree.layout(constraints).expect("overscroll layout");
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(200., 100. + stretch))
    );
    assert!(controller.jump_to(0.));
    tree.layout(constraints).expect("settled layout");
    // Shrink during overscroll follows the same propagation path.
    controller.apply_physics(physics, -40.);
    assert!(-controller.offset() > 0.);
    inner_height.set(30.);
    inner_revision.set(2);
    tree.layout(constraints).expect("shrunk layout");
    assert_eq!(controller.offset(), 0.);
    assert_eq!(tree.render_size(header), Some(Size::new(200., 30.)));
    assert_eq!(controller.content_extent(), 830.);
    // Repeated settled layouts converge with no drift and no loops.
    for _ in 0..3 {
        tree.layout(constraints).expect("repeat layout");
    }
    assert_eq!(tree.render_size(header), Some(Size::new(200., 30.)));
    assert_eq!(controller.content_extent(), 830.);
    controller.apply_physics(physics, -40.);
    let stretch = -controller.offset();
    assert!(stretch > 0.);
    tree.layout(constraints).expect("final overscroll layout");
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(200., 30. + stretch))
    );
    assert!(controller.jump_to(0.));
    tree.layout(constraints).expect("recovered layout");
    assert_eq!(tree.render_size(header), Some(Size::new(200., 30.)));
    assert_eq!(controller.content_extent(), 830.);
}

#[test]
fn fixed_size_inner_viewport_activity_preserves_outer_measurement() {
    use incular_core::Offset;
    use incular_scroll::ScrollPhysics;
    use incular_widgets::{SizedBox, SliverHeaderOverscrollBehavior, SliverHeaderScrollBehavior};
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::Instant;

    // A fixed-size inner viewport reports a stable size no matter what its
    // content does, so inner scrolling and inner content changes must leave
    // the outer header measurement and overscroll untouched.
    let controller = ScrollController::new();
    let physics = ScrollPhysics::default().bouncing();
    let inner = ScrollController::new();
    let inner_height = Rc::new(Cell::new(100.));
    let inner_revision = Rc::new(Cell::new(0_u64));
    let inner_slivers: Vec<Box<dyn Sliver>> = vec![
        Box::new(SliverToBoxAdapter::new(Widget::box_(
            Size::new(200., 700.),
            Color::WHITE,
        ))),
        Box::new(SliverToBoxAdapter::new(Widget::stateful_layout_builder(
            inner_revision.clone(),
            {
                let inner_height = inner_height.clone();
                move |_, _| Widget::from(SizedBox::new().height(inner_height.get()))
            },
        ))),
    ];
    let slivers: Vec<Box<dyn Sliver>> = vec![
        Box::new(
            SliverNaturalHeader::new({
                let inner_viewport: Widget = CustomScrollView::new(inner_slivers)
                    .controller(inner.clone())
                    .into();
                Widget::from(SizedBox::new().height(100.).child(inner_viewport))
            })
            .scroll_behavior(SliverHeaderScrollBehavior::Pinned)
            .overscroll_behavior(SliverHeaderOverscrollBehavior::Stretch),
        ),
        Box::new(SliverToBoxAdapter::new(Widget::box_(
            Size::new(200., 800.),
            Color::BLACK,
        ))),
    ];
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            CustomScrollView::new(slivers)
                .controller(controller.clone())
                .physics(physics)
                .into(),
        )
        .expect("mount");
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let header = tree
        .render_id(tree.children(root).expect("children")[0])
        .expect("header");
    assert_eq!(tree.render_size(header), Some(Size::new(200., 100.)));
    assert_eq!(controller.content_extent(), 900.);
    assert_eq!(inner.content_extent(), 800.);
    controller.apply_physics(physics, -40.);
    let stretch = -controller.offset();
    assert!(stretch > 0.);
    // Scrolling the inner viewport is offset-only activity: the outer header
    // keeps its stretched presentation and the outer overscroll survives.
    assert!(inner.jump_to(300.));
    tree.layout(constraints).expect("inner scroll layout");
    tree.update_compositor(Instant::now()).expect("compositor");
    assert_eq!(inner.offset(), 300.);
    assert_eq!(controller.offset(), -stretch);
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(200., 100. + stretch))
    );
    assert_eq!(controller.content_extent(), 900.);
    // Growing inner content changes the inner range but not the fixed outer
    // size: invalidation stops at the fixed-size boundary.
    inner_height.set(200.);
    inner_revision.set(1);
    tree.layout(constraints).expect("inner growth layout");
    assert_eq!(inner.content_extent(), 900.);
    assert_eq!(controller.offset(), -stretch);
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(200., 100. + stretch))
    );
    assert_eq!(tree.render_origin(header), Offset::ZERO);
    assert_eq!(controller.content_extent(), 900.);
    assert!(controller.jump_to(0.));
    tree.layout(constraints).expect("recovered layout");
    assert_eq!(tree.render_size(header), Some(Size::new(200., 100.)));
}
