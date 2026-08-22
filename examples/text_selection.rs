//! Drag across read-only labels, use Shift+Arrow to extend, and Ctrl/Cmd+C to copy.
use incular::prelude::*;

fn main() {
    let app = Application::new(|_| {
        SelectionArea::new(Widget::column(vec![
            Text::new("Read-only selection")
                .style(TextStyle {
                    size: 28.,
                    color: Color::rgba(220, 230, 255, 255),
                    ..TextStyle::default()
                })
                .into(),
            SelectableText::new("Drag from this Latin text…")
                .style(TextStyle {
                    size: 20.,
                    color: Color::rgba(180, 220, 255, 255),
                    ..TextStyle::default()
                })
                .into(),
            SelectableText::new("…through this mixed bidi line: עברית / English / 世界")
                .style(TextStyle {
                    size: 20.,
                    color: Color::rgba(255, 220, 180, 255),
                    ..TextStyle::default()
                })
                .into(),
            Text::new("Selection is read-only: there is no caret or IME session.").into(),
        ])
        .into()
    })
    .expect("valid selection application");
    incular::run(app).expect("native selection application");
}
