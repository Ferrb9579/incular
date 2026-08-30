mod application;
mod console;
mod memory;
mod network;
mod performance;
mod shared;
mod shell;
mod widget_inspector;

pub(crate) use self::shared::{
    APP_BACKGROUND, BORDER, CONTROL, CONTROL_ACTIVE, DANGER, PRIMARY, SUCCESS, SURFACE, TEXT_MUTED,
    TEXT_PRIMARY, compact_button, gap, section, ui_text,
};
pub(crate) use self::shell::{ToolView, initial_tool_view};

use crate::{
    inspector::{InspectorSection, Shared},
    performance::{TraceRange, flamegraph_boxes, rank_traces},
    transport::ClientBridge,
};
use incular::prelude::*;
use incular::widgets::internal::{ScrollController, TextEditingController};
use incular_devtools_protocol::{
    DebugOption, DevWidgetId, DevWindowId, DevtoolsProfilerMode, FrameRecordEvent, MemorySnapshot,
    NodeDetails, SignalSubscriber, SignalSummary, TargetInfo, WindowSummary,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

struct ViewSnapshot {
    header: String,
    target_info: Option<TargetInfo>,
    connected: bool,
    windows: Vec<WindowSummary>,
    active_window: Option<DevWindowId>,
    row_count: usize,
    select_mode: bool,
    has_selection: bool,
    details: Vec<String>,
    selected_details: Option<NodeDetails>,
    frames: Vec<FrameRecordEvent>,
    memory: Option<MemorySnapshot>,
    memory_diff: Vec<String>,
    fps: Option<f32>,
    latest_frame: Option<FrameRecordEvent>,
    console_entries: Vec<crate::inspector::ConsoleEntry>,
    signals: Vec<SignalSummary>,
    selected_signal: Option<SignalSummary>,
    signal_subscribers: Vec<SignalSubscriber>,
    debug_options: HashSet<DebugOption>,
    animation_scale: f32,
    profiler_mode: DevtoolsProfilerMode,
    recording: bool,
    flame_boxes: Vec<crate::performance::FlameBox>,
    ranked: Vec<crate::performance::RankedTrace>,
    trace_status: Option<String>,
    selected_frame_detail: Option<String>,
    selected_frame: Option<(DevWindowId, u64)>,
    trace_range: TraceRange,
    selected_range: Option<(u64, u64)>,
    error: Option<String>,
}

pub(crate) fn run(shared: Shared, bridge: ClientBridge) {
    let tick = Signal::new(0_u64);
    let pending = Rc::new(Cell::new(false));
    let search = TextEditingController::new();
    let console_filter = TextEditingController::new();
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
    let app_console_filter = console_filter.clone();
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
            let snapshot = {
                let state = app_shared.lock().expect("inspector state");
                let header = if state.connected {
                    format!("Connected to {}", state.target)
                } else {
                    "Connecting to target".into()
                };
                let target_info = state.target_info.clone();
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
                let memory = state.memory.clone();
                let fps = state.fps(state.active_window);
                let latest_frame = state.latest_frame(state.active_window).cloned();
                let console_entries = state.filtered_console();
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
                    .and_then(|(window, selected)| {
                        state
                            .frames
                            .iter()
                            .find(|frame| frame.window == window && frame.frame == selected)
                    })
                    .map(|frame| {
                        format!(
                            "Frame #{} · CPU {}µs · budget {} · input {} runtime {} BUILD {} LAYOUT {} PAINT {} SEMANTICS {} COMPOSITE {} · renderer prepare {} encode {} submit {} · GPU {} · draws {} instances {} uploads {}B",
                            frame.frame,
                            frame.timings.cpu_total,
                            frame
                                .budget_us
                                .map_or_else(|| "unavailable".into(), |value| format!("{value}µs")),
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
                            frame
                                .timings
                                .gpu_us
                                .map_or_else(|| "unavailable".into(), |value| format!("{value:.0}µs")),
                            frame.draw_calls,
                            frame.instances,
                            frame.upload_bytes,
                        )
                    });
                ViewSnapshot {
                    header,
                    target_info,
                    connected: state.connected,
                    windows: state.windows.clone(),
                    active_window: state.active_window,
                    row_count: state.rows.len(),
                    select_mode: state.select_mode,
                    has_selection: state.selected.is_some(),
                    details: state.details_lines(active_inspector_section),
                    selected_details: state.details.clone(),
                    frames,
                    memory,
                    memory_diff: state.memory_diff_lines(),
                    fps,
                    latest_frame,
                    console_entries,
                    signals: state.signals.clone(),
                    selected_signal: state.selected_signal.and_then(|id| {
                        state.signals.iter().find(|signal| signal.id == id).cloned()
                    }),
                    signal_subscribers: state.signal_subscribers.clone(),
                    debug_options: state.debug_options.clone(),
                    animation_scale: state.animation_scale.unwrap_or(1.),
                    profiler_mode: state.profiler_mode,
                    recording: state.recording,
                    flame_boxes,
                    ranked,
                    trace_status,
                    selected_frame_detail,
                    selected_frame: state.selected_frame,
                    trace_range: state.trace_range,
                    selected_range: state.selected_range,
                    error: state.error.clone(),
                }
            };

            let inspector = widget_inspector::build_inspector(
                widget_inspector::InspectorData {
                    error: snapshot.error.clone(),
                    windows: snapshot.windows.clone(),
                    active_window: snapshot.active_window,
                    select_mode: snapshot.select_mode,
                    debug_options: snapshot.debug_options.clone(),
                    animation_scale: snapshot.animation_scale,
                    details: snapshot.details.clone(),
                    selected_details: snapshot.selected_details.clone(),
                    has_selection: snapshot.has_selection,
                    signals: snapshot.signals.clone(),
                    selected_signal: snapshot.selected_signal.clone(),
                    signal_subscribers: snapshot.signal_subscribers.clone(),
                    row_count: snapshot.row_count,
                },
                widget_inspector::InspectorBuildContext {
                    shared: Arc::clone(&app_shared),
                    bridge: app_bridge.clone(),
                    tick: app_tick.clone(),
                    search: app_search.clone(),
                    signal_value: app_signal_value.clone(),
                    property_value: app_property_value.clone(),
                    property_binding: app_property_binding.clone(),
                    inspector_section: app_inspector_section.clone(),
                    tree_scroll: app_tree_scroll.clone(),
                },
            );
            let console_content = console::build_console(
                snapshot.console_entries.clone(),
                Arc::clone(&app_shared),
                app_tick.clone(),
                app_console_filter.clone(),
            );
            let network_content = network::build_network();
            let memory_content = memory::build_memory(
                snapshot.memory.clone(),
                snapshot.memory_diff.clone(),
                app_bridge.clone(),
            );
            let application_content = application::build_application(
                snapshot.target_info.clone(),
                snapshot.connected,
                snapshot.windows.clone(),
                snapshot.frames.clone(),
                app_bridge.clone(),
            );
            let performance_content = performance::build_performance(
                performance::PerformanceData {
                    fps: snapshot.fps,
                    active_window: snapshot.active_window,
                    frames: snapshot.frames,
                    latest_frame: snapshot.latest_frame,
                    profiler_mode: snapshot.profiler_mode,
                    recording: snapshot.recording,
                    flame_boxes: snapshot.flame_boxes,
                    ranked: snapshot.ranked,
                    trace_status: snapshot.trace_status,
                    selected_frame_detail: snapshot.selected_frame_detail,
                    selected_frame: snapshot.selected_frame,
                    trace_range: snapshot.trace_range,
                    selected_range: snapshot.selected_range,
                },
                Arc::clone(&app_shared),
                app_bridge.clone(),
                app_tick.clone(),
            );
            let page_content: Widget = match active_view {
                ToolView::Widgets => inspector.content,
                ToolView::Console => console_content,
                ToolView::Network => network_content,
                ToolView::Performance => performance_content,
                ToolView::Memory => memory_content,
                ToolView::Application => application_content,
            };
            shell::build_shell(shell::ShellData {
                active_view,
                tool_view: app_tool_view.clone(),
                header: snapshot.header,
                connected: snapshot.connected,
                row_count: snapshot.row_count,
                search_field: inspector.search_field,
                tree_list: inspector.tree_list,
                inspector_scroll: app_inspector_scroll.clone(),
                page_content,
            })
        },
    );
    match app {
        Ok(app) => incular::run(app).expect("devtools application"),
        Err(error) => eprintln!("unable to start DevTools: {error:?}"),
    }
}
