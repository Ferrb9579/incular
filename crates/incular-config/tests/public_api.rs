use incular_config::{
    Alignment, AlignmentDirectional, ApplicationDefaults, EdgeInsets, Locale, LocaleResolver,
    LocalizationCatalog, PluralForms, RuntimeEnvironment, TextDirection, WidgetDefaults,
};
use incular_core::{Offset, Size};

#[test]
fn alignment_places_child_in_remaining_space() {
    assert_eq!(
        Alignment::CENTER.within(Size::new(100.0, 80.0), Size::new(20.0, 10.0)),
        Offset::new(40.0, 35.0)
    );
    assert_eq!(
        AlignmentDirectional::CENTER_START.resolve(TextDirection::Rtl),
        Alignment::CENTER_RIGHT
    );
}

#[test]
fn application_defaults_are_valid_for_initialization() {
    let defaults = ApplicationDefaults::DEFAULT;
    assert!(defaults.initial_window_size.width > 0.0);
    assert!(defaults.initial_window_size.height > 0.0);
    assert!(defaults.scale_factor.is_finite() && defaults.scale_factor > 0.0);
    assert!(defaults.text_scale.is_finite() && defaults.text_scale > 0.0);
    assert_eq!(defaults, ApplicationDefaults::default());
}

#[test]
fn widget_defaults_are_explicit_and_safe_for_lazy_layout() {
    let defaults = WidgetDefaults::DEFAULT;
    assert!(defaults.text_size > 0.0);
    assert!(defaults.lazy_item_extent > 0.0);
    assert!(defaults.sliver_cache_extent >= 0.0);
    assert!(defaults.sliver_fill_viewport_extent > 0.0);
    assert_eq!(defaults.grid_cross_axis_count, 1);
    assert_eq!(defaults, WidgetDefaults::default());
}

#[test]
fn environment_normalizes_only_invalid_input() {
    let environment = RuntimeEnvironment {
        scale_factor: f64::NAN,
        text_scale: -1.,
        safe_insets: EdgeInsets::only(-1., 2., f32::NAN, 4.),
        locales: vec!["en-IN".parse().expect("valid ICU locale")],
        ..RuntimeEnvironment::default()
    }
    .normalized();
    assert_eq!(environment.scale_factor, 1.);
    assert_eq!(environment.text_scale, 1.);
    assert_eq!(environment.safe_insets, EdgeInsets::only(0., 2., 0., 4.));
    assert_eq!(
        environment
            .primary_locale()
            .map(ToString::to_string)
            .as_deref(),
        Some("en-IN")
    );
}

#[test]
fn locales_use_icu_canonical_casing() {
    let locale: Locale = "en-us".parse().expect("valid ICU locale");
    assert_eq!(locale.to_string(), "en-US");
}

#[test]
fn normalized_locale_resolves_direction_with_icu() {
    let environment = RuntimeEnvironment {
        locales: vec!["ar".parse().expect("valid ICU locale")],
        text_direction: TextDirection::Ltr,
        ..RuntimeEnvironment::default()
    }
    .normalized();
    assert_eq!(environment.text_direction, TextDirection::Rtl);
}

#[test]
fn constructors_and_dimensions_are_consistent() {
    let insets = EdgeInsets::from_xy(2.0, 3.0);
    assert_eq!(insets, EdgeInsets::only(2.0, 3.0, 2.0, 3.0));
    assert_eq!(insets.dimensions(), (4.0, 6.0));
}

#[test]
fn normalization_removes_invalid_values() {
    let normalized = EdgeInsets::only(-1.0, f32::NAN, 2.0, f32::INFINITY).normalized();
    assert_eq!(normalized, EdgeInsets::only(0.0, 0.0, 2.0, 0.0));
}

struct TestCatalog {
    locales: Vec<Locale>,
}

impl LocalizationCatalog for TestCatalog {
    fn supported_locales(&self) -> &[Locale] {
        &self.locales
    }

    fn message(&self, locale: &Locale, key: &str) -> Option<&str> {
        match (locale.to_string().as_str(), key) {
            ("en", "title") => Some("Welcome"),
            ("fr", "title") => Some("Bienvenue"),
            _ => None,
        }
    }
}

fn locale(value: &str) -> Locale {
    value.parse().expect("valid ICU locale")
}

#[test]
fn preferred_locale_uses_icu_parent_fallback_before_catalog_default() {
    let catalog = TestCatalog {
        locales: vec![locale("en"), locale("fr")],
    };
    let preferred = [locale("fr-CA")];
    let message = LocaleResolver::resolve_message(&preferred, &catalog, "title")
        .expect("French parent locale resolves");
    assert_eq!(message.locale, locale("fr"));
    assert_eq!(message.value, "Bienvenue");
    assert_eq!(
        LocaleResolver::resolve(&[locale("ja")], catalog.supported_locales()),
        Some(locale("en"))
    );
}

#[test]
fn direction_and_formats_use_icu_data() {
    assert_eq!(
        LocaleResolver::text_direction(&locale("ar")),
        TextDirection::Rtl
    );
    assert_eq!(
        LocaleResolver::text_direction(&locale("en-Arab")),
        TextDirection::Rtl
    );
    assert_eq!(
        LocaleResolver::text_direction(&locale("en")),
        TextDirection::Ltr
    );
    assert_eq!(
        LocaleResolver::format_integer(&locale("bn"), 1_000_007)
            .expect("compiled ICU decimal data"),
        "১০,০০,০০৭"
    );
    let date = LocaleResolver::format_iso_date(&locale("en-US"), 2025, 1, 15)
        .expect("compiled ICU date data");
    assert!(date.contains("2025"));
}

#[test]
fn plural_selection_uses_icu_cardinal_categories() {
    let english = PluralForms {
        one: Some("one file"),
        other: "many files",
        ..PluralForms::default()
    };
    assert_eq!(
        LocaleResolver::select_plural(&locale("en"), 1, english),
        Ok("one file")
    );
    assert_eq!(
        LocaleResolver::select_plural(&locale("en"), 2, english),
        Ok("many files")
    );
    let russian = PluralForms {
        one: Some("one"),
        few: Some("few"),
        many: Some("many"),
        other: "other",
        ..PluralForms::default()
    };
    assert_eq!(
        LocaleResolver::select_plural(&locale("ru"), 22, russian),
        Ok("few")
    );
}
