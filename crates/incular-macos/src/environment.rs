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
    rc::{Rc, Weak},
    sync::Arc,
};

#[derive(Clone)]
pub(crate) struct MacosEnvironmentState {
    state: Rc<CallbackState>,
    observers: Rc<ObserverLease>,
}

#[derive(Default)]
struct CallbackState {
    settings_dirty: Cell<bool>,
    lifecycle: RefCell<VecDeque<PlatformLifecycle>>,
    active: Cell<bool>,
}

#[derive(Default)]
struct ObserverLease {
    state: Rc<CallbackState>,
    observer_tokens: RefCell<Vec<(Retained<NSNotificationCenter>, Retained<NSObject>)>>,
    watching: Cell<bool>,
}

impl ObserverLease {
    fn stop(&self) {
        self.state.active.set(false);
        self.watching.set(false);
        let mut observers = self.observer_tokens.borrow_mut();
        if observers.is_empty() {
            return;
        }
        // SAFETY: the desktop runner creates and drops platform services on
        // AppKit's main thread. Each observer retains the exact notification
        // center that created it, and removal is synchronous.
        for (center, observer) in observers.drain(..) {
            unsafe { center.removeObserver(&observer) };
        }
    }
}

impl Drop for ObserverLease {
    fn drop(&mut self) {
        self.state.active.set(false);
        self.watching.set(false);
        for (center, observer) in self.observer_tokens.get_mut().drain(..) {
            // SAFETY: see `stop`; the lease is dropped with the platform
            // service on the AppKit event-loop thread.
            unsafe { center.removeObserver(&observer) };
        }
    }
}

impl Default for MacosEnvironmentState {
    fn default() -> Self {
        let state = Rc::new(CallbackState::default());
        let observers = Rc::new(ObserverLease {
            state: state.clone(),
            ..ObserverLease::default()
        });
        Self { state, observers }
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
        if self.observers.watching.replace(true) {
            return;
        }
        self.state.active.set(true);
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
            let dirty = Rc::downgrade(&self.state);
            let wake = wake.clone();
            let block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
                if let Some(state) = active_state(&dirty) {
                    state.settings_dirty.set(true);
                    wake();
                }
            });
            let observer = unsafe {
                center.addObserverForName_object_queue_usingBlock(
                    Some(locale_notification),
                    None,
                    None,
                    &block,
                )
            };
            self.observers
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
            let dirty = Rc::downgrade(&self.state);
            let wake = wake.clone();
            let block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
                if let Some(state) = active_state(&dirty) {
                    state.settings_dirty.set(true);
                    wake();
                }
            });
            let observer = unsafe {
                workspace_center.addObserverForName_object_queue_usingBlock(
                    Some(accessibility_notification),
                    Some(&workspace),
                    None,
                    &block,
                )
            };
            self.observers
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
            let state = Rc::downgrade(&self.state);
            let wake = wake.clone();
            let block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
                if let Some(state) = active_state(&state) {
                    state.lifecycle.borrow_mut().push_back(lifecycle);
                    wake();
                }
            });
            let observer = unsafe {
                center.addObserverForName_object_queue_usingBlock(Some(name), None, None, &block)
            };
            self.observers
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
            let state = Rc::downgrade(&self.state);
            let wake = wake.clone();
            let block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
                if let Some(state) = active_state(&state) {
                    state.lifecycle.borrow_mut().push_back(lifecycle);
                    wake();
                }
            });
            let observer = unsafe {
                workspace_center.addObserverForName_object_queue_usingBlock(
                    Some(name),
                    Some(&workspace),
                    None,
                    &block,
                )
            };
            self.observers
                .observer_tokens
                .borrow_mut()
                .push((workspace_center.clone(), observer));
        }
    }

    pub(crate) fn take_settings_change(&self) -> bool {
        self.state.settings_dirty.replace(false)
    }

    pub(crate) fn take_lifecycle_events(&self) -> Vec<PlatformLifecycle> {
        self.state.lifecycle.borrow_mut().drain(..).collect()
    }
}

fn active_state(state: &Weak<CallbackState>) -> Option<Rc<CallbackState>> {
    state.upgrade().filter(|state| state.active.get())
}
