//! ICU4X locale fallback, directionality, and localized formatting.
use incular::prelude::*;

struct DemoCatalog {
    supported: Vec<Locale>,
}

impl LocalizationCatalog for DemoCatalog {
    fn supported_locales(&self) -> &[Locale] {
        &self.supported
    }

    fn message(&self, locale: &Locale, key: &str) -> Option<&str> {
        match (locale.to_string().as_str(), key) {
            ("en", "heading") => Some("ICU4X localization"),
            ("fr", "heading") => Some("Localisation ICU4X"),
            ("ar", "heading") => Some("التوطين عبر ICU4X"),
            _ => None,
        }
    }
}

fn locale(value: &str) -> Locale {
    value.parse().expect("valid ICU locale")
}

#[path = "../tests/support/mod.rs"]
pub(crate) mod example_support;
#[path = "tests/simulations.rs"]
pub(crate) mod simulations;

fn main() {
    let catalog = DemoCatalog {
        supported: vec![locale("en"), locale("fr"), locale("ar")],
    };
    // `fr-CA` intentionally demonstrates ICU's parent-locale fallback to `fr`.
    let preferred = vec![locale("fr-CA")];
    let selected = LocaleResolver::resolve(&preferred, catalog.supported_locales())
        .expect("catalog has a fallback locale");
    let heading = LocaleResolver::resolve_message(&preferred, &catalog, "heading")
        .expect("catalog heading")
        .value
        .to_owned();
    let bangla = locale("bn");
    let formatted_number =
        LocaleResolver::format_integer(&bangla, 1_000_007).expect("compiled ICU decimal data");
    let formatted_date =
        LocaleResolver::format_iso_date(&selected, 2026, 8, 18).expect("compiled ICU date data");
    let arabic = locale("ar");

    let detail = format!(
        "Preferred: fr-CA\nResolved catalog locale: {selected}\n\n{heading}\n\nBengali number: {formatted_number}\nLocalized date: {formatted_date}\nArabic direction: {:?}\n\nResize or run the environment example to inspect the platform locale.",
        LocaleResolver::text_direction(&arabic),
    );
    let app = Application::new(move |_| {
        Padding::all(
            28.,
            DecoratedBox::new(Text::new(detail.clone()).style(TextStyle {
                size: 20.,
                color: Color::rgba(242, 246, 255, 255),
                ..TextStyle::default()
            }))
            .background(Color::rgba(32, 48, 76, 255))
            .radius(16.),
        )
        .into()
    })
    .expect("valid localization application");
    example_support::spawn_if_requested(app.simulation(), simulations::run);
    incular::run(app).expect("native localization application");
}
