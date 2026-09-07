use incular_config::Constraints;
use incular_core::{Color, Offset, Size};
use incular_material::{AppBar, SliverAppBar};
use incular_scroll::{ScrollController, ScrollPhysics};
use incular_widgets::{
    CustomScrollView, Sliver, SliverToBoxAdapter, Text, Widget, internal::WidgetTree,
};
use std::time::Instant;

#[test]
fn replacing_title_preserves_configured_app_bar_slots_and_background() {
    use incular_rendering::{Brush, PaintCommand};

    let colors = [
        Color::rgba(1, 2, 3, 255),
        Color::rgba(4, 5, 6, 255),
        Color::rgba(7, 8, 9, 255),
        Color::rgba(10, 11, 12, 255),
        Color::rgba(13, 14, 15, 255),
    ];
    let old_title = Color::rgba(16, 17, 18, 255);
    let marker = |color| Widget::box_(Size::new(12., 12.), color);
    let header = SliverAppBar::from_app_bar(
        AppBar::new(marker(old_title))
            .leading(marker(colors[0]))
            .actions([marker(colors[1])])
            .bottom(marker(colors[2]))
            .background_color(colors[3])
            .toolbar_height(40.),
    )
    .title(marker(colors[4]));
    let mut tree = WidgetTree::new();
    tree.mount(header.into()).expect("mount");
    tree.layout(Constraints::tight(Size::new(200., 52.)))
        .expect("layout");
    let commands = tree.paint();
    let painted = |expected| {
        commands.commands().iter().any(|command| {
            matches!(command,
                PaintCommand::Rect { color, .. }
                | PaintCommand::RRect { brush: Brush::Solid(color), .. } if *color == expected
            )
        })
    };
    for color in colors {
        assert!(
            painted(color),
            "configured slot or background was lost: {color:?}"
        );
    }
    assert!(!painted(old_title));
}

#[test]
fn floating_material_header_reappears_with_measured_bottom_content() {
    use incular_widgets::SizedBox;

    for pinned in [false, true] {
        let controller = ScrollController::new();
        let header = SliverAppBar::from_app_bar(
            AppBar::new(Text::new("Header"))
                .toolbar_height(40.)
                .bottom(SizedBox::new().height(15.)),
        )
        .floating(true)
        .pinned(pinned);
        let slivers: Vec<Box<dyn Sliver>> = vec![
            Box::new(header),
            Box::new(SliverToBoxAdapter::new(Widget::box_(
                Size::new(200., 800.),
                Color::WHITE,
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
        assert_eq!(tree.render_size(render), Some(Size::new(200., 55.)));
        let before = tree.diagnostics();
        for (offset, origin) in [(400., -55.), (390., -45.), (345., 0.)] {
            assert!(controller.jump_to(offset));
            tree.update_compositor(Instant::now()).expect("scroll");
            assert_eq!(
                tree.render_origin(render),
                Offset::new(0., if pinned { 0. } else { origin })
            );
            assert_eq!(tree.render_size(render), Some(Size::new(200., 55.)));
        }
        assert_eq!(tree.diagnostics().rebuilds, before.rebuilds);
    }
}

#[test]
fn material_header_pinning_reaches_the_retained_viewport() {
    for (pinned, reverse) in [(true, false), (false, false), (true, true), (false, true)] {
        let controller = ScrollController::new();
        let slivers: Vec<Box<dyn Sliver>> = vec![
            Box::new(
                SliverAppBar::from_app_bar(AppBar::new(Text::new("Header")).toolbar_height(40.))
                    .pinned(pinned),
            ),
            Box::new(SliverToBoxAdapter::new(Widget::box_(
                Size::new(200., 800.),
                Color::WHITE,
            ))),
        ];
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(
                CustomScrollView::new(slivers)
                    .controller(controller.clone())
                    .reverse(reverse)
                    .into(),
            )
            .expect("mount");
        tree.layout(Constraints::tight(Size::new(200., 200.)))
            .expect("layout");
        let header = tree.children(root).expect("slivers")[0];
        let render = tree.render_id(header).expect("render");
        assert_eq!(tree.render_size(render), Some(Size::new(200., 40.)));
        assert!(controller.jump_to(40.));
        tree.update_compositor(Instant::now())
            .expect("first scroll");
        let before = tree.diagnostics();
        let initial_origin = tree.render_origin(render);
        assert!(controller.jump_to(80.));
        tree.update_compositor(Instant::now()).expect("scroll");
        assert_eq!(
            tree.render_origin(render),
            if pinned {
                Offset::new(0., if reverse { 160. } else { 0. })
            } else {
                initial_origin + Offset::new(0., if reverse { 40. } else { -40. })
            }
        );
        assert_eq!(tree.diagnostics().rebuilds, before.rebuilds);
    }
}

#[test]
fn header_measures_bottom_content_and_resolves_the_mounted_theme() {
    use incular_controls::{ControlTheme, ControlThemeScope};
    use incular_rendering::{Brush, PaintCommand};
    use incular_widgets::SizedBox;

    let surface = Color::rgba(23, 45, 67, 255);
    let mut theme = ControlTheme::light();
    theme.colors.surface = surface;
    let slivers: Vec<Box<dyn Sliver>> = vec![Box::new(
        SliverAppBar::from_app_bar(
            AppBar::new(Text::new("Header"))
                .toolbar_height(40.)
                .bottom(SizedBox::new().height(15.)),
        )
        .pinned(true),
    )];
    let mut tree = WidgetTree::new();
    tree.mount(ControlThemeScope::new(theme, CustomScrollView::new(slivers)).into())
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let commands = tree.paint();
    assert!(commands.commands().iter().any(|command| matches!(command,
        PaintCommand::RRect { brush: Brush::Solid(color), .. } if *color == surface
    )));
    // Inspect the sliver protocol's measured extent independently of text size.
    let header = SliverAppBar::from_app_bar(
        AppBar::new(Text::new("Header"))
            .toolbar_height(40.)
            .bottom(SizedBox::new().height(15.)),
    )
    .pinned(true);
    let mut direct = WidgetTree::new();
    let root = direct
        .mount(CustomScrollView::new(vec![Box::new(header) as Box<dyn Sliver>]).into())
        .expect("mount");
    direct
        .layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let child = direct.children(root).expect("children")[0];
    assert_eq!(
        direct.render_size(direct.render_id(child).expect("render")),
        Some(Size::new(200., 55.))
    );
}

#[test]
fn natural_stretch_expands_toolbar_and_tracks_later_content_changes() {
    use incular_rendering::{Brush, PaintCommand};
    use incular_widgets::Semantics;

    let background = Color::rgba(11, 22, 33, 255);
    let marker = |label: &str, height: f32| -> Widget {
        Semantics::new(Widget::box_(Size::new(12., height), Color::WHITE))
            .role(incular_semantics::Role::Group)
            .label(label)
            .into()
    };
    for (pinned, floating) in [(true, false), (false, false), (false, true), (true, true)] {
        let controller = ScrollController::new();
        let physics = ScrollPhysics::default().bouncing();
        let view = |bottom_height: f32| {
            let slivers: Vec<Box<dyn Sliver>> = vec![
                Box::new(
                    SliverAppBar::from_app_bar(
                        AppBar::new(marker("Title", 15.))
                            .toolbar_height(40.)
                            .bottom(marker("Bottom", bottom_height))
                            .background_color(background),
                    )
                    .pinned(pinned)
                    .floating(floating)
                    .stretch(true),
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
        let root = tree.mount(view(15.)).expect("mount");
        let constraints = Constraints::tight(Size::new(200., 200.));
        tree.layout(constraints).expect("layout");
        let header = tree
            .render_id(tree.children(root).expect("header")[0])
            .expect("render");
        assert_eq!(tree.render_size(header), Some(Size::new(200., 55.)));
        tree.update_semantics();
        let identities = |tree: &WidgetTree| {
            tree.semantics()
                .iter()
                .filter_map(|(id, node)| {
                    node.label
                        .as_ref()
                        .filter(|label| ["Title", "Bottom"].contains(&label.as_str()))
                        .map(|label| (label.clone(), id))
                })
                .collect::<Vec<_>>()
        };
        let before = identities(&tree);
        assert_eq!(before.len(), 2);
        // Repeated overscroll/recovery without extent drift.
        for _ in 0..3 {
            controller.apply_physics(physics, -40.);
            let stretch = -controller.offset();
            assert!(stretch > 0., "{pinned}/{floating}");
            tree.layout(constraints).expect("overscroll layout");
            tree.update_compositor(Instant::now()).expect("compositor");
            let height = 55. + stretch;
            assert_eq!(
                tree.render_size(header),
                Some(Size::new(200., height)),
                "{pinned}/{floating}"
            );
            assert_eq!(tree.render_origin(header), Offset::ZERO);
            let body = tree
                .render_id(tree.children(root).expect("body")[1])
                .expect("body");
            assert_eq!(tree.render_origin(body), Offset::new(0., height));
            assert_eq!(controller.content_extent(), 855.);
            let display = tree.paint();
            assert!(
                display.commands().iter().any(|command| matches!(command,
                    PaintCommand::RRect { rrect, brush: Brush::Solid(color), .. }
                    if *color == background && rrect.rect.size.height == height - 15.
                )),
                "toolbar must fill remaining height at {pinned}/{floating}"
            );
            assert!(
                tree.hit_test(Offset::new(100., height - 1.)).is_some(),
                "stretched natural header must stay hit-testable"
            );
            tree.update_semantics();
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
            assert_eq!(tree.render_size(header), Some(Size::new(200., 55.)));
            assert_eq!(controller.content_extent(), 855.);
        }
        // Later natural content changes are learned instead of sticking to the
        // first measurement; stretched samples never become the new natural.
        tree.update(root, view(25.)).expect("update");
        tree.layout(constraints).expect("changed layout");
        assert_eq!(tree.render_size(header), Some(Size::new(200., 65.)));
        tree.update_semantics();
        let changed_bottom = tree
            .semantics()
            .iter()
            .find_map(|(_, node)| (node.label.as_deref() == Some("Bottom")).then_some(node.bounds))
            .expect("changed bottom");
        assert_eq!(changed_bottom.origin.y, 65. - 25.);
        assert_eq!(changed_bottom.size.height, 25.);
        controller.apply_physics(physics, -30.);
        let stretch = -controller.offset();
        assert!(stretch > 0.);
        tree.layout(constraints).expect("stretched changed layout");
        tree.update_compositor(Instant::now()).expect("compositor");
        assert_eq!(
            tree.render_size(header),
            Some(Size::new(200., 65. + stretch))
        );
        assert!(controller.jump_to(0.));
        tree.layout(constraints).expect("recovered changed layout");
        assert_eq!(tree.render_size(header), Some(Size::new(200., 65.)));
        assert_eq!(controller.content_extent(), 865.);
        tree.update_semantics();
        assert_eq!(identities(&tree).len(), 2);
    }
}

#[test]
fn natural_stretch_honors_disabled_clamping_and_theme() {
    use incular_controls::{ControlTheme, ControlThemeScope};
    use incular_rendering::{Brush, PaintCommand};
    use incular_widgets::SizedBox;

    // stretch(false) keeps the measured header fixed under overscroll.
    let controller = ScrollController::new();
    let physics = ScrollPhysics::default().bouncing();
    let slivers: Vec<Box<dyn Sliver>> = vec![
        Box::new(
            SliverAppBar::from_app_bar(
                AppBar::new(Text::new("Header"))
                    .toolbar_height(40.)
                    .bottom(SizedBox::new().height(15.)),
            )
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
    assert_eq!(tree.render_size(render), Some(Size::new(200., 55.)));

    // Clamping physics does not manufacture overscroll for stretch headers.
    let clamped = ScrollController::new();
    let slivers: Vec<Box<dyn Sliver>> = vec![
        Box::new(
            SliverAppBar::from_app_bar(
                AppBar::new(Text::new("Header"))
                    .toolbar_height(40.)
                    .bottom(SizedBox::new().height(15.)),
            )
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
        Some(Size::new(200., 55.))
    );

    // Stretching keeps mounted theme resolution: the toolbar still paints the
    // scoped surface while stretched.
    let surface = Color::rgba(23, 45, 67, 255);
    let mut theme = ControlTheme::light();
    theme.colors.surface = surface;
    let themed_controller = ScrollController::new();
    let slivers: Vec<Box<dyn Sliver>> = vec![
        Box::new(
            SliverAppBar::from_app_bar(
                AppBar::new(Text::new("Header"))
                    .toolbar_height(40.)
                    .bottom(SizedBox::new().height(15.)),
            )
            .pinned(true)
            .stretch(true),
        ),
        Box::new(SliverToBoxAdapter::new(Widget::box_(
            Size::new(200., 800.),
            Color::BLACK,
        ))),
    ];
    let mut themed = WidgetTree::new();
    themed
        .mount(
            ControlThemeScope::new(
                theme,
                CustomScrollView::new(slivers)
                    .controller(themed_controller.clone())
                    .physics(physics),
            )
            .into(),
        )
        .expect("mount");
    themed.layout(constraints).expect("layout");
    themed_controller.apply_physics(physics, -30.);
    themed.layout(constraints).expect("stretched layout");
    assert!(
        themed
            .paint()
            .commands()
            .iter()
            .any(|command| matches!(command,
                PaintCommand::RRect { brush: Brush::Solid(color), .. } if *color == surface
            ))
    );
    assert!(themed_controller.jump_to(0.));
    themed.layout(constraints).expect("settled layout");
}

#[test]
fn natural_stretch_supports_reversed_viewports() {
    use incular_widgets::SizedBox;

    for (pinned, floating) in [(false, false), (true, false), (false, true), (true, true)] {
        let controller = ScrollController::new();
        let physics = ScrollPhysics::default().bouncing();
        let slivers: Vec<Box<dyn Sliver>> = vec![
            Box::new(
                SliverAppBar::from_app_bar(
                    AppBar::new(Text::new("Header"))
                        .toolbar_height(40.)
                        .bottom(SizedBox::new().height(15.)),
                )
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
        assert_eq!(tree.render_size(render), Some(Size::new(200., 55.)));
        let max = controller.max_offset();
        // A reversed viewport may already sit at the leading edge after the
        // initial anchor correction; reaching it is what matters, not whether
        // the offset changed.
        controller.jump_to(max);
        tree.layout(constraints).expect("end layout");
        controller.apply_physics(physics, 40.);
        let stretch = controller.offset() - max;
        assert!(stretch > 0., "{pinned}/{floating}");
        tree.layout(constraints).expect("overscroll layout");
        tree.update_compositor(Instant::now()).expect("compositor");
        assert_eq!(
            tree.render_size(render),
            Some(Size::new(200., 55. + stretch)),
            "{pinned}/{floating}"
        );
        assert_eq!(controller.content_extent(), 855.);
        assert!(controller.jump_to(max));
        tree.layout(constraints).expect("settled layout");
        assert_eq!(tree.render_size(render), Some(Size::new(200., 55.)));
    }
}

#[test]
fn natural_stretch_measures_hintless_layout_builder_bottom() {
    use incular_rendering::{Brush, PaintCommand};
    use incular_widgets::{LayoutBuilder, Semantics, SizedBox};

    let background = Color::rgba(11, 22, 33, 255);
    for (pinned, floating) in [(true, false), (false, false), (false, true), (true, true)] {
        let controller = ScrollController::new();
        let physics = ScrollPhysics::default().bouncing();
        // A custom-composed bottom with no usable extent hint must still be
        // measured through retained layout; the toolbar fills the remainder.
        let bottom = || -> Widget {
            Semantics::new(Widget::from(LayoutBuilder::new(|_, _| {
                Widget::from(SizedBox::new().height(20.))
            })))
            .role(incular_semantics::Role::Group)
            .label("Bottom")
            .into()
        };
        let slivers: Vec<Box<dyn Sliver>> = vec![
            Box::new(
                SliverAppBar::from_app_bar(
                    AppBar::new(Text::new("Header"))
                        .toolbar_height(40.)
                        .bottom(bottom())
                        .background_color(background),
                )
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
                    .into(),
            )
            .expect("mount");
        let constraints = Constraints::tight(Size::new(200., 200.));
        tree.layout(constraints).expect("layout");
        let header = tree
            .render_id(tree.children(root).expect("header")[0])
            .expect("render");
        // Natural extent is measured (40 + 20), not guessed or left at a default.
        assert_eq!(
            tree.render_size(header),
            Some(Size::new(200., 60.)),
            "{pinned}/{floating}"
        );
        for _ in 0..2 {
            controller.apply_physics(physics, -30.);
            let stretch = -controller.offset();
            assert!(stretch > 0., "{pinned}/{floating}");
            tree.layout(constraints).expect("overscroll layout");
            tree.update_compositor(Instant::now()).expect("compositor");
            let height = 60. + stretch;
            assert_eq!(
                tree.render_size(header),
                Some(Size::new(200., height)),
                "{pinned}/{floating}"
            );
            assert_eq!(tree.render_origin(header), Offset::ZERO);
            let body = tree
                .render_id(tree.children(root).expect("body")[1])
                .expect("body");
            assert_eq!(tree.render_origin(body), Offset::new(0., height));
            assert_eq!(controller.content_extent(), 860.);
            let display = tree.paint();
            assert!(
                display.commands().iter().any(|command| matches!(command,
                    PaintCommand::RRect { rrect, brush: Brush::Solid(color), .. }
                    if *color == background && rrect.rect.size.height == height - 20.
                )),
                "toolbar must fill remaining height at {pinned}/{floating}"
            );
            assert!(
                tree.hit_test(Offset::new(100., height - 1.)).is_some(),
                "stretched header must stay hit-testable"
            );
            // The clipped toolbar paints exactly the stretched header; nothing
            // overflows the visible placement.
            tree.update_semantics();
            let bottom_bounds = tree
                .semantics()
                .iter()
                .find_map(|(_, node)| {
                    (node.label.as_deref() == Some("Bottom")).then_some(node.bounds)
                })
                .expect("bottom");
            assert_eq!(bottom_bounds.origin.y, height - 20.);
            assert_eq!(bottom_bounds.size.height, 20.);
            assert!(controller.jump_to(0.));
            tree.layout(constraints).expect("settled layout");
            assert_eq!(tree.render_size(header), Some(Size::new(200., 60.)));
            assert_eq!(controller.content_extent(), 860.);
        }
    }
}

#[test]
fn natural_stretch_uses_wrapped_text_height_and_tracks_cross_resize() {
    use incular_rendering::{Brush, PaintCommand};
    use incular_widgets::Semantics;

    let background = Color::rgba(44, 55, 66, 255);
    let long = "Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua";
    let bottom = || -> Widget {
        Semantics::new(Text::new(long))
            .role(incular_semantics::Role::Group)
            .label("Bottom")
            .into()
    };
    let controller = ScrollController::new();
    let physics = ScrollPhysics::default().bouncing();
    let view = || {
        let slivers: Vec<Box<dyn Sliver>> = vec![
            Box::new(
                SliverAppBar::from_app_bar(
                    AppBar::new(Text::new("Header"))
                        .toolbar_height(40.)
                        .bottom(bottom())
                        .background_color(background),
                )
                .pinned(true)
                .stretch(true),
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
    let narrow = Constraints::tight(Size::new(200., 200.));
    tree.layout(narrow).expect("layout");
    let header = tree
        .render_id(tree.children(root).expect("header")[0])
        .expect("render");
    tree.update_semantics();
    let bottom_height = tree
        .semantics()
        .iter()
        .find_map(|(_, node)| {
            (node.label.as_deref() == Some("Bottom")).then_some(node.bounds.size.height)
        })
        .expect("bottom");
    // The text wraps to several lines: its actual height differs from any
    // single-line hint, and the natural header is toolbar plus actual bottom.
    assert!(bottom_height > 40., "bottom must wrap, got {bottom_height}");
    let natural = 40. + bottom_height;
    assert_eq!(tree.render_size(header), Some(Size::new(200., natural)));
    assert_eq!(controller.content_extent(), 800. + natural);

    // Stretching preserves the measured bottom and fills the toolbar.
    controller.apply_physics(physics, -30.);
    let stretch = -controller.offset();
    assert!(stretch > 0.);
    tree.layout(narrow).expect("overscroll layout");
    tree.update_compositor(Instant::now()).expect("compositor");
    let height = natural + stretch;
    assert_eq!(tree.render_size(header), Some(Size::new(200., height)));
    let display = tree.paint();
    assert!(
        display.commands().iter().any(|command| matches!(command,
            PaintCommand::RRect { rrect, brush: Brush::Solid(color), .. }
            if *color == background && rrect.rect.size.height == height - bottom_height
        )),
        "toolbar must fill remaining height above wrapped bottom"
    );
    tree.update_semantics();
    let stretched_bottom = tree
        .semantics()
        .iter()
        .find_map(|(_, node)| (node.label.as_deref() == Some("Bottom")).then_some(node.bounds))
        .expect("stretched bottom");
    assert_eq!(stretched_bottom.size.height, bottom_height);
    assert_eq!(stretched_bottom.origin.y, height - bottom_height);
    assert!(controller.jump_to(0.));
    tree.layout(narrow).expect("settled layout");

    // Widening the viewport re-wraps the same retained bottom (same delegate,
    // no reconstruction) and establishes a new natural extent and range.
    let wide = Constraints::tight(Size::new(300., 200.));
    tree.layout(wide).expect("wide layout");
    tree.update_semantics();
    let wide_bottom = tree
        .semantics()
        .iter()
        .find_map(|(_, node)| {
            (node.label.as_deref() == Some("Bottom")).then_some(node.bounds.size.height)
        })
        .expect("wide bottom");
    assert!(
        wide_bottom < bottom_height,
        "wider viewport must shorten wrapped bottom"
    );
    let wide_natural = 40. + wide_bottom;
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(300., wide_natural))
    );
    assert_eq!(controller.content_extent(), 800. + wide_natural);

    // Resizing back during overscroll keeps the re-measured bottom with the
    // toolbar taking the remainder; no drift accumulates.
    controller.apply_physics(physics, -30.);
    let stretch = -controller.offset();
    assert!(stretch > 0.);
    tree.layout(wide).expect("wide overscroll layout");
    tree.update_compositor(Instant::now()).expect("compositor");
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(300., wide_natural + stretch))
    );
    tree.layout(narrow).expect("narrow overscroll layout");
    tree.update_semantics();
    let rewrapped = tree
        .semantics()
        .iter()
        .find_map(|(_, node)| {
            (node.label.as_deref() == Some("Bottom")).then_some(node.bounds.size.height)
        })
        .expect("rewrapped bottom");
    assert_eq!(rewrapped, bottom_height);
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(200., wide_natural + stretch))
    );
    assert!(controller.jump_to(0.));
    tree.layout(narrow).expect("recovered layout");
    assert_eq!(tree.render_size(header), Some(Size::new(200., natural)));
    assert_eq!(controller.content_extent(), 800. + natural);
    controller.apply_physics(physics, -30.);
    tree.layout(narrow).expect("repeat overscroll layout");
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(200., natural + -controller.offset()))
    );
    assert!(controller.jump_to(0.));
    tree.layout(narrow).expect("repeat settled layout");
    assert_eq!(tree.render_size(header), Some(Size::new(200., natural)));
}

#[test]
fn natural_header_replacement_during_overscroll_keeps_true_size() {
    use incular_widgets::{LayoutBuilder, Semantics, SizedBox};

    let controller = ScrollController::new();
    let physics = ScrollPhysics::default().bouncing();
    let bottom = || -> Widget {
        Semantics::new(Widget::from(LayoutBuilder::new(|_, _| {
            Widget::from(SizedBox::new().height(20.))
        })))
        .role(incular_semantics::Role::Group)
        .label("Bottom")
        .into()
    };
    let view = |title: &str| {
        let slivers: Vec<Box<dyn Sliver>> = vec![
            Box::new(
                SliverAppBar::from_app_bar(
                    AppBar::new(Text::new(title))
                        .toolbar_height(40.)
                        .bottom(bottom()),
                )
                .pinned(true)
                .stretch(true),
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
    let root = tree.mount(view("First")).expect("mount");
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let header = tree
        .render_id(tree.children(root).expect("header")[0])
        .expect("render");
    assert_eq!(tree.render_size(header), Some(Size::new(200., 60.)));
    controller.apply_physics(physics, -30.);
    let stretch = -controller.offset();
    assert!(stretch > 0.);
    // Replace the header descriptor while overscroll is active. The new
    // retained header must present the true natural size plus overscroll
    // (not an unverified estimate), and the scroll activity must survive.
    tree.update(root, view("Second")).expect("update");
    tree.layout(constraints).expect("replaced layout");
    tree.update_compositor(Instant::now()).expect("compositor");
    assert_eq!(controller.offset(), -stretch);
    assert_eq!(
        tree.render_size(header),
        Some(Size::new(200., 60. + stretch))
    );
    assert_eq!(controller.content_extent(), 860.);
    assert!(controller.jump_to(0.));
    tree.layout(constraints).expect("settled layout");
    assert_eq!(tree.render_size(header), Some(Size::new(200., 60.)));
    assert_eq!(controller.content_extent(), 860.);
}
