//! Retained Gaussian blur and arbitrary-subtree drop-shadow showcase.
use incular::prelude::*;
use std::{
    rc::Rc,
    time::{Duration, Instant},
};

fn gradient() -> LinearGradient {
    LinearGradient {
        start: Offset::ZERO,
        end: Offset::new(260., 0.),
        stops: GradientStops::new(vec![
            GradientStop {
                offset: 0.,
                color: Color::rgba(64, 126, 235, 255),
            },
            GradientStop {
                offset: 0.5,
                color: Color::rgba(205, 92, 210, 255),
            },
            GradientStop {
                offset: 1.,
                color: Color::rgba(56, 202, 174, 255),
            },
        ]),
    }
}

fn main() {
    let blur_controller = BlurController::new(3.);
    let shadow_controller =
        DropShadowController::new(Offset::new(0., 10.), 10., Color::rgba(0, 0, 0, 150));
    let blur_target = Rc::new(std::cell::Cell::new(false));
    let shadow_target = Rc::new(std::cell::Cell::new(false));
    let image = ImageHandle::from_rgba8(
        2,
        2,
        [
            255, 220, 80, 255, 40, 60, 110, 180, 40, 60, 110, 180, 255, 100, 100, 255,
        ],
    )
    .expect("generated image");

    let app = Application::new(move |_| {
        let blur_for_button = blur_controller.clone();
        let shadow_for_button = shadow_controller.clone();
        let blur_target_button = blur_target.clone();
        let shadow_target_button = shadow_target.clone();
        Widget::column(vec![
            Text::new("Incular Effects")
                .style(TextStyle {
                    size: 28.,
                    color: Color::rgba(255, 230, 165, 255),
                    ..TextStyle::default()
                })
                .into(),
            Text::new("Gaussian blur")
                .color(Color::rgba(220, 225, 240, 255))
                .into(),
            Widget::row(vec![
                DropShadow::controlled(
                    shadow_controller.clone(),
                    DecoratedBox::new(Text::new("sharp card").color(Color::WHITE))
                        .size(Size::new(180., 74.))
                        .background(gradient())
                        .radius(12.),
                )
                .into(),
                Blur::controlled(
                    blur_controller.clone(),
                    DecoratedBox::new(Text::new("blurred card").color(Color::WHITE))
                        .size(Size::new(180., 74.))
                        .background(gradient())
                        .radius(12.),
                )
                .into(),
            ]),
            Text::new("Blurred image and mixed subtree")
                .color(Color::rgba(220, 225, 240, 255))
                .into(),
            Blur::new(
                7.,
                Widget::row(vec![
                    Image::new(image.clone()).width(70.).height(70.).into(),
                    Icon::new(icons::plus())
                        .size(64.)
                        .brush(Color::rgba(255, 235, 130, 255))
                        .into(),
                    Text::new("text + image + path").color(Color::WHITE).into(),
                ]),
            )
            .into(),
            Text::new("Large-radius blur")
                .color(Color::rgba(220, 225, 240, 255))
                .into(),
            Blur::new(
                48.,
                DecoratedBox::new(Widget::box_(
                    Size::new(320., 70.),
                    Color::rgba(240, 90, 130, 230),
                ))
                .size(Size::new(320., 70.))
                .background(gradient())
                .radius(14.),
            )
            .into(),
            Widget::row(vec![
                Button::new("Animate blur")
                    .on_press(move || {
                        let target = if blur_target_button.get() { 2. } else { 18. };
                        blur_target_button.set(!blur_target_button.get());
                        blur_for_button.animate_to(
                            target,
                            Duration::from_millis(700),
                            Instant::now(),
                        );
                    })
                    .into(),
                Button::new("Move shadow")
                    .on_press(move || {
                        let target = if shadow_target_button.get() { 4. } else { 22. };
                        shadow_target_button.set(!shadow_target_button.get());
                        shadow_for_button.animate_offset_to(
                            Offset::new(0., target),
                            Duration::from_millis(700),
                            Instant::now(),
                        );
                    })
                    .into(),
            ]),
        ])
    })
    .expect("valid effects application");
    incular::run(app).expect("native effects application");
}
