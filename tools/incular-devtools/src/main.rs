//! Standalone Incular DevTools, built with Incular itself.
//!
//! The inspector retains data rows, not widget rows. A 100k-node target thus
//! keeps a compact id/depth index while [`VirtualList`] creates only rows at
//! the viewport.

use futures_util::{SinkExt, StreamExt};
use incular::prelude::*;
use incular_devtools_protocol::{
    DebugOption, DebugProperty, DebugValue, DeepFrameTrace, DevSignalId, DevWidgetId, DevWindowId,
    DevtoolsProfilerMode, DiscoveryRecord, EditableValue, FrameRecordEvent, Hello, LayoutDetails,
    MemorySnapshot, Message, NodeDetails, PROTOCOL_VERSION, PeerKind, RequestMethod,
    ResponsePayload, SignalSubscriber, SignalSummary, TargetEvent, TracePhase, TreeDelta,
    WidgetNode, WindowSummary,
};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet, VecDeque},
    rc::Rc,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

type Shared = Arc<Mutex<InspectorModel>>;

#[derive(Clone, Debug, PartialEq, Eq)]
struct TreeRow {
    id: DevWidgetId,
    depth: u16,
}

#[derive(Clone, Debug, PartialEq)]
struct FlameBox {
    event: usize,
    node: DevWidgetId,
    phase: TracePhase,
    x: f32,
    width: f32,
    depth: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RankedTrace {
    node: DevWidgetId,
    phase: TracePhase,
    total_us: u64,
    count: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum TraceRange {
    CurrentFrame,
    SelectedRange,
    #[default]
    EntireRecording,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ToolView {
    #[default]
    Inspector,
    Performance,
    Memory,
    Signals,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum InspectorSection {
    #[default]
    Properties,
    Layout,
    Signals,
    Why,
    Semantics,
}

fn initial_tool_view() -> ToolView {
    match std::env::var("INCULAR_DEVTOOLS_VIEW")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "performance" => ToolView::Performance,
        "memory" => ToolView::Memory,
        "signals" => ToolView::Signals,
        _ => ToolView::Inspector,
    }
}

const APP_BACKGROUND: Color = Color::rgba(9, 13, 21, 255);
const SURFACE: Color = Color::rgba(17, 24, 36, 255);
const SURFACE_RAISED: Color = Color::rgba(23, 32, 47, 255);
const BORDER: Color = Color::rgba(48, 63, 86, 255);
const CONTROL: Color = Color::rgba(42, 54, 74, 255);
const CONTROL_ACTIVE: Color = Color::rgba(58, 99, 187, 255);
const PRIMARY: Color = Color::rgba(74, 119, 224, 255);
const DANGER: Color = Color::rgba(178, 69, 78, 255);
const TEXT_PRIMARY: Color = Color::rgba(238, 243, 252, 255);
const TEXT_MUTED: Color = Color::rgba(153, 169, 194, 255);
const SUCCESS: Color = Color::rgba(74, 205, 151, 255);

fn ui_text(value: impl Into<String>, size: f32, color: Color) -> Widget {
    Text::new(value)
        .style(TextStyle {
            size,
            color,
            ..TextStyle::default()
        })
        .into()
}

fn gap(width: f32, height: f32) -> Widget {
    Widget::fixed_box(Size::new(width, height), Color::TRANSPARENT)
}

fn compact_button(
    label: impl Into<String>,
    selected: bool,
    callback: impl Fn() + 'static,
) -> Widget {
    Button::new(label)
        .size(Size::new(0., 34.))
        .padding(EdgeInsets::symmetric(12., 7.))
        .label_style(TextStyle {
            size: 13.,
            color: TEXT_PRIMARY,
            ..TextStyle::default()
        })
        .color(if selected { CONTROL_ACTIVE } else { CONTROL })
        .on_press(callback)
        .into()
}

fn section(title: impl Into<String>, description: impl Into<String>, child: Widget) -> Widget {
    DecoratedBox::new(Padding::all(
        16.,
        Column::new([
            ui_text(title, 16., TEXT_PRIMARY),
            gap(1., 4.),
            ui_text(description, 12., TEXT_MUTED),
            gap(1., 14.),
            child,
        ]),
    ))
    .background(SURFACE_RAISED)
    .border(Border::new(1., BORDER))
    .radius(10.)
    .into()
}

fn flamegraph_boxes(
    trace: &DeepFrameTrace,
    phase: Option<TracePhase>,
    width: f32,
) -> Vec<FlameBox> {
    let selected = trace
        .events
        .iter()
        .enumerate()
        .filter(|(_, event)| phase.is_none_or(|phase| event.phase == phase))
        .collect::<Vec<_>>();
    let Some(start) = selected.iter().map(|(_, event)| event.start_us).min() else {
        return Vec::new();
    };
    let end = selected
        .iter()
        .map(|(_, event)| event.start_us.saturating_add(event.duration_us))
        .max()
        .unwrap_or(start);
    let extent = end.saturating_sub(start).max(1) as f32;
    selected
        .into_iter()
        .map(|(index, event)| {
            let mut depth = 0_u16;
            let mut parent = event.parent;
            while let Some(parent_index) = parent {
                let Some(parent_event) = trace.events.get(parent_index as usize) else {
                    break;
                };
                if phase.is_none_or(|phase| parent_event.phase == phase) {
                    depth = depth.saturating_add(1);
                }
                parent = parent_event.parent;
            }
            FlameBox {
                event: index,
                node: event.node,
                phase: event.phase,
                x: (event.start_us.saturating_sub(start) as f32 / extent) * width,
                width: (event.duration_us as f32 / extent * width).max(1.),
                depth,
            }
        })
        .collect()
}

fn rank_traces<'a>(
    traces: impl IntoIterator<Item = &'a DeepFrameTrace>,
    phase: Option<TracePhase>,
) -> Vec<RankedTrace> {
    let mut totals = HashMap::<(DevWidgetId, TracePhase), (u64, u32)>::new();
    for event in traces.into_iter().flat_map(|trace| trace.events.iter()) {
        if phase.is_some_and(|phase| phase != event.phase) {
            continue;
        }
        let total = totals.entry((event.node, event.phase)).or_default();
        total.0 = total.0.saturating_add(u64::from(event.duration_us));
        total.1 = total.1.saturating_add(1);
    }
    let mut ranked = totals
        .into_iter()
        .map(|((node, phase), (total_us, count))| RankedTrace {
            node,
            phase,
            total_us,
            count,
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .total_us
            .cmp(&left.total_us)
            .then_with(|| right.count.cmp(&left.count))
            .then_with(|| left.node.index().cmp(&right.node.index()))
    });
    ranked
}

/// Protocol-facing inspector state. It remains UI-framework independent so
/// generation handling, deltas, and virtualization can be tested cheaply.
#[derive(Default)]
struct InspectorModel {
    connected: bool,
    error: Option<String>,
    target: String,
    windows: Vec<WindowSummary>,
    active_window: Option<DevWindowId>,
    nodes: HashMap<DevWidgetId, WidgetNode>,
    roots: HashMap<DevWindowId, DevWidgetId>,
    expanded: HashSet<DevWidgetId>,
    rows: Vec<TreeRow>,
    selected: Option<DevWidgetId>,
    hovered: Option<DevWidgetId>,
    select_mode: bool,
    details: Option<NodeDetails>,
    frames: VecDeque<FrameRecordEvent>,
    deep_traces: VecDeque<DeepFrameTrace>,
    deep_trace_events: usize,
    profiler_mode: DevtoolsProfilerMode,
    recording: bool,
    selected_frame: Option<(DevWindowId, u64)>,
    range_anchor: Option<u64>,
    selected_range: Option<(u64, u64)>,
    trace_range: TraceRange,
    timeline_offset: usize,
    timeline_visible: usize,
    memory: Option<MemorySnapshot>,
    memory_a: Option<MemorySnapshot>,
    memory_b: Option<MemorySnapshot>,
    signals: Vec<SignalSummary>,
    selected_signal: Option<DevSignalId>,
    signal_subscribers: Vec<SignalSubscriber>,
    debug_options: HashSet<DebugOption>,
    animation_scale: Option<f32>,
    tree_retry_sent: bool,
    tree_revision: u64,
    search: String,
}

fn editable_value(signal: &SignalSummary, input: &str) -> Option<EditableValue> {
    match signal.type_name.rsplit("::").next()? {
        "bool" => input.parse().ok().map(EditableValue::Bool),
        "i64" => input.parse().ok().map(EditableValue::Int),
        "u64" => input.parse().ok().map(EditableValue::Uint),
        "f64" => input.parse().ok().map(EditableValue::Float),
        "String" => Some(EditableValue::Str(input.to_owned())),
        _ => None,
    }
}

impl InspectorModel {
    const FRAME_HISTORY: usize = 300;
    const MAX_TRACE_EVENTS: usize = 200_000;

    fn push_deep_trace(&mut self, mut trace: DeepFrameTrace) {
        if trace.events.len() > Self::MAX_TRACE_EVENTS {
            let dropped = trace.events.len() - Self::MAX_TRACE_EVENTS;
            trace.events.truncate(Self::MAX_TRACE_EVENTS);
            trace.truncated = true;
            trace.dropped_events = trace
                .dropped_events
                .saturating_add(u32::try_from(dropped).unwrap_or(u32::MAX));
        }
        while self.deep_traces.len() >= Self::FRAME_HISTORY
            || self.deep_trace_events.saturating_add(trace.events.len()) > Self::MAX_TRACE_EVENTS
        {
            let Some(removed) = self.deep_traces.pop_front() else {
                break;
            };
            self.deep_trace_events = self.deep_trace_events.saturating_sub(removed.events.len());
        }
        self.deep_trace_events = self.deep_trace_events.saturating_add(trace.events.len());
        self.deep_traces.push_back(trace);
    }

    fn memory_diff_lines(&self) -> Vec<String> {
        let (Some(a), Some(b)) = (&self.memory_a, &self.memory_b) else {
            return Vec::new();
        };
        let signed = |before: usize, after: usize| -> String {
            let delta = after as i128 - before as i128;
            format!("{delta:+}")
        };
        vec![
            format!("Memory diff: {} → {}", a.label, b.label),
            format!(
                "  RSS {} MB → {} MB ({})",
                a.counts.rss_mb,
                b.counts.rss_mb,
                signed(a.counts.rss_mb, b.counts.rss_mb)
            ),
            format!(
                "  elements {} → {} ({}) · render objects {} → {} ({})",
                a.counts.elements,
                b.counts.elements,
                signed(a.counts.elements, b.counts.elements),
                a.counts.render_objects,
                b.counts.render_objects,
                signed(a.counts.render_objects, b.counts.render_objects),
            ),
            format!(
                "  semantics {} → {} ({}) · signals {} → {} ({}) · tasks {} → {} ({})",
                a.counts.semantics_nodes,
                b.counts.semantics_nodes,
                signed(a.counts.semantics_nodes, b.counts.semantics_nodes),
                a.counts.signals,
                b.counts.signals,
                signed(a.counts.signals, b.counts.signals),
                a.counts.tasks_active,
                b.counts.tasks_active,
                signed(a.counts.tasks_active, b.counts.tasks_active),
            ),
        ]
    }

    fn apply_tree(&mut self, revision: u64, deltas: impl IntoIterator<Item = TreeDelta>) {
        self.tree_revision = revision;
        for delta in deltas {
            match delta {
                TreeDelta::Snapshot {
                    window,
                    root,
                    nodes,
                    ..
                } => {
                    self.active_window.get_or_insert(window);
                    if self.active_window == Some(window) {
                        self.nodes.clear();
                        self.roots.insert(window, root.id);
                        self.expanded.insert(root.id);
                        self.selected.get_or_insert(root.id);
                        self.nodes.insert(root.id, *root);
                        self.nodes
                            .extend(nodes.into_iter().map(|node| (node.id, node)));
                    }
                }
                TreeDelta::Insert { node } | TreeDelta::Update { node } => {
                    self.nodes.insert(node.id, *node);
                }
                TreeDelta::Remove { id } => self.remove_subtree(id),
                TreeDelta::Move {
                    id,
                    parent,
                    position,
                } => self.move_node(id, parent, position),
            }
        }
        self.rebuild_rows();
    }

    fn remove_subtree(&mut self, id: DevWidgetId) {
        let mut work = vec![id];
        while let Some(current) = work.pop() {
            if let Some(node) = self.nodes.remove(&current) {
                work.extend(node.child_ids);
            }
            self.expanded.remove(&current);
            if self.selected == Some(current) {
                self.selected = None;
                self.details = None;
            }
        }
        for node in self.nodes.values_mut() {
            node.child_ids.retain(|child| *child != id);
        }
    }

    fn move_node(&mut self, id: DevWidgetId, parent: DevWidgetId, position: usize) {
        for node in self.nodes.values_mut() {
            node.child_ids.retain(|child| *child != id);
        }
        if let Some(node) = self.nodes.get_mut(&id) {
            node.parent = Some(parent);
        }
        if let Some(parent_node) = self.nodes.get_mut(&parent) {
            parent_node
                .child_ids
                .insert(position.min(parent_node.child_ids.len()), id);
        }
    }

    fn rebuild_rows(&mut self) {
        self.rows.clear();
        let Some(window) = self.active_window else {
            return;
        };
        let Some(root) = self.roots.get(&window).copied() else {
            return;
        };
        let query = self.search.to_ascii_lowercase();
        let mut work = vec![(root, 0_u16)];
        while let Some((id, depth)) = work.pop() {
            let Some(node) = self.nodes.get(&id) else {
                continue;
            };
            let matches = query.is_empty()
                || node.type_name.to_ascii_lowercase().contains(&query)
                || node
                    .label
                    .as_deref()
                    .is_some_and(|text| text.to_ascii_lowercase().contains(&query));
            if matches {
                self.rows.push(TreeRow { id, depth });
            }
            if self.expanded.contains(&id) || !query.is_empty() {
                for child in node.child_ids.iter().rev() {
                    work.push((*child, depth.saturating_add(1)));
                }
            }
        }
    }

    fn reveal(&mut self, id: DevWidgetId) {
        let mut current = self.nodes.get(&id).and_then(|node| node.parent);
        while let Some(parent) = current {
            self.expanded.insert(parent);
            current = self.nodes.get(&parent).and_then(|node| node.parent);
        }
        self.rebuild_rows();
    }

    fn toggle_expanded(&mut self, id: DevWidgetId) -> bool {
        if self
            .nodes
            .get(&id)
            .is_none_or(|node| node.child_ids.is_empty())
        {
            return false;
        }
        if !self.expanded.insert(id) {
            self.expanded.remove(&id);
        }
        self.rebuild_rows();
        true
    }

    fn collapse_all(&mut self) {
        self.expanded.clear();
        self.rebuild_rows();
    }

    fn expand_all(&mut self) {
        self.expanded = self
            .nodes
            .iter()
            .filter_map(|(id, node)| (!node.child_ids.is_empty()).then_some(*id))
            .collect();
        self.rebuild_rows();
    }

    fn row_label(&self, row: &TreeRow) -> String {
        let Some(node) = self.nodes.get(&row.id) else {
            return "<stale node>".into();
        };
        let label = node
            .label
            .as_ref()
            .map(|label| format!("  {label}"))
            .unwrap_or_default();
        format!("{}{label}", node.type_name)
    }

    fn details_lines(&self, section: InspectorSection) -> Vec<String> {
        let Some(details) = &self.details else {
            let Some(selected) = self.selected else {
                return vec!["Select a widget to inspect its properties.".into()];
            };
            let Some(node) = self.nodes.get(&selected) else {
                return vec!["The selected widget is no longer retained.".into()];
            };
            return vec![
                format!("{}  {}", node.type_name, node.key.as_deref().unwrap_or("")),
                format!(
                    "id: {} · children: {} · revision: {}",
                    node.id,
                    node.child_ids.len(),
                    node.revision
                ),
                node.label.as_ref().map_or_else(
                    || "No semantic label".into(),
                    |label| format!("label: {label}"),
                ),
                "Select a rendered child to inspect detailed retained layout diagnostics.".into(),
            ];
        };
        let mut lines = vec![format!(
            "{}  {}",
            details.type_name,
            details.key.as_deref().unwrap_or("")
        )];
        match section {
            InspectorSection::Properties => {
                lines.push(format!("id: {} · render {:?}", details.id, details.render));
                lines.push(format!(
                    "bounds: {:?} · size: {:?} · offset: {:?}",
                    details.state.world_bounds, details.state.size, details.state.offset
                ));
                lines.push(format!(
                    "BUILD {} · LAYOUT {} · PAINT {} · COMPOSITE {}",
                    details.state.builds,
                    details.state.layouts,
                    details.state.paints,
                    details.state.composites
                ));
                if let Some(source) = &details.source {
                    lines.push(format!(
                        "source: {}:{}:{}",
                        source.file, source.line, source.column
                    ));
                }
                lines.extend(details.properties.iter().map(property_line));
            }
            InspectorSection::Layout => {
                if let Some(layout) = &details.layout {
                    lines.push(format!(
                        "resolved {:.1} × {:.1} · local ({:.1}, {:.1})",
                        layout.resolved_size[0],
                        layout.resolved_size[1],
                        layout.local_offset[0],
                        layout.local_offset[1]
                    ));
                    lines.push(layout.incoming_constraints.as_ref().map_or_else(
                        || "incoming constraints unavailable".into(),
                        |constraints| format!("incoming: {}", debug_value(constraints)),
                    ));
                    lines.push(format!(
                        "world ({:.1}, {:.1}) {:.1} × {:.1} · baseline {:?} · clip {:?}",
                        layout.world_bounds[0],
                        layout.world_bounds[1],
                        layout.world_bounds[2],
                        layout.world_bounds[3],
                        layout.baseline,
                        layout.clip
                    ));
                    if let Some(padding) = layout.padding {
                        lines.push(format!(
                            "padding L{:.1} T{:.1} R{:.1} B{:.1}",
                            padding[0], padding[1], padding[2], padding[3]
                        ));
                    }
                    lines.extend(layout_detail_lines(&layout.details));
                } else {
                    lines.push("This retained node has no layout snapshot.".into());
                }
                if !details.layout_history.is_empty() {
                    lines.push("Recent Deep layout changes".into());
                    lines.extend(details.layout_history.iter().rev().take(16).map(|entry| {
                        format!(
                            "#{} {:.1}×{:.1} → {:.1}×{:.1} · {}",
                            entry.sequence,
                            entry.old_size[0],
                            entry.old_size[1],
                            entry.new_size[0],
                            entry.new_size[1],
                            entry.cause.as_deref().unwrap_or("cause unavailable")
                        )
                    }));
                }
            }
            InspectorSection::Signals => {
                if details.consumed_signals.is_empty() {
                    lines.push("No debug-visible Signals were consumed by this node.".into());
                } else {
                    lines.push("Consumed Signals".into());
                    lines.extend(details.consumed_signals.iter().map(|id| {
                        let name = self
                            .signals
                            .iter()
                            .find(|signal| signal.id == *id)
                            .and_then(|signal| signal.name.as_deref())
                            .unwrap_or("<unnamed>");
                        format!("{name} · {id}")
                    }));
                }
            }
            InspectorSection::Why => {
                lines.push(details.invalidation.as_ref().map_or_else(
                    || "No retained rebuild cause is available.".into(),
                    |reason| format!("Why did this rebuild? {reason:?}"),
                ));
                for cause in &details.invalidation_causes {
                    lines.push(format!("{cause:?}  →  {} BUILD", details.id));
                }
                for change in &details.property_changes {
                    lines.push(format!(
                        "{}: {} → {}",
                        change.name,
                        change
                            .old
                            .as_ref()
                            .map(debug_value)
                            .unwrap_or_else(|| "<unset>".into()),
                        change
                            .new
                            .as_ref()
                            .map(debug_value)
                            .unwrap_or_else(|| "<unset>".into()),
                    ));
                }
                for (phase, reason) in [
                    ("LAYOUT", details.work_reasons.layout.as_deref()),
                    ("PAINT", details.work_reasons.paint.as_deref()),
                    ("COMPOSITE", details.work_reasons.composite.as_deref()),
                ] {
                    if let Some(reason) = reason {
                        lines.push(format!("Why {phase}? {reason}"));
                    }
                }
            }
            InspectorSection::Semantics => {
                lines.push(format!("semantic node: {:?}", details.semantics));
                lines.push(format!(
                    "semantic updates: {}",
                    details.state.semantic_updates
                ));
                lines.push(format!(
                    "semantic/world bounds: {:?}",
                    details.state.world_bounds
                ));
                let semantic_properties = details.properties.iter().filter(|property| {
                    let name = property.name.to_ascii_lowercase();
                    name.contains("semantic") || name.contains("label") || name.contains("role")
                });
                lines.extend(semantic_properties.map(property_line));
            }
        }
        lines
    }
}

fn layout_detail_lines(details: &LayoutDetails) -> Vec<String> {
    match details {
        LayoutDetails::Box => vec!["  Box layout".into()],
        LayoutDetails::Flex {
            axis,
            available_main,
            non_flex_extent,
            flexible_extent,
            used_extent,
            remaining_extent,
            overflow,
            children,
        } => {
            let mut lines = vec![format!(
                "  Flex {axis}: available {} · non-flex {:.1} · flexible {:.1} · used {:.1} · free {:.1} · overflow {:.1}",
                available_main.map_or_else(|| "unbounded".into(), |value| format!("{value:.1}")),
                non_flex_extent,
                flexible_extent,
                used_extent,
                remaining_extent,
                overflow
            )];
            lines.extend(children.iter().map(|child| {
                format!(
                    "    #{} {}: {:.1} × {:.1} @ ({:.1}, {:.1}) · flex {:?} {:?} · allocation {:?}",
                    child.index,
                    child.type_name,
                    child.size[0],
                    child.size[1],
                    child.offset[0],
                    child.offset[1],
                    child.flex,
                    child.fit,
                    child.allocated_main_extent
                )
            }));
            lines
        }
        LayoutDetails::Stack {
            alignment,
            indexed_active,
            children,
        } => {
            let mut lines = vec![format!(
                "  Stack alignment ({:.1}, {:.1}) · active {:?}",
                alignment[0], alignment[1], indexed_active
            )];
            lines.extend(children.iter().map(|child| format!(
                "    #{} {}: ({:.1}, {:.1}) {:.1} × {:.1} · painted {} · anchors L{:?} R{:?} T{:?} B{:?}",
                child.index,
                child.type_name,
                child.bounds[0], child.bounds[1], child.bounds[2], child.bounds[3], child.painted,
                child.left, child.right, child.top, child.bottom
            )));
            lines
        }
        LayoutDetails::Positioned {
            left,
            right,
            top,
            bottom,
            width,
            height,
        } => vec![format!(
            "  Positioned: L{left:?} R{right:?} T{top:?} B{bottom:?} W{width:?} H{height:?}"
        )],
        LayoutDetails::Transform {
            matrix,
            determinant,
            invertible,
        } => vec![format!(
            "  Transform [{:.3} {:.3} {:.3} {:.3} {:.1} {:.1}] · det {:.3} · inverse {}",
            matrix[0],
            matrix[1],
            matrix[2],
            matrix[3],
            matrix[4],
            matrix[5],
            determinant,
            if *invertible {
                "available"
            } else {
                "unavailable"
            }
        )],
        LayoutDetails::Fitted {
            fit,
            alignment,
            source_size,
            destination_size,
            determinant,
            invertible,
            ..
        } => vec![format!(
            "  Fitted {fit}: source {source_size:?} → {:.1} × {:.1}, align ({:.1}, {:.1}), det {:.3}, inverse {}",
            destination_size[0],
            destination_size[1],
            alignment[0],
            alignment[1],
            determinant,
            if *invertible {
                "available"
            } else {
                "unavailable"
            }
        )],
        LayoutDetails::Scroll {
            viewport_extent,
            content_extent,
            offset,
            min_scroll,
            max_scroll,
            ..
        } => vec![format!(
            "  Scroll: viewport {:.1} · content {:.1} · offset {:.1} · range {:.1}..{:.1}",
            viewport_extent, content_extent, offset, min_scroll, max_scroll
        )],
        LayoutDetails::LazyViewport {
            item_count,
            materialized_start,
            materialized_end,
            materialized_items,
            viewport_extent,
            cache_extent,
            scroll_offset,
        } => vec![format!(
            "  Lazy viewport: {item_count} items · materialized {materialized_start}..{materialized_end} ({materialized_items}) · viewport {:.1} · cache {:.1} · offset {:.1}",
            viewport_extent, cache_extent, scroll_offset
        )],
        LayoutDetails::Text {
            text_length,
            max_lines,
            overflow,
            line_count,
        } => vec![format!(
            "  Text: {text_length} chars · {line_count:?} retained lines · max {max_lines:?} · {overflow}"
        )],
        LayoutDetails::Custom { layout_kind } => vec![format!("  retained layout: {layout_kind}")],
    }
}

fn property_line(property: &DebugProperty) -> String {
    let override_mark = if property.overridden {
        " [DEV OVERRIDE]"
    } else {
        ""
    };
    format!(
        "{}: {}{override_mark}",
        property.name,
        debug_value(&property.value)
    )
}

fn debug_value(value: &DebugValue) -> String {
    match value {
        DebugValue::Bool(value) => value.to_string(),
        DebugValue::Int(value) => value.to_string(),
        DebugValue::Uint(value) => value.to_string(),
        DebugValue::Float(value) => format!("{value:.3}"),
        DebugValue::Str(value) | DebugValue::Enum(value) => value.clone(),
        DebugValue::Color(r, g, b, a) => format!("#{r:02X}{g:02X}{b:02X}{a:02X}"),
        DebugValue::Size([width, height]) => format!("{width:.1} × {height:.1}"),
        DebugValue::Offset([x, y]) => format!("({x:.1}, {y:.1})"),
        DebugValue::Rect([x, y, width, height]) => {
            format!("({x:.1}, {y:.1}) {width:.1} × {height:.1}")
        }
        DebugValue::Insets([left, top, right, bottom]) => {
            format!("L{left:.1} T{top:.1} R{right:.1} B{bottom:.1}")
        }
        DebugValue::Constraints {
            min_width,
            max_width,
            min_height,
            max_height,
        } => format!("w {min_width:.1}..{max_width:.1}; h {min_height:.1}..{max_height:.1}"),
        DebugValue::Optional(Some(value)) => debug_value(value),
        DebugValue::Optional(None) => "none".into(),
        DebugValue::List(values) => format!("{} values", values.len()),
        DebugValue::Redacted => "<redacted>".into(),
    }
}

fn parse_debug_value(template: &DebugValue, input: &str) -> Option<DebugValue> {
    let input = input.trim();
    let floats = |expected: usize| -> Option<Vec<f32>> {
        let values = input
            .split([',', ' ', '×'])
            .filter(|part| !part.is_empty())
            .map(str::parse::<f32>)
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        (values.len() == expected && values.iter().all(|value| value.is_finite())).then_some(values)
    };
    match template {
        DebugValue::Bool(_) => input.parse().ok().map(DebugValue::Bool),
        DebugValue::Int(_) => input.parse().ok().map(DebugValue::Int),
        DebugValue::Uint(_) => input.parse().ok().map(DebugValue::Uint),
        DebugValue::Float(_) => input
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .map(DebugValue::Float),
        DebugValue::Str(_) => Some(DebugValue::Str(input.into())),
        DebugValue::Enum(_) => Some(DebugValue::Enum(input.into())),
        DebugValue::Color(_, _, _, _) => {
            let hex = input.strip_prefix('#').unwrap_or(input);
            if !matches!(hex.len(), 6 | 8) {
                return None;
            }
            let color = u32::from_str_radix(hex, 16).ok()?;
            let (r, g, b, a) = if hex.len() == 6 {
                (
                    ((color >> 16) & 0xff) as u8,
                    ((color >> 8) & 0xff) as u8,
                    (color & 0xff) as u8,
                    255,
                )
            } else {
                (
                    ((color >> 24) & 0xff) as u8,
                    ((color >> 16) & 0xff) as u8,
                    ((color >> 8) & 0xff) as u8,
                    (color & 0xff) as u8,
                )
            };
            Some(DebugValue::Color(r, g, b, a))
        }
        DebugValue::Size(_) => floats(2).map(|v| DebugValue::Size([v[0], v[1]])),
        DebugValue::Offset(_) => floats(2).map(|v| DebugValue::Offset([v[0], v[1]])),
        DebugValue::Rect(_) => floats(4).map(|v| DebugValue::Rect([v[0], v[1], v[2], v[3]])),
        DebugValue::Insets(_) => floats(4).map(|v| DebugValue::Insets([v[0], v[1], v[2], v[3]])),
        DebugValue::Constraints { .. }
        | DebugValue::Optional(_)
        | DebugValue::List(_)
        | DebugValue::Redacted => None,
    }
}

#[derive(Clone)]
struct ClientBridge {
    requests: mpsc::Sender<RequestMethod>,
    updates: Arc<Mutex<mpsc::Receiver<()>>>,
}
impl ClientBridge {
    fn send(&self, request: RequestMethod) {
        let _ = self.requests.send(request);
    }
}

fn sessions_dir() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("dev", "incular", "incular-devtools")
        .map(|dirs| dirs.data_dir().join("sessions"))
}

fn list_sessions() -> Vec<DiscoveryRecord> {
    let mut sessions = Vec::new();
    let Some(dir) = sessions_dir() else {
        return sessions;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return sessions;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        match std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<DiscoveryRecord>(&text).ok())
        {
            Some(record) if std::path::Path::new(&format!("/proc/{}", record.pid)).exists() => {
                sessions.push(record)
            }
            _ => {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    sessions.sort_by_key(|record| record.started_unix_ms);
    sessions
}

fn requested_target_pid(
    args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
) -> Option<u32> {
    let mut arguments = args.into_iter();
    while let Some(argument) = arguments.next() {
        if argument.as_ref() == "--target-pid"
            && let Some(pid) = arguments
                .next()
                .and_then(|value| value.as_ref().to_str()?.parse().ok())
        {
            return Some(pid);
        }
    }
    None
}

fn select_session(
    sessions: &[DiscoveryRecord],
    target_pid: Option<u32>,
) -> Option<DiscoveryRecord> {
    target_pid.map_or_else(
        || sessions.last().cloned(),
        |pid| sessions.iter().find(|record| record.pid == pid).cloned(),
    )
}

fn main() {
    let sessions = list_sessions();
    let target_pid = requested_target_pid(std::env::args_os());
    let Some(record) = select_session(&sessions, target_pid) else {
        if let Some(pid) = target_pid {
            eprintln!("no live Incular DevTools target found for pid {pid}");
        } else {
            eprintln!("no DevTools targets discovered; run a devtools-enabled app with --devtools");
        }
        return;
    };
    let shared = Arc::new(Mutex::new(InspectorModel::default()));
    let bridge = start_client(record, Arc::clone(&shared));
    let tick = Signal::new(0_u64);
    let pending = Rc::new(Cell::new(false));
    let search = TextEditingController::new();
    let signal_value = TextEditingController::new();
    let property_value = TextEditingController::new();
    let property_binding = Rc::new(RefCell::new(None::<(DevWidgetId, String)>));
    let tool_view = Signal::new(initial_tool_view());
    let inspector_section = Signal::new(InspectorSection::Properties);
    let tree_scroll = ScrollController::new();
    let inspector_scroll = ScrollController::new();
    let app_shared = Arc::clone(&shared);
    let app_bridge = bridge.clone();
    let app_tick = tick.clone();
    let app_pending = pending.clone();
    let app_search = search.clone();
    let app_signal_value = signal_value.clone();
    let app_property_value = property_value.clone();
    let app_property_binding = property_binding.clone();
    let app_tool_view = tool_view.clone();
    let app_inspector_section = inspector_section.clone();
    let app_tree_scroll = tree_scroll.clone();
    let app_inspector_scroll = inspector_scroll.clone();
    let app = Application::new_with_options(
        WindowOptions {
            title: "Incular DevTools".into(),
            initial_logical_size: Size::new(1440., 900.),
            minimum_logical_size: Some(Size::new(960., 640.)),
            ..WindowOptions::default()
        },
        move |cx| {
            // Only this tiny UI-thread Signal changes. The model remains shared
            // and bounded, avoiding full tree copies every DevTools repaint.
            let _ = app_tick.get();
            let active_view = app_tool_view.get();
            let active_inspector_section = app_inspector_section.get();
            if !app_pending.replace(true) {
                let updates = app_bridge.updates.clone();
                let tick = app_tick.clone();
                let pending = app_pending.clone();
                cx.spawn_blocking(
                    move || {
                        updates
                            .lock()
                            .ok()
                            .is_some_and(|rx| rx.recv_timeout(Duration::from_millis(125)).is_ok())
                    },
                    move |updated, _| {
                        pending.set(false);
                        if matches!(updated, Ok(true)) {
                            tick.update(|value| *value = value.wrapping_add(1));
                        }
                    },
                );
            }
            let (
                header,
                connected,
                windows,
                active_window,
                row_count,
                select_mode,
                has_selection,
                details,
                selected_details,
                frames,
                memory,
                memory_diff,
                signals,
                selected_signal,
                signal_subscribers,
                debug_options,
                animation_scale,
                profiler_mode,
                recording,
                flame_boxes,
                ranked,
                trace_status,
                selected_frame_detail,
                selected_frame,
                trace_range,
                selected_range,
                error,
            ) = {
                let state = app_shared.lock().expect("inspector state");
                let header = if state.connected {
                    format!("Connected to {}", state.target)
                } else {
                    "Connecting to target".into()
                };
                let visible_frames = state.timeline_visible.max(6);
                let frames = state
                    .frames
                    .iter()
                    .filter(|frame| Some(frame.window) == state.active_window)
                    .rev()
                    .skip(state.timeline_offset)
                    .take(visible_frames)
                    .cloned()
                    .collect::<Vec<_>>();
                let memory = state.memory.as_ref().map(|snapshot| {
                    format!(
                        "{}: RSS {} MB · elements {} · render {} · semantics {} · signals {}",
                        snapshot.label,
                        snapshot.counts.rss_mb,
                        snapshot.counts.elements,
                        snapshot.counts.render_objects,
                        snapshot.counts.semantics_nodes,
                        snapshot.counts.signals
                    )
                });
                let selected_trace = state
                    .selected_frame
                    .and_then(|(window, frame)| {
                        state
                            .deep_traces
                            .iter()
                            .find(|trace| trace.window == window && trace.frame == frame)
                    })
                    .or_else(|| {
                        state
                            .deep_traces
                            .iter()
                            .rev()
                            .find(|trace| Some(trace.window) == state.active_window)
                    });
                let flame_boxes = if active_view == ToolView::Performance {
                    selected_trace
                        .map(|trace| flamegraph_boxes(trace, None, 900.))
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                let ranked = if active_view == ToolView::Performance {
                    rank_traces(
                        state.deep_traces.iter().filter(|trace| {
                            Some(trace.window) == state.active_window
                                && match state.trace_range {
                                    TraceRange::CurrentFrame => {
                                        selected_trace.is_some_and(|selected| {
                                            selected.frame == trace.frame
                                                && selected.window == trace.window
                                        })
                                    }
                                    TraceRange::SelectedRange => {
                                        state.selected_range.is_some_and(|(start, end)| {
                                            (start.min(end)..=start.max(end)).contains(&trace.frame)
                                        })
                                    }
                                    TraceRange::EntireRecording => true,
                                }
                        }),
                        None,
                    )
                } else {
                    Vec::new()
                };
                let trace_status = selected_trace.map(|trace| {
                    format!(
                        "Deep frame #{} · {} events{}",
                        trace.frame,
                        trace.events.len(),
                        if trace.truncated {
                            format!(" · truncated, {} dropped", trace.dropped_events)
                        } else {
                            String::new()
                        }
                    )
                });
                let selected_frame_detail = state
                .selected_frame
                .and_then(|(window, selected)| state.frames.iter().find(|frame| frame.window == window && frame.frame == selected))
                .map(|frame| {
                    format!(
                        "Frame #{} · CPU {}µs · budget {} · input {} runtime {} BUILD {} LAYOUT {} PAINT {} SEMANTICS {} COMPOSITE {} · renderer prepare {} encode {} submit {} · GPU {} · draws {} instances {} uploads {}B",
                        frame.frame,
                        frame.timings.cpu_total,
                        frame.budget_us.map_or_else(|| "unavailable".into(), |value| format!("{value}µs")),
                        frame.timings.event_processing,
                        frame.timings.runtime_messages,
                        frame.timings.build,
                        frame.timings.layout,
                        frame.timings.paint,
                        frame.timings.semantics,
                        frame.timings.composite,
                        frame.timings.prepare,
                        frame.timings.encode,
                        frame.timings.submit,
                        frame.timings.gpu_us.map_or_else(|| "unavailable".into(), |value| format!("{value:.0}µs")),
                        frame.draw_calls,
                        frame.instances,
                        frame.upload_bytes,
                    )
                });
                (
                    header,
                    state.connected,
                    state.windows.clone(),
                    state.active_window,
                    state.rows.len(),
                    state.select_mode,
                    state.selected.is_some(),
                    state.details_lines(active_inspector_section),
                    state.details.clone(),
                    frames,
                    memory,
                    state.memory_diff_lines(),
                    state.signals.clone(),
                    state.selected_signal.and_then(|id| {
                        state.signals.iter().find(|signal| signal.id == id).cloned()
                    }),
                    state.signal_subscribers.clone(),
                    state.debug_options.clone(),
                    state.animation_scale.unwrap_or(1.),
                    state.profiler_mode,
                    state.recording,
                    flame_boxes,
                    ranked,
                    trace_status,
                    selected_frame_detail,
                    state.selected_frame,
                    state.trace_range,
                    state.selected_range,
                    state.error.clone(),
                )
            };
            let mut inspector_controls = Vec::new();
            let mut debug_controls = Vec::new();
            let mut animation_controls = Vec::new();
            let mut performance_controls = Vec::new();
            let mut timeline_controls = Vec::new();
            let mut performance_body = Vec::new();
            let mut memory_controls = Vec::new();
            let mut memory_body = Vec::new();
            let mut signal_body = Vec::new();
            if let Some(error) = error {
                inspector_controls.push(ui_text(format!("Connection error: {error}"), 13., DANGER));
            }
            let bridge = app_bridge.clone();
            signal_body.push(compact_button("Refresh signal list", false, move || {
                bridge.send(RequestMethod::ListSignals)
            }));
            for (mode, label) in [
                (DevtoolsProfilerMode::Basic, "Profiler: Basic"),
                (DevtoolsProfilerMode::Performance, "Profiler: Performance"),
                (DevtoolsProfilerMode::Deep, "Profiler: Deep"),
            ] {
                let bridge = app_bridge.clone();
                performance_controls.push(compact_button(
                    label,
                    profiler_mode == mode,
                    move || bridge.send(RequestMethod::SetProfilerMode { mode }),
                ));
            }
            let bridge = app_bridge.clone();
            performance_controls.push(
                Button::new(if recording {
                    "Stop recording"
                } else {
                    "Start recording"
                })
                .size(Size::new(0., 34.))
                .padding(EdgeInsets::symmetric(12., 7.))
                .label_style(TextStyle {
                    size: 13.,
                    color: TEXT_PRIMARY,
                    ..TextStyle::default()
                })
                .color(if recording { DANGER } else { PRIMARY })
                .on_press(move || {
                    bridge.send(if recording {
                        RequestMethod::StopRecording
                    } else {
                        RequestMethod::StartRecording
                    })
                })
                .into(),
            );
            let shared = Arc::clone(&app_shared);
            let tick = app_tick.clone();
            performance_controls.push(compact_button("Clear recording", false, move || {
                if let Ok(mut state) = shared.lock() {
                    state.frames.clear();
                    state.deep_traces.clear();
                    state.deep_trace_events = 0;
                    state.timeline_offset = 0;
                    state.selected_frame = None;
                    state.selected_range = None;
                    state.range_anchor = None;
                }
                tick.update(|value| *value = value.wrapping_add(1));
            }));
            for window in windows {
                let bridge = app_bridge.clone();
                let shared = Arc::clone(&app_shared);
                let tick = app_tick.clone();
                inspector_controls.push(compact_button(
                    format!(
                        "{} · {:.0}×{:.0}",
                        window.title, window.logical_size[0], window.logical_size[1]
                    ),
                    active_window == Some(window.id),
                    move || {
                        if let Ok(mut state) = shared.lock() {
                            state.active_window = Some(window.id);
                            state.nodes.clear();
                            state.rows.clear();
                            state.selected = None;
                            state.hovered = None;
                            state.details = None;
                            state.selected_frame = None;
                            state.selected_range = None;
                            state.range_anchor = None;
                            state.timeline_offset = 0;
                            state.tree_retry_sent = false;
                        }
                        bridge.send(RequestMethod::GetWidgetTree { window: window.id });
                        tick.update(|value| *value = value.wrapping_add(1));
                    },
                ));
            }
            let bridge = app_bridge.clone();
            let shared = Arc::clone(&app_shared);
            let tick = app_tick.clone();
            inspector_controls.push(
                Button::new(if select_mode {
                    "Stop selecting"
                } else {
                    "Select widget in target"
                })
                .size(Size::new(0., 34.))
                .padding(EdgeInsets::symmetric(12., 7.))
                .label_style(TextStyle {
                    size: 13.,
                    color: TEXT_PRIMARY,
                    ..TextStyle::default()
                })
                .color(if select_mode { DANGER } else { PRIMARY })
                .on_press(move || {
                    let selection = shared.lock().ok().and_then(|mut state| {
                        state.select_mode = !state.select_mode;
                        state
                            .active_window
                            .map(|window| (window, state.select_mode))
                    });
                    if let Some((window, enabled)) = selection {
                        bridge.send(if enabled {
                            RequestMethod::StartInspectMode { window }
                        } else {
                            RequestMethod::StopInspectMode { window }
                        });
                    }
                    tick.update(|value| *value = value.wrapping_add(1));
                })
                .into(),
            );
            let bridge = app_bridge.clone();
            let shared = Arc::clone(&app_shared);
            let tick = app_tick.clone();
            inspector_controls.push(compact_button("Clear selection", false, move || {
                let window = shared.lock().ok().and_then(|mut state| {
                    state.selected = None;
                    state.hovered = None;
                    state.details = None;
                    state.active_window
                });
                if let Some(window) = window {
                    bridge.send(RequestMethod::HighlightNode { window, id: None });
                }
                tick.update(|value| *value = value.wrapping_add(1));
            }));
            let shared = Arc::clone(&app_shared);
            let tick = app_tick.clone();
            inspector_controls.push(compact_button("Collapse tree", false, move || {
                if let Ok(mut state) = shared.lock() {
                    state.collapse_all();
                }
                tick.update(|value| *value = value.wrapping_add(1));
            }));
            let shared = Arc::clone(&app_shared);
            let tick = app_tick.clone();
            inspector_controls.push(compact_button("Expand tree", false, move || {
                if let Ok(mut state) = shared.lock() {
                    state.expand_all();
                }
                tick.update(|value| *value = value.wrapping_add(1));
            }));
            for (option, label) in [
                (DebugOption::LayoutBounds, "Bounds: selected"),
                (DebugOption::LayoutBoundsSubtree, "Bounds: subtree"),
                (DebugOption::LayoutBoundsWholeWindow, "Bounds: whole window"),
                (DebugOption::PaddingContent, "Padding/content"),
                (DebugOption::Baselines, "Baselines"),
                (DebugOption::Clips, "Clips"),
                (DebugOption::HitTestRegions, "Hit test"),
                (DebugOption::SemanticsBounds, "Semantics bounds"),
                (DebugOption::ScrollViewports, "Scroll viewports"),
                (DebugOption::LayerBoundaries, "Layer boundaries"),
                (DebugOption::HighlightBuild, "BUILD flash"),
                (DebugOption::HighlightLayout, "LAYOUT flash"),
                (DebugOption::HighlightPaint, "PAINT flash"),
                (DebugOption::HighlightSemantics, "SEMANTICS flash"),
                (DebugOption::HighlightComposite, "COMPOSITE flash"),
                (DebugOption::RepaintRainbow, "Repaint rainbow"),
            ] {
                let bridge = app_bridge.clone();
                let shared = Arc::clone(&app_shared);
                let tick = app_tick.clone();
                let enabled = debug_options.contains(&option);
                debug_controls.push(compact_button(label, enabled, move || {
                    let enabled = if let Ok(mut state) = shared.lock() {
                        if !state.debug_options.insert(option) {
                            state.debug_options.remove(&option);
                            false
                        } else {
                            true
                        }
                    } else {
                        false
                    };
                    bridge.send(RequestMethod::SetDebugOption {
                        name: option,
                        enabled,
                    });
                    tick.update(|value| *value = value.wrapping_add(1));
                }));
            }
            for (scale, label) in [
                (1., "Animations 1×"),
                (0.5, "Animations 0.5×"),
                (0.25, "Animations 0.25×"),
                (0.1, "Animations 0.1×"),
                (0., "Pause animations"),
            ] {
                let bridge = app_bridge.clone();
                animation_controls.push(compact_button(
                    label,
                    scale == animation_scale,
                    move || bridge.send(RequestMethod::SetAnimationSpeed { scale }),
                ));
            }
            let bridge = app_bridge.clone();
            memory_controls.push(compact_button("Capture snapshot", false, move || {
                bridge.send(RequestMethod::TakeMemorySnapshot {
                    label: "manual".into(),
                })
            }));
            let bridge = app_bridge.clone();
            memory_controls.push(compact_button("Set baseline A", false, move || {
                bridge.send(RequestMethod::TakeMemorySnapshot { label: "A".into() })
            }));
            let bridge = app_bridge.clone();
            memory_controls.push(compact_button("Compare snapshot B", false, move || {
                bridge.send(RequestMethod::TakeMemorySnapshot { label: "B".into() })
            }));
            for (label, visible_delta, offset_delta) in [
                ("Timeline zoom in", -4_isize, 0_isize),
                ("Timeline zoom out", 4, 0),
                ("Timeline older", 0, 4),
                ("Timeline newer", 0, -4),
            ] {
                let shared = Arc::clone(&app_shared);
                let tick = app_tick.clone();
                timeline_controls.push(compact_button(label, false, move || {
                    if let Ok(mut state) = shared.lock() {
                        state.timeline_visible = state
                            .timeline_visible
                            .max(6)
                            .saturating_add_signed(visible_delta)
                            .clamp(6, 120);
                        state.timeline_offset = state
                            .timeline_offset
                            .saturating_add_signed(offset_delta)
                            .min(state.frames.len().saturating_sub(1));
                    }
                    tick.update(|value| *value = value.wrapping_add(1));
                }));
            }
            performance_body.extend(frames.into_iter().map(|frame| {
                let label = format!(
                    "#{} · CPU {}µs{} · build {} · layout {} · paint {} · draws {}",
                    frame.frame,
                    frame.timings.cpu_total,
                    if frame.over_budget { " · JANK" } else { "" },
                    frame.timings.build,
                    frame.timings.layout,
                    frame.timings.paint,
                    frame.draw_calls,
                );
                let shared = Arc::clone(&app_shared);
                let tick = app_tick.clone();
                Button::new(label)
                    .size(Size::new(0., 34.))
                    .padding(EdgeInsets::symmetric(10., 7.))
                    .label_style(TextStyle {
                        size: 12.,
                        color: TEXT_PRIMARY,
                        ..TextStyle::default()
                    })
                    .color(if selected_frame == Some((frame.window, frame.frame)) {
                        CONTROL_ACTIVE
                    } else {
                        CONTROL
                    })
                    .on_press(move || {
                        if let Ok(mut state) = shared.lock() {
                            state.selected_frame = Some((frame.window, frame.frame));
                            if let Some(anchor) = state.range_anchor.take() {
                                state.selected_range = Some((anchor, frame.frame));
                            }
                        }
                        tick.update(|value| *value = value.wrapping_add(1));
                    })
                    .into()
            }));
            if let Some(detail) = selected_frame_detail {
                performance_body.push(ui_text(detail, 12., TEXT_MUTED));
            }
            let shared = Arc::clone(&app_shared);
            let tick = app_tick.clone();
            timeline_controls.push(compact_button(
                if selected_range.is_some() {
                    "Reset range start to selected frame"
                } else {
                    "Set range start from selected frame"
                },
                selected_range.is_some(),
                move || {
                    if let Ok(mut state) = shared.lock() {
                        state.range_anchor = state.selected_frame.map(|(_, frame)| frame);
                        state.selected_range = None;
                    }
                    tick.update(|value| *value = value.wrapping_add(1));
                },
            ));
            if let Some(status) = trace_status {
                performance_body.push(ui_text(status, 13., TEXT_PRIMARY));
            } else {
                performance_body.push(ui_text(
                    "Detailed traces appear here when Deep profiling is active.",
                    13.,
                    TEXT_MUTED,
                ));
            }
            if !flame_boxes.is_empty() {
                let height =
                    flame_boxes.iter().map(|item| item.depth).max().unwrap_or(0) as f32 * 20. + 24.;
                let mut canvas = Canvas::default();
                for item in &flame_boxes {
                    let color = match item.phase {
                        TracePhase::Build => Color::rgba(238, 103, 93, 230),
                        TracePhase::Layout => Color::rgba(242, 188, 64, 230),
                        TracePhase::Paint => Color::rgba(91, 156, 246, 230),
                        TracePhase::Semantics => Color::rgba(82, 196, 145, 230),
                        TracePhase::Composite => Color::rgba(167, 105, 234, 230),
                    };
                    canvas.rect(
                        incular::core::Rect::from_origin_size(
                            Offset::new(item.x, item.depth as f32 * 20.),
                            Size::new(item.width, 18.),
                        ),
                        color,
                    );
                }
                performance_body
                    .push(CustomPaint::new(Size::new(900., height), canvas.finish()).into());
            }
            performance_body.push(ui_text(
                format!("Deep profiler — ranked {:?}", trace_range),
                14.,
                TEXT_PRIMARY,
            ));
            for (range, label) in [
                (TraceRange::CurrentFrame, "Rank current frame"),
                (TraceRange::SelectedRange, "Rank selected range"),
                (TraceRange::EntireRecording, "Rank entire recording"),
            ] {
                let shared = Arc::clone(&app_shared);
                let tick = app_tick.clone();
                timeline_controls.push(compact_button(label, trace_range == range, move || {
                    if let Ok(mut state) = shared.lock() {
                        state.trace_range = range;
                    }
                    tick.update(|value| *value = value.wrapping_add(1));
                }));
            }
            performance_body.extend(ranked.into_iter().take(24).map(|entry| {
                let label = format!(
                    "{:?} · {} · {}µs across {} events",
                    entry.phase, entry.node, entry.total_us, entry.count
                );
                let bridge = app_bridge.clone();
                let shared = Arc::clone(&app_shared);
                let tick = app_tick.clone();
                Button::new(label)
                    .size(Size::new(0., 34.))
                    .padding(EdgeInsets::symmetric(10., 7.))
                    .label_style(TextStyle {
                        size: 12.,
                        color: TEXT_PRIMARY,
                        ..TextStyle::default()
                    })
                    .color(CONTROL)
                    .on_press(move || {
                        let window = if let Ok(mut state) = shared.lock() {
                            state.selected = Some(entry.node);
                            state.active_window
                        } else {
                            None
                        };
                        bridge.send(RequestMethod::GetNodeDetails { id: entry.node });
                        if let Some(window) = window {
                            bridge.send(RequestMethod::HighlightNode {
                                window,
                                id: Some(entry.node),
                            });
                        }
                        tick.update(|value| *value = value.wrapping_add(1));
                    })
                    .into()
            }));
            if let Some(memory) = memory {
                memory_body.push(ui_text(memory, 13., TEXT_PRIMARY));
            }
            memory_body.extend(
                memory_diff
                    .into_iter()
                    .map(|line| ui_text(line, 13., TEXT_MUTED)),
            );
            signal_body.extend(signals.into_iter().map(|signal| {
                let label = format!(
                    "{} · {} · generation {} · writes {} · subscribers {}{}",
                    signal.name.as_deref().unwrap_or("<unnamed>"),
                    signal.type_name,
                    signal.generation,
                    signal.write_count,
                    signal.subscriber_count,
                    signal
                        .last_write_summary
                        .as_ref()
                        .map(|value| format!(" · last {value}"))
                        .unwrap_or_default(),
                );
                let bridge = app_bridge.clone();
                let shared = Arc::clone(&app_shared);
                let tick = app_tick.clone();
                compact_button(
                    label,
                    selected_signal
                        .as_ref()
                        .is_some_and(|item| item.id == signal.id),
                    move || {
                        if let Ok(mut state) = shared.lock() {
                            state.selected_signal = Some(signal.id);
                            state.signal_subscribers.clear();
                        }
                        bridge.send(RequestMethod::GetSignalSubscribers { id: signal.id });
                        tick.update(|value| *value = value.wrapping_add(1));
                    },
                )
            }));
            if let Some(signal) = selected_signal {
                signal_body.push(ui_text(
                    format!(
                        "Signal {} subscribers:",
                        signal.name.as_deref().unwrap_or("<unnamed>")
                    ),
                    14.,
                    TEXT_PRIMARY,
                ));
                if signal_subscribers.is_empty() {
                    signal_body.push(ui_text("No retained subscribers.", 13., TEXT_MUTED));
                } else {
                    signal_body.extend(
                        signal_subscribers
                            .into_iter()
                            .map(|subscriber| ui_text(subscriber.path, 13., TEXT_MUTED)),
                    );
                }
                if signal.editable {
                    let bridge = app_bridge.clone();
                    let tick = app_tick.clone();
                    signal_body.push(
                        TextField::new(app_signal_value.clone())
                            .placeholder("New signal value; press Enter")
                            .on_submit(move |input| {
                                if let Some(value) = editable_value(&signal, &input) {
                                    bridge.send(RequestMethod::EditSignal {
                                        id: signal.id,
                                        value,
                                    });
                                    tick.update(|value| *value = value.wrapping_add(1));
                                }
                            })
                            .into(),
                    );
                }
            }
            let search_field: Widget = TextField::new(app_search.clone())
                .placeholder("Filter widget tree; press Enter")
                .on_submit({
                    let shared = Arc::clone(&app_shared);
                    let tick = app_tick.clone();
                    move |query| {
                        if let Ok(mut state) = shared.lock() {
                            state.search = query;
                            state.rebuild_rows();
                        }
                        tick.update(|value| *value = value.wrapping_add(1));
                    }
                })
                .into();
            let list_shared = Arc::clone(&app_shared);
            let list_bridge = app_bridge.clone();
            let list_tick = app_tick.clone();
            let tree_list = VirtualList::fixed_extent_with_controller(
                row_count,
                32.,
                app_tree_scroll.clone(),
                move |index| {
                    let snapshot = list_shared.lock().ok().and_then(|state| {
                        let row = state.rows.get(index)?.clone();
                        let node = state.nodes.get(&row.id)?;
                        Some((
                            state.row_label(&row),
                            row,
                            state.selected,
                            state.hovered,
                            state.expanded.contains(&node.id),
                            !node.child_ids.is_empty(),
                        ))
                    });
                    let Some((label, row, selected, hovered, expanded, has_children)) = snapshot
                    else {
                        return Widget::text("<stale row>");
                    };
                    let color = if selected == Some(row.id) {
                        Color::rgba(46, 112, 202, 255)
                    } else if hovered == Some(row.id) {
                        Color::rgba(43, 57, 78, 255)
                    } else {
                        Color::rgba(25, 32, 45, 255)
                    };
                    let hover_shared = Arc::clone(&list_shared);
                    let hover_bridge = list_bridge.clone();
                    let hover_tick = list_tick.clone();
                    let hover_id = row.id;
                    let exit_shared = Arc::clone(&list_shared);
                    let exit_bridge = list_bridge.clone();
                    let exit_tick = list_tick.clone();
                    let exit_id = row.id;
                    let press_shared = Arc::clone(&list_shared);
                    let press_bridge = list_bridge.clone();
                    let press_tick = list_tick.clone();
                    let disclosure: Widget = if has_children {
                        let icon: Widget = Icon::new(icons::chevron_right())
                            .size(14.)
                            .brush(if selected == Some(row.id) {
                                TEXT_PRIMARY
                            } else {
                                TEXT_MUTED
                            })
                            .into();
                        let icon = if expanded {
                            Transform::rotation(std::f32::consts::FRAC_PI_2, icon).into()
                        } else {
                            icon
                        };
                        let toggle_shared = Arc::clone(&list_shared);
                        let toggle_tick = list_tick.clone();
                        Button::new(if expanded { "Collapse" } else { "Expand" })
                            .size(Size::new(28., 28.))
                            .padding(EdgeInsets::all(6.))
                            .content(icon)
                            .color(Color::TRANSPARENT)
                            .on_press(move || {
                                if let Ok(mut state) = toggle_shared.lock() {
                                    state.toggle_expanded(row.id);
                                }
                                toggle_tick.update(|value| *value = value.wrapping_add(1));
                            })
                            .into()
                    } else {
                        gap(28., 28.)
                    };
                    let content = Align::new(
                        Alignment::CENTER_LEFT,
                        Padding::new(
                            EdgeInsets::symmetric(8., 6.),
                            ui_text(
                                label.clone(),
                                13.,
                                if selected == Some(row.id) {
                                    TEXT_PRIMARY
                                } else {
                                    TEXT_MUTED
                                },
                            ),
                        ),
                    );
                    let selection: Widget = Button::new(label)
                        .size(Size::new(0., 30.))
                        .content(content)
                        .color(color)
                        .on_hover(move || {
                            let window = hover_shared.lock().ok().and_then(|mut state| {
                                state.hovered = Some(hover_id);
                                state.active_window
                            });
                            if let Some(window) = window {
                                hover_bridge.send(RequestMethod::HighlightNode {
                                    window,
                                    id: Some(hover_id),
                                });
                            }
                            hover_tick.update(|value| *value = value.wrapping_add(1));
                        })
                        .on_exit(move || {
                            let (window, selected) = exit_shared
                                .lock()
                                .ok()
                                .map(|mut state| {
                                    if state.hovered == Some(exit_id) {
                                        state.hovered = None;
                                    }
                                    (state.active_window, state.selected)
                                })
                                .unwrap_or((None, None));
                            if let Some(window) = window {
                                exit_bridge.send(RequestMethod::HighlightNode {
                                    window,
                                    id: selected,
                                });
                            }
                            exit_tick.update(|value| *value = value.wrapping_add(1));
                        })
                        .on_press(move || {
                            let window = if let Ok(mut state) = press_shared.lock() {
                                state.selected = Some(row.id);
                                state.reveal(row.id);
                                state.active_window
                            } else {
                                None
                            };
                            if let Some(window) = window {
                                press_bridge.send(RequestMethod::GetNodeDetails { id: row.id });
                                press_bridge.send(RequestMethod::HighlightNode {
                                    window,
                                    id: Some(row.id),
                                });
                            }
                            press_tick.update(|value| *value = value.wrapping_add(1));
                        })
                        .into();
                    Row::new([
                        gap(row.depth as f32 * 16., 1.),
                        disclosure,
                        gap(4., 1.),
                        Expanded::new(selection).into(),
                    ])
                    .into()
                },
            );

            let mut property_editor = Vec::new();
            if active_inspector_section == InspectorSection::Properties
                && let Some(details) = selected_details.as_ref()
            {
                if let Some(property) = details.properties.iter().find(|property| property.editable)
                {
                    let binding = (details.id, property.name.clone());
                    if app_property_binding.borrow().as_ref() != Some(&binding) {
                        app_property_value.set_text(debug_value(&property.value));
                        *app_property_binding.borrow_mut() = Some(binding);
                    }
                    property_editor.push(ui_text(
                        format!(
                            "Edit {}{}",
                            property.name,
                            if property.overridden {
                                " · DEV OVERRIDE"
                            } else {
                                ""
                            }
                        ),
                        13.,
                        if property.overridden {
                            SUCCESS
                        } else {
                            TEXT_PRIMARY
                        },
                    ));
                    let bridge = app_bridge.clone();
                    let tick = app_tick.clone();
                    let id = details.id;
                    let name = property.name.clone();
                    let template = property.value.clone();
                    property_editor.push(
                        TextField::new(app_property_value.clone())
                            .placeholder("Enter a value and press Enter")
                            .on_submit(move |input| {
                                if let Some(value) = parse_debug_value(&template, &input) {
                                    bridge.send(RequestMethod::EditProperty {
                                        id,
                                        name: name.clone(),
                                        value,
                                    });
                                    tick.update(|value| *value = value.wrapping_add(1));
                                }
                            })
                            .into(),
                    );
                    if property.overridden {
                        let bridge = app_bridge.clone();
                        property_editor.push(compact_button(
                            "Reset property overrides",
                            false,
                            move || bridge.send(RequestMethod::ResetOverrides),
                        ));
                    }
                    property_editor.push(gap(1., 10.));
                } else {
                    property_editor.push(ui_text(
                        "This widget exposes read-only retained properties.",
                        12.,
                        TEXT_MUTED,
                    ));
                    property_editor.push(gap(1., 8.));
                }
            }

            let details_content = if has_selection {
                let mut detail_tabs = Vec::new();
                for (section, label) in [
                    (InspectorSection::Properties, "Properties"),
                    (InspectorSection::Layout, "Layout"),
                    (InspectorSection::Signals, "Signals"),
                    (InspectorSection::Why, "Why"),
                    (InspectorSection::Semantics, "Semantics"),
                ] {
                    let section_signal = app_inspector_section.clone();
                    detail_tabs.push(compact_button(
                        label,
                        active_inspector_section == section,
                        move || {
                            section_signal.set(section);
                        },
                    ));
                }
                Column::new([
                    Wrap::new(detail_tabs).spacing(8.).run_spacing(8.).into(),
                    gap(1., 12.),
                    Column::new(property_editor).into(),
                    Column::new(
                        details
                            .into_iter()
                            .map(|line| ui_text(line, 13., TEXT_MUTED)),
                    )
                    .into(),
                ])
                .into()
            } else {
                Column::new([
                    ui_text("Nothing selected", 18., TEXT_PRIMARY),
                    gap(1., 6.),
                    ui_text(
                        "Choose a row in the tree or start target selection, then point at the running application.",
                        13.,
                        TEXT_MUTED,
                    ),
                ])
                .into()
            };
            let inspector_content = Column::new([
                ui_text("Inspector", 22., TEXT_PRIMARY),
                ui_text(
                    "Explore retained widgets, layout decisions, and target overlays.",
                    13.,
                    TEXT_MUTED,
                ),
                gap(1., 16.),
                section(
                    "Selection",
                    "Target window and widget-picking controls",
                    Wrap::new(inspector_controls)
                        .spacing(8.)
                        .run_spacing(8.)
                        .into(),
                ),
                gap(1., 12.),
                section(
                    "Selected widget",
                    "Properties and retained layout diagnostics",
                    details_content,
                ),
                gap(1., 12.),
                section(
                    "Visual debugging",
                    "Overlay real retained geometry without changing application layout",
                    Wrap::new(debug_controls).spacing(8.).run_spacing(8.).into(),
                ),
                gap(1., 12.),
                section(
                    "Animation speed",
                    "Slow or pause target animations while inspecting frames",
                    Wrap::new(animation_controls)
                        .spacing(8.)
                        .run_spacing(8.)
                        .into(),
                ),
            ]);

            let performance_content = Column::new([
                ui_text("Performance", 22., TEXT_PRIMARY),
                ui_text(
                    "Record frames, inspect jank, and rank retained work by phase.",
                    13.,
                    TEXT_MUTED,
                ),
                gap(1., 16.),
                section(
                    "Profiler",
                    "Choose the amount of tracing before recording",
                    Wrap::new(performance_controls)
                        .spacing(8.)
                        .run_spacing(8.)
                        .into(),
                ),
                gap(1., 12.),
                section(
                    "Frame timeline",
                    "Select frames and compare a bounded range",
                    Column::new([
                        Wrap::new(timeline_controls)
                            .spacing(8.)
                            .run_spacing(8.)
                            .into(),
                        gap(1., 12.),
                        Column::new(performance_body).into(),
                    ])
                    .into(),
                ),
            ]);

            if memory_body.is_empty() {
                memory_body.push(ui_text(
                    "Capture a snapshot to inspect the framework inventory.",
                    13.,
                    TEXT_MUTED,
                ));
            }
            let memory_content = Column::new([
                ui_text("Memory", 22., TEXT_PRIMARY),
                ui_text(
                    "Compare bounded framework counts and process RSS.",
                    13.,
                    TEXT_MUTED,
                ),
                gap(1., 16.),
                section(
                    "Snapshots",
                    "Use A and B to measure changes after a target interaction",
                    Column::new([
                        Wrap::new(memory_controls)
                            .spacing(8.)
                            .run_spacing(8.)
                            .into(),
                        gap(1., 12.),
                        Column::new(memory_body).into(),
                    ])
                    .into(),
                ),
            ]);

            if signal_body.len() == 1 {
                signal_body.push(ui_text(
                    "No debug-enabled signals are registered by this target.",
                    13.,
                    TEXT_MUTED,
                ));
            }
            let signals_content = Column::new([
                ui_text("Signals", 22., TEXT_PRIMARY),
                ui_text(
                    "Inspect writes, subscribers, and explicitly editable debug values.",
                    13.,
                    TEXT_MUTED,
                ),
                gap(1., 16.),
                section(
                    "Registered signals",
                    "Values remain redacted unless a signal opts into editing",
                    Column::new(signal_body).into(),
                ),
            ]);

            let page_content: Widget = match active_view {
                ToolView::Inspector => inspector_content.into(),
                ToolView::Performance => performance_content.into(),
                ToolView::Memory => memory_content.into(),
                ToolView::Signals => signals_content.into(),
            };
            let mut tabs = Vec::new();
            for (view, label) in [
                (ToolView::Inspector, "Inspector"),
                (ToolView::Performance, "Performance"),
                (ToolView::Memory, "Memory"),
                (ToolView::Signals, "Signals"),
            ] {
                let selected = active_view == view;
                let tool_view = app_tool_view.clone();
                tabs.push(compact_button(label, selected, move || {
                    tool_view.set(view);
                }));
            }
            let inspector_scroll = app_inspector_scroll.clone();
            let header_widget = Row::new([
                ui_text("Incular DevTools", 20., TEXT_PRIMARY),
                gap(14., 1.),
                ui_text(header, 12., if connected { SUCCESS } else { TEXT_MUTED }),
            ]);
            LayoutBuilder::new(move |constraints| {
                let width = constraints.max_width.max(960.);
                let height = constraints.max_height.max(640.);
                let body_height = (height - 116.).max(1.);
                let tree_width = (width * 0.34).clamp(340., 470.);
                let tree_panel: Widget = DecoratedBox::new(Padding::all(
                    14.,
                    Column::new([
                        ui_text("Widget tree", 16., TEXT_PRIMARY),
                        ui_text(
                            format!("{row_count} visible retained widgets"),
                            12.,
                            TEXT_MUTED,
                        ),
                        gap(1., 10.),
                        search_field.clone(),
                        gap(1., 10.),
                        Expanded::new(tree_list.clone()).into(),
                    ]),
                ))
                .background(SURFACE)
                .border(Border::new(1., BORDER))
                .radius(10.)
                .into();
                let details_panel: Widget = DecoratedBox::new(ScrollView::vertical(
                    inspector_scroll.clone(),
                    Padding::all(20., page_content.clone()),
                ))
                .background(SURFACE)
                .border(Border::new(1., BORDER))
                .radius(10.)
                .into();
                let body: Widget = if active_view == ToolView::Inspector {
                    Row::new([
                        ConstrainedBox::new(
                            Constraints::tight(Size::new(tree_width, body_height)),
                            tree_panel,
                        )
                        .into(),
                        gap(12., 1.),
                        Expanded::new(details_panel).into(),
                    ])
                    .into()
                } else {
                    ConstrainedBox::new(
                        Constraints::tight(Size::new(width - 32., body_height)),
                        details_panel,
                    )
                    .into()
                };
                SizedBox::new(
                    Size::new(width, height),
                    DecoratedBox::new(Padding::all(
                        16.,
                        Column::new([
                            header_widget.clone().into(),
                            gap(1., 10.),
                            Wrap::new(tabs.clone()).spacing(8.).run_spacing(8.).into(),
                            gap(1., 12.),
                            ConstrainedBox::new(
                                Constraints::tight(Size::new(width - 32., body_height)),
                                body,
                            )
                            .into(),
                        ]),
                    ))
                    .background(APP_BACKGROUND),
                )
                .into()
            })
            .into()
        },
    );
    match app {
        Ok(app) => incular::run(app).expect("devtools application"),
        Err(error) => eprintln!("unable to start DevTools: {error:?}"),
    }
}

fn start_client(record: DiscoveryRecord, model: Shared) -> ClientBridge {
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

fn run_client(
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
            let needs_window_request = info.windows.is_empty();
            let window = if let Ok(mut state) = model.lock() {
                state.connected = true;
                state.error = None;
                state.target = format!("pid {} · {}", info.pid, info.platform);
                state.windows = info.windows;
                state.active_window = state.windows.first().map(|window| window.id);
                state.tree_retry_sent = false;
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
        Message::Event(TargetEvent::Log { message, .. }) => {
            if let Ok(mut state) = model.lock()
                && message.contains("recording stopped")
            {
                state.recording = false;
                state.error = Some(message);
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

fn note_error(model: &Shared, updates: &mpsc::Sender<()>, message: String) {
    if let Ok(mut state) = model.lock() {
        state.error = Some(message);
        state.connected = false;
    }
    let _ = updates.send(());
}

#[cfg(test)]
mod tests {
    use super::*;
    use incular::runtime::Runtime;
    fn id(index: u64) -> DevWidgetId {
        DevWidgetId::new(index, 0)
    }
    fn node(index: u64, children: Vec<DevWidgetId>) -> WidgetNode {
        WidgetNode {
            id: id(index),
            parent: None,
            type_name: format!("Node{index}"),
            key: None,
            label: None,
            child_ids: children,
            revision: 0,
        }
    }
    #[test]
    fn snapshot_and_deltas_keep_a_compact_virtual_row_index() {
        let window = DevWindowId::new(1, 0);
        let mut state = InspectorModel::default();
        let root = node(1, (2..=100_001).map(id).collect());
        let children = (2..=100_001).map(|index| node(index, Vec::new())).collect();
        state.apply_tree(
            1,
            [TreeDelta::Snapshot {
                window,
                root: Box::new(root),
                nodes: children,
                truncated: false,
            }],
        );
        assert_eq!(state.rows.len(), 100_001);
        assert_eq!(state.selected, Some(id(1)));
        assert!(std::mem::size_of::<TreeRow>() <= 24);
        state.apply_tree(2, [TreeDelta::Remove { id: id(2) }]);
        assert_eq!(state.rows.len(), 100_000);
    }
    #[test]
    fn collapsed_branch_does_not_create_descendant_rows() {
        let window = DevWindowId::new(1, 0);
        let mut state = InspectorModel::default();
        state.apply_tree(
            1,
            [TreeDelta::Snapshot {
                window,
                root: Box::new(node(1, vec![id(2)])),
                nodes: vec![node(2, vec![id(3)]), node(3, Vec::new())],
                truncated: false,
            }],
        );
        assert_eq!(state.rows.len(), 2);
        state.expanded.insert(id(2));
        state.rebuild_rows();
        assert_eq!(state.rows.len(), 3);
    }

    #[test]
    fn tree_disclosure_expand_and_collapse_do_not_change_selection() {
        let window = DevWindowId::new(1, 0);
        let mut state = InspectorModel::default();
        state.apply_tree(
            1,
            [TreeDelta::Snapshot {
                window,
                root: Box::new(node(1, vec![id(2)])),
                nodes: vec![node(2, vec![id(3)]), node(3, Vec::new())],
                truncated: false,
            }],
        );
        assert_eq!(state.selected, Some(id(1)));
        assert!(state.toggle_expanded(id(2)));
        assert_eq!(state.rows.len(), 3);
        assert_eq!(state.selected, Some(id(1)));
        state.collapse_all();
        assert_eq!(state.rows.len(), 1);
        assert_eq!(state.selected, Some(id(1)));
        state.expand_all();
        assert_eq!(state.rows.len(), 3);
        assert_eq!(state.selected, Some(id(1)));
    }

    #[test]
    fn signal_editor_parses_only_the_explicit_supported_type() {
        let number = SignalSummary {
            id: DevSignalId::new(1, 1),
            name: Some("counter".into()),
            type_name: "i64".into(),
            generation: 0,
            write_count: 0,
            subscriber_count: 0,
            last_write_summary: None,
            editable: true,
        };
        assert_eq!(editable_value(&number, "42"), Some(EditableValue::Int(42)));
        assert_eq!(editable_value(&number, "not a number"), None);

        let unsupported = SignalSummary {
            type_name: "my_app::State".into(),
            ..number
        };
        assert_eq!(editable_value(&unsupported, "anything"), None);
    }

    #[test]
    fn property_editor_preserves_the_inspected_debug_value_type() {
        assert_eq!(
            parse_debug_value(&DebugValue::Float(0.), "0.35"),
            Some(DebugValue::Float(0.35))
        );
        assert_eq!(
            parse_debug_value(&DebugValue::Color(0, 0, 0, 0), "#336699CC"),
            Some(DebugValue::Color(0x33, 0x66, 0x99, 0xCC))
        );
        assert_eq!(
            parse_debug_value(&DebugValue::Float(0.), "not-a-number"),
            None
        );
        assert_eq!(parse_debug_value(&DebugValue::Redacted, "anything"), None);
    }

    #[test]
    fn memory_diff_is_explicitly_a_framework_inventory_delta() {
        let state = InspectorModel {
            memory_a: Some(MemorySnapshot {
                label: "A".into(),
                counts: incular_devtools_protocol::ResourceCounts {
                    elements: 10,
                    rss_mb: 100,
                    ..Default::default()
                },
            }),
            memory_b: Some(MemorySnapshot {
                label: "B".into(),
                counts: incular_devtools_protocol::ResourceCounts {
                    elements: 13,
                    rss_mb: 104,
                    ..Default::default()
                },
            }),
            ..Default::default()
        };
        let lines = state.memory_diff_lines();
        assert!(
            lines
                .iter()
                .any(|line| line.contains("RSS 100 MB → 104 MB (+4)"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("elements 10 → 13 (+3)"))
        );
    }

    fn trace() -> DeepFrameTrace {
        DeepFrameTrace {
            window: DevWindowId::new(1, 0),
            frame: 7,
            events: vec![
                incular_devtools_protocol::TraceEvent {
                    node: id(1),
                    phase: TracePhase::Layout,
                    parent: None,
                    start_us: 10,
                    duration_us: 100,
                },
                incular_devtools_protocol::TraceEvent {
                    node: id(2),
                    phase: TracePhase::Layout,
                    parent: Some(0),
                    start_us: 20,
                    duration_us: 70,
                },
                incular_devtools_protocol::TraceEvent {
                    node: id(2),
                    phase: TracePhase::Paint,
                    parent: None,
                    start_us: 115,
                    duration_us: 40,
                },
            ],
            truncated: false,
            dropped_events: 0,
        }
    }

    #[test]
    fn flamegraph_preserves_trace_hierarchy_and_duration_geometry() {
        let boxes = flamegraph_boxes(&trace(), Some(TracePhase::Layout), 100.);
        assert_eq!(boxes.len(), 2);
        assert_eq!(boxes[0].depth, 0);
        assert_eq!(boxes[1].depth, 1);
        assert_eq!(boxes[0].x, 0.);
        assert_eq!(boxes[0].width, 100.);
        assert!(boxes[1].x > 0. && boxes[1].width < boxes[0].width);
    }

    #[test]
    fn ranked_trace_aggregates_frames_and_sorts_total_duration() {
        let first = trace();
        let second = trace();
        let ranked = rank_traces([&first, &second], Some(TracePhase::Layout));
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].node, id(1));
        assert_eq!(ranked[0].total_us, 200);
        assert_eq!(ranked[0].count, 2);
        assert_eq!(ranked[1].total_us, 140);
    }

    #[test]
    fn deep_recording_has_frame_and_total_event_memory_bounds() {
        let mut model = InspectorModel::default();
        for frame in 0..4 {
            let mut batch = trace();
            batch.frame = frame;
            batch.events = vec![batch.events[0].clone(); 80_000];
            model.push_deep_trace(batch);
        }
        assert!(model.deep_traces.len() <= InspectorModel::FRAME_HISTORY);
        assert!(model.deep_trace_events <= InspectorModel::MAX_TRACE_EVENTS);
        assert_eq!(
            model.deep_trace_events,
            model
                .deep_traces
                .iter()
                .map(|trace| trace.events.len())
                .sum::<usize>()
        );
    }

    #[test]
    fn target_pid_argument_selects_exact_session_and_default_is_newest() {
        let session = |pid, started_unix_ms| DiscoveryRecord {
            pid,
            app_name: format!("app-{pid}"),
            port: pid as u16,
            auth_token: format!("token-{pid}"),
            started_unix_ms,
            protocol_version: PROTOCOL_VERSION,
        };
        let sessions = vec![session(10, 100), session(20, 200)];
        assert_eq!(
            requested_target_pid(["devtools", "--target-pid", "10"]),
            Some(10)
        );
        assert_eq!(select_session(&sessions, Some(10)).unwrap().pid, 10);
        assert_eq!(select_session(&sessions, None).unwrap().pid, 20);
        assert!(select_session(&sessions, Some(30)).is_none());
    }

    #[test]
    fn revealing_target_selection_expands_every_ancestor() {
        let window = DevWindowId::new(1, 0);
        let root = node(1, vec![id(2)]);
        let mut middle = node(2, vec![id(3)]);
        middle.parent = Some(id(1));
        let mut leaf = node(3, Vec::new());
        leaf.parent = Some(id(2));
        let mut state = InspectorModel::default();
        state.apply_tree(
            1,
            [TreeDelta::Snapshot {
                window,
                root: Box::new(root),
                nodes: vec![middle, leaf],
                truncated: false,
            }],
        );
        state.expanded.clear();
        state.reveal(id(3));
        assert!(state.expanded.contains(&id(1)));
        assert!(state.expanded.contains(&id(2)));
        assert_eq!(state.rows.len(), 3);
    }

    #[test]
    fn devtools_shell_tabs_remain_hittable_above_the_scrolling_body() {
        let active = Signal::new(ToolView::Inspector);
        let observed = active.clone();
        let scroll = ScrollController::new();
        let root: Widget = LayoutBuilder::new(move |constraints| {
            let width = constraints.max_width.max(960.);
            let height = constraints.max_height.max(640.);
            let body_height = (height - 116.).max(1.);
            let mut tabs = Vec::new();
            for (view, label) in [
                (ToolView::Inspector, "Inspector"),
                (ToolView::Performance, "Performance"),
                (ToolView::Memory, "Memory"),
                (ToolView::Signals, "Signals"),
            ] {
                let active = active.clone();
                tabs.push(compact_button(label, active.get() == view, move || {
                    active.set(view);
                }));
            }
            let body: Widget = DecoratedBox::new(ScrollView::vertical(
                scroll.clone(),
                Padding::all(20., Column::new([gap(1., 1_200.)])),
            ))
            .background(SURFACE)
            .border(Border::new(1., BORDER))
            .radius(10.)
            .into();
            SizedBox::new(
                Size::new(width, height),
                DecoratedBox::new(Padding::all(
                    16.,
                    Column::new([
                        ui_text("Incular DevTools", 20., TEXT_PRIMARY),
                        gap(1., 10.),
                        Wrap::new(tabs).spacing(8.).run_spacing(8.).into(),
                        gap(1., 12.),
                        ConstrainedBox::new(
                            Constraints::tight(Size::new(width - 32., body_height)),
                            body,
                        )
                        .into(),
                    ]),
                ))
                .background(APP_BACKGROUND),
            )
            .into()
        })
        .into();
        let mut runtime = Runtime::new(root).unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(1440., 900.)))
            .unwrap();
        let down = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(157., 70.),
        });
        let up = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Up,
            position: Offset::new(157., 70.),
        });
        assert!(
            down.is_some_and(|target| target.action.is_some()),
            "the Performance tab must own its visual bounds; got {down:?}"
        );
        assert!(
            up.is_some_and(|target| target.action.is_some()),
            "a completed Performance-tab press must dispatch; got {up:?}"
        );
        assert_eq!(observed.get(), ToolView::Performance);
    }
}
