use incular_config::{Axis, Constraints};
use incular_core::{Color, Size};
use incular_scroll::{ScrollController, SliverConstraints};
use incular_widgets::{
    CustomScrollView, LayoutBuilder, Sliver, SliverFloatingHeader, Widget, internal::WidgetTree,
};

#[test]
fn floating_header_returns_immediately_after_a_long_scroll() {
    for axis in [Axis::Vertical, Axis::Horizontal] {
        for reverse in [false, true] {
            let header = SliverFloatingHeader::new(Widget::box_(Size::new(40., 40.), Color::WHITE));
            let mut render = header.create_render_sliver(&ScrollController::new(), axis, reverse);
            let constraints = |offset| {
                SliverConstraints::new(axis, reverse, offset, 0., 0., 200., 100., 200., 400., 0.)
            };
            for (offset, visible) in [
                (0., 40.),
                (400., 0.),
                (390., 10.),
                (390., 10.),
                (360., 40.),
                (380., 20.),
                (0., 40.),
            ] {
                let layout = render.perform_layout(constraints(offset));
                assert_eq!(layout.geometry.paint_extent, visible);
                assert_eq!(layout.geometry.hit_test_extent, visible);
                assert_eq!(layout.geometry.scroll_extent, 40.);
            }
        }
    }
}

#[test]
fn floating_header_measures_deferred_content() {
    for axis in [Axis::Vertical, Axis::Horizontal] {
        let header = SliverFloatingHeader::new(LayoutBuilder::new(|_, _| {
            Widget::box_(Size::new(75., 75.), Color::WHITE)
        }));
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(
                CustomScrollView::new(vec![Box::new(header) as Box<dyn Sliver>])
                    .scroll_direction(axis)
                    .into(),
            )
            .expect("mount");
        tree.layout(Constraints::tight(Size::new(200., 200.)))
            .expect("layout");
        let child = tree.children(root).expect("header")[0];
        let size = tree
            .render_size(tree.render_id(child).expect("render"))
            .expect("size");
        assert_eq!(axis.main_extent(size), 75.);
    }
}

#[test]
fn wrapped_floating_header_refreshes_inside_a_materialized_cache_window() {
    use incular_config::EdgeInsets;
    use incular_core::Offset;
    use incular_widgets::{SliverPadding, SliverToBoxAdapter};
    use std::time::Instant;

    for compositor_only in [false, true] {
        let controller = ScrollController::new();
        let header = SliverPadding::new(
            EdgeInsets::ZERO,
            SliverFloatingHeader::new(Widget::box_(Size::new(200., 40.), Color::WHITE)),
        );
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
                    .cache_extent(1000.)
                    .into(),
            )
            .expect("mount");
        let constraints = Constraints::tight(Size::new(200., 200.));
        tree.layout(constraints).expect("layout");
        let child = tree.children(root).expect("header")[0];
        let render = tree.render_id(child).expect("render");
        let before = tree.diagnostics();
        for (offset, origin) in [(400., -40.), (390., -30.), (360., 0.)] {
            assert!(controller.jump_to(offset));
            if compositor_only {
                tree.update_compositor(Instant::now()).expect("compositor");
            } else {
                tree.layout(constraints).expect("layout");
            }
            assert_eq!(tree.render_origin(render), Offset::new(0., origin));
        }
        assert_eq!(tree.diagnostics().rebuilds, before.rebuilds);
    }
}

#[test]
fn returning_header_paints_and_hits_above_flowing_content() {
    use incular_core::Offset;
    use incular_rendering::PaintCommand;
    use incular_widgets::SliverToBoxAdapter;
    use std::time::Instant;

    let controller = ScrollController::new();
    let slivers: Vec<Box<dyn Sliver>> = vec![
        Box::new(SliverFloatingHeader::new(Widget::box_(
            Size::new(200., 40.),
            Color::WHITE,
        ))),
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
    let header = tree
        .render_id(tree.children(root).expect("header")[0])
        .expect("render");
    for offset in [400., 390.] {
        assert!(controller.jump_to(offset));
        tree.update_compositor(Instant::now()).expect("scroll");
    }
    assert_eq!(tree.hit_test(Offset::new(10., 5.)), Some(header));
    let display = tree.paint();
    let colors: Vec<_> = display
        .commands()
        .iter()
        .filter_map(|command| match command {
            PaintCommand::Rect { color, .. } => Some(*color),
            _ => None,
        })
        .collect();
    assert_eq!(colors, vec![Color::BLACK, Color::WHITE]);
}
