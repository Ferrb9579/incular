//! Text shaping and grayscale glyph-raster quality on a HiDPI window.
use incular::prelude::*;

#[path = "../tests/support/mod.rs"]
pub(crate) mod example_support;
#[path = "tests/simulations.rs"]
pub(crate) mod simulations;

fn main() {
    let app = Application::new(|_| {
        Widget::from(Column::new(Vec::<Widget>::from([
            Text::new("Incular Typography")
                .style(TextStyle {
                    size: 28.0,
                    color: Color::rgba(255, 210, 120, 255),
                    ..TextStyle::default()
                })
                .into(),
            Text::new("12 px — H E F   O C S G   A V W M   0123456789")
                .style(TextStyle {
                    size: 12.0,
                    color: Color::rgba(235, 240, 250, 255),
                    ..TextStyle::default()
                })
                .into(),
            Text::new("14 px — The quick brown fox jumps over the lazy dog.")
                .style(TextStyle {
                    size: 14.0,
                    color: Color::rgba(235, 240, 250, 255),
                    ..TextStyle::default()
                })
                .into(),
            Text::new("16 px — H E F   O C S G   A V W M")
                .style(TextStyle {
                    size: 16.0,
                    color: Color::rgba(235, 240, 250, 255),
                    ..TextStyle::default()
                })
                .into(),
            Text::new("20 px — Smooth grayscale coverage")
                .style(TextStyle {
                    size: 20.0,
                    color: Color::rgba(150, 220, 255, 255),
                    ..TextStyle::default()
                })
                .into(),
            Text::new("28 px — Curves O C S G and diagonals A V W M")
                .style(TextStyle {
                    size: 28.0,
                    color: Color::rgba(220, 195, 255, 255),
                    ..TextStyle::default()
                })
                .into(),
            Text::new("40 px — Incular text")
                .style(TextStyle {
                    size: 40.0,
                    color: Color::rgba(255, 235, 180, 255),
                    ..TextStyle::default()
                })
                .into(),
            Text::new("56 px — AVOCS")
                .style(TextStyle {
                    size: 56.0,
                    color: Color::rgba(255, 255, 255, 255),
                    ..TextStyle::default()
                })
                .into(),
            Text::new(
                "Small text wraps in logical pixels; the GPU raster cache uses physical pixels.",
            )
            .style(TextStyle {
                size: 14.0,
                color: Color::rgba(210, 220, 235, 255),
                ..TextStyle::default()
            })
            .into(),
            Text::new("Unicode: café — नमस्ते — 世界 — emoji 🙂")
                .style(TextStyle {
                    size: 20.0,
                    color: Color::rgba(130, 210, 255, 255),
                    ..TextStyle::default()
                })
                .into(),
            Text::new("Font fallback: Hello — नमस्ते — 日本語 — مرحبا")
                .style(TextStyle {
                    size: 20.0,
                    color: Color::rgba(160, 230, 190, 255),
                    ..TextStyle::default()
                })
                .into(),
            Text::new("Greek Ελληνικά · Cyrillic Привет · Hebrew שלום · Thai สวัสดี")
                .style(TextStyle {
                    size: 16.0,
                    color: Color::rgba(220, 210, 255, 255),
                    ..TextStyle::default()
                })
                .into(),
            DecoratedBox::new(
                Text::new("Dark text on a light rectangle").style(TextStyle {
                    size: 20.0,
                    color: Color::rgba(25, 30, 40, 255),
                    ..TextStyle::default()
                }),
            )
            .background(Color::rgba(240, 244, 250, 255))
            .into(),
        ])))
    })
    .expect("valid typography application");
    example_support::spawn_if_requested(app.simulation(), simulations::run);
    incular::run(app).expect("native typography application");
}
