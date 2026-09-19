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

fn themed_box() -> Widget {
    LayoutBuilder::new(|context, _| {
        let theme = Theme::of(context).expect("ambient animated theme");
        Container::new()
            .width(40.0)
            .height(20.0)
            .color(theme.core().color_scheme.primary)
            .into()
    })
    .into()
}

fn animated_root(start: ThemeData, target: ThemeData) -> Widget {
    Theme::new(
        start,
        AnimatedTheme::new(target, Duration::from_millis(100), themed_box()),
    )
    .into()
}

#[test]
fn animated_theme_retargets_from_presented_value() {
    let start = ThemeData::from_seed(Color::rgba(220, 20, 40, 255));
    let first = ThemeData::from_seed(Color::rgba(20, 60, 220, 255));
    let second = ThemeData::from_seed(Color::rgba(20, 190, 70, 255));
    let start_primary = start.core().color_scheme.primary;
    let first_primary = first.core().color_scheme.primary;
    let second_primary = second.core().color_scheme.primary;
    let constraints = Constraints::loose(Size::new(100.0, 60.0));
    let origin = Instant::now();

    let mut tree = WidgetTree::new();
    let root = tree
        .mount(animated_root(start.clone(), first.clone()))
        .expect("mount animated theme");
    tree.layout(constraints).expect("initial layout");
    tree.update_compositor(origin).expect("start first target");
    tree.update_compositor(origin + Duration::from_millis(50))
        .expect("advance first target");
    tree.layout(constraints).expect("first midpoint layout");
    let presented = start_primary.lerp(&first_primary, 0.5);
    assert_eq!(painted_surface(&mut tree), presented);

    tree.update(root, animated_root(start, second))
        .expect("retarget animated theme");
    tree.layout(constraints).expect("retarget layout");
    assert_eq!(painted_surface(&mut tree), presented);

    tree.update_compositor(origin + Duration::from_millis(50))
        .expect("start retarget");
    tree.update_compositor(origin + Duration::from_millis(100))
        .expect("advance retarget");
    tree.layout(constraints).expect("retarget midpoint layout");
    assert_eq!(
        painted_surface(&mut tree),
        presented.lerp(&second_primary, 0.5)
    );
}

#[test]
fn equal_animated_theme_target_preserves_in_flight_timeline() {
    let start = ThemeData::from_seed(Color::rgba(220, 20, 40, 255));
    let target = ThemeData::from_seed(Color::rgba(20, 60, 220, 255));
    let start_primary = start.core().color_scheme.primary;
    let target_primary = target.core().color_scheme.primary;
    let constraints = Constraints::loose(Size::new(100.0, 60.0));
    let origin = Instant::now();

    let mut tree = WidgetTree::new();
    let root = tree
        .mount(animated_root(start.clone(), target.clone()))
        .expect("mount animated theme");
    tree.layout(constraints).expect("initial layout");
    tree.update_compositor(origin).expect("start animation");
    tree.update_compositor(origin + Duration::from_millis(50))
        .expect("advance animation");
    tree.layout(constraints).expect("midpoint layout");

    tree.update(root, animated_root(start, target))
        .expect("rebuild equal target");
    tree.update_compositor(origin + Duration::from_millis(75))
        .expect("continue old timeline");
    tree.layout(constraints).expect("continued layout");
    assert_eq!(
        painted_surface(&mut tree),
        start_primary.lerp(&target_primary, 0.75)
    );
}
