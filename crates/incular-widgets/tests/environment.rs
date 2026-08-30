//! Ambient configuration and localization behavior tests.

use incular_config::{Locale, LocalizationCatalog, RuntimeEnvironment, TextDirection};
use incular_widgets::{Localizations, MediaQuery, MediaQueryData, Widget};

struct Catalog;

impl LocalizationCatalog for Catalog {
    fn supported_locales(&self) -> &[Locale] {
        static LOCALES: std::sync::OnceLock<Vec<Locale>> = std::sync::OnceLock::new();
        LOCALES
            .get_or_init(|| vec!["en".parse().expect("valid locale")])
            .as_slice()
    }

    fn message(&self, locale: &Locale, key: &str) -> Option<&str> {
        (locale.to_string() == "en" && key == "greeting").then_some("Hello")
    }
}

#[test]
fn localizations_use_icu_fallback_and_direction() {
    let localizations = Localizations::new(
        "en-GB".parse().expect("valid locale"),
        Catalog,
        Widget::column(Vec::<Widget>::new()),
    );
    assert_eq!(localizations.text("greeting").as_deref(), Some("Hello"));
    assert_eq!(localizations.text_direction(), TextDirection::Ltr);
    assert_eq!(
        localizations
            .resolve("greeting")
            .unwrap()
            .locale
            .to_string(),
        "en"
    );
}

#[test]
fn media_query_data_is_the_normalized_runtime_environment() {
    let data = MediaQueryData {
        viewport: incular_core::Size::new(640., 480.),
        scale_factor: f64::NAN,
        brightness: incular_config::Brightness::Dark,
        ..RuntimeEnvironment::default()
    };
    let query = MediaQuery::new(data, Widget::column(Vec::<Widget>::new()));
    assert_eq!(query.data().viewport, incular_core::Size::new(640., 480.));
    assert_eq!(query.data().scale_factor, 1.);
    assert_eq!(query.data().brightness, incular_config::Brightness::Dark);
}

#[test]
fn media_query_resize_updates_consumers() {
    let child = Widget::column(Vec::<Widget>::new());
    let first = MediaQuery::new(
        RuntimeEnvironment {
            viewport: incular_core::Size::new(320., 240.),
            ..RuntimeEnvironment::default()
        },
        child.clone(),
    );
    let second = MediaQuery::new(
        RuntimeEnvironment {
            viewport: incular_core::Size::new(800., 600.),
            ..RuntimeEnvironment::default()
        },
        child,
    );
    assert_ne!(first.data().viewport, second.data().viewport);
}

#[test]
fn media_query_brightness_update() {
    let child = Widget::column(Vec::<Widget>::new());
    let light = MediaQuery::new(RuntimeEnvironment::default(), child.clone());
    let dark = MediaQuery::new(
        RuntimeEnvironment {
            brightness: incular_config::Brightness::Dark,
            ..RuntimeEnvironment::default()
        },
        child,
    );
    assert_ne!(light.data().brightness, dark.data().brightness);
}

#[test]
fn locale_change_rebuilds_only_consumers() {
    let localizations = Localizations::new(
        "en".parse().expect("valid locale"),
        Catalog,
        Widget::column(Vec::<Widget>::new()),
    );
    let changed = localizations
        .clone()
        .with_locale("ar".parse().expect("valid locale"));
    assert_eq!(localizations.locale().to_string(), "en");
    assert_eq!(changed.locale().to_string(), "ar");
    assert_eq!(changed.text_direction(), TextDirection::Rtl);
}
