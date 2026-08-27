//! A declarative native counter: no element IDs, action IDs, or runtime wiring.
use incular::material::ElevatedButton;
use incular::prelude::*;
use incular_controls::ButtonStyle;

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
            ElevatedButton::new("Increment")
                .style(ButtonStyle::new())
                .on_click(move || callback_count.update(|count| *count += 1))
                .into(),
        ])
    })
    .expect("valid application");
    incular::run(app).expect("native counter application");
}
