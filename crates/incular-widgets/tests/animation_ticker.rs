use incular_animation::AnimationController;
use incular_config::Constraints;
use incular_core::Size;
use incular_widgets::{SizedBox, Widget, internal::WidgetTree};
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};

#[test]
fn animation_ticker_advances_controller_and_local_revision() {
    let controller = AnimationController::new(Duration::from_millis(100));
    let revision = Rc::new(Cell::new(0_u64));
    let widget = Widget::animation_ticker_with_revision(
        controller.clone(),
        true,
        revision.clone(),
        SizedBox::new().width(10.0).height(10.0),
    );
    let mut tree = WidgetTree::new();
    tree.mount(widget).expect("mount ticker");
    tree.layout(Constraints::loose(Size::new(40.0, 40.0)))
        .expect("layout ticker");
    let origin = Instant::now();
    tree.update_compositor(origin).expect("start ticker");
    assert_eq!(revision.get(), 0);
    tree.update_compositor(origin + Duration::from_millis(50))
        .expect("tick ticker");
    assert_eq!(revision.get(), 1);
    assert!((controller.value() - 0.5).abs() < 0.001);
}
