//! WebSocket session: handshake, request routing, telemetry streaming.

use crate::commands;
use futures_util::{SinkExt, StreamExt};
use incular_devtools_protocol::{
    ErrorCode, Hello, Message, PROTOCOL_VERSION, PeerKind, ResponsePayload, TargetEvent,
    TargetInfo, check_hello,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio_tungstenite::tungstenite::Message as WsMessage;

/// Configuration used when starting a target-side DevTools session.
pub struct SessionConfig {
    /// Human-readable application name shown in target discovery.
    pub app_name: String,
}

/// Transport state passed from the platform runner to the DevTools accept loop.
pub struct ServeArgs {
    /// Ephemeral localhost port selected for this session.
    pub port: u16,
    /// OS-generated session token required during the WebSocket handshake.
    pub token: String,
    /// Human-readable application name associated with the session.
    pub app_name: String,
    /// Bound localhost listener transferred to the async accept loop.
    pub listener: std::net::TcpListener,
    /// Bounded UI command queue populated by connected DevTools clients.
    pub command_sender: std::sync::mpsc::SyncSender<commands::UiCommand>,
    /// Replies produced by the UI thread for connected DevTools clients.
    pub reply_receiver: Arc<std::sync::Mutex<std::sync::mpsc::Receiver<commands::UiReply>>>,
    /// Bounded telemetry queue produced by the application runtime.
    pub telemetry_receiver: tokio::sync::mpsc::Receiver<TargetEvent>,
    /// Number of telemetry messages dropped because the queue was full.
    pub dropped: Arc<AtomicU64>,
    /// Set by the platform runner to stop the accept loop.
    pub shutdown: Arc<AtomicBool>,
}

struct SessionState {
    token: String,
    command_sender: std::sync::mpsc::SyncSender<commands::UiCommand>,
    reply_receiver: Arc<std::sync::Mutex<std::sync::mpsc::Receiver<commands::UiReply>>>,
    telemetry_receiver: tokio::sync::mpsc::Receiver<TargetEvent>,
    dropped: Arc<AtomicU64>,
    shutdown: Arc<AtomicBool>,
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
pub async fn serve(args: ServeArgs) {
    let ServeArgs {
        listener,
        token,
        command_sender,
        reply_receiver,
        telemetry_receiver,
        dropped,
        shutdown,
        ..
    } = args;
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
        reply_receiver,
        telemetry_receiver,
        dropped,
        shutdown: shutdown.clone(),
    };

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        let accepted =
            tokio::time::timeout(std::time::Duration::from_millis(250), listener.accept()).await;
        let stream = match accepted {
            Ok(Ok((stream, _))) => stream,
            Ok(Err(_)) | Err(_) => {
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }
        };
        let Ok(websocket) = tokio_tungstenite::accept_async(stream).await else {
            continue;
        };
        let _ = run_session(websocket, &mut state).await;
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
        sink.send(WsMessage::Text(text)).await.map_err(|_| ())
    }

    // ---- Handshake (5 s budget) ----
    let first = match tokio::time::timeout(std::time::Duration::from_secs(5), source.next()).await {
        Ok(Some(Ok(WsMessage::Text(text)))) => text,
        _ => return Err(()),
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
    let mut last_dropped = 0_u64;
    loop {
        if state.shutdown.load(Ordering::Relaxed) {
            break;
        }
        tokio::select! {
            incoming = source.next() => {
                match incoming {
                    Some(Ok(WsMessage::Text(text))) => {
                        match serde_json::from_str::<Message>(&text) {
                            Ok(Message::Request { request_id, body }) => {
                                if state.command_sender.try_send(commands::UiCommand { request_id, body }).is_err() {
                                    send_message(&mut sink, &Message::Response {
                                        request_id,
                                        payload: Err(ErrorCode::InternalError),
                                    }).await.ok();
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
            reply = async {
                let receiver = state.reply_receiver.clone();
                tokio::task::spawn_blocking(move || {
                    receiver.lock().ok().and_then(|guarded| {
                        guarded.recv_timeout(std::time::Duration::from_millis(50)).ok()
                    })
                })
                .await
                .unwrap_or(None)
            } => {
                if let Some(reply) = reply {
                    let message = Message::Response {
                        request_id: reply.request_id,
                        payload: Ok(reply.payload),
                    };
                    send_message(&mut sink, &message).await.ok();
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
                send_message(&mut sink, &Message::Event(event)).await?;
            }
        }
    }
    Ok(())
}
