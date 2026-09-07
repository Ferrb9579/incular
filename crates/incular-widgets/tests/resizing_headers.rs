use incular_config::{Axis, Constraints};
use incular_core::{Color, Size};
use incular_scroll::{ScrollController, SliverConstraints};
use incular_widgets::{
    CustomScrollView, Sliver, SliverResizingHeader, SliverToBoxAdapter, Widget,
    internal::WidgetTree,
};

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
