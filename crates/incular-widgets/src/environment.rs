//! Ambient configuration and inherited environment widgets.

use incular_config::{
    Locale, LocaleResolver, LocalizationCatalog, LocalizedMessage, RuntimeEnvironment,
    TextDirection,
};
use incular_core::{BuildContext, Color};
use incular_scroll::{ScrollController, ScrollPhysics};
use incular_text::TextStyle;
use std::{any::Any, rc::Rc};
use typed_builder::TypedBuilder;

use crate::{LayoutBuilder, Widget};

pub use incular_config::ContentSensitivity;

/// A typed, window-local environment snapshot for descendants that need
/// viewport, scale, safe-area, brightness, or accessibility information.
/// The runtime installs its authoritative snapshot on each `WidgetTree`; this
/// descriptor is useful for isolated subtrees and deterministic tests.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct MediaQuery {
    #[builder(setter(transform = |data: RuntimeEnvironment| data.normalized()))]
    data: RuntimeEnvironment,
    #[builder(setter(into))]
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
#[derive(Clone, TypedBuilder)]
pub struct Localizations {
    #[builder(setter(into))]
    locale: Locale,
    #[builder(setter(
        fn transform<C>(catalog: C) -> Rc<dyn LocalizationCatalog>
        where
            C: LocalizationCatalog + 'static,
        {
            Rc::new(catalog)
        }
    ))]
    catalog: Rc<dyn LocalizationCatalog>,
    #[builder(setter(into))]
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
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct DefaultSelectionStyle {
    #[builder(default, setter(strip_option))]
    cursor_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    selection_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    handle_color: Option<Color>,
    #[builder(setter(into))]
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
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct Directionality {
    text_direction: TextDirection,
    #[builder(setter(into))]
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
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct DefaultTextStyle {
    style: TextStyle,
    #[builder(setter(into))]
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
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct IconTheme {
    #[builder(default, setter(strip_option))]
    color: Option<Color>,
    #[builder(default, setter(transform = |size: f32| Some(size.max(0.0))))]
    size: Option<f32>,
    #[builder(default, setter(transform = |opacity: f32| Some(opacity.clamp(0.0, 1.0))))]
    opacity: Option<f32>,
    #[builder(setter(into))]
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
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct ScrollConfiguration {
    #[builder(default, setter(strip_option))]
    physics: Option<ScrollPhysics>,
    #[builder(setter(into))]
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
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct PrimaryScrollController {
    controller: ScrollController,
    #[builder(default = true, setter(skip))]
    automatically_inherit_for_platforms: bool,
    #[builder(setter(into))]
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
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct TickerMode {
    enabled: bool,
    #[builder(setter(into))]
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

/// Tracks the sensitivity requests contributed by a retained widget subtree.
///
/// Flutter resolves multiple [`SensitiveContent`] widgets by priority rather
/// than by nearest-ancestor shadowing: any `Sensitive` request wins, then any
/// `AutoSensitive` request, then `NotSensitive`.  The runtime takes a snapshot
/// of this value at its window boundary and emits one normalized platform
/// command when that snapshot changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SensitiveContentHost {
    sensitive_count: usize,
    auto_sensitive_count: usize,
    not_sensitive_count: usize,
    fallback: ContentSensitivity,
}

impl Default for SensitiveContentHost {
    fn default() -> Self {
        Self::new()
    }
}

impl SensitiveContentHost {
    /// Creates an empty host whose neutral fallback is `NotSensitive`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sensitive_count: 0,
            auto_sensitive_count: 0,
            not_sensitive_count: 0,
            fallback: ContentSensitivity::NotSensitive,
        }
    }

    /// Creates an empty host with an explicit policy to restore after its last
    /// sensitive-content registration is removed.
    #[must_use]
    pub const fn with_fallback(fallback: ContentSensitivity) -> Self {
        Self {
            fallback,
            ..Self::new()
        }
    }

    /// Registers one retained [`SensitiveContent`] request.
    pub fn register(&mut self, sensitivity: ContentSensitivity) {
        match sensitivity {
            ContentSensitivity::Sensitive => self.sensitive_count += 1,
            ContentSensitivity::AutoSensitive => self.auto_sensitive_count += 1,
            ContentSensitivity::NotSensitive => self.not_sensitive_count += 1,
        }
    }

    /// Removes one request. Returns `false` when the corresponding count was
    /// already empty, keeping stale lifecycle notifications harmless.
    pub fn unregister(&mut self, sensitivity: ContentSensitivity) -> bool {
        let count = match sensitivity {
            ContentSensitivity::Sensitive => &mut self.sensitive_count,
            ContentSensitivity::AutoSensitive => &mut self.auto_sensitive_count,
            ContentSensitivity::NotSensitive => &mut self.not_sensitive_count,
        };
        if *count == 0 {
            return false;
        }
        *count -= 1;
        true
    }

    /// Returns the highest-priority request currently registered, or `None`
    /// when no [`SensitiveContent`] widget is mounted.
    #[must_use]
    pub const fn calculated_content_sensitivity(&self) -> Option<ContentSensitivity> {
        if self.sensitive_count > 0 {
            Some(ContentSensitivity::Sensitive)
        } else if self.auto_sensitive_count > 0 {
            Some(ContentSensitivity::AutoSensitive)
        } else if self.not_sensitive_count > 0 {
            Some(ContentSensitivity::NotSensitive)
        } else {
            None
        }
    }

    /// Returns the policy that should be active at a window boundary,
    /// including the configured fallback when no widget is registered.
    #[must_use]
    pub const fn effective_content_sensitivity(&self) -> ContentSensitivity {
        match self.calculated_content_sensitivity() {
            Some(sensitivity) => sensitivity,
            None => self.fallback,
        }
    }

    /// Returns whether this host currently tracks at least one widget.
    #[must_use]
    pub const fn has_widgets(&self) -> bool {
        self.sensitive_count > 0 || self.auto_sensitive_count > 0 || self.not_sensitive_count > 0
    }

    /// Returns the number of registrations for one sensitivity value.
    #[must_use]
    pub const fn count(&self, sensitivity: ContentSensitivity) -> usize {
        match sensitivity {
            ContentSensitivity::Sensitive => self.sensitive_count,
            ContentSensitivity::AutoSensitive => self.auto_sensitive_count,
            ContentSensitivity::NotSensitive => self.not_sensitive_count,
        }
    }
}

/// Stops retained typed-environment lookup at this subtree boundary.
///
/// The retained equivalent of Flutter's static `LookupBoundary` methods is
/// [`LookupBoundary::lookup`]. Ordinary environment scopes nested inside the
/// boundary remain visible to descendants; only scopes outside the nearest
/// boundary are hidden.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct LookupBoundary {
    #[builder(setter(into))]
    child: Widget,
}

impl LookupBoundary {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }

    #[must_use]
    pub fn child(&self) -> &Widget {
        &self.child
    }

    /// Reads a typed environment value visible from the currently materialized
    /// retained builder. A lookup never crosses the nearest boundary.
    #[must_use]
    pub fn lookup<T: Any + Clone>() -> Option<T> {
        crate::tree::current_build_environment::<T>()
    }
}

impl From<LookupBoundary> for Widget {
    fn from(value: LookupBoundary) -> Self {
        Widget::environment_boundary(value.child)
    }
}

/// Marks content as sensitive to obscure it from window sharing, screen recording, and diagnostics.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct SensitiveContent {
    #[builder(default = ContentSensitivity::Sensitive)]
    sensitivity: ContentSensitivity,
    #[builder(setter(into))]
    child: Widget,
}

impl SensitiveContent {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            sensitivity: ContentSensitivity::Sensitive,
            child: child.into(),
        }
    }

    /// Creates a scope with Flutter's explicit sensitivity policy.
    #[must_use]
    pub fn with_sensitivity(sensitivity: ContentSensitivity, child: impl Into<Widget>) -> Self {
        Self {
            sensitivity,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn sensitivity(&self) -> ContentSensitivity {
        self.sensitivity
    }

    /// Compatibility setter for the original Incular boolean surface.
    /// `true` maps to `Sensitive`; `false` maps to `NotSensitive`.
    #[must_use]
    pub fn sensitive(mut self, sensitive: bool) -> Self {
        self.sensitivity = if sensitive {
            ContentSensitivity::Sensitive
        } else {
            ContentSensitivity::NotSensitive
        };
        self
    }
}

impl From<SensitiveContent> for Widget {
    fn from(value: SensitiveContent) -> Self {
        Widget::environment_scope(value.sensitivity, value.child)
    }
}
