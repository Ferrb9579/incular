use ashpd::desktop::settings::{Contrast, ReducedMotion, Settings};
use futures_util::StreamExt;
use incular_platform::{SystemEnvironmentPreferences, canonicalize_system_locales};
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
pub(crate) struct LinuxEnvironmentState {
    inner: Arc<Inner>,
}

struct Inner {
    preferences: RwLock<SystemEnvironmentPreferences>,
    dirty: AtomicBool,
    watching: AtomicBool,
}

impl Default for LinuxEnvironmentState {
    fn default() -> Self {
        let locales = canonicalize_system_locales(sys_locale::get_locales());
        Self {
            inner: Arc::new(Inner {
                preferences: RwLock::new(SystemEnvironmentPreferences {
                    locales: (!locales.is_empty()).then_some(locales),
                    ..SystemEnvironmentPreferences::default()
                }),
                dirty: AtomicBool::new(false),
                watching: AtomicBool::new(false),
            }),
        }
    }
}

impl LinuxEnvironmentState {
    pub(crate) fn preferences(&self) -> SystemEnvironmentPreferences {
        self.inner
            .preferences
            .read()
            .expect("Linux environment preferences")
            .clone()
    }

    pub(crate) fn take_change(&self) -> bool {
        self.inner.dirty.swap(false, Ordering::AcqRel)
    }

    pub(crate) fn start_watch(
        &self,
        tokio: incular_runtime::TokioHandle,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) {
        if self.inner.watching.swap(true, Ordering::AcqRel) {
            return;
        }

        let state = self.inner.clone();
        let wake_reduced = wake.clone();
        tokio.spawn(async move {
            let Ok(settings) = Settings::new().await else {
                return;
            };
            if let Ok(value) = settings.reduced_motion().await {
                update_reduced_motion(&state, value, wake_reduced.as_ref());
            }
            let Ok(mut changes) = settings.receive_reduced_motion_changed().await else {
                return;
            };
            while let Some(value) = changes.next().await {
                update_reduced_motion(&state, value, wake_reduced.as_ref());
            }
        });

        let state = self.inner.clone();
        tokio.spawn(async move {
            let Ok(settings) = Settings::new().await else {
                return;
            };
            if let Ok(value) = settings.contrast().await {
                update_contrast(&state, value, wake.as_ref());
            }
            let Ok(mut changes) = settings.receive_contrast_changed().await else {
                return;
            };
            while let Some(value) = changes.next().await {
                update_contrast(&state, value, wake.as_ref());
            }
        });
    }
}

fn update_reduced_motion(inner: &Inner, value: ReducedMotion, wake: &dyn Fn()) {
    let reduced = matches!(value, ReducedMotion::ReducedMotion);
    let changed = {
        let mut preferences = inner
            .preferences
            .write()
            .expect("Linux environment preferences");
        if preferences.reduced_motion == Some(reduced) {
            false
        } else {
            preferences.reduced_motion = Some(reduced);
            true
        }
    };
    publish_change(inner, changed, wake);
}

fn update_contrast(inner: &Inner, value: Contrast, wake: &dyn Fn()) {
    let high_contrast = matches!(value, Contrast::High);
    let changed = {
        let mut preferences = inner
            .preferences
            .write()
            .expect("Linux environment preferences");
        if preferences.high_contrast == Some(high_contrast) {
            false
        } else {
            preferences.high_contrast = Some(high_contrast);
            true
        }
    };
    publish_change(inner, changed, wake);
}

fn publish_change(inner: &Inner, changed: bool, wake: &dyn Fn()) {
    if changed {
        inner.dirty.store(true, Ordering::Release);
        wake();
    }
}
