//! Color-matrix, blend-mode, and ordered effect-chain showcase.
use incular::prelude::*;
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};

fn gradient() -> LinearGradient {
    LinearGradient {
        start: Offset::ZERO,
        end: Offset::new(220., 0.),
        stops: GradientStops::new(vec![
            GradientStop {
                offset: 0.,
                color: Color::rgba(70, 125, 230, 230),
            },
            GradientStop {
                offset: 0.5,
                color: Color::rgba(220, 80, 170, 190),
            },
            GradientStop {
                offset: 1.,
                color: Color::rgba(55, 205, 180, 210),
            },
        ]),
    }
}

fn card(label: &str, brush: impl Into<Brush>) -> Widget {
    DecoratedBox::new(Text::new(label).color(Color::WHITE))
        .size(Size::new(170., 62.))
        .background(brush)
        .radius(10.)
        .into()
}

fn main() {
    let saturation = ColorFilterController::new(ColorFilter::saturate(0.));
    let animate_target = Rc::new(Cell::new(false));
    let overlap = TranslationController::new();
    overlap.set_offset(Offset::new(-92., 0.));
    let second_overlap = TranslationController::new();
    second_overlap.set_offset(Offset::new(-184., 0.));
    let image = ImageHandle::from_rgba8(
        2,
        2,
        [
            255, 220, 80, 220, 40, 60, 110, 170, 40, 60, 110, 120, 255, 100, 100, 220,
        ],
    )
    .expect("generated transparent image");

    let app = Application::new(move |_| {
        let saturation_for_button = saturation.clone();
        let animate_target_for_button = animate_target.clone();
        Widget::column(vec![
            Text::new("Incular Color Effects")
                .style(TextStyle {
                    size: 28.,
                    color: Color::rgba(255, 230, 165, 255),
                    ..TextStyle::default()
                })
                .into(),
            Text::new("Straight-RGBA color matrices over premultiplied retained textures")
                .color(Color::rgba(220, 225, 240, 255))
                .into(),
            Widget::row(vec![
                card("Original", Color::rgba(70, 125, 230, 255)),
                ColorFiltered::new(
                    ColorFilter::grayscale(1.),
                    card("Grayscale", Color::rgba(70, 190, 180, 255)),
                )
                .into(),
                ColorFiltered::new(
                    ColorFilter::sepia(1.),
                    card("Sepia", Color::rgba(230, 120, 80, 255)),
                )
                .into(),
            ]),
            Widget::row(vec![
                ColorFiltered::controlled(
                    saturation.clone(),
                    card("Animated saturation", Color::rgba(220, 90, 170, 255)),
                )
                .into(),
                ColorFiltered::new(
                    ColorFilter::contrast(1.8),
                    card("High contrast", Color::rgba(100, 170, 230, 255)),
                )
                .into(),
            ]),
            Text::new("Blend modes: overlapping transparent groups")
                .color(Color::rgba(220, 225, 240, 255))
                .into(),
            Widget::row(vec![
                card("Destination", Color::rgba(240, 80, 80, 230)),
                Widget::translate(
                    overlap.clone(),
                    Blend::new(
                        BlendMode::Multiply,
                        card("Multiply", Color::rgba(80, 120, 240, 190)),
                    )
                    .into(),
                ),
                Widget::translate(
                    second_overlap.clone(),
                    Blend::new(
                        BlendMode::Screen,
                        card("Screen", Color::rgba(80, 230, 180, 170)),
                    )
                    .into(),
                ),
            ]),
            Widget::row(vec![
                Image::new(image.clone()).width(70.).height(70.).into(),
                Blend::new(
                    BlendMode::Overlay,
                    card("Overlay", Color::rgba(250, 180, 70, 180)),
                )
                .into(),
                Blend::new(
                    BlendMode::Difference,
                    card("Difference", Color::rgba(90, 220, 240, 180)),
                )
                .into(),
            ]),
            Text::new("Effect ordering: blur → grayscale versus grayscale → blur")
                .color(Color::rgba(220, 225, 240, 255))
                .into(),
            Widget::row(vec![
                Effects::new(card("blur then grayscale", gradient()))
                    .blur(5.)
                    .color_filter(ColorFilter::grayscale(1.))
                    .build(),
                Effects::new(card("grayscale then blur", gradient()))
                    .color_filter(ColorFilter::grayscale(1.))
                    .blur(5.)
                    .build(),
            ]),
            Button::new("Animate saturation")
                .on_press(move || {
                    let target = if animate_target_for_button.get() {
                        0.
                    } else {
                        1.
                    };
                    animate_target_for_button.set(!animate_target_for_button.get());
                    saturation_for_button.animate_to(
                        ColorFilter::saturate(target),
                        Duration::from_millis(700),
                        Instant::now(),
                    );
                })
                .into(),
        ])
    })
    .expect("valid color effects application");
    incular::run(app).expect("native color effects application");
}
