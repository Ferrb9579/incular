//! A declarative native counter: no element IDs, action IDs, or runtime wiring.
#[cfg(all(not(feature = "material"), feature = "controls"))]
use incular::controls_prelude::PrimaryButton;
#[cfg(feature = "material")]
use incular::material_prelude::RawMaterialButton;
use incular::prelude::*;

#[cfg(feature = "material")]
fn compact_increment_button(on_click: impl Fn() + 'static) -> Widget {
    // RawMaterialButton is only the hit/semantic surface. The visible child is
    // an explicit, compact Container with no border or focus-color layer.
    RawMaterialButton::with_child(
        Container::builder()
            .padding(EdgeInsets::symmetric(10.0, 5.0))
            .color(Color::rgba(60, 110, 220, 255))
            .child(
                Text::new("Increment").style(TextStyle::new().font_size(13.0).color(Color::WHITE)),
            )
            .build(),
    )
    .on_click(on_click)
    .into()
}

#[cfg(all(not(feature = "material"), feature = "controls"))]
fn compact_increment_button(on_click: impl Fn() + 'static) -> Widget {
    PrimaryButton::builder()
        .label("Increment")
        .on_click(on_click)
        .build()
        .into()
}

#[cfg(not(any(feature = "material", feature = "controls")))]
fn compact_increment_button(_on_click: impl Fn() + 'static) -> Widget {
    Text::new("Increment").into()
}

fn main() {
    let count = Signal::new(0_u32);
    let app_count = count.clone();
    let app = Application::new(move |_cx| {
        let value = app_count.get();
        let callback_count = app_count.clone();
        let increment = compact_increment_button(move || {
            callback_count.update(|count| *count += 1);
        });

        // Both the Column and Container builders accept generic Widget
        // children, while the same components retain their fluent APIs.
        Container::builder()
            .child(
                Column::builder()
                    .children(vec![
                        Text::new("Incular Counter")
                            .style(TextStyle::new().font_size(14.0).color(Color::WHITE))
                            .into(),
                        Text::new(format!("Count: {value}"))
                            .style(TextStyle::new().font_size(13.0))
                            .into(),
                        increment,
                    ])
                    .main_axis_size(MainAxisSize::Min)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .spacing(8.0)
                    .build(),
            )
            .width(200.0)
            .height(120.0)
            .padding(EdgeInsets::all(16.0))
            .alignment(Alignment::CENTER)
            .build()
            .into()
    })
    .expect("valid application");
    incular::run(app).expect("native counter application");
}
