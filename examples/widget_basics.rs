//! Widget Basics Example
//! Demonstrates unstyled core primitives, constraint semantics, and decoupled styled components.

use incular::prelude::*;
use incular::material::RawMaterialButton;
use incular::widgets::internal::SplitView;

fn main() {
    let app = Application::new(|_cx| {
        // 1. Unstyled neutral Button: sizes strictly to its child, transparent background
        let neutral_button = RawMaterialButton::with_child(
            Text::new("Neutral Button (Unstyled)")
                .style(TextStyle::new().font_size(13.0).color(Color::WHITE)),
        )
        .on_click(|| println!("Clicked unstyled button"));

        // 2. Application-styled Button: uses Container for padding, background, border, radius
        let styled_button = RawMaterialButton::with_child(
            Container::new()
                .padding(EdgeInsets::symmetric(14.0, 8.0))
                .decoration(
                    BoxDecoration::new()
                        .color(Color::rgba(60, 110, 220, 255))
                        .border_radius(BorderRadius::all(Radius::circular(6.0))),
                )
                .child(
                    Text::new("Styled Action Button")
                        .style(
                            TextStyle::new()
                                .font_size(13.0)
                                .font_weight(FontWeight::BOLD)
                                .color(Color::WHITE),
                        ),
                ),
        )
        .on_click(|| println!("Clicked styled action button"));

        // 3. Multi-line typography with explicit LineHeight::Multiplier
        let multi_line_text = Text::new(
            "Line 1: Typography respects multiplier line heights.\n\
             Line 2: Lines are properly spaced without overlap.\n\
             Line 3: 1.5x multiplier means 1.5 * font_size logical pixels.",
        )
        .style(
            TextStyle::new()
                .font_size(14.0)
                .line_height_multiplier(1.5)
                .color(Color::rgba(220, 225, 235, 255)),
        );

        // 4. SplitView with explicit second_extent
        let left_pane = Container::new()
            .padding(EdgeInsets::all(16.0))
            .color(Color::rgba(30, 32, 38, 255))
            .child(
                Column::new([
                    Text::new("Left Pane (Expanded)")
                        .style(
                            TextStyle::new()
                                .font_size(16.0)
                                .font_weight(FontWeight::BOLD)
                                .color(Color::WHITE),
                        )
                        .into(),
                    SizedBox::new().height(12.0).into(),
                    multi_line_text.into(),
                    SizedBox::new().height(16.0).into(),
                    Row::new([neutral_button.into(), SizedBox::new().width(12.0).into(), styled_button.into()]).into(),
                ]),
            );

        let right_pane = Container::new()
            .padding(EdgeInsets::all(16.0))
            .color(Color::rgba(38, 40, 48, 255))
            .child(
                Column::new([
                    Text::new("Right Pane (Fixed 280px)")
                        .style(
                            TextStyle::new()
                                .font_size(14.0)
                                .font_weight(FontWeight::BOLD)
                                .color(Color::rgba(140, 180, 255, 255)),
                        )
                        .into(),
                    SizedBox::new().height(8.0).into(),
                    Text::new("Uses SplitView::horizontal(...).second_extent(280.0)")
                        .style(
                            TextStyle::new()
                                .font_size(12.0)
                                .color(Color::rgba(180, 185, 195, 255)),
                        )
                        .into(),
                ]),
            );

        let split_demo = SplitView::horizontal(left_pane, right_pane)
            .second_extent(280.0)
            .divider_thickness(6.0)
            .divider_color(Color::rgba(60, 65, 75, 255));

        Container::new()
            .color(Color::rgba(20, 22, 26, 255))
            .child(split_demo)
            .into()
    })
    .expect("valid basics app");

    if let Err(err) = incular::run(app) {
        eprintln!("Error running widget basics example: {err:?}");
    }
}
