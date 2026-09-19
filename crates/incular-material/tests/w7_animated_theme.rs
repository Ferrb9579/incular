use std::time::{Duration, Instant};

use incular_config::Constraints;
use incular_core::{Color, Lerp, Size};
use incular_material::{AnimatedTheme, Theme, ThemeData};
use incular_rendering::{Brush, PaintCommand};
use incular_widgets::{Container, LayoutBuilder, Widget, internal::WidgetTree};

fn painted_surface(tree: &mut WidgetTree) -> Color {
    tree.paint()
        .commands()
        .iter()
        .find_map(|command| match command {
            PaintCommand::RRect {
                brush: Brush::Solid(color),
                ..
            } => Some(*color),
            _ => None,
        })
        .expect("themed surface color")
}

#[test]
fn animated_theme_uses_retained_frame_ticks_instead_of_discarding_duration() {
    let start = ThemeData::from_seed(Color::rgba(220, 20, 40, 255));
    let target = ThemeData::from_seed(Color::rgba(20, 60, 220, 255));
    let start_primary = start.core().color_scheme.primary;
    let target_primary = target.core().color_scheme.primary;
    let content: Widget = LayoutBuilder::new(|context, _| {
        let theme = Theme::of(context).expect("ambient animated theme");
        Container::new()
            .width(40.0)
            .height(20.0)
            .color(theme.core().color_scheme.primary)
            .into()
    })
    .into();
    let root: Widget = Theme::new(
        start,
        AnimatedTheme::new(target, Duration::from_millis(100), content),
    )
    .into();

    let mut tree = WidgetTree::new();
    tree.mount(root).expect("mount animated theme");
    let constraints = Constraints::loose(Size::new(100.0, 60.0));
    tree.layout(constraints).expect("initial layout");
    assert_eq!(painted_surface(&mut tree), start_primary);

    let origin = Instant::now();
    tree.update_compositor(origin).expect("start animation");
    tree.update_compositor(origin + Duration::from_millis(50))
        .expect("mid animation");
    tree.layout(constraints).expect("mid animation rebuild");
    assert_eq!(
        painted_surface(&mut tree),
        start_primary.lerp(&target_primary, 0.5)
    );

    tree.update_compositor(origin + Duration::from_millis(100))
        .expect("finish animation");
    tree.layout(constraints).expect("final animation rebuild");
    assert_eq!(painted_surface(&mut tree), target_primary);
}

#[test]
fn zero_duration_animated_theme_settles_immediately() {
    let target = ThemeData::from_seed(Color::rgba(20, 60, 220, 255));
    let expected = target.core().color_scheme.primary;
    let content: Widget = LayoutBuilder::new(|context, _| {
        let theme = Theme::of(context).expect("ambient theme");
        Container::new()
            .width(40.0)
            .height(20.0)
            .color(theme.core().color_scheme.primary)
            .into()
    })
    .into();
    let mut tree = WidgetTree::new();
    tree.mount(AnimatedTheme::new(target, Duration::ZERO, content).into())
        .expect("mount zero duration theme");
    tree.layout(Constraints::loose(Size::new(100.0, 60.0)))
        .expect("layout");
    assert_eq!(painted_surface(&mut tree), expected);
}
