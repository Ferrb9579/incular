use incular::prelude::*;
use incular::runtime::Runtime;
use incular::widgets::internal::ScrollView;
use incular_devtools_protocol::*;
use incular_devtools_ui::{
    inspector::{InspectorModel, TreeRow, editable_value, parse_debug_value},
    performance::{flamegraph_boxes, rank_traces},
    session::{requested_target_pid, select_session},
    views::{
        APP_BACKGROUND, BORDER, SURFACE, TEXT_PRIMARY, ToolView, compact_button, gap, ui_text,
    },
};

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
    assert_eq!(state.row_count(), 100_001);
    assert_eq!(state.selected_node(), Some(id(1)));
    assert!(std::mem::size_of::<TreeRow>() <= 24);
    state.apply_tree(2, [TreeDelta::Remove { id: id(2) }]);
    assert_eq!(state.row_count(), 100_000);
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
    assert_eq!(state.row_count(), 2);
    assert!(state.toggle_expanded(id(2)));
    assert_eq!(state.row_count(), 3);
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
    assert_eq!(state.selected_node(), Some(id(1)));
    assert!(state.toggle_expanded(id(2)));
    assert_eq!(state.row_count(), 3);
    assert_eq!(state.selected_node(), Some(id(1)));
    state.collapse_all();
    assert_eq!(state.row_count(), 1);
    assert_eq!(state.selected_node(), Some(id(1)));
    state.expand_all();
    assert_eq!(state.row_count(), 3);
    assert_eq!(state.selected_node(), Some(id(1)));
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
    let mut state = InspectorModel::default();
    state.set_memory_snapshots(
        MemorySnapshot {
            label: "A".into(),
            counts: incular_devtools_protocol::ResourceCounts {
                elements: 10,
                rss_mb: 100,
                ..Default::default()
            },
        },
        MemorySnapshot {
            label: "B".into(),
            counts: incular_devtools_protocol::ResourceCounts {
                elements: 13,
                rss_mb: 104,
                ..Default::default()
            },
        },
    );
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
    assert!(model.deep_trace_count() <= InspectorModel::FRAME_HISTORY);
    assert!(model.deep_trace_event_count() <= InspectorModel::MAX_TRACE_EVENTS);
    assert_eq!(
        model.deep_trace_event_count(),
        model.deep_trace_event_total()
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
    state.collapse_all();
    state.reveal(id(3));
    assert!(state.is_expanded(id(1)));
    assert!(state.is_expanded(id(2)));
    assert_eq!(state.row_count(), 3);
}

#[test]
fn devtools_shell_tabs_remain_hittable_above_the_scrolling_body() {
    let active = Signal::new(ToolView::Widgets);
    let observed = active.clone();
    let scroll = ScrollController::new();
    let root: Widget = LayoutBuilder::new(move |constraints| {
        let width = constraints.max_width.max(960.);
        let height = constraints.max_height.max(640.);
        let body_height = (height - 116.).max(1.);
        let mut tabs = Vec::new();
        for (view, label) in [
            (ToolView::Widgets, "Widgets"),
            (ToolView::Console, "Console"),
            (ToolView::Network, "Network"),
            (ToolView::Performance, "Performance"),
            (ToolView::Memory, "Memory"),
            (ToolView::Application, "Application"),
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
        SizedBox::from_size(Size::new(width, height))
            .child(
                DecoratedBox::new(Padding::all(
                    16.,
                    Column::new([
                        Row::new(tabs).spacing(6.).into(),
                        gap(1., 8.),
                        ui_text("Incular DevTools", 20., TEXT_PRIMARY),
                        gap(1., 12.),
                        ConstrainedBox::new(
                            Constraints::tight(Size::new(width - 32., body_height)),
                            body,
                        )
                        .into(),
                    ])
                    .cross_axis_alignment(CrossAxisAlignment::Start),
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
        position: Offset::new(310., 32.),
    });
    let up = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Up,
        position: Offset::new(310., 32.),
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
