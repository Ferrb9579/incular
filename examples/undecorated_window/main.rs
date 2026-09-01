//! A native window without the operating system's title bar or border.
//!
//! Run with `cargo run -p incular --example undecorated_window`.
use incular::prelude::*;

#[path = "../tests/support/mod.rs"]
pub(crate) mod example_support;
#[path = "tests/simulations.rs"]
pub(crate) mod simulations;

fn chrome_button(label: &'static str, on_tap: impl Fn() + 'static) -> Widget {
    GestureDetector::new(
        Container::new()
            .width(48.0)
            .height(44.0)
            .alignment(Alignment::CENTER)
            .child(Text::new(label).style(TextStyle::new().font_size(14.0).color(Color::WHITE))),
    )
    .on_tap(on_tap)
    .into()
}

fn main() {
    let options = WindowOptions {
        initial_logical_size: Size::new(520.0, 320.0),
        size_policy: WindowSizePolicy::Content,
        decorations: false,
        ..WindowOptions::new("Incular Undecorated Window")
    };

    let app = Application::new_with_options(options, |cx| {
        let window = cx.window_handle().expect("managed desktop window");
        let minimize = window.clone();
        let close = window;

        let title: Widget = Container::new()
            .width(424.0)
            .height(44.0)
            .alignment(Alignment::CENTER_LEFT)
            .padding(EdgeInsets::symmetric(0.0, 16.0))
            .child(
                Text::new("Incular custom chrome")
                    .style(TextStyle::new().font_size(13.0).bold().color(Color::WHITE)),
            )
            .into();
        let chrome: Widget =
            WindowDragRegion::new(Container::new().color(Color::rgba(15, 23, 42, 255)).child(
                Row::new([
                    title,
                    chrome_button("—", move || {
                        let _ = minimize.set_minimized(true);
                    }),
                    chrome_button("×", move || {
                        let _ = close.close();
                    }),
                ]),
            ))
            .into();

        let body: Widget = Container::new()
            .width(520.0)
            .height(276.0)
            .color(Color::rgba(15, 23, 42, 255))
            .alignment(Alignment::CENTER)
            .child(
                Container::new()
                    .width(420.0)
                    .padding(EdgeInsets::all(32.0))
                    .decoration(
                        BoxDecoration::new()
                            .color(Color::rgba(30, 41, 59, 255))
                            .border(Border::new(1.0, Color::rgba(71, 85, 105, 255)))
                            .border_radius(BorderRadius::circular(18.0)),
                    )
                    .child(
                        Column::new([
                            Widget::from(
                                Text::new("NATIVE CUSTOM CHROME").style(
                                    TextStyle::new()
                                        .font_size(12.0)
                                        .bold()
                                        .letter_spacing(2.0)
                                        .color(Color::rgba(45, 212, 191, 255)),
                                ),
                            ),
                            Widget::from(Text::new("Drag the title bar.").style(
                                TextStyle::new().font_size(26.0).bold().color(Color::WHITE),
                            )),
                            Widget::from(
                                Text::new(
                                    "WindowDragRegion delegates movement to the native window \
                                     manager. The minimize and close controls remain interactive \
                                     descendants and are not swallowed by the drag region.",
                                )
                                .style(
                                    TextStyle::new()
                                        .font_size(14.0)
                                        .line_height_multiplier(1.45)
                                        .color(Color::rgba(203, 213, 225, 255)),
                                ),
                            ),
                        ])
                        .main_axis_size(MainAxisSize::Min)
                        .spacing(14.0),
                    ),
            )
            .into();
        Column::new([chrome, body]).into()
    })
    .expect("valid undecorated-window application");

    example_support::spawn_if_requested(app.simulation(), simulations::run);
    incular::run(app).expect("native undecorated-window application");
}
