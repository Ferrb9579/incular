//! Core Flutter `widgets.dart` parity gallery.
//!
//! This example intentionally imports only `incular::prelude::*`.  It is a
//! visual smoke test for layout, clipping, text inheritance, lazy scrolling,
//! paging, and retained transitions without a Material or controls dependency.

use incular::prelude::*;

const PANEL: Color = Color::rgba(38, 46, 66, 255);
const ACCENT: Color = Color::rgba(90, 150, 245, 255);

fn label(value: impl Into<String>, size: f32) -> Widget {
    Text::new(value)
        .style(TextStyle::new().font_size(size).color(Color::WHITE))
        .into()
}

fn panel(title: &str, child: impl Into<Widget>) -> Widget {
    Container::new()
        .padding(EdgeInsets::all(14.))
        .margin(EdgeInsets::all(6.))
        .decoration(
            BoxDecoration::new()
                .color(PANEL)
                .border_radius(BorderRadius::circular(10.)),
        )
        .child(Widget::column(vec![label(title, 16.), child.into()]))
        .into()
}

#[path = "../tests/support/mod.rs"]
pub(crate) mod example_support;
#[path = "tests/simulations.rs"]
pub(crate) mod simulations;

fn main() {
    let page = PageView::new([
        panel(
            "Layout and clipping",
            Widget::column(vec![
                label("Container expands under bounded constraints", 13.),
                SizedBox::new()
                    .width(260.)
                    .height(74.)
                    .child(ClipRRect::new(
                        12.,
                        Container::new()
                            .color(ACCENT)
                            .alignment(Alignment::CENTER)
                            .child(label("ClipRRect", 18.)),
                    ))
                    .into(),
                Row::new([
                    Widget::from(Expanded::new(label("Expanded", 13.))),
                    Widget::from(SizedBox::new().width(90.).child(label("SizedBox", 13.))),
                ])
                .into(),
            ]),
        ),
        panel(
            "Text and transforms",
            DefaultTextStyle::new(
                TextStyle::new()
                    .font_size(15.)
                    .color(Color::rgba(220, 228, 244, 255)),
                Widget::column(vec![
                    label("DefaultTextStyle is inherited by descendants", 15.),
                    Transform::rotation(0.04, label("retained transform", 20.)).into(),
                    Opacity::new(0.65, label("compositor opacity", 15.)).into(),
                ]),
            ),
        ),
        panel(
            "Lazy scrolling",
            SizedBox::new().width(420.).height(260.).child(
                ListView::builder(2_000, |index| {
                    Container::new()
                        .height(34.)
                        .padding(EdgeInsets::symmetric(10., 4.))
                        .color(if index % 2 == 0 {
                            Color::rgba(48, 61, 88, 255)
                        } else {
                            Color::rgba(42, 53, 76, 255)
                        })
                        .child(label(format!("row {index:04}"), 13.))
                })
                .cache_extent(240.),
            ),
        ),
        panel(
            "Explicit transition",
            SizeTransition::new(
                0.85,
                Container::new()
                    .width(260.)
                    .height(70.)
                    .color(ACCENT)
                    .padding(EdgeInsets::all(12.))
                    .child(label("SizeTransition", 16.)),
            ),
        ),
    ])
    .scroll_direction(Axis::Horizontal);

    let root = SafeArea::new(Widget::column(vec![
        label("Flutter Widgets 3.47.1 surface", 24.),
        label(
            "No Material: layout • text • scrolling • clipping • animation",
            13.,
        ),
        SizedBox::new().height(8.).into(),
        SizedBox::new().height(620.).child(page).into(),
    ]));
    let app =
        Application::new_with_options(WindowOptions::new("Incular Widgets gallery"), move |_| {
            root.clone().into()
        })
        .expect("valid widgets gallery application");
    example_support::spawn_if_requested(app.simulation(), simulations::run);
    incular::run(app).expect("native widgets gallery application");
}
