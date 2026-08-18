use std::sync::Arc;

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

#[test]
fn facade_exposes_hsl_and_hsv_configuration_colors() {
    let hsl = HslColor::new(240.0, 1.0, 0.5, 1.0);
    assert_eq!(hsl.to_color(), Color::rgba(0, 0, 255, 255));

    let hsv = HsvColor::from_color(Color::rgba(255, 128, 0, 128));
    assert_eq!(hsv.to_color(), Color::rgba(255, 128, 0, 128));
}

#[test]
fn facade_exposes_colored_box_and_mouse_region() {
    let _: Widget = ColoredBox::new(Size::new(8., 8.), Color::BLACK).into();
    let mouse = MouseRegion::new();
    mouse.enter(Offset::new(2., 3.));
    assert!(mouse.is_inside());
    mouse.exit(Offset::new(2., 3.));
    assert!(!mouse.is_inside());
}

#[test]
fn facade_exposes_opt_in_restoration_contracts() {
    let key = RestorationKey::new("counter").expect("stable restoration key");
    assert_eq!(key.as_str(), "counter");

    let store = Arc::new(InMemoryRestorationStore::new());
    let config = RestorationConfig::new("dev.incular.facade-test", 3, store)
        .with_snapshot_limit(16 * 1024)
        .with_debounce(std::time::Duration::from_millis(1));
    assert_eq!(config.application_id(), "dev.incular.facade-test");
    assert_eq!(config.application_schema_version(), 3);

    let route_id = RestorableRouteId::new("/editor").expect("stable route identity");
    assert_eq!(route_id.as_str(), "/editor");
    let window_id = WindowRestorationId::new("inspector").expect("stable window identity");
    assert_eq!(window_id.as_key().as_str(), "inspector");
    let snapshot = NavigatorSnapshot::default();
    assert!(snapshot.routes.is_empty());

    fn receives_restorable_value(_: Option<Restorable<u32>>) {}
    fn receives_restoration_handle(_: Option<RestorationHandle>) {}
    receives_restorable_value(None);
    receives_restoration_handle(None);
}
