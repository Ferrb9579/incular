use incular::prelude::*;

fn main() {
    // State must live OUTSIDE the builder: builders re-run on every invalidation,
    // so a Signal created inside would be recreated (reset) each time.
    let pressed = Signal::new(false);
    let app = Application::new(move |_cx| {
        let c = pressed.clone();
        let display = c.get();
        println!("Build: pressed={display}");
        Widget::column(vec![
            Text::new("Incular Custom Application")
                .color(Color::rgba(220, 230, 255, 255))
                .into(),
            Text::new("This is a custom application example.").into(),
            Button::new("Press me")
                .on_press({
                    let pressed = pressed.clone();
                    move || {
                        pressed.update(|pressed| *pressed = !*pressed);
                    }
                })
                .into(),
            Text::new(display.to_string()).into(),
        ])
    })
    .expect("valid application");
    incular::run(app).expect("native custom application");
}
