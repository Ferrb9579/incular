//! Retained offscreen group-opacity showcase.
use incular::material::RawMaterialButton;
use incular::prelude::*;
use incular::widgets::internal::icons;
use incular::widgets::internal::{OpacityController, TranslationController};
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};

fn stops(colors: &[(f32, Color)]) -> GradientStops {
    GradientStops::new(
        colors
            .iter()
            .map(|&(offset, color)| GradientStop { offset, color })
            .collect(),
    )
}

#[path = "../support/mod.rs"]
mod example_support;
#[cfg(test)]
#[path = "tests.rs"]
mod example_tests;
mod simulations;

fn main() {
    let overlap_translation = TranslationController::new();
    overlap_translation.set_offset(Offset::new(-80., 0.));

    let fade = OpacityController::new();
    let fade_button_controller = fade.clone();
    let fade_target = Rc::new(Cell::new(false));
    let fade_target_button = fade_target.clone();

    let rainbow = LinearGradient {
        start: Offset::ZERO,
        end: Offset::new(330., 0.),
        stops: stops(&[
            (0., Color::rgba(70, 125, 230, 255)),
            (0.5, Color::rgba(180, 90, 220, 255)),
            (1., Color::rgba(45, 195, 180, 255)),
        ]),
    };
    let image = ImageHandle::from_rgba8(
        2,
        2,
        [
            255, 220, 80, 255, 40, 60, 110, 180, 40, 60, 110, 180, 255, 100, 100, 255,
        ],
    )
    .expect("valid generated image");

    let app = Application::new(move |_| {
        let fade_controller = fade.clone();
        let fade_button_controller = fade_button_controller.clone();
        let fade_target_button = fade_target_button.clone();
        Widget::column(vec![
            Text::new("Incular Group Opacity")
                .style(TextStyle {
                    size: 28.,
                    color: Color::rgba(255, 230, 165, 255),
                    ..TextStyle::default()
                })
                .into(),
            Text::new("Correct isolated opacity: overlapping children are composited once")
                .color(Color::rgba(220, 225, 240, 255))
                .into(),
            Opacity::new(
                0.5,
                Widget::row(vec![
                    Widget::box_(Size::new(170., 90.), Color::rgba(235, 65, 70, 255)),
                    Widget::translate(
                        overlap_translation.clone(),
                        Widget::box_(Size::new(170., 90.), Color::rgba(55, 100, 235, 255)),
                    ),
                ]),
            )
            .into(),
            Text::new("Nested opacity: outer 0.6 × inner 0.5")
                .color(Color::rgba(220, 225, 240, 255))
                .into(),
            Opacity::new(
                0.6,
                Widget::column(vec![
                    DecoratedBox::new(Widget::box_(
                        Size::new(330., 42.),
                        Color::rgba(240, 170, 60, 255),
                    ))
                    .radius(8.)
                    .into(),
                    Opacity::new(
                        0.5,
                        DecoratedBox::new(Widget::box_(
                            Size::new(240., 42.),
                            Color::rgba(80, 210, 180, 255),
                        ))
                        .radius(8.),
                    )
                    .into(),
                ]),
            )
            .into(),
            Text::new("Cached content fade: text, image, gradient, and path")
                .color(Color::rgba(220, 225, 240, 255))
                .into(),
            Opacity::controlled(
                fade_controller,
                DecoratedBox::new(Widget::row(vec![
                    Image::new(image.clone())
                        .width(56.)
                        .height(56.)
                        .sampling(ImageSampling::Nearest)
                        .into(),
                    Text::new("Cached group")
                        .style(TextStyle {
                            size: 20.,
                            color: Color::WHITE,
                            ..TextStyle::default()
                        })
                        .into(),
                    Icon::new(icons::plus())
                        .size(42.)
                        .brush(Color::rgba(255, 235, 130, 255))
                        .into(),
                ]))
                .size(Size::new(330., 70.))
                .background(rainbow.clone())
                .radius(12.),
            )
            .into(),
            RawMaterialButton::new("Fade")
                .on_press(move || {
                    let target = if fade_target_button.get() { 1. } else { 0.1 };
                    fade_target_button.set(!fade_target_button.get());
                    fade_button_controller.animate_to(
                        target,
                        Duration::from_millis(800),
                        Instant::now(),
                    );
                })
                .into(),
        ])
    })
    .expect("valid opacity application");
    example_support::spawn_if_requested(app.simulation(), simulations::run);
    incular::run(app).expect("native opacity application");
}
