//! WebSocket session: handshake, request routing, telemetry streaming.

use crate::commands;
use futures_util::{FutureExt, SinkExt, StreamExt, stream::FuturesUnordered};
use incular_devtools_protocol::{
    ErrorCode, Hello, Message, PROTOCOL_VERSION, PeerKind, ResponsePayload, TargetEvent,
    TargetInfo, check_hello,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use tokio_tungstenite::tungstenite::Message as WsMessage;

const INCOMING_REQUEST_LIMIT: usize = 64 * 1024;
const SEND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Configuration used when starting a target-side DevTools session.
pub struct SessionConfig {
    /// Human-readable application name shown in target discovery.
    pub app_name: String,
}

/// Transport state passed from the platform runner to the DevTools accept loop.
pub(crate) struct ServeArgs {
    /// OS-generated session token required during the WebSocket handshake.
    pub token: String,
    /// Bound localhost listener transferred to the async accept loop.
    pub listener: std::net::TcpListener,
    /// Bounded UI command queue populated by connected DevTools clients.
    pub command_sender: std::sync::mpsc::SyncSender<commands::UiCommand>,
    pub command_payload_bytes: Arc<AtomicUsize>,
    pub response_payload_bytes: Arc<AtomicUsize>,
    /// Wakes the native event loop when new DevTools work becomes pending.
    pub command_wake: std::sync::Arc<dyn Fn() + Send + Sync>,
    /// Coalesces native event-loop wakes across queued commands.
    pub wake_pending: std::sync::Arc<AtomicBool>,
    /// Bounded telemetry queue produced by the application runtime.
    pub telemetry_receiver: tokio::sync::mpsc::Receiver<crate::BudgetedTelemetry>,
    /// Number of telemetry messages dropped because the queue was full.
    pub dropped: Arc<AtomicU64>,
    /// Set by the platform runner to stop the accept loop.
    pub shutdown: Arc<AtomicBool>,
    pub shutdown_notify: Arc<tokio::sync::Notify>,
    pub session_cleanup_pending: Arc<AtomicBool>,
    pub stopped: Arc<AtomicBool>,
    pub stopped_notify: Arc<tokio::sync::Notify>,
}

struct SessionState {
    token: String,
    command_sender: std::sync::mpsc::SyncSender<commands::UiCommand>,
    command_payload_bytes: Arc<AtomicUsize>,
    response_payload_bytes: Arc<AtomicUsize>,
    command_wake: std::sync::Arc<dyn Fn() + Send + Sync>,
    wake_pending: std::sync::Arc<AtomicBool>,
    telemetry_receiver: tokio::sync::mpsc::Receiver<crate::BudgetedTelemetry>,
    dropped: Arc<AtomicU64>,
    shutdown: Arc<AtomicBool>,
    shutdown_notify: Arc<tokio::sync::Notify>,
    session_cleanup_pending: Arc<AtomicBool>,
}

struct StopCompletion {
    stopped: Arc<AtomicBool>,
    notify: Arc<tokio::sync::Notify>,
}

impl Drop for StopCompletion {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        self.notify.notify_waiters();
    }
}

/// Generates a 128-bit hexadecimal session token from the platform OS random
/// source. DevTools is local-only, but the token still protects the session
/// from other local processes that can reach the discovery endpoint.
/// Returns `None` when the platform cannot provide OS-backed entropy.
#[must_use]
pub fn generate_token() -> Option<String> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).ok()?;
    Some(
        bytes
            .iter()
            .flat_map(|byte| {
                let byte = *byte;
                [HEX[(byte >> 4) as usize], HEX[(byte & 0xF) as usize]]
            })
            .map(|nibble| nibble as char)
            .collect(),
    )
}

/// Validates a DevTools hello against the target's session token.
pub fn validate(
    hello: &Hello,
    expected_token: &str,
) -> Result<(), incular_devtools_protocol::ErrorCode> {
    check_hello(hello, PeerKind::Devtools, Some(expected_token))
}

/// Single-client accept loop; the target serves one DevTools at a time.
pub(crate) async fn serve(args: ServeArgs) {
    let ServeArgs {
        listener,
        token,
        command_sender,
        command_payload_bytes,
        response_payload_bytes,
        command_wake,
        wake_pending,
        telemetry_receiver,
        dropped,
        shutdown,
        shutdown_notify,
        session_cleanup_pending,
        stopped,
        stopped_notify,
    } = args;
    let _completion = StopCompletion {
        stopped,
        notify: stopped_notify,
    };
    // Convert the blocking socket now that a Tokio reactor exists.
    if listener.set_nonblocking(true).is_err() {
        return;
    }
    let Ok(listener) = tokio::net::TcpListener::from_std(listener) else {
        return;
    };
    let mut state = SessionState {
        token,
        command_sender,
        command_payload_bytes,
        response_payload_bytes,
        command_wake,
        wake_pending,
        telemetry_receiver,
        dropped,
        shutdown: shutdown.clone(),
        shutdown_notify: shutdown_notify.clone(),
        session_cleanup_pending,
    };

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        let stream = tokio::select! {
            _ = shutdown_notify.notified() => break,
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => stream,
                Err(_) => {
                    if shutdown.load(Ordering::Relaxed) {
                        break;
                    }
                    tokio::select! {
                        _ = shutdown_notify.notified() => break,
                        _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => continue,
                    }
                }
            }
        };
        let websocket_config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig {
            max_message_size: Some(INCOMING_REQUEST_LIMIT),
            max_frame_size: Some(INCOMING_REQUEST_LIMIT),
            ..Default::default()
        };
        let Ok(websocket) =
            tokio_tungstenite::accept_async_with_config(stream, Some(websocket_config)).await
        else {
            continue;
        };
        let _ = run_session(websocket, &mut state).await;
        state.session_cleanup_pending.store(true, Ordering::Release);
        if !state.wake_pending.swap(true, Ordering::AcqRel) {
            (state.command_wake)();
        }
    }
}

async fn run_session<S>(
    websocket: tokio_tungstenite::WebSocketStream<S>,
    state: &mut SessionState,
) -> Result<(), ()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (mut sink, mut source) = websocket.split();
    async fn send_message<S>(sink: &mut S, message: &Message) -> Result<(), ()>
    where
        S: SinkExt<WsMessage> + std::marker::Unpin,
    {
        let text = serde_json::to_string(message).map_err(|_| ())?;
        tokio::time::timeout(SEND_TIMEOUT, sink.send(WsMessage::Text(text)))
            .await
            .map_err(|_| ())?
            .map_err(|_| ())
    }

    // ---- Handshake (5 s budget) ----
    let first = tokio::select! {
        _ = state.shutdown_notify.notified() => return Err(()),
        first = tokio::time::timeout(std::time::Duration::from_secs(5), source.next()) => {
            match first {
                Ok(Some(Ok(WsMessage::Text(text)))) => text,
                _ => return Err(()),
            }
        }
    };
    let Ok(hello) = serde_json::from_str::<Hello>(&first) else {
        send_message(
            &mut sink,
            &Message::Rejection {
                code: ErrorCode::InvalidRequest,
                message: "handshake must be Hello".into(),
            },
        )
        .await?;
        return Err(());
    };
    if let Err(code) = validate(&hello, &state.token) {
        let message = if code == ErrorCode::ProtocolVersionMismatch {
            format!(
                "unsupported protocol version {}; target supports {}..={PROTOCOL_VERSION}",
                hello.protocol_version,
                incular_devtools_protocol::MIN_SUPPORTED_PROTOCOL_VERSION,
            )
        } else {
            "DevTools handshake rejected".into()
        };
        send_message(&mut sink, &Message::Rejection { code, message }).await?;
        return Err(());
    }

    // Telemetry is session-scoped. Do not replay frames or deltas from a
    // previous client into a newly authenticated model; the client's first
    // tree request establishes a fresh delta baseline on the UI thread.
    while state.telemetry_receiver.try_recv().is_ok() {}

    // Handshake ack doubles as TargetInfo delivery.
    let info = TargetInfo {
        protocol_version: PROTOCOL_VERSION,
        framework_version: env!("CARGO_PKG_VERSION").to_owned(),
        pid: std::process::id(),
        executable: std::env::current_exe()
            .ok()
            .and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_default(),
        platform: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        windows: Vec::new(),
    };
    send_message(
        &mut sink,
        &Message::Response {
            request_id: 0,
            payload: Ok(ResponsePayload::TargetInfo(Box::new(info))),
        },
    )
    .await?;

    // ---- Main loop ----
    let mut last_dropped = state.dropped.load(Ordering::Relaxed);
    let mut outstanding = std::collections::HashSet::new();
    let mut completions = FuturesUnordered::new();
    loop {
        if state.shutdown.load(Ordering::Relaxed) {
            break;
        }
        tokio::select! {
            _ = state.shutdown_notify.notified() => break,
            incoming = source.next() => {
                match incoming {
                    Some(Ok(WsMessage::Text(text))) => {
                        match serde_json::from_str::<Message>(&text) {
                            Ok(Message::Request { request_id, body }) => {
                                if request_id == 0 || outstanding.contains(&request_id) {
                                    send_message(&mut sink, &Message::Response {
                                        request_id,
                                        payload: Err(ErrorCode::InvalidRequest),
                                    }).await.ok();
                                    continue;
                                }
                                if outstanding.len() >= 64 {
                                    send_message(&mut sink, &Message::Response {
                                        request_id,
                                        payload: Err(ErrorCode::InternalError),
                                    }).await.ok();
                                    continue;
                                }
                                let Some(request_permit) = crate::BytePermit::try_acquire(
                                    &state.command_payload_bytes,
                                    text.len(),
                                    crate::COMMAND_PAYLOAD_BUDGET,
                                ) else {
                                    send_message(&mut sink, &Message::Response {
                                        request_id,
                                        payload: Err(ErrorCode::InternalError),
                                    }).await.ok();
                                    continue;
                                };
                                let (sender, receiver) = tokio::sync::oneshot::channel();
                                let command = commands::UiCommand {
                                    body,
                                    completion: commands::CommandCompletion::new(
                                        sender,
                                        Arc::clone(&state.response_payload_bytes),
                                    ),
                                    _request_permit: request_permit,
                                };
                                if state.command_sender.try_send(command).is_err() {
                                    send_message(&mut sink, &Message::Response {
                                        request_id,
                                        payload: Err(ErrorCode::InternalError),
                                    }).await.ok();
                                    continue;
                                }
                                outstanding.insert(request_id);
                                completions.push(async move {
                                    (request_id, receiver.await)
                                }.boxed());
                                if !state.wake_pending.swap(true, Ordering::AcqRel) {
                                    (state.command_wake)();
                                }
                            }
                            Ok(_) => {}
                            Err(_) => {
                                send_message(&mut sink, &Message::Rejection {
                                    code: ErrorCode::InvalidRequest,
                                    message: "malformed message".into(),
                                }).await.ok();
                            }
                        }
                    }
                    Some(Ok(_)) => {}
                    _ => break,
                }
            }
            completion = completions.next(), if !completions.is_empty() => {
                if let Some((request_id, completion)) = completion {
                    outstanding.remove(&request_id);
                    let payload = match completion {
                        Ok(payload) => payload.result,
                        Err(_) => Err(ErrorCode::InternalError),
                    };
                    let message = Message::Response {
                        request_id,
                        payload,
                    };
                    send_message(&mut sink, &message).await?;
                }
            }
            event = state.telemetry_receiver.recv() => {
                let Some(event) = event else { break };
                let dropped_total = state.dropped.load(Ordering::Relaxed);
                if dropped_total > last_dropped {
                    send_message(&mut sink, &Message::Event(TargetEvent::DroppedTelemetry {
                        count: dropped_total - last_dropped,
                    })).await.ok();
                    last_dropped = dropped_total;
                }
                send_message(&mut sink, &Message::Event(event.event)).await?;
            }
        }
    }
    Ok(())
}
