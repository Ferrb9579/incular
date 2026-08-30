use super::{
    DANGER, PRIMARY, SUCCESS, TEXT_MUTED, TEXT_PRIMARY, compact_button, gap, section, ui_text,
};
use crate::{
    inspector::{InspectorSection, Shared, debug_value, editable_value, parse_debug_value},
    transport::ClientBridge,
};
use incular::controls::TextField;
use incular::material::RawMaterialButton;
use incular::prelude::*;
use incular::widgets::internal::TextEditingController;
use incular::widgets::internal::icons;
use incular_devtools_protocol::{
    DebugOption, DevWidgetId, DevWindowId, NodeDetails, RequestMethod, SignalSubscriber,
    SignalSummary, WindowSummary,
};
use std::{cell::RefCell, collections::HashSet, rc::Rc, sync::Arc};

pub(crate) struct InspectorData {
    pub(crate) error: Option<String>,
    pub(crate) windows: Vec<WindowSummary>,
    pub(crate) active_window: Option<DevWindowId>,
    pub(crate) select_mode: bool,
    pub(crate) debug_options: HashSet<DebugOption>,
    pub(crate) animation_scale: f32,
    pub(crate) details: Vec<String>,
    pub(crate) selected_details: Option<NodeDetails>,
    pub(crate) has_selection: bool,
    pub(crate) signals: Vec<SignalSummary>,
    pub(crate) selected_signal: Option<SignalSummary>,
    pub(crate) signal_subscribers: Vec<SignalSubscriber>,
    pub(crate) row_count: usize,
}

pub(crate) struct InspectorWidgets {
    pub(crate) content: Widget,
    pub(crate) search_field: Widget,
    pub(crate) tree_list: Widget,
}

pub(crate) struct InspectorBuildContext {
    pub(crate) shared: Shared,
    pub(crate) bridge: ClientBridge,
    pub(crate) tick: Signal<u64>,
    pub(crate) search: TextEditingController,
    pub(crate) signal_value: TextEditingController,
    pub(crate) property_value: TextEditingController,
    pub(crate) property_binding: Rc<RefCell<Option<(DevWidgetId, String)>>>,
    pub(crate) inspector_section: Signal<InspectorSection>,
    pub(crate) tree_scroll: ScrollController,
}

pub(crate) fn build_inspector(
    data: InspectorData,
    context: InspectorBuildContext,
) -> InspectorWidgets {
    let InspectorBuildContext {
        shared,
        bridge,
        tick,
        search,
        signal_value,
        property_value,
        property_binding,
        inspector_section,
        tree_scroll,
    } = context;
    let InspectorData {
        error,
        windows,
        active_window,
        select_mode,
        debug_options,
        animation_scale,
        details,
        selected_details,
        has_selection,
        signals,
        selected_signal,
        signal_subscribers,
        row_count,
    } = data;
    let mut inspector_controls = Vec::new();
    let mut debug_controls = Vec::new();
    let mut animation_controls = Vec::new();
    let mut signal_body = Vec::new();

    if let Some(error) = error.as_ref() {
        inspector_controls.push(ui_text(format!("Connection error: {error}"), 13., DANGER));
    }
    for window in windows {
        let window_bridge = bridge.clone();
        let window_shared = Arc::clone(&shared);
        let window_tick = tick.clone();
        inspector_controls.push(compact_button(
            format!(
                "{} · {:.0}×{:.0}",
                window.title, window.logical_size[0], window.logical_size[1]
            ),
            active_window == Some(window.id),
            move || {
                if let Ok(mut state) = window_shared.lock() {
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
                window_bridge.send(RequestMethod::GetWidgetTree { window: window.id });
                window_tick.update(|value| *value = value.wrapping_add(1));
            },
        ));
    }
    let selection_bridge = bridge.clone();
    let selection_shared = Arc::clone(&shared);
    let selection_tick = tick.clone();
    inspector_controls.push(
        RawMaterialButton::new(if select_mode {
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
            let selection = selection_shared.lock().ok().and_then(|mut state| {
                state.select_mode = !state.select_mode;
                state
                    .active_window
                    .map(|window| (window, state.select_mode))
            });
            if let Some((window, enabled)) = selection {
                selection_bridge.send(if enabled {
                    RequestMethod::StartInspectMode { window }
                } else {
                    RequestMethod::StopInspectMode { window }
                });
            }
            selection_tick.update(|value| *value = value.wrapping_add(1));
        })
        .into(),
    );
    let clear_bridge = bridge.clone();
    let clear_shared = Arc::clone(&shared);
    let clear_tick = tick.clone();
    inspector_controls.push(compact_button("Clear selection", false, move || {
        let window = clear_shared.lock().ok().and_then(|mut state| {
            state.selected = None;
            state.hovered = None;
            state.details = None;
            state.active_window
        });
        if let Some(window) = window {
            clear_bridge.send(RequestMethod::HighlightNode { window, id: None });
        }
        clear_tick.update(|value| *value = value.wrapping_add(1));
    }));
    let collapse_shared = Arc::clone(&shared);
    let collapse_tick = tick.clone();
    inspector_controls.push(compact_button("Collapse tree", false, move || {
        if let Ok(mut state) = collapse_shared.lock() {
            state.collapse_all();
        }
        collapse_tick.update(|value| *value = value.wrapping_add(1));
    }));
    let expand_shared = Arc::clone(&shared);
    let expand_tick = tick.clone();
    inspector_controls.push(compact_button("Expand tree", false, move || {
        if let Ok(mut state) = expand_shared.lock() {
            state.expand_all();
        }
        expand_tick.update(|value| *value = value.wrapping_add(1));
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
        let option_bridge = bridge.clone();
        let option_shared = Arc::clone(&shared);
        let option_tick = tick.clone();
        let enabled = debug_options.contains(&option);
        debug_controls.push(compact_button(label, enabled, move || {
            let enabled = if let Ok(mut state) = option_shared.lock() {
                if !state.debug_options.insert(option) {
                    state.debug_options.remove(&option);
                    false
                } else {
                    true
                }
            } else {
                false
            };
            option_bridge.send(RequestMethod::SetDebugOption {
                name: option,
                enabled,
            });
            option_tick.update(|value| *value = value.wrapping_add(1));
        }));
    }
    for (scale, label) in [
        (1., "Animations 1×"),
        (0.5, "Animations 0.5×"),
        (0.25, "Animations 0.25×"),
        (0.1, "Animations 0.1×"),
        (0., "Pause animations"),
    ] {
        let animation_bridge = bridge.clone();
        animation_controls.push(compact_button(label, scale == animation_scale, move || {
            animation_bridge.send(RequestMethod::SetAnimationSpeed { scale })
        }));
    }

    let signals_bridge = bridge.clone();
    signal_body.push(compact_button("Refresh signal list", false, move || {
        signals_bridge.send(RequestMethod::ListSignals)
    }));
    for signal in signals {
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
        let signal_bridge = bridge.clone();
        let signal_shared = Arc::clone(&shared);
        let signal_tick = tick.clone();
        signal_body.push(compact_button(
            label,
            selected_signal
                .as_ref()
                .is_some_and(|item| item.id == signal.id),
            move || {
                if let Ok(mut state) = signal_shared.lock() {
                    state.selected_signal = Some(signal.id);
                    state.signal_subscribers.clear();
                }
                signal_bridge.send(RequestMethod::GetSignalSubscribers { id: signal.id });
                signal_tick.update(|value| *value = value.wrapping_add(1));
            },
        ));
    }
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
            let signal_bridge = bridge.clone();
            let signal_tick = tick.clone();
            signal_body.push(
                TextField::new(signal_value)
                    .placeholder("New signal value; press Enter")
                    .on_submit(move |input| {
                        if let Some(value) = editable_value(&signal, &input) {
                            signal_bridge.send(RequestMethod::EditSignal {
                                id: signal.id,
                                value,
                            });
                            signal_tick.update(|value| *value = value.wrapping_add(1));
                        }
                    })
                    .into(),
            );
        }
    }
    if signal_body.len() == 1 {
        signal_body.push(ui_text(
            "No debug-enabled signals are registered by this target.",
            13.,
            TEXT_MUTED,
        ));
    }

    let search_field: Widget = TextField::new(search)
        .placeholder("Filter widget tree; press Enter")
        .on_submit({
            let search_shared = Arc::clone(&shared);
            let search_tick = tick.clone();
            move |query| {
                if let Ok(mut state) = search_shared.lock() {
                    state.search = query;
                    state.rebuild_rows();
                }
                search_tick.update(|value| *value = value.wrapping_add(1));
            }
        })
        .into();
    let list_shared = Arc::clone(&shared);
    let list_bridge = bridge.clone();
    let list_tick = tick.clone();
    let tree_list: Widget = CustomScrollView::new(vec![Box::new(SliverFixedExtentList::new(
        row_count,
        32.,
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
            let Some((label, row, selected, hovered, expanded, has_children)) = snapshot else {
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
                RawMaterialButton::new(if expanded { "Collapse" } else { "Expand" })
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
            let selection: Widget = RawMaterialButton::new(label)
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
    )) as Box<dyn Sliver>])
    .controller(tree_scroll)
    .into();

    let mut property_editor = Vec::new();
    if inspector_section.get() == InspectorSection::Properties
        && let Some(details) = selected_details.as_ref()
        && let Some(property) = details.properties.iter().find(|property| property.editable)
    {
        let binding = (details.id, property.name.clone());
        if property_binding.borrow().as_ref() != Some(&binding) {
            property_value.set_text(debug_value(&property.value));
            *property_binding.borrow_mut() = Some(binding);
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
        let property_bridge = bridge.clone();
        let property_tick = tick.clone();
        let id = details.id;
        let name = property.name.clone();
        let template = property.value.clone();
        property_editor.push(
            TextField::new(property_value)
                .placeholder("Enter a value and press Enter")
                .on_submit(move |input| {
                    if let Some(value) = parse_debug_value(&template, &input) {
                        property_bridge.send(RequestMethod::EditProperty {
                            id,
                            name: name.clone(),
                            value,
                        });
                        property_tick.update(|value| *value = value.wrapping_add(1));
                    }
                })
                .into(),
        );
        if property.overridden {
            let reset_bridge = bridge.clone();
            property_editor.push(compact_button(
                "Reset property overrides",
                false,
                move || reset_bridge.send(RequestMethod::ResetOverrides),
            ));
        }
        property_editor.push(gap(1., 10.));
    } else if inspector_section.get() == InspectorSection::Properties && selected_details.is_some()
    {
        property_editor.push(ui_text(
            "This widget exposes read-only retained properties.",
            12.,
            TEXT_MUTED,
        ));
        property_editor.push(gap(1., 8.));
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
            let section_signal = inspector_section.clone();
            detail_tabs.push(compact_button(
                label,
                inspector_section.get() == section,
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
    let content = Column::new([
        ui_text("Widgets", 22., TEXT_PRIMARY),
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
        gap(1., 12.),
        section(
            "Signals",
            "Inspect writes, subscribers, and explicitly editable debug values",
            Column::new(signal_body).into(),
        ),
    ])
    .into();

    InspectorWidgets {
        content,
        search_field,
        tree_list,
    }
}
