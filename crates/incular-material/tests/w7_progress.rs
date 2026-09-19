use incular_config::Constraints;
use incular_core::{Color, Size};
use incular_material::{
    CircularProgressIndicator, ProgressIndicatorStrokeCap, ProgressIndicatorTheme,
    ProgressIndicatorThemeData,
};
use incular_rendering::{Brush, LineCap, PaintCommand};
use incular_widgets::internal::WidgetTree;

fn paint(
    indicator: CircularProgressIndicator,
    theme: ProgressIndicatorThemeData,
) -> Vec<PaintCommand> {
    let mut tree = WidgetTree::new();
    tree.mount(ProgressIndicatorTheme::new(theme, indicator).into())
        .expect("mount progress");
    tree.layout(Constraints::loose(Size::new(120.0, 120.0)))
        .expect("layout progress");
    tree.paint().commands().to_vec()
}

#[test]
fn explicit_progress_stroke_width_overrides_theme_instead_of_taking_numeric_max() {
    let commands = paint(
        CircularProgressIndicator::new()
            .value(0.5)
            .stroke_width(2.0),
        ProgressIndicatorThemeData::default().stroke_width(8.0),
    );
    assert!(commands.iter().any(|command| matches!(
        command,
        PaintCommand::StrokePath { stroke, .. } if (stroke.width - 2.0).abs() < f32::EPSILON
    )));
}

#[test]
fn circular_progress_consumes_theme_stroke_cap_and_width() {
    let commands = paint(
        CircularProgressIndicator::new().value(0.5),
        ProgressIndicatorThemeData::default()
            .stroke_width(5.0)
            .stroke_cap(ProgressIndicatorStrokeCap::Square)
            .color(Color::rgba(10, 20, 30, 255)),
    );
    assert!(commands.iter().any(|command| matches!(
        command,
        PaintCommand::StrokePath { stroke, brush: Brush::Solid(color), .. }
            if (stroke.width - 5.0).abs() < f32::EPSILON
                && stroke.cap == LineCap::Square
                && *color == Color::rgba(10, 20, 30, 255)
    )));
}
