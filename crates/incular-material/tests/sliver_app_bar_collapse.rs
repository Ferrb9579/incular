use incular_config::Constraints;
use incular_core::{Color, Offset, Size};
use incular_material::{AppBar, SliverAppBar};
use incular_scroll::{ScrollController, ScrollPhysics};
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

#[test]
fn explicit_stretch_expands_toolbar_and_recovers_without_range_drift() {
    use incular_rendering::{Brush, PaintCommand};
    use incular_widgets::Semantics;

    let background = Color::rgba(30, 60, 90, 255);
    for (pinned, floating) in [(false, false), (true, false), (false, true), (true, true)] {
        let controller = ScrollController::new();
        let physics = ScrollPhysics::default().bouncing();
        let marker = |label: &str| -> Widget {
            Semantics::new(Widget::box_(Size::new(12., 15.), Color::WHITE))
                .role(incular_semantics::Role::Group)
                .label(label)
                .into()
        };
        let header = SliverAppBar::from_app_bar(
            AppBar::new(marker("Title"))
                .leading(marker("Leading"))
                .actions([marker("Action")])
                .bottom(marker("Bottom"))
                .background_color(background),
        )
        .expanded_height(120.)
        .collapsed_height(60.)
        .pinned(pinned)
        .floating(floating)
        .stretch(true);
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
                    .physics(physics)
                    .into(),
            )
            .expect("mount");
        let constraints = Constraints::tight(Size::new(200., 200.));
        tree.layout(constraints).expect("layout");
        let child = tree.children(root).expect("header")[0];
        let render = tree.render_id(child).expect("render");
        assert_eq!(tree.render_size(render), Some(Size::new(200., 120.)));
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
        for _ in 0..2 {
            controller.apply_physics(physics, -40.);
            let stretch = -controller.offset();
            assert!(stretch > 0., "{pinned}/{floating}");
            tree.layout(constraints).expect("overscroll layout");
            tree.update_compositor(Instant::now()).expect("compositor");
            let height = 120. + stretch;
            assert_eq!(
                tree.render_size(render),
                Some(Size::new(200., height)),
                "{pinned}/{floating}"
            );
            assert_eq!(tree.render_origin(render), Offset::ZERO);
            let body = tree
                .render_id(tree.children(root).expect("body")[1])
                .expect("body");
            assert_eq!(tree.render_origin(body), Offset::new(0., height));
            // Logical scroll range does not grow because of stretching.
            assert_eq!(controller.content_extent(), 920.);
            // Bottom keeps its measured height; the toolbar fills the rest.
            let display = tree.paint();
            assert!(
                display.commands().iter().any(|command| matches!(command,
                    PaintCommand::RRect { rrect, brush: Brush::Solid(color), .. }
                    if *color == background && rrect.rect.size.height == height - 15.
                )),
                "toolbar must fill remaining height at {pinned}/{floating}"
            );
            // The stretched header stays hit-testable; paint agreement above
            // already ties the toolbar to the stretched height.
            assert!(
                tree.hit_test(Offset::new(100., height - 1.)).is_some(),
                "stretched header must stay hit-testable at {pinned}/{floating}"
            );
            tree.update_semantics();
            // Slot identity survives stretching and the bottom keeps its
            // measured height at the stretched extent.
            assert_eq!(identities(&tree), before);
            let bottom = tree
                .semantics()
                .iter()
                .find_map(|(_, node)| {
                    (node.label.as_deref() == Some("Bottom")).then_some(node.bounds)
                })
                .expect("bottom");
            assert_eq!(bottom.origin.y, height - 15.);
            assert_eq!(bottom.size.height, 15.);
            assert!(controller.jump_to(0.));
            tree.layout(constraints).expect("settled layout");
            assert_eq!(tree.render_size(render), Some(Size::new(200., 120.)));
            assert_eq!(controller.content_extent(), 920.);
            tree.update_semantics();
            assert_eq!(identities(&tree), before);
        }
    }
}

#[test]
fn explicit_stretch_honors_disabled_clamping_and_non_leading_overlap() {
    // stretch(false) preserves existing behavior under bouncing overscroll.
    let controller = ScrollController::new();
    let physics = ScrollPhysics::default().bouncing();
    let slivers: Vec<Box<dyn Sliver>> = vec![
        Box::new(
            SliverAppBar::from_app_bar(AppBar::new(Text::new("Header")).toolbar_height(40.))
                .expanded_height(120.)
                .collapsed_height(60.)
                .pinned(true),
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
    let render = tree
        .render_id(tree.children(root).expect("header")[0])
        .expect("render");
    controller.apply_physics(physics, -40.);
    tree.layout(constraints).expect("overscroll layout");
    assert_eq!(tree.render_size(render), Some(Size::new(200., 120.)));

    // Clamping physics does not manufacture overscroll.
    let clamped = ScrollController::new();
    let slivers: Vec<Box<dyn Sliver>> = vec![
        Box::new(
            SliverAppBar::from_app_bar(AppBar::new(Text::new("Header")).toolbar_height(40.))
                .expanded_height(120.)
                .collapsed_height(60.)
                .pinned(true)
                .stretch(true),
        ),
        Box::new(SliverToBoxAdapter::new(Widget::box_(
            Size::new(200., 800.),
            Color::BLACK,
        ))),
    ];
    let mut clamped_tree = WidgetTree::new();
    let clamped_root = clamped_tree
        .mount(
            CustomScrollView::new(slivers)
                .controller(clamped.clone())
                .physics(ScrollPhysics::default())
                .into(),
        )
        .expect("mount");
    clamped_tree.layout(constraints).expect("layout");
    clamped.apply_physics(ScrollPhysics::default(), -40.);
    assert_eq!(clamped.offset(), 0.);
    clamped_tree.layout(constraints).expect("clamped layout");
    let clamped_render = clamped_tree
        .render_id(clamped_tree.children(clamped_root).expect("header")[0])
        .expect("render");
    assert_eq!(
        clamped_tree.render_size(clamped_render),
        Some(Size::new(200., 120.))
    );

    // A non-leading header does not stretch from another sliver's overlap.
    let trailing = ScrollController::new();
    let slivers: Vec<Box<dyn Sliver>> = vec![
        Box::new(SliverToBoxAdapter::new(Widget::box_(
            Size::new(200., 50.),
            Color::WHITE,
        ))),
        Box::new(
            SliverAppBar::from_app_bar(AppBar::new(Text::new("Header")).toolbar_height(40.))
                .expanded_height(120.)
                .collapsed_height(60.)
                .pinned(true)
                .stretch(true),
        ),
        Box::new(SliverToBoxAdapter::new(Widget::box_(
            Size::new(200., 800.),
            Color::BLACK,
        ))),
    ];
    let mut trailing_tree = WidgetTree::new();
    let trailing_root = trailing_tree
        .mount(
            CustomScrollView::new(slivers)
                .controller(trailing.clone())
                .physics(physics)
                .into(),
        )
        .expect("mount");
    trailing_tree.layout(constraints).expect("layout");
    trailing.apply_physics(physics, -40.);
    trailing_tree.layout(constraints).expect("overlap layout");
    let second = trailing_tree
        .render_id(trailing_tree.children(trailing_root).expect("header")[1])
        .expect("render");
    assert_eq!(
        trailing_tree.render_size(second),
        Some(Size::new(200., 120.))
    );
}

#[test]
fn explicit_stretch_supports_reversed_viewports() {
    for (pinned, floating) in [(false, false), (true, false), (false, true), (true, true)] {
        let controller = ScrollController::new();
        let physics = ScrollPhysics::default().bouncing();
        let slivers: Vec<Box<dyn Sliver>> = vec![
            Box::new(
                SliverAppBar::from_app_bar(AppBar::new(Text::new("Header")).toolbar_height(40.))
                    .expanded_height(120.)
                    .collapsed_height(60.)
                    .pinned(pinned)
                    .floating(floating)
                    .stretch(true),
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
                    .reverse(true)
                    .into(),
            )
            .expect("mount");
        let constraints = Constraints::tight(Size::new(200., 200.));
        tree.layout(constraints).expect("layout");
        let render = tree
            .render_id(tree.children(root).expect("header")[0])
            .expect("render");
        let max = controller.max_offset();
        assert!(controller.jump_to(max));
        tree.layout(constraints).expect("end layout");
        // Leading overscroll in a reversed viewport pulls beyond the logical
        // maximum; Scroll owns that offset and Widgets stretches presentation.
        controller.apply_physics(physics, 40.);
        let stretch = controller.offset() - max;
        assert!(stretch > 0., "{pinned}/{floating}");
        tree.layout(constraints).expect("overscroll layout");
        tree.update_compositor(Instant::now()).expect("compositor");
        // At the reversed leading edge the header is fully expanded, so the
        // stretched extent is the maximum plus overscroll in every mode.
        assert_eq!(
            tree.render_size(render),
            Some(Size::new(200., 120. + stretch)),
            "{pinned}/{floating}"
        );
        assert_eq!(controller.content_extent(), 920.);
        assert!(controller.jump_to(max));
        tree.layout(constraints).expect("settled layout");
        assert_eq!(tree.render_size(render), Some(Size::new(200., 120.)));
    }
}
