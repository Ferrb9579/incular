//! A native window without the operating system's title bar or border.
//!
//! Run with `cargo run -p incular --example undecorated_window`.
use incular::prelude::*;

#[path = "../tests/support/mod.rs"]
pub(crate) mod example_support;
#[path = "tests/simulations.rs"]
pub(crate) mod simulations;

fn main() {
    let options = WindowOptions {
        initial_logical_size: Size::new(520.0, 320.0),
        size_policy: WindowSizePolicy::Content,
        decorations: false,
        ..WindowOptions::new("Incular Undecorated Window")
    };

    let app = Application::new_with_options(options, |_cx| {
        Container::new()
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
                                Text::new("UNDECORATED WINDOW").style(
                                    TextStyle::new()
                                        .font_size(12.0)
                                        .bold()
                                        .letter_spacing(2.0)
                                        .color(Color::rgba(45, 212, 191, 255)),
                                ),
                            ),
                            Widget::from(Text::new("The native frame is gone.").style(
                                TextStyle::new().font_size(26.0).bold().color(Color::WHITE),
                            )),
                            Widget::from(
                                Text::new(
                                    "This content reaches the window edges because \
                                     decorations are disabled. Content sizing also lets the native \
                                     host follow primary layout changes without coupling menus or \
                                     other transient overlays to top-level size.",
                                )
                                .style(
                                    TextStyle::new()
                                        .font_size(14.0)
                                        .line_height_multiplier(1.45)
                                        .color(Color::rgba(203, 213, 225, 255)),
                                ),
                            ),
                            Widget::from(
                                Text::new("Use your platform's window shortcut to close it.")
                                    .style(
                                        TextStyle::new()
                                            .font_size(12.0)
                                            .color(Color::rgba(148, 163, 184, 255)),
                                    ),
                            ),
                        ])
                        .main_axis_size(MainAxisSize::Min)
                        .spacing(14.0),
                    ),
            )
            .into()
    })
    .expect("valid undecorated-window application");

    example_support::spawn_if_requested(app.simulation(), simulations::run);
    incular::run(app).expect("native undecorated-window application");
}
