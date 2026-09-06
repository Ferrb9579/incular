//! Application-scoped native global shortcut requests.

use crate::{
    request_channel::{RequestChannel, RequestReceiver},
    tasks::RuntimeWake,
};
use incular_platform::{
    CapabilitySupport, GlobalShortcutChord, GlobalShortcutError, GlobalShortcutId,
    PlatformCapabilities,
};
use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    pin::Pin,
    sync::{Arc, RwLock, mpsc},
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
        let receiver = self.bridge.register(id, chord)?;
        Ok(GlobalShortcutRegistrationRequest { receiver })
    }
}

#[must_use = "global shortcut registration must be awaited or explicitly dropped"]
pub struct GlobalShortcutRegistrationRequest {
    receiver: RequestReceiver<Result<GlobalShortcutRegistration, GlobalShortcutError>>,
}
impl Unpin for GlobalShortcutRegistrationRequest {}
impl Future for GlobalShortcutRegistrationRequest {
    type Output = Result<GlobalShortcutRegistration, GlobalShortcutError>;
    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        self.receiver
            .poll_result(context, || Err(GlobalShortcutError::ApplicationStopped))
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
        let receiver = self.bridge.unregister(self.id)?;
        self.released = true;
        Ok(GlobalShortcutUnregistrationRequest { receiver })
    }
}

impl Drop for GlobalShortcutRegistration {
    fn drop(&mut self) {
        if !self.released {
            let _ = self.bridge.release(self.id);
            self.released = true;
        }
    }
}

#[must_use = "global shortcut unregistration must be awaited or explicitly dropped"]
pub struct GlobalShortcutUnregistrationRequest {
    receiver: RequestReceiver<Result<(), GlobalShortcutError>>,
}

impl Unpin for GlobalShortcutUnregistrationRequest {}

impl Future for GlobalShortcutUnregistrationRequest {
    type Output = Result<(), GlobalShortcutError>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        self.receiver
            .poll_result(context, || Err(GlobalShortcutError::ApplicationStopped))
    }
}

pub(crate) struct QueuedGlobalShortcutRequest {
    pub(crate) request_id: GlobalShortcutRequestId,
    pub(crate) operation: NativeGlobalShortcutOperation,
    pub(crate) reply: ShortcutReply,
}

pub(crate) struct PendingGlobalShortcutRequest {
    pub(crate) operation: NativeGlobalShortcutOperation,
    pub(crate) reply: ShortcutReply,
}

pub(crate) struct GlobalShortcutBridge {
    channel: RequestChannel<QueuedGlobalShortcutRequest>,
}
impl GlobalShortcutBridge {
    pub(crate) fn new(sender: mpsc::Sender<QueuedGlobalShortcutRequest>) -> Self {
        Self {
            channel: RequestChannel::new(sender),
        }
    }
    fn register(
        self: &Arc<Self>,
        id: GlobalShortcutId,
        chord: GlobalShortcutChord,
    ) -> Result<
        RequestReceiver<Result<GlobalShortcutRegistration, GlobalShortcutError>>,
        GlobalShortcutError,
    > {
        self.channel
            .request(|request_id| {
                let (sender, receiver) = oneshot::channel();
                (
                    QueuedGlobalShortcutRequest {
                        request_id: GlobalShortcutRequestId::new(request_id),
                        operation: NativeGlobalShortcutOperation::Register { id, chord },
                        reply: ShortcutReply::Registration {
                            sender,
                            bridge: self.clone(),
                        },
                    },
                    RequestReceiver::new(receiver, || {}),
                )
            })
            .map_err(|_| GlobalShortcutError::ApplicationStopped)
    }
    fn unregister(
        &self,
        id: GlobalShortcutId,
    ) -> Result<RequestReceiver<Result<(), GlobalShortcutError>>, GlobalShortcutError> {
        self.channel
            .request(|request_id| {
                let (sender, receiver) = oneshot::channel();
                (
                    QueuedGlobalShortcutRequest {
                        request_id: GlobalShortcutRequestId::new(request_id),
                        operation: NativeGlobalShortcutOperation::Unregister { id },
                        reply: ShortcutReply::Unregistration(sender),
                    },
                    RequestReceiver::new(receiver, || {}),
                )
            })
            .map_err(|_| GlobalShortcutError::ApplicationStopped)
    }
    fn release(&self, id: GlobalShortcutId) -> Result<(), GlobalShortcutError> {
        self.channel
            .request(|request_id| {
                (
                    QueuedGlobalShortcutRequest {
                        request_id: GlobalShortcutRequestId::new(request_id),
                        operation: NativeGlobalShortcutOperation::Unregister { id },
                        reply: ShortcutReply::Detached,
                    },
                    (),
                )
            })
            .map_err(|_| GlobalShortcutError::ApplicationStopped)
    }
    pub(crate) fn set_wake(&self, wake: Arc<dyn RuntimeWake>) {
        self.channel.set_wake(wake);
    }
    pub(crate) fn stop(&self) {
        self.channel.stop();
    }
}

pub(crate) fn drain_queued(
    receiver: &mpsc::Receiver<QueuedGlobalShortcutRequest>,
    pending: &mut crate::request_registry::RequestRegistry<
        GlobalShortcutRequestId,
        PendingGlobalShortcutRequest,
    >,
    native: &mut VecDeque<NativeGlobalShortcutRequest>,
) {
    while let Ok(queued) = receiver.try_recv() {
        if matches!(
            queued.operation,
            NativeGlobalShortcutOperation::Register { .. }
        ) && queued.reply.is_abandoned()
        {
            continue;
        }
        pending.insert(
            queued.request_id,
            PendingGlobalShortcutRequest {
                operation: queued.operation,
                reply: queued.reply,
            },
        );
        native.push_back(NativeGlobalShortcutRequest {
            request_id: queued.request_id,
            operation: queued.operation,
        });
    }
}

pub(crate) enum ShortcutReply {
    Registration {
        sender: oneshot::Sender<Result<GlobalShortcutRegistration, GlobalShortcutError>>,
        bridge: Arc<GlobalShortcutBridge>,
    },
    Unregistration(oneshot::Sender<Result<(), GlobalShortcutError>>),
    Detached,
}
impl ShortcutReply {
    pub(crate) fn is_abandoned(&self) -> bool {
        match self {
            Self::Registration { sender, .. } => sender.is_closed(),
            Self::Unregistration(sender) => sender.is_closed(),
            Self::Detached => false,
        }
    }
    pub(crate) fn finish(
        self,
        operation: NativeGlobalShortcutOperation,
        result: Result<(), GlobalShortcutError>,
    ) -> GlobalShortcutCompletionStatus {
        match self {
            Self::Registration { sender, bridge } => {
                let NativeGlobalShortcutOperation::Register { id, chord } = operation else {
                    unreachable!("registration reply requires a register operation");
                };
                let result = result.map(|()| GlobalShortcutRegistration {
                    id,
                    chord,
                    bridge,
                    released: false,
                });
                if let Err(Ok(mut registration)) = sender.send(result) {
                    // The host rolls back immediately. Disarm the returned lease
                    // so its destructor cannot enqueue a second unregistration.
                    registration.released = true;
                    return GlobalShortcutCompletionStatus::AbandonedRegistration(id);
                }
            }
            Self::Unregistration(sender) => {
                let _ = sender.send(result);
            }
            Self::Detached => {}
        }
        GlobalShortcutCompletionStatus::Completed
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
