//! Runner-side DevTools glue: agent lifecycle, per-frame command draining,
//! debug overlays drawn OUTSIDE the widget tree (compositor-only), and
//! Select Widget input interception.

use incular_devtools::AgentHandle;
use incular_devtools::commands::{UiCommand, UiReply};
use incular_devtools_protocol::{
    DebugOption, DebugValue, DevWidgetId, DevWindowId, DevtoolsProfilerMode, ErrorCode,
    MemorySnapshot, RequestMethod, ResponsePayload, TargetEvent, TreeDelta, WidgetNode,
};
use std::time::{Duration, Instant};

type PhaseCounters = std::collections::HashMap<DevWidgetId, [u64; 5]>;
type PhaseFlashes = Vec<([f32; 4], [bool; 5])>;

#[derive(Clone)]
struct PropertyOverride {
    original: DebugValue,
    current: DebugValue,
}

/// Overlay + inspect-mode state for the whole application.
#[derive(Default)]
pub struct DevToolsState {
    agent: Option<AgentHandle>,
    /// Stable per-session mapping protocol id -> platform window.
    window_map: std::collections::HashMap<DevWindowId, incular_platform::WindowId>,
    /// Window id currently in Select Widget mode.
    select_window: Option<incular_platform::WindowId>,
    /// Hovered element while selecting.
    hover: Option<(DevWidgetId, [f32; 4])>,
    /// Node highlighted from the DevTools tree (per window).
    highlight: Option<(DevWidgetId, [f32; 4])>,
    highlight_content: Option<[f32; 4]>,
    highlight_baseline: Option<f32>,
    highlight_clip: Option<[f32; 4]>,
    options: std::collections::BTreeSet<DebugOption>,
    /// Per-window bounded geometry cached for the diagnostic display-list
    /// pass. This lives outside the target Widget tree.
    layout_bounds: std::collections::HashMap<incular_platform::WindowId, Vec<[f32; 4]>>,
    hit_regions: std::collections::HashMap<incular_platform::WindowId, Vec<[f32; 4]>>,
    semantics_bounds: std::collections::HashMap<incular_platform::WindowId, Vec<[f32; 4]>>,
    scroll_viewports: std::collections::HashMap<incular_platform::WindowId, Vec<[f32; 4]>>,
    layer_bounds: std::collections::HashMap<incular_platform::WindowId, Vec<[f32; 4]>>,
    last_layout_bounds_sample: Option<Instant>,
    phase_counters: std::collections::HashMap<incular_platform::WindowId, PhaseCounters>,
    phase_flashes: std::collections::HashMap<incular_platform::WindowId, PhaseFlashes>,
    repaint_generations: std::collections::HashMap<
        incular_platform::WindowId,
        std::collections::HashMap<DevWidgetId, u8>,
    >,
    repaint_rainbow: std::collections::HashMap<incular_platform::WindowId, Vec<([f32; 4], u8)>>,
    animation_speed: f32,
    frame_counter: u64,
    profiler_mode: DevtoolsProfilerMode,
    recording: bool,
    recorded_frames: u64,
    /// Only windows explicitly opened in the inspector pay for delta state.
    tree_subscriptions: std::collections::HashSet<DevWindowId>,
    tree_cache:
        std::collections::HashMap<DevWindowId, std::collections::HashMap<DevWidgetId, WidgetNode>>,
    last_tree_sample: Option<Instant>,
    property_overrides: std::collections::HashMap<(DevWidgetId, String), PropertyOverride>,
}

impl DevToolsState {
    pub fn new(agent: Option<AgentHandle>) -> Self {
        Self {
            agent,
            last_tree_sample: None,
            animation_speed: 1.,
            ..Self::default()
        }
    }

    pub fn is_selecting(&self, window: incular_platform::WindowId) -> bool {
        self.select_window == Some(window)
    }

    pub fn animation_speed(&self) -> f32 {
        self.animation_speed
    }

    /// Records a hovered candidate during Select Widget mode.
    pub fn note_hover(&mut self, id: DevWidgetId, bounds: [f32; 4]) {
        self.hover = Some((id, bounds));
    }

    /// Completes selection: emits the event and exits mode unless locked.
    pub fn complete_selection(
        &mut self,
        window: incular_platform::WindowId,
    ) -> Option<DevWidgetId> {
        if self.is_selecting(window) {
            self.select_window = None;
            self.hover.as_ref().map(|(id, _)| *id)
        } else {
            None
        }
    }

    /// Maps a generational protocol window id to its live platform window.
    fn resolve_window(
        &mut self,
        application: &incular_runtime::Application,
        id: DevWindowId,
    ) -> Option<incular_platform::WindowId> {
        if let Some(existing) = self.window_map.get(&id) {
            return Some(*existing);
        }
        let mapped = application.active_window_ids().into_iter().find(|window| {
            u64::from(window.index()) + 1 == id.index()
                && u64::from(window.generation()) == id.generation()
        })?;
        self.window_map.insert(id, mapped);
        Some(mapped)
    }

    /// Monotonic per-session frame counter for streamed records.
    pub fn next_frame(&mut self) -> u64 {
        self.frame_counter += 1;
        self.frame_counter
    }

    /// Drains queued DevTools commands; called once per frame on UI thread.
    pub fn drain(&mut self, application: &mut incular_runtime::Application) {
        let Some(agent) = self.agent.as_ref().map(|agent| agent.commands.clone()) else {
            return;
        };
        while let Some(command) = agent.try_next() {
            let UiCommand { request_id, body } = command;
            let reply = match body {
                RequestMethod::StartInspectMode { window } => {
                    if let Some(platform_window) = self.resolve_window(application, window) {
                        self.select_window = Some(platform_window);
                        UiReply::ok(request_id)
                    } else {
                        UiReply::error(request_id, ErrorCode::UnknownId)
                    }
                }
                RequestMethod::StopInspectMode { window } => {
                    if let Some(platform_window) = self.resolve_window(application, window)
                        && self.is_selecting(platform_window)
                    {
                        self.select_window = None;
                    }
                    UiReply::ok(request_id)
                }
                RequestMethod::GetTargetInfo => UiReply::with(
                    request_id,
                    ResponsePayload::TargetInfo(Box::new(incular_devtools::target_info(
                        env!("CARGO_PKG_VERSION"),
                        application.devtools_windows(),
                    ))),
                ),
                RequestMethod::GetWidgetTree { window } => {
                    match application.devtools_widget_tree(window) {
                        Ok(deltas) => {
                            self.tree_subscriptions.insert(window);
                            self.remember_tree(window, &deltas);
                            UiReply::with(
                                request_id,
                                ResponsePayload::WidgetTree {
                                    deltas,
                                    tree_revision: self.frame_counter,
                                },
                            )
                        }
                        Err(code) => UiReply::error(request_id, code),
                    }
                }
                RequestMethod::GetNodeDetails { id } => match application.devtools_node_details(id)
                {
                    Ok(mut details) => {
                        for property in &mut details.properties {
                            property.overridden = self
                                .property_overrides
                                .contains_key(&(id, property.name.clone()));
                        }
                        UiReply::with(request_id, ResponsePayload::NodeDetails(Box::new(details)))
                    }
                    Err(code) => UiReply::error(request_id, code),
                },
                RequestMethod::EditProperty { id, name, value } => {
                    let original = self
                        .property_overrides
                        .get(&(id, name.clone()))
                        .map(|entry| entry.original.clone())
                        .or_else(|| {
                            application
                                .devtools_node_details(id)
                                .ok()?
                                .properties
                                .into_iter()
                                .find(|property| property.name == name && property.editable)
                                .map(|property| property.value)
                        });
                    if let Some(original) = original
                        && application.devtools_edit_property(id, &name, &value)
                    {
                        self.property_overrides.insert(
                            (id, name),
                            PropertyOverride {
                                original,
                                current: value,
                            },
                        );
                        UiReply::with(request_id, ResponsePayload::Edited)
                    } else {
                        UiReply::error(request_id, ErrorCode::Unsupported)
                    }
                }
                RequestMethod::HighlightNode { id, .. } => {
                    let geometry = id.and_then(|id| {
                        application
                            .devtools_node_overlay_geometry(id)
                            .map(|geometry| (id, geometry))
                    });
                    self.highlight = geometry.map(|(id, geometry)| (id, geometry.bounds));
                    self.highlight_content =
                        geometry.and_then(|(_, geometry)| geometry.content_bounds);
                    self.highlight_baseline =
                        geometry.and_then(|(_, geometry)| geometry.baseline_y);
                    self.highlight_clip = geometry.and_then(|(_, geometry)| geometry.clip_bounds);
                    UiReply::ok(request_id)
                }
                RequestMethod::SetDebugOption { name, enabled } => {
                    if enabled {
                        self.options.insert(name);
                    } else {
                        self.options.remove(&name);
                    }
                    UiReply::ok(request_id)
                }
                RequestMethod::SetAnimationSpeed { scale } => {
                    self.animation_speed = if scale.is_finite() {
                        scale.clamp(0., 1.)
                    } else {
                        1.
                    };
                    application.set_animation_time_scale(self.animation_speed);
                    UiReply::with(
                        request_id,
                        ResponsePayload::AnimationSpeedSet(self.animation_speed()),
                    )
                }
                RequestMethod::SetProfilerMode { mode } => {
                    self.profiler_mode = mode;
                    application.set_profiler_mode(match mode {
                        DevtoolsProfilerMode::Basic => incular_runtime::ProfilerMode::Normal,
                        DevtoolsProfilerMode::Performance => {
                            incular_runtime::ProfilerMode::Diagnostic
                        }
                        DevtoolsProfilerMode::Deep => incular_runtime::ProfilerMode::Profiling,
                    });
                    UiReply::with(request_id, ResponsePayload::ProfilerModeSet(mode))
                }
                RequestMethod::StartRecording => {
                    self.recording = true;
                    self.recorded_frames = 0;
                    UiReply::with(request_id, ResponsePayload::RecordingStarted)
                }
                RequestMethod::StopRecording => {
                    self.recording = false;
                    UiReply::with(
                        request_id,
                        ResponsePayload::RecordingStopped {
                            frames: self.recorded_frames,
                        },
                    )
                }
                RequestMethod::TakeMemorySnapshot { label } => {
                    let mut counts = application.devtools_resource_counts();
                    counts.rss_mb = incular_devtools::process_rss_mb();
                    UiReply::with(
                        request_id,
                        ResponsePayload::MemorySnapshot(MemorySnapshot { label, counts }),
                    )
                }
                RequestMethod::ListSignals => UiReply::with(
                    request_id,
                    ResponsePayload::Signals(application.devtools_signals()),
                ),
                RequestMethod::GetSignalSubscribers { id } => UiReply::with(
                    request_id,
                    ResponsePayload::SignalSubscribers(application.devtools_signal_subscribers(id)),
                ),
                RequestMethod::EditSignal { id, value } => {
                    if application.devtools_edit_signal(id, &value) {
                        UiReply::with(request_id, ResponsePayload::Edited)
                    } else {
                        UiReply::error(
                            request_id,
                            incular_devtools_protocol::ErrorCode::Unsupported,
                        )
                    }
                }
                RequestMethod::ResetOverrides => {
                    let overrides = std::mem::take(&mut self.property_overrides);
                    for ((id, name), property) in overrides {
                        let _ = application.devtools_edit_property(id, &name, &property.original);
                    }
                    UiReply::with(request_id, ResponsePayload::OverridesReset)
                }
                _ => UiReply::error(
                    request_id,
                    incular_devtools_protocol::ErrorCode::Unsupported,
                ),
            };
            if let Some(agent) = &self.agent {
                let _ = agent.reply_sender.send(reply);
            }
        }
        self.property_overrides.retain(|(id, name), property| {
            application.devtools_edit_property(*id, name, &property.current)
        });
        self.refresh_layout_bounds(application);
        self.refresh_auxiliary_overlays(application);
        self.refresh_phase_flashes(application);
    }

    fn refresh_auxiliary_overlays(&mut self, application: &incular_runtime::Application) {
        self.hit_regions.clear();
        self.semantics_bounds.clear();
        self.scroll_viewports.clear();
        self.layer_bounds.clear();
        const LIMIT: usize = incular_widgets::devtools::MAX_OVERLAY_NODES;
        for window in application.active_window_ids() {
            if self.options.contains(&DebugOption::HitTestRegions) {
                self.hit_regions.insert(
                    window,
                    application.devtools_window_hit_regions(window, LIMIT),
                );
            }
            if self.options.contains(&DebugOption::SemanticsBounds) {
                self.semantics_bounds.insert(
                    window,
                    application.devtools_window_semantics_bounds(window, LIMIT),
                );
            }
            if self.options.contains(&DebugOption::ScrollViewports) {
                self.scroll_viewports.insert(
                    window,
                    application.devtools_window_scroll_viewports(window, LIMIT),
                );
            }
            if self.options.contains(&DebugOption::LayerBoundaries) {
                self.layer_bounds.insert(
                    window,
                    application.devtools_window_layer_bounds(window, LIMIT),
                );
            }
        }
    }

    fn refresh_layout_bounds(&mut self, application: &incular_runtime::Application) {
        let active = self.options.contains(&DebugOption::LayoutBoundsSubtree)
            || self.options.contains(&DebugOption::LayoutBoundsWholeWindow);
        if !active {
            self.layout_bounds.clear();
            return;
        }
        if self
            .last_layout_bounds_sample
            .is_some_and(|last| last.elapsed() < Duration::from_millis(125))
        {
            return;
        }
        self.last_layout_bounds_sample = Some(Instant::now());
        self.layout_bounds.clear();
        const LIMIT: usize = incular_widgets::devtools::MAX_OVERLAY_NODES;
        if self.options.contains(&DebugOption::LayoutBoundsWholeWindow) {
            for window in application.active_window_ids() {
                self.layout_bounds.insert(
                    window,
                    application.devtools_window_layout_bounds(window, LIMIT),
                );
            }
        } else if let Some((id, _)) = self.highlight
            && let Some((window, bounds)) = application.devtools_subtree_layout_bounds(id, LIMIT)
        {
            self.layout_bounds.insert(window, bounds);
        }
    }

    fn refresh_phase_flashes(&mut self, application: &incular_runtime::Application) {
        let enabled = [
            DebugOption::HighlightBuild,
            DebugOption::HighlightLayout,
            DebugOption::HighlightPaint,
            DebugOption::HighlightSemantics,
            DebugOption::HighlightComposite,
        ]
        .into_iter()
        .any(|option| self.options.contains(&option))
            || self.options.contains(&DebugOption::RepaintRainbow);
        if !enabled {
            self.phase_flashes.clear();
            self.phase_counters.clear();
            self.repaint_generations.clear();
            self.repaint_rainbow.clear();
            return;
        }
        const LIMIT: usize = incular_widgets::devtools::MAX_OVERLAY_NODES;
        for window in application.active_window_ids() {
            let previous = self.phase_counters.entry(window).or_default();
            let mut next = std::collections::HashMap::new();
            let mut flashes = Vec::new();
            let generations = self.repaint_generations.entry(window).or_default();
            let mut rainbow = Vec::new();
            for node in application.devtools_window_phase_nodes(window, LIMIT) {
                let counters = [
                    node.builds,
                    node.layouts,
                    node.paints,
                    node.semantics,
                    node.composites,
                ];
                if let Some(before) = previous.get(&node.id) {
                    let changed = [
                        counters[0] > before[0],
                        counters[1] > before[1],
                        counters[2] > before[2],
                        counters[3] > before[3],
                        counters[4] > before[4],
                    ];
                    if changed.into_iter().any(|value| value) {
                        flashes.push((node.bounds, changed));
                    }
                    if changed[2] {
                        let generation = generations
                            .entry(node.id)
                            .and_modify(|generation| *generation = generation.wrapping_add(1))
                            .or_insert(0);
                        rainbow.push((node.bounds, *generation));
                    }
                }
                next.insert(node.id, counters);
            }
            *previous = next;
            self.phase_flashes.insert(window, flashes);
            self.repaint_rainbow.insert(window, rainbow);
        }
    }

    /// Streams one frame record to connected DevTools clients.
    pub fn push_frame(&self, event: TargetEvent) {
        if let Some(agent) = &self.agent {
            agent.telemetry.try_push(event);
        }
    }

    /// Activates bounded node timestamps only for an explicit Deep recording.
    pub fn begin_deep_frame(
        &self,
        application: &mut incular_runtime::Application,
        window: incular_platform::WindowId,
    ) {
        if self.recording && self.profiler_mode == DevtoolsProfilerMode::Deep {
            const MAX_EVENTS_PER_FRAME: usize = 4_096;
            application.devtools_begin_deep_trace(window, MAX_EVENTS_PER_FRAME);
        }
    }

    /// Returns whether this rendered frame belongs to the bounded recording.
    /// At 300 frames capture stops explicitly and a visible diagnostic is sent.
    pub fn note_recorded_frame(&mut self) -> bool {
        if !self.recording {
            return false;
        }
        self.recorded_frames = self.recorded_frames.saturating_add(1);
        if self.recorded_frames >= 300 {
            self.recording = false;
            self.push_frame(TargetEvent::Log {
                level: "info".into(),
                target: "incular::devtools".into(),
                message: "recording stopped: 300-frame limit reached".into(),
            });
        }
        true
    }

    pub fn deep_recording(&self) -> bool {
        self.profiler_mode == DevtoolsProfilerMode::Deep
            && (self.recording || self.recorded_frames == 300)
    }

    /// Draws the selected/hovered exact world bounds after the retained
    /// display list. This is a compositor-only debug adornment: it neither
    /// enters the widget tree nor changes its constraints, hit testing, or
    /// retained layout state.
    pub fn paint_overlay(
        &self,
        window: incular_platform::WindowId,
        list: &mut incular_rendering::DisplayList,
    ) {
        // Adjacent Rect commands are consumed by the renderer's existing
        // rect-instance batch. This is one diagnostic display-list pass, not
        // a widget or a GPU pass per target node.
        if let Some(bounds) = self.layout_bounds.get(&window) {
            for &[x, y, width, height] in bounds {
                if width > 0. && height > 0. {
                    list.push(incular_rendering::PaintCommand::Rect {
                        rect: incular_core::Rect::from_origin_size(
                            incular_core::Offset::new(x, y),
                            incular_core::Size::new(width, height),
                        ),
                        color: incular_core::Color::rgba(255, 172, 44, 22),
                    });
                }
            }
        }
        for (regions, color) in [
            (
                self.hit_regions.get(&window),
                incular_core::Color::rgba(72, 214, 164, 42),
            ),
            (
                self.semantics_bounds.get(&window),
                incular_core::Color::rgba(246, 214, 72, 42),
            ),
            (
                self.scroll_viewports.get(&window),
                incular_core::Color::rgba(65, 188, 246, 42),
            ),
            (
                self.layer_bounds.get(&window),
                incular_core::Color::rgba(190, 102, 246, 34),
            ),
        ] {
            for &[x, y, width, height] in regions.into_iter().flatten() {
                if width > 0. && height > 0. {
                    list.push(incular_rendering::PaintCommand::Rect {
                        rect: incular_core::Rect::from_origin_size(
                            incular_core::Offset::new(x, y),
                            incular_core::Size::new(width, height),
                        ),
                        color,
                    });
                }
            }
        }
        if self.options.contains(&DebugOption::PaddingContent)
            && let Some([x, y, width, height]) = self.highlight_content
        {
            list.push(incular_rendering::PaintCommand::Rect {
                rect: incular_core::Rect::from_origin_size(
                    incular_core::Offset::new(x, y),
                    incular_core::Size::new(width, height),
                ),
                color: incular_core::Color::rgba(62, 218, 142, 72),
            });
        }
        if let Some(flashes) = self.phase_flashes.get(&window) {
            for &(bounds, changed) in flashes {
                for (index, (option, color)) in [
                    (
                        DebugOption::HighlightBuild,
                        incular_core::Color::rgba(248, 98, 88, 70),
                    ),
                    (
                        DebugOption::HighlightLayout,
                        incular_core::Color::rgba(246, 194, 62, 70),
                    ),
                    (
                        DebugOption::HighlightPaint,
                        incular_core::Color::rgba(114, 166, 255, 70),
                    ),
                    (
                        DebugOption::HighlightSemantics,
                        incular_core::Color::rgba(78, 212, 146, 70),
                    ),
                    (
                        DebugOption::HighlightComposite,
                        incular_core::Color::rgba(170, 108, 244, 70),
                    ),
                ]
                .into_iter()
                .enumerate()
                {
                    if changed[index] && self.options.contains(&option) {
                        let [x, y, width, height] = bounds;
                        list.push(incular_rendering::PaintCommand::Rect {
                            rect: incular_core::Rect::from_origin_size(
                                incular_core::Offset::new(x, y),
                                incular_core::Size::new(width, height),
                            ),
                            color,
                        });
                    }
                }
            }
        }
        if self.options.contains(&DebugOption::RepaintRainbow)
            && let Some(regions) = self.repaint_rainbow.get(&window)
        {
            const COLORS: [incular_core::Color; 6] = [
                incular_core::Color::rgba(242, 94, 112, 72),
                incular_core::Color::rgba(244, 177, 72, 72),
                incular_core::Color::rgba(122, 202, 112, 72),
                incular_core::Color::rgba(72, 184, 222, 72),
                incular_core::Color::rgba(118, 126, 242, 72),
                incular_core::Color::rgba(206, 112, 234, 72),
            ];
            for &(bounds, generation) in regions {
                let [x, y, width, height] = bounds;
                list.push(incular_rendering::PaintCommand::Rect {
                    rect: incular_core::Rect::from_origin_size(
                        incular_core::Offset::new(x, y),
                        incular_core::Size::new(width, height),
                    ),
                    color: COLORS[usize::from(generation) % COLORS.len()],
                });
            }
        }
        if self.options.contains(&DebugOption::Clips)
            && let Some([x, y, width, height]) = self.highlight_clip
            && width > 0.
            && height > 0.
        {
            list.push(incular_rendering::PaintCommand::Rect {
                rect: incular_core::Rect::from_origin_size(
                    incular_core::Offset::new(x, y),
                    incular_core::Size::new(width, height),
                ),
                color: incular_core::Color::rgba(240, 72, 92, 48),
            });
        }
        if self.options.contains(&DebugOption::Baselines)
            && let Some(y) = self.highlight_baseline
            && let Some((_, [x, _, width, _])) = self.highlight
            && width > 0.
        {
            list.push(incular_rendering::PaintCommand::Rect {
                rect: incular_core::Rect::from_origin_size(
                    incular_core::Offset::new(x, y),
                    incular_core::Size::new(width, 1.),
                ),
                color: incular_core::Color::rgba(104, 232, 130, 180),
            });
        }
        let bounds = self
            .highlight
            .or_else(|| self.hover.filter(|_| self.select_window.is_some()));
        let Some((_, [x, y, width, height])) = bounds else {
            return;
        };
        if width <= 0. || height <= 0. {
            return;
        }
        list.push(incular_rendering::PaintCommand::Rect {
            rect: incular_core::Rect::from_origin_size(
                incular_core::Offset::new(x, y),
                incular_core::Size::new(width, height),
            ),
            color: incular_core::Color::rgba(40, 160, 255, 72),
        });
    }

    /// Samples subscribed trees at most four times per second and sends only
    /// structural/configuration deltas. This bounded cadence prevents a
    /// static 100k-node target from being repeatedly serialized at frame rate.
    pub fn stream_tree_updates(&mut self, application: &incular_runtime::Application) {
        if self.tree_subscriptions.is_empty()
            || self
                .last_tree_sample
                .is_some_and(|last| last.elapsed() < Duration::from_millis(250))
        {
            return;
        }
        self.last_tree_sample = Some(Instant::now());
        let windows: Vec<_> = self.tree_subscriptions.iter().copied().collect();
        for window in windows {
            let Ok(snapshot) = application.devtools_widget_tree(window) else {
                self.tree_subscriptions.remove(&window);
                self.tree_cache.remove(&window);
                continue;
            };
            let next = tree_nodes(&snapshot);
            let previous = self.tree_cache.entry(window).or_default();
            let mut deltas = Vec::new();
            for id in previous.keys().filter(|id| !next.contains_key(id)) {
                deltas.push(TreeDelta::Remove { id: *id });
            }
            for (id, node) in &next {
                match previous.get(id) {
                    None => deltas.push(TreeDelta::Insert {
                        node: Box::new(node.clone()),
                    }),
                    Some(old) if old != node => deltas.push(TreeDelta::Update {
                        node: Box::new(node.clone()),
                    }),
                    Some(_) => {}
                }
            }
            *previous = next;
            if !deltas.is_empty() {
                self.push_frame(TargetEvent::WidgetTreeDeltas {
                    window,
                    deltas,
                    tree_revision: self.frame_counter,
                });
            }
        }
    }

    fn remember_tree(&mut self, window: DevWindowId, deltas: &[TreeDelta]) {
        self.tree_cache.insert(window, tree_nodes(deltas));
    }

    /// Emits user-driven widget selection during Select Widget mode.
    pub fn emit_selection(&mut self, window: incular_platform::WindowId, id: DevWidgetId) {
        let dev_window = self
            .window_map
            .iter()
            .find(|(_, platform)| **platform == window)
            .map(|(id, _)| *id);
        if let Some(dev_window) = dev_window {
            self.push_frame(TargetEvent::WidgetSelectedByUser {
                window: dev_window,
                id,
            });
        }
    }

    /// Intercepts Select Widget pointer input before normal dispatch. Hover
    /// never alters app state; click reports the retained target and consumes
    /// this one selection gesture.
    pub fn inspect_pointer(
        &mut self,
        application: &incular_runtime::Application,
        window: incular_platform::WindowId,
        point: incular_core::Offset,
        select: bool,
    ) -> bool {
        if !self.is_selecting(window) {
            return false;
        }
        if let Some((id, bounds)) = application.devtools_hit_test(window, point) {
            self.note_hover(id, bounds);
            if select {
                self.emit_selection(window, id);
                self.select_window = None;
            }
        }
        true
    }
}

fn tree_nodes(deltas: &[TreeDelta]) -> std::collections::HashMap<DevWidgetId, WidgetNode> {
    let mut nodes = std::collections::HashMap::new();
    for delta in deltas {
        if let TreeDelta::Snapshot {
            root, nodes: rest, ..
        } = delta
        {
            nodes.insert(root.id, (**root).clone());
            nodes.extend(rest.iter().cloned().map(|node| (node.id, node)));
        }
    }
    nodes
}
