//! Visual smoke test for Phase 9.1B vector painting and retained gradients.
use incular::prelude::*;
use incular::widgets::internal::{PathView, icons};
use std::sync::Arc;

fn stops(colors: &[(f32, Color)]) -> GradientStops {
    GradientStops::new(
        colors
            .iter()
            .map(|&(offset, color)| GradientStop { offset, color })
            .collect(),
    )
}
fn star() -> Arc<Path> {
    let mut p = Path::builder();
    for (index, point) in [
        (50., 0.),
        (61., 35.),
        (98., 35.),
        (68., 56.),
        (79., 92.),
        (50., 70.),
        (21., 92.),
        (32., 56.),
        (2., 35.),
        (39., 35.),
    ]
    .into_iter()
    .enumerate()
    {
        if index == 0 {
            p.move_to(Offset::new(point.0, point.1));
        } else {
            p.line_to(Offset::new(point.0, point.1));
        }
    }
    p.close();
    Arc::new(p.build())
}
#[path = "../tests/support/mod.rs"]
pub(crate) mod example_support;
#[path = "tests/simulations.rs"]
pub(crate) mod simulations;

fn main() {
    let rainbow = LinearGradient {
        start: Offset::ZERO,
        end: Offset::new(300., 0.),
        stops: stops(&[
            (0., Color::rgba(240, 50, 50, 255)),
            (0.25, Color::rgba(250, 220, 45, 255)),
            (0.5, Color::rgba(50, 205, 90, 255)),
            (0.75, Color::rgba(40, 205, 220, 255)),
            (1., Color::rgba(55, 100, 245, 255)),
        ]),
    };
    let radial = RadialGradient {
        center: Offset::new(70., 70.),
        radius: 70.,
        stops: stops(&[
            (0., Color::WHITE),
            (0.45, Color::rgba(240, 65, 190, 255)),
            (1., Color::rgba(40, 65, 180, 255)),
        ]),
    };
    let star = star();
    let app = Application::new(move |_| {
        Widget::column(vec![
            Text::new("Incular Painting")
                .style(TextStyle {
                    size: 30.,
                    color: Color::rgba(255, 230, 165, 255),
                    ..TextStyle::default()
                })
                .into(),
            Text::new("Multi-stop linear gradient: red → yellow → green → cyan → blue").into(),
            DecoratedBox::new(Widget::box_(Size::new(300., 34.), Color::TRANSPARENT))
                .background(rainbow.clone())
                .radius(8.)
                .into(),
            Widget::row(vec![
                DecoratedBox::new(Widget::box_(Size::new(140., 140.), Color::TRANSPARENT))
                    .background(radial.clone())
                    .radius(24.)
                    .border(Border::new(2., Color::WHITE))
                    .into(),
                PathView::new(star.clone())
                    .fill(Color::rgba(255, 195, 40, 255))
                    .stroke(
                        Color::WHITE,
                        Stroke {
                            width: 3.,
                            ..Stroke::default()
                        },
                    )
                    .size(Size::new(100., 100.))
                    .into(),
                PathView::new(star.clone())
                    .fill(rainbow.clone())
                    .size(Size::new(100., 100.))
                    .into(),
            ]),
            Text::new("Shared cached vector icons").into(),
            Widget::row(vec![
                Icon::new(icons::check())
                    .size(36.)
                    .brush(Color::rgba(85, 225, 150, 255))
                    .into(),
                Icon::new(icons::close())
                    .size(36.)
                    .brush(Color::rgba(245, 115, 120, 255))
                    .into(),
                Icon::new(icons::plus())
                    .size(36.)
                    .brush(rainbow.clone())
                    .into(),
                Icon::new(icons::chevron_right())
                    .size(36.)
                    .brush(Color::WHITE)
                    .into(),
                Icon::new(icons::check())
                    .size(36.)
                    .brush(Color::rgba(85, 225, 150, 255))
                    .into(),
            ]),
        ])
    })
    .expect("valid painting application");
    example_support::spawn_if_requested(app.simulation(), simulations::run);
    incular::run(app).expect("native painting application");
}
