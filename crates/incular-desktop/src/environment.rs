//! Desktop environment normalization.
//!
//! One provider belongs to each native top-level window. Application-scoped OS
//! preferences come from the platform facade; Winit owns per-window theme,
//! focus, occlusion, metrics, and observed input device classes.

use incular_config::{Brightness, InputCapabilities, RuntimeEnvironment};
use incular_platform::SystemEnvironmentPreferences;
use winit::{window::Theme, window::Window};

#[derive(Clone, Debug)]
pub(crate) struct DesktopEnvironmentProvider {
    preferences: SystemEnvironmentPreferences,
    brightness: Option<Brightness>,
    focused: bool,
    occluded: bool,
    observed_input: InputCapabilities,
}

impl DesktopEnvironmentProvider {
    #[must_use]
    pub(crate) fn new(window: &Window, preferences: SystemEnvironmentPreferences) -> Self {
        Self {
            preferences,
            // `None` is a real backend state: Winit documents theme as
            // optional on platforms that cannot expose the system setting.
            // Keep that absence explicit and fall back through
            // RuntimeEnvironment's documented default policy at snapshot time
            // instead of pretending Light was observed from the OS.
            brightness: window.theme().map(brightness_from_theme),
            focused: window.has_focus(),
            occluded: false,
            observed_input: InputCapabilities::default(),
        }
    }

    pub(crate) fn replace_preferences(
        &mut self,
        preferences: SystemEnvironmentPreferences,
    ) -> bool {
        if self.preferences == preferences {
            return false;
        }
        self.preferences = preferences;
        true
    }

    pub(crate) fn set_theme(&mut self, theme: Theme) -> bool {
        let brightness = Some(brightness_from_theme(theme));
        if self.brightness == brightness {
            return false;
        }
        self.brightness = brightness;
        true
    }

    pub(crate) fn set_focused(&mut self, focused: bool) -> bool {
        if self.focused == focused {
            return false;
        }
        self.focused = focused;
        true
    }

    pub(crate) fn set_occluded(&mut self, occluded: bool) -> bool {
        if self.occluded == occluded {
            return false;
        }
        self.occluded = occluded;
        true
    }

    pub(crate) fn note_mouse(&mut self) -> bool {
        if self.observed_input.mouse {
            return false;
        }
        self.observed_input.mouse = true;
        true
    }

    pub(crate) fn note_touch(&mut self) -> bool {
        if self.observed_input.touch {
            return false;
        }
        self.observed_input.touch = true;
        true
    }

    pub(crate) fn note_keyboard(&mut self) -> bool {
        if self.observed_input.keyboard {
            return false;
        }
        self.observed_input.keyboard = true;
        true
    }

    pub(crate) fn note_stylus(&mut self) -> bool {
        if self.observed_input.stylus {
            return false;
        }
        self.observed_input.stylus = true;
        true
    }

    pub(crate) fn note_trackpad(&mut self) -> bool {
        if self.observed_input.trackpad {
            return false;
        }
        self.observed_input.trackpad = true;
        true
    }

    #[must_use]
    pub(crate) fn is_occluded(&self) -> bool {
        self.occluded
    }

    #[must_use]
    pub(crate) fn snapshot(&self, metrics: incular_platform::WindowMetrics) -> RuntimeEnvironment {
        let defaults = RuntimeEnvironment::default();
        let mut environment = RuntimeEnvironment {
            viewport: metrics.logical_size(),
            physical_width: metrics.physical_size.width,
            physical_height: metrics.physical_size.height,
            scale_factor: metrics.scale_factor,
            brightness: self.brightness.unwrap_or(defaults.brightness),
            window_focused: self.focused,
            window_occluded: self.occluded,
            ..defaults
        };
        self.preferences.apply_to(&mut environment);
        // A platform capability query is authoritative when available, but
        // observing a real native event can only add a capability, never erase
        // one. This avoids the old desktop-wide mouse/touch/keyboard guesses.
        environment.input.mouse |= self.observed_input.mouse;
        environment.input.touch |= self.observed_input.touch;
        environment.input.keyboard |= self.observed_input.keyboard;
        environment.input.stylus |= self.observed_input.stylus;
        environment.input.trackpad |= self.observed_input.trackpad;
        environment.normalized()
    }
}

#[must_use]
pub(crate) const fn brightness_from_theme(theme: Theme) -> Brightness {
    match theme {
        Theme::Light => Brightness::Light,
        Theme::Dark => Brightness::Dark,
    }
}
