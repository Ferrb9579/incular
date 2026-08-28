//! A declarative native counter: no element IDs, action IDs, or runtime wiring.
use incular::prelude::*;
use incular_controls::PrimaryButton;

fn main() {
    let count = Signal::new(0_u32);
    let app_count = count.clone();
    let app = Application::new(move |_cx| {
        let value = app_count.get();
        let callback_count = app_count.clone();
        Container::with_child(Widget::column(vec![
            Text::new("Incular Counter")
                .color(Color::rgba(220, 230, 255, 255))
                .into(),
            Text::new(format!("Count: {value}")).into(),
            PrimaryButton::new("Increment")
                .on_click(move || callback_count.update(|count| *count += 1))
                .into(),
        ]))
        .width(200.)
        .height(150.)
        .alignment(Alignment::CENTER)
        .into()
    })
    .expect("valid application");
    incular::run(app).expect("native counter application");
}
