//! Stable, logical application-environment values.
//!
//! These types deliberately contain no window handles or native event-loop
//! details. `incular-runtime` owns their current value and invalidation; native
//! adapters only normalize platform data into this snapshot.

use crate::{ApplicationDefaults, EdgeInsets, LocaleResolver, TextDirection};
use icu_locale::Locale;
use incular_core::Size;

/// The platform colour preference known to the application.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Brightness {
    #[default]
    Light,
    Dark,
}

/// The normalized sensitivity requested for a window's captured content.
///
/// The values intentionally mirror Flutter's `ContentSensitivity` policy:
/// `Sensitive` has the highest priority, followed by `AutoSensitive`, then
/// `NotSensitive`.  Platform crates consume this value without needing to
/// know anything about widget identity or the retained tree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ContentSensitivity {
    #[default]
    NotSensitive,
    AutoSensitive,
    Sensitive,
}

impl ContentSensitivity {
    /// Returns the more protective of two requested sensitivities.
    #[must_use]
    pub const fn max(self, other: Self) -> Self {
        if self as u8 >= other as u8 {
            self
        } else {
            other
        }
    }
}

/// Stable pointer/input capabilities reported by a platform adapter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputCapabilities {
    pub mouse: bool,
    pub touch: bool,
    pub keyboard: bool,
    pub stylus: bool,
}

/// A normalized, logical window/application environment snapshot.
///
/// Insets and viewport dimensions are logical pixels. Physical dimensions are
/// retained only for resource/cache decisions; layout must use `viewport`.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeEnvironment {
    pub viewport: Size,
    pub physical_width: u32,
    pub physical_height: u32,
    pub scale_factor: f64,
    pub brightness: Brightness,
    pub text_scale: f32,
    pub safe_insets: EdgeInsets,
    pub view_insets: EdgeInsets,
    /// Ordered locale preferences parsed and canonicalized by ICU4X.
    pub locales: Vec<Locale>,
    pub text_direction: TextDirection,
    pub reduced_motion: bool,
    /// Native high-contrast/accessibility preference. This is intentionally
    /// independent of [`Brightness`]; platforms that cannot observe it leave
    /// the documented default (`false`) rather than inferring it from theme.
    pub high_contrast: bool,
    pub input: InputCapabilities,
    /// Native activation for this window. Logical widget focus is retained by
    /// the runtime separately, so an unfocused window can resume its prior
    /// text-field focus when activated again.
    pub window_focused: bool,
    /// Whether the native compositor reports this window as fully occluded.
    /// Minimized-state scheduling is handled by the window adapter separately;
    /// this flag represents only an actual occlusion signal.
    pub window_occluded: bool,
}

impl Default for RuntimeEnvironment {
    fn default() -> Self {
        let defaults = ApplicationDefaults::DEFAULT;
        Self {
            viewport: Size::ZERO,
            physical_width: 0,
            physical_height: 0,
            scale_factor: defaults.scale_factor,
            brightness: defaults.brightness,
            text_scale: defaults.text_scale,
            safe_insets: EdgeInsets::ZERO,
            view_insets: EdgeInsets::ZERO,
            locales: Vec::new(),
            text_direction: defaults.text_direction,
            reduced_motion: defaults.reduced_motion,
            high_contrast: false,
            input: InputCapabilities::default(),
            window_focused: false,
            window_occluded: false,
        }
    }
}

impl RuntimeEnvironment {
    /// Normalizes externally supplied values without inventing platform policy.
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.scale_factor = if self.scale_factor.is_finite() && self.scale_factor > 0.0 {
            self.scale_factor
        } else {
            1.0
        };
        self.text_scale = if self.text_scale.is_finite() && self.text_scale > 0.0 {
            self.text_scale
        } else {
            1.0
        };
        self.safe_insets = self.safe_insets.normalized();
        self.view_insets = self.view_insets.normalized();
        if let Some(locale) = self.primary_locale() {
            self.text_direction = LocaleResolver::text_direction(locale);
        }
        self
    }

    #[must_use]
    pub fn primary_locale(&self) -> Option<&Locale> {
        self.locales.first()
    }

    /// Resolves application-supported locales from the platform's ordered
    /// preferences using ICU4X parent-locale fallback.
    #[must_use]
    pub fn resolve_locale(&self, supported: &[Locale]) -> Option<Locale> {
        LocaleResolver::resolve(&self.locales, supported)
    }
}
