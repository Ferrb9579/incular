//! A text-containing card translated by retained compositor state.
use incular::material::RawMaterialButton;
use incular::prelude::*;
use incular::widgets::internal::TranslationController;
use std::time::{Duration, Instant};

#[path = "../tests/support/mod.rs"]
pub(crate) mod example_support;
#[path = "tests/simulations.rs"]
pub(crate) mod simulations;

fn main() {
    let translation = TranslationController::new();
    let moving = translation.clone();
    let app = Application::new(move |_| {
        let trigger = moving.clone();
        Widget::column(vec![
            RawMaterialButton::new("Move")
                .on_press(move || {
                    trigger.animate_to(
                        Offset::new(180., 0.),
                        Duration::from_millis(900),
                        Instant::now(),
                    )
                })
                .into(),
            Widget::translate(
                moving.clone(),
                Widget::column(vec![
                    Widget::box_(Size::new(180., 70.), Color::rgba(130, 70, 200, 255)),
                    Text::new("Cached text moves with the card")
                        .color(Color::WHITE)
                        .into(),
                ]),
            ),
        ])
    })
    .expect("valid animation application");
    example_support::spawn_if_requested(app.simulation(), simulations::run);
    incular::run(app).expect("native animation application");
}
