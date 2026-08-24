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

pub struct SessionConfig {
    pub app_name: String,
}

pub struct ServeArgs {
    pub port: u16,
    pub token: String,
    pub app_name: String,
    pub listener: std::net::TcpListener,
    pub command_sender: std::sync::mpsc::SyncSender<commands::UiCommand>,
    pub reply_receiver: Arc<std::sync::Mutex<std::sync::mpsc::Receiver<commands::UiReply>>>,
    pub telemetry_receiver: tokio::sync::mpsc::Receiver<TargetEvent>,
    pub dropped: Arc<AtomicU64>,
    pub shutdown: Arc<AtomicBool>,
}

/// Validates a DevTools hello against this target's session token.
/// 32 hex characters from the OS entropy pool (with a time/pid fallback so
/// non-Unix targets still get an unpredictable-enough development token).
pub fn generate_token() -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut bytes = [0u8; 16];
    match std::fs::File::open("/dev/urandom") {
        Ok(mut file) => {
            if std::io::Read::read_exact(&mut file, &mut bytes).is_err() {
                fill_fallback(&mut bytes);
            }
        }
        Err(_) => fill_fallback(&mut bytes),
    }
    bytes
        .iter()
        .flat_map(|byte| {
            let byte = *byte;
            [HEX[(byte >> 4) as usize], HEX[(byte & 0xF) as usize]]
        })
        .map(|nibble| nibble as char)
        .collect()
}

pub fn validate(
    hello: &Hello,
    expected_token: &str,
) -> Result<(), incular_devtools_protocol::ErrorCode> {
    check_hello(hello, PeerKind::Devtools, Some(expected_token))
}

fn fill_fallback(bytes: &mut [u8; 16]) {
    let mut state = now_unix_ms()
        ^ (u64::from(std::process::id()) << 31)
        ^ (std::time::Instant::now().elapsed().as_nanos() as u64);
    for byte in bytes.iter_mut() {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *byte = (state >> 33) as u8;
    }
}

fn dummy_listener() -> std::net::TcpListener {
    std::net::TcpListener::bind(("127.0.0.1", 0)).expect("dummy listener")
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

/// Single-client accept loop; the target serves one DevTools at a time.
pub async fn serve(mut args: ServeArgs) {
    let shutdown = args.shutdown.clone();
    // Convert the blocking socket now that a Tokio reactor exists.
    args.listener.set_nonblocking(true).ok();
    let listener =
        tokio::net::TcpListener::from_std(std::mem::replace(&mut args.listener, dummy_listener()))
            .expect("listener converts to tokio");

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
        let _ = run_session(websocket, &mut args).await;
    }
}

async fn run_session<S>(
    websocket: tokio_tungstenite::WebSocketStream<S>,
    args: &mut ServeArgs,
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
    if let Err(code) = validate(&hello, &args.token) {
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
        if args.shutdown.load(Ordering::Relaxed) {
            break;
        }
        tokio::select! {
            incoming = source.next() => {
                match incoming {
                    Some(Ok(WsMessage::Text(text))) => {
                        match serde_json::from_str::<Message>(&text) {
                            Ok(Message::Request { request_id, body }) => {
                                if args.command_sender.try_send(commands::UiCommand { request_id, body }).is_err() {
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
                let receiver = args.reply_receiver.clone();
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
            event = args.telemetry_receiver.recv() => {
                let Some(event) = event else { break };
                let dropped_total = args.dropped.load(Ordering::Relaxed);
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
