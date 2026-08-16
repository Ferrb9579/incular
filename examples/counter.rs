//! A declarative native counter: no element IDs, action IDs, or runtime wiring.
use incular::prelude::*;

fn main() {
    let count = Signal::new(0_u32);
    let app_count = count.clone();
    let app = Application::new(move |_cx| {
        let value = app_count.get();
        let callback_count = app_count.clone();
        Widget::column(vec![
            Text::new("Incular Counter")
                .color(Color::rgba(220, 230, 255, 255))
                .into(),
            Text::new(format!("Count: {value}")).into(),
            Button::new("Increment")
                .on_press(move || callback_count.update(|count| *count += 1))
                .into(),
        ])
    })
    .expect("valid application");
    incular::run(app).expect("native counter application");
}
