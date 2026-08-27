//! Ambient configuration and inherited environment widgets.

use incular_config::{
    Locale, LocaleResolver, LocalizationCatalog, LocalizedMessage, RuntimeEnvironment,
    TextDirection,
};
use incular_core::{BuildContext, Color};
use incular_scroll::{ScrollController, ScrollPhysics};
use incular_text::TextStyle;
use std::rc::Rc;

use crate::{LayoutBuilder, Widget};

/// A typed, window-local environment snapshot for descendants that need
/// viewport, scale, safe-area, brightness, or accessibility information.
/// The runtime installs its authoritative snapshot on each `WidgetTree`; this
/// descriptor is useful for isolated subtrees and deterministic tests.
#[derive(Clone, Debug, PartialEq)]
pub struct MediaQuery {
    data: RuntimeEnvironment,
    child: Widget,
}

/// The renderer-neutral data carried by [`MediaQuery`]. Incular's runtime
/// environment already owns viewport, scale, safe-area, brightness, and
/// accessibility values, so this is an alias rather than a second snapshot
/// type to keep synchronized.
pub type MediaQueryData = RuntimeEnvironment;

impl MediaQuery {
    #[must_use]
    pub fn new(data: RuntimeEnvironment, child: impl Into<Widget>) -> Self {
        Self {
            data: data.normalized(),
            child: child.into(),
        }
    }

    #[must_use]
    pub fn data(&self) -> &RuntimeEnvironment {
        &self.data
    }

    #[must_use]
    pub fn into_data(self) -> RuntimeEnvironment {
        self.data
    }
}

impl From<MediaQuery> for Widget {
    fn from(value: MediaQuery) -> Self {
        Widget::environment_scope(value.data, value.child)
    }
}

/// Typed localization scope for a retained subtree.
///
/// Catalog storage remains application-owned through [`LocalizationCatalog`]
/// and is exposed to deferred builders as `(Locale, Rc<dyn
/// LocalizationCatalog>)`. This keeps locale selection in `incular-config`
/// without recreating Flutter's delegate/inherited-widget hierarchy.
#[derive(Clone)]
pub struct Localizations {
    locale: Locale,
    catalog: Rc<dyn LocalizationCatalog>,
    child: Widget,
}

impl Localizations {
    #[must_use]
    pub fn new(
        locale: Locale,
        catalog: impl LocalizationCatalog + 'static,
        child: impl Into<Widget>,
    ) -> Self {
        Self {
            locale,
            catalog: Rc::new(catalog),
            child: child.into(),
        }
    }

    #[must_use]
    pub fn locale(&self) -> &Locale {
        &self.locale
    }

    #[must_use]
    pub fn catalog(&self) -> &Rc<dyn LocalizationCatalog> {
        &self.catalog
    }

    /// Replaces the preferred locale while retaining the same catalog and
    /// child subtree. Rebuilding a scope with this value invalidates only
    /// descendants that read the localization environment.
    #[must_use]
    pub fn with_locale(mut self, locale: Locale) -> Self {
        self.locale = locale;
        self
    }

    /// Resolves a message key through ICU4X locale fallback and this scope's
    /// application-owned catalog.
    #[must_use]
    pub fn resolve(&self, key: &str) -> Option<LocalizedMessage<'_>> {
        LocaleResolver::resolve_message(
            std::slice::from_ref(&self.locale),
            self.catalog.as_ref(),
            key,
        )
    }

    #[must_use]
    pub fn text(&self, key: &str) -> Option<String> {
        self.resolve(key).map(|message| message.value.to_owned())
    }

    #[must_use]
    pub fn text_direction(&self) -> TextDirection {
        LocaleResolver::text_direction(&self.locale)
    }
}

impl From<Localizations> for Widget {
    fn from(value: Localizations) -> Self {
        Widget::environment_scope((value.locale, value.catalog), value.child)
    }
}

/// Ambient selection colors for editable text descendants.
#[derive(Clone, Debug, PartialEq)]
pub struct DefaultSelectionStyle {
    cursor_color: Option<Color>,
    selection_color: Option<Color>,
    handle_color: Option<Color>,
    child: Widget,
}

impl DefaultSelectionStyle {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            cursor_color: None,
            selection_color: None,
            handle_color: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn cursor_color(mut self, color: Color) -> Self {
        self.cursor_color = Some(color);
        self
    }

    #[must_use]
    pub fn selection_color(mut self, color: Color) -> Self {
        self.selection_color = Some(color);
        self
    }

    #[must_use]
    pub fn handle_color(mut self, color: Color) -> Self {
        self.handle_color = Some(color);
        self
    }
}

impl From<DefaultSelectionStyle> for Widget {
    fn from(value: DefaultSelectionStyle) -> Self {
        let style = (
            value.cursor_color,
            value.selection_color,
            value.handle_color,
        );
        Widget::environment_scope(style, value.child)
    }
}

/// Directionality provides ambient text direction (`Ltr` or `Rtl`) to its subtree.
#[derive(Clone, Debug, PartialEq)]
pub struct Directionality {
    text_direction: TextDirection,
    child: Widget,
}

impl Directionality {
    #[must_use]
    pub fn new(text_direction: TextDirection, child: impl Into<Widget>) -> Self {
        Self {
            text_direction,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn text_direction(&self) -> TextDirection {
        self.text_direction
    }
}

impl From<Directionality> for Widget {
    fn from(value: Directionality) -> Self {
        Widget::environment_scope(value.text_direction, value.child)
    }
}

/// Sets the default [`TextStyle`] for descendant [`Text`](crate::Text) widgets.
#[derive(Clone, Debug, PartialEq)]
pub struct DefaultTextStyle {
    style: TextStyle,
    child: Widget,
}

impl DefaultTextStyle {
    #[must_use]
    pub fn new(style: TextStyle, child: impl Into<Widget>) -> Self {
        Self {
            style,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn style(&self) -> &TextStyle {
        &self.style
    }
}

impl From<DefaultTextStyle> for Widget {
    fn from(value: DefaultTextStyle) -> Self {
        Widget::environment_scope(value.style, value.child)
    }
}

/// Defines default color, size, and opacity for descendant [`Icon`](crate::Icon) widgets.
#[derive(Clone, Debug, PartialEq)]
pub struct IconTheme {
    color: Option<Color>,
    size: Option<f32>,
    opacity: Option<f32>,
    child: Widget,
}

impl IconTheme {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            color: None,
            size: None,
            opacity: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    #[must_use]
    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size.max(0.0));
        self
    }

    #[must_use]
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = Some(opacity.clamp(0.0, 1.0));
        self
    }
}

impl From<IconTheme> for Widget {
    fn from(value: IconTheme) -> Self {
        let child = value.child.clone();
        Widget::environment_scope(value, child)
    }
}

/// Device / viewport orientation.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Orientation {
    Portrait,
    Landscape,
}

/// Builds a widget subtree depending on the parent's orientation.
#[derive(Clone)]
#[allow(clippy::type_complexity)]
pub struct OrientationBuilder {
    builder: Rc<dyn Fn(&BuildContext, Orientation) -> Widget>,
}

impl OrientationBuilder {
    #[must_use]
    pub fn new<W>(builder: impl Fn(&BuildContext, Orientation) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            builder: Rc::new(move |ctx, orient| builder(ctx, orient).into()),
        }
    }
}

impl From<OrientationBuilder> for Widget {
    fn from(value: OrientationBuilder) -> Self {
        let builder = value.builder;
        LayoutBuilder::new(move |constraints| {
            let orientation = if constraints.max_width > constraints.max_height {
                Orientation::Landscape
            } else {
                Orientation::Portrait
            };
            let dummy_ctx = BuildContext::new();
            builder(&dummy_ctx, orientation)
        })
        .into()
    }
}

/// Controls ambient scroll physics and behavior for descendant scroll views.
#[derive(Clone, Debug, PartialEq)]
pub struct ScrollConfiguration {
    physics: Option<ScrollPhysics>,
    child: Widget,
}

impl ScrollConfiguration {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            physics: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn physics(mut self, physics: ScrollPhysics) -> Self {
        self.physics = Some(physics);
        self
    }
}

impl From<ScrollConfiguration> for Widget {
    fn from(value: ScrollConfiguration) -> Self {
        Widget::environment_scope(value.physics, value.child)
    }
}

/// Associates a [`ScrollController`] with the subtree as the default primary scroll controller.
#[derive(Clone, Debug, PartialEq)]
pub struct PrimaryScrollController {
    controller: ScrollController,
    automatically_inherit_for_platforms: bool,
    child: Widget,
}

impl PrimaryScrollController {
    #[must_use]
    pub fn new(controller: ScrollController, child: impl Into<Widget>) -> Self {
        Self {
            controller,
            automatically_inherit_for_platforms: true,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn controller(&self) -> &ScrollController {
        &self.controller
    }
}

impl From<PrimaryScrollController> for Widget {
    fn from(value: PrimaryScrollController) -> Self {
        Widget::environment_scope(value.controller, value.child)
    }
}

/// Enables or disables animation ticking for its subtree.
#[derive(Clone, Debug, PartialEq)]
pub struct TickerMode {
    enabled: bool,
    child: Widget,
}

impl TickerMode {
    #[must_use]
    pub fn new(enabled: bool, child: impl Into<Widget>) -> Self {
        Self {
            enabled,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl From<TickerMode> for Widget {
    fn from(value: TickerMode) -> Self {
        Widget::environment_scope(value.enabled, value.child)
    }
}

/// Marks content as sensitive to obscure it from window sharing, screen recording, and diagnostics.
#[derive(Clone, Debug, PartialEq)]
pub struct SensitiveContent {
    sensitive: bool,
    child: Widget,
}

impl SensitiveContent {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            sensitive: true,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn sensitive(mut self, sensitive: bool) -> Self {
        self.sensitive = sensitive;
        self
    }
}

impl From<SensitiveContent> for Widget {
    fn from(value: SensitiveContent) -> Self {
        Widget::environment_scope(value.sensitive, value.child)
    }
}

/// Boundary host coordinating sensitive content obscuration.
#[derive(Clone, Debug, PartialEq)]
pub struct SensitiveContentHost {
    child: Widget,
}

impl SensitiveContentHost {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<SensitiveContentHost> for Widget {
    fn from(value: SensitiveContentHost) -> Self {
        value.child
    }
}

/// Isolates inherited widget lookups across boundaries.
#[derive(Clone, Debug, PartialEq)]
pub struct LookupBoundary {
    child: Widget,
}

impl LookupBoundary {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<LookupBoundary> for Widget {
    fn from(value: LookupBoundary) -> Self {
        value.child
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
