//! ICU4X-backed locale selection and basic application localization services.
//!
//! This module owns no files, parsers, or generated CLDR data. Applications
//! provide their catalog through [`LocalizationCatalog`]; ICU4X compiled data
//! resolves locale fallback, directionality, decimal symbols, and dates.

use crate::{Locale, TextDirection};
use icu_datetime::{DateTimeFormatter, fieldsets, input::Date};
use icu_decimal::{DecimalFormatter, input::Decimal};
use icu_locale::{Direction, LocaleDirectionality, fallback::LocaleFallbacker};
pub use icu_plurals::PluralCategory;
use icu_plurals::PluralRules;
use icu_provider::DataLocale;
use std::fmt;

/// Failure while constructing an ICU4X formatter or validating an ISO date.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalizationError {
    message: String,
}

impl LocalizationError {
    fn from_display(error: impl fmt::Display) -> Self {
        Self {
            message: error.to_string(),
        }
    }
}

impl fmt::Display for LocalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for LocalizationError {}

/// Application-owned lookup boundary for localized resources.
///
/// The trait deliberately keeps catalog storage and message syntax outside of
/// the runtime. A catalog may be static, generated, embedded, or supplied by
/// an application, but all selection must use its declared supported locales.
pub trait LocalizationCatalog {
    /// Locales for which this catalog has complete resources.
    fn supported_locales(&self) -> &[Locale];

    /// Resolves a message key in one already-resolved supported locale.
    fn message(&self, locale: &Locale, key: &str) -> Option<&str>;
}

/// A catalog message together with the actual supported locale selected for
/// it. The locale may be a fallback of the user's preferred locale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalizedMessage<'a> {
    /// The catalog locale which supplied [`Self::value`].
    pub locale: Locale,
    /// The application-owned localized message text.
    pub value: &'a str,
}

/// Application-supplied variants for one cardinal-plural message.
///
/// The catalog owns message syntax and interpolation. This small adapter uses
/// ICU4X only to select the CLDR cardinal category for a locale.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PluralForms<'a> {
    pub zero: Option<&'a str>,
    pub one: Option<&'a str>,
    pub two: Option<&'a str>,
    pub few: Option<&'a str>,
    pub many: Option<&'a str>,
    pub other: &'a str,
}

impl<'a> PluralForms<'a> {
    /// Returns the supplied category's text, falling back to the mandatory
    /// `other` form when a catalog does not define that category.
    #[must_use]
    pub fn for_category(self, category: PluralCategory) -> &'a str {
        match category {
            PluralCategory::Zero => self.zero.unwrap_or(self.other),
            PluralCategory::One => self.one.unwrap_or(self.other),
            PluralCategory::Two => self.two.unwrap_or(self.other),
            PluralCategory::Few => self.few.unwrap_or(self.other),
            PluralCategory::Many => self.many.unwrap_or(self.other),
            PluralCategory::Other => self.other,
        }
    }
}

/// Resolves preferred locales against application-supported locales using
/// ICU4X's CLDR fallback data.
#[derive(Clone, Debug, Default)]
pub struct LocaleResolver;

impl LocaleResolver {
    /// Selects the first preferred locale supported by the application,
    /// following ICU4X parent-locale fallback. If none match, the first
    /// application-supported locale is the explicit deterministic fallback.
    #[must_use]
    pub fn resolve(preferred: &[Locale], supported: &[Locale]) -> Option<Locale> {
        for preferred_locale in preferred {
            let mut fallback = LocaleFallbacker::new()
                .for_config(Default::default())
                .fallback_for(DataLocale::from(preferred_locale));
            loop {
                let candidate = *fallback.get();
                if let Some(supported_locale) = supported
                    .iter()
                    .find(|supported_locale| DataLocale::from(*supported_locale) == candidate)
                {
                    return Some(supported_locale.clone());
                }
                if candidate == DataLocale::default() {
                    break;
                }
                fallback.step();
            }
        }
        supported.first().cloned()
    }

    /// Resolves a message through an application catalog after selecting its
    /// best available locale.
    #[must_use]
    pub fn resolve_message<'a>(
        preferred: &[Locale],
        catalog: &'a dyn LocalizationCatalog,
        key: &str,
    ) -> Option<LocalizedMessage<'a>> {
        let locale = Self::resolve(preferred, catalog.supported_locales())?;
        let value = catalog.message(&locale, key)?;
        Some(LocalizedMessage { locale, value })
    }

    /// Uses ICU4X script direction data and likely-subtags expansion to
    /// resolve the simple widget-facing directionality value.
    #[must_use]
    pub fn text_direction(locale: &Locale) -> TextDirection {
        match LocaleDirectionality::new_extended().get(&locale.id) {
            Some(Direction::RightToLeft) => TextDirection::Rtl,
            Some(Direction::LeftToRight) | Some(_) | None => TextDirection::Ltr,
        }
    }

    /// Formats an integer with the locale's decimal symbols and numbering
    /// system using ICU4X compiled data.
    pub fn format_integer(locale: &Locale, value: i64) -> Result<String, LocalizationError> {
        let formatter = DecimalFormatter::try_new(locale.clone().into(), Default::default())
            .map_err(LocalizationError::from_display)?;
        Ok(formatter.format(&Decimal::from(value)).to_string())
    }

    /// Formats an ISO Gregorian date using the locale's medium year/month/day
    /// field set and any supported calendar preference encoded in the locale.
    pub fn format_iso_date(
        locale: &Locale,
        year: i32,
        month: u8,
        day: u8,
    ) -> Result<String, LocalizationError> {
        let formatter = DateTimeFormatter::try_new(locale.clone().into(), fieldsets::YMD::medium())
            .map_err(LocalizationError::from_display)?;
        let date = Date::try_new_iso(year, month, day).map_err(LocalizationError::from_display)?;
        Ok(formatter.format(&date).to_string())
    }

    /// Selects the CLDR cardinal category for an integer using ICU4X's
    /// compiled locale data.
    pub fn plural_category(
        locale: &Locale,
        value: i64,
    ) -> Result<PluralCategory, LocalizationError> {
        let rules = PluralRules::try_new_cardinal(locale.clone().into())
            .map_err(LocalizationError::from_display)?;
        Ok(rules.category_for(value))
    }

    /// Selects an application-provided plural form through ICU4X cardinal
    /// rules. Formatting the numeric argument itself remains explicit via
    /// [`Self::format_integer`], keeping catalog syntax application-owned.
    pub fn select_plural<'a>(
        locale: &Locale,
        value: i64,
        forms: PluralForms<'a>,
    ) -> Result<&'a str, LocalizationError> {
        Ok(forms.for_category(Self::plural_category(locale, value)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
