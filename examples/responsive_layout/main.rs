//! Resize this window to inspect the public configuration values flowing
//! through retained layout widgets: constraints, insets, alignment and wraps.
use incular::prelude::*;

fn swatch(name: &str, color: Color) -> Widget {
    DecoratedBox::new(Center::new(Text::new(name).color(Color::WHITE)))
        .size(Size::new(108., 52.))
        .background(color)
        .radius(12.)
        .border(Border::new(1., Color::rgba(255, 255, 255, 105)))
        .into()
}

#[path = "../support/mod.rs"]
mod example_support;
#[cfg(test)]
#[path = "tests.rs"]
mod example_tests;
mod simulations;

fn main() {
    let app = Application::new(move |_| {
        let table_cells: Vec<Widget> = vec![
            Text::new("Constraints")
                .color(Color::rgba(150, 215, 255, 255))
                .into(),
            Text::new("max width: 560 logical pixels").into(),
            Text::new("Insets")
                .color(Color::rgba(150, 215, 255, 255))
                .into(),
            Text::new("24 horizontal / 20 vertical").into(),
            Text::new("Alignment")
                .color(Color::rgba(150, 215, 255, 255))
                .into(),
            Text::new("bottom-right media caption").into(),
        ];
        ConstrainedBox::new(
            Constraints::new(0., 560., 0., 760.),
            Padding::new(
                EdgeInsets::symmetric(24., 20.),
                Widget::column(vec![
                    Text::new("Responsive configuration gallery")
                        .style(TextStyle {
                            size: 26.,
                            color: Color::rgba(255, 230, 165, 255),
                            ..TextStyle::default()
                        })
                        .into(),
                    Text::new("A bounded Wrap reflows swatches; the next card uses a fractional width and a 16:9 aspect ratio.")
                        .color(Color::rgba(215, 225, 245, 255))
                        .into(),
                    Padding::all(
                        12.,
                        Wrap::new([
                            swatch("ocean", Color::rgba(48, 118, 220, 255)),
                            swatch("mint", Color::rgba(45, 184, 151, 255)),
                            swatch("sun", Color::rgba(242, 166, 55, 255)),
                            swatch("plum", Color::rgba(151, 79, 202, 255)),
                            swatch("rose", Color::rgba(222, 83, 119, 255)),
                        ])
                        .spacing(12.)
                        .run_spacing(12.),
                    )
                    .into(),
                    FractionallySizedBox::new(AspectRatio::new(
                        16. / 9.,
                        DecoratedBox::new(Align::new(
                            Alignment::BOTTOM_RIGHT,
                            Padding::all(
                                14.,
                                Text::new("fractionally sized media panel")
                                    .color(Color::WHITE),
                            ),
                        ))
                        .background(LinearGradient {
                            start: Offset::ZERO,
                            end: Offset::new(420., 190.),
                            stops: GradientStops::new(vec![
                                GradientStop { offset: 0., color: Color::rgba(42, 72, 154, 255) },
                                GradientStop { offset: 1., color: Color::rgba(170, 72, 158, 255) },
                            ]),
                        })
                        .radius(18.),
                    ))
                    .width_factor(0.9)
                    .into(),
                    Padding::new(
                        EdgeInsets::only(0., 18., 0., 0.),
                        Table::new(2, table_cells)
                        .column_spacing(26.)
                        .row_spacing(8.),
                    )
                    .into(),
                ]),
            ),
        )
        .into()
    })
    .expect("valid responsive layout application");
    example_support::spawn_if_requested(app.simulation(), simulations::run);
    incular::run(app).expect("native responsive layout application");
}
