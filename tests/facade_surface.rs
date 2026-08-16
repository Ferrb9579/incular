use incular::prelude::*;

#[test]
fn public_facade_composes_scroll_gesture_and_navigation_features() {
    let controller = ScrollController::new();
    let header = SliverAppBar::new(32., Widget::fixed_box(Size::new(120., 32.), Color::WHITE));
    assert!(header.is_pinned());

    let slivers: Vec<Box<dyn Sliver>> = vec![Box::new(header)];
    let view = CustomScrollView::build(controller, slivers);
    let gestures = GestureRegion::new(
        GestureCallbacks::default(),
        Widget::fixed_box(Size::new(120., 80.), Color::BLACK),
    );
    let _: Widget = Widget::column(vec![view, gestures.into()]);

    let registry = RouteRegistry::new();
    registry.register("settings/", || {
        Page::new(
            "settings",
            Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
        )
    });
    let navigator = Navigator::new();
    assert!(registry.navigate(&navigator, "/settings").is_some());
}

#[test]
fn facade_exposes_the_extracted_subsystems() {
    let constraints = incular::config::Constraints::tight(Size::new(40., 20.));
    assert_eq!(constraints.min_width, 40.);

    let mut canvas = incular::rendering::Canvas::default();
    canvas.rect(
        incular::core::Rect::from_origin_size(Offset::ZERO, Size::new(1., 1.)),
        Color::WHITE,
    );
    assert_eq!(canvas.finish().len(), 1);

    let image = incular::image::DecodedImage::from_rgba8(1, 1, vec![255, 255, 255, 255]).unwrap();
    assert_eq!(image.width(), 1);
}
