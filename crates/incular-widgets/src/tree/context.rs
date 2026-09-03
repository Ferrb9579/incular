//! Build-time inherited environment access.
//!
//! This module owns borrowed build context views and inherited descriptor values; retained dependency tracking remains owned by `WidgetTree`.

use super::*;
use incular_config::{
    Brightness, InputCapabilities, Locale, LocaleResolver, RuntimeEnvironment, TextDirection,
};

#[derive(Clone, Copy, Debug, PartialEq)]
struct RuntimeViewport(Size);
#[derive(Clone, Copy, Debug, PartialEq)]
struct RuntimeScaleFactor(f64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuntimeBrightness(Brightness);
#[derive(Clone, Copy, Debug, PartialEq)]
struct RuntimeTextScale(f32);
#[derive(Clone, Copy, Debug, PartialEq)]
struct RuntimeSafeInsets(EdgeInsets);
#[derive(Clone, Copy, Debug, PartialEq)]
struct RuntimeViewInsets(EdgeInsets);
#[derive(Clone, Debug, PartialEq)]
struct RuntimeLocales(Vec<Locale>);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuntimeTextDirection(TextDirection);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuntimeReducedMotion(bool);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuntimeHighContrast(bool);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuntimeInputCapabilities(InputCapabilities);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuntimeWindowFocused(bool);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuntimeWindowOccluded(bool);

/// A borrowed view of the retained inherited environment for one live build
/// operation. The reference cannot escape the callback that receives it.
pub struct BuildContext<'a> {
    inner: &'a DependencyContext,
}

pub(crate) type LayoutBuilderCallback = dyn for<'a> Fn(&BuildContext<'a>, Constraints) -> Widget;

impl<'a> BuildContext<'a> {
    pub(super) fn new(inner: &'a DependencyContext) -> Self {
        Self { inner }
    }

    /// Reads the nearest inherited value and registers this element as a
    /// dependent of the exact retained scope that supplied it.
    #[must_use]
    pub fn depend_on<T: Any + Clone>(&self) -> Option<T> {
        self.inner.depend::<T>()
    }

    /// Reads the nearest inherited value without creating an invalidation edge.
    #[must_use]
    pub fn find<T: Any + Clone>(&self) -> Option<T> {
        self.inner.read::<T>()
    }

    /// Shared-ownership form of [`BuildContext::depend_on`].
    #[must_use]
    pub fn depend_on_shared<T: Any>(&self) -> Option<Rc<T>> {
        self.inner.depend_shared::<T>()
    }

    /// Shared-ownership form of [`BuildContext::find`].
    #[must_use]
    pub fn find_shared<T: Any>(&self) -> Option<Rc<T>> {
        self.inner.read_shared::<T>()
    }

    /// Current logical viewport from the native/runtime environment. This is
    /// independent of inherited `MediaQuery` overrides and subscribes only to
    /// viewport changes in the owning retained window.
    #[must_use]
    pub fn viewport(&self) -> Size {
        self.inner
            .depend::<RuntimeViewport>()
            .map_or(Size::ZERO, |value| value.0)
    }

    #[must_use]
    pub fn scale_factor(&self) -> f64 {
        self.inner
            .depend::<RuntimeScaleFactor>()
            .map_or(1.0, |value| value.0)
    }

    #[must_use]
    pub fn brightness(&self) -> Brightness {
        self.inner
            .depend::<RuntimeBrightness>()
            .map_or(Brightness::Light, |value| value.0)
    }

    #[must_use]
    pub fn text_scale(&self) -> f32 {
        self.inner
            .depend::<RuntimeTextScale>()
            .map_or(1.0, |value| value.0)
    }

    #[must_use]
    pub fn safe_insets(&self) -> EdgeInsets {
        self.inner
            .depend::<RuntimeSafeInsets>()
            .map_or(EdgeInsets::ZERO, |value| value.0)
    }

    #[must_use]
    pub fn view_insets(&self) -> EdgeInsets {
        self.inner
            .depend::<RuntimeViewInsets>()
            .map_or(EdgeInsets::ZERO, |value| value.0)
    }

    #[must_use]
    pub fn locales(&self) -> Vec<Locale> {
        self.inner
            .depend::<RuntimeLocales>()
            .map_or_else(Vec::new, |value| value.0)
    }

    #[must_use]
    pub fn resolve_locale(&self, supported: &[Locale]) -> Option<Locale> {
        LocaleResolver::resolve(&self.locales(), supported)
    }

    #[must_use]
    pub fn text_direction(&self) -> TextDirection {
        self.inner
            .depend::<RuntimeTextDirection>()
            .map_or(TextDirection::Ltr, |value| value.0)
    }

    #[must_use]
    pub fn reduced_motion(&self) -> bool {
        self.inner
            .depend::<RuntimeReducedMotion>()
            .is_some_and(|value| value.0)
    }

    #[must_use]
    pub fn high_contrast(&self) -> bool {
        self.inner
            .depend::<RuntimeHighContrast>()
            .is_some_and(|value| value.0)
    }

    #[must_use]
    pub fn input_capabilities(&self) -> InputCapabilities {
        self.inner
            .depend::<RuntimeInputCapabilities>()
            .map_or_else(InputCapabilities::default, |value| value.0)
    }

    #[must_use]
    pub fn window_focused(&self) -> bool {
        self.inner
            .depend::<RuntimeWindowFocused>()
            .is_some_and(|value| value.0)
    }

    #[must_use]
    pub fn window_occluded(&self) -> bool {
        self.inner
            .depend::<RuntimeWindowOccluded>()
            .is_some_and(|value| value.0)
    }
}

pub(super) fn install_runtime_environment(
    context: &DependencyContext,
    environment: &RuntimeEnvironment,
) {
    context.set_erased(
        TypeId::of::<RuntimeViewport>(),
        Rc::new(RuntimeViewport(environment.viewport)),
    );
    context.set_erased(
        TypeId::of::<RuntimeScaleFactor>(),
        Rc::new(RuntimeScaleFactor(environment.scale_factor)),
    );
    context.set_erased(
        TypeId::of::<RuntimeBrightness>(),
        Rc::new(RuntimeBrightness(environment.brightness)),
    );
    context.set_erased(
        TypeId::of::<RuntimeTextScale>(),
        Rc::new(RuntimeTextScale(environment.text_scale)),
    );
    context.set_erased(
        TypeId::of::<RuntimeSafeInsets>(),
        Rc::new(RuntimeSafeInsets(environment.safe_insets)),
    );
    context.set_erased(
        TypeId::of::<RuntimeViewInsets>(),
        Rc::new(RuntimeViewInsets(environment.view_insets)),
    );
    context.set_erased(
        TypeId::of::<RuntimeLocales>(),
        Rc::new(RuntimeLocales(environment.locales.clone())),
    );
    context.set_erased(
        TypeId::of::<RuntimeTextDirection>(),
        Rc::new(RuntimeTextDirection(environment.text_direction)),
    );
    context.set_erased(
        TypeId::of::<RuntimeReducedMotion>(),
        Rc::new(RuntimeReducedMotion(environment.reduced_motion)),
    );
    context.set_erased(
        TypeId::of::<RuntimeHighContrast>(),
        Rc::new(RuntimeHighContrast(environment.high_contrast)),
    );
    context.set_erased(
        TypeId::of::<RuntimeInputCapabilities>(),
        Rc::new(RuntimeInputCapabilities(environment.input)),
    );
    context.set_erased(
        TypeId::of::<RuntimeWindowFocused>(),
        Rc::new(RuntimeWindowFocused(environment.window_focused)),
    );
    context.set_erased(
        TypeId::of::<RuntimeWindowOccluded>(),
        Rc::new(RuntimeWindowOccluded(environment.window_occluded)),
    );
}

pub(super) fn update_runtime_environment(
    context: &DependencyContext,
    previous: &RuntimeEnvironment,
    next: &RuntimeEnvironment,
) {
    macro_rules! replace_if_changed {
        ($field:ident, $wrapper:ident) => {
            if previous.$field != next.$field {
                context.set_erased(
                    TypeId::of::<$wrapper>(),
                    Rc::new($wrapper(next.$field.clone())),
                );
            }
        };
    }

    replace_if_changed!(viewport, RuntimeViewport);
    replace_if_changed!(scale_factor, RuntimeScaleFactor);
    replace_if_changed!(brightness, RuntimeBrightness);
    replace_if_changed!(text_scale, RuntimeTextScale);
    replace_if_changed!(safe_insets, RuntimeSafeInsets);
    replace_if_changed!(view_insets, RuntimeViewInsets);
    replace_if_changed!(locales, RuntimeLocales);
    replace_if_changed!(text_direction, RuntimeTextDirection);
    replace_if_changed!(reduced_motion, RuntimeReducedMotion);
    replace_if_changed!(high_contrast, RuntimeHighContrast);
    replace_if_changed!(input, RuntimeInputCapabilities);
    replace_if_changed!(window_focused, RuntimeWindowFocused);
    replace_if_changed!(window_occluded, RuntimeWindowOccluded);
}

#[derive(Clone)]
#[doc(hidden)]
pub struct InheritedScopeValue {
    pub(crate) type_id: TypeId,
    pub(crate) value: Rc<dyn Any>,
}

impl InheritedScopeValue {
    pub(super) fn new<T: Any>(value: T) -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            value: Rc::new(value),
        }
    }
}
