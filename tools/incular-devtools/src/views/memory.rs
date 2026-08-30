use super::{TEXT_MUTED, TEXT_PRIMARY, compact_button, gap, section, ui_text};
use crate::transport::ClientBridge;
use incular::prelude::*;
use incular_devtools_protocol::{MemorySnapshot, RequestMethod};

pub(crate) fn build_memory(
    memory: Option<MemorySnapshot>,
    memory_diff: Vec<String>,
    bridge: ClientBridge,
) -> Widget {
    let mut controls = Vec::new();
    let mut body = Vec::new();
    let capture_bridge = bridge.clone();
    controls.push(compact_button("Capture snapshot", false, move || {
        capture_bridge.send(RequestMethod::TakeMemorySnapshot {
            label: "manual".into(),
        })
    }));
    let baseline_bridge = bridge.clone();
    controls.push(compact_button("Set baseline A", false, move || {
        baseline_bridge.send(RequestMethod::TakeMemorySnapshot { label: "A".into() })
    }));
    let compare_bridge = bridge;
    controls.push(compact_button("Compare snapshot B", false, move || {
        compare_bridge.send(RequestMethod::TakeMemorySnapshot { label: "B".into() })
    }));

    if let Some(memory) = memory {
        let counts = memory.counts;
        body.push(ui_text(
            format!("{} · RSS {} MB", memory.label, counts.rss_mb),
            14.,
            TEXT_PRIMARY,
        ));
        body.extend(
            [
                format!(
                    "Elements {} · render objects {} · layers {}",
                    counts.elements, counts.render_objects, counts.layers
                ),
                format!(
                    "Semantics {} · signals {} · active tasks {}",
                    counts.semantics_nodes, counts.signals, counts.tasks_active
                ),
                format!(
                    "Glyph atlas pages {} · images {} · gradients {} · paths {}",
                    counts.glyph_atlas_pages,
                    counts.image_resources,
                    counts.gradient_resources,
                    counts.path_meshes
                ),
                format!(
                    "Offscreen {} B · effect cache {} B",
                    counts.offscreen_bytes, counts.effect_cached_bytes
                ),
            ]
            .into_iter()
            .map(|line| ui_text(line, 13., TEXT_MUTED)),
        );
    }
    body.extend(
        memory_diff
            .into_iter()
            .map(|line| ui_text(line, 13., TEXT_MUTED)),
    );
    if body.is_empty() {
        body.push(ui_text(
            "Capture a snapshot to inspect the framework inventory.",
            13.,
            TEXT_MUTED,
        ));
    }

    Column::new([
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
                Wrap::new(controls).spacing(8.).run_spacing(8.).into(),
                gap(1., 12.),
                Column::new(body).into(),
            ])
            .into(),
        ),
    ])
    .into()
}
