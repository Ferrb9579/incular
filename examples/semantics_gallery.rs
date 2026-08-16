//! A visible control panel plus a printed retained semantics tree. Run this
//! example from a terminal to inspect the accessibility nodes it produces.
use incular::{prelude::*, widgets::WidgetTree};

fn dashboard() -> Widget {
    let controller = TextEditingController::new();
    let heading: Widget = Text::new("Accessible controls")
        .style(TextStyle {
            size: 28.,
            color: Color::rgba(255, 230, 165, 255),
            ..TextStyle::default()
        })
        .into();
    let decorative: Widget = Padding::all(
        12.,
        DecoratedBox::new(
            Text::new("decorative paint is intentionally excluded from screen readers")
                .color(Color::rgba(180, 190, 220, 255)),
        )
        .size(Size::new(440., 48.))
        .background(Color::rgba(38, 50, 88, 255))
        .radius(10.),
    )
    .into();
    let name_field: Widget = TextField::new(controller)
        .placeholder("Your display name")
        .into();
    Widget::column(vec![
        heading
            .accessibility_label("Settings heading")
            .accessibility_description("Demonstration of the retained semantic tree"),
        Text::new("Each meaningful control has a role, label, bounds, and supported actions.")
            .color(Color::rgba(215, 225, 245, 255))
            .into(),
        decorative.exclude_semantics(),
        name_field
            .accessibility_label("Display name")
            .accessibility_description("Enter the name shown to other users"),
        Widget::row(vec![
            Button::new("Save")
                .on_press(|| println!("save activated"))
                .into(),
            Button::new("Cancel")
                .color(Color::rgba(112, 76, 156, 255))
                .on_press(|| println!("cancel activated"))
                .into(),
        ]),
        Widget::box_(Size::new(280., 26.), Color::rgba(48, 187, 147, 255))
            .accessibility_label("Storage usage")
            .accessibility_description("42 percent of available storage used"),
    ])
}

fn main() {
    let mut tree = WidgetTree::new();
    tree.mount(dashboard()).expect("mount semantic dashboard");
    tree.layout(Constraints::tight(Size::new(640., 460.)));
    tree.update_semantics();
    println!("{}", tree.semantics_debug_dump());
    println!("semantics diagnostics: {:?}", tree.semantics_diagnostics());

    let app = Application::new(move |_| dashboard()).expect("valid semantics application");
    incular::run(app).expect("native semantics application");
}
