//! Native editing: type `abc`, press Backspace, and watch it become `ab`.
use incular::prelude::*;

fn main() {
    let name = TextEditingController::with_text("Ada");
    let note = TextEditingController::with_text("First line\nSecond line\n");
    let app = Application::new(move |_cx| {
        Widget::column(vec![
            Text::new("Incular editable text")
                .color(Color::rgba(220, 230, 255, 255))
                .into(),
            TextField::new(name.clone())
                .placeholder("Name")
                .size(Size::new(320., 42.))
                .into(),
            Text::new("Multiline (Enter and Shift+Enter add a line)").into(),
            TextArea::new(note.clone())
                .placeholder("Type Unicode or use an IME")
                .size(Size::new(320., 180.))
                .into(),
        ])
    })
    .expect("valid text field application");
    incular::linux::run_application(app).expect("native text field application");
}
