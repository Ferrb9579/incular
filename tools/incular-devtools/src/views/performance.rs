use super::{
    CONTROL, CONTROL_ACTIVE, DANGER, PRIMARY, SUCCESS, TEXT_MUTED, TEXT_PRIMARY, compact_button,
    gap, section, ui_text,
};
use crate::{
    inspector::Shared,
    performance::{FlameBox, RankedTrace, TraceRange},
    transport::ClientBridge,
};
use incular::material::RawMaterialButton;
use incular::prelude::*;
use incular_devtools_protocol::{
    DevWindowId, DevtoolsProfilerMode, FrameRecordEvent, RequestMethod, TracePhase,
};

pub(crate) struct PerformanceData {
    pub(crate) fps: Option<f32>,
    pub(crate) active_window: Option<DevWindowId>,
    pub(crate) frames: Vec<FrameRecordEvent>,
    pub(crate) latest_frame: Option<FrameRecordEvent>,
    pub(crate) profiler_mode: DevtoolsProfilerMode,
    pub(crate) recording: bool,
    pub(crate) flame_boxes: Vec<FlameBox>,
    pub(crate) ranked: Vec<RankedTrace>,
    pub(crate) trace_status: Option<String>,
    pub(crate) selected_frame_detail: Option<String>,
    pub(crate) selected_frame: Option<(DevWindowId, u64)>,
    pub(crate) trace_range: TraceRange,
    pub(crate) selected_range: Option<(u64, u64)>,
}

pub(crate) fn build_performance(
    data: PerformanceData,
    shared: Shared,
    bridge: ClientBridge,
    tick: Signal<u64>,
) -> Widget {
    let PerformanceData {
        fps,
        active_window,
        frames,
        latest_frame,
        profiler_mode,
        recording,
        flame_boxes,
        ranked,
        trace_status,
        selected_frame_detail,
        selected_frame,
        trace_range,
        selected_range,
    } = data;
    let mut performance_controls = Vec::new();
    let mut timeline_controls = Vec::new();
    let mut performance_body = Vec::new();

    for (mode, label) in [
        (DevtoolsProfilerMode::Basic, "Profiler: Basic"),
        (DevtoolsProfilerMode::Performance, "Profiler: Performance"),
        (DevtoolsProfilerMode::Deep, "Profiler: Deep"),
    ] {
        let mode_bridge = bridge.clone();
        performance_controls.push(compact_button(label, profiler_mode == mode, move || {
            mode_bridge.send(RequestMethod::SetProfilerMode { mode })
        }));
    }
    let recording_bridge = bridge.clone();
    performance_controls.push(
        RawMaterialButton::new(if recording {
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
            recording_bridge.send(if recording {
                RequestMethod::StopRecording
            } else {
                RequestMethod::StartRecording
            })
        })
        .into(),
    );
    let clear_shared = shared.clone();
    let clear_tick = tick.clone();
    performance_controls.push(compact_button("Clear recording", false, move || {
        if let Ok(mut state) = clear_shared.lock() {
            state.frames.clear();
            state.deep_traces.clear();
            state.deep_trace_events = 0;
            state.timeline_offset = 0;
            state.selected_frame = None;
            state.selected_range = None;
            state.range_anchor = None;
        }
        clear_tick.update(|value| *value = value.wrapping_add(1));
    }));

    for (label, visible_delta, offset_delta) in [
        ("Timeline zoom in", -4_isize, 0_isize),
        ("Timeline zoom out", 4, 0),
        ("Timeline older", 0, 4),
        ("Timeline newer", 0, -4),
    ] {
        let timeline_shared = shared.clone();
        let timeline_tick = tick.clone();
        timeline_controls.push(compact_button(label, false, move || {
            if let Ok(mut state) = timeline_shared.lock() {
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
            timeline_tick.update(|value| *value = value.wrapping_add(1));
        }));
    }

    performance_body.push(ui_text(
        format!(
            "{} FPS · {} active window · {} retained frame samples",
            fps.map_or_else(|| "—".into(), |value| format!("{value:.1}")),
            active_window.map_or_else(|| "none".into(), |window| window.to_string()),
            frames.len(),
        ),
        18.,
        if fps.is_some() { SUCCESS } else { TEXT_MUTED },
    ));
    if let Some(frame) = latest_frame {
        performance_body.push(ui_text(
            format!(
                "Latest frame #{} · CPU {}µs · GPU {} · {} draws · {} instances",
                frame.frame,
                frame.timings.cpu_total,
                frame
                    .timings
                    .gpu_us
                    .map_or_else(|| "unavailable".into(), |value| format!("{value:.0}µs"),),
                frame.draw_calls,
                frame.instances,
            ),
            13.,
            TEXT_PRIMARY,
        ));
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
        let frame_shared = shared.clone();
        let frame_tick = tick.clone();
        RawMaterialButton::new(label)
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
                if let Ok(mut state) = frame_shared.lock() {
                    state.selected_frame = Some((frame.window, frame.frame));
                    if let Some(anchor) = state.range_anchor.take() {
                        state.selected_range = Some((anchor, frame.frame));
                    }
                }
                frame_tick.update(|value| *value = value.wrapping_add(1));
            })
            .into()
    }));
    if let Some(detail) = selected_frame_detail {
        performance_body.push(ui_text(detail, 12., TEXT_MUTED));
    }
    let range_shared = shared.clone();
    let range_tick = tick.clone();
    timeline_controls.push(compact_button(
        if selected_range.is_some() {
            "Reset range start to selected frame"
        } else {
            "Set range start from selected frame"
        },
        selected_range.is_some(),
        move || {
            if let Ok(mut state) = range_shared.lock() {
                state.range_anchor = state.selected_frame.map(|(_, frame)| frame);
                state.selected_range = None;
            }
            range_tick.update(|value| *value = value.wrapping_add(1));
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
        performance_body.push(CustomPaint::new(Size::new(900., height), canvas.finish()).into());
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
        let rank_shared = shared.clone();
        let rank_tick = tick.clone();
        timeline_controls.push(compact_button(label, trace_range == range, move || {
            if let Ok(mut state) = rank_shared.lock() {
                state.trace_range = range;
            }
            rank_tick.update(|value| *value = value.wrapping_add(1));
        }));
    }
    performance_body.extend(ranked.into_iter().take(24).map(|entry| {
        let label = format!(
            "{:?} · {} · {}µs across {} events",
            entry.phase, entry.node, entry.total_us, entry.count
        );
        let entry_bridge = bridge.clone();
        let entry_shared = shared.clone();
        let entry_tick = tick.clone();
        RawMaterialButton::new(label)
            .size(Size::new(0., 34.))
            .padding(EdgeInsets::symmetric(10., 7.))
            .label_style(TextStyle {
                size: 12.,
                color: TEXT_PRIMARY,
                ..TextStyle::default()
            })
            .color(CONTROL)
            .on_press(move || {
                let window = if let Ok(mut state) = entry_shared.lock() {
                    state.selected = Some(entry.node);
                    state.active_window
                } else {
                    None
                };
                entry_bridge.send(RequestMethod::GetNodeDetails { id: entry.node });
                if let Some(window) = window {
                    entry_bridge.send(RequestMethod::HighlightNode {
                        window,
                        id: Some(entry.node),
                    });
                }
                entry_tick.update(|value| *value = value.wrapping_add(1));
            })
            .into()
    }));

    Column::new([
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
    ])
    .into()
}
