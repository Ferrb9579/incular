use crate::inspector::{InspectorModel, Shared};
use futures_util::{SinkExt, StreamExt};
use incular_devtools_protocol::{
    DiscoveryRecord, Hello, Message, PROTOCOL_VERSION, PeerKind, RequestMethod, ResponsePayload,
    TargetEvent,
};
use std::sync::{Arc, Mutex, mpsc};
use std::{thread, time::Duration};

#[derive(Clone)]
pub(crate) struct ClientBridge {
    pub(crate) requests: mpsc::Sender<RequestMethod>,
    pub(crate) updates: Arc<Mutex<mpsc::Receiver<()>>>,
}
impl ClientBridge {
    pub(crate) fn send(&self, request: RequestMethod) {
        let _ = self.requests.send(request);
    }
}

pub(crate) fn start_client(record: DiscoveryRecord, model: Shared) -> ClientBridge {
    let (requests, receiver) = mpsc::channel();
    let (updates, update_receiver) = mpsc::channel();
    thread::Builder::new()
        .name("incular-devtools-ui-client".into())
        .spawn(move || run_client(record, model, receiver, updates))
        .expect("start DevTools client thread");
    ClientBridge {
        requests,
        updates: Arc::new(Mutex::new(update_receiver)),
    }
}

pub(crate) fn run_client(
    record: DiscoveryRecord,
    model: Shared,
    requests: mpsc::Receiver<RequestMethod>,
    updates: mpsc::Sender<()>,
) {
    if !(incular_devtools_protocol::MIN_SUPPORTED_PROTOCOL_VERSION..=PROTOCOL_VERSION)
        .contains(&record.protocol_version)
    {
        return note_error(
            &model,
            &updates,
            format!(
                "protocol mismatch: target uses {}, this DevTools supports {}..={PROTOCOL_VERSION}; rebuild the DevTools UI",
                record.protocol_version,
                incular_devtools_protocol::MIN_SUPPORTED_PROTOCOL_VERSION,
            ),
        );
    }
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return note_error(&model, &updates, format!("Tokio runtime: {error}")),
    };
    runtime.block_on(async move {
        let Ok((websocket, _)) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}/", record.port)).await else { return note_error(&model, &updates, "connection refused".into()) };
        let (mut sink, mut source) = websocket.split();
        let hello = Hello { protocol_version: PROTOCOL_VERSION, devtools_version: incular_devtools_protocol::DEVTOOLS_VERSION.into(), kind: PeerKind::Devtools, auth_token: record.auth_token };
        let Ok(text) = serde_json::to_string(&hello) else { return note_error(&model, &updates, "handshake encoding failed".into()) };
        if sink.send(tokio_tungstenite::tungstenite::Message::Text(text)).await.is_err() { return note_error(&model, &updates, "handshake failed".into()) }
        let mut next_request = 1; let mut command_tick = tokio::time::interval(Duration::from_millis(20));
        loop { tokio::select! {
            incoming = source.next() => {
                let Some(Ok(message)) = incoming else { return note_error(&model, &updates, "target disconnected".into()) };
                let Ok(text) = message.into_text() else { continue }; let Ok(message) = serde_json::from_str::<Message>(&text) else { continue };
                if let Message::Rejection { code, message } = &message {
                    return note_error(&model, &updates, format!("target rejected connection ({code:?}): {message}"));
                }
                apply_message(&model, &updates, message, &mut sink, &mut next_request).await;
            }
            _ = command_tick.tick() => while let Ok(request) = requests.try_recv() { send_request(&mut sink, &mut next_request, request).await; }
        }}
    });
}

async fn apply_message<S>(
    model: &Shared,
    updates: &mpsc::Sender<()>,
    message: Message,
    sink: &mut S,
    next: &mut u64,
) where
    S: futures_util::Sink<tokio_tungstenite::tungstenite::Message> + Unpin,
{
    match message {
        Message::Response {
            payload: Ok(ResponsePayload::TargetInfo(info)),
            ..
        } => {
            let info = *info;
            let needs_window_request = info.windows.is_empty();
            let window = if let Ok(mut state) = model.lock() {
                state.connected = true;
                state.error = None;
                state.target = format!("pid {} · {}", info.pid, info.platform);
                state.target_info = Some(info.clone());
                state.windows = info.windows.clone();
                state.active_window = state.windows.first().map(|window| window.id);
                state.tree_retry_sent = false;
                state.push_console(
                    "info",
                    "devtools",
                    format!("connected to {} ({})", info.executable, info.platform),
                );
                state.active_window
            } else {
                None
            };
            if needs_window_request {
                send_request(sink, next, RequestMethod::GetTargetInfo).await;
            } else if let Some(window) = window {
                send_request(sink, next, RequestMethod::GetWidgetTree { window }).await;
                send_request(sink, next, RequestMethod::ListSignals).await;
                send_request(
                    sink,
                    next,
                    RequestMethod::TakeMemorySnapshot {
                        label: "connected".into(),
                    },
                )
                .await;
            }
        }
        Message::Response {
            payload:
                Ok(ResponsePayload::WidgetTree {
                    deltas,
                    tree_revision,
                }),
            ..
        } => {
            let selected = if let Ok(mut state) = model.lock() {
                state.apply_tree(tree_revision, deltas);
                state.selected
            } else {
                None
            };
            if let Some(id) = selected {
                send_request(sink, next, RequestMethod::GetNodeDetails { id }).await;
            }
        }
        Message::Response {
            payload: Ok(ResponsePayload::NodeDetails(details)),
            ..
        } => {
            if let Ok(mut state) = model.lock() {
                if state.selected == Some(details.id) && state.active_window == Some(details.window)
                {
                    state.details = Some(*details);
                }
            }
        }
        Message::Response {
            payload: Ok(ResponsePayload::MemorySnapshot(snapshot)),
            ..
        } => {
            if let Ok(mut state) = model.lock() {
                match snapshot.label.as_str() {
                    "A" => state.memory_a = Some(snapshot.clone()),
                    "B" => state.memory_b = Some(snapshot.clone()),
                    _ => {}
                }
                state.memory = Some(snapshot);
            }
        }
        Message::Response {
            payload: Ok(ResponsePayload::Signals(signals)),
            ..
        } => {
            if let Ok(mut state) = model.lock() {
                state.signals = signals;
            }
        }
        Message::Response {
            payload: Ok(ResponsePayload::SignalSubscribers(subscribers)),
            ..
        } => {
            if let Ok(mut state) = model.lock() {
                state.signal_subscribers = subscribers;
            }
        }
        Message::Response {
            payload: Ok(ResponsePayload::Edited),
            ..
        } => {
            // A successful edit follows the target's normal Signal::set path;
            // refresh bounded summaries rather than assuming a local value.
            send_request(sink, next, RequestMethod::ListSignals).await;
            let selected = model.lock().ok().and_then(|state| state.selected);
            if let Some(id) = selected {
                send_request(sink, next, RequestMethod::GetNodeDetails { id }).await;
            }
        }
        Message::Response {
            payload: Ok(ResponsePayload::OverridesReset),
            ..
        } => {
            let selected = model.lock().ok().and_then(|state| state.selected);
            if let Some(id) = selected {
                send_request(sink, next, RequestMethod::GetNodeDetails { id }).await;
            }
        }
        Message::Response {
            payload: Ok(ResponsePayload::ProfilerModeSet(mode)),
            ..
        } => {
            if let Ok(mut state) = model.lock() {
                state.profiler_mode = mode;
            }
        }
        Message::Response {
            payload: Ok(ResponsePayload::AnimationSpeedSet(scale)),
            ..
        } => {
            if let Ok(mut state) = model.lock() {
                state.animation_scale = Some(scale);
            }
        }
        Message::Response {
            payload: Ok(ResponsePayload::RecordingStarted),
            ..
        } => {
            if let Ok(mut state) = model.lock() {
                state.recording = true;
                state.frames.clear();
                state.deep_traces.clear();
                state.deep_trace_events = 0;
                state.selected_frame = None;
                state.selected_range = None;
                state.range_anchor = None;
                state.timeline_offset = 0;
            }
        }
        Message::Response {
            payload: Ok(ResponsePayload::RecordingStopped { .. }),
            ..
        } => {
            if let Ok(mut state) = model.lock() {
                state.recording = false;
            }
        }
        Message::Response {
            payload: Ok(ResponsePayload::Error { code, message }),
            ..
        } => note_error(model, updates, format!("target error {code:?}: {message}")),
        Message::Event(TargetEvent::FrameRecord(frame)) => {
            let retry_window = if let Ok(mut state) = model.lock() {
                state.note_frame_arrival(frame.window);
                let retry_window = (!state.tree_retry_sent && state.rows.is_empty())
                    .then_some(state.active_window)
                    .flatten();
                if retry_window.is_some() {
                    state.tree_retry_sent = true;
                }
                if state.frames.len() == InspectorModel::FRAME_HISTORY {
                    state.frames.pop_front();
                }
                state.frames.push_back(frame);
                retry_window
            } else {
                None
            };
            if let Some(window) = retry_window {
                send_request(sink, next, RequestMethod::GetWidgetTree { window }).await;
            }
        }
        Message::Event(TargetEvent::DeepTrace(trace)) => {
            if let Ok(mut state) = model.lock() {
                state.push_deep_trace(trace);
            }
        }
        Message::Event(TargetEvent::WidgetTreeDeltas {
            window,
            deltas,
            tree_revision,
        }) => {
            if let Ok(mut state) = model.lock() {
                if state.active_window == Some(window) {
                    state.apply_tree(tree_revision, deltas);
                }
            }
        }
        Message::Event(TargetEvent::WidgetSelectedByUser { window, id }) => {
            if let Ok(mut state) = model.lock() {
                state.active_window = Some(window);
                state.selected = Some(id);
                state.hovered = None;
                state.select_mode = false;
                state.reveal(id);
            }
            send_request(sink, next, RequestMethod::GetNodeDetails { id }).await;
            send_request(
                sink,
                next,
                RequestMethod::HighlightNode {
                    window,
                    id: Some(id),
                },
            )
            .await;
        }
        Message::Event(TargetEvent::Log {
            level,
            target,
            message,
        }) => {
            if let Ok(mut state) = model.lock() {
                state.push_console(level, target, message.clone());
                if message.contains("recording stopped") {
                    state.recording = false;
                    state.error = Some(message);
                }
            }
        }
        Message::Event(TargetEvent::WindowsChanged) => {
            send_request(sink, next, RequestMethod::GetTargetInfo).await;
        }
        Message::Event(TargetEvent::DroppedTelemetry { count }) => {
            if let Ok(mut state) = model.lock() {
                state.push_console(
                    "warn",
                    "devtools",
                    format!("target dropped {count} telemetry events"),
                );
            }
        }
        Message::Event(TargetEvent::InspectModeEnded { .. }) => {
            if let Ok(mut state) = model.lock() {
                state.select_mode = false;
            }
        }
        Message::Response {
            payload: Err(code), ..
        }
        | Message::Rejection { code, .. } => {
            note_error(model, updates, format!("target rejected request: {code:?}"))
        }
        _ => {}
    }
    let _ = updates.send(());
}

async fn send_request<S>(sink: &mut S, next: &mut u64, body: RequestMethod)
where
    S: futures_util::Sink<tokio_tungstenite::tungstenite::Message> + Unpin,
{
    let request = Message::Request {
        request_id: *next,
        body,
    };
    *next = next.wrapping_add(1);
    if let Ok(text) = serde_json::to_string(&request) {
        let _ = sink
            .send(tokio_tungstenite::tungstenite::Message::Text(text))
            .await;
    }
}

pub(crate) fn note_error(model: &Shared, updates: &mpsc::Sender<()>, message: String) {
    if let Ok(mut state) = model.lock() {
        state.push_console("error", "devtools", message.clone());
        state.error = Some(message);
        state.connected = false;
    }
    let _ = updates.send(());
}
