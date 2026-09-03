//! Win32 application-shell integration.

use incular_platform::{
    ApplicationShellError, ApplicationShellFeature, CapabilitySupport, NativeWindowSystem,
    PlatformCapabilities, TrayItemId, WindowIcon,
};
use incular_runtime::{
    NativeApplicationShellEvent, NativeApplicationShellOperation, NativeApplicationShellRequest,
};
use incular_widgets::{
    MenuItemId, PlatformMenuEvent, PlatformMenuSnapshot, PlatformMenuSnapshotNode,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

type EventSink = Arc<dyn Fn(NativeApplicationShellEvent) + Send + Sync>;
type InstalledSink = (u64, EventSink);

static EVENT_HANDLER_INSTALLED: OnceLock<()> = OnceLock::new();
static EVENT_SINK: OnceLock<Mutex<Option<InstalledSink>>> = OnceLock::new();
static NEXT_SINK_GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Default)]
pub(crate) struct WindowsApplicationShell {
    inner: Rc<WindowsApplicationShellInner>,
}

#[derive(Default)]
struct WindowsApplicationShellInner {
    trays: RefCell<HashMap<TrayItemId, tray_icon::TrayIcon>>,
    notification_identity: RefCell<Option<String>>,
    sink_generation: Cell<Option<u64>>,
}

impl Drop for WindowsApplicationShellInner {
    fn drop(&mut self) {
        self.trays.get_mut().clear();
        let Some(generation) = self.sink_generation.get() else {
            return;
        };
        let Some(sink) = EVENT_SINK.get() else {
            return;
        };
        let mut sink = sink
            .lock()
            .expect("Windows application shell event sink lock");
        if sink
            .as_ref()
            .is_some_and(|(installed, _)| *installed == generation)
        {
            sink.take();
        }
    }
}

impl WindowsApplicationShell {
    pub(crate) fn refine_capabilities(
        &self,
        system: NativeWindowSystem,
        capabilities: &mut PlatformCapabilities,
    ) {
        let supported = system == NativeWindowSystem::Win32;
        let support = CapabilitySupport::from_supported(supported);
        let unsupported = CapabilitySupport::Unsupported;
        let services = &mut capabilities.application_services;
        services.tray_or_status_item = support;
        services.notifications = support;
        services.notification_actions = support;
        services.notification_update = unsupported;
        services.notification_dismiss = unsupported;
        services.taskbar_progress = support;
        services.application_badge = unsupported;
        services.taskbar_overlay_icon = support;
    }

    pub(crate) fn start_watch(&self, sink: EventSink) {
        EVENT_HANDLER_INSTALLED.get_or_init(|| {
            tray_icon::menu::MenuEvent::set_event_handler(Some(
                |event: tray_icon::menu::MenuEvent| {
                    let Some((tray, item)) = decode_menu_id(event.id.0.as_str()) else {
                        return;
                    };
                    emit_event(NativeApplicationShellEvent::TrayMenu {
                        id: tray,
                        event: PlatformMenuEvent::Selected(MenuItemId::new(item)),
                    });
                },
            ));
            tray_icon::TrayIconEvent::set_event_handler(Some(|event: tray_icon::TrayIconEvent| {
                let tray_icon::TrayIconEvent::Click {
                    id,
                    button: tray_icon::MouseButton::Left,
                    button_state: tray_icon::MouseButtonState::Up,
                    ..
                } = event
                else {
                    return;
                };
                let Some(tray) = decode_tray_id(id.as_ref()) else {
                    return;
                };
                emit_event(NativeApplicationShellEvent::TrayActivated { id: tray });
            }));
        });

        let generation = NEXT_SINK_GENERATION.fetch_add(1, Ordering::Relaxed);
        *EVENT_SINK
            .get_or_init(|| Mutex::new(None))
            .lock()
            .expect("Windows application shell event sink lock") = Some((generation, sink));
        self.inner.sink_generation.set(Some(generation));
    }

    pub(crate) fn set_notification_identity(&self, identity: Option<String>) {
        *self.inner.notification_identity.borrow_mut() = identity;
    }

    pub(crate) fn apply(
        &self,
        system: NativeWindowSystem,
        request: NativeApplicationShellRequest,
        target_window: Option<&winit::window::Window>,
    ) -> Result<(), ApplicationShellError> {
        if system != NativeWindowSystem::Win32 {
            return Err(ApplicationShellError::Unsupported(feature_for(
                &request.operation,
            )));
        }
        match request.operation {
            NativeApplicationShellOperation::CreateTray {
                id,
                presentation,
                menu,
            } => {
                let native_menu = build_menu(id, &menu)?;
                let mut builder = tray_icon::TrayIconBuilder::new()
                    .with_id(encode_tray_id(id))
                    .with_menu(Box::new(native_menu));
                if let Some(icon) = presentation.icon.as_ref() {
                    builder = builder.with_icon(native_icon(icon)?);
                }
                if let Some(tooltip) = presentation.tooltip.as_ref() {
                    builder = builder.with_tooltip(tooltip);
                }
                if let Some(title) = presentation.title.as_ref() {
                    builder = builder.with_title(title);
                }
                let tray = builder.build().map_err(native_failure)?;
                tray.set_visible(presentation.visible)
                    .map_err(native_failure)?;
                self.inner.trays.borrow_mut().insert(id, tray);
                Ok(())
            }
            NativeApplicationShellOperation::UpdateTray {
                id,
                presentation,
                menu,
            } => {
                let trays = self.inner.trays.borrow();
                let tray = trays.get(&id).ok_or(ApplicationShellError::StaleResource)?;
                tray.set_menu(Some(Box::new(build_menu(id, &menu)?)));
                tray.set_icon(presentation.icon.as_ref().map(native_icon).transpose()?)
                    .map_err(native_failure)?;
                tray.set_tooltip(presentation.tooltip.as_deref())
                    .map_err(native_failure)?;
                tray.set_title(presentation.title.as_deref());
                tray.set_visible(presentation.visible)
                    .map_err(native_failure)?;
                Ok(())
            }
            NativeApplicationShellOperation::RemoveTray { id } => self
                .inner
                .trays
                .borrow_mut()
                .remove(&id)
                .map(|_| ())
                .ok_or(ApplicationShellError::StaleResource),
            NativeApplicationShellOperation::ShowNotification { id, presentation } => {
                self.show_notification(id, presentation)
            }
            NativeApplicationShellOperation::UpdateNotification { .. } => Err(
                ApplicationShellError::Unsupported(ApplicationShellFeature::NotificationUpdate),
            ),
            NativeApplicationShellOperation::CloseNotification { .. } => Err(
                ApplicationShellError::Unsupported(ApplicationShellFeature::NotificationDismiss),
            ),
            NativeApplicationShellOperation::SetTaskbarDockState(state) => {
                set_taskbar_state(state, target_window)
            }
        }
    }

    fn show_notification(
        &self,
        id: incular_platform::NotificationId,
        presentation: incular_platform::NotificationPresentation,
    ) -> Result<(), ApplicationShellError> {
        let identity = self.inner.notification_identity.borrow();
        let identity = identity.as_deref().ok_or_else(|| {
            ApplicationShellError::PlatformConfigurationRequired(
                "Windows notifications require the installed application's AppUserModelID; call ApplicationShellService::set_notification_identity".to_owned(),
            )
        })?;
        let mut toast = tauri_winrt_notification::Toast::new(identity)
            .title(&presentation.title)
            .text1(&presentation.body);
        for action in &presentation.actions {
            toast = toast.add_button(&action.label, action.id.as_str());
        }
        toast = toast.on_activated(move |action| {
            emit_event(match action {
                Some(action) => NativeApplicationShellEvent::NotificationAction {
                    id,
                    action: incular_platform::NotificationActionId::new(action),
                },
                None => NativeApplicationShellEvent::NotificationActivated { id },
            });
            Ok(())
        });
        toast = toast.on_dismissed(move |_| {
            emit_event(NativeApplicationShellEvent::NotificationDismissed { id });
            Ok(())
        });
        toast.show().map_err(native_failure)
    }
}

fn set_taskbar_state(
    state: incular_platform::TaskbarDockState,
    target_window: Option<&winit::window::Window>,
) -> Result<(), ApplicationShellError> {
    use windows::Win32::{
        Foundation::{HWND, RPC_E_CHANGED_MODE},
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        },
        UI::Shell::{
            ITaskbarList3, TBPF_ERROR, TBPF_INDETERMINATE, TBPF_NOPROGRESS, TBPF_NORMAL,
            TBPF_PAUSED, TaskbarList,
        },
    };
    use winit::raw_window_handle::RawWindowHandle;

    if state.badge != incular_platform::ApplicationBadge::None {
        return Err(ApplicationShellError::Unsupported(
            ApplicationShellFeature::ApplicationBadge,
        ));
    }
    if target_window.is_none()
        && state.progress == incular_platform::TaskbarProgress::None
        && state.overlay_icon.is_none()
    {
        return Ok(());
    }
    let window = target_window.ok_or(ApplicationShellError::StaleResource)?;
    let RawWindowHandle::Win32(handle) = incular_platform::raw_window_handles(window).window else {
        return Err(ApplicationShellError::Unsupported(
            ApplicationShellFeature::TaskbarProgress,
        ));
    };
    let hwnd = HWND(handle.hwnd.get() as *mut std::ffi::c_void);
    // COM initialization is thread-local. Winit does not promise that the
    // application event-loop thread has initialized COM, while taskbar APIs
    // require it. Balance S_OK/S_FALSE with CoUninitialize; if another
    // apartment model already owns the thread, RPC_E_CHANGED_MODE still means
    // COM is initialized and usable, so no uninitialize is owed by Incular.
    let coinit = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let _com = if coinit.is_ok() {
        Some(ComUninitializeGuard)
    } else if coinit == RPC_E_CHANGED_MODE {
        None
    } else {
        return Err(ApplicationShellError::NativeFailure(format!(
            "failed to initialize COM for taskbar integration: {coinit}"
        )));
    };

    // SAFETY: COM is initialized for this event-loop thread and `hwnd` belongs
    // to the live Winit window passed by the shared runner.
    unsafe {
        let taskbar: ITaskbarList3 =
            CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER).map_err(native_failure)?;
        taskbar.HrInit().map_err(native_failure)?;
        let (flag, fraction) = match state.progress {
            incular_platform::TaskbarProgress::None => (TBPF_NOPROGRESS, None),
            incular_platform::TaskbarProgress::Indeterminate => (TBPF_INDETERMINATE, None),
            incular_platform::TaskbarProgress::Normal(value) => (TBPF_NORMAL, Some(value)),
            incular_platform::TaskbarProgress::Paused(value) => (TBPF_PAUSED, Some(value)),
            incular_platform::TaskbarProgress::Error(value) => (TBPF_ERROR, Some(value)),
        };
        taskbar
            .SetProgressState(hwnd, flag)
            .map_err(native_failure)?;
        if let Some(fraction) = fraction {
            let completed = (fraction * 10_000.0).round() as u64;
            taskbar
                .SetProgressValue(hwnd, completed, 10_000)
                .map_err(native_failure)?;
        }
        if let Some(icon) = state.overlay_icon.as_ref() {
            let icon = OwnedTaskbarIcon::new(icon)?;
            taskbar
                .SetOverlayIcon(hwnd, icon.handle, windows::core::PCWSTR::null())
                .map_err(native_failure)?;
        } else {
            taskbar
                .SetOverlayIcon(
                    hwnd,
                    windows::Win32::UI::WindowsAndMessaging::HICON::default(),
                    windows::core::PCWSTR::null(),
                )
                .map_err(native_failure)?;
        }
    }
    Ok(())
}

struct ComUninitializeGuard;

impl Drop for ComUninitializeGuard {
    fn drop(&mut self) {
        // SAFETY: this guard is created only after a successful CoInitializeEx
        // on this event-loop thread and never leaves the synchronous call.
        unsafe { windows::Win32::System::Com::CoUninitialize() };
    }
}

struct OwnedTaskbarIcon {
    handle: windows::Win32::UI::WindowsAndMessaging::HICON,
}

impl OwnedTaskbarIcon {
    fn new(icon: &WindowIcon) -> Result<Self, ApplicationShellError> {
        use windows::Win32::UI::WindowsAndMessaging::CreateIcon;

        let mut bgra = icon.rgba().to_vec();
        let (pixels, remainder) = bgra.as_chunks_mut::<4>();
        debug_assert!(remainder.is_empty());
        for pixel in pixels {
            pixel.swap(0, 2);
        }
        let pixel_count = usize::try_from(icon.width())
            .ok()
            .and_then(|width| {
                usize::try_from(icon.height())
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .ok_or_else(|| {
                ApplicationShellError::NativeFailure(
                    "taskbar overlay icon dimensions overflow".to_owned(),
                )
            })?;
        // Match the proven 32-bpp Win32 icon path used by tray-icon/winit: the
        // monochrome mask inverts alpha while the XOR plane carries BGRA.
        let (rgba_pixels, remainder) = icon.rgba().as_chunks::<4>();
        debug_assert!(remainder.is_empty());
        let and_mask = rgba_pixels
            .iter()
            .map(|pixel| pixel[3].wrapping_sub(u8::MAX))
            .collect::<Vec<_>>();
        debug_assert_eq!(and_mask.len(), pixel_count);
        let width = i32::try_from(icon.width()).map_err(|_| {
            ApplicationShellError::NativeFailure("taskbar overlay icon is too wide".to_owned())
        })?;
        let height = i32::try_from(icon.height()).map_err(|_| {
            ApplicationShellError::NativeFailure("taskbar overlay icon is too tall".to_owned())
        })?;
        let handle =
            unsafe { CreateIcon(None, width, height, 1, 32, and_mask.as_ptr(), bgra.as_ptr()) }
                .map_err(native_failure)?;
        Ok(Self { handle })
    }
}

impl Drop for OwnedTaskbarIcon {
    fn drop(&mut self) {
        // SAFETY: handle was created by CreateIcon and this RAII owner is the
        // unique object responsible for releasing it after SetOverlayIcon has
        // copied the icon into the taskbar.
        let _ = unsafe { windows::Win32::UI::WindowsAndMessaging::DestroyIcon(self.handle) };
    }
}

fn feature_for(operation: &NativeApplicationShellOperation) -> ApplicationShellFeature {
    match operation {
        NativeApplicationShellOperation::CreateTray { .. }
        | NativeApplicationShellOperation::UpdateTray { .. }
        | NativeApplicationShellOperation::RemoveTray { .. } => {
            ApplicationShellFeature::TrayOrStatusItem
        }
        NativeApplicationShellOperation::ShowNotification { .. } => {
            ApplicationShellFeature::Notifications
        }
        NativeApplicationShellOperation::UpdateNotification { .. } => {
            ApplicationShellFeature::NotificationUpdate
        }
        NativeApplicationShellOperation::CloseNotification { .. } => {
            ApplicationShellFeature::NotificationDismiss
        }
        NativeApplicationShellOperation::SetTaskbarDockState(state) => shell_state_feature(state),
    }
}

fn shell_state_feature(state: &incular_platform::TaskbarDockState) -> ApplicationShellFeature {
    if state.overlay_icon.is_some() {
        ApplicationShellFeature::TaskbarOverlayIcon
    } else if state.badge != incular_platform::ApplicationBadge::None {
        ApplicationShellFeature::ApplicationBadge
    } else {
        ApplicationShellFeature::TaskbarProgress
    }
}

fn emit_event(event: NativeApplicationShellEvent) {
    let sink = EVENT_SINK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("Windows application shell event sink lock")
        .as_ref()
        .map(|(_, sink)| Arc::clone(sink));
    if let Some(sink) = sink {
        sink(event);
    }
}

fn encode_tray_id(id: TrayItemId) -> String {
    format!("incular-tray-icon:{}:{}", id.index(), id.generation())
}

fn decode_tray_id(value: &str) -> Option<TrayItemId> {
    let rest = value.strip_prefix("incular-tray-icon:")?;
    let (index, generation) = rest.split_once(':')?;
    Some(TrayItemId::from_parts(
        index.parse().ok()?,
        generation.parse().ok()?,
    ))
}

fn native_icon(icon: &WindowIcon) -> Result<tray_icon::Icon, ApplicationShellError> {
    tray_icon::Icon::from_rgba(icon.rgba().to_vec(), icon.width(), icon.height())
        .map_err(native_failure)
}

fn build_menu(
    id: TrayItemId,
    snapshot: &PlatformMenuSnapshot,
) -> Result<tray_icon::menu::Menu, ApplicationShellError> {
    let root = tray_icon::menu::Menu::new();
    for node in &snapshot.menus {
        root.append(&build_submenu(id, node)?)
            .map_err(native_failure)?;
    }
    Ok(root)
}

fn build_submenu(
    id: TrayItemId,
    node: &PlatformMenuSnapshotNode,
) -> Result<tray_icon::menu::Submenu, ApplicationShellError> {
    let submenu = tray_icon::menu::Submenu::with_id(
        encode_menu_id(id, node.id.as_str()),
        &node.label,
        node.enabled,
    );
    for child in &node.children {
        if child.separator {
            submenu
                .append(&tray_icon::menu::PredefinedMenuItem::separator())
                .map_err(native_failure)?;
        } else if child.children.is_empty() {
            let item = tray_icon::menu::MenuItem::with_id(
                encode_menu_id(id, child.id.as_str()),
                &child.label,
                child.enabled,
                None,
            );
            submenu.append(&item).map_err(native_failure)?;
        } else {
            submenu
                .append(&build_submenu(id, child)?)
                .map_err(native_failure)?;
        }
    }
    Ok(submenu)
}

fn encode_menu_id(tray: TrayItemId, item: &str) -> String {
    format!("incular-tray:{}:{}:{item}", tray.index(), tray.generation())
}

fn decode_menu_id(value: &str) -> Option<(TrayItemId, String)> {
    let rest = value.strip_prefix("incular-tray:")?;
    let mut parts = rest.splitn(3, ':');
    let index = parts.next()?.parse().ok()?;
    let generation = parts.next()?.parse().ok()?;
    Some((
        TrayItemId::from_parts(index, generation),
        parts.next()?.to_owned(),
    ))
}

fn native_failure(error: impl std::fmt::Display) -> ApplicationShellError {
    ApplicationShellError::NativeFailure(error.to_string())
}
