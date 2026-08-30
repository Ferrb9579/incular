use crate::performance::TraceRange;
use incular_devtools_protocol::{
    DebugOption, DebugProperty, DebugValue, DeepFrameTrace, DevSignalId, DevWidgetId, DevWindowId,
    DevtoolsProfilerMode, EditableValue, FrameRecordEvent, LayoutDetails, MemorySnapshot,
    NodeDetails, SignalSubscriber, SignalSummary, TargetInfo, TreeDelta, WidgetNode, WindowSummary,
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub(crate) type Shared = Arc<Mutex<InspectorModel>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TreeRow {
    pub(crate) id: DevWidgetId,
    pub(crate) depth: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ConsoleEntry {
    pub(crate) level: String,
    pub(crate) target: String,
    pub(crate) message: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum InspectorSection {
    #[default]
    Properties,
    Layout,
    Signals,
    Why,
    Semantics,
}

/// Protocol-facing inspector state. It remains UI-framework independent so
/// generation handling, deltas, and virtualization can be tested cheaply.
#[derive(Default)]
pub(crate) struct InspectorModel {
    pub(crate) connected: bool,
    pub(crate) error: Option<String>,
    pub(crate) target: String,
    pub(crate) target_info: Option<TargetInfo>,
    pub(crate) windows: Vec<WindowSummary>,
    pub(crate) active_window: Option<DevWindowId>,
    pub(crate) nodes: HashMap<DevWidgetId, WidgetNode>,
    pub(crate) roots: HashMap<DevWindowId, DevWidgetId>,
    pub(crate) expanded: HashSet<DevWidgetId>,
    pub(crate) rows: Vec<TreeRow>,
    pub(crate) selected: Option<DevWidgetId>,
    pub(crate) hovered: Option<DevWidgetId>,
    pub(crate) select_mode: bool,
    pub(crate) details: Option<NodeDetails>,
    pub(crate) frames: VecDeque<FrameRecordEvent>,
    pub(crate) deep_traces: VecDeque<DeepFrameTrace>,
    pub(crate) deep_trace_events: usize,
    pub(crate) profiler_mode: DevtoolsProfilerMode,
    pub(crate) recording: bool,
    pub(crate) selected_frame: Option<(DevWindowId, u64)>,
    pub(crate) range_anchor: Option<u64>,
    pub(crate) selected_range: Option<(u64, u64)>,
    pub(crate) trace_range: TraceRange,
    pub(crate) timeline_offset: usize,
    pub(crate) timeline_visible: usize,
    pub(crate) memory: Option<MemorySnapshot>,
    pub(crate) memory_a: Option<MemorySnapshot>,
    pub(crate) memory_b: Option<MemorySnapshot>,
    pub(crate) console: VecDeque<ConsoleEntry>,
    pub(crate) console_filter: String,
    pub(crate) frame_arrivals: HashMap<DevWindowId, VecDeque<Instant>>,
    pub(crate) signals: Vec<SignalSummary>,
    pub(crate) selected_signal: Option<DevSignalId>,
    pub(crate) signal_subscribers: Vec<SignalSubscriber>,
    pub(crate) debug_options: HashSet<DebugOption>,
    pub(crate) animation_scale: Option<f32>,
    pub(crate) tree_retry_sent: bool,
    pub(crate) tree_revision: u64,
    pub(crate) search: String,
}

pub(crate) fn editable_value(signal: &SignalSummary, input: &str) -> Option<EditableValue> {
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
    pub(crate) const FRAME_HISTORY: usize = 300;
    pub(crate) const MAX_TRACE_EVENTS: usize = 200_000;
    pub(crate) const CONSOLE_HISTORY: usize = 500;

    pub(crate) fn push_console(
        &mut self,
        level: impl Into<String>,
        target: impl Into<String>,
        message: impl Into<String>,
    ) {
        while self.console.len() >= Self::CONSOLE_HISTORY {
            self.console.pop_front();
        }
        self.console.push_back(ConsoleEntry {
            level: level.into(),
            target: target.into(),
            message: message.into(),
        });
    }

    pub(crate) fn note_frame_arrival(&mut self, window: DevWindowId) {
        let now = Instant::now();
        let arrivals = self.frame_arrivals.entry(window).or_default();
        arrivals.push_back(now);
        while arrivals
            .front()
            .is_some_and(|arrival| now.duration_since(*arrival) > Duration::from_secs(2))
        {
            arrivals.pop_front();
        }
    }

    pub(crate) fn fps(&self, window: Option<DevWindowId>) -> Option<f32> {
        let arrivals = self.frame_arrivals.get(&window?)?;
        let first = *arrivals.front()?;
        let last = *arrivals.back()?;
        let elapsed = last.duration_since(first).as_secs_f32();
        (arrivals.len() > 1 && elapsed > f32::EPSILON)
            .then(|| ((arrivals.len() - 1) as f32 / elapsed).min(240.))
    }

    pub(crate) fn latest_frame(&self, window: Option<DevWindowId>) -> Option<&FrameRecordEvent> {
        self.frames
            .iter()
            .rev()
            .find(|frame| Some(frame.window) == window)
    }

    pub(crate) fn filtered_console(&self) -> Vec<ConsoleEntry> {
        let query = self.console_filter.trim().to_ascii_lowercase();
        self.console
            .iter()
            .filter(|entry| {
                query.is_empty()
                    || entry.level.to_ascii_lowercase().contains(&query)
                    || entry.target.to_ascii_lowercase().contains(&query)
                    || entry.message.to_ascii_lowercase().contains(&query)
            })
            .cloned()
            .collect()
    }

    pub(crate) fn push_deep_trace(&mut self, mut trace: DeepFrameTrace) {
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

    pub(crate) fn memory_diff_lines(&self) -> Vec<String> {
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

    pub(crate) fn apply_tree(
        &mut self,
        revision: u64,
        deltas: impl IntoIterator<Item = TreeDelta>,
    ) {
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

    pub(crate) fn remove_subtree(&mut self, id: DevWidgetId) {
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

    pub(crate) fn move_node(&mut self, id: DevWidgetId, parent: DevWidgetId, position: usize) {
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

    pub(crate) fn rebuild_rows(&mut self) {
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

    pub(crate) fn reveal(&mut self, id: DevWidgetId) {
        let mut current = self.nodes.get(&id).and_then(|node| node.parent);
        while let Some(parent) = current {
            self.expanded.insert(parent);
            current = self.nodes.get(&parent).and_then(|node| node.parent);
        }
        self.rebuild_rows();
    }

    pub(crate) fn toggle_expanded(&mut self, id: DevWidgetId) -> bool {
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

    pub(crate) fn collapse_all(&mut self) {
        self.expanded.clear();
        self.rebuild_rows();
    }

    pub(crate) fn expand_all(&mut self) {
        self.expanded = self
            .nodes
            .iter()
            .filter_map(|(id, node)| (!node.child_ids.is_empty()).then_some(*id))
            .collect();
        self.rebuild_rows();
    }

    pub(crate) fn row_label(&self, row: &TreeRow) -> String {
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

    pub(crate) fn details_lines(&self, section: InspectorSection) -> Vec<String> {
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

pub(crate) fn layout_detail_lines(details: &LayoutDetails) -> Vec<String> {
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

pub(crate) fn property_line(property: &DebugProperty) -> String {
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

pub(crate) fn debug_value(value: &DebugValue) -> String {
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

pub(crate) fn parse_debug_value(template: &DebugValue, input: &str) -> Option<DebugValue> {
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
