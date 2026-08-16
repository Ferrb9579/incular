//! A million logical rows, with only the viewport and a 240 logical-pixel
//! cache materialized at once. Drag the visible right-hand thumb or click its
//! track; no row strings or Widgets are preallocated.
use incular::prelude::*;

fn main() {
    let controller = ScrollController::new();
    #[cfg(debug_assertions)]
    eprintln!("virtual list: logical items=1,000,000, extent=40, cache=240 logical px");
    let app = Application::new(move |_| {
        VirtualList::fixed_extent_with_controller(1_000_000, 40., controller.clone(), |index| {
            Button::new(format!("Item {index}"))
                .color(Color::rgba(55, 90 + (index % 4) as u8 * 25, 155, 255))
                .on_press(move || eprintln!("clicked Item {index}"))
        })
    })
    .expect("valid virtual-list application");
    incular::run(app).expect("native virtual-list application");
}
