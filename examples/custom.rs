use incular::prelude::*;

fn main() {
    let app = Application::new(move |_cx| {
        let pressed = Signal::new(false);
        let c = pressed.clone();
        Widget::column(vec![
            Text::new("Incular Custom Application")
                .color(Color::rgba(220, 230, 255, 255))
                .into(),
            Text::new("This is a custom application example.").into(),
            Button::new("Press me")
                .on_press(move || pressed.update(|pressed| *pressed = !*pressed))
                .into(),
            Text::new(c.get().to_string()).into(),
        ])
    })
    .expect("valid application");
    incular::run(app).expect("native custom application");
}
