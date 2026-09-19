use crate::inspector::{ConnectionState, InspectorModel, Shared};
use futures_util::{SinkExt, StreamExt};
use incular_devtools_protocol::{
    DiscoveryRecord, Hello, Message, PROTOCOL_VERSION, PeerKind, RequestMethod, ResponsePayload,
    TargetEvent,
};
use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

const REQUEST_QUEUE_CAPACITY: usize = 128;
const REQUEST_MESSAGE_LIMIT: usize = 64 * 1024;
const REQUEST_PAYLOAD_BUDGET: usize = 2 * 1024 * 1024;
const INCOMING_TARGET_MESSAGE_LIMIT: usize = 16 * 1024 * 1024;
const MAX_IN_FLIGHT_REQUESTS: usize = 64;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RequestTicket(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClientSendError {
    Full,
    Disconnected,
    Oversized,
    Exhausted,
}

struct QueuedRequest {
    ticket: RequestTicket,
    body: RequestMethod,
    permit: RequestBytePermit,
}

#[derive(Clone, Copy, Debug)]
enum ClientControl {
    Retry,
}

struct PendingRequest {
    _ticket: RequestTicket,
    body: RequestMethod,
    sent_at: Instant,
    _permit: Option<RequestBytePermit>,
}

struct RequestBytePermit {
    used: Arc<AtomicUsize>,
    bytes: usize,
}

impl RequestBytePermit {
    fn try_acquire(used: &Arc<AtomicUsize>, bytes: usize) -> Option<Self> {
        used.fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
            current
                .checked_add(bytes)
                .filter(|next| *next <= REQUEST_PAYLOAD_BUDGET)
        })
        .ok()?;
        Some(Self {
            used: Arc::clone(used),
            bytes,
        })
    }
}

impl Drop for RequestBytePermit {
    fn drop(&mut self) {
        self.used.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

#[derive(Default)]
struct ClientWorker {
    join: Mutex<Option<thread::JoinHandle<()>>>,
}

impl Drop for ClientWorker {
    fn drop(&mut self) {
        if let Ok(join) = self.join.get_mut()
            && let Some(join) = join.take()
        {
            let _ = join.join();
        }
    }
}

#[derive(Clone)]
pub(crate) struct ClientBridge {
    requests: tokio::sync::mpsc::Sender<QueuedRequest>,
    controls: tokio::sync::mpsc::Sender<ClientControl>,
    pub(crate) updates: Arc<tokio::sync::Notify>,
    next_ticket: Arc<AtomicU64>,
    model: Shared,
    _worker: Arc<ClientWorker>,
    request_payload_bytes: Arc<AtomicUsize>,
}
impl ClientBridge {
    pub(crate) fn send(&self, request: RequestMethod) -> Result<RequestTicket, ClientSendError> {
        if self
            .model
            .lock()
            .map_or(true, |state| state.connection != ConnectionState::Connected)
        {
            return Err(ClientSendError::Disconnected);
        }
        let bytes = serde_json::to_vec(&request).map_err(|_| ClientSendError::Oversized)?;
        if bytes.len() > REQUEST_MESSAGE_LIMIT {
            return Err(ClientSendError::Oversized);
        }
        let permit = RequestBytePermit::try_acquire(&self.request_payload_bytes, bytes.len())
            .ok_or(ClientSendError::Full)?;
        let ticket = self
            .next_ticket
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map(RequestTicket)
            .map_err(|_| ClientSendError::Exhausted)?;
        match self.requests.try_send(QueuedRequest {
            ticket,
            body: request,
            permit,
        }) {
            Ok(()) => Ok(ticket),
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => Err(ClientSendError::Full),
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                Err(ClientSendError::Disconnected)
            }
        }
    }

    pub(crate) fn send_or_report(&self, request: RequestMethod) {
        if let Err(error) = self.send(request) {
            note_request_error(
                &self.model,
                &self.updates,
                format!("request admission failed: {error:?}"),
            );
        }
    }

    pub(crate) fn retry(&self) {
        match self.controls.try_send(ClientControl::Retry) {
            Ok(()) => set_connection_state(
                &self.model,
                &self.updates,
                ConnectionState::Discovering,
                None,
            ),
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                note_request_error(&self.model, &self.updates, "retry already pending".into());
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                note_connection_error(
                    &self.model,
                    &self.updates,
                    "DevTools client has stopped".into(),
                );
            }
        }
    }
}

pub(crate) fn start_client(record: DiscoveryRecord, model: Shared) -> ClientBridge {
    let (requests, receiver) = tokio::sync::mpsc::channel(REQUEST_QUEUE_CAPACITY);
    let (controls, control_receiver) = tokio::sync::mpsc::channel(1);
    let updates = Arc::new(tokio::sync::Notify::new());
    let request_payload_bytes = Arc::new(AtomicUsize::new(0));
    let worker = match thread::Builder::new()
        .name("incular-devtools-ui-client".into())
        .spawn({
            let model = Arc::clone(&model);
            let updates = Arc::clone(&updates);
            move || run_client(record, model, receiver, control_receiver, updates)
        }) {
        Ok(join) => Arc::new(ClientWorker {
            join: Mutex::new(Some(join)),
        }),
        Err(error) => {
            note_connection_error(
                &model,
                &updates,
                format!("unable to start DevTools client thread: {error}"),
            );
            Arc::new(ClientWorker::default())
        }
    };
    ClientBridge {
        requests,
        controls,
        updates,
        next_ticket: Arc::new(AtomicU64::new(1)),
        model,
        _worker: worker,
        request_payload_bytes,
    }
}

fn run_client(
    record: DiscoveryRecord,
    model: Shared,
    mut requests: tokio::sync::mpsc::Receiver<QueuedRequest>,
    mut controls: tokio::sync::mpsc::Receiver<ClientControl>,
    updates: Arc<tokio::sync::Notify>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            return note_connection_error(&model, &updates, format!("Tokio runtime: {error}"));
        }
    };
    runtime.block_on(async move {
        let target_pid = record.pid;
        let mut next_record = Some(record);
        'service: loop {
            let record = match next_record.take() {
                Some(record) => record,
                None => loop {
                    tokio::select! {
                        control = controls.recv() => match control {
                            Some(ClientControl::Retry) => {
                                set_connection_state(
                                    &model,
                                    &updates,
                                    ConnectionState::Discovering,
                                    None,
                                );
                                let report = crate::session::scan_sessions();
                                for warning in report.warnings {
                                    note_request_error(
                                        &model,
                                        &updates,
                                        format!("discovery: {warning}"),
                                    );
                                }
                                if let Some(record) =
                                    crate::session::select_session(&report.sessions, Some(target_pid))
                                {
                                    break record;
                                }
                                note_connection_error(
                                    &model,
                                    &updates,
                                    format!("no live DevTools target found for pid {target_pid}"),
                                );
                            }
                            None => {
                                set_connection_state(
                                    &model,
                                    &updates,
                                    ConnectionState::Stopping,
                                    None,
                                );
                                break 'service;
                            }
                        },
                        request = requests.recv() => match request {
                            Some(_) => note_request_error(
                                &model,
                                &updates,
                                "request rejected: DevTools is disconnected; retry first".into(),
                            ),
                            None => {
                                set_connection_state(
                                    &model,
                                    &updates,
                                    ConnectionState::Stopping,
                                    None,
                                );
                                break 'service;
                            }
                        }
                    }
                },
            };
            if !run_connection(record, &model, &updates, &mut requests).await {
                set_connection_state(&model, &updates, ConnectionState::Stopping, None);
                break;
            }
            while requests.try_recv().is_ok() {
                note_request_error(
                    &model,
                    &updates,
                    "queued request abandoned after disconnect; it will not be replayed".into(),
                );
            }
        }
    });
}

async fn run_connection(
    record: DiscoveryRecord,
    model: &Shared,
    updates: &Arc<tokio::sync::Notify>,
    requests: &mut tokio::sync::mpsc::Receiver<QueuedRequest>,
) -> bool {
    if !(incular_devtools_protocol::MIN_SUPPORTED_PROTOCOL_VERSION..=PROTOCOL_VERSION)
        .contains(&record.protocol_version)
    {
        note_connection_error(
            model,
            updates,
            format!(
                "protocol mismatch: target uses {}, this DevTools supports {}..={PROTOCOL_VERSION}; rebuild the DevTools UI",
                record.protocol_version,
                incular_devtools_protocol::MIN_SUPPORTED_PROTOCOL_VERSION,
            ),
        );
        return true;
    }
    set_connection_state(model, updates, ConnectionState::Connecting, None);
    let websocket_config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig {
        max_message_size: Some(INCOMING_TARGET_MESSAGE_LIMIT),
        max_frame_size: Some(INCOMING_TARGET_MESSAGE_LIMIT),
        ..Default::default()
    };
    let Ok((websocket, _)) = tokio_tungstenite::connect_async_with_config(
        format!("ws://127.0.0.1:{}/", record.port),
        Some(websocket_config),
        false,
    )
    .await
    else {
        note_connection_error(model, updates, "connection refused".into());
        return true;
    };
    let (mut sink, mut source) = websocket.split();
    set_connection_state(model, updates, ConnectionState::Authenticating, None);
    let hello = Hello {
        protocol_version: PROTOCOL_VERSION,
        devtools_version: incular_devtools_protocol::DEVTOOLS_VERSION.into(),
        kind: PeerKind::Devtools,
        auth_token: record.auth_token,
    };
    let Ok(text) = serde_json::to_string(&hello) else {
        note_connection_error(model, updates, "handshake encoding failed".into());
        return true;
    };
    if sink
        .send(tokio_tungstenite::tungstenite::Message::Text(text))
        .await
        .is_err()
    {
        note_connection_error(model, updates, "handshake failed".into());
        return true;
    }
    let mut next_request = 1_u64;
    let mut pending = HashMap::<u64, PendingRequest>::new();
    let mut timeout_tick = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            incoming = source.next() => {
                let Some(Ok(message)) = incoming else {
                    report_pending_disconnect(model, updates, &pending);
                    note_connection_error(model, updates, "target disconnected".into());
                    return true;
                };
                let Ok(text) = message.into_text() else { continue }; let Ok(message) = serde_json::from_str::<Message>(&text) else { continue };
                if let Message::Rejection { code, message } = &message {
                    report_pending_disconnect(model, updates, &pending);
                    note_connection_error(model, updates, format!("target rejected connection ({code:?}): {message}"));
                    return true;
                }
                apply_message(model, updates, message, &mut sink, &mut next_request, &mut pending).await;
            }
            request = requests.recv() => {
                let Some(request) = request else { return false };
                if pending.len() >= MAX_IN_FLIGHT_REQUESTS {
                    note_request_error(
                        model,
                        updates,
                        "request rejected: DevTools already has 64 in-flight requests".into(),
                    );
                    continue;
                }
                if send_request(
                    &mut sink,
                    &mut next_request,
                    &mut pending,
                    request.ticket,
                    request.body,
                    Some(request.permit),
                ).await.is_err() {
                    report_pending_disconnect(model, updates, &pending);
                    note_connection_error(model, updates, "request send failed".into());
                    return true;
                }
            }
            _ = timeout_tick.tick() => {
                let now = Instant::now();
                let timed_out = pending
                    .iter()
                    .filter_map(|(id, request)|
                        (now.duration_since(request.sent_at) >= REQUEST_TIMEOUT).then_some(*id))
                    .collect::<Vec<_>>();
                for id in timed_out {
                    if let Some(request) = pending.remove(&id) {
                        let suffix = if request_is_mutation(&request.body) {
                            "; the target may have applied this mutation, so refresh state before retrying"
                        } else {
                            ""
                        };
                        note_request_error(
                            model,
                            updates,
                            format!(
                                "request {id} timed out after {}s{suffix}",
                                REQUEST_TIMEOUT.as_secs()
                            ),
                        );
                    }
                }
            }
        }
    }
    #[allow(unreachable_code)]
    true
}

async fn apply_message<S>(
    model: &Shared,
    updates: &Arc<tokio::sync::Notify>,
    message: Message,
    sink: &mut S,
    next: &mut u64,
    pending: &mut HashMap<u64, PendingRequest>,
) where
    S: futures_util::Sink<tokio_tungstenite::tungstenite::Message> + Unpin,
{
    let request = match &message {
        Message::Response { request_id: 0, .. } => None,
        Message::Response {
            request_id,
            payload,
        } => match pending.remove(request_id) {
            Some(request) => {
                if !response_matches_request(&request.body, payload) {
                    note_request_error(
                        model,
                        updates,
                        format!("ignored mismatched response for request id {request_id}"),
                    );
                    return;
                }
                Some(request.body)
            }
            None => {
                note_request_error(
                    model,
                    updates,
                    format!("ignored response for unknown request id {request_id}"),
                );
                return;
            }
        },
        _ => None,
    };
    match message {
        Message::Response {
            payload: Ok(ResponsePayload::TargetInfo(info)),
            request_id,
        } => {
            let info = *info;
            if !payload_fits(&info, InspectorModel::MAX_TARGET_INFO_PAYLOAD_BYTES) {
                note_request_error(
                    model,
                    updates,
                    "target info exceeded retained model budget".into(),
                );
                return;
            }
            let window = if let Ok(mut state) = model.lock() {
                state.connected = true;
                state.connection = ConnectionState::Connected;
                state.error = None;
                state.target = format!("pid {} · {}", info.pid, info.platform);
                state.target_info = Some(info.clone());
                state.windows = info.windows.clone();
                let live = state
                    .windows
                    .iter()
                    .map(|window| window.id)
                    .collect::<HashSet<_>>();
                state
                    .frame_arrivals
                    .retain(|window, _| live.contains(window));
                state.roots.retain(|window, _| live.contains(window));
                let previous_window = state.active_window;
                if state
                    .active_window
                    .is_none_or(|active| !state.windows.iter().any(|window| window.id == active))
                {
                    state.active_window = state.windows.first().map(|window| window.id);
                }
                if state.active_window != previous_window {
                    state.nodes.clear();
                    state.tree_payload_bytes = 0;
                    state.rows.clear();
                    state.expanded.clear();
                    state.selected = None;
                    state.hovered = None;
                    state.details = None;
                    state.tree_revision = 0;
                    state.tree_truncated = false;
                }
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
            if request_id == 0 && window.is_none() {
                let _ =
                    send_internal_request(sink, next, pending, RequestMethod::GetTargetInfo).await;
            } else if let Some(window) = window {
                let _ = send_internal_request(
                    sink,
                    next,
                    pending,
                    RequestMethod::GetWidgetTree { window },
                )
                .await;
                let _ =
                    send_internal_request(sink, next, pending, RequestMethod::ListSignals).await;
                send_request(
                    sink,
                    next,
                    pending,
                    RequestTicket(0),
                    RequestMethod::TakeMemorySnapshot {
                        label: "connected".into(),
                    },
                    None,
                )
                .await
                .ok();
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
            let requested_window = match request.as_ref() {
                Some(RequestMethod::GetWidgetTree { window }) => Some(*window),
                _ => None,
            };
            let selected = if let Ok(mut state) = model.lock() {
                if requested_window.is_some() && requested_window != state.active_window {
                    return;
                }
                state.apply_tree(tree_revision, deltas);
                state.tree_resync_pending = false;
                state.selected
            } else {
                None
            };
            if let Some(id) = selected {
                let _ = send_internal_request(
                    sink,
                    next,
                    pending,
                    RequestMethod::GetNodeDetails { id },
                )
                .await;
            }
        }
        Message::Response {
            payload: Ok(ResponsePayload::NodeDetails(details)),
            ..
        } => {
            if !payload_fits(&details, InspectorModel::MAX_DETAILS_PAYLOAD_BYTES) {
                note_request_error(
                    model,
                    updates,
                    "node details exceeded retained model budget".into(),
                );
                return;
            }
            if let Ok(mut state) = model.lock()
                && state.selected == Some(details.id)
                && state.active_window == Some(details.window)
            {
                state.details = Some(*details);
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
            if !payload_fits(&signals, InspectorModel::MAX_SIGNALS_PAYLOAD_BYTES) {
                note_request_error(
                    model,
                    updates,
                    "signal list exceeded retained model budget".into(),
                );
                return;
            }
            if let Ok(mut state) = model.lock() {
                state.signals = signals;
            }
        }
        Message::Response {
            payload: Ok(ResponsePayload::SignalSubscribers(subscribers)),
            ..
        } => {
            if !payload_fits(&subscribers, InspectorModel::MAX_SUBSCRIBERS_PAYLOAD_BYTES) {
                note_request_error(
                    model,
                    updates,
                    "signal subscribers exceeded retained model budget".into(),
                );
                return;
            }
            if let Ok(mut state) = model.lock()
                && match request.as_ref() {
                    Some(RequestMethod::GetSignalSubscribers { id }) => {
                        state.selected_signal == Some(*id)
                    }
                    _ => false,
                }
            {
                state.signal_subscribers = subscribers;
            }
        }
        Message::Response {
            payload: Ok(ResponsePayload::Edited),
            ..
        } => {
            // A successful edit follows the target's normal Signal::set path;
            // refresh bounded summaries rather than assuming a local value.
            let _ = send_internal_request(sink, next, pending, RequestMethod::ListSignals).await;
            let selected = model.lock().ok().and_then(|state| state.selected);
            if let Some(id) = selected {
                let _ = send_internal_request(
                    sink,
                    next,
                    pending,
                    RequestMethod::GetNodeDetails { id },
                )
                .await;
            }
        }
        Message::Response {
            payload: Ok(ResponsePayload::OverridesReset),
            ..
        } => {
            let selected = model.lock().ok().and_then(|state| state.selected);
            if let Some(id) = selected {
                let _ = send_internal_request(
                    sink,
                    next,
                    pending,
                    RequestMethod::GetNodeDetails { id },
                )
                .await;
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
        } => note_request_error(model, updates, format!("target error {code:?}: {message}")),
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
                let _ = send_internal_request(
                    sink,
                    next,
                    pending,
                    RequestMethod::GetWidgetTree { window },
                )
                .await;
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
            if let Ok(mut state) = model.lock()
                && state.active_window == Some(window)
            {
                state.apply_tree(tree_revision, deltas);
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
            let _ =
                send_internal_request(sink, next, pending, RequestMethod::GetNodeDetails { id })
                    .await;
            let _ = send_internal_request(
                sink,
                next,
                pending,
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
            let _ = send_internal_request(sink, next, pending, RequestMethod::GetTargetInfo).await;
        }
        Message::Event(TargetEvent::DroppedTelemetry { count }) => {
            let refresh = if let Ok(mut state) = model.lock() {
                state.push_console(
                    "warn",
                    "devtools",
                    format!("target dropped {count} telemetry events"),
                );
                if state.tree_resync_pending {
                    None
                } else {
                    state.tree_resync_pending = state.active_window.is_some();
                    state.active_window
                }
            } else {
                None
            };
            if let Some(window) = refresh
                && send_internal_request(
                    sink,
                    next,
                    pending,
                    RequestMethod::GetWidgetTree { window },
                )
                .await
                .is_err()
                && let Ok(mut state) = model.lock()
            {
                state.tree_resync_pending = false;
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
            note_request_error(model, updates, format!("target rejected request: {code:?}"))
        }
        _ => {}
    }
    let _ = request;
    updates.notify_one();
}

async fn send_internal_request<S>(
    sink: &mut S,
    next: &mut u64,
    pending: &mut HashMap<u64, PendingRequest>,
    body: RequestMethod,
) -> Result<(), ()>
where
    S: futures_util::Sink<tokio_tungstenite::tungstenite::Message> + Unpin,
{
    send_request(sink, next, pending, RequestTicket(0), body, None).await
}

async fn send_request<S>(
    sink: &mut S,
    next: &mut u64,
    pending: &mut HashMap<u64, PendingRequest>,
    ticket: RequestTicket,
    body: RequestMethod,
    permit: Option<RequestBytePermit>,
) -> Result<(), ()>
where
    S: futures_util::Sink<tokio_tungstenite::tungstenite::Message> + Unpin,
{
    let request_id = *next;
    *next = next.checked_add(1).ok_or(())?;
    let request = Message::Request {
        request_id,
        body: body.clone(),
    };
    let text = serde_json::to_string(&request).map_err(|_| ())?;
    if text.len() > REQUEST_MESSAGE_LIMIT {
        return Err(());
    }
    if pending.len() >= MAX_IN_FLIGHT_REQUESTS {
        return Err(());
    }
    pending.insert(
        request_id,
        PendingRequest {
            _ticket: ticket,
            body,
            sent_at: Instant::now(),
            _permit: permit,
        },
    );
    if sink
        .send(tokio_tungstenite::tungstenite::Message::Text(text))
        .await
        .is_err()
    {
        pending.remove(&request_id);
        return Err(());
    }
    Ok(())
}

pub(crate) fn note_connection_error(
    model: &Shared,
    updates: &Arc<tokio::sync::Notify>,
    message: String,
) {
    if let Ok(mut state) = model.lock() {
        state.push_console("error", "devtools", message.clone());
        state.error = Some(message);
        state.connected = false;
        state.connection = ConnectionState::Disconnected;
        state.selected = None;
        state.hovered = None;
        state.details = None;
        state.signal_subscribers.clear();
        state.select_mode = false;
        state.recording = false;
        state.tree_resync_pending = false;
    }
    updates.notify_one();
}

fn payload_fits(value: &impl serde::Serialize, limit: usize) -> bool {
    serde_json::to_vec(value).is_ok_and(|payload| payload.len() <= limit)
}

fn response_matches_request(
    request: &RequestMethod,
    payload: &Result<ResponsePayload, incular_devtools_protocol::ErrorCode>,
) -> bool {
    let Ok(payload) = payload else {
        return true;
    };
    if matches!(payload, ResponsePayload::Error { .. }) {
        return true;
    }
    match (request, payload) {
        (RequestMethod::GetTargetInfo, ResponsePayload::TargetInfo(_))
        | (RequestMethod::GetWidgetTree { .. }, ResponsePayload::WidgetTree { .. })
        | (RequestMethod::EditProperty { .. }, ResponsePayload::Edited)
        | (RequestMethod::StartInspectMode { .. }, ResponsePayload::Ok)
        | (RequestMethod::StopInspectMode { .. }, ResponsePayload::Ok)
        | (RequestMethod::HighlightNode { .. }, ResponsePayload::Ok)
        | (RequestMethod::SetDebugOption { .. }, ResponsePayload::Ok)
        | (RequestMethod::StartRecording, ResponsePayload::RecordingStarted)
        | (RequestMethod::StopRecording, ResponsePayload::RecordingStopped { .. })
        | (RequestMethod::ListSignals, ResponsePayload::Signals(_))
        | (RequestMethod::EditSignal { .. }, ResponsePayload::Edited)
        | (RequestMethod::ResetOverrides, ResponsePayload::OverridesReset) => true,
        (RequestMethod::GetNodeDetails { id }, ResponsePayload::NodeDetails(details)) => {
            details.id == *id
        }
        (
            RequestMethod::TakeMemorySnapshot { label },
            ResponsePayload::MemorySnapshot(snapshot),
        ) => snapshot.label == *label,
        (RequestMethod::GetSignalSubscribers { .. }, ResponsePayload::SignalSubscribers(_)) => true,
        (RequestMethod::ClearCache { which }, ResponsePayload::CacheCleared(report)) => {
            report.which == *which
        }
        (RequestMethod::SetAnimationSpeed { .. }, ResponsePayload::AnimationSpeedSet(_)) => true,
        (
            RequestMethod::SetProfilerMode { mode },
            ResponsePayload::ProfilerModeSet(response_mode),
        ) => mode == response_mode,
        _ => false,
    }
}

fn request_is_mutation(request: &RequestMethod) -> bool {
    !matches!(
        request,
        RequestMethod::GetTargetInfo
            | RequestMethod::GetWidgetTree { .. }
            | RequestMethod::GetNodeDetails { .. }
            | RequestMethod::TakeMemorySnapshot { .. }
            | RequestMethod::ListSignals
            | RequestMethod::GetSignalSubscribers { .. }
    )
}

fn report_pending_disconnect(
    model: &Shared,
    updates: &Arc<tokio::sync::Notify>,
    pending: &HashMap<u64, PendingRequest>,
) {
    let mutations = pending
        .values()
        .filter(|request| request_is_mutation(&request.body))
        .count();
    if mutations > 0 {
        note_request_error(
            model,
            updates,
            format!(
                "{mutations} in-flight mutation request(s) lost their response; effects may have occurred and will not be replayed"
            ),
        );
    }
}

fn set_connection_state(
    model: &Shared,
    updates: &Arc<tokio::sync::Notify>,
    connection: ConnectionState,
    error: Option<String>,
) {
    if let Ok(mut state) = model.lock() {
        state.connection = connection;
        state.connected = connection == ConnectionState::Connected;
        if let Some(error) = error {
            state.error = Some(error);
        } else if connection != ConnectionState::Disconnected {
            state.error = None;
        }
    }
    updates.notify_one();
}

pub(crate) fn note_request_error(
    model: &Shared,
    updates: &Arc<tokio::sync::Notify>,
    message: String,
) {
    if let Ok(mut state) = model.lock() {
        state.push_console("error", "devtools", message.clone());
        state.error = Some(message);
    }
    updates.notify_one();
}
