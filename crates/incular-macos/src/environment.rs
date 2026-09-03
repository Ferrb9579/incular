use block2::RcBlock;
use incular_platform::{
    PlatformLifecycle, SystemEnvironmentPreferences, canonicalize_system_locales,
};
use objc2::rc::Retained;
use objc2_app_kit::{
    NSApplication, NSApplicationDidBecomeActiveNotification,
    NSApplicationDidResignActiveNotification, NSWorkspace,
    NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification, NSWorkspaceDidWakeNotification,
    NSWorkspaceWillSleepNotification,
};
use objc2_foundation::{
    MainThreadMarker, NSCurrentLocaleDidChangeNotification, NSNotification, NSNotificationCenter,
    NSObject,
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    ptr::NonNull,
    rc::Rc,
    sync::Arc,
};

#[derive(Clone, Default)]
pub(crate) struct MacosEnvironmentState {
    inner: Rc<Inner>,
}

#[derive(Default)]
struct Inner {
    settings_dirty: Cell<bool>,
    lifecycle: RefCell<VecDeque<PlatformLifecycle>>,
    observer_tokens: RefCell<Vec<(Retained<NSNotificationCenter>, Retained<NSObject>)>>,
    watching: Cell<bool>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        if self.observer_tokens.get_mut().is_empty() {
            return;
        }
        // SAFETY: the desktop runner creates and drops platform services on
        // AppKit's main thread. Each observer retains the exact notification
        // center that created it, and removal is synchronous.
        for (center, observer) in self.observer_tokens.get_mut().drain(..) {
            unsafe { center.removeObserver(&observer) };
        }
    }
}

impl MacosEnvironmentState {
    pub(crate) fn preferences(&self) -> SystemEnvironmentPreferences {
        let locales = canonicalize_system_locales(sys_locale::get_locales());
        // SAFETY: the platform service is queried on AppKit's event-loop
        // thread. NSWorkspace returns a retained process-global workspace and
        // the accessibility accessors are synchronous reads.
        let workspace = unsafe { NSWorkspace::sharedWorkspace() };
        SystemEnvironmentPreferences {
            locales: (!locales.is_empty()).then_some(locales),
            reduced_motion: Some(unsafe { workspace.accessibilityDisplayShouldReduceMotion() }),
            high_contrast: Some(unsafe { workspace.accessibilityDisplayShouldIncreaseContrast() }),
            ..SystemEnvironmentPreferences::default()
        }
    }

    pub(crate) fn application_active(&self) -> Option<bool> {
        let mtm = MainThreadMarker::new()?;
        let application = NSApplication::sharedApplication(mtm);
        // SAFETY: `mtm` proves this query runs on AppKit's main thread.
        Some(unsafe { application.isActive() })
    }

    pub(crate) fn start_watch(&self, wake: Arc<dyn Fn() + Send + Sync>) {
        if self.inner.watching.replace(true) {
            return;
        }
        // SAFETY: watcher installation happens on the AppKit event-loop thread.
        // NSNotificationCenter copies each block and the returned observer token
        // is retained in `Inner` until explicit removal during Drop.
        let center = unsafe { NSNotificationCenter::defaultCenter() };
        let workspace = unsafe { NSWorkspace::sharedWorkspace() };
        let workspace_center = unsafe { workspace.notificationCenter() };

        // Locale changes are Foundation notifications and use the default
        // notification center.
        let locale_notification = unsafe { NSCurrentLocaleDidChangeNotification };
        {
            let dirty = self.inner.clone();
            let wake = wake.clone();
            let block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
                dirty.settings_dirty.set(true);
                wake();
            });
            let observer = unsafe {
                center.addObserverForName_object_queue_usingBlock(
                    Some(locale_notification),
                    None,
                    None,
                    &block,
                )
            };
            self.inner
                .observer_tokens
                .borrow_mut()
                .push((center.clone(), observer));
        }

        // Apple requires accessibility-display changes to be observed from
        // NSWorkspace.notificationCenter; registering this name on the default
        // Foundation center silently misses the notification.
        let accessibility_notification =
            unsafe { NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification };
        {
            let dirty = self.inner.clone();
            let wake = wake.clone();
            let block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
                dirty.settings_dirty.set(true);
                wake();
            });
            let observer = unsafe {
                workspace_center.addObserverForName_object_queue_usingBlock(
                    Some(accessibility_notification),
                    Some(&workspace),
                    None,
                    &block,
                )
            };
            self.inner
                .observer_tokens
                .borrow_mut()
                .push((workspace_center.clone(), observer));
        }

        // SAFETY: same immutable framework-global lifetime invariant as above.
        let lifecycle_notifications = unsafe {
            [
                (
                    NSApplicationDidBecomeActiveNotification,
                    PlatformLifecycle::Active,
                ),
                (
                    NSApplicationDidResignActiveNotification,
                    PlatformLifecycle::Inactive,
                ),
            ]
        };
        for (name, lifecycle) in lifecycle_notifications {
            let state = self.inner.clone();
            let wake = wake.clone();
            let block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
                state.lifecycle.borrow_mut().push_back(lifecycle);
                wake();
            });
            let observer = unsafe {
                center.addObserverForName_object_queue_usingBlock(Some(name), None, None, &block)
            };
            self.inner
                .observer_tokens
                .borrow_mut()
                .push((center.clone(), observer));
        }

        // System sleep/wake is a genuine runnable-state transition on macOS,
        // unlike Winit's synthetic desktop `resumed` callback. NSWorkspace
        // owns these notifications, so observe them on its notification center.
        let sleep_notifications = unsafe {
            [
                (
                    NSWorkspaceWillSleepNotification,
                    PlatformLifecycle::Suspended,
                ),
                (NSWorkspaceDidWakeNotification, PlatformLifecycle::Resumed),
            ]
        };
        for (name, lifecycle) in sleep_notifications {
            let state = self.inner.clone();
            let wake = wake.clone();
            let block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
                state.lifecycle.borrow_mut().push_back(lifecycle);
                wake();
            });
            let observer = unsafe {
                workspace_center.addObserverForName_object_queue_usingBlock(
                    Some(name),
                    Some(&workspace),
                    None,
                    &block,
                )
            };
            self.inner
                .observer_tokens
                .borrow_mut()
                .push((workspace_center.clone(), observer));
        }
    }

    pub(crate) fn take_settings_change(&self) -> bool {
        self.inner.settings_dirty.replace(false)
    }

    pub(crate) fn take_lifecycle_events(&self) -> Vec<PlatformLifecycle> {
        self.inner.lifecycle.borrow_mut().drain(..).collect()
    }
}
