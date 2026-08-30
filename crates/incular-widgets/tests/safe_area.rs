//! Safe-area descriptor behavior tests.

use incular_config::EdgeInsets;
use incular_core::{Color, Size};
use incular_widgets::{ColoredBox, SafeArea, SizedBox};

#[test]
fn descriptor_resolves_without_platform_dependencies() {
    let safe = SafeArea::new(
        SizedBox::from_size(Size::new(1., 1.))
            .child(ColoredBox::new(Color::WHITE, SizedBox::shrink())),
    )
    .minimum(EdgeInsets::all(3.))
    .sides(true, false, true, false);
    let _widget = safe.resolve(EdgeInsets::only(8., 9., 2., 1.));
}
