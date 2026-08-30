//! A hands-on retained gesture surface.
//!
//! Tap the panel, drag with one pointer, or use a touchscreen to pinch with
//! two contacts. The status labels and the translated card are rebuilt from
//! the callbacks owned by `GestureDetector`.
use incular::material::RawMaterialButton;
use incular::prelude::*;
use incular::widgets::internal::{GestureCallbacks, TranslationController};
use std::rc::Rc;

fn label(value: impl Into<String>) -> Widget {
    Text::new(value)
        .color(Color::rgba(224, 232, 250, 255))
        .into()
}

#[path = "../tests/support/mod.rs"]
pub(crate) mod example_support;
#[path = "tests/simulations.rs"]
pub(crate) mod simulations;

fn main() {
    let taps = Signal::new(0_u32);
    let pan = Signal::new(Offset::ZERO);
    let scale = Signal::new(1_f32);
    let translation = TranslationController::new();

    let app_taps = taps.clone();
    let app_pan = pan.clone();
    let app_scale = scale.clone();
    let app_translation = translation.clone();
    let app = Application::new(move |_| {
        let tap_value = app_taps.get();
        let pan_value = app_pan.get();
        let scale_value = app_scale.get();

        let tap_target = app_taps.clone();
        let pan_target = app_pan.clone();
        let scale_target = app_scale.clone();
        let move_target = app_translation.clone();
        let callbacks = GestureCallbacks {
            on_tap: Some(Rc::new(move || tap_target.update(|value| *value += 1))),
            on_pan_update: Some(Rc::new(move |delta| {
                pan_target.set(delta);
                move_target.set_offset(delta);
            })),
            on_scale_update: Some(Rc::new(move |details| {
                scale_target.set(details.scale);
            })),
            ..GestureCallbacks::default()
        };

        let reset_pan = app_pan.clone();
        let reset_scale = app_scale.clone();
        let reset_translation = app_translation.clone();
        Widget::column(vec![
            Text::new("Gesture gallery")
                .style(TextStyle {
                    size: 30.,
                    color: Color::rgba(255, 224, 145, 255),
                    ..TextStyle::default()
                })
                .into(),
            label("Tap the panel; drag one pointer; pinch on a touchscreen with two contacts."),
            label(format!(
                "taps: {tap_value}  pan: ({:.0}, {:.0})  scale: {scale_value:.2}×",
                pan_value.x, pan_value.y
            )),
            GestureDetector::new(
                DecoratedBox::new(Widget::stack(
                    Alignment::CENTER,
                    vec![
                        Widget::box_(Size::new(460., 280.), Color::rgba(27, 42, 72, 255)),
                        Widget::translate(
                            app_translation.clone(),
                            DecoratedBox::new(Text::new("gesture target").style(TextStyle {
                                size: 24.,
                                color: Color::WHITE,
                                ..TextStyle::default()
                            }))
                            .size(Size::new(
                                180. * scale_value.clamp(0.5, 2.),
                                80. * scale_value.clamp(0.5, 2.),
                            ))
                            .background(Color::rgba(72, 136, 235, 255))
                            .radius(16.)
                            .into(),
                        ),
                    ],
                ))
                .size(Size::new(460., 280.))
                .background(Color::rgba(27, 42, 72, 255))
                .radius(18.),
            )
            .callbacks(callbacks)
            .into(),
            RawMaterialButton::new("Reset interaction")
                .on_press(move || {
                    reset_pan.set(Offset::ZERO);
                    reset_scale.set(1.);
                    reset_translation.set_offset(Offset::ZERO);
                })
                .into(),
        ])
    })
    .expect("valid gesture gallery application");

    example_support::spawn_if_requested(app.simulation(), simulations::run);
    incular::run(app).expect("native gesture gallery application");
}
