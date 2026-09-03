//! Linux application-shell integration through StatusNotifierItem and the
//! freedesktop.org notification service.

use incular_platform::{
    ApplicationShellError, ApplicationShellFeature, CapabilitySupport, NativeWindowSystem,
    PlatformCapabilities, TrayItemId, WindowIcon,
};
use incular_runtime::{
    NativeApplicationShellEvent, NativeApplicationShellOperation, NativeApplicationShellRequest,
};
use incular_widgets::{PlatformMenuEvent, PlatformMenuSnapshot, PlatformMenuSnapshotNode};
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

static EVENT_SINK: OnceLock<Mutex<Option<InstalledSink>>> = OnceLock::new();
static NEXT_SINK_GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Default)]
pub(crate) struct LinuxApplicationShell {
    inner: Rc<LinuxApplicationShellInner>,
}

#[derive(Default)]
struct LinuxApplicationShellInner {
    trays: RefCell<HashMap<TrayItemId, ksni::blocking::Handle<LinuxTray>>>,
    notifications:
        RefCell<HashMap<incular_platform::NotificationId, Arc<notify_rust::NotificationHandle>>>,
    notification_watches: Arc<Mutex<HashMap<incular_platform::NotificationId, u64>>>,
    next_notification_watch: Cell<u64>,
    sink_generation: Cell<Option<u64>>,
}

impl Drop for LinuxApplicationShellInner {
    fn drop(&mut self) {
        self.notification_watches
            .lock()
            .expect("Linux notification watch lock")
            .clear();
        for (_, tray) in self.trays.get_mut().drain() {
            tray.shutdown().wait();
        }
        for (_, notification) in self.notifications.get_mut().drain() {
            pollster::block_on(notification.close_async());
        }
        let Some(generation) = self.sink_generation.get() else {
            return;
        };
        let Some(sink) = EVENT_SINK.get() else {
            return;
        };
        let mut sink = sink
            .lock()
            .expect("Linux application shell event sink lock");
        if sink
            .as_ref()
            .is_some_and(|(installed, _)| *installed == generation)
        {
            sink.take();
        }
    }
}

impl LinuxApplicationShell {
    pub(crate) fn refine_capabilities(
        &self,
        system: NativeWindowSystem,
        capabilities: &mut PlatformCapabilities,
    ) {
        let shell_supported = matches!(
            system,
            NativeWindowSystem::X11 | NativeWindowSystem::Wayland
        );
        let support = CapabilitySupport::from_supported(shell_supported);
        let unsupported = CapabilitySupport::Unsupported;
        let services = &mut capabilities.application_services;
        services.tray_or_status_item = support;
        services.notifications = support;
        services.notification_actions = support;
        services.notification_update = support;
        services.notification_dismiss = support;
        services.taskbar_progress = unsupported;
        services.application_badge = unsupported;
        services.taskbar_overlay_icon = unsupported;
    }

    pub(crate) fn start_watch(&self, sink: EventSink) {
        let generation = NEXT_SINK_GENERATION.fetch_add(1, Ordering::Relaxed);
        *EVENT_SINK
            .get_or_init(|| Mutex::new(None))
            .lock()
            .expect("Linux application shell event sink lock") = Some((generation, sink));
        self.inner.sink_generation.set(Some(generation));
    }

    pub(crate) fn apply(
        &self,
        system: NativeWindowSystem,
        request: NativeApplicationShellRequest,
        _target_window: Option<&winit::window::Window>,
    ) -> Result<(), ApplicationShellError> {
        if !matches!(
            system,
            NativeWindowSystem::X11 | NativeWindowSystem::Wayland
        ) {
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
                use ksni::blocking::TrayMethods;
                let handle = LinuxTray::new(id, presentation, menu)
                    .spawn()
                    .map_err(native_failure)?;
                self.inner.trays.borrow_mut().insert(id, handle);
                Ok(())
            }
            NativeApplicationShellOperation::UpdateTray {
                id,
                presentation,
                menu,
            } => {
                let trays = self.inner.trays.borrow();
                let tray = trays.get(&id).ok_or(ApplicationShellError::StaleResource)?;
                tray.update(|tray| {
                    tray.presentation = presentation;
                    tray.menu = menu;
                })
                .ok_or_else(|| {
                    ApplicationShellError::NativeFailure(
                        "Linux StatusNotifierItem service stopped before update".to_owned(),
                    )
                })?;
                Ok(())
            }
            NativeApplicationShellOperation::RemoveTray { id } => {
                let tray = self
                    .inner
                    .trays
                    .borrow_mut()
                    .remove(&id)
                    .ok_or(ApplicationShellError::StaleResource)?;
                tray.shutdown().wait();
                Ok(())
            }
            NativeApplicationShellOperation::ShowNotification { id, presentation } => {
                let notification = linux_notification(&presentation)?;
                let watch_generation = self.install_notification_watch(id);
                let handle = Arc::new(notification.show().map_err(|error| {
                    self.remove_notification_watch(id, watch_generation);
                    native_failure(error)
                })?);
                if let Err(error) = watch_notification(
                    id,
                    watch_generation,
                    Arc::clone(&self.inner.notification_watches),
                    Arc::clone(&handle),
                ) {
                    self.remove_notification_watch(id, watch_generation);
                    pollster::block_on(handle.close_async());
                    return Err(error);
                }
                self.inner.notifications.borrow_mut().insert(id, handle);
                Ok(())
            }
            NativeApplicationShellOperation::UpdateNotification { id, presentation } => {
                let native_id = {
                    let notifications = self.inner.notifications.borrow();
                    notifications
                        .get(&id)
                        .ok_or(ApplicationShellError::StaleResource)?
                        .id()
                };
                let mut notification = linux_notification(&presentation)?;
                notification.id(native_id);
                let previous_watch = self.current_notification_watch(id);
                let watch_generation = self.install_notification_watch(id);
                let handle = match notification.show() {
                    Ok(handle) => Arc::new(handle),
                    Err(error) => {
                        self.restore_notification_watch(id, watch_generation, previous_watch);
                        return Err(native_failure(error));
                    }
                };
                if let Err(error) = watch_notification(
                    id,
                    watch_generation,
                    Arc::clone(&self.inner.notification_watches),
                    Arc::clone(&handle),
                ) {
                    // The original watcher is still subscribed to the same
                    // freedesktop notification ID. Re-authorize it rather than
                    // leave the successfully replaced native notification with
                    // no event path at all.
                    self.restore_notification_watch(id, watch_generation, previous_watch);
                    self.inner.notifications.borrow_mut().insert(id, handle);
                    return Err(error);
                }
                self.inner.notifications.borrow_mut().insert(id, handle);
                Ok(())
            }
            NativeApplicationShellOperation::CloseNotification { id } => {
                self.clear_notification_watch(id);
                let handle = self
                    .inner
                    .notifications
                    .borrow_mut()
                    .remove(&id)
                    .ok_or(ApplicationShellError::StaleResource)?;
                pollster::block_on(handle.close_async());
                Ok(())
            }
            NativeApplicationShellOperation::SetTaskbarDockState(state) => {
                if state.overlay_icon.is_some() {
                    Err(ApplicationShellError::Unsupported(
                        ApplicationShellFeature::TaskbarOverlayIcon,
                    ))
                } else if state.badge != incular_platform::ApplicationBadge::None {
                    Err(ApplicationShellError::Unsupported(
                        ApplicationShellFeature::ApplicationBadge,
                    ))
                } else {
                    Err(ApplicationShellError::Unsupported(
                        ApplicationShellFeature::TaskbarProgress,
                    ))
                }
            }
        }
    }

    fn install_notification_watch(&self, id: incular_platform::NotificationId) -> u64 {
        let generation = self.inner.next_notification_watch.get();
        self.inner
            .next_notification_watch
            .set(generation.wrapping_add(1));
        self.inner
            .notification_watches
            .lock()
            .expect("Linux notification watch lock")
            .insert(id, generation);
        generation
    }

    fn current_notification_watch(&self, id: incular_platform::NotificationId) -> Option<u64> {
        self.inner
            .notification_watches
            .lock()
            .expect("Linux notification watch lock")
            .get(&id)
            .copied()
    }

    fn restore_notification_watch(
        &self,
        id: incular_platform::NotificationId,
        attempted: u64,
        previous: Option<u64>,
    ) {
        let mut watches = self
            .inner
            .notification_watches
            .lock()
            .expect("Linux notification watch lock");
        if watches.get(&id).copied() != Some(attempted) {
            return;
        }
        match previous {
            Some(previous) => {
                watches.insert(id, previous);
            }
            None => {
                watches.remove(&id);
            }
        }
    }

    fn remove_notification_watch(&self, id: incular_platform::NotificationId, expected: u64) {
        let mut watches = self
            .inner
            .notification_watches
            .lock()
            .expect("Linux notification watch lock");
        if watches.get(&id).copied() == Some(expected) {
            watches.remove(&id);
        }
    }

    fn clear_notification_watch(&self, id: incular_platform::NotificationId) {
        self.inner
            .notification_watches
            .lock()
            .expect("Linux notification watch lock")
            .remove(&id);
    }
}

fn linux_notification(
    presentation: &incular_platform::NotificationPresentation,
) -> Result<notify_rust::Notification, ApplicationShellError> {
    let mut notification = notify_rust::Notification::new();
    notification
        .summary(&presentation.title)
        .body(&presentation.body);
    if let Some(icon) = presentation.icon.as_ref() {
        let image = notify_rust::Image::from_rgba(
            i32::try_from(icon.width()).map_err(native_failure)?,
            i32::try_from(icon.height()).map_err(native_failure)?,
            icon.rgba().to_vec(),
        )
        .map_err(native_failure)?;
        notification.image_data(image);
    }
    for action in &presentation.actions {
        notification.action(action.id.as_str(), &action.label);
    }
    Ok(notification)
}

fn watch_notification(
    id: incular_platform::NotificationId,
    generation: u64,
    watches: Arc<Mutex<HashMap<incular_platform::NotificationId, u64>>>,
    handle: Arc<notify_rust::NotificationHandle>,
) -> Result<(), ApplicationShellError> {
    std::thread::Builder::new()
        .name("incular-notification".into())
        .spawn(move || {
            pollster::block_on(handle.wait_for_action_async(move |response| {
                let current = {
                    let mut watches = watches.lock().expect("Linux notification watch lock");
                    if watches.get(&id).copied() != Some(generation) {
                        false
                    } else {
                        watches.remove(&id);
                        true
                    }
                };
                if !current {
                    return;
                }
                let event = match response {
                    notify_rust::NotificationResponse::Default => {
                        NativeApplicationShellEvent::NotificationActivated { id }
                    }
                    notify_rust::NotificationResponse::Action(action) => {
                        NativeApplicationShellEvent::NotificationAction {
                            id,
                            action: incular_platform::NotificationActionId::new(action),
                        }
                    }
                    notify_rust::NotificationResponse::Closed(_) => {
                        NativeApplicationShellEvent::NotificationDismissed { id }
                    }
                    notify_rust::NotificationResponse::Reply(_) => return,
                };
                emit_event(event);
            }));
        })
        .map(|_| ())
        .map_err(native_failure)
}

struct LinuxTray {
    id: TrayItemId,
    presentation: incular_platform::TrayItemPresentation,
    menu: PlatformMenuSnapshot,
}

impl LinuxTray {
    fn new(
        id: TrayItemId,
        presentation: incular_platform::TrayItemPresentation,
        menu: PlatformMenuSnapshot,
    ) -> Self {
        Self {
            id,
            presentation,
            menu,
        }
    }
}

impl ksni::Tray for LinuxTray {
    fn id(&self) -> String {
        format!("incular-{}-{}", self.id.index(), self.id.generation())
    }

    fn title(&self) -> String {
        self.presentation
            .title
            .clone()
            .unwrap_or_else(|| "Incular".to_owned())
    }

    fn status(&self) -> ksni::Status {
        if self.presentation.visible {
            ksni::Status::Active
        } else {
            ksni::Status::Passive
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        emit_event(NativeApplicationShellEvent::TrayActivated { id: self.id });
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.presentation
            .icon
            .as_ref()
            .map_or_else(Vec::new, |icon| vec![linux_icon(icon)])
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: self.presentation.title.clone().unwrap_or_default(),
            description: self.presentation.tooltip.clone().unwrap_or_default(),
            icon_pixmap: self.icon_pixmap(),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        self.menu
            .menus
            .iter()
            .map(|node| linux_menu_node(self.id, node))
            .collect()
    }
}

fn linux_icon(icon: &WindowIcon) -> ksni::Icon {
    let mut data = Vec::with_capacity(icon.rgba().len());
    let (pixels, remainder) = icon.rgba().as_chunks::<4>();
    debug_assert!(remainder.is_empty());
    for pixel in pixels {
        data.extend_from_slice(&[pixel[3], pixel[0], pixel[1], pixel[2]]);
    }
    ksni::Icon {
        width: icon.width() as i32,
        height: icon.height() as i32,
        data,
    }
}

fn linux_menu_node(tray: TrayItemId, node: &PlatformMenuSnapshotNode) -> ksni::MenuItem<LinuxTray> {
    if node.separator {
        return ksni::MenuItem::Separator;
    }
    if !node.children.is_empty() {
        return ksni::menu::SubMenu {
            label: node.label.clone(),
            enabled: node.enabled,
            submenu: node
                .children
                .iter()
                .map(|child| linux_menu_node(tray, child))
                .collect(),
            ..Default::default()
        }
        .into();
    }
    let item_id = node.id.clone();
    ksni::menu::StandardItem {
        label: node.label.clone(),
        enabled: node.enabled,
        activate: Box::new(move |_| {
            emit_event(NativeApplicationShellEvent::TrayMenu {
                id: tray,
                event: PlatformMenuEvent::Selected(item_id.clone()),
            });
        }),
        ..Default::default()
    }
    .into()
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
        .expect("Linux application shell event sink lock")
        .as_ref()
        .map(|(_, sink)| Arc::clone(sink));
    if let Some(sink) = sink {
        sink(event);
    }
}

fn native_failure(error: impl std::fmt::Display) -> ApplicationShellError {
    ApplicationShellError::NativeFailure(error.to_string())
}
