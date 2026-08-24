use incular::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn test_semantics_builder_contract() {
    let tapped = Rc::new(Cell::new(false));
    let tapped_clone = tapped.clone();

    let semantics_widget: Widget = Semantics::new(Text::new("Checkout"))
        .label("Checkout button")
        .hint("Proceeds to the payment screen")
        .button(true)
        .enabled(true)
        .selected(false)
        .on_tap(move || tapped_clone.set(true))
        .into();

    let _ = semantics_widget;
}
