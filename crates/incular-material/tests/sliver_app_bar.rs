use incular_config::Constraints;
use incular_core::{Color, Offset, Size};
use incular_material::{AppBar, SliverAppBar};
use incular_scroll::ScrollController;
use incular_widgets::{
    CustomScrollView, Sliver, SliverToBoxAdapter, Text, Widget, internal::WidgetTree,
};
use std::time::Instant;

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
