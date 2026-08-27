use std::sync::Arc;

use incular::prelude::*;

#[test]
fn public_facade_composes_scroll_gesture_and_navigation_features() {
    let controller = ScrollController::new();
    let view = SingleChildScrollView::new(Widget::fixed_box(Size::new(120., 32.), Color::WHITE))
        .controller(controller);
    let gestures = GestureDetector::new(Widget::fixed_box(Size::new(120., 80.), Color::BLACK));
    let _: Widget = Column::new([Widget::from(view), Widget::from(gestures)]).into();

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
fn facade_exposes_material_components_at_the_material_boundary() {
    use incular::material_prelude::{
        ElevatedButton, OutlinedButton, SelectableText, SelectionArea, TextButton, TextField,
    };
    use incular::widgets::internal::TextEditingController;
    let controller = TextEditingController::with_text("Ada");
    let _: Widget = ElevatedButton::new("Save").into();
    let _: Widget = OutlinedButton::new("Cancel").into();
    let _: Widget = TextButton::new("Later").into();
    let _: Widget = TextField::new(controller).max_lines(None).into();
    let _: Widget = SelectableText::new("copy me").into();
    let _: Widget = SelectionArea::new(Text::new("select me")).into();
}

#[test]
fn facade_exposes_variable_extent_lazy_lists() {
    let index = incular::scroll::MeasuredExtentIndex::new(10, 32.);
    let controller = ScrollController::new();
    let _: Widget = ListView::variable_extent(10, 32., |_| {
        Widget::fixed_box(Size::new(20., 32.), Color::WHITE)
    })
    .controller(controller)
    .into();
    assert_eq!(index.index_at_offset(96.), Some(3));
}

#[test]
fn facade_exposes_hsl_and_hsv_configuration_colors() {
    let hsl = HslColor::new(240.0, 1.0, 0.5, 1.0);
    assert_eq!(hsl.to_color(), Color::rgba(0, 0, 255, 255));

    let hsv = HsvColor::from_color(Color::rgba(255, 128, 0, 128));
    assert_eq!(hsv.to_color(), Color::rgba(255, 128, 0, 128));
}

#[test]
fn facade_exposes_icu_localization_catalog_boundary() {
    struct Catalog {
        locales: Vec<Locale>,
    }
    impl LocalizationCatalog for Catalog {
        fn supported_locales(&self) -> &[Locale] {
            &self.locales
        }

        fn message(&self, locale: &Locale, key: &str) -> Option<&str> {
            (locale.to_string() == "en" && key == "greeting").then_some("Hello")
        }
    }

    let catalog = Catalog {
        locales: vec!["en".parse().expect("valid ICU locale")],
    };
    let message = LocaleResolver::resolve_message(
        &["en-GB".parse().expect("valid ICU locale")],
        &catalog,
        "greeting",
    )
    .expect("parent locale resolves");
    assert_eq!(message.value, "Hello");
    assert_eq!(
        LocaleResolver::text_direction(&message.locale),
        TextDirection::Ltr
    );
}

#[test]
fn facade_exposes_colored_box_and_mouse_region() {
    let child = Widget::fixed_box(Size::new(8., 8.), Color::WHITE);
    let _: Widget = ColoredBox::new(Color::BLACK, child.clone()).into();
    let _: Widget = MouseRegion::new(child).into();
}

#[test]
fn facade_exposes_retained_affine_widgets() {
    use incular::widgets::internal::ScaleController;
    let child = Widget::fixed_box(Size::new(8., 8.), Color::WHITE);
    let _: Widget = Transform::rotation(0.25, child.clone()).into();
    let _: Widget = FittedBox::new(child.clone()).fit(BoxFit::Contain).into();
    let _: Widget = ScaleTransition::new(ScaleController::new(), child.clone()).into();
    let affine = AffineTransform::skew(0.1, 0.2);
    assert!(affine.inverse().is_some());
}

#[test]
fn facade_exposes_retained_layout_closure_widgets() {
    let child = Widget::fixed_box(Size::new(8., 8.), Color::WHITE);
    let _: Widget = LimitedBox::new(child.clone())
        .max_width(20.)
        .max_height(20.)
        .into();
    let _: Widget = OverflowBox::new(child.clone()).max_width(40.).into();
    let _: Widget = Flexible::new(child.clone()).flex(1).into();
    let _: Widget = Expanded::new(child.clone()).into();
    let _: Widget = Spacer::new().into();
    let _: Widget = Positioned::new(child.clone()).left(2.).top(3.).into();
    let _: Widget = IndexedStack::new([child.clone()]).index(0).into();
    let _: Widget = LayoutBuilder::new(move |_| child.clone()).into();
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

#[test]
fn facade_exposes_nested_navigation_contracts() {
    let root_navigation = Navigator::new();
    root_navigation.push_page(Page::new(
        "home",
        Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
    ));
    root_navigation.push_page(Page::new(
        "settings",
        Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
    ));
    let child_navigation = Navigator::new();
    child_navigation.push_page(Page::new(
        "overview",
        Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
    ));
    child_navigation.push_page(Page::new(
        "details",
        Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
    ));
    let root = BackDispatcher::new(root_navigation);
    let child = BackDispatcher::new(child_navigation);
    root.attach_child(&child);
    root.set_active_child(Some(&child));
    assert_eq!(root.dispatch_back().depth, 1);
    let observer = root.navigator().observe(|event| {
        let _: NavigationEvent = event;
    });
    assert!(observer.is_active());
    root.navigator().set_pop_guard(|_| PopDecision::Allow);
}
