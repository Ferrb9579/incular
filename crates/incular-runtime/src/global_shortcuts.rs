//! Application-scoped native global shortcut requests.

use crate::{request_admission::RequestAdmission, tasks::RuntimeWake};
use incular_platform::{
    CapabilitySupport, GlobalShortcutChord, GlobalShortcutError, GlobalShortcutId,
    PlatformCapabilities,
};
use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    task::{Context, Poll},
};
use tokio::sync::oneshot;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GlobalShortcutRequestId(u64);

impl GlobalShortcutRequestId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeGlobalShortcutOperation {
    Register {
        id: GlobalShortcutId,
        chord: GlobalShortcutChord,
    },
    Unregister {
        id: GlobalShortcutId,
    },
}

impl NativeGlobalShortcutOperation {
    #[must_use]
    pub const fn id(self) -> GlobalShortcutId {
        match self {
            Self::Register { id, .. } | Self::Unregister { id } => id,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeGlobalShortcutRequest {
    pub request_id: GlobalShortcutRequestId,
    pub operation: NativeGlobalShortcutOperation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeGlobalShortcutCompletion {
    pub request_id: GlobalShortcutRequestId,
    pub result: Result<(), GlobalShortcutError>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlobalShortcutCompletionStatus {
    Completed,
    /// The caller dropped a registration future after native registration.
    /// The desktop host must immediately unregister the supplied ID.
    AbandonedRegistration(GlobalShortcutId),
    UnknownRequest,
}

#[derive(Clone)]
pub struct GlobalShortcutService {
    bridge: Arc<GlobalShortcutBridge>,
    capabilities: Arc<RwLock<PlatformCapabilities>>,
}

impl std::fmt::Debug for GlobalShortcutService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("GlobalShortcutService(..)")
    }
}

impl GlobalShortcutService {
    pub(crate) fn new(
        bridge: Arc<GlobalShortcutBridge>,
        capabilities: Arc<RwLock<PlatformCapabilities>>,
    ) -> Self {
        Self {
            bridge,
            capabilities,
        }
    }

    #[must_use]
    pub fn support(&self) -> CapabilitySupport {
        self.capabilities
            .read()
            .expect("application capability snapshot lock")
            .application_services
            .global_shortcuts
    }

    /// Queues native registration. The returned future resolves only after the
    /// OS accepted or rejected the chord, so conflicts are never reported as a
    /// false success. Registration may be queued before desktop capability
    /// discovery; an explicit `Unsupported` snapshot is the only state that
    /// rejects the request before it reaches the native host.
    pub fn register(
        &self,
        id: GlobalShortcutId,
        chord: GlobalShortcutChord,
    ) -> Result<GlobalShortcutRegistrationRequest, GlobalShortcutError> {
        if self.support() == CapabilitySupport::Unsupported {
            return Err(GlobalShortcutError::Unsupported);
        }
        let receiver = self
            .bridge
            .request(NativeGlobalShortcutOperation::Register { id, chord }, true)?;
        Ok(GlobalShortcutRegistrationRequest {
            id,
            chord,
            bridge: self.bridge.clone(),
            receiver,
        })
    }
}

#[must_use = "global shortcut registration must be awaited or explicitly dropped"]
pub struct GlobalShortcutRegistrationRequest {
    id: GlobalShortcutId,
    chord: GlobalShortcutChord,
    bridge: Arc<GlobalShortcutBridge>,
    receiver: oneshot::Receiver<Result<(), GlobalShortcutError>>,
}

impl Unpin for GlobalShortcutRegistrationRequest {}

impl Future for GlobalShortcutRegistrationRequest {
    type Output = Result<GlobalShortcutRegistration, GlobalShortcutError>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.receiver).poll(context) {
            Poll::Ready(Ok(Ok(()))) => Poll::Ready(Ok(GlobalShortcutRegistration {
                id: self.id,
                chord: self.chord,
                bridge: self.bridge.clone(),
                released: false,
            })),
            Poll::Ready(Ok(Err(error))) => Poll::Ready(Err(error)),
            Poll::Ready(Err(_)) => Poll::Ready(Err(GlobalShortcutError::ApplicationStopped)),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// RAII ownership of one active native global shortcut.
///
/// Dropping the handle queues native unregistration. Use [`Self::unregister`]
/// when the caller needs to observe a typed native unregistration result.
pub struct GlobalShortcutRegistration {
    id: GlobalShortcutId,
    chord: GlobalShortcutChord,
    bridge: Arc<GlobalShortcutBridge>,
    released: bool,
}

impl std::fmt::Debug for GlobalShortcutRegistration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GlobalShortcutRegistration")
            .field("id", &self.id)
            .field("chord", &self.chord)
            .field("released", &self.released)
            .finish()
    }
}

impl GlobalShortcutRegistration {
    #[must_use]
    pub const fn id(&self) -> GlobalShortcutId {
        self.id
    }

    #[must_use]
    pub const fn chord(&self) -> GlobalShortcutChord {
        self.chord
    }

    pub fn unregister(
        mut self,
    ) -> Result<GlobalShortcutUnregistrationRequest, GlobalShortcutError> {
        let receiver = self.bridge.request(
            NativeGlobalShortcutOperation::Unregister { id: self.id },
            true,
        )?;
        self.released = true;
        Ok(GlobalShortcutUnregistrationRequest { receiver })
    }
}

impl Drop for GlobalShortcutRegistration {
    fn drop(&mut self) {
        if !self.released {
            let _ = self.bridge.request(
                NativeGlobalShortcutOperation::Unregister { id: self.id },
                false,
            );
            self.released = true;
        }
    }
}

#[must_use = "global shortcut unregistration must be awaited or explicitly dropped"]
pub struct GlobalShortcutUnregistrationRequest {
    receiver: oneshot::Receiver<Result<(), GlobalShortcutError>>,
}

impl Unpin for GlobalShortcutUnregistrationRequest {}

impl Future for GlobalShortcutUnregistrationRequest {
    type Output = Result<(), GlobalShortcutError>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.receiver).poll(context) {
            Poll::Ready(Ok(result)) => Poll::Ready(result),
            Poll::Ready(Err(_)) => Poll::Ready(Err(GlobalShortcutError::ApplicationStopped)),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub(crate) struct QueuedGlobalShortcutRequest {
    pub(crate) request_id: GlobalShortcutRequestId,
    pub(crate) operation: NativeGlobalShortcutOperation,
    pub(crate) sender: Option<oneshot::Sender<Result<(), GlobalShortcutError>>>,
}

pub(crate) struct PendingGlobalShortcutRequest {
    pub(crate) operation: NativeGlobalShortcutOperation,
    pub(crate) sender: Option<oneshot::Sender<Result<(), GlobalShortcutError>>>,
}

pub(crate) struct GlobalShortcutBridge {
    sender: mpsc::Sender<QueuedGlobalShortcutRequest>,
    wake: Mutex<Option<Arc<dyn RuntimeWake>>>,
    next_request: AtomicU64,
    admission: RequestAdmission,
}

impl GlobalShortcutBridge {
    pub(crate) fn new(sender: mpsc::Sender<QueuedGlobalShortcutRequest>) -> Self {
        Self {
            sender,
            wake: Mutex::new(None),
            next_request: AtomicU64::new(1),
            admission: RequestAdmission::new(),
        }
    }

    fn request(
        &self,
        operation: NativeGlobalShortcutOperation,
        wants_result: bool,
    ) -> Result<oneshot::Receiver<Result<(), GlobalShortcutError>>, GlobalShortcutError> {
        let receiver = self
            .admission
            .admit(|| {
                let request_id = self
                    .next_request
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                        current.checked_add(1)
                    })
                    .map(GlobalShortcutRequestId::new)
                    .map_err(|_| GlobalShortcutError::ApplicationStopped)?;
                let (sender, receiver) = oneshot::channel();
                let queued_sender = wants_result.then_some(sender);
                self.sender
                    .send(QueuedGlobalShortcutRequest {
                        request_id,
                        operation,
                        sender: queued_sender,
                    })
                    .map_err(|_| GlobalShortcutError::ApplicationStopped)?;
                Ok(receiver)
            })
            .unwrap_or(Err(GlobalShortcutError::ApplicationStopped))?;
        let wake = self.wake.lock().expect("global shortcut wake lock").clone();
        if let Some(wake) = wake {
            wake.wake();
        }
        Ok(receiver)
    }

    pub(crate) fn set_wake(&self, wake: Arc<dyn RuntimeWake>) {
        *self.wake.lock().expect("global shortcut wake lock") = Some(wake);
    }

    pub(crate) fn stop(&self) {
        self.admission.stop();
        self.wake.lock().expect("global shortcut wake lock").take();
    }
}

pub(crate) fn drain_queued(
    receiver: &mpsc::Receiver<QueuedGlobalShortcutRequest>,
    pending: &mut std::collections::HashMap<GlobalShortcutRequestId, PendingGlobalShortcutRequest>,
    native: &mut VecDeque<NativeGlobalShortcutRequest>,
) {
    while let Ok(mut queued) = receiver.try_recv() {
        if matches!(
            queued.operation,
            NativeGlobalShortcutOperation::Register { .. }
        ) && queued
            .sender
            .as_ref()
            .is_some_and(oneshot::Sender::is_closed)
        {
            continue;
        }
        if queued
            .sender
            .as_ref()
            .is_some_and(oneshot::Sender::is_closed)
        {
            queued.sender = None;
        }
        let previous = pending.insert(
            queued.request_id,
            PendingGlobalShortcutRequest {
                operation: queued.operation,
                sender: queued.sender,
            },
        );
        debug_assert!(previous.is_none());
        native.push_back(NativeGlobalShortcutRequest {
            request_id: queued.request_id,
            operation: queued.operation,
        });
    }
}

/// Deterministic native-side adapter for tests and non-OS embedders.
///
/// It implements the same stable-ID/chord conflict rules expected from a
/// desktop backend without registering any host operating-system hotkeys.
#[derive(Debug)]
pub struct MemoryGlobalShortcutAdapter {
    support: CapabilitySupport,
    registrations: HashMap<GlobalShortcutId, GlobalShortcutChord>,
}

impl Default for MemoryGlobalShortcutAdapter {
    fn default() -> Self {
        Self::new(CapabilitySupport::Supported)
    }
}

impl MemoryGlobalShortcutAdapter {
    #[must_use]
    pub fn new(support: CapabilitySupport) -> Self {
        Self {
            support,
            registrations: HashMap::new(),
        }
    }

    #[must_use]
    pub const fn support(&self) -> CapabilitySupport {
        self.support
    }

    #[must_use]
    pub fn registered_chord(&self, id: GlobalShortcutId) -> Option<GlobalShortcutChord> {
        self.registrations.get(&id).copied()
    }

    #[must_use]
    pub fn registration_count(&self) -> usize {
        self.registrations.len()
    }

    pub fn respond(
        &mut self,
        request: NativeGlobalShortcutRequest,
    ) -> NativeGlobalShortcutCompletion {
        let result = if self.support != CapabilitySupport::Supported {
            Err(GlobalShortcutError::Unsupported)
        } else {
            match request.operation {
                NativeGlobalShortcutOperation::Register { id, chord } => {
                    if self.registrations.contains_key(&id) {
                        Err(GlobalShortcutError::AlreadyRegistered(id))
                    } else if self
                        .registrations
                        .values()
                        .any(|registered| *registered == chord)
                    {
                        Err(GlobalShortcutError::Conflict(chord))
                    } else {
                        self.registrations.insert(id, chord);
                        Ok(())
                    }
                }
                NativeGlobalShortcutOperation::Unregister { id } => {
                    if self.registrations.remove(&id).is_some() {
                        Ok(())
                    } else {
                        Err(GlobalShortcutError::NotRegistered(id))
                    }
                }
            }
        };
        NativeGlobalShortcutCompletion {
            request_id: request.request_id,
            result,
        }
    }

    /// Produces the same application-level activation as a real native hotkey
    /// press when `id` is currently registered.
    #[must_use]
    pub fn invoke(&self, id: GlobalShortcutId) -> Option<incular_platform::ApplicationActivation> {
        self.registrations
            .contains_key(&id)
            .then_some(incular_platform::ApplicationActivation::GlobalShortcutInvoked(id))
    }
}
