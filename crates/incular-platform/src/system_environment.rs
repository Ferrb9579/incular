//! Stable operating-system environment preference contracts.
//!
//! The desktop shell combines these application-level preferences with
//! per-window state (metrics, Winit theme, focus, occlusion) to create a full
//! [`RuntimeEnvironment`]. Unsupported values are represented by `None`; when
//! applied they intentionally reset to Incular's documented defaults rather
//! than preserving a stale value from an earlier snapshot.

use incular_config::{EdgeInsets, InputCapabilities, Locale, RuntimeEnvironment};
use std::sync::{Arc, RwLock};

/// Application-level system preferences observable by a native adapter.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SystemEnvironmentPreferences {
    /// Ordered OS locale preferences. Order is significant.
    pub locales: Option<Vec<Locale>>,
    /// Accessibility text scaling when the platform exposes a meaningful
    /// application text-scale value distinct from window DPI.
    pub text_scale: Option<f32>,
    pub reduced_motion: Option<bool>,
    pub high_contrast: Option<bool>,
    pub input: Option<InputCapabilities>,
    /// Desktop platforms normally leave these unsupported (`None`), but the
    /// contract is explicit for shells with desktop safe/view insets.
    pub safe_insets: Option<EdgeInsets>,
    pub view_insets: Option<EdgeInsets>,
}

impl SystemEnvironmentPreferences {
    /// Applies this complete preference snapshot. Every unsupported field is
    /// reset to the stable `RuntimeEnvironment::default()` value so a backend
    /// losing support cannot leave old native state cached indefinitely.
    pub fn apply_to(&self, environment: &mut RuntimeEnvironment) {
        let defaults = RuntimeEnvironment::default();
        environment.locales = self.locales.clone().unwrap_or(defaults.locales);
        environment.text_scale = self.text_scale.unwrap_or(defaults.text_scale);
        environment.reduced_motion = self.reduced_motion.unwrap_or(defaults.reduced_motion);
        environment.high_contrast = self.high_contrast.unwrap_or(defaults.high_contrast);
        environment.input = self.input.unwrap_or(defaults.input);
        environment.safe_insets = self.safe_insets.unwrap_or(defaults.safe_insets);
        environment.view_insets = self.view_insets.unwrap_or(defaults.view_insets);
    }
}

/// Snapshot source used by platform adapters and deterministic tests.
pub trait SystemEnvironmentProvider {
    fn snapshot(&self) -> SystemEnvironmentPreferences;
}

/// Thread-safe deterministic provider for headless hosts and tests.
#[derive(Clone, Default)]
pub struct MemorySystemEnvironmentProvider {
    snapshot: Arc<RwLock<SystemEnvironmentPreferences>>,
}

impl MemorySystemEnvironmentProvider {
    #[must_use]
    pub fn new(snapshot: SystemEnvironmentPreferences) -> Self {
        Self {
            snapshot: Arc::new(RwLock::new(snapshot)),
        }
    }

    /// Atomically replaces the complete preference snapshot and returns
    /// whether any semantic value changed.
    pub fn replace(&self, snapshot: SystemEnvironmentPreferences) -> bool {
        let mut current = self.snapshot.write().expect("environment provider lock");
        if *current == snapshot {
            return false;
        }
        *current = snapshot;
        true
    }
}

impl SystemEnvironmentProvider for MemorySystemEnvironmentProvider {
    fn snapshot(&self) -> SystemEnvironmentPreferences {
        self.snapshot
            .read()
            .expect("environment provider lock")
            .clone()
    }
}

/// Parses OS-provided BCP-47 locale strings through ICU4X, preserving order
/// while dropping malformed and duplicate canonical locales.
#[must_use]
pub fn canonicalize_system_locales(
    locales: impl IntoIterator<Item = impl AsRef<str>>,
) -> Vec<Locale> {
    let mut result = Vec::new();
    for value in locales {
        let Ok(locale) = value.as_ref().parse::<Locale>() else {
            continue;
        };
        if !result.contains(&locale) {
            result.push(locale);
        }
    }
    result
}
