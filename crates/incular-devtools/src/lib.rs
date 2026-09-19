//! Target-side DevTools agent.
//!
//! Owns the localhost WebSocket server, discovery registration, session
//! authentication, and the bounded queues that move telemetry between the
//! application's UI thread and connected DevTools clients. The UI thread
//! never blocks on network IO: it uses `try_send` and counts drops.

#![warn(missing_docs)]

pub mod commands;
pub mod discovery;
pub mod session;

use incular_devtools_protocol::{
    ErrorCode, Hello, Message, PROTOCOL_VERSION, ResponsePayload, TargetEvent, TargetInfo,
};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const COMMAND_QUEUE_CAPACITY: usize = 256;
const TELEMETRY_QUEUE_CAPACITY: usize = 1024;
const COMMAND_PAYLOAD_BUDGET: usize = 4 * 1024 * 1024;
const RESPONSE_PAYLOAD_BUDGET: usize = 32 * 1024 * 1024;
const TELEMETRY_PAYLOAD_BUDGET: usize = 16 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct BytePermit {
    used: Arc<AtomicUsize>,
    bytes: usize,
}

impl BytePermit {
    pub(crate) fn try_acquire(used: &Arc<AtomicUsize>, bytes: usize, limit: usize) -> Option<Self> {
        used.fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
            current.checked_add(bytes).filter(|next| *next <= limit)
        })
        .ok()?;
        Some(Self {
            used: Arc::clone(used),
            bytes,
        })
    }
}

impl Drop for BytePermit {
    fn drop(&mut self) {
        self.used.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

pub(crate) struct BudgetedTelemetry {
    event: TargetEvent,
    _permit: BytePermit,
}

/// Events queued by the UI thread for delivery to DevTools. Bounded: a slow
/// client causes drops (reported via `DroppedTelemetry`), never blocking.
#[derive(Clone)]
pub struct TelemetryQueue {
    inner: tokio::sync::mpsc::Sender<BudgetedTelemetry>,
    dropped: Arc<AtomicU64>,
    payload_bytes: Arc<AtomicUsize>,
}
impl TelemetryQueue {
    /// Non-blocking push used by the UI thread. A full bounded queue counts
    /// a drop; DevTools receives periodic `DroppedTelemetry` accounting.
    #[must_use]
    pub fn try_push(&self, event: TargetEvent) -> bool {
        let Some(bytes) = serde_json::to_vec(&event).ok().map(|payload| payload.len()) else {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return false;
        };
        let Some(permit) =
            BytePermit::try_acquire(&self.payload_bytes, bytes, TELEMETRY_PAYLOAD_BUDGET)
        else {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return false;
        };
        if self
            .inner
            .try_send(BudgetedTelemetry {
                event,
                _permit: permit,
            })
            .is_err()
        {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            false
        } else {
            true
        }
    }
    /// Returns the cumulative number of telemetry events rejected by the
    /// bounded queue or aggregate byte budget.
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

/// Commands waiting for the UI thread. The runner drains this every frame;
/// it must never block.
pub struct CommandPump {
    state: Mutex<CommandPumpState>,
    wake_pending: Arc<std::sync::atomic::AtomicBool>,
    session_cleanup_pending: Arc<std::sync::atomic::AtomicBool>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

struct CommandPumpState {
    receiver: std::sync::mpsc::Receiver<commands::UiCommand>,
    prefetched: Option<commands::UiCommand>,
}
impl CommandPump {
    /// Removes the next admitted UI command without blocking the UI thread.
    pub fn try_next(&self) -> Option<commands::UiCommand> {
        let mut state = self.state.lock().ok()?;
        state
            .prefetched
            .take()
            .or_else(|| state.receiver.try_recv().ok())
    }

    /// Acknowledges the current coalesced wake and reports whether work remains.
    #[must_use]
    pub fn acknowledge_wake(&self) -> bool {
        self.wake_pending.store(false, Ordering::Release);
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => return false,
        };
        if state.prefetched.is_none() {
            state.prefetched = state.receiver.try_recv().ok();
        }
        let pending = state.prefetched.is_some();
        drop(state);
        if pending {
            self.request_wake();
        }
        pending
    }

    /// Consumes a pending session-cleanup request before normal commands run.
    #[must_use]
    pub fn take_session_cleanup(&self) -> bool {
        self.session_cleanup_pending.swap(false, Ordering::AcqRel)
    }

    fn request_wake(&self) {
        if !self.wake_pending.swap(true, Ordering::AcqRel) {
            (self.wake)();
        }
    }
}

/// Handle held by the platform adapter while the target is running.
pub struct AgentHandle {
    /// Bounded telemetry admission handle owned by the desktop integration.
    pub telemetry: TelemetryQueue,
    /// Bounded UI-command pump serviced from the native event loop.
    pub commands: Arc<CommandPump>,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
    shutdown_notify: Arc<tokio::sync::Notify>,
    stopped: Arc<std::sync::atomic::AtomicBool>,
    stopped_notify: Arc<tokio::sync::Notify>,
    _registration: discovery::DiscoveryRegistration,
}
impl AgentHandle {
    /// Signals the agent to stop. Discovery ownership is released on drop.
    pub fn stop(&self) {
        if !self.shutdown.swap(true, Ordering::AcqRel) {
            self.shutdown_notify.notify_waiters();
        }
    }

    /// Waits until the async accept/session task has fully stopped.
    pub async fn stopped(&self) {
        loop {
            if self.stopped.load(Ordering::Acquire) {
                return;
            }
            let notified = self.stopped_notify.notified();
            if self.stopped.load(Ordering::Acquire) {
                return;
            }
            notified.await;
        }
    }
}
impl Drop for AgentHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Failures that can occur before a DevTools target agent becomes discoverable.
#[derive(Debug)]
pub enum SpawnError {
    /// The loopback listener could not be bound.
    Bind(std::io::Error),
    /// The bound listener's local address could not be queried.
    ListenerAddress(std::io::Error),
    /// The listener could not be configured for nonblocking Tokio adoption.
    NonBlocking(std::io::Error),
    /// The operating system could not provide secure random bytes for authentication.
    Entropy,
    /// The per-user discovery record could not be registered.
    Discovery(std::io::Error),
}

impl std::fmt::Display for SpawnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bind(error) => write!(f, "loopback bind failed: {error}"),
            Self::ListenerAddress(error) => write!(f, "listener address failed: {error}"),
            Self::NonBlocking(error) => write!(f, "listener setup failed: {error}"),
            Self::Entropy => f.write_str("secure session token generation failed"),
            Self::Discovery(error) => write!(f, "session discovery registration failed: {error}"),
        }
    }
}

impl std::error::Error for SpawnError {}

/// Launches the DevTools transport on the application's established Tokio
/// runtime. The server remains isolated from retained UI state by its bounded
/// command/telemetry channels; it does not need (or create) a second runtime.
pub fn spawn(
    config: crate::session::SessionConfig,
    runtime: incular_runtime::TokioHandle,
    wake: Arc<dyn Fn() + Send + Sync>,
) -> Result<AgentHandle, SpawnError> {
    let std_listener = std::net::TcpListener::bind(("127.0.0.1", 0)).map_err(SpawnError::Bind)?;
    let port = std_listener
        .local_addr()
        .map_err(SpawnError::ListenerAddress)?
        .port();
    // Non-blocking so the agent thread's Tokio reactor can adopt it cleanly.
    std_listener
        .set_nonblocking(true)
        .map_err(SpawnError::NonBlocking)?;
    let token = session::generate_token().ok_or(SpawnError::Entropy)?;
    let app_name = config.app_name.clone();
    let _ = config;

    let registration = discovery::register_session(discovery::DiscoveryFileEntry {
        pid: std::process::id(),
        app_name: app_name.clone(),
        port,
        auth_token: token.clone(),
        started_unix_ms: now_unix_ms(),
        protocol_version: PROTOCOL_VERSION,
    })
    .map_err(SpawnError::Discovery)?;

    let (command_sender, command_receiver) =
        std::sync::mpsc::sync_channel::<commands::UiCommand>(COMMAND_QUEUE_CAPACITY);
    let (telemetry_sender, telemetry_receiver) =
        tokio::sync::mpsc::channel::<BudgetedTelemetry>(TELEMETRY_QUEUE_CAPACITY);
    let token_for_thread = token.clone();
    let std_listener_for_thread = std_listener;
    let dropped = Arc::new(AtomicU64::new(0));
    let dropped_for_handle = dropped.clone();
    let command_payload_bytes = Arc::new(AtomicUsize::new(0));
    let response_payload_bytes = Arc::new(AtomicUsize::new(0));
    let telemetry_payload_bytes = Arc::new(AtomicUsize::new(0));
    let telemetry_payload_bytes_for_handle = telemetry_payload_bytes.clone();
    let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let shutdown_for_thread = shutdown.clone();
    let shutdown_notify = Arc::new(tokio::sync::Notify::new());
    let shutdown_notify_for_thread = shutdown_notify.clone();
    let stopped = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stopped_for_thread = stopped.clone();
    let stopped_notify = Arc::new(tokio::sync::Notify::new());
    let stopped_notify_for_thread = stopped_notify.clone();
    let wake_pending = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let session_cleanup_pending = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let session_cleanup_for_thread = session_cleanup_pending.clone();
    let command_pump = Arc::new(CommandPump {
        state: Mutex::new(CommandPumpState {
            receiver: command_receiver,
            prefetched: None,
        }),
        wake_pending: wake_pending.clone(),
        session_cleanup_pending,
        wake: wake.clone(),
    });

    runtime.spawn(session::serve(session::ServeArgs {
        token: token_for_thread,
        listener: std_listener_for_thread,
        command_sender,
        command_payload_bytes,
        response_payload_bytes,
        command_wake: wake,
        wake_pending,
        telemetry_receiver,
        dropped,
        shutdown: shutdown_for_thread,
        shutdown_notify: shutdown_notify_for_thread,
        session_cleanup_pending: session_cleanup_for_thread,
        stopped: stopped_for_thread,
        stopped_notify: stopped_notify_for_thread,
    }));
    Ok(AgentHandle {
        telemetry: TelemetryQueue {
            inner: telemetry_sender,
            dropped: dropped_for_handle,
            payload_bytes: telemetry_payload_bytes_for_handle,
        },
        commands: command_pump,
        shutdown,
        shutdown_notify,
        stopped,
        stopped_notify,
        _registration: registration,
    })
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

/// Builds the post-handshake [`TargetInfo`] payload.
pub fn target_info(
    framework_version: &str,
    windows: Vec<incular_devtools_protocol::WindowSummary>,
) -> TargetInfo {
    TargetInfo {
        protocol_version: PROTOCOL_VERSION,
        framework_version: framework_version.to_owned(),
        pid: std::process::id(),
        executable: std::env::current_exe()
            .ok()
            .and_then(|path| path.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default(),
        platform: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        windows,
    }
}

/// Wraps a payload as a successful response frame.
pub fn ok_response(request_id: u64, payload: ResponsePayload) -> Message {
    Message::Response {
        request_id,
        payload: Ok(payload),
    }
}

/// Wraps an error code as a rejection response frame.
pub fn error_response(request_id: u64, code: ErrorCode) -> Message {
    Message::Response {
        request_id,
        payload: Err(code),
    }
}

/// Validates a DevTools hello against this target's token.
pub fn validate_hello(hello: &Hello, expected_token: &str) -> Result<(), ErrorCode> {
    crate::session::validate(hello, expected_token)
}

/// Current process RSS in MiB, obtained without retaining a `sysinfo`
/// snapshot or sampling unrelated processes. Non-Linux targets report zero
/// until their platform adapter supplies an equivalent value.
#[must_use]
pub fn process_rss_mb() -> usize {
    #[cfg(target_os = "linux")]
    {
        let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
            return 0;
        };
        status
            .lines()
            .find_map(|line| line.strip_prefix("VmRSS:"))
            .and_then(|value| value.split_whitespace().next())
            .and_then(|kilobytes| kilobytes.parse::<usize>().ok())
            .map(|kilobytes| kilobytes / 1024)
            .unwrap_or_default()
    }
    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}
