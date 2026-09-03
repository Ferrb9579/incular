//! Runtime ownership for application-scoped desktop shell resources.

use crate::tasks::RuntimeWake;
use incular_platform::{
    ApplicationShellError, ApplicationShellFeature, CapabilitySupport, NotificationActionId,
    NotificationId, NotificationPresentation, PlatformCapabilities, TaskbarDockState, TrayItemId,
    TrayItemPresentation,
};
use incular_widgets::internal::PlatformMenuCommandModel;
use incular_widgets::{
    PlatformMenu, PlatformMenuBuildError, PlatformMenuEvent, PlatformMenuSnapshot,
};
use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    sync::{Arc, Mutex, RwLock},
};

#[derive(Clone, Debug)]
pub enum NativeApplicationShellOperation {
    CreateTray {
        id: TrayItemId,
        presentation: TrayItemPresentation,
        menu: PlatformMenuSnapshot,
    },
    UpdateTray {
        id: TrayItemId,
        presentation: TrayItemPresentation,
        menu: PlatformMenuSnapshot,
    },
    RemoveTray {
        id: TrayItemId,
    },
    ShowNotification {
        id: NotificationId,
        presentation: NotificationPresentation,
    },
    UpdateNotification {
        id: NotificationId,
        presentation: NotificationPresentation,
    },
    CloseNotification {
        id: NotificationId,
    },
    SetTaskbarDockState(TaskbarDockState),
}

#[derive(Clone, Debug)]
pub struct NativeApplicationShellRequest {
    pub request_id: ApplicationShellRequestId,
    pub operation: NativeApplicationShellOperation,
}

#[derive(Clone, Debug)]
pub struct NativeApplicationShellCompletion {
    pub request_id: ApplicationShellRequestId,
    pub result: Result<(), ApplicationShellError>,
}

/// Whether a native shell request finished during the current event-loop turn
/// or will report its result asynchronously through the shell completion sink.
///
/// UserNotifications on macOS is genuinely asynchronous (including permission
/// authorization), so forcing every backend through a synchronous `Result`
/// would either block AppKit or lie about completion. Backends which complete
/// synchronously use `Completed`; `Deferred` means exactly one later
/// [`NativeApplicationShellCompletion`] is required.
#[derive(Clone, Debug)]
pub enum NativeApplicationShellApplyResult {
    Completed(Result<(), ApplicationShellError>),
    Deferred,
}

impl From<Result<(), ApplicationShellError>> for NativeApplicationShellApplyResult {
    fn from(result: Result<(), ApplicationShellError>) -> Self {
        Self::Completed(result)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ApplicationShellRequestId(u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeApplicationShellEvent {
    TrayMenu {
        id: TrayItemId,
        event: PlatformMenuEvent,
    },
    TrayActivated {
        id: TrayItemId,
    },
    NotificationActivated {
        id: NotificationId,
    },
    NotificationAction {
        id: NotificationId,
        action: NotificationActionId,
    },
    NotificationDismissed {
        id: NotificationId,
    },
}

struct Slot<T> {
    generation: u32,
    value: Option<T>,
}

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Self {
            generation: 0,
            value: None,
        }
    }
}

struct TrayRecord {
    presentation: TrayItemPresentation,
    menu: PlatformMenuCommandModel,
    on_activated: Option<Arc<dyn Fn() + Send + Sync>>,
}

struct NotificationRecord {
    presentation: NotificationPresentation,
    on_activated: Option<Arc<dyn Fn() + Send + Sync>>,
    on_action: Option<Arc<dyn Fn(NotificationActionId) + Send + Sync>>,
    on_dismissed: Option<Arc<dyn Fn() + Send + Sync>>,
}

#[derive(Default)]
struct ShellState {
    trays: Vec<Slot<TrayRecord>>,
    notifications: Vec<Slot<NotificationRecord>>,
    requests: VecDeque<NativeApplicationShellRequest>,
    completions: VecDeque<NativeApplicationShellCompletion>,
    next_request: u64,
    stopped: bool,
    notification_identity: Option<String>,
}

#[derive(Clone)]
pub struct ApplicationShellService {
    state: Rc<RefCell<ShellState>>,
    capabilities: Arc<RwLock<PlatformCapabilities>>,
    wake: Rc<RefCell<Option<Arc<dyn RuntimeWake>>>>,
}

impl ApplicationShellService {
    pub(crate) fn new(capabilities: Arc<RwLock<PlatformCapabilities>>) -> Self {
        Self {
            state: Rc::new(RefCell::new(ShellState::default())),
            capabilities,
            wake: Rc::new(RefCell::new(None)),
        }
    }

    pub(crate) fn set_wake(&self, wake: Arc<dyn RuntimeWake>) {
        *self.wake.borrow_mut() = Some(wake);
    }

    fn wake(&self) {
        if let Some(wake) = self.wake.borrow().as_ref() {
            wake.wake();
        }
    }

    #[doc(hidden)]
    #[must_use]
    pub fn new_for_test(capabilities: Arc<RwLock<PlatformCapabilities>>) -> Self {
        Self::new(capabilities)
    }

    #[must_use]
    pub fn capabilities(&self) -> incular_platform::ApplicationServiceCapabilities {
        self.capabilities
            .read()
            .expect("platform capabilities lock")
            .application_services
    }

    pub fn create_tray_item(
        &self,
        presentation: TrayItemPresentation,
        menus: &[PlatformMenu],
    ) -> Result<TrayItemHandle, ApplicationShellError> {
        self.require_support(
            self.capabilities().tray_or_status_item,
            ApplicationShellFeature::TrayOrStatusItem,
        )?;
        let menu = PlatformMenuCommandModel::compile(menus).map_err(menu_build_error)?;
        let mut state = self.state.borrow_mut();
        ensure_running(&state)?;
        let id = reserve_tray(
            &mut state.trays,
            TrayRecord {
                presentation: presentation.clone(),
                menu: menu.clone(),
                on_activated: None,
            },
        );
        enqueue(
            &mut state,
            NativeApplicationShellOperation::CreateTray {
                id,
                presentation,
                menu: menu.snapshot().clone(),
            },
        );
        self.wake();
        Ok(TrayItemHandle {
            id,
            service: self.clone(),
            released: false,
        })
    }

    pub fn show_notification(
        &self,
        presentation: NotificationPresentation,
    ) -> Result<NotificationHandle, ApplicationShellError> {
        let capabilities = self.capabilities();
        self.require_support(
            capabilities.notifications,
            ApplicationShellFeature::Notifications,
        )?;
        if !presentation.actions.is_empty() {
            self.require_support(
                capabilities.notification_actions,
                ApplicationShellFeature::NotificationActions,
            )?;
        }
        let mut state = self.state.borrow_mut();
        ensure_running(&state)?;
        let id = reserve_notification(
            &mut state.notifications,
            NotificationRecord {
                presentation: presentation.clone(),
                on_activated: None,
                on_action: None,
                on_dismissed: None,
            },
        );
        enqueue(
            &mut state,
            NativeApplicationShellOperation::ShowNotification { id, presentation },
        );
        self.wake();
        Ok(NotificationHandle {
            id,
            service: self.clone(),
            released: false,
        })
    }

    /// Sets the installed desktop identity used by notification systems that
    /// require one (for example a Windows AppUserModelID). Incular never uses a
    /// misleading shell identity as a fallback.
    pub fn set_notification_identity(
        &self,
        identity: impl Into<String>,
    ) -> Result<(), ApplicationShellError> {
        let identity = identity.into();
        if identity.trim().is_empty() || identity.chars().any(char::is_control) {
            return Err(ApplicationShellError::PlatformConfigurationRequired(
                "notification identity must be a non-empty printable installed application identity".to_owned(),
            ));
        }
        let mut state = self.state.borrow_mut();
        ensure_running(&state)?;
        state.notification_identity = Some(identity);
        Ok(())
    }

    #[doc(hidden)]
    #[must_use]
    pub fn notification_identity(&self) -> Option<String> {
        self.state.borrow().notification_identity.clone()
    }

    pub fn set_taskbar_dock_state(
        &self,
        state_value: TaskbarDockState,
    ) -> Result<(), ApplicationShellError> {
        let capabilities = self.capabilities();
        if state_value.progress != incular_platform::TaskbarProgress::None {
            self.require_support(
                capabilities.taskbar_progress,
                ApplicationShellFeature::TaskbarProgress,
            )?;
        }
        if state_value.badge != incular_platform::ApplicationBadge::None {
            self.require_support(
                capabilities.application_badge,
                ApplicationShellFeature::ApplicationBadge,
            )?;
        }
        if state_value.overlay_icon.is_some() {
            self.require_support(
                capabilities.taskbar_overlay_icon,
                ApplicationShellFeature::TaskbarOverlayIcon,
            )?;
        }
        let mut state = self.state.borrow_mut();
        ensure_running(&state)?;
        enqueue(
            &mut state,
            NativeApplicationShellOperation::SetTaskbarDockState(state_value),
        );
        self.wake();
        Ok(())
    }

    fn require_support(
        &self,
        support: CapabilitySupport,
        feature: ApplicationShellFeature,
    ) -> Result<(), ApplicationShellError> {
        if support == CapabilitySupport::Unsupported {
            Err(ApplicationShellError::Unsupported(feature))
        } else {
            Ok(())
        }
    }

    pub(crate) fn take_native_requests(&self) -> Vec<NativeApplicationShellRequest> {
        self.state.borrow_mut().requests.drain(..).collect()
    }

    #[doc(hidden)]
    pub fn take_native_requests_for_test(&self) -> Vec<NativeApplicationShellRequest> {
        self.take_native_requests()
    }

    pub(crate) fn complete(&self, completion: NativeApplicationShellCompletion) {
        self.state.borrow_mut().completions.push_back(completion);
    }

    #[doc(hidden)]
    pub fn complete_for_test(&self, completion: NativeApplicationShellCompletion) {
        self.complete(completion);
    }

    #[must_use]
    pub fn take_completions(&self) -> Vec<NativeApplicationShellCompletion> {
        self.state.borrow_mut().completions.drain(..).collect()
    }

    pub(crate) fn handle_native_event(&self, event: NativeApplicationShellEvent) -> bool {
        match event {
            NativeApplicationShellEvent::TrayMenu { id, event } => {
                let callback = {
                    let state = self.state.borrow_mut();
                    tray_record(&state.trays, id).map(|record| record.menu.clone())
                };
                callback.is_some_and(|menu| {
                    matches!(
                        menu.dispatch(&event),
                        incular_widgets::MenuDispatchResult::Handled
                    )
                })
            }
            NativeApplicationShellEvent::TrayActivated { id } => {
                let callback = {
                    let state = self.state.borrow();
                    tray_record(&state.trays, id).and_then(|record| record.on_activated.clone())
                };
                callback.is_some_and(|callback| {
                    callback();
                    true
                })
            }
            NativeApplicationShellEvent::NotificationActivated { id } => {
                notification_callback(&self.state, id, |record| record.on_activated.clone())
            }
            NativeApplicationShellEvent::NotificationAction { id, action } => {
                let callback = {
                    let state = self.state.borrow_mut();
                    notification_record(&state.notifications, id).and_then(|record| {
                        record
                            .presentation
                            .actions
                            .iter()
                            .any(|known| known.id == action)
                            .then(|| record.on_action.clone())
                            .flatten()
                    })
                };
                callback.is_some_and(|callback| {
                    callback(action);
                    true
                })
            }
            NativeApplicationShellEvent::NotificationDismissed { id } => {
                let handled =
                    notification_callback(&self.state, id, |record| record.on_dismissed.clone());
                let dismiss_native =
                    self.capabilities().notification_dismiss != CapabilitySupport::Unsupported;
                let mut state = self.state.borrow_mut();
                let released = release_notification(&mut state.notifications, id);
                if released && dismiss_native {
                    enqueue(
                        &mut state,
                        NativeApplicationShellOperation::CloseNotification { id },
                    );
                }
                drop(state);
                if released && dismiss_native {
                    self.wake();
                }
                handled || released
            }
        }
    }

    #[doc(hidden)]
    pub fn handle_native_event_for_test(&self, event: NativeApplicationShellEvent) -> bool {
        self.handle_native_event(event)
    }

    pub(crate) fn stop(&self) {
        let mut state = self.state.borrow_mut();
        state.stopped = true;
        state.requests.clear();
        for slot in &mut state.trays {
            slot.value = None;
            slot.generation = slot.generation.wrapping_add(1);
        }
        for slot in &mut state.notifications {
            slot.value = None;
            slot.generation = slot.generation.wrapping_add(1);
        }
    }
}

fn notification_callback<T>(
    state: &Rc<RefCell<ShellState>>,
    id: NotificationId,
    select: impl FnOnce(&NotificationRecord) -> Option<Arc<T>>,
) -> bool
where
    T: Fn() + Send + Sync + ?Sized,
{
    let callback = {
        let state = state.borrow();
        notification_record(&state.notifications, id).and_then(select)
    };
    callback.is_some_and(|callback| {
        callback();
        true
    })
}

fn menu_build_error(error: PlatformMenuBuildError) -> ApplicationShellError {
    ApplicationShellError::NativeFailure(error.to_string())
}

fn ensure_running(state: &ShellState) -> Result<(), ApplicationShellError> {
    (!state.stopped)
        .then_some(())
        .ok_or(ApplicationShellError::ApplicationStopped)
}

fn enqueue(state: &mut ShellState, operation: NativeApplicationShellOperation) {
    let request_id = ApplicationShellRequestId(state.next_request);
    state.next_request = state.next_request.wrapping_add(1);
    state.requests.push_back(NativeApplicationShellRequest {
        request_id,
        operation,
    });
}

fn reserve_tray(slots: &mut Vec<Slot<TrayRecord>>, value: TrayRecord) -> TrayItemId {
    if let Some((index, slot)) = slots
        .iter_mut()
        .enumerate()
        .find(|(_, slot)| slot.value.is_none())
    {
        slot.value = Some(value);
        return TrayItemId::from_parts(index as u32, slot.generation);
    }
    let index = slots.len() as u32;
    slots.push(Slot {
        generation: 0,
        value: Some(value),
    });
    TrayItemId::from_parts(index, 0)
}

fn reserve_notification(
    slots: &mut Vec<Slot<NotificationRecord>>,
    value: NotificationRecord,
) -> NotificationId {
    if let Some((index, slot)) = slots
        .iter_mut()
        .enumerate()
        .find(|(_, slot)| slot.value.is_none())
    {
        slot.value = Some(value);
        return NotificationId::from_parts(index as u32, slot.generation);
    }
    let index = slots.len() as u32;
    slots.push(Slot {
        generation: 0,
        value: Some(value),
    });
    NotificationId::from_parts(index, 0)
}

fn tray_record(slots: &[Slot<TrayRecord>], id: TrayItemId) -> Option<&TrayRecord> {
    slots
        .get(id.index() as usize)
        .filter(|slot| slot.generation == id.generation())?
        .value
        .as_ref()
}

fn tray_record_mut(slots: &mut [Slot<TrayRecord>], id: TrayItemId) -> Option<&mut TrayRecord> {
    slots
        .get_mut(id.index() as usize)
        .filter(|slot| slot.generation == id.generation())?
        .value
        .as_mut()
}

fn notification_record(
    slots: &[Slot<NotificationRecord>],
    id: NotificationId,
) -> Option<&NotificationRecord> {
    slots
        .get(id.index() as usize)
        .filter(|slot| slot.generation == id.generation())?
        .value
        .as_ref()
}

fn notification_record_mut(
    slots: &mut [Slot<NotificationRecord>],
    id: NotificationId,
) -> Option<&mut NotificationRecord> {
    slots
        .get_mut(id.index() as usize)
        .filter(|slot| slot.generation == id.generation())?
        .value
        .as_mut()
}

fn release_notification(slots: &mut [Slot<NotificationRecord>], id: NotificationId) -> bool {
    let Some(slot) = slots
        .get_mut(id.index() as usize)
        .filter(|slot| slot.generation == id.generation())
    else {
        return false;
    };
    if slot.value.take().is_none() {
        return false;
    }
    slot.generation = slot.generation.wrapping_add(1);
    true
}

pub struct TrayItemHandle {
    id: TrayItemId,
    service: ApplicationShellService,
    released: bool,
}

impl TrayItemHandle {
    #[must_use]
    pub const fn id(&self) -> TrayItemId {
        self.id
    }

    pub fn update(
        &self,
        presentation: TrayItemPresentation,
        menus: &[PlatformMenu],
    ) -> Result<(), ApplicationShellError> {
        self.service.require_support(
            self.service.capabilities().tray_or_status_item,
            ApplicationShellFeature::TrayOrStatusItem,
        )?;
        let menu = PlatformMenuCommandModel::compile(menus).map_err(menu_build_error)?;
        let mut state = self.service.state.borrow_mut();
        ensure_running(&state)?;
        let record = tray_record_mut(&mut state.trays, self.id)
            .ok_or(ApplicationShellError::StaleResource)?;
        record.presentation = presentation.clone();
        record.menu = menu.clone();
        enqueue(
            &mut state,
            NativeApplicationShellOperation::UpdateTray {
                id: self.id,
                presentation,
                menu: menu.snapshot().clone(),
            },
        );
        drop(state);
        self.service.wake();
        Ok(())
    }

    pub fn on_activated(
        &self,
        callback: impl Fn() + Send + Sync + 'static,
    ) -> Result<(), ApplicationShellError> {
        let mut state = self.service.state.borrow_mut();
        ensure_running(&state)?;
        let record = tray_record_mut(&mut state.trays, self.id)
            .ok_or(ApplicationShellError::StaleResource)?;
        record.on_activated = Some(Arc::new(callback));
        Ok(())
    }

    pub fn remove(mut self) -> Result<(), ApplicationShellError> {
        self.service.remove_tray(self.id)?;
        self.released = true;
        Ok(())
    }
}

impl Drop for TrayItemHandle {
    fn drop(&mut self) {
        if !self.released {
            let _ = self.service.remove_tray(self.id);
        }
    }
}

impl ApplicationShellService {
    fn remove_tray(&self, id: TrayItemId) -> Result<(), ApplicationShellError> {
        let mut state = self.state.borrow_mut();
        ensure_running(&state)?;
        let slot = state
            .trays
            .get_mut(id.index() as usize)
            .filter(|slot| slot.generation == id.generation())
            .ok_or(ApplicationShellError::StaleResource)?;
        slot.value
            .take()
            .ok_or(ApplicationShellError::StaleResource)?;
        slot.generation = slot.generation.wrapping_add(1);
        enqueue(
            &mut state,
            NativeApplicationShellOperation::RemoveTray { id },
        );
        drop(state);
        self.wake();
        Ok(())
    }
}

pub struct NotificationHandle {
    id: NotificationId,
    service: ApplicationShellService,
    released: bool,
}

impl NotificationHandle {
    #[must_use]
    pub const fn id(&self) -> NotificationId {
        self.id
    }

    pub fn on_activated(
        &self,
        callback: impl Fn() + Send + Sync + 'static,
    ) -> Result<(), ApplicationShellError> {
        self.with_record(|record| record.on_activated = Some(Arc::new(callback)))
    }

    pub fn on_action(
        &self,
        callback: impl Fn(NotificationActionId) + Send + Sync + 'static,
    ) -> Result<(), ApplicationShellError> {
        self.with_record(|record| record.on_action = Some(Arc::new(callback)))
    }

    pub fn on_dismissed(
        &self,
        callback: impl Fn() + Send + Sync + 'static,
    ) -> Result<(), ApplicationShellError> {
        self.with_record(|record| record.on_dismissed = Some(Arc::new(callback)))
    }

    pub fn update(
        &self,
        presentation: NotificationPresentation,
    ) -> Result<(), ApplicationShellError> {
        self.service.require_support(
            self.service.capabilities().notification_update,
            ApplicationShellFeature::NotificationUpdate,
        )?;
        if !presentation.actions.is_empty() {
            self.service.require_support(
                self.service.capabilities().notification_actions,
                ApplicationShellFeature::NotificationActions,
            )?;
        }
        let mut state = self.service.state.borrow_mut();
        ensure_running(&state)?;
        let record = notification_record_mut(&mut state.notifications, self.id)
            .ok_or(ApplicationShellError::StaleResource)?;
        record.presentation = presentation.clone();
        enqueue(
            &mut state,
            NativeApplicationShellOperation::UpdateNotification {
                id: self.id,
                presentation,
            },
        );
        drop(state);
        self.service.wake();
        Ok(())
    }

    fn with_record(
        &self,
        update: impl FnOnce(&mut NotificationRecord),
    ) -> Result<(), ApplicationShellError> {
        let mut state = self.service.state.borrow_mut();
        ensure_running(&state)?;
        let record = notification_record_mut(&mut state.notifications, self.id)
            .ok_or(ApplicationShellError::StaleResource)?;
        update(record);
        Ok(())
    }

    pub fn close(mut self) -> Result<(), ApplicationShellError> {
        self.service.require_support(
            self.service.capabilities().notification_dismiss,
            ApplicationShellFeature::NotificationDismiss,
        )?;
        self.service.release_notification_handle(self.id, true)?;
        self.released = true;
        Ok(())
    }
}

impl Drop for NotificationHandle {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        // Dropping a handle always releases the runtime callback/resource
        // identity. If the backend can dismiss native notifications (or its
        // capability is not known yet), also request native teardown. A backend
        // such as Windows which explicitly cannot retract a toast still gets a
        // stale stable ID, so a late native activation can never reach dropped
        // application callbacks.
        let dismiss_native =
            self.service.capabilities().notification_dismiss != CapabilitySupport::Unsupported;
        let _ = self
            .service
            .release_notification_handle(self.id, dismiss_native);
    }
}

impl ApplicationShellService {
    fn release_notification_handle(
        &self,
        id: NotificationId,
        dismiss_native: bool,
    ) -> Result<(), ApplicationShellError> {
        let mut state = self.state.borrow_mut();
        ensure_running(&state)?;
        if !release_notification(&mut state.notifications, id) {
            return Err(ApplicationShellError::StaleResource);
        }
        if dismiss_native {
            enqueue(
                &mut state,
                NativeApplicationShellOperation::CloseNotification { id },
            );
        }
        drop(state);
        if dismiss_native {
            self.wake();
        }
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct MemoryApplicationShellAdapter {
    operations: Arc<Mutex<Vec<NativeApplicationShellOperation>>>,
}

impl MemoryApplicationShellAdapter {
    pub fn apply(
        &self,
        request: NativeApplicationShellRequest,
    ) -> NativeApplicationShellCompletion {
        self.operations
            .lock()
            .expect("memory shell adapter lock")
            .push(request.operation);
        NativeApplicationShellCompletion {
            request_id: request.request_id,
            result: Ok(()),
        }
    }

    #[must_use]
    pub fn operations(&self) -> Vec<NativeApplicationShellOperation> {
        self.operations
            .lock()
            .expect("memory shell adapter lock")
            .clone()
    }
}
