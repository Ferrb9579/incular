use super::environment::MaterialScrollBehavior;
use crate::foundation::Theme;
use crate::{ThemeData, ThemeMode};
use incular_config::{Brightness, Locale, RuntimeEnvironment};
use incular_widgets::{BuildContext, CheckedModeBanner, DefaultTextStyle, SizedBox, Widget};
use std::collections::BTreeMap;
use std::rc::Rc;
use typed_builder::TypedBuilder;

/// A Material application root.
///
/// This is intentionally a retained descriptor rather than a runtime handle.
/// Runtime-owned routing can select a child through [`MaterialApp::route`]
/// and the navigation crate can replace that selection with its Navigator
/// projection at the integration boundary.
#[derive(Clone, TypedBuilder)]
#[builder(builder_method(name = typed_builder))]
pub struct MaterialApp {
    #[builder(default, setter(strip_option, into))]
    home: Option<Widget>,
    #[builder(default)]
    routes: BTreeMap<String, Widget>,
    #[builder(default, setter(strip_option, into))]
    initial_route: Option<String>,
    #[builder(default, setter(strip_option, into))]
    title: Option<String>,
    // ThemeData carries all of the component defaults and is intentionally
    // fairly rich. Keep it behind a pointer in the application descriptor so
    // fluent builder chains do not repeatedly move a large value on the
    // stack (and so a MaterialApp remains cheap to clone).
    #[builder(
        default = ThemeData::light_shared(),
        setter(transform = |theme: ThemeData| Rc::new(theme))
    )]
    theme: Rc<ThemeData>,
    #[builder(
        default,
        setter(transform = |theme: ThemeData| Some(Rc::new(theme)))
    )]
    dark_theme: Option<Rc<ThemeData>>,
    #[builder(default = ThemeMode::System)]
    theme_mode: ThemeMode,
    #[builder(default, setter(strip_option))]
    locale: Option<Locale>,
    #[builder(
        default = Vec::new(),
        setter(transform = |locales: impl IntoIterator<Item = Locale>| {
            locales.into_iter().collect::<Vec<_>>()
        })
    )]
    supported_locales: Vec<Locale>,
    #[builder(default, setter(strip_option, into))]
    restoration_scope_id: Option<String>,
    #[builder(default)]
    scroll_behavior: MaterialScrollBehavior,
    #[builder(
        default,
        setter(
            prefix = "with_",
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn(Widget) -> Widget>>
            where
                F: Fn(Widget) -> Widget + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    builder: Option<Rc<dyn Fn(Widget) -> Widget>>,
    #[builder(default)]
    debug_show_checked_mode_banner: bool,
}

impl MaterialApp {
    #[must_use]
    pub fn new(home: impl Into<Widget>) -> Self {
        Self {
            home: Some(home.into()),
            routes: BTreeMap::new(),
            initial_route: None,
            title: None,
            theme: ThemeData::light_shared(),
            dark_theme: None,
            theme_mode: ThemeMode::System,
            locale: None,
            supported_locales: Vec::new(),
            restoration_scope_id: None,
            scroll_behavior: MaterialScrollBehavior::default(),
            builder: None,
            debug_show_checked_mode_banner: false,
        }
    }

    /// Creates an app whose first child is supplied by the builder.
    #[must_use]
    pub fn from_builder(builder: impl Fn() -> Widget + 'static) -> Self {
        let mut app = Self::new(SizedBox::shrink());
        app.home = None;
        app.builder = Some(Rc::new(move |_| builder()));
        app
    }

    #[must_use]
    pub fn home(mut self, home: impl Into<Widget>) -> Self {
        self.home = Some(home.into());
        self
    }

    #[must_use]
    pub fn route(mut self, name: impl Into<String>, child: impl Into<Widget>) -> Self {
        self.routes.insert(name.into(), child.into());
        self
    }

    #[must_use]
    pub fn routes(mut self, routes: impl IntoIterator<Item = (String, Widget)>) -> Self {
        self.routes.extend(routes);
        self
    }

    #[must_use]
    pub fn initial_route(mut self, route: impl Into<String>) -> Self {
        self.initial_route = Some(route.into());
        self
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    #[must_use]
    pub fn theme(mut self, theme: ThemeData) -> Self {
        self.theme = Rc::new(theme);
        self
    }

    /// Sets a theme already owned by the application without copying the
    /// large descriptor through the native UI stack.
    #[must_use]
    pub fn theme_shared(mut self, theme: Rc<ThemeData>) -> Self {
        self.theme = theme;
        self
    }

    #[must_use]
    pub fn dark_theme(mut self, theme: ThemeData) -> Self {
        self.dark_theme = Some(Rc::new(theme));
        self
    }

    /// Sets a shared dark theme without copying the descriptor.
    #[must_use]
    pub fn dark_theme_shared(mut self, theme: Rc<ThemeData>) -> Self {
        self.dark_theme = Some(theme);
        self
    }

    #[must_use]
    pub fn theme_mode(mut self, mode: ThemeMode) -> Self {
        self.theme_mode = mode;
        self
    }

    /// Selects the locale exposed to the retained localization environment.
    /// Material text remains locale-neutral until an application installs its
    /// ICU4X catalog, but carrying the value here keeps the application root
    /// compatible with Flutter's common `locale`/`supportedLocales` API.
    #[must_use]
    pub fn locale(mut self, locale: Locale) -> Self {
        self.locale = Some(locale);
        self
    }

    #[must_use]
    pub fn supported_locales(mut self, locales: impl IntoIterator<Item = Locale>) -> Self {
        self.supported_locales = locales.into_iter().collect();
        self
    }

    #[must_use]
    pub fn restoration_scope_id(mut self, value: impl Into<String>) -> Self {
        self.restoration_scope_id = Some(value.into());
        self
    }

    #[must_use]
    pub fn scroll_behavior(mut self, behavior: MaterialScrollBehavior) -> Self {
        self.scroll_behavior = behavior;
        self
    }

    #[must_use]
    pub fn builder(mut self, builder: impl Fn(Widget) -> Widget + 'static) -> Self {
        self.builder = Some(Rc::new(builder));
        self
    }

    #[must_use]
    pub fn debug_show_checked_mode_banner(mut self, show: bool) -> Self {
        self.debug_show_checked_mode_banner = show;
        self
    }

    #[must_use]
    pub fn get_debug_show_checked_mode_banner(&self) -> bool {
        self.debug_show_checked_mode_banner
    }

    #[must_use]
    pub fn get_title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    #[must_use]
    pub fn get_theme_mode(&self) -> ThemeMode {
        self.theme_mode
    }

    #[must_use]
    pub fn get_locale(&self) -> Option<&Locale> {
        self.locale.as_ref()
    }

    #[must_use]
    pub fn get_supported_locales(&self) -> &[Locale] {
        &self.supported_locales
    }

    #[must_use]
    pub fn get_restoration_scope_id(&self) -> Option<&str> {
        self.restoration_scope_id.as_deref()
    }

    #[must_use]
    pub fn get_scroll_behavior(&self) -> &MaterialScrollBehavior {
        &self.scroll_behavior
    }

    #[must_use]
    pub fn build(&self, context: &BuildContext<'_>) -> Widget {
        let child = self
            .initial_route
            .as_ref()
            .and_then(|route| self.routes.get(route))
            .cloned()
            .or_else(|| self.home.clone())
            .unwrap_or_else(|| SizedBox::shrink().into());
        let system_brightness = context
            .depend_on::<RuntimeEnvironment>()
            .map_or(Brightness::Light, |environment| environment.brightness);
        let theme = match self.theme_mode {
            ThemeMode::Dark => self
                .dark_theme
                .clone()
                .unwrap_or_else(ThemeData::dark_shared),
            ThemeMode::Light => self.theme.clone(),
            ThemeMode::System if system_brightness == Brightness::Dark => self
                .dark_theme
                .clone()
                .unwrap_or_else(ThemeData::dark_shared),
            ThemeMode::System => self.theme.clone(),
        };
        let text_style = theme.core().text_theme.body_medium.clone();
        let themed: Widget = Theme::scope_shared(theme, child);
        // MaterialApp supplies the ambient body style just like Flutter's
        // WidgetsApp/MaterialApp. Plain `Text::new` remains intentionally
        // lightweight outside an application root, while descendants of a
        // Material app inherit the theme's readable foreground color.
        let themed = DefaultTextStyle::new(text_style, themed).into();
        let localized = if let Some(locale) = self.locale.clone() {
            Widget::environment_scope(locale, themed)
        } else {
            themed
        };
        let configured = self.scroll_behavior.wrap(localized);
        let configured = if let Some(builder) = self.builder.as_ref() {
            builder(configured)
        } else {
            configured
        };
        if self.debug_show_checked_mode_banner {
            CheckedModeBanner::new(configured).into()
        } else {
            configured
        }
    }
}

impl Default for MaterialApp {
    fn default() -> Self {
        Self::typed_builder().build()
    }
}

impl From<MaterialApp> for Widget {
    fn from(value: MaterialApp) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |context, _| value.build(context))
    }
}
