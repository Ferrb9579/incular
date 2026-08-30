//! Target-side DevTools agent.
//!
//! Owns the localhost WebSocket server, discovery registration, session
//! authentication, and the bounded queues that move telemetry between the
//! application's UI thread and connected DevTools clients. The UI thread
//! never blocks on network IO: it uses `try_send` and counts drops.

pub mod commands;
pub mod discovery;
pub mod session;

use incular_devtools_protocol::{
    ErrorCode, Hello, Message, PROTOCOL_VERSION, ResponsePayload, TargetEvent, TargetInfo,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Events queued by the UI thread for delivery to DevTools. Bounded: a slow
/// client causes drops (reported via `DroppedTelemetry`), never blocking.
#[derive(Clone)]
pub struct TelemetryQueue {
    inner: tokio::sync::mpsc::Sender<TargetEvent>,
    dropped: Arc<AtomicU64>,
}
impl TelemetryQueue {
    /// Non-blocking push used by the UI thread. A full bounded queue counts
    /// a drop; DevTools receives periodic `DroppedTelemetry` accounting.
    pub fn try_push(&self, event: TargetEvent) {
        if self.inner.try_send(event).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

/// Commands waiting for the UI thread. The runner drains this every frame;
/// it must never block.
pub struct CommandPump {
    receiver: Mutex<std::sync::mpsc::Receiver<commands::UiCommand>>,
}
impl CommandPump {
    pub fn try_next(&self) -> Option<commands::UiCommand> {
        self.receiver
            .lock()
            .ok()?
            .recv_timeout(std::time::Duration::from_micros(0))
            .ok()
    }
}

/// Handle held by the platform adapter while the target is running.
pub struct AgentHandle {
    pub telemetry: TelemetryQueue,
    pub commands: Arc<CommandPump>,
    /// Replies computed by the UI thread flow back to the connected client.
    pub reply_sender: std::sync::mpsc::SyncSender<commands::UiReply>,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
    pub port: u16,
    pub token: String,
}
impl AgentHandle {
    /// Stops the agent thread and removes its discovery record.
    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        discovery::remove_session(self.port);
    }
}
impl Drop for AgentHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Launches the DevTools transport on the application's established Tokio
/// runtime. The server remains isolated from retained UI state by its bounded
/// command/telemetry channels; it does not need (or create) a second runtime.
pub fn spawn(
    config: crate::session::SessionConfig,
    runtime: incular_runtime::TokioHandle,
) -> Option<AgentHandle> {
    let std_listener = std::net::TcpListener::bind(("127.0.0.1", 0)).ok()?;
    let port = std_listener.local_addr().ok()?.port();
    // Non-blocking so the agent thread's Tokio reactor can adopt it cleanly.
    std_listener.set_nonblocking(true).ok()?;
    let token = session::generate_token()?;
    let app_name = config.app_name.clone();
    let _ = config;

    discovery::register_session(discovery::DiscoveryFileEntry {
        pid: std::process::id(),
        app_name: app_name.clone(),
        port,
        auth_token: token.clone(),
        started_unix_ms: now_unix_ms(),
        protocol_version: PROTOCOL_VERSION,
    });

    let (command_sender, command_receiver) =
        std::sync::mpsc::sync_channel::<commands::UiCommand>(256);
    let (reply_sender, reply_receiver) = std::sync::mpsc::sync_channel::<commands::UiReply>(256);
    let reply_receiver = Arc::new(Mutex::new(reply_receiver));
    let (telemetry_sender, telemetry_receiver) = tokio::sync::mpsc::channel::<TargetEvent>(1024);
    let token_for_thread = token.clone();
    let std_listener_for_thread = std_listener;
    let app_name_for_thread = app_name.clone();
    let dropped = Arc::new(AtomicU64::new(0));
    let dropped_for_handle = dropped.clone();
    let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let shutdown_for_thread = shutdown.clone();

    runtime.spawn(session::serve(session::ServeArgs {
        port,
        token: token_for_thread,
        app_name: app_name_for_thread,
        listener: std_listener_for_thread,
        command_sender,
        reply_receiver,
        telemetry_receiver,
        dropped,
        shutdown: shutdown_for_thread,
    }));
    Some(AgentHandle {
        telemetry: TelemetryQueue {
            inner: telemetry_sender,
            dropped: dropped_for_handle,
        },
        commands: Arc::new(CommandPump {
            receiver: Mutex::new(command_receiver),
        }),
        reply_sender,
        shutdown,
        port,
        token,
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
