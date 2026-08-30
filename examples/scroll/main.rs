//! Retained vertical scrolling: wheel input changes only the viewport transform.
use incular::material::RawMaterialButton;
use incular::prelude::*;
use incular::widgets::internal::ScrollView;

#[path = "../support/mod.rs"]
mod example_support;
#[cfg(test)]
#[path = "tests.rs"]
mod example_tests;
mod simulations;

fn main() {
    let controller = ScrollController::new();
    let items = (0..100)
        .map(|index| {
            RawMaterialButton::new(format!("Item {index}"))
                .color(Color::rgba(55, 90 + (index % 4) as u8 * 25, 155, 255))
                .on_press(move || eprintln!("clicked Item {index}"))
                .into()
        })
        .collect::<Vec<Widget>>();
    let app = Application::new(move |_| {
        ScrollView::vertical(controller.clone(), Widget::column(items.clone()))
    })
    .expect("valid scrolling application");
    example_support::spawn_if_requested(app.simulation(), simulations::run);
    incular::run(app).expect("native scroll application");
}
