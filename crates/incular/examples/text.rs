//! Text shaping, logical wrapping, colors, and mixed Unicode on a HiDPI window.
use incular::prelude::*;

fn main() {
    let app = Application::new(|_| {
        Widget::column(vec![
            Text::new("Incular Typography")
                .style(TextStyle {
                    size: 28.0,
                    color: Color::rgba(255, 210, 120, 255),
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
        ])
    })
    .expect("valid typography application");
    incular::run(app).expect("native typography application");
}
