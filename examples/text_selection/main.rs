//! Drag across read-only labels, use Shift+Arrow to extend, and Ctrl/Cmd+C to copy.
use incular::material_prelude::{SelectableText, SelectionArea};
use incular::prelude::*;

#[path = "../tests/support/mod.rs"]
pub(crate) mod example_support;
#[path = "tests/simulations.rs"]
pub(crate) mod simulations;

fn main() {
    let app = Application::new(|_| {
        SelectionArea::new(Widget::from(Column::new(Vec::<Widget>::from([
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
        ]))))
        .into()
    })
    .expect("valid selection application");
    example_support::spawn_if_requested(app.simulation(), simulations::run);
    incular::run(app).expect("native selection application");
}
