use incular::prelude::*;
use incular_controls::Button;

fn main() {
    // State must live OUTSIDE the builder: builders re-run on every invalidation,
    // so a Signal created inside would be recreated (reset) each time.
    let pressed = Signal::new(false);
    let app = Application::new(move |_cx| {
        let display_pressed = pressed.get();
        // This diagnostic runs once per builder evaluation. Do not put
        // external side effects such as network requests or file writes here.
        println!("Build: pressed");
        Container::new()
            .background(Color::rgba(255, 0, 0, 255))
            .child(Widget::column(vec![
                Text::new("Incular Custom Application")
                    .color(Color::rgba(220, 230, 255, 255))
                    .into(),
                Text::new("This is a custom application example.").into(),
                Button::new("Press me")
                    .on_click({
                        let pressed = pressed.clone();
                        move || {
                            pressed.update(|pressed| *pressed = !*pressed);
                            println!("Click: pressed={}", pressed.get());
                        }
                    })
                    .into(),
                Text::new(display_pressed.to_string()).into(),
            ]))
            .into()
    })
    .expect("valid application");
    incular::run(app).expect("native custom application");
}
