use incular_config::Constraints;
use incular_core::{Color, Offset, Size};
use incular_material::{AppBar, SliverAppBar};
use incular_scroll::ScrollController;
use incular_widgets::{
    CustomScrollView, Sliver, SliverToBoxAdapter, Text, Widget, internal::WidgetTree,
};
use std::time::Instant;

#[test]
fn explicit_header_extents_collapse_in_every_scroll_mode() {
    for (pinned, floating) in [(false, false), (true, false), (false, true), (true, true)] {
        let controller = ScrollController::new();
        let header = SliverAppBar::from_app_bar(AppBar::new(Text::new("Header")))
            .expanded_height(120.)
            .collapsed_height(60.)
            .pinned(pinned)
            .floating(floating);
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
        let constraints = Constraints::tight(Size::new(200., 200.));
        tree.layout(constraints).expect("layout");
        let child = tree.children(root).expect("header")[0];
        let render = tree.render_id(child).expect("render");
        assert_eq!(tree.render_size(render), Some(Size::new(200., 120.)));
        for (step, offset) in [30., 400., 390., 330., 0.].into_iter().enumerate() {
            assert!(controller.jump_to(offset));
            tree.layout(constraints).expect("scroll layout");
            tree.update_compositor(Instant::now()).expect("compositor");
            let (height, origin) = match (pinned, floating) {
                (false, false) => ((120. - offset).clamp(60., 120.), -(offset - 60.).max(0.)),
                (true, false) => ((120. - offset).clamp(60., 120.), 0.),
                (false, true) => [(90., 0.), (60., -60.), (60., -50.), (70., 0.), (120., 0.)][step],
                (true, true) => [(90., 0.), (60., 0.), (70., 0.), (120., 0.), (120., 0.)][step],
            };
            assert_eq!(tree.render_size(render), Some(Size::new(200., height)));
            assert_eq!(tree.render_origin(render), Offset::new(0., origin));
            assert_eq!(tree.render_id(child), Some(render));
        }
    }
}

#[test]
fn collapsing_composition_preserves_slots_and_measures_bottom_before_toolbar() {
    use incular_rendering::{Brush, PaintCommand};
    use incular_widgets::Semantics;

    let marker = |label: &str| -> Widget {
        Semantics::new(Widget::box_(Size::new(12., 15.), Color::WHITE))
            .role(incular_semantics::Role::Group)
            .label(label)
            .into()
    };
    let background = Color::rgba(30, 60, 90, 255);
    let controller = ScrollController::new();
    let header = SliverAppBar::from_app_bar(
        AppBar::new(marker("Title"))
            .leading(marker("Leading"))
            .actions([marker("Action")])
            .bottom(marker("Bottom"))
            .background_color(background),
    )
    .expanded_height(120.)
    .collapsed_height(60.)
    .pinned(true);
    let slivers: Vec<Box<dyn Sliver>> = vec![
        Box::new(header),
        Box::new(SliverToBoxAdapter::new(Widget::box_(
            Size::new(200., 800.),
            Color::BLACK,
        ))),
    ];
    let mut tree = WidgetTree::new();
    tree.mount(
        CustomScrollView::new(slivers)
            .controller(controller.clone())
            .into(),
    )
    .expect("mount");
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    tree.update_semantics();
    let identities = |tree: &WidgetTree| {
        tree.semantics()
            .iter()
            .filter_map(|(id, node)| {
                node.label
                    .as_ref()
                    .filter(|label| {
                        ["Title", "Leading", "Action", "Bottom"].contains(&label.as_str())
                    })
                    .map(|label| (label.clone(), id))
            })
            .collect::<Vec<_>>()
    };
    let before = identities(&tree);
    assert_eq!(before.len(), 4);
    for (offset, height) in [(0., 120.), (30., 90.), (80., 60.), (0., 120.)] {
        controller.jump_to(offset);
        tree.layout(constraints).expect("scroll layout");
        tree.update_compositor(Instant::now()).expect("compositor");
        tree.update_semantics();
        assert_eq!(identities(&tree), before);
        let bottom = tree
            .semantics()
            .iter()
            .find_map(|(_, node)| (node.label.as_deref() == Some("Bottom")).then_some(node.bounds))
            .expect("bottom");
        assert_eq!(bottom.origin.y, height - 15.);
        assert_eq!(bottom.size.height, 15.);
        let display = tree.paint();
        assert!(
            display.commands().iter().any(|command| matches!(command,
                PaintCommand::RRect { rrect, brush: Brush::Solid(color), .. }
                if *color == background && rrect.rect.size.height == height - 15.
            )),
            "toolbar background must fill remaining height at {offset}"
        );
    }
}

#[test]
fn explicit_bounds_use_the_same_defaults_and_normalization_for_both_builders() {
    for (min, max, expected) in [
        (60., 120., 120.),
        (90., 40., 90.),
        (f32::NAN, 80., 80.),
        (40., f32::INFINITY, 40.),
        (-10., -20., 0.),
    ] {
        for header in [
            SliverAppBar::new()
                .collapsed_height(min)
                .expanded_height(max),
            SliverAppBar::builder()
                .collapsed_height(min)
                .expanded_height(max)
                .build(),
        ] {
            let mut tree = WidgetTree::new();
            let root = tree
                .mount(CustomScrollView::new(vec![Box::new(header) as Box<dyn Sliver>]).into())
                .expect("mount");
            tree.layout(Constraints::tight(Size::new(200., 200.)))
                .expect("layout");
            let child = tree.children(root).expect("header")[0];
            assert_eq!(
                tree.render_size(tree.render_id(child).expect("render")),
                Some(Size::new(200., expected))
            );
        }
    }
    for header in [
        SliverAppBar::from_app_bar(AppBar::new(Text::new("Header")).toolbar_height(40.))
            .expanded_height(120.),
        SliverAppBar::builder()
            .app_bar(AppBar::new(Text::new("Header")).toolbar_height(40.))
            .expanded_height(120.)
            .build(),
    ] {
        let controller = ScrollController::new();
        let slivers: Vec<Box<dyn Sliver>> = vec![
            Box::new(header.pinned(true)),
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
        let constraints = Constraints::tight(Size::new(200., 200.));
        tree.layout(constraints).expect("layout");
        let render = tree
            .render_id(tree.children(root).expect("header")[0])
            .expect("render");
        assert_eq!(tree.render_size(render), Some(Size::new(200., 120.)));
        assert!(controller.jump_to(400.));
        tree.layout(constraints).expect("collapsed layout");
        assert_eq!(tree.render_size(render), Some(Size::new(200., 40.)));
    }
}
