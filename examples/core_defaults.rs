//! Core Widget Defaults Gallery
//!
//! Demonstrates Flutter-like structural, layout, and behavioral defaults
//! on unstyled core primitives without arbitrary injected styling.

use incular::prelude::*;

fn main() {
    let _app = Application::new(|_cx| {
        let title = Text::new("Incular Core Widget Defaults")
            .style(TextStyle::new().font_size(20.0).bold());

        let subtitle = Text::new(
            "Verifying unstyled primitive behavior: Container expansion, SizedBox sizing, Align positioning, and Raw Gestures.",
        )
        .style(TextStyle::new().font_size(13.0).color(Color::rgba(140, 145, 160, 255)));

        // 1. Container Bounded vs Unbounded Sizing
        let bounded_container_demo = Column::new([
            Widget::from(Text::new("1. Container Expansion in Bounded Parent").style(TextStyle::new().bold())),
            Widget::from(Text::new("Empty Container() fills available bounded width/height with 0 visual paint commands.")
                .style(TextStyle::new().font_size(12.0))),
            Widget::from(Container::new()
                .height(60.0)
                .color(Color::rgba(45, 52, 70, 255))
                .alignment(Alignment::CENTER)
                .child(
                    Text::new("Container fills width, aligns child to center")
                        .style(TextStyle::new().font_size(12.0).color(Color::WHITE)),
                )),
        ])
        .spacing(6.0);

        // 2. SizedBox and ConstrainedBox
        let sizing_demo = Column::new([
            Widget::from(
                Text::new("2. SizedBox and ConstrainedBox").style(TextStyle::new().bold()),
            ),
            Widget::from(Row::new([
                Widget::from(
                    SizedBox::new().width(120.0).height(36.0).child(
                        Container::new()
                            .color(Color::rgba(60, 70, 95, 255))
                            .alignment(Alignment::CENTER)
                            .child(Text::new("120x36").style(TextStyle::new().font_size(12.0))),
                    ),
                ),
                Widget::from(SizedBox::new().width(12.0)),
                Widget::from(ConstrainedBox::new(
                    Constraints::new(80.0, 160.0, 36.0, 36.0),
                    Container::new()
                        .color(Color::rgba(70, 80, 110, 255))
                        .alignment(Alignment::CENTER)
                        .child(
                            Text::new("Min: 80, Max: 160").style(TextStyle::new().font_size(12.0)),
                        ),
                )),
            ])),
        ])
        .spacing(6.0);

        // 3. Align and Center
        let align_demo = Column::new([
            Widget::from(Text::new("3. Align and Center Semantics").style(TextStyle::new().bold())),
            Widget::from(
                Container::new()
                    .height(80.0)
                    .color(Color::rgba(35, 40, 55, 255))
                    .child(
                        Row::new([
                            Widget::from(Align::new(
                                Alignment::TOP_LEFT,
                                Text::new("TopLeft").style(TextStyle::new().font_size(11.0)),
                            )),
                            Widget::from(Center::new(
                                Text::new("Center").style(TextStyle::new().font_size(11.0).bold()),
                            )),
                            Widget::from(Align::new(
                                Alignment::BOTTOM_RIGHT,
                                Text::new("BottomRight").style(TextStyle::new().font_size(11.0)),
                            )),
                        ])
                        .main_axis_alignment(MainAxisAlignment::SpaceBetween),
                    ),
            ),
        ])
        .spacing(6.0);

        // 4. Raw Unstyled Button
        let raw_button_demo = Column::new([
            Widget::from(Text::new("4. Raw Button Primitive (Zero Injected Paint)").style(TextStyle::new().bold())),
            Widget::from(Text::new("Button::with_child wraps hit test and tap dispatch without injecting colors or padding.")
                .style(TextStyle::new().font_size(12.0))),
            Widget::from(Button::with_child(
                Container::new()
                    .padding(EdgeInsets::symmetric(12.0, 6.0))
                    .color(Color::rgba(50, 120, 230, 255))
                    .child(Text::new("Explicitly Styled Container in Raw Button").style(TextStyle::new().color(Color::WHITE))),
            )
            .on_click(|| println!("Raw button clicked!"))),
        ])
        .spacing(6.0);

        Container::new()
            .padding(EdgeInsets::all(24.0))
            .color(Color::rgba(20, 22, 28, 255))
            .child(
                Column::new([
                    Widget::from(title),
                    Widget::from(subtitle),
                    Widget::from(SizedBox::new().height(16.0)),
                    Widget::from(bounded_container_demo),
                    Widget::from(SizedBox::new().height(12.0)),
                    Widget::from(sizing_demo),
                    Widget::from(SizedBox::new().height(12.0)),
                    Widget::from(align_demo),
                    Widget::from(SizedBox::new().height(12.0)),
                    Widget::from(raw_button_demo),
                ])
                .spacing(8.0),
            )
            .into()
    });
    incular::run(_app.expect("valid Core Widget Defaults application"))
        .expect("native Core Widget Defaults application");
}
