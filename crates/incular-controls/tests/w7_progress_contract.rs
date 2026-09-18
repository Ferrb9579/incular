use incular_config::Constraints;
use incular_controls::{ControlTheme, progress::Root as Progress};
use incular_core::{Color, Size};
use incular_semantics::Role;
use incular_widgets::{Widget, internal::WidgetTree};

#[test]
fn custom_progress_keeps_busy_range_and_label_semantics() {
    let mut tree = WidgetTree::new();
    tree.mount(
        Progress::with_child(Widget::box_(Size::new(120.0, 8.0), Color::WHITE))
            .range(10.0, 20.0)
            .indeterminate(true)
            .label("Upload")
            .build(&ControlTheme::light()),
    )
    .expect("mount custom progress");
    tree.layout(Constraints::loose(Size::new(200.0, 40.0)))
        .expect("layout");
    tree.update_semantics();
    let node = tree
        .semantics()
        .iter()
        .find(|(_, node)| node.role == Role::ProgressBar)
        .map(|(_, node)| node)
        .expect("progress semantics");
    assert_eq!(node.label.as_deref(), Some("Upload"));
    assert!(node.state.busy);
    assert_eq!(node.state.numeric_min, Some(10.0));
    assert_eq!(node.state.numeric_max, Some(20.0));
    assert_eq!(node.state.numeric_value, None);
}

#[test]
fn zero_progress_has_no_active_fill_command() {
    let mut tree = WidgetTree::new();
    tree.mount(Progress::new().value(0.0).build(&ControlTheme::light()))
        .expect("mount progress");
    tree.layout(Constraints::loose(Size::new(240.0, 40.0)))
        .expect("layout");
    let commands = tree.paint();
    let accent = ControlTheme::light().colors.accent;
    assert!(!commands.commands().iter().any(|command| {
        matches!(
            command,
            incular_rendering::PaintCommand::RRect {
                brush: incular_rendering::Brush::Solid(color),
                ..
            } if *color == accent
        )
    }));
}
