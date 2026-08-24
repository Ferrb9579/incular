//! Typed, versioned wire protocol between a running Incular application (the
//! *target*) and Incular DevTools.
//!
//! This crate intentionally depends only on `serde`: it must stay usable by
//! the standalone DevTools UI, the in-target agent, and offline tooling
//! without pulling in the framework runtime or renderer.

use serde::{Deserialize, Serialize};

/// Oldest wire protocol that remains compatible with this implementation.
/// Protocol 2 only adds typed property-edit requests, so protocol 1 clients
/// can continue to inspect a protocol 2 target.
pub const MIN_SUPPORTED_PROTOCOL_VERSION: u32 = 1;

/// Current wire protocol version.
pub const PROTOCOL_VERSION: u32 = 2;

/// DevTools implementation version carried in the handshake for diagnostics.
pub const DEVTOOLS_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerKind {
    Target,
    Devtools,
}

/// First message sent by either side after the WebSocket opens. The target
/// validates `auth_token` against its random session token before accepting
/// any further frame.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub protocol_version: u32,
    pub devtools_version: String,
    pub kind: PeerKind,
    pub auth_token: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    ProtocolVersionMismatch,
    Unauthorized,
    UnknownId,
    StaleId,
    Unsupported,
    InvalidRequest,
    InternalError,
}

// ---------------------------------------------------------------------------
// Opaque identifiers: raw arena indices and pointers never cross the wire.
// The generation field lets targets reject ids whose retained object was
// destroyed and whose slot was reused.
// ---------------------------------------------------------------------------

macro_rules! opaque_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name {
            index: u64,
            generation: u64,
        }
        impl $name {
            pub const fn new(index: u64, generation: u64) -> Self {
                Self { index, generation }
            }
            pub const fn index(&self) -> u64 {
                self.index
            }
            pub const fn generation(&self) -> u64 {
                self.generation
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(
                    f,
                    "{}:{}:{}",
                    stringify!($name),
                    self.index,
                    self.generation
                )
            }
        }
    };
}

opaque_id!(DevWindowId);
opaque_id!(DevWidgetId);
opaque_id!(DevRenderId);
opaque_id!(DevLayerId);
opaque_id!(DevSemanticsId);
opaque_id!(DevSignalId);

// ---------------------------------------------------------------------------
// Structured diagnostic values (closed set; no arbitrary JSON).
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugValue {
    Bool(bool),
    Int(i64),
    Uint(u64),
    Float(f64),
    /// Truncated string content; sensitive sources become [`Self::Redacted`].
    Str(String),
    Color(u8, u8, u8, u8),
    Enum(String),
    Size([f32; 2]),
    Offset([f32; 2]),
    Rect([f32; 4]),
    Insets([f32; 4]),
    Constraints {
        min_width: f32,
        max_width: f32,
        min_height: f32,
        max_height: f32,
    },
    Optional(Option<Box<DebugValue>>),
    List(Vec<DebugValue>),
    Redacted,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DebugProperty {
    pub name: String,
    pub value: DebugValue,
    /// `DEV OVERRIDE` active for this property.
    #[serde(default)]
    pub overridden: bool,
    #[serde(default)]
    pub editable: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PropertyChange {
    pub name: String,
    pub old: Option<DebugValue>,
    pub new: Option<DebugValue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum InvalidationReason {
    SignalWrite {
        signal: DevSignalId,
        name: String,
        old: Option<String>,
        new: Option<String>,
    },
    ParentReconciliation {
        changed: Vec<PropertyChange>,
    },
    WidgetConfigurationChanged,
    EnvironmentChanged {
        field: String,
        old: Option<String>,
        new: Option<String>,
    },
    LocaleChanged,
    WindowMetricsChanged,
    ConstraintsChanged,
    Animation,
    TaskCompletion,
    Navigation,
    Restoration,
    ManualInvalidation,
    Mounted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

// ---------------------------------------------------------------------------
// Widget tree snapshot + incremental deltas.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WidgetNode {
    pub id: DevWidgetId,
    pub parent: Option<DevWidgetId>,
    pub type_name: String,
    pub key: Option<String>,
    /// Short label for Text-like leaves (truncated; sensitive → redacted).
    pub label: Option<String>,
    pub child_ids: Vec<DevWidgetId>,
    /// Bumped whenever inspected configuration changes; deltas compare this
    /// instead of resending unchanged subtrees.
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum TreeDelta {
    Snapshot {
        window: DevWindowId,
        root: Box<WidgetNode>,
        nodes: Vec<WidgetNode>,
        truncated: bool,
    },
    Insert {
        node: Box<WidgetNode>,
    },
    Remove {
        id: DevWidgetId,
    },
    Move {
        id: DevWidgetId,
        parent: DevWidgetId,
        position: usize,
    },
    Update {
        node: Box<WidgetNode>,
    },
}

// ---------------------------------------------------------------------------
// Details, telemetry, signals.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ElementState {
    pub mount_generation: u64,
    pub builds: u64,
    pub layouts: u64,
    pub paints: u64,
    pub composites: u64,
    pub semantic_updates: u64,
    pub constraints: Option<DebugValue>,
    pub size: Option<[f32; 2]>,
    pub offset: Option<[f32; 2]>,
    pub world_bounds: Option<[f32; 4]>,
    /// Resolved retained baseline, when this render object establishes one.
    #[serde(default)]
    pub baseline: Option<f32>,
    /// The actual rectangular clip used by a retained clip layer, expressed
    /// in the same target coordinate space as `world_bounds`.
    #[serde(default)]
    pub clip: Option<[f32; 4]>,
}

/// Read-only geometry captured from the retained layout result.  This is a
/// snapshot, never a handle to a mutable render object; DevTools must not run
/// layout again merely to answer an inspection request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutInspection {
    pub node: DevWidgetId,
    pub parent: Option<DevWidgetId>,
    pub incoming_constraints: Option<DebugValue>,
    pub resolved_size: [f32; 2],
    pub local_offset: [f32; 2],
    /// Kurbo affine coefficients `[xx, yx, xy, yy, dx, dy]`.
    pub local_transform: [f32; 6],
    /// Retained world transform, expressed using Kurbo's coefficients.
    pub world_transform: [f32; 6],
    pub world_bounds: [f32; 4],
    pub content_bounds: [f32; 4],
    pub padding: Option<[f32; 4]>,
    pub clip: Option<[f32; 4]>,
    pub baseline: Option<f32>,
    pub details: LayoutDetails,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutHistoryEntry {
    pub sequence: u64,
    pub old_constraints: Option<DebugValue>,
    pub new_constraints: DebugValue,
    pub old_size: [f32; 2],
    pub new_size: [f32; 2],
    pub cause: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkReasons {
    pub layout: Option<String>,
    pub paint: Option<String>,
    pub composite: Option<String>,
}

/// Layout-strategy-specific, compact inspection data.  Every coordinate and
/// allocation comes from the completed retained layout pass; the DevTools UI
/// only visualizes this data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum LayoutDetails {
    Box,
    Flex {
        axis: String,
        available_main: Option<f32>,
        non_flex_extent: f32,
        flexible_extent: f32,
        used_extent: f32,
        remaining_extent: f32,
        overflow: f32,
        children: Vec<FlexChildInspection>,
    },
    Stack {
        alignment: [f32; 2],
        indexed_active: Option<usize>,
        children: Vec<StackChildInspection>,
    },
    Positioned {
        left: Option<f32>,
        right: Option<f32>,
        top: Option<f32>,
        bottom: Option<f32>,
        width: Option<f32>,
        height: Option<f32>,
    },
    Transform {
        matrix: [f32; 6],
        determinant: f32,
        invertible: bool,
    },
    Fitted {
        fit: String,
        alignment: [f32; 2],
        source_size: Option<[f32; 2]>,
        destination_size: [f32; 2],
        matrix: [f32; 6],
        determinant: f32,
        invertible: bool,
    },
    Scroll {
        axis: String,
        viewport_extent: f32,
        content_extent: f32,
        offset: f32,
        min_scroll: f32,
        max_scroll: f32,
    },
    LazyViewport {
        item_count: usize,
        materialized_start: usize,
        materialized_end: usize,
        materialized_items: usize,
        viewport_extent: f32,
        cache_extent: f32,
        scroll_offset: f32,
    },
    Text {
        text_length: usize,
        max_lines: Option<usize>,
        overflow: String,
        line_count: Option<usize>,
    },
    Custom {
        layout_kind: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FlexChildInspection {
    pub node: DevWidgetId,
    pub index: usize,
    pub type_name: String,
    pub size: [f32; 2],
    pub offset: [f32; 2],
    pub flex: Option<u32>,
    pub fit: Option<String>,
    /// The actual max constraint delivered to this child in the main axis.
    pub allocated_main_extent: Option<f32>,
    pub actual_main_extent: f32,
    pub cross_extent: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StackChildInspection {
    pub node: DevWidgetId,
    pub index: usize,
    pub type_name: String,
    pub bounds: [f32; 4],
    pub painted: bool,
    pub left: Option<f32>,
    pub right: Option<f32>,
    pub top: Option<f32>,
    pub bottom: Option<f32>,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeDetails {
    pub id: DevWidgetId,
    pub window: DevWindowId,
    pub type_name: String,
    pub key: Option<String>,
    pub state: ElementState,
    /// Exact retained layout state for the selected node.  This is optional
    /// only for compatibility with targets that cannot expose a live render.
    #[serde(default)]
    pub layout: Option<LayoutInspection>,
    /// Last relevant retained geometry changes, populated only while Deep
    /// capture is active and bounded by the target.
    #[serde(default)]
    pub layout_history: Vec<LayoutHistoryEntry>,
    #[serde(default)]
    pub work_reasons: WorkReasons,
    pub properties: Vec<DebugProperty>,
    /// Bounded diff captured from the last compatible widget configuration
    /// update. It uses only curated inspectable properties.
    #[serde(default)]
    pub property_changes: Vec<PropertyChange>,
    /// Bounded coalesced invalidations observed before this selected node's
    /// next BUILD. `invalidation` remains the latest cause for compatibility.
    #[serde(default)]
    pub invalidation_causes: Vec<InvalidationReason>,
    /// Debug-visible signals this live element actually consumed during its
    /// current reactive subscription. Stale/unmounted entries are omitted.
    #[serde(default)]
    pub consumed_signals: Vec<DevSignalId>,
    pub invalidation: Option<InvalidationReason>,
    pub render: Option<DevRenderId>,
    pub semantics: Option<DevSemanticsId>,
    pub source: Option<SourceLocation>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FrameTimingsWire {
    pub event_processing: u32,
    pub runtime_messages: u32,
    pub build: u32,
    pub layout: u32,
    pub composite: u32,
    pub semantics: u32,
    pub paint: u32,
    pub cpu_total: u32,
    pub prepare: u32,
    pub encode: u32,
    pub submit: u32,
    /// GPU main-pass time in microseconds when timestamp queries resolve.
    pub gpu_us: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameRecordEvent {
    pub window: DevWindowId,
    pub frame: u64,
    pub timings: FrameTimingsWire,
    /// Actual monitor-derived CPU frame budget when the platform exposes a
    /// refresh rate. `None` means the target cannot report one.
    #[serde(default)]
    pub budget_us: Option<u32>,
    pub over_budget: bool,
    pub draw_calls: u32,
    pub instances: u32,
    pub upload_bytes: u64,
    pub pipelines_created: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevtoolsProfilerMode {
    #[default]
    Basic,
    Performance,
    Deep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TracePhase {
    Build,
    Layout,
    Paint,
    Semantics,
    Composite,
}

/// One real, hierarchical per-node timing recorded only in Deep mode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraceEvent {
    pub node: DevWidgetId,
    pub phase: TracePhase,
    pub parent: Option<u32>,
    pub start_us: u32,
    pub duration_us: u32,
}

/// Bounded trace batch for one target/window frame. `truncated` and
/// `dropped_events` make partial data impossible to mistake for a full trace.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeepFrameTrace {
    pub window: DevWindowId,
    pub frame: u64,
    pub events: Vec<TraceEvent>,
    pub truncated: bool,
    pub dropped_events: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ResourceCounts {
    pub elements: usize,
    pub render_objects: usize,
    pub layers: usize,
    pub semantics_nodes: usize,
    pub signals: usize,
    pub tasks_active: usize,
    pub glyph_atlas_pages: usize,
    pub image_resources: usize,
    pub gradient_resources: usize,
    pub path_meshes: usize,
    pub offscreen_bytes: usize,
    pub effect_cached_bytes: usize,
    pub rss_mb: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MemorySnapshot {
    pub label: String,
    pub counts: ResourceCounts,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignalSummary {
    pub id: DevSignalId,
    pub name: Option<String>,
    pub type_name: String,
    /// Monotonic value generation; increments only after a real signal write.
    #[serde(default)]
    pub generation: u64,
    pub write_count: u64,
    pub subscriber_count: usize,
    pub last_write_summary: Option<String>,
    pub editable: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignalSubscriber {
    pub signal: DevSignalId,
    pub element: DevWidgetId,
    pub path: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugOption {
    LayoutBounds,
    LayoutBoundsSubtree,
    LayoutBoundsWholeWindow,
    PaddingContent,
    Baselines,
    Clips,
    HitTestRegions,
    SemanticsBounds,
    ScrollViewports,
    LayerBoundaries,
    RepaintRainbow,
    FlashRebuilds,
    HighlightBuild,
    HighlightLayout,
    HighlightPaint,
    HighlightSemantics,
    HighlightComposite,
    HighlightOversizedImages,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheKind {
    TextLayouts,
    GlyphAtlas,
    Images,
    Paths,
    Gradients,
    Offscreen,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditableValue {
    Bool(bool),
    Int(i64),
    Uint(u64),
    Float(f64),
    Str(String),
    Color(u8, u8, u8, u8),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CacheReport {
    pub which: CacheKind,
    pub entries: usize,
    pub hits: u64,
    pub misses: u64,
}

// ---------------------------------------------------------------------------
// Envelope: requests, responses, target events.
// ---------------------------------------------------------------------------

/// DevTools → target. Every request produces exactly one
/// [`ResponsePayload`] correlated by `request_id`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "method")]
pub enum RequestMethod {
    GetTargetInfo,
    GetWidgetTree {
        window: DevWindowId,
    },
    GetNodeDetails {
        id: DevWidgetId,
    },
    EditProperty {
        id: DevWidgetId,
        name: String,
        value: DebugValue,
    },
    StartInspectMode {
        window: DevWindowId,
    },
    StopInspectMode {
        window: DevWindowId,
    },
    HighlightNode {
        window: DevWindowId,
        id: Option<DevWidgetId>,
    },
    SetDebugOption {
        name: DebugOption,
        enabled: bool,
    },
    SetProfilerMode {
        mode: DevtoolsProfilerMode,
    },
    StartRecording,
    StopRecording,
    TakeMemorySnapshot {
        label: String,
    },
    ListSignals,
    GetSignalSubscribers {
        id: DevSignalId,
    },
    EditSignal {
        id: DevSignalId,
        value: EditableValue,
    },
    ResetOverrides,
    ClearCache {
        which: CacheKind,
    },
    SetAnimationSpeed {
        scale: f32,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "result")]
pub enum ResponsePayload {
    Ok,
    TargetInfo(Box<TargetInfo>),
    WidgetTree {
        deltas: Vec<TreeDelta>,
        tree_revision: u64,
    },
    NodeDetails(Box<NodeDetails>),
    RecordingStarted,
    RecordingStopped {
        frames: u64,
    },
    MemorySnapshot(MemorySnapshot),
    Signals(Vec<SignalSummary>),
    SignalSubscribers(Vec<SignalSubscriber>),
    Edited,
    OverridesReset,
    CacheCleared(CacheReport),
    AnimationSpeedSet(f32),
    ProfilerModeSet(DevtoolsProfilerMode),
    Error {
        code: ErrorCode,
        message: String,
    },
}

/// Target → DevTools unsolicited events (bounded queues apply upstream).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "event")]
pub enum TargetEvent {
    /// Incremental retained-tree update for a window that the client has
    /// explicitly inspected. Targets never serialize these while no client
    /// has subscribed to a tree.
    WidgetTreeDeltas {
        window: DevWindowId,
        deltas: Vec<TreeDelta>,
        tree_revision: u64,
    },
    FrameRecord(FrameRecordEvent),
    DeepTrace(DeepFrameTrace),
    WidgetSelectedByUser {
        window: DevWindowId,
        id: DevWidgetId,
    },
    Log {
        level: String,
        target: String,
        message: String,
    },
    WindowsChanged,
    DroppedTelemetry {
        count: u64,
    },
    InspectModeEnded {
        window: DevWindowId,
    },
}

/// One wire frame after the handshake.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Message {
    Request {
        request_id: u64,
        body: RequestMethod,
    },
    Response {
        request_id: u64,
        payload: Result<ResponsePayload, ErrorCode>,
    },
    Event(TargetEvent),
    /// Human-readable rejection during handshake; the peer closes after.
    Rejection {
        code: ErrorCode,
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TargetInfo {
    pub protocol_version: u32,
    pub framework_version: String,
    pub pid: u32,
    pub executable: String,
    pub platform: String,
    pub windows: Vec<WindowSummary>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowSummary {
    pub id: DevWindowId,
    pub title: String,
    pub logical_size: [f32; 2],
    pub scale_factor: f64,
}

/// Per-user discovery record written by devtools-enabled targets so the
/// DevTools launcher can list running applications without port scanning.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiscoveryRecord {
    pub pid: u32,
    pub app_name: String,
    pub port: u16,
    /// Session token; the discovery file lives in a user-only directory, and
    /// DevTools still authenticates over the socket before any data flows.
    pub auth_token: String,
    pub started_unix_ms: u64,
    pub protocol_version: u32,
}

/// Validates a handshake from the *other* side. Targets call this with the
/// client hello (expecting [`PeerKind::Devtools`]) and vice versa.
pub fn check_hello(
    hello: &Hello,
    expected_kind: PeerKind,
    expected_token: Option<&str>,
) -> Result<(), ErrorCode> {
    if !(MIN_SUPPORTED_PROTOCOL_VERSION..=PROTOCOL_VERSION).contains(&hello.protocol_version) {
        return Err(ErrorCode::ProtocolVersionMismatch);
    }
    if let Some(token) = expected_token {
        // Constant-time-ish comparison: equal-length early exit leaks only
        // length, which the wire already reveals.
        if !token_eq(&hello.auth_token, token) {
            return Err(ErrorCode::Unauthorized);
        }
    }
    if hello.kind != expected_kind {
        return Err(ErrorCode::InvalidRequest);
    }
    Ok(())
}

fn token_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_round_trips_and_validates() {
        let hello = Hello {
            protocol_version: PROTOCOL_VERSION,
            devtools_version: DEVTOOLS_VERSION.to_owned(),
            kind: PeerKind::Devtools,
            auth_token: "tok".into(),
        };
        let json = serde_json::to_string(&hello).unwrap();
        let parsed: Hello = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, hello);
        assert_eq!(
            check_hello(&parsed, PeerKind::Devtools, Some("tok")),
            Ok(())
        );
    }

    #[test]
    fn wrong_version_rejected() {
        let hello = Hello {
            protocol_version: PROTOCOL_VERSION + 1,
            devtools_version: "x".into(),
            kind: PeerKind::Devtools,
            auth_token: "t".into(),
        };
        assert_eq!(
            check_hello(&hello, PeerKind::Devtools, None),
            Err(ErrorCode::ProtocolVersionMismatch)
        );
    }

    #[test]
    fn previous_additive_protocol_version_remains_compatible() {
        let hello = Hello {
            protocol_version: MIN_SUPPORTED_PROTOCOL_VERSION,
            devtools_version: "older-ui".into(),
            kind: PeerKind::Devtools,
            auth_token: "t".into(),
        };
        assert_eq!(check_hello(&hello, PeerKind::Devtools, Some("t")), Ok(()));
    }

    #[test]
    fn wrong_token_rejected() {
        let hello = Hello {
            protocol_version: PROTOCOL_VERSION,
            devtools_version: "x".into(),
            kind: PeerKind::Devtools,
            auth_token: "bad".into(),
        };
        assert_eq!(
            check_hello(&hello, PeerKind::Devtools, Some("good")),
            Err(ErrorCode::Unauthorized)
        );
    }

    #[test]
    fn request_response_round_trip() {
        let message = Message::Request {
            request_id: 7,
            body: RequestMethod::HighlightNode {
                window: DevWindowId::new(1, 2),
                id: Some(DevWidgetId::new(3, 4)),
            },
        };
        let json = serde_json::to_string(&message).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, message);

        let response = Message::Response {
            request_id: 7,
            payload: Err(ErrorCode::StaleId),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("stale_id"));
        assert_eq!(serde_json::from_str::<Message>(&json).unwrap(), response);
    }

    #[test]
    fn malformed_message_rejected() {
        let result = serde_json::from_str::<Message>("{\"type\":\"nonsense\"}");
        assert!(result.is_err());
    }

    #[test]
    fn deep_trace_round_trip_preserves_hierarchy_and_drop_state() {
        let message = Message::Event(TargetEvent::DeepTrace(DeepFrameTrace {
            window: DevWindowId::new(2, 1),
            frame: 9,
            events: vec![
                TraceEvent {
                    node: DevWidgetId::new(1, 0),
                    phase: TracePhase::Layout,
                    parent: None,
                    start_us: 4,
                    duration_us: 20,
                },
                TraceEvent {
                    node: DevWidgetId::new(2, 0),
                    phase: TracePhase::Layout,
                    parent: Some(0),
                    start_us: 6,
                    duration_us: 8,
                },
            ],
            truncated: true,
            dropped_events: 3,
        }));
        let encoded = serde_json::to_string(&message).unwrap();
        assert_eq!(serde_json::from_str::<Message>(&encoded).unwrap(), message);
    }

    #[test]
    fn tree_delta_round_trip() {
        let delta = TreeDelta::Snapshot {
            window: DevWindowId::new(0, 1),
            root: Box::new(WidgetNode {
                id: DevWidgetId::new(0, 1),
                parent: None,
                type_name: "Column".into(),
                key: None,
                label: None,
                child_ids: vec![DevWidgetId::new(1, 1)],
                revision: 3,
            }),
            nodes: Vec::new(),
            truncated: false,
        };
        let json = serde_json::to_string(&delta).unwrap();
        assert_eq!(serde_json::from_str::<TreeDelta>(&json).unwrap(), delta);
    }

    #[test]
    fn opaque_ids_hide_raw_values_and_detect_generations() {
        let a = DevWidgetId::new(5, 9);
        let b = DevWidgetId::new(5, 10);
        assert_ne!(a, b, "generation distinguishes reused slots");
        let text = a.to_string();
        assert!(text.contains("5") && text.contains("9"));
    }
}
