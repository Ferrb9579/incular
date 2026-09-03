use incular_config::InputCapabilities;
use incular_platform::{
    PlatformLifecycle, SystemEnvironmentPreferences, canonicalize_system_locales,
};
use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, AtomicU8, Ordering},
};
use windows::{
    Foundation::{EventRegistrationToken, TypedEventHandler},
    UI::ViewManagement::UISettings,
    core::IInspectable,
};
use windows_sys::Win32::UI::{
    Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
    WindowsAndMessaging::{
        GetForegroundWindow, GetSystemMetrics, GetWindowThreadProcessId, MSG, NID_EXTERNAL_PEN,
        NID_EXTERNAL_TOUCH, NID_INTEGRATED_PEN, NID_INTEGRATED_TOUCH, PBT_APMRESUMEAUTOMATIC,
        PBT_APMRESUMECRITICAL, PBT_APMRESUMESTANDBY, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND,
        SM_DIGITIZER, SM_MOUSEPRESENT, SPI_GETCLIENTAREAANIMATION, SPI_GETHIGHCONTRAST,
        SystemParametersInfoW, WM_ACTIVATEAPP, WM_DEVICECHANGE, WM_POWERBROADCAST,
        WM_SETTINGCHANGE,
    },
};

const ACTIVE_FALSE: u8 = 0;
const ACTIVE_TRUE: u8 = 1;

#[derive(Clone)]
pub(crate) struct WindowsEnvironmentState {
    inner: Arc<Inner>,
}

struct Inner {
    settings_dirty: AtomicBool,
    active: AtomicU8,
    suspended: AtomicBool,
    lifecycle: Mutex<VecDeque<PlatformLifecycle>>,
    text_scale_watch: Mutex<Option<TextScaleWatch>>,
}

struct TextScaleWatch {
    settings: UISettings,
    token: EventRegistrationToken,
}

impl Drop for TextScaleWatch {
    fn drop(&mut self) {
        // UISettings is agile (`Send + Sync` in windows-rs), so the handler
        // may be removed safely even if the final provider clone is released
        // away from the event-loop callback that installed it.
        let _ = self.settings.RemoveTextScaleFactorChanged(self.token);
    }
}

impl Default for WindowsEnvironmentState {
    fn default() -> Self {
        let active = query_application_active();
        Self {
            inner: Arc::new(Inner {
                settings_dirty: AtomicBool::new(false),
                active: AtomicU8::new(if active { ACTIVE_TRUE } else { ACTIVE_FALSE }),
                suspended: AtomicBool::new(false),
                lifecycle: Mutex::new(VecDeque::new()),
                text_scale_watch: Mutex::new(None),
            }),
        }
    }
}

impl WindowsEnvironmentState {
    pub(crate) fn preferences(&self) -> SystemEnvironmentPreferences {
        let locales = canonicalize_system_locales(sys_locale::get_locales());
        SystemEnvironmentPreferences {
            locales: (!locales.is_empty()).then_some(locales),
            text_scale: query_text_scale_factor(),
            reduced_motion: query_client_area_animation().map(|enabled| !enabled),
            high_contrast: query_high_contrast(),
            input: Some(query_input_capabilities()),
            ..SystemEnvironmentPreferences::default()
        }
    }

    pub(crate) fn application_active(&self) -> bool {
        self.inner.active.load(Ordering::Acquire) == ACTIVE_TRUE
    }

    pub(crate) fn take_settings_change(&self) -> bool {
        self.inner.settings_dirty.swap(false, Ordering::AcqRel)
    }

    pub(crate) fn take_lifecycle_events(&self) -> Vec<PlatformLifecycle> {
        self.inner
            .lifecycle
            .lock()
            .expect("Windows lifecycle queue")
            .drain(..)
            .collect()
    }

    pub(crate) fn start_watch(&self, wake: Arc<dyn Fn() + Send + Sync>) {
        let mut installed = self
            .inner
            .text_scale_watch
            .lock()
            .expect("Windows text-scale watcher mutex");
        if installed.is_some() {
            return;
        }

        let Ok(settings) = UISettings::new() else {
            // Older/unsupported Windows environments keep text_scale absent;
            // the portable provider will reset it to the documented default.
            return;
        };
        let weak: Weak<Inner> = Arc::downgrade(&self.inner);
        let handler = TypedEventHandler::<UISettings, IInspectable>::new(move |_, _| {
            if let Some(inner) = weak.upgrade() {
                inner.settings_dirty.store(true, Ordering::Release);
                wake();
            }
            Ok(())
        });
        let Ok(token) = settings.TextScaleFactorChanged(&handler) else {
            return;
        };
        *installed = Some(TextScaleWatch { settings, token });
    }

    pub(crate) fn message_hook(&self) -> Box<dyn FnMut(*const std::ffi::c_void) -> bool + 'static> {
        let inner = self.inner.clone();
        Box::new(move |message| {
            if message.is_null() {
                return false;
            }
            // SAFETY: Winit documents `with_msg_hook` as passing a valid
            // borrowed Win32 `MSG` pointer for the duration of this callback.
            let message = unsafe { &*message.cast::<MSG>() };
            match message.message {
                WM_SETTINGCHANGE | WM_DEVICECHANGE => {
                    inner.settings_dirty.store(true, Ordering::Release);
                }
                WM_ACTIVATEAPP => {
                    let active = if message.wParam != 0 {
                        ACTIVE_TRUE
                    } else {
                        ACTIVE_FALSE
                    };
                    if inner.active.swap(active, Ordering::AcqRel) != active {
                        inner
                            .lifecycle
                            .lock()
                            .expect("Windows lifecycle queue")
                            .push_back(if active == ACTIVE_TRUE {
                                PlatformLifecycle::Active
                            } else {
                                PlatformLifecycle::Inactive
                            });
                    }
                }
                WM_POWERBROADCAST => match u32::try_from(message.wParam).unwrap_or_default() {
                    PBT_APMSUSPEND => {
                        if !inner.suspended.swap(true, Ordering::AcqRel) {
                            inner
                                .lifecycle
                                .lock()
                                .expect("Windows lifecycle queue")
                                .push_back(PlatformLifecycle::Suspended);
                        }
                    }
                    PBT_APMRESUMEAUTOMATIC
                    | PBT_APMRESUMESUSPEND
                    | PBT_APMRESUMECRITICAL
                    | PBT_APMRESUMESTANDBY
                        if inner.suspended.swap(false, Ordering::AcqRel) =>
                    {
                        inner
                            .lifecycle
                            .lock()
                            .expect("Windows lifecycle queue")
                            .push_back(PlatformLifecycle::Resumed);
                    }
                    _ => {}
                },
                _ => {}
            }
            // Observation only. Winit must continue its normal dispatch.
            false
        })
    }
}

fn query_application_active() -> bool {
    // SAFETY: both Win32 functions are process-global, synchronous queries.
    let foreground = unsafe { GetForegroundWindow() };
    if foreground == 0 {
        return false;
    }
    let mut process_id = 0;
    // SAFETY: `process_id` is writable for the duration of the query and the
    // foreground HWND is borrowed only by value.
    unsafe { GetWindowThreadProcessId(foreground, &mut process_id) };
    process_id == std::process::id()
}

fn query_client_area_animation() -> Option<bool> {
    let mut enabled: i32 = 0;
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL into `enabled` and
    // retains no pointer after the synchronous call.
    (unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            (&mut enabled as *mut i32).cast(),
            0,
        )
    } != 0)
        .then_some(enabled != 0)
}

fn query_text_scale_factor() -> Option<f32> {
    let scale = UISettings::new().ok()?.TextScaleFactor().ok()?;
    // Windows documents TextScaleFactor as 1.0 through 2.25. Reject malformed
    // native values rather than clamping them into a framework guess.
    if !scale.is_finite() || !(1.0..=2.25).contains(&scale) {
        return None;
    }
    Some(scale as f32)
}

fn query_high_contrast() -> Option<bool> {
    let mut value = HIGHCONTRASTW {
        cbSize: u32::try_from(std::mem::size_of::<HIGHCONTRASTW>()).ok()?,
        dwFlags: 0,
        lpszDefaultScheme: std::ptr::null_mut(),
    };
    // SAFETY: SPI_GETHIGHCONTRAST writes the correctly-sized stack value and
    // retains no pointer after the synchronous call.
    (unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            value.cbSize,
            (&mut value as *mut HIGHCONTRASTW).cast(),
            0,
        )
    } != 0)
        .then_some(value.dwFlags & HCF_HIGHCONTRASTON != 0)
}

fn query_input_capabilities() -> InputCapabilities {
    // SAFETY: GetSystemMetrics is a synchronous process-global query with no
    // borrowed memory. A zero digitizer mask truthfully means no reported
    // touch/pen digitizer; keyboard capability becomes true after a real Winit
    // keyboard event because Win32 has no corresponding presence metric.
    let mouse = unsafe { GetSystemMetrics(SM_MOUSEPRESENT) } != 0;
    let digitizer = unsafe { GetSystemMetrics(SM_DIGITIZER) } as u32;
    InputCapabilities {
        mouse,
        touch: digitizer & (NID_INTEGRATED_TOUCH | NID_EXTERNAL_TOUCH) != 0,
        keyboard: false,
        stylus: digitizer & (NID_INTEGRATED_PEN | NID_EXTERNAL_PEN) != 0,
    }
}
