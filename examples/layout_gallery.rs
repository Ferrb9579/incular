//! A visual gallery for the retained layout primitives.
use incular::prelude::*;
use incular::widgets::internal::ScrollView;

const SURFACE: Color = Color::rgba(35, 42, 58, 255);
const MUTED: Color = Color::rgba(190, 202, 222, 255);

fn text(value: impl Into<String>, size: f32, color: Color) -> Widget {
    Text::new(value)
        .style(TextStyle {
            size,
            color,
            ..TextStyle::default()
        })
        .into()
}

fn heading(value: &str) -> Widget {
    Padding::all(8., text(value, 20., Color::WHITE)).into()
}

fn card(child: impl Into<Widget>) -> Widget {
    DecoratedBox::new(Padding::all(12., child))
        .background(SURFACE)
        .border(Border::new(1., Color::rgba(85, 100, 130, 255)))
        .radius(10.)
        .into()
}

fn chip(label: &str, color: Color) -> Widget {
    DecoratedBox::new(Padding::all(8., text(label, 15., Color::WHITE)))
        .background(color)
        .radius(14.)
        .into()
}

fn main() {
    let controller = ScrollController::new();
    let app = Application::new(move |_| {
        ScrollView::vertical(
            controller.clone(),
            Padding::all(
                16.,
                Column::new([
                    text(
                        "Incular layout gallery",
                        30.,
                        Color::rgba(120, 190, 255, 255),
                    ),
                    text(
                        "Scroll through concrete retained layout primitives",
                        16.,
                        MUTED,
                    ),
                    heading("Wrap · flowing tags"),
                    card(
                        Wrap::new([
                            chip("layout", Color::rgba(73, 125, 205, 255)),
                            chip("painting", Color::rgba(117, 83, 198, 255)),
                            chip("semantics", Color::rgba(39, 157, 139, 255)),
                            chip("animation", Color::rgba(197, 83, 123, 255)),
                            chip("native", Color::rgba(193, 130, 54, 255)),
                            chip("widgets", Color::rgba(52, 155, 194, 255)),
                        ])
                        .spacing(8.)
                        .run_spacing(8.),
                    ),
                    heading("Table · measured grid"),
                    card(
                        Table::new(
                            3,
                            [
                                text("Primitive", 16., Color::WHITE),
                                text("Purpose", 16., Color::WHITE),
                                text("Status", 16., Color::WHITE),
                                text("Stack", 15., MUTED),
                                text("Layering", 15., MUTED),
                                text("Ready", 15., Color::rgba(110, 225, 168, 255)),
                                text("AspectRatio", 15., MUTED),
                                text("Proportions", 15., MUTED),
                                text("Ready", 15., Color::rgba(110, 225, 168, 255)),
                            ],
                        )
                        .column_spacing(26.)
                        .row_spacing(10.),
                    ),
                    heading("Stack · overlay paint order"),
                    card(
                        SizedBox::from_size(Size::new(360., 150.)).child(
                            Stack::new([
                                DecoratedBox::new(Widget::box_(
                                    Size::new(300., 112.),
                                    Color::TRANSPARENT,
                                ))
                                .background(Color::rgba(48, 103, 190, 255))
                                .radius(20.)
                                .into(),
                                Align::new(
                                    Alignment::TOP_LEFT,
                                    Padding::all(18., text("top-left", 15., Color::WHITE)),
                                )
                                .into(),
                                Align::new(
                                    Alignment::BOTTOM_RIGHT,
                                    Padding::all(18., text("front-most", 15., Color::WHITE)),
                                )
                                .into(),
                                text("Stack", 30., Color::WHITE),
                            ])
                            .alignment(Alignment::CENTER),
                        ),
                    ),
                    heading("Fractional sizing + AspectRatio"),
                    card(
                        SizedBox::from_size(Size::new(400., 140.)).child(
                            FractionallySizedBox::new(AspectRatio::new(
                                16. / 9.,
                                DecoratedBox::new(Center::new(text("16 : 9", 28., Color::WHITE)))
                                    .background(Color::rgba(120, 74, 185, 255))
                                    .radius(12.),
                            ))
                            .width_factor(0.70)
                            .height_factor(0.90),
                        ),
                    ),
                    heading("Baseline + constraints"),
                    card(Row::new(vec![
                        Widget::from(Baseline::new(
                            38.,
                            text("Baseline", 28., Color::rgba(255, 203, 102, 255)),
                        )),
                        Widget::from(Baseline::new(38., text("aligned", 16., Color::WHITE))),
                        Widget::from(ConstrainedBox::new(
                            Constraints::tight(Size::new(112., 44.)),
                            DecoratedBox::new(Center::new(text("tight", 15., Color::WHITE)))
                                .background(Color::rgba(35, 150, 133, 255))
                                .radius(8.),
                        )),
                    ])),
                    heading("UnconstrainedBox + Visibility"),
                    card(Row::new(vec![
                        Widget::from(UnconstrainedBox::new(
                            DecoratedBox::new(text("natural width", 16., Color::WHITE))
                                .background(Color::rgba(191, 79, 117, 255))
                                .radius(8.),
                        )),
                        Widget::from(
                            Visibility::new(text("This stays hidden", 16., Color::WHITE))
                                .visible(false),
                        ),
                        text("Hidden sibling omitted", 16., MUTED),
                    ])),
                ]),
            ),
        )
    })
    .expect("valid layout gallery application");
    incular::run(app).expect("native layout gallery application");
}
